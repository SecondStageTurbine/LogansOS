<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-032: Phase 6 BUILD-4 — Intel Gen9.5 GPU compute driver (substrate)

| | |
|---|---|
| **Status** | Accepted — Phase 6 BUILD-4 (Intel Gen9.5 GPU compute) shipped; Phase 6 closed 2026-06-07. The C1 legacy-ring→execlist follow-on was resolved in **Phase 7 BUILD-4X** (GuC-free execlist/LRC on silicon, `7626f62`). *(Status corrected 2026-06-17: was "Proposed", stale post-close.)* |
| **Date** | 2026-06-05 |
| **Authority** | PM/Cerebro + PL |
| **Related** | `Plans/THINK-2026-06-05-phase-6-build-4-real-gpu.md` (the research + recommendation this ratifies), ADR-030 (Phase 6 substrate / AccelDriver trait), ADR-025 (typed MMIO/PIO wrappers), ADR-023 (DRM-expressibility forward-marker), ANSPEC-001 §9; bible Articles 2.2/2.3 (kernel-managed accelerators), 4.1 (firmware-blob reality), 7.5 (tensor memory) |

## Context

BUILD-1–3 built and CI-proved the accelerator *abstraction and transport* (accel_cpu → accel_sim → virtio-accel). BUILD-4 is the real-silicon payoff: kernel-managed tensor compute on an actual GPU. Per the THINK pass, the target is fixed by a fortunate alignment — the bare-metal box (OptiPlex 7080) carries an **Intel UHD 630 (Gen9.5 iGPU)**, which is simultaneously the hardware in hand, the best-publicly-documented GPU (open PRMs + Gen9 compute whitepaper + open EU ISA), and the simplest memory model (GTT-mapped system RAM, no discrete VRAM). This ADR fixes the bring-up decisions; the EU-command and GTT byte layouts are **Intel-defined (PRM)**, so our discipline is to assert the bytes we emit against hand-computed PRM values, not to invent a format.

This is the most hardware-dependent, **least CI-protected** build in the project. Decisions below are split into **Decided** and **Confirm-at-BUILD** (things that need the PRM/silicon to settle — flagged honestly rather than guessed).

## Decisions (decided)

### D1 — Target & driver
Intel **Gen9.5** iGPU (UHD 630) on the OptiPlex 7080. Driver `kernel/src/drivers/accel/accel_intel.rs`, **compute-only** (no display/modeset/3D). Registers as an `AccelDriver` (ADR-030 trait) on `Device::Accel(n)`, behind a **default-off Cargo feature `accel-intel`**, and **only when the iGPU is probed present** — so CI (which never enables it) is unaffected.

### D2 — GuC-free submission
Submit compute via a **GuC-free path** so we take no GuC/HuC firmware-load dependency in the first cut. The Gen9 GPGPU pipeline (`MEDIA_VFE_STATE` → `MEDIA_INTERFACE_DESCRIPTOR_LOAD` → optional `MEDIA_CURBE_LOAD` → `GPGPU_WALKER`) is emitted into a command buffer on the render engine. *(Which exact GuC-free mechanism — legacy ringbuffer vs direct execlist/ELSP submission — is Confirm-at-BUILD C1.)*

### D3 — Memory = GTT-mapped system RAM
`DeviceStorage` for `accel_intel` wraps a kernel-owned system-RAM region mapped into the GPU's graphics address space via the **GTT**. No discrete VRAM allocator. CPU↔GPU visibility relies on Gen9's LLC-coherent paths **plus explicit pipe-control flushes**; correctness here is C2. `copy_to/from_device` are host↔(GTT-mapped buffer) memcpys; a tensor migrated to `Accel(n)` lives in GTT-mapped memory.

### D4 — EU kernels are compiled offline, embedded as provenance-pinned blobs
We do **not** hand-assemble Gen ISA. Compute kernels are written once, compiled **offline** with the open Intel toolchain (`intel-graphics-compiler`/IGA/vISA) to Gen9 binaries, and embedded as `const` blobs. The kernel *source*, the *exact pinned compiler version*, and the *build recipe* are checked in so a blob's provenance is reproducible (not an opaque artifact). This is a build-time tool dependency, not a kernel runtime dependency (Article 4.3 unaffected).

**Finalizer lane closed (2026-06-06).** The pinned finalizer now exists: `tools/intel-gen9-finalizer/` (IGC `igc-1.0.17193.4` + compute-runtime `24.26.30049.6`, Skylake/Kaby Lake = Production) deterministically compiles the checked-in OpenCL-C sources with `ocloc` and extracts the bare `.text.<kernel>` EU stream. `ADD_F32X2_KERNEL_GEN9` (640 B, real f32 add) and `NOOP_KERNEL_GEN9` (192 B, EOT-only) are now finalizer-built with `ocloc disasm` as the independent byte oracle (`intel_gen9_kernels/*.gen9.asm`); full provenance + the substitution flags (`.visa`→OpenCL-C since no standalone vISA finalizer/`iga64` ships prebuilt) are in `intel_gen9_kernels/README.md`. **The Add ships stateless A64** (`-cl-intel-greater-than-4GB-buffer-required`): it accesses its buffers via stateless A64 messages with the three base pointers passed in the cross-thread payload (offsets 32/40/48; the "Cross-thread argument ABI" table in the kernel README is the contract iter7 fills) — **no binding table / `SURFACE_STATE` / Surface State Base**. Stateful BTI surface access is deferred to a later hardening build (the eventual ADR-023 DRM-expressible path).

### D5 — Completion: poll a seqno first
Signal completion with a `PIPE_CONTROL` post-sync write of a sequence number to a known memory location; the driver **polls** it first (simplest, deterministic for bring-up), with the GPU interrupt as a later optimization. `wait()` honors the ADR-030 contract (drive to completion / fail; bounded, no tight unyielding spin).

### D6 — Equivalence is within-tolerance, not byte-exact
The GPU uses hardware FP; `accel_cpu` uses soft-float — so the oracle is **`accel_intel ≈ accel_cpu` within a defined per-op f32 tolerance** (ANSPEC §9.5 already allows this), with exact-integer ops (e.g. Embedding gather) held **byte-exact**. The tolerance policy is C3 and must be justified, not hand-waved.

### D7 — Command bytes asserted against the PRM (the oracle, adapted)
The formats (GTT entries, MI/GPGPU command packets, interface descriptors) are Intel's. CLAUDE.md's independent-oracle intent applies as: **the command-buffer bytes the driver emits are asserted against hand-computed, PRM-cited expected values** in a host test — so we don't rely solely on "the silicon accepted it." (Bytes we *can* check off-hardware; semantics need the box.)

### D8 — Framebuffer is expendable during a GPU run
The iGPU also drives the Limine console — which is part of our evidence channel. Decision: **do not modeset or fight the scanout**; treat the framebuffer as **expendable** during a compute run, and make the **block-device log sink** the authoritative evidence path (as in Phase 3/4 bare-metal). Whether we must quiesce the scanout engine to safely drive compute is C4.

### D9 — Validation is bare-metal evidence, not CI
BUILD-4's gate is a **captured bare-metal run** on the OptiPlex (`--bare-metal-mode`, serial + block-device log) reviewed by PM/PL, with the D6 tolerance oracle. There is **no CI scenario** for the GPU path; the CI suite must remain green with `accel-intel` off.

### D10 — DRM-expressibility + firmware stance
The driver must stay **expressible** as a Linux-DRM-compatible char device for the future personality (ADR-023) — an interface constraint, not an internal structure. Firmware: Gen9 GuC-free per D2; if the silicon forces any firmware blob, it is surfaced explicitly (Article 4.1 acknowledges accelerator firmware as a reality — but we do not bury it).

## Confirm-at-BUILD resolutions (BUILD-4a Stage A; Stage B validates silicon)
- **C1** — BUILD-4a uses the GuC-free **legacy render ring** path. The first Stage-B OptiPlex run proved probe + registration + submit (`00:02.0`, device `0x9bc8`, ring `0x100000`, seqno GGTT `0x133000`) but timed out waiting for the seqno with no panic, page fault, GPU fault, or reset. Iteration 2 then made the Gen9.5 legacy-ring requirements explicit (render forcewake, masked `RING_MODE`, `RING_CTL` verification, and GT/ring status logging), but the Stage-B run hard-hung before any accel-intel forcewake/ring line reached the persisted NVMe log. Iteration 3 keeps the legacy-ring decision and adds per-step breadcrumb flushes around `mmio-map`, `gt-bar-mapped`, forcewake polls/writes, ring-mode setup, submit, wait, and named status reads so the next Stage-B run records the last surviving MMIO step. If `RING_HEAD` still does not advance under forcewake with execlists disabled, the next PM decision is the execlist/ELSP path; the driver still does not fall back to CPU or silently switch mechanisms.
- **C2** — BUILD-4a maps kernel-owned, physically contiguous `DmaBuffer`s into GGTT addresses by programming GTTMMADR PTEs for the ring, state heap, EU kernel, two inputs, output, and seqno page. The fixed ordering is: CPU fills buffers, GGTT PTEs are installed as present/LLC-backed system-memory mappings, the command stream starts with a flush/stall `PIPE_CONTROL`, the walker runs the Add kernel, and a post-sync `PIPE_CONTROL` writes the completion seqno before CPU readback. Iteration 3 treats the iter2 hard-hang as a GT-MMIO observability and mapping failure: the driver maps the full 16 MiB GTTMMADR BAR before GT access, validates the high-offset registers (`FORCEWAKE_ACK_GT_GEN9 = 0x130044`, `GEN6_GT_CORE_STATUS = 0x138060`) and every GGTT PTE offset against that mapped length, logs `mmio-skip` instead of faulting on any out-of-range access, and keeps forcewake/status polls finite with timeout logging. Stage B validates the sequence on silicon.
- **C3** — BUILD-4a's Add oracle is **byte-exact** (`ADD_RESULT_TOLERANCE_ULP = 0`): `[1.0, 2.0] + [3.0, 4.0]` is finite IEEE-754 addition with exactly representable inputs and outputs (`[4.0, 6.0]`). BUILD-4b may define wider tolerances for nontrivial hardware-FP ops; integer/exact data-movement ops remain byte-exact.
- **C4** — BUILD-4a treats the framebuffer as expendable and does **not** modeset or quiesce scanout. The authoritative evidence path is the NVMe block-device log sink; iteration 3 requires each accel-intel breadcrumb to flush immediately so a hard hang leaves a final persisted marker. The probe accepts the Gen9 GT2 candidate PCI IDs `0x1912` (HD 530, Skylake Gen9 — the Lenovo M710q), `0x9bc8`/`0x9bc5` (UHD 630, Comet Lake Gen9.5), and `0x3e92`/`0x3e98` (UHD 630, Coffee Lake-S Gen9.5); the Stage-B log must record the exact device ID and bound SKU and prove that the iGPU is enabled. Every accepted ID is a **`1x3x8` GT2 part (24 EU)** — the property that makes the single hardcoded `MEDIA_VFE_STATE` max-thread constant (24 EU × 7 = 168) correct for all of them; a non-GT2 Gen9 part must not be added without making that constant per-SKU first.

**Iteration 4 resolution update.** Iteration 4 adds a ring-only bisector before
the Add compute batch: the scenario submits only a `PIPE_CONTROL` post-sync
seqno store on the same legacy ring and logs `smoke-seqno=0x...`. If that smoke
seqno times out, the next build is localized to ring execution. If the smoke
seqno lands and the Add batch still times out, the next build is localized to
compute dispatch/EU retirement. Iteration 4 also logs GGTT PTE values (label,
GGTT address, physical address, flags, final PTE, and MMIO offset), the ring
batch byte offset, interface-descriptor fields, and pre-kick/post-kick/mid-wait
/timeout status samples. The source-level Add kernel terminator is the vISA
`ret (M1, 1)`; the standard Neo container lacks the Intel finalizer/disassembler,
so Stage A keeps the embedded blob stable and asserts the existing pinned tail
bytes while Stage B decides whether a real EOT re-finalization pass is needed.

**Iteration 5 resolution update (the A/B bisect).** Iter4's OptiPlex run was
decisive about what *works*: legacy-ring execution, `PIPE_CONTROL`, GGTT seqno
(`smoke-seqno=0x2`), both forcewake ACKs, the Add blob's pinned EOT tail
(`pinned_tail_match=true`), and the interface descriptor (`kernel_ptr_matches=true`,
`kernel_start=0x120000`, lhs/rhs/output set, `thread_count=1 simd_width=1`). The
**wall** is inside EU thread execution: the Add walker dispatches but never retires
— `RING_HEAD=ACTHD=0x50` parked past the walker, `seqno=0`, `INSTDONE` shifts
`0xfffffffe` (smoke, near-idle) → `0xffdeffff` (compute, units parked),
`RING_FAULT=0`. Two suspects remain: **(A)** dispatch mechanics
(`MEDIA_VFE_STATE` / interface descriptor / `GPGPU_WALKER`) or **(B)** the Add
kernel's EU data-port access (its `lhs`/`rhs` read or `output` write faults/hangs
because data-port addressing — stateless A64/A32 vs stateful surface state, PPGTT
vs GGTT, MOCS — is unset even though the command streamer reaches GGTT fine).

Iter5 separates (A) from (B) with a **no-op (EOT-only) EU kernel** that performs
**no memory access**, dispatched through the *same*
VFE/interface-descriptor/`GPGPU_WALKER` path as Add (only the kernel page's blob
differs). It is hand-laid directly from the Gen8/Gen9 EU ISA (a `mov g127, g0`
header copy then a `send` to the Thread Spawner with EOT set, mlen 1, rlen 0), so
it needs no Intel finalizer. The load-bearing control words (opcode/ExecSize, the
send's SFID at instruction bits 27:24, and the EOT bit at instruction bit 127 with
mlen/rlen) are derived from named bitfields and re-asserted by an independent
host byte oracle in `tools/ai-ops-tests`; the register-region sub-fields are
hand-laid per the Gen8 null-EOT-kernel encoding, with Stage-B silicon (and iter6's
finalizer if needed) as the authority. The bisect verdict is read from the
Stage-B log:

- **no-op retires** (`noop-kernel-seqno` lands) → dispatch mechanics are correct;
  the Add hang is **(B)** its kernel body's EU data access → iter6 = EU data-port
  addressing (surface state / stateless / PPGTT / MOCS).
- **no-op also hangs** → the dispatch setup is **(A)** the bug → iter6 = VFE /
  walker / descriptor config, independent of the kernel body.

Iter5 also widens the stuck-unit decode (ISC-2): beyond `RING_INSTDONE` it samples
`SC_INSTDONE` (0x7100), `SAMPLER_INSTDONE` (0xe160), `ROW_INSTDONE` (0xe100), and
`TD_CTL` (0xe400), and decodes which `RING_INSTDONE` bits are *clear* (a clear bit
= that unit reports not-done/busy). The iter4 compute value `0xffdeffff` has busy
mask `0x00210000` — bits 16 and 21 (two EU-area units parked past the walker) —
versus the smoke value `0xfffffffe` (busy mask `0x1`, the command streamer alone).
The ring smoke and Add compute paths are unchanged; the no-op is a third sub-mode
with per-mode distinct log reasons.

**Iteration 5 Stage-B result — bisect verdict: SUSPECT A (dispatch setup).** The
OptiPlex run (`bd15863`; evidence
`Plans/evidence/phase6-build4a-iter5-optiplex-20260606-accel-intel-bisect.md`,
extracted-log SHA256 `3652128c…07eaef`) was decisive: the **no-op EOT-only kernel
also hung**, parking at the byte-identical signature as Add (`RING_HEAD=ACTHD=0x50`,
`INSTDONE=0xffdeffff`, `seqno=0`, `noop-kernel-seqno=0x0`,
`bisect=dispatch_setup_suspect`). Because the no-op performs no memory access, this
**eliminates suspect (B)** (EU data-port / surface state / stateless / PPGTT / MOCS).
The new EU decode localizes it: `SC_INSTDONE`/`SAMPLER_INSTDONE` both `0xffffffff`
(idle); only row-0 EU thread-dispatch (bit 16) and instruction-fetch (bit 21) stuck;
`TD_CTL=0`, `RING_FAULT=0` — the thread reaches the EU but its instruction fetch
never completes. iter6 therefore targets the dispatch setup (`MEDIA_VFE_STATE`
thread-pool/URB/scratch, `GPGPU_WALKER` params, interface-descriptor kernel pointer),
**not** the kernel data path, and likely needs the real Intel finalizer (IGC/ocloc/
vISA) added to the toolchain so a known-good EOT confirms (A) and rules out the
residual "both blobs malformed" reading — a separate image task for Prometheus, not
pulled in here.

**Iteration 6 resolution — STATE_BASE_ADDRESS (the SUSPECT-A fix).** iter5's
signature (thread dispatched to EU row-0, instruction fetch stuck at INSTDONE bit
21, SC/SAMPLER idle, `RING_FAULT=0`) is the textbook "the EU cannot fetch the
kernel's instructions" case: on Gen9 the EU fetches code from **Instruction Base
Address + the descriptor's kernel-start pointer**, and the interface descriptor
itself is read from **Dynamic State Base Address** — and the pre-iter6 batch never
issued `STATE_BASE_ADDRESS`, so those bases were unset and the kernel pointer
resolved to nowhere. iter6 emits a PRM-correct 19-DWord Gen9 `STATE_BASE_ADDRESS`
(header `0x61010011`) before the walker, with every base = 0 (absolute GGTT) and all
four buffer-size bounds at max (`0xFFFFF001`), plus a post-SBA cache-invalidate
`PIPE_CONTROL` so the EU fetches from the new Instruction Base. The rest of the
dispatch path was re-verified against the PRM (Mesa `gen9.xml` / i965): the
`GPGPU_WALKER` is corrected from the Gen7 13-DWord form to **Gen9's 15-DWord form
(`0x7105000d`) with the SIMD-size field set to SIMD32** (both kernels compile
SIMD32/128 GRF per their zeinfo); `MEDIA_VFE_STATE` gets `NumberofURBEntries = 2` /
`URBEntryAllocationSize = 2` (Gen8+); `PIPELINE_SELECT` is the masked GPGPU select
`0x69040302`; `MEDIA_INTERFACE_DESCRIPTOR_LOAD` length is one 32-byte IDD; and the
interface descriptor is now a real Gen9 IDD (Kernel Start Pointer + Number of
Threads in Thread Group = 1, no sampler / no binding table / no CURBE) instead of
the earlier placeholder that stuffed lhs/rhs/output GGTT pointers into the
FP-mode / sampler / binding-table DWords. General State Base = 0 is left in place so
iter7's stateless-A64 Add can add its cross-thread pointer payload (offsets a=32 /
b=40 / o=48) and a `MEDIA_CURBE_LOAD` without reworking the bases. The
command-byte oracle in `tools/ai-ops-tests` is re-laid to the new 69-DWord batch.
iter6's success metric is **the finalizer no-op retires** (`noop-kernel-seqno`
lands → dispatch fixed); the scenario still FAILs on the Add until iter7 fills the
arg payload, so iter6 does not merge alone.

**Iteration 6 Stage-B result — dispatch_ok / SUSPECT B (Add EU data access).** The
OptiPlex run (2026-06-07, evidence
`Plans/evidence/phase6-build4a-iter6-optiplex-20260607-state-base-address.md`,
extracted-log sha256 `a8bcd0cb744b8ecde7ee7e70110c758ddcbed480cec133126fef9885ffd38d0a`)
confirms the fix: the finalizer no-op **retired**
(`noop-kernel complete … noop-kernel-seqno=0x00000003`,
`accel_intel_bisect=noop_retired verdict=dispatch_ok suspect=B_add_eu_data_access`).
The load-bearing proof is independent of the seqno read — the no-op's EU INSTDONE
returns to idle `0xfffffffe` (no stuck EU bits), versus iter5's stuck `0xffdeffff`
(bits 16/21); `RING_HEAD` advances to `RING_TAIL` (`0x110`) and `RING_FAULT=0`. An
EU cannot go from stuck-at-fetch to idle without fetching and executing the kernel
to its `{EOT}` → `STATE_BASE_ADDRESS` resolved the instruction fetch, settling
SUSPECT A. The Add now fails differently: it fetches and spins up the EU (INSTDONE
bits 17 + 21) then raises `RING_FAULT=0x000009e1` (bit 0 Valid, type bits[2:1]=0 →
**invalid PTE / page not present**) with `seqno=0`. This is the predicted SUSPECT-B
signature: the stateless-A64 Add dereferences `lhs`/`rhs`/`output` pointers that
arrive in a cross-thread payload iter6 does not yet load (no `MEDIA_CURBE_LOAD`,
General State Base = 0), so the A64 gather/scatter touches an unmapped VA. iter7
fills the payload (a=32 / b=40 / o=48 + `MEDIA_CURBE_LOAD` + General State Base data)
so the Add retires `[4,6]`. One cosmetic caveat logged in the evidence: the no-op
`pre-kick` read `seqno=0x3` (target value) before the kick while the Add correctly
read `0x0` on the same GGTT page — not load-bearing (the verdict rests on the
EU-idle INSTDONE transition), but worth a driver-code check on no-op seqno-page
seeding.

**Iteration 7 — stateless-A64 argument payload (the BUILD-4a→main trigger build).**
iter7 wires the cross-thread (CURBE) payload the iter6 SUSPECT-B fault demanded.
The finalizer Add reads its three buffer base pointers as raw A64 addresses from
the GPGPU cross-thread payload — `add_f32x2.gen9.asm` reads them from
`r8.0/r8.1/r8.2` and the per-lane `local_id.x` from `r1/r2`, i.e. the thread
payload is `r0` header + 6 GRF per-thread `local_id` (`r1..r6`) + 3 GRF
cross-thread (`r7..r9`), so `a@byte32 → r8.0`, `b@byte40 → r8.1q`, `o@byte48 →
r8.2q`. The driver now: (1) fills a 96-byte cross-thread payload (`global_id_offset`
0,0,0; `local_size`/`enqueued_local_size` 2,1,1; the three little-endian A64
pointers = the GGTT bases `0x130000/0x131000/0x132000` at bytes 32/40/48) into a new
GGTT page `curbe_base = 0x134000`; (2) emits `MEDIA_CURBE_LOAD` (header `0x70010002`,
128-byte 64-aligned total length, data start = `curbe_base`) after `MEDIA_VFE_STATE`,
raising the VFE CURBE Allocation Size to 4 (`ALIGN(3 GRF, 2)`); (3) sets the IDD
Cross-Thread Constant Data Read Length (DW7[7:0]) = 3 GRF so the dispatcher copies
the pointers into the thread; (4) trims the `GPGPU_WALKER` right/bottom execution
masks to `0x3` (the 2 live SIMD32 lanes) so the 30 idle lanes issue no stray A64
scattering writes. The byte layout is independently oracle'd: a hand-laid 96-byte
payload fixture (offsets cross-checked against `add_f32x2.gen9.zeinfo`) and a re-laid
292-byte command oracle spot-asserting the `MEDIA_CURBE_LOAD` header/length/address,
the VFE CURBE alloc, and the trimmed walker masks. General State Base stays 0:
stateless A64 addresses are absolute, so loading the payload — not a GSB — is the
fix. Stateless A64 is kept (no BTI/SURFACE_STATE; ADR-032 D4 hardening deferred).

The no-op bisector is preserved byte-for-byte (`encode_noop_batch`, 69 DWords, no
CURBE, all-lanes masks, IDD cross-thread length 0): trimming the SIMD masks would
truncate the no-op's `r0→r127` header copy and risk its EOT, so iter7 splits the
Add batch (`encode_add_batch`, 73 DWords) from the no-op rather than growing a shared
one. A retiring no-op therefore still independently proves the dispatch path.

**Iteration 7 instrumentation (ISC-5a/5b) — fault address capture, so the Stage-B
run is conclusive either way.** The GT snapshot now reads the Gen8/9 fault-address
pair (`GEN8_FAULT_TLB_DATA0`=`0x4b10`, `DATA1`=`0x4b14`) alongside `RING_FAULT`, and
on any valid fault logs `ring-fault-decode`: the fault type/srcid, the GTT-vs-PPGTT
select (DATA1 bit 4), and the faulting GPU VA = `((DATA1 & 0xf) << 44) | (DATA0 <<
12)` (i915 `gen8_check_faults`), with an `in_build4a_window` flag for the
`0x130000..0x134fff` buffer range. This de-blinds a repeat of iter6's `0x9e1`:
no fault + `output==[4,6]` → BUILD-4a closes; fault at `0x0013_xxxx` → the payload is
right and the bug is the A64 data-port **address space** (GGTT vs PPGTT — A64
dataport translation is not guaranteed to use the GGTT path instruction fetch does),
escalate with the address; fault at a garbage address → the CURBE pointers landed in
the wrong GRF (fix the cross-thread read length / CURBE offset). The iter6 cosmetic
no-op `seqno=0x3` pre-kick caveat was glanced (ISC-9): `PendingNoop::new` already
zeroes the seqno at construction, so there is no driver pre-seed to fix; the reading
is a hardware/timing artifact and the completion gate also requires the EU-idle
INSTDONE, so it cannot masquerade as a retire. Stage A (this commit): builds
feature-on (ISO) + feature-off, re-laid command oracle + hand-laid payload oracle +
both EU-kernel-EOT tests, `shard-coverage 4`, fmt, host clippy — all green. Stage B
(PL, OptiPlex): `output_base == [4.0f, 6.0f]` with the seqno landed = BUILD-4a→main.

**Iteration 7 Stage-B result — CONCLUSIVE: pointers landed in the wrong GRF (ISC-5b
branch 3), not the address-space risk.** The OptiPlex run (2026-06-07, evidence
`Plans/evidence/phase6-build4a-iter7-optiplex-20260607-stateless-add-arg-payload.md`,
extracted-log sha256 `7086936095742407da4352ee490eb389dd60d54e0ee5848a4323e36cc0fa785e`)
did not retire `[4,6]`, but the ISC-5a fault-address capture made it conclusive. All
iter7 wiring is confirmed on silicon (no-op still retires `dispatch_ok`; the Add carries
`command_len=292`; `ggtt-pte label=curbe ggtt=0x134000`; IDD `cross_thread_read_grf=3`;
`MEDIA_CURBE_LOAD` raises no fault and the EU spins up). The fault decode:
`ring-fault-decode … RING_FAULT=0x000009e1 valid=true fault_type=0 (invalid-PTE) srcid=0x3c
addr_space=GGTT fault_gpu_va=0xffb1ce9bb000 … in_build4a_window=false`. The faulting address
is garbage — not one of the `0x0013_xxxx` buffers — so the Add read its `a` pointer from
`r8.0` and got `0xfb1ce9bb…` instead of `0x130000`: the cross-thread CURBE pointers did **not**
land at `r7..r9` where `add_f32x2.gen9.asm` reads them. This rules out the iter7-prime
address-space risk (ISC-5b branch 2): `addr_space=GGTT` confirms the A64 data port translates
through GGTT (the same path instruction fetch uses), and a correctly-placed pointer would have
faulted in-window or succeeded. Root cause is the Gen9 GPGPU thread-payload GRF layout: the
kernel expects `r0` + 6-GRF per-thread `local_id` (`r1..r6`) + 3-GRF cross-thread (`r7..r9`),
but iter7 declared the cross-thread read length without making the dispatcher emit/account for
the 6-GRF per-thread `local_id` block, so the cross-thread data is mis-placed. The CURBE
*contents* are byte-correct (the hand-laid payload oracle). iter8 fixes the payload-assembly
programming — make `GPGPU_WALKER` generate the per-thread `local_id` and/or set the IDD
per-thread Constant/Indirect URB Entry Read Length so cross-thread lands at `r7..r9` (KBL PRM
Vol. 2a INTERFACE_DESCRIPTOR_DATA + GPGPU_WALKER payload layout; i965 `brw_emit_gpgpu_walker` /
compute-runtime `EncodeDispatchKernel` local-ID emit) — recommended with a Cerebro
substrate-survey on the Gen9 local-ID / cross-thread payload model first. The fault-address
capture stays in place so the iter8 run remains self-diagnosing. The branch does not merge.

**Iteration 8 ISC-1 — the authoritative NEO Gen9 thread-payload field map (read from
compute-runtime `releases/24.35` before encoding).** The blob was built by `ocloc` (IGC
`igc-1.0.17193.4` / CR `24.26.30049.6`), so its payload convention is **NEO's**, not i965's —
the iter7 encoder commented against i965 / `gen9.xml` was the deeper root. Confirmed from
source:

- **Delivery = the GPGPU_WALKER *indirect-data* path, NOT `MEDIA_CURBE_LOAD`.** NEO's Gen9
  helper (`opencl/source/helpers/hardware_commands_helper_*.inl`) emits
  `MEDIA_INTERFACE_DESCRIPTOR_LOAD` but **no `MEDIA_CURBE_LOAD`**; `sendIndirectState` writes
  the payload into the Indirect Object Heap (IOH) and points the walker at it. iter8 drops
  `MEDIA_CURBE_LOAD` and the VFE CURBE allocation (NEO's `programVfeState` does not set CURBE
  Allocation Size — it sets URB Entry Allocation Size `0x782`, Number of URB Entries `1`).
- **IOH order = cross-thread first, then per-thread.** `sendIndirectState`:
  `offsetCrossThreadData = sendCrossThreadData(...)` then `programPerThreadData(...)`. So the
  heap is `[crossThread 96 B][perThread 192 B]` = 288 B at the heap base.
- **`GPGPU_WALKER` indirect-data fields:** `setIndirectDataStartAddress(offsetCrossThreadData)`
  (= heap base, our `GGTT_CURBE_BASE` `0x134000`); `setIndirectDataLength(alignUp(
  sizeCrossThreadData + sizePerThreadDataTotal, 64))` = `alignUp(96+192,64) = 320` (`0x140`).
- **`INTERFACE_DESCRIPTOR_DATA` read lengths** (`EncodeDispatchKernel::setGrfInfo`, integer
  division by `grfSize=32`): `CrossThreadConstantDataReadLength = sizeCrossThreadData/32 = 3`
  (IDD DW7[7:0]); `ConstantIndirectURBEntryReadLength = max(sizePerThreadData/32, 1) = 6`
  (IDD DW5[31:16]).
- **`local_id` channel layout** (`generateLocalIDsSimd`, `local_id_gen.inl`): for SIMD32,
  `threadSkipSize = 32 × sizeof(uint16) = 64 B`; `dimensionsOrder {0,1,2}` ⇒ `local_id.x` at
  per-thread byte 0 (r1/r2), `.y` at 64 (r3/r4), `.z` at 128 (r5/r6); 32 lanes × `uint16` per
  dim; total per-thread = `3 × 64 = 192 B = 6 GRF`. This matches the kernel disasm
  (`add … r1.0<16;16,1>:uw`) and the zeinfo `per_thread_payload_arguments local_id size 192`.
- **Resulting GRF the kernel reads** (NEO programming + the HW convention that places per-thread
  before cross-thread): `r0` header → `r1..r6` per-thread `local_id` → `r7..r9` cross-thread,
  so the kernel's pointers at `r8.0/.1/.2` resolve — the exact ABI `add_f32x2.gen9.asm`
  encodes. General State Base stays 0 / A64 absolute (iter7-proven); the indirect-data start
  is a General-State-relative offset (= absolute GGTT, GSB=0).

Sources: compute-runtime `releases/24.35`
`opencl/source/helpers/hardware_commands_helper_base.inl` (sendIndirectState / IDL, no CURBE),
`shared/source/command_container/command_encoder_bdw_and_later.inl` (setGrfInfo read lengths),
`shared/source/helpers/local_id_gen.inl` (channel layout),
`shared/source/helpers/preamble_bdw_and_later.inl` (VFE URB/CURBE). In-repo ABI oracle:
`intel_gen9_kernels/add_f32x2.gen9.asm` + `.zeinfo`.

**Iteration 8 implementation — the NEO indirect-data dispatch.** Per the field map above,
iter8 replaces the iter7 `MEDIA_CURBE_LOAD` path with the NEO Gen9 indirect-data path: the Add
batch drops `MEDIA_CURBE_LOAD` (back to 69 DWords, the no-op shape) and instead programs the
`GPGPU_WALKER` Indirect Data fields — DW2 Indirect Data Length = `WALKER_INDIRECT_DATA_LENGTH`
(`alignUp(96+192,64)=320`), DW3 Indirect Data Start Address = `GGTT_CURBE_BASE` `0x134000`. The
driver fills that page as the Indirect Object Heap `[cross-thread 96 B][per-thread local_id
192 B]` (`build_curbe_payload` + new `build_local_id_payload`, the latter laying SIMD32
`local_id.x = [0,1,0,…]` per `generateLocalIDsSimd`), and the IDD now carries both read lengths
(`write_interface_descriptor(state, cross=3, per_thread=6)`: DW7[7:0]=3, DW5[31:16]=6) so the
dispatcher lands per-thread at `r1..r6` and cross-thread at `r7..r9`. VFE CURBE Allocation
returns to 0. The no-op batch stays byte-identical (indirect-data fields 0, masks `0xffffffff`,
IDD 0/0) — still the silicon-proven bisector. New independent oracles: a hand-laid 192-byte
`local_id` fixture and a re-laid 276-byte command oracle spot-asserting the absence of
`MEDIA_CURBE_LOAD` (IDL directly after VFE), the walker Indirect Data DW2/DW3, and the 2-lane
masks; the iter7 cross-thread payload oracle is retained. A new `indirect-heap` breadcrumb logs
the assembled heap (cross `a` pointer, `local_id` lane 1) so a failing Stage-B run distinguishes
a mis-assembled heap from wrong GRF placement; the iter7 fault-address capture stays in place.
Stage A (this commit): builds feature-on (ISO) + feature-off, all five oracles + EU-EOT tests,
`shard-coverage 4`, fmt, host clippy — green. Stage B (PL, OptiPlex): `output_base == [4.0f,
6.0f]` with the seqno landed = BUILD-4a→main.

**Iteration 8 Stage-B result — PASS. BUILD-4a is green on silicon.** The OptiPlex run
(2026-06-07, evidence
`Plans/evidence/phase6-build4a-iter8-optiplex-20260607-PASS-gpgpu-thread-payload.md`,
extracted-log sha256 `2cf58dd46b2e9ba2d26e15634d32dfbea40d631a782e4fc2a4f02b621875075b`)
computed `[1,2] + [3,4] = [4,6]` on the real UHD 630 EU:
`complete add token=4 output=[00,00,80,40, 00,00,c0,40]` = `0x40800000, 0x40c00000`;
`event=PASS … output_bits=[0x40800000,0x40c00000] tolerance_ulp=0`. The iter6/7
`RING_FAULT=0x9e1` is gone — `post-kick`/`wait-complete` show `RING_FAULT=0x00000000`,
`INSTDONE=0xfffffffe` (EU idle), and the seqno landed (`0x4`). The new instrumentation
confirms the wiring: `indirect-heap curbe_ggtt=0x134000 indirect_len=320
cross_a=0x0000000000130000 local_id_lane1=1`, and the IDD `cross_thread_read_grf=3
per_thread_read_grf=6`. So the NEO indirect-data dispatch lands per-thread `local_id` at
`r1..r6` and the cross-thread A64 pointers at `r7..r9`, the kernel's `r8.0/.1/.2` resolve
to the real `lhs/rhs/output` GGTT addresses, and the stateless-A64 gather/scatter produce
the oracle. This also closes the iter7 address-space question for good: `RING_FAULT=0`
with a correctly-placed payload confirms A64 stateless data access resolves through GGTT
in legacy-ring mode. The full stack — PCI probe → forcewake → GGTT mapping → legacy ring →
`STATE_BASE_ADDRESS` (iter6) → stateless-A64 + indirect-data payload (iter8) — is
silicon-proven end-to-end. BUILD-4a (C1/C2/C4) is settled on real hardware; the branch
merges to `main`.

## BUILD-4b-1 — generalize the op dispatcher + land matmul

BUILD-4a hard-wired `IntelAccelDriver::dispatch` to the Add. BUILD-4b-1 turns it into a
**kernel-table op dispatcher** on the *same* proven 4a substrate (legacy ring, GGTT,
`STATE_BASE_ADDRESS`, the NEO indirect-data payload, fault-address capture) — only the
kernel blob, the cross-thread payload, and the walker work-size change per op.

- **The dispatcher.** `dispatch` matches `OpKind`: a covered op builds a `PendingCompute`
  via a `ComputeKind` (Add | Matmul{m,n,k}); an uncovered op returns
  `AccelError::Unsupported`. `PendingAdd` generalized to `PendingCompute` (the Add path
  stays byte-identical — same blob, payload, 69-DW batch, oracle). The encoder grew
  `build_local_id_payload_n(lanes)`, `lane_mask(work_items)`, and `encode_matmul_batch`;
  the batch shape is unchanged, only the GPGPU_WALKER execution masks track the live-lane
  count (m·n for matmul, 2 for the Add).
- **The matmul kernel (ISC-1).** `intel_gen9_kernels/matmul_f32.cl` (row-major
  `C[i] = Σ_k A·B`, 1-D NDRange over m·n, matching `ai::ops::matmul`) finalized by the
  pinned ocloc toolchain → 1216-byte stateless-A64 `MATMUL_F32_KERNEL_GEN9` (src
  `07fa1b2b…`, krn `7bed2611…`; `{EOT}`, `a64 dword gathering/scattering`, no surface
  sends). Provenance + oracles (`matmul_f32.gen9.asm`/`.zeinfo`) checked in.
- **The matmul ABI (ISC-2) — read from its OWN zeinfo, not assumed.** Cross-thread payload
  (96 B, 3 GRF): `a@32 / b@40 / c@48` (A64 ptrs, same offsets as the Add's a/b/o), then
  **byvalue dims `m@56 / n@60 / k@64`** (i32), `enqueued_local_size@68`. Per-thread
  `local_id` 192 B (6 GRF); SIMD32, 128 GRF. The disasm reads the dims from `r8.6/r8.7/r9.0`
  and pointers from `r8.0/.1/.2` — the same r7–r9 cross-thread placement the 4a indirect-data
  path produces. Only the dims + enqueued offset differ from the Add.
- **Tolerance (ISC-5 / Q2).** Phase-5 precedent, not a new policy: integer/data-movement ops
  byte-exact (the Add stays 0 ULP); accumulating ops (matmul) within **≤ 2⁻¹² relative error
  vs host f32** (`MATMUL_REL_TOLERANCE = 1/4096`; reference is host f32, not accel_cpu
  soft-float, since hardware f32 tracks host f32 more closely). `matmul_within_tolerance`
  encodes `|g−h| ≤ tol·max(|h|,1)`.
- **Fallback (ISC-4) — graded by transparency.** `exec::run_with_driver` →
  `allocate_result` now routes an op the driver reports `Unsupported` (an uncovered opcode,
  or a matmul too large for this slice) to `accel_cpu` (`default_driver()`), which covers the
  full op set. The batch **completes** with byte-identical results — it is never failed — so a
  forward pass mixing GPU-covered and CPU-fallback ops matches the all-`accel_cpu` result.
- **Slice limits.** The GPU matmul path handles one SIMD32 thread (m·n ≤ 32) with one GGTT
  page per operand; larger shapes return `Unsupported` → CPU fallback (a clean later
  refinement, BUILD-4b-2). Stateless A64 only (4a-proven); no DRM interface, no BTI.
- **Oracles + gate.** Hand-laid matmul cross-thread payload byte fixture + matmul kernel-EOT
  + the tolerance checker (host suite); the Add command/payload regression oracles stay green.
  Stage A: builds feature-on (ISO) + feature-off, 38 host tests, `shard-coverage 4`, fmt,
  host clippy. Stage B (PL, OptiPlex): the `accel_intel_add_bringup` scenario now runs Add
  (regression) → **two** matmuls on the GPU and asserts an oversize matmul routes
  `Unsupported`. The matmuls are deliberately split (dispatch §5 refinement) so the tolerance
  claim is non-vacuous: an **exact-integer** 2×2 (`[1,2;3,4]·[5,6;7,8] = [19,22;43,50]`,
  byte-exact — isolates dim/stride wiring, mode-B) and a **non-exact** 2×2
  (`[0.1,0.2;0.3,0.4]·[0.5,0.6;0.7,0.8]`, dot-products that round) whose reference is **native
  hardware f32** (not accel_cpu soft-float), so the ≤2⁻¹² GPU-f32-vs-host-f32 path actually
  fires (mode-C); the run logs `matmul-exact`/`matmul-nonexact` with the max relative error in
  2⁻²⁴ units (tolerance = 4096). The byte-identical forward-pass transparency oracle (mixed
  GPU/CPU model via `run_with_driver`) needs a model-disk bundle and lands with 4b-2.

**BUILD-4b-1 Stage-B result — PASS.** The OptiPlex run (2026-06-07, evidence
`Plans/evidence/phase6-build4b1-optiplex-20260607-PASS-op-dispatcher-matmul.md`,
extracted-log sha256 `7c16fab4e63685b9248f0d368ada538c7a372dd8f7ba37a691ef610de47c3b57`)
retired the full sequence on the real UHD 630: Add `[4,6]` (regression, byte-exact) →
exact matmul `[1,2;3,4]·[5,6;7,8] = [19,22,43,50]` (`max_rel_e24=0`, mode-B) → non-exact
matmul `[0.1,0.2;0.3,0.4]·[0.5,0.6;0.7,0.8] = [0.19,0.22,0.43,0.50]`
(`matmul-nonexact … max_rel_e24=0 tol_e24=4096 within_tol=true`, mode-C) → oversize matmul
(m·n=64) → `Unsupported` (CPU-fallback routing). `event=PASS … stage=4b1`. Every dispatch
shows `RING_FAULT=0`, `INSTDONE=0xfffffffe` (EU idle). The non-exact delta is **0**: the GPU
f32 matched the native host-f32 reference bit-for-bit (the kernel's separate `add …:f`
accumulation, no FMA), so the ≤2⁻¹² bound is validated on genuinely rounding inputs with
headroom, not assumed. The kernel-table dispatcher + matmul are silicon-proven on the 4a
substrate; the Add stays byte-exact. BUILD-4b-1 is settled on real hardware; the branch
merges to `main`. (4b-2: remaining ops + forward-pass-equivalence + the model-disk
transparency oracle + tensor-migration close, which closes Phase 6.)

## BUILD-4b-2 — GPU GELU + forward-pass-equivalence + migration (closes Phase 6)

BUILD-4b-1 left two things open: the tolerance machinery was never exercised where it
bites (matmul came out bit-exact, `max_rel=0`, because Gen9 has no FMA), and the
forward-pass transparency oracle was deferred. BUILD-4b-2 lands **one transcendental on
the GPU — GELU — and** does the integration close, on the same 4a/4b-1 substrate.

- **The GELU kernel (ISC-1) — hardware math is the point.** `intel_gen9_kernels/gelu_f32.cl`
  computes the *same* tanh-approx form as `ai::ops::gelu`
  (`x·0.5·(1+tanh(√(2/π)·(x+0.044715·x³)))`), using OpenCL `tanh()` so the pinned ocloc
  lowers it to the Gen9 **hardware extended-math** unit. The committed disasm
  `gelu_f32.gen9.asm` shows `math.exp` / `math.inv` / `math.fdiv` implementing `tanh` — a
  host gate asserts that `math.exp` is present (CI fails if a re-finalize loses it).
  4096-byte stateless-A64 `GELU_F32_KERNEL_GEN9` (src `06538782…`, krn `f7ec6c94…`; `{EOT}`).
- **The GELU ABI (ISC-2) — read from its OWN zeinfo.** Pointwise (`in`@32, `out`@40 A64 ptrs;
  `count`@48 byvalue i32; `enqueued_local_size`@52). The payload span ends at byte 64, so the
  cross-thread block is **2 GRF** (NEO `setGrfInfo`: ⌈64/32⌉=2) — *one fewer* than the
  Add/matmul's 3 (their span runs to 76/80). The dispatcher grew a `Gelu{n}` `ComputeKind`
  arm, `build_gelu_curbe_payload`, `encode_gelu_batch`, and a per-op cross-thread-GRF read
  length threaded into the interface descriptor; Add/matmul stay byte-identical. Single input
  → output, so the `rhs` buffer is an unused mapped placeholder. N ≤ 32 (one SIMD32 thread);
  larger → `Unsupported` → accel_cpu.
- **The tolerance-reference model (ISC-3) — the design resolution.** GELU exposes that
  **GPU-gelu ≠ cpu-gelu (soft-float) ≠ host-gelu (libm)** are three independent
  approximations, so "GPU == CPU" is the wrong gate. The rule (amended into the AI
  subsystem spec's Accelerator Driver Interface as **§9.4** — the dispatch named "§9.5",
  but §9 had only §9.1–9.3, so the rule lands as the next subsection §9.4, not a gap):
  - **Op-level:** the GPU GELU is validated against a **host-f32 (libm) reference** within
    the per-op **≤ 2⁻¹² relative** bound — *not* against the soft-float `accel_cpu` backend
    (which is itself only ~2⁻²⁴ off and would be a near-vacuous gate). This is where the
    tolerance **bites**: the scenario runs GELU on 8 spread inputs `[-2,-1,-0.5,0,0.5,1,2,3]`
    where the hardware `tanh` genuinely diverges from libm, and logs `gelu-op … max_rel_e24`.
  - **Forward-pass-level:** the assertion is **not** "GPU-mixed == CPU-pure" (both diverge
    from truth independently). It is that **both** the pure-`accel_cpu` forward **and** the
    mixed-GPU/CPU forward land within an **accumulated** relative bound **B of the host-f32
    reference**. B is derived, not magic: the path has 5 ops, each ≤ 2⁻¹² relative vs host-f32
    (first-order accumulation 5·2⁻¹²), rounded up to **B = 8·2⁻¹² = 2⁻⁹** for the O(1) per-op
    amplification headroom (the norms/softmax). The soft-float regression (`accel_cpu` vs
    host) keeps its Phase-5 bound unchanged.
- **Forward-pass-equivalence (ISC-4) — the deferred transparency oracle.** The scenario runs
  a real forward pass through `exec::run_with_driver` on the `valid_transformer_block` model
  (`LayerNorm → MatMul(4×4) → Gelu → RmsNorm → Softmax`): **MatMul + Gelu route to the GPU;
  LayerNorm/RmsNorm/Softmax return `Unsupported` and fall back to `accel_cpu` transparently
  (complete, never fail).** The pure-`accel_cpu` run reproduces the byte-exact Phase-5 oracle
  (`TRANSFORMER_EXPECTED_CPU` — the BUILD-1 transparency regression); both runs land within B
  of `TRANSFORMER_REF_HOST`. The model rides **embedded in the kernel image** (a `[u8; 4304]`
  const + a test-only synthetic `FileProvenance`), not a disk — so the blind bare-metal run
  gains no new disk/mount hardware dependency. (Note: the terminal softmax flattens the output
  to `~[0.25; 4]`, washing out the upstream GELU divergence — so the forward-level bound is
  satisfied *trivially* here, by design; the tolerance that genuinely bites is the op-level
  GELU case above. ISC-3's op-level/forward-level split is exactly this distinction.)
- **Tensor migration on real GPU (ISC-5).** `copy_to_device`/`copy_from_device` are proven
  real non-identity transfers between host and the GGTT-mapped device buffer (mutate the host
  source after the copy; the device copy is independent); a tensor migrated **cpu→intel→cpu**
  round-trips byte-exact; no host-storage leak — mirroring `accel_sim_migrate_roundtrip` on
  silicon.
- **Oracles + gate (ISC-6/7).** Hand-laid GELU cross-thread payload byte fixture (in/out/count
  at the zeinfo offsets, 2-GRF), GELU kernel-EOT + hardware-`math` disasm scan, the GELU
  tolerance checker vs host-f32, and the embedded forward references self-checked against the
  live `ai::ops` + libm computations (host suite). Add/matmul oracles stay green (42 host
  tests). Stage A: builds feature-on (ISO) + feature-off, host suite, `shard-coverage 4`, fmt,
  host clippy.

**BUILD-4b-2 Stage-B result — PASS (2 hardware iterations). Phase 6 is closed on silicon.**
Evidence `Plans/evidence/phase6-build4b2-optiplex-20260607-PASS-gelu-forward-migrate.md`,
extracted-log sha256 `35ff3a780b69dec302eb28bfab468f6d19e76349e726f46635fd561286d75b7d`.
Run #1 (`713f489`) failed at GELU — only lane 0 dispatched (`output[1..7]=0`): GELU was the
**first >4-lane GPU dispatch ever run on this silicon** (Add=2, matmul=4), and the
hand-trimmed walker execution mask (`0xFF`) dispatched only lane 0. Run #2 (`0a061be`) fixed
it by dispatching a **full SIMD32 thread** (`GELU_DISPATCH_LANES=32`, mask `0xFFFFFFFF`) and
relying on the GELU kernel's own `if (idx >= count) return;` bounds check (count = N) to mask
the partial — what real NEO does; idle lanes return before any A64 access. The OptiPlex run
then retired: Add `[4,6]` (byte-exact) → exact matmul `[19,22,43,50]` (`max_rel_e24=0`) →
non-exact matmul (`max_rel_e24=0`) → **GELU on `[-2,-1,-0.5,0,0.5,1,2,3]` vs host-f32:
`gelu-op … max_rel_e24=1 tol_e24=4096 within_tol=true`** — all 8 lanes, and `max_rel_e24 > 0`
proves the bound fires **non-vacuously** (Gen9 hardware `tanh` diverges from libm by ~1 unit of
2⁻²⁴ at `x=-2`) → the mixed `valid_transformer_block` forward `cpu_rel_e24=0 mixed_rel_e24=0
bound_e24=32768` (MatMul+Gelu on GPU, LayerNorm/RmsNorm/Softmax fall back to `accel_cpu`,
pure-CPU byte-exact to the Phase-5 oracle) → `migrate … tensor_migrate=cpu_gpu_cpu …
storage_freed=ok`. `event=PASS … stage=4b2 … phase6=closed`. Every dispatch `RING_FAULT=0`,
`INSTDONE=0xfffffffe` (EU idle). The accel driver class (CPU / sim / virtio + the real Intel
GPU running matmul/add/gelu with mixed-precision fallback + migration) is bare-metal verified;
the stateful-BTI / DRM-expressibility path stays a documented post-Phase-6 hardening marker
(ADR-023). Latent follow-up: matmul/add still use the hand-trimmed mask (proven only at ≤4
lanes); back-port GELU's full-SIMD32 + bounds-check pattern to make their ≤32 slice real.

## Slice (bring-up-first)
- **BUILD-4a** — minimal compute: probe + MMIO/GTT + one embedded `Add` EU kernel + GuC-free submit + seqno completion + readback `≈` oracle, **bare-metal-captured**. Proves the whole pipeline on the smallest surface. (Settles C1/C2/C4.)
- **BUILD-4b** — full op set as EU kernels + `accel_intel` as a complete `AccelDriver` + forward-pass equivalence (within tolerance) + tensor migration to GTT memory + the DRM-expressibility (ADR-023) and ANSPEC §9 amendments.

## Anti-scope
No graphics / modeset / 3D / display; no discrete GPU; no GuC-based submission; no multi-engine or preemption; no inference scheduler (Phase 7). Compute-only, single-engine, poll-completion, default-off.

## Consequences
- The accelerator abstraction (ADR-030) is exercised against real silicon for the first time; the transport/async machinery is already proven, so the new surface is Intel-specific (EU/GTT/ring/PRM-command encoding) + the offline kernel toolchain.
- BUILD-4 is bare-metal-gated and PL-hands-on; "done" = captured evidence, and it will likely run as a multi-build sub-arc.
- A new offline build dependency (the EU-kernel compiler) with pinned provenance.

## BUILD-4-1 — PPGTT engaged: compute through per-tenant page tables (the positive slice)

**Status:** Stage-A complete (builds + host oracles green); **Stage-B (silicon) pending PL run.** **Date:** 2026-06-08. **Builds on:** BUILD-4a (the proven legacy-ring/GGTT substrate). **Implements:** the THINK `Plans/THINK-2026-06-07-phase-7-build-4-ppgtt-spatial-isolation.md` reframe (A) — PPGTT on the legacy ring, enforcement-first, execlists deferred. **Opens toward:** ANSPEC §9.7's device-resident seam (4-2).

This is the **fourth governance line's positive half**: a single tenant's existing compute (Add, matmul, GELU) runs with its **data buffers resolved through a per-tenant PPGTT instead of GGTT-global**. No cross-tenant negative test (that is 4-2); no execlists, no concurrency (still temporal).

1. **Per-tenant 48-bit PPGTT (ISC-1).** Four DMA frames (PML4 → PDP → PD → PT) map the tenant's three data-buffer VAs (lhs/rhs/output — **numerically the 4a GGTT VAs** `0x13_0000/0x13_1000/0x13_2000`, so the CURBE A64 payload is byte-unchanged) to the buffers' physical frames, **present + R/W**. All three VAs fall under PML4[0] → PDP[0] → PD[0] in one PT (PT[0x130/0x131/0x132]). The PTE bit layout (present bit 0, R/W bit 1, address bits [47:12]) is pinned by an **independent hand-laid byte oracle** (ISC-5) — including the read-only leaf (the 4-2 enforcement bit) as a forward proof. PRM: KBL Vol 5 "Memory Views".

2. **PPGTT-root load on the legacy ring (ISC-2).** A two-`MI_LOAD_REGISTER_IMM(1)` prefix is prepended to the ring command stream (UDW then LDW) loading the **PML4 root into the engine's PDP0** register pair; the engine is put in 48-bit mode via `GFX_MODE` bit 7 (`0x0080_0080`, masked) before the kick — so PDP0 is interpreted as the PML4 root and the EU's A64 data accesses resolve through this PPGTT. The LRI bytes are pinned by a second byte oracle. STATE_BASE_ADDRESS / Instruction Base stay GGTT (command/state fetch is **not** PPGTT — proven by 4a). PRM: KBL Vol 2a (MI_LOAD_REGISTER_IMM), Vol 2c (GFX_MODE, PDP registers); cross-checked vs i915 `gen8_ppgtt.c` / `i915_reg.h`.

3. **Data buffers GGTT→PPGTT only (ISC-3).** With PPGTT engaged the data buffers are **unmapped in GGTT** (`map_compute_buffers` skips them); ring/command/state/EU-kernel/seqno/CURBE stay GGTT. This is what makes the gate self-proving (below). Default-off — the BUILD-4a GGTT path and `accel_intel_add_bringup` are byte-for-byte unchanged; `accel_intel_ppgtt_bringup` sets `set_ppgtt_enabled(true)`.

4. **PDP-root capture (ISC-4, capture-before-you-need-it).** Each PPGTT batch logs the loaded PML4 root and stores it (`debug_ppgtt_last_root`) — 4-2's cross-tenant discriminator needs "which PPGTT was active when a fault fired"; it is captured now, alongside the existing fault-address + GTT/PPGTT-`select` decode.

### The self-proving gate

The gate is **not** "compute works" — it is "compute works **and** the data access resolved through PPGTT (`select=PPGTT`), with the buffers absent from GGTT." Because the data VAs are mapped only in PPGTT, a correct `[4,6]` (and `[19,22,43,50]`) is itself proof the A64 data port went through PPGTT — if PPGTT had not engaged, the access would fall to GGTT, find the VAs unmapped, and **fault**. The `select` bit confirms it independently, ruling out a stale-GGTT-mapping silent false-pass.

### Failure-mode + discriminator table (REQUIRED — blind-hardware)

| # | Mode | Likely cause | Discriminator |
|---|---|---|---|
| 1 | **Compute faults** (own Add/matmul faults) | PPGTT tables don't map the tenant's own buffers, or PTE bits wrong | fault addr = a data-buffer VA (`0x13_0000…`) + `select=PPGTT` → page-table construction bug (ISC-1; check the PTE oracle vs the live tables); `select=GGTT` → PPGTT not loaded, fell to GGTT (ISC-2: GFX_MODE 48-bit not set, or the PDP0 LRI didn't land — the PDP-root log shows whether the root was even computed) |
| 2 | **Compute hangs** (no retire, no fault) | the PDP-load LRI wedged the ring / malformed prefix | last breadcrumb + `RING_FAULT`/`INSTDONE` (the 4a EU-idle-vs-stuck signature); the PDP-root log shows whether the load happened; the command oracle (`debug_last_command_oracle_match`) confirms the LRI+batch bytes are well-formed |
| 3 | **Compute *succeeds* but `select=GGTT`** | a stale GGTT mapping of the buffers still exists → the access didn't actually use PPGTT (the silent false-pass) | the `select` bit on the data access must read **PPGTT**; if it reads GGTT, ISC-3's "unmapped in GGTT" wasn't honored — re-check `map_compute_buffers` skipped lhs/rhs/output |
| 4 | **`select=PPGTT`, correct result** | — | ✅ PPGTT engaged; 4-1 done |

### Stage-A result / Stage-B contract

**Stage A (green):** builds on+off; the PTE + PDP-load byte oracles + the 4a Add/matmul/GELU command oracles + §9.5 accounting + §9.7 sharing host suites; `shard-coverage`; fmt; clippy. The `accel_intel_ppgtt_bringup` ISO builds. **Stage B (PL, OptiPlex):** the Add (and matmul/GELU) retire with correct results, **data buffers absent from GGTT**, the data-access decode showing **`select=PPGTT`**, the PDP root logged. On any miss, report the fault addr + PDP root + `select` + the discriminator branch above. **Then 4-2:** the negative test (cross-tenant OOB access + RW-on-RO write must fault at the device boundary) — the governance line made real.

### BUILD-4-1 silicon result (3 iterations) — legacy-ring PPGTT FALSIFIED on Gen9

Stage B ran three iterations on the OptiPlex UHD 630, exercising **every** PPGTT-root mechanism the legacy ring can express (48-bit PML4→PDP0, 32-bit 4×PDP, Gen7 `PP_DIR_BASE`). All three read **`select=GGTT`** at the EU's A64 data access (fault at `0x13_0000`, the lhs VA, once GGTT was cleared). The tables, root load, and EU execution were correct (byte oracles green, INSTDONE shows the EU running) — the engine simply uses GGTT for data because, on Gen8+, the PPGTT descriptor lives in the **LRC/execlist context**, which a pure legacy ring lacks. The self-proving gate worked exactly as designed: it made the negative conclusive rather than a silent false-pass. The 4-1 branch is retained unmerged as the conclusive legacy-ring record (`Plans/BLOCKER-2026-06-08-phase7-build41-ppgtt-requires-execlists.md`). **Decision: reopen as BUILD-4X (execlist/LRC bring-up) — the only path to a hardware-enforced PPGTT boundary on Gen9.**

## BUILD-4X-1 — GuC-free execlist submission runs a context (the bring-up)

**Status:** Stage-A complete (builds + host oracles green); **Stage-B (silicon) pending PL run.** **Date:** 2026-06-09. **Builds on:** BUILD-4a (the proven command stream) + BUILD-4-1 (the PPGTT machinery, reused) + the cited spec `Plans/RESEARCH-2026-06-08-gen9-execlist-lrc-bringup.md`. **Implements:** `Plans/DISPATCH-2026-06-08-phase-7-build-4X-1-execlist-bringup-jarvis.md` (+ its 2026-06-08 addendum). **Opens toward:** 4X-2 (the LRC carries a per-tenant PPGTT root → `select=PPGTT`, 4-1's gate finally reachable).

BUILD-4-1 proved the legacy ring cannot engage PPGTT on Gen9 — the engine binds the per-process address space to the LRC that **execlist** submission loads. 4X-1's only job: prove a context submitted via the **Execlist Submit Port (ELSP)** actually executes on this silicon, GuC-free, **decoupled from PPGTT**. It uses the **finalizer no-op** (EOT-only kernel, no memory access), not the Add — a no-op that retires proves the execlist context ran the dispatch pipeline with **zero PPGTT-data dependency** (instruction fetch is Instruction-Base/GGTT; the seqno write is GGTT post-sync). The LRC carries a minimal valid PPGTT root that nothing dereferences this build.

Every offset/bit/sequence is **transcribed from the cited spec, not from memory** (the addendum's instruction). Mechanism:

1. **LRC context image (ISC-1).** A 22-page (+ PPHWSP) GGTT-pinned Gen9 RCS Logical Ring Context. Page 0 = PPHWSP; the register-state image is page 1. **Dword 0 is `MI_NOOP`; the first `MI_LOAD_REGISTER_IMM(14)` header is at dword 0x01** (the off-by-one trap the spec caught — my first draft put it at 0x00). Three LRI blocks (the "+1 value-slot" rule: `CTX_*` = a register *address* dword, value at +1): header_0 @0x01 (ring HEAD/TAIL/START/CTL + BB/indirect-ctx), header_1 @0x21 (CTX_TIMESTAMP + the 8 PDP dwords in **descending** order PDP3…PDP0), header_2 @0x41 (R_PWR_CLK_STATE). **Gen9 is 48-bit**: a single PML4 page address goes into **CTX_PDP0 (value at 0x31 upper / 0x33 lower)**, not the 4×PDP form. RPCS = 0 (enable bit clear → boot-default fused GT2 config; an explicit RPCS is a 4X-2 knob).

2. **Context descriptor (ISC-2).** 64-bit: `VALID(bit0) | PRIVILEGE(bit8) | (addr_mode=3 << 3)` (Gen9 48-bit = `LEGACY_64B`) `| (LRCA & 0xFFFFF000)` (the PPHWSP page GGTT address) `| (ctx_id << 32)` (Gen9 bits 32-52 — **not** the Gen11 split).

3. **ELSP submission (ISC-3).** Four writes to `RING_ELSP` (RCS 0x2230, the classic register — **Gen9 is not ELSQ**): el1.hi, el1.lo, el0.hi, el0.lo with `desc[1]=0`; the final write latches. Posted via a read of `EXECLIST_STATUS_LO` (0x2234).

4. **CSB completion (ISC-4).** Poll the Context Status Buffer via MMIO (6 entries at 0x2370, PTR at 0x23a0, mask 0x7, 0x7-reset → start at 5). Watch `IDLE_ACTIVE` (bit0, switch-in) → `ACTIVE_IDLE` (bit3, idle), cross-checking each entry's ctx-id dword against our descriptor.

5. **Bring-up (`configure_execlist_engine`).** Forcewake held; `RING_HWSTAM` ← ~0; `RING_MODE_GEN7` ← `0x80008000` (masked-enable `GFX_RUN_LIST_ENABLE` bit15 — the Gen9 bit); `RING_MI_MODE` ← clear `STOP_RING`; `RING_HWS_PGA` ← GGTT status-page offset + posting read. The ring registers are **not** MMIO-programmed — the engine loads them from the LRC on switch-in.

6. **WaIdleLiteRestore (corrected — ISC).** Never resubmit with the same `RING_TAIL` (HW reads `HEAD==TAIL` as an empty ring and wedges). Each submission pads **2 `MI_NOOP`s (8 bytes, `WA_TAIL_DWORDS=2`)** after the batch so the tail always advances. (The brief said "8 dwords"; the spec corrects it to 2.)

### Byte oracles (ISC-6) — including the non-i915 cross-check

Independent hand-laid fixtures pin: the **context descriptor** (a known LRCA+ctx-id → the 64-bit value) and the **LRC register-state image** (the ring-pointer block + the PML4/PDP0 slot + the three LRI headers). **The non-i915 cross-check (the spec's open byte-oracle action, now closed):** the `CTX_*` dword map and the descriptor bit ranges are independently confirmed against **Fuchsia Magma `msd-intel-gen` `register_state_helper.h`** (a clean-room driver, not derived from i915) — MI_LRI headers at 0x1/0x21/0x41, ring head/tail/start/control values at 0x05/0x07/0x09/0x0b, the single PML4 at PDP0 0x31/0x33. The descriptor oracle also rebuilds the value field-by-field from the PRM Vol 2d "Context Descriptor Format" bit ranges (Valid bit0, addr-mode 4:3, LRCA 31:12, ctx-id 52:32), closing the "two i915 readers agree even if both wrong" loophole. The 4-1 PTE oracle stands.

### Failure-mode decision tree (REQUIRED — blind-hardware; replaces the linear table)

Captured each run: CSB dump (status bits + ctx-id) + `EXECLIST_STATUS` + `RING_HEAD` + fault address + active PML4 root + the `RING_MODE` execlist-enable readback.

- **No CSB entry + STATUS shows nothing accepted** → submission rejected. **Check cheapest first:** `RING_MODE` bit-15 readback + forcewake held during the ELSP write (`treeA`); *then* descriptor `VALID`/addressing bits. (These share the signature; execlist-enable/forcewake are the common mistakes.)
- **`RING_HEAD` advanced but CSB all-zero** → HWSP/CSB not set up (`treeD`).
- **CSB active → COMPLETE, `RING_HEAD` unmoved** → empty ring / bad `CTX_RING_TAIL` / the WaIdleLiteRestore same-tail trap (`treeB`).
- **CSB active → fault/hang** → bad LRC/ring-start/PDPs **vs** bad GGTT mapping → **split by fault address: GGTT range ⇒ mapping (`treeC` GGTT); PPGTT range ⇒ LRC/PML4 (`treeC` PPGTT).**

The driver's timeout path computes and logs the matching branch directly (`branch=treeA…treeE`), so a failing blind run is self-diagnosing.

### Scope guard / Stage contract

No PPGTT-data positive test (`select=PPGTT` — that is 4X-2), no cross-tenant negative test (4X-3), no data-buffer compute (the no-op touches no data), no GuC/HuC firmware. **Stage A (green):** builds on+off; the descriptor + LRC byte oracles (incl. the non-i915 cross-check) + the 4-1 PTE oracle + the 4a command oracles + §9.5/§9.7 host suites; `shard-coverage`; fmt; clippy. The `accel_intel_execlist_bringup` ISO builds. **Stage B (PL, OptiPlex):** the no-op context submitted via ELSP **runs** — the CSB shows switch-in→idle, the no-op seqno lands, no hang/fault. Report the CSB dump + seqno + `EXECLIST_STATUS` + (on any miss) the decision-tree branch. **Golden-state caveat:** Gen9 needs a null-render-state batch before the first *real* compute — moot for the no-op, a flagged risk for 4X-2.

### iter1 silicon result — execlist submission WORKS; premise corrected (the no-op is NOT zero-PPGTT)

The first OptiPlex run (2026-06-09, `Plans/evidence/phase7-build4X1-iter1-optiplex-execlist-RAN-fault-PPGTT-statepage.log`) is a **breakthrough**, not a failure:

- **The context ran.** `execlist_enabled=true`, ring un-stopped, HWS_PGA programmed; the ELSP write was accepted (`EXECLIST_STATUS` upper dword = our ctx-id 1); the **CSB posted `IDLE_ACTIVE` (switch-in)**; and **`RING_HEAD` advanced 0→0x110** — the command streamer executed the ring. GuC-free execlist submission **works on this Gen9 silicon** — the 4X-1 core question is answered YES.
- **PPGTT engaged** (the opposite of BUILD-4-1's `select=GGTT`). It then faulted with `RING_FAULT=0xc1`, **`addr_space=PPGTT`**, `fault_gpu_va=0x110000` (the STATE page). The decision tree localized it exactly: `treeC_csb+fault_PPGTT → bad LRC/PML4`.
- **Premise corrected.** The dispatch assumed "the no-op touches no data — instruction fetch is Instruction-Base/GGTT, the seqno is GGTT post-sync — so the empty PML4 is never dereferenced." iter1 falsifies this: **under an execlist context the engine resolves the dispatch pipeline's STATE_BASE / MEDIA_INTERFACE_DESCRIPTOR_LOAD / kernel-fetch accesses through the context PPGTT, not GGTT.** An empty PML4 therefore cannot run even the no-op. (The ring buffer itself is still GGTT-fetched via `RING_BUFFER_START` — that is why `RING_HEAD` advanced before the fault.)

**iter2 fix:** replace the empty PML4 with a real 48-bit PML4→PDP→PD→PT (four frames, reusing 4-1's `leaf_pte`/`dir_entry`) mapping the no-op's own pipeline pages (ring/state/kernel/data/seqno) at their VAs. All those VAs share PML4[0]→PDP[0]→PD[0] in one PT (leaves at PT[0x100…0x133]); a host oracle pins the 4-level decomposition. This is *not* the 4X-2 isolation test — it is the plumbing that makes any execlist context's pipeline reachable.

### iter2 silicon result — **STAGE B PASS** (BUILD-4X-1 complete on silicon)

The OptiPlex iter2 run (2026-06-09, `Plans/evidence/phase7-build4X1-iter2-optiplex-execlist-PASS-noop-retired.log`) **passed cleanly**:

- `execlist-pml4 built … maps=ring/state/kernel/lhs/rhs/output/seqno pt_idx_state=0x110` — the real PML4 was built.
- CSB: entry 0 `status=0x01` (`IDLE_ACTIVE`, switch-in) → entry 1 `status=0x18` (`ACTIVE_IDLE`+`COMPLETE`), **`ctxid=1`, `ctxid_matches_ours=true`**; summary **`switch_in_then_idle=true`**. A clean **switch-in → idle** of *our* context.
- `RING_HEAD=0x118 == RING_TAIL=0x118` (ran to completion), **`RING_FAULT=0`**, `INSTDONE=0xfffffffe` (only the command-streamer bit set — engine idle), and **`seqno=0x00000001`** (the no-op retired).
- `event=PASS … context=ran no_op=retired csb=switch_in_idle`.

**GuC-free execlist submission runs a context on Gen9 silicon, with the context's PPGTT as the active address space.** This both closes 4X-1 *and* de-risks 4X-2: BUILD-4-1's `select=PPGTT` device boundary is now reachable — the only remaining step is to map a *per-tenant* data PPGTT into the LRC (the data-isolation positive test) and the cross-tenant negative test (4X-3). The branch is mergeable (Stage A green + Stage B PASS).

## BUILD-4X-2 — tensor data through the per-context PPGTT (`select=PPGTT`, the positive boundary)

**Status:** Stage-A complete (builds + host oracles green); **Stage-B (silicon) pending PL run.** **Date:** 2026-06-09. **Builds on:** BUILD-4X-1 (the execlist context with its PPGTT active) + BUILD-4-1 PPGTT machinery + the 4a Add. **Implements:** `Plans/DISPATCH-2026-06-08-phase-7-build-4X-2-ppgtt-data-jarvis.md`. **Opens toward:** 4X-3 (the cross-tenant negative + RO-enforcement — the §9.8 device boundary made real).

4X-1 proved the execlist context runs with its PPGTT as the active address space (the no-op's *pipeline* pages resolved through PPGTT). **4X-2 proves the actual *tensor data* resolves there:** it runs the proven 4a **Add** (`[1,2]+[3,4]→[4,6]`) through the same execlist context, with the **data buffers (lhs/rhs/output) mapped PPGTT-only — cleared from GGTT.** This is BUILD-4-1's original `select=PPGTT` gate, now reachable. One tenant; no cross-tenant test, no RO-enforcement (both 4X-3).

1. **Data buffers into the context PML4 (ISC-1).** The 4X-1 path is parameterized by an `ExeclistKind {NoOp, Add}`. The Add extends the per-context 48-bit PML4 to map the data buffers (lhs/rhs/output) **and the CURBE indirect-data heap** (the GPGPU_WALKER reads it through PPGTT — the 4X-1 iter1 lesson that *more* resolves through PPGTT than on the legacy ring) at their A64 VAs, present+RW, in the same single PT (leaves PT[0x130/0x131/0x132/0x134]). The A64 base pointers in the CURBE stay the 4a VAs; the EU's stateless A64 access resolves them through the context PPGTT. Instruction Base / command stream stay GGTT (4a-proven; the ring is GGTT-fetched).

2. **Data buffers PPGTT-only (ISC-2).** `map_execlist_buffers` **actively clears** lhs/rhs/output from GGTT (writing invalid PTEs — the GPU GGTT persists across reboot, so clear ≠ skip, per the BUILD-4-1 lesson). The CURBE/state/kernel stay GGTT-mapped (pipeline state, not the tested data). So the only way the Add's A64 data accesses succeed is through the per-context PPGTT.

3. **Run the Add via the execlist context (ISC-3).** Reuses 4X-1's ELSP submission + CSB completion for the 4a Add batch; on retire, reads `output` back and compares to `add_oracle_output()` = `[4.0, 6.0]`.

4. **The self-proving gate (ISC-4).** With the data buffers absent from GGTT, **a correct `[4,6]` is only possible if the data resolved through PPGTT** (a GGTT fallthrough finds nothing and faults). On a fault the iter7 decode prints the `select` bit + addr as the discriminator (GGTT = buffers weren't unmapped; PPGTT+buffer-VA = PML4 gap; PPGTT+state-VA = the 4X-1 iter1 pattern). The inverse-proof of 4-1's `select=GGTT` dead end.

5. **Byte oracle (ISC-5).** A host oracle pins the data-buffer + CURBE VAs' 4-level decomposition (all under one PT) and the present+RW leaf bytes (reusing the 4-1 PTE oracle).

### Failure-mode decision tree (REQUIRED — blind-hardware)

Captures: `select` bit + fault addr + PML4 root + CSB + RING_HEAD. The driver logs the matching branch (the 4X-1 `treeA…treeE` decode + the `ring-fault-decode select` bit).

- **Add faults, `select=GGTT`** → a stale GGTT mapping of the data survived (ISC-2 not honored). The `select` must be PPGTT; GGTT = buffers weren't unmapped.
- **Add faults, `select=PPGTT`, addr = a data-buffer VA** → the PML4 doesn't map that buffer (page-table construction). Fault addr = which buffer.
- **Add faults, `select=PPGTT`, addr = a STATE/pipeline page** → the context routes that page through PPGTT and it's unmapped (4X-1 iter1 pattern) → add it to the PML4.
- **Add runs, wrong result** → data mapped to the wrong frame/stride, or a compute regression → compare to the GGTT-path 4b result.
- **Correct `[4,6]` + buffers absent from GGTT** → ✅ tensor data isolated through the per-context PPGTT; 4X-2 done → 4X-3.

### Scope guard / Stage contract

Positive single-tenant data-isolation only — no cross-tenant negative, no RO-enforcement (both 4X-3), no concurrency, no change to 4X-1's submission or the 4a command stream. **Stage A (green):** builds on+off; the data-mapping PTE oracle + 4X-1/4-1/4a/4b + accounting/admission/sharing host suites (51 oracles); `shard-coverage` (197); fmt; my code clippy-clean. The `accel_intel_execlist_data_bringup` ISO builds. **Stage B (PL, OptiPlex):** the Add retires `[4,6]` via the execlist context, **data buffers absent from GGTT**, CSB switch-in→idle. On a PASS, **§9.8 flips spatial-isolation from "mechanism-proven" to "positive-proven"** (full "enforcement-proven" waits for 4X-3's negative test).

### Stage B silicon result — **STAGE B PASS** (BUILD-4X-2 complete on silicon)

The OptiPlex run (2026-06-09, `Plans/evidence/phase7-build4X2-optiplex-execlist-PASS-add-46-select-ppgtt.log`) **passed**:

- `execlist-pml4 built kind=add … leaf_pages=8 maps=ring/state/kernel/lhs/rhs/output/seqno/curbe` — the per-context PML4 maps all 8 pipeline+data pages.
- `ggtt-pte-clear label=xl-lhs/xl-rhs/xl-output` + `data buffers CLEARED from GGTT (… resolve via per-context PPGTT only)` — the data buffers are PPGTT-only (the self-proving gate).
- CSB clean **switch-in → idle** (ctx-id 1 matched, `switch_in_then_idle=true`); `RING_HEAD==RING_TAIL`, **`RING_FAULT=0`**, `seqno=0x1`.
- `execlist-add output=[00,00,80,40, 00,00,c0,40]` = **`[4.0, 6.0]`**, `match=true`. With the data buffers absent from GGTT, a correct result means the EU's A64 data accesses **resolved through the per-context PPGTT** — `event=PASS … add=[4,6] resolved_via=per_context_ppgtt`.

**Tensor data is isolated through the per-context PPGTT on Gen9 silicon** — BUILD-4-1's `select=PPGTT` gate, finally reached on the execlist context. ANSPEC §9.8 spatial-isolation moves from "mechanism-proven" (4X-1) to **"positive-proven"** (4X-2); full "enforcement-proven" awaits 4X-3 (the cross-tenant negative + RO-on-write fault). The branch is mergeable (Stage A green + Stage B PASS).

*(Operational note: the first two boot attempts stopped during SMP bring-up — an intermittent LAPIC/IPI flake on this OptiPlex, before the dispatch ran; a cold reboot got past it. Unrelated to the GPU path, but a known retry hazard for bare-metal runs on this box.)*

## BUILD-4X-3 — the cross-tenant negative test (the silicon-enforced device boundary)

**Status:** Stage-A complete (builds + host oracles green); **Stage-B (silicon) pending PL run.** **Date:** 2026-06-09. **Builds on:** BUILD-4X-1/4X-2 (execlist context + per-context PPGTT + positive boundary) + BUILD-3 (capability RO sharing at the op boundary — this adds its *device-side* enforcement). **Implements:** `Plans/DISPATCH-2026-06-08-phase-7-build-4X-3-negative-boundary-jarvis.md`. **Closes:** the §9.8 device-boundary governance line (the fourth and last) + BUILD-3's §9.7 device-resident-RO seam.

4X-2 proved a tenant's data resolves through *its own* PPGTT. **4X-3 proves the boundary holds against an adversary:** a *second* tenant's device-side access to the *first*'s tensor must **page-fault**, and an RO-shared tensor must **fault on write**. The validation is a **negative test, and the catastrophic failure looks like success** (tenant B silently reads A's data, run looks fine) — so **the gate is the fault, proven correct**, paired with a mandatory positive control.

Reuses 4X-1/4X-2 wholesale via `ExeclistKind` — three contexts of the *proven* one, differing only in the per-context PML4 data-buffer mapping; run temporally (the boundary is spatial — separate page trees — independent of execution order):

1. **Positive control (`Add`).** Tenant B's Add on its **own** tensor (lhs present+RW in B's PML4) must **succeed** `[4,6]`. Without it a fault is ambiguous: isolation vs B's-tables-broken.
2. **Cross-tenant (`AddCrossTenant`, ISC-2 — the gate).** B's PML4 **omits** the lhs leaf (lhs VA = "tenant A's tensor", present in A's separate page tree, absent from B's). B's EU A64 read of lhs **must fault**. Captured and asserted: the run did **not** complete, `RING_FAULT` valid, **`select=PPGTT`**, fault VA **= the lhs VA**, B's PML4 root active. A *success* here is the silent isolation hole the gate exists to catch (`reason=…_SILENT_HOLE`).
3. **RW-on-RO (`AddReadOnly`, ISC-3).** lhs + output mapped **read-only** (PT RW-bit clear). The lhs *read* succeeds (RO-read works); the EU *write* to output **must fault** at the output VA (`select=PPGTT`). The fault VA being the *output* (not lhs) proves the RO read succeeded and only the write was blocked — the device-side enforcement of BUILD-3's §9.7 RO sharing (a shared-RO tensor is now hardware-unwritable by the grantee, not just policy-unwritable).

**Engine hygiene:** the scenario runs success + two fault contexts in one boot, so `configure_execlist_engine` clears any sticky `RING_FAULT` before each submission — a left-over fault must not bleed into the next run's gate (execlists isolate a faulting context, so the next ELSP submission is processed on a clean engine).

### Failure-mode decision tree (REQUIRED — the negative-test shape)

Captures per expected fault: `select` + fault addr + active PML4 root + CSB + RING_HEAD (driver getters `debug_last_fault_{valid,select_ppgtt,gpu_va,type}` + `debug_ppgtt_last_root`).

- **Cross-access succeeds (no fault)** → the silent hole: PMLs share a mapping / B maps A's frame. Gate FAILs (`SILENT_HOLE`).
- **B's OWN access faults** (positive control fails) → B's PML4 broken (construction), not isolation. Gate FAILs early — never trust the negative without this.
- **Cross-access faults but wrong context** → the active-PML4-root capture: the fault must fire with B's root.
- **RW-on-RO doesn't fault** → the RO tensor's PTE was written RW (cross-check the RO PTE oracle).
- **cross faults (A's VA, B's root, select=PPGTT) + own succeeds + RW-on-RO faults (output VA) + RO-read succeeds** → ✅ the device boundary is silicon-enforced; 4X-3 done.

### Scope guard / Stage contract

The cross-tenant negative + RW/RO device-side enforcement only — no concurrency (the boundary is spatial; two temporal contexts suffice), no new ops, no preemption, no change to 4X-1/4X-2 submission or PPGTT machinery. **Stage A (green):** builds on+off; the RO + two-context PML4 oracles (`intel_gen9_execlist_negative_pte_invariants_oracle`) + 4X-1/4X-2/4-1/4a/4b + accounting/admission/sharing host suites (52 oracles); `shard-coverage` (198); fmt; my code clippy-clean. The `accel_intel_execlist_negative_bringup` ISO builds. **Stage B (PL, OptiPlex):** B's access to A's tensor **faults** (`select=PPGTT`, A's VA, B's root); B's **own** access succeeds; RW-on-RO **faults** (output VA), RO-read succeeds. On PASS: **§9.8 spatial-isolation flips to ENFORCEMENT-PROVEN — the fourth governance line is real; BUILD-3's §9.7 seam is closed; the kernel governs AI between adversarial tenants, enforced by the accelerator's page tables.** *(SMP-flake hazard: cold power-cycle + retry if it wedges pre-dispatch.)*

### iter1 silicon result — cross-tenant isolation PROVEN; a Gen9 fault-reporting finding on RW-on-RO

The first OptiPlex run (2026-06-09, `Plans/evidence/phase7-build4X3-iter1-optiplex-cross-tenant-PASS-ro-write-no-ringfault.log`) **proved the primary boundary** and surfaced a precise hardware finding:

- **Positive control PASS** — B's own Add retired `[4,6]`.
- **Cross-tenant PASS (the §9.8 device boundary)** — B's read of A's tensor VA (omitted from B's PML4) **page-faulted**: `RING_FAULT=0x1e1`, `fault_type=0 (not-present)`, **`addr_space=PPGTT`**, **`fault_gpu_va=0x130000`** (exactly A's VA), **B's PML4 root active** (`0x…3a2000`), the Add did not complete. **A second tenant cannot reach the first's tensor — the isolation holds on silicon.**
- **RW-on-RO finding** — the write to the RO output did **not** raise `RING_FAULT` (it read `0`); the EU wedged/preempted (CSB status `0x8002`) and the run timed out (`treeE`). **Gen9 reports a *not-present* fault through `RING_FAULT` but a *write-permission* fault does not surface there:** the page walk succeeds (page present) and the permission check fails at the dataport, wedging the EU. The RO bit *is* enforced (the write did not complete), but the observation method was wrong.

**iter2 fix:** split the RO test into `AddRoRead` (lhs RO → must **complete** `[4,6]`, proving RO reads are allowed) and `AddRoWrite` (output RO → must **not complete**, and the proof of the block is the **output frame staying unmodified** — the all-RW control writes `[4,6]`, so an all-zero output means the RO write never landed). The gate no longer expects a `RING_FAULT` for the write-permission violation. Order: the two success runs first (clean engine), then the two fault/block runs (`configure_execlist_engine` clears `RING_FAULT` between them). The cross-tenant proof is unchanged (it PASSED). Re-staged for iter2.
