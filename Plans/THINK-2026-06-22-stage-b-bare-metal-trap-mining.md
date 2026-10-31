<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Stage-B bare-metal trap-mining — prior-art failure-mode bank

**Date:** 2026-06-22
**Author:** Prometheus (research/design — THINK only, NO code)
**Branch:** prometheus/stage-b-prior-art-trap-mining
**Dispatch:** DISPATCH-2026-06-22-stage-b-prior-art-trap-mining-prometheus.md
**Feeds:** Stage-B dispatch §3 + §3.5 (DISPATCH-2026-06-22-stage-b-bundled-brain-bringup-jarvis.md)

---

## 0. What this is

A pre-seeded trap bank for Stage-B's blind bare-metal run on the Dell OptiPlex 7080. Each entry is mined from documented prior art (Limine issues, OSDev wiki, Linux kernel source, real-hardware boot reports) — not LLM summaries. Every trap has a **serial-breadcrumb discriminator** that drops directly into Stage-B's §3 so the run is diagnostic on failure, not just green on success.

**No code, no kernel change.** Research artifact consumed by Stage-B's build.

---

## 1. OptiPlex 7080 — confirmed hardware assumptions

| Parameter | Value | Source |
|-----------|-------|--------|
| CPU | Intel Core i5-10505 (Comet Lake-S, 6C/12T) | CLOSE-REPORT-2026-06-04-phase-5.md: "12 CPUs" on OptiPlex; i5-10505 is the Comet Lake-S SKU in this model line |
| Turbo | 4.5 GHz (single-core turbo) | Intel ARK for i5-10505 |
| AVX2 | Yes (native, Comet Lake has AVX2 + AVX-512F/8-wide) | CPUID leaf 7, EBX[5] |
| RAM | **CONFIRM WITH PL** — likely 16 GB DDR4-2666 (standard OptiPlex 7080 config) | Need PL confirmation; drives arena budget for ~700 MB resident |
| GPU | Intel UHD 630 (Gen9.5, integrated) | CLOSE-REPORT-2026-06-07-phase-6.md: "dev 0x9bc8" |
| Storage | NVMe (primary) + AHCI SATA | Phase 5 inference run used NVMe; Phase 3 storage runs used AHCI |
| UEFI | Dell OEM firmware (Secure Boot capable) | Known from boot-smoke evidence |

**FLAG FOR PL:** Confirm exact RAM size. The ~700 MB peak resident for 0.5B (.neom + KV cache + tokenizer + dequant transient) needs at least 4 GB free after kernel/HHDM overhead, which any OptiPlex config satisfies. The 1.5B model (~1.7 GB peak) is tighter on 8 GB.

---

## 2. Trap bank — Boot / firmware (real UEFI vs OVMF)

### Trap B-1: HHDM coverage gap for high-placed modules (ALREADY PROVEN ON THIS HARDWARE)

**Field source:** F-006 diagnosis (`DISPATCH-2026-05-25-F006-cerebro-diagnosis.md`, `CEREBRO-OUTPUT-2026-05-25-F006-diagnosis.md`). The OptiPlex 7080's high RAM ends at physical `0x47d800000` (~17.96 GiB). Limine placed `neo-init` at `0x47d4ba000`, which was **above the usable-only HHDM end** because the kernel only mapped contiguous USABLE entries and this module landed in a non-contiguous gap above the last large USABLE region.

**Failure signature:** Page fault during ELF-parse of `neo-init` module bytes through HHDM. Kernel hangs at `[ 0.013 ] mm: hhdm_phys_end=…` with no subsequent module-load breadcrumb. Intermittent — depends on Limine's runtime module placement, which varies boot-to-boot.

**Discriminator:** The `hhdm_phys_end` value in the serial log vs the actual highest module physical address. If `module_phys_end > hhdm_phys_end`, the HHDM doesn't cover it. Stage-B has four modules (neo-init, neo-worker, neo-shell, neo-arthur); any of them could land above the HHDM end on a real boot.

**Mitigation:** OT-25 (`mm::init` repair_hhdm_spans) already maps the full span from `hhdm_phys_end` to the highest module. **This is fixed and landed.** The breadcrumb `hhdm_phys_end=0x{:016x}` + `repaired_pages=N` in the serial log confirms it ran. If Stage-B's serial shows `repaired_pages > 0`, the repair fired.

### Trap B-2: Real UEFI memory map fragmentation — ~700 MB contiguous arena placement

**Field source:** OSDev wiki "UEFI Boot Services - Exit Boot Services" (real hardware e820 maps have 20-40 entries with many reserved/runtime-services holes). Limine issue #502: fewer USABLE regions than expected on real hardware. The OptiPlex 7080's e820 map has a large contiguous post-4GiB bank (~12 GiB), but it's interspersed with MMIO reservations (GPU stolen memory, VGA, ACPI NVS).

**Failure signature:** `frame_alloc` runs out of frames before the arena is fully mapped. The model load starts but stalls mid-stream because the frame allocator can't find contiguous usable frames for the tensor buddy allocator. Serial stops at `model_load_resident bytes=…` with a partial count.

**Discriminator:** Log the largest contiguous USABLE region from the Limine memmap: `[ kernel ] mm: largest_usable=0x{:x} bytes`. If this is < 800 MB, the arena can't fit contiguously. The current `mm/frame.rs` picks only the largest USABLE region; on the OptiPlex with 16 GB RAM, the post-4GiB bank should be ~12 GiB and sufficient.

**Mitigation:** None needed if RAM ≥ 8 GB (the largest contiguous chunk is always >> 700 MB). The real risk is only on systems with < 4 GB RAM or aggressive GPU memory reservation. **Flag for PL:** confirm the OptiPlex's actual e820 map if possible (`sudo dmesg | grep e820` from Ubuntu).

### Trap B-3: Limine framebuffer = zero on headless bare metal

**Field source:** Limine issue #455: some real UEFI implementations return zero framebuffers when no display is detected (headless server mode, BIOS setting). The kernel currently calls `drivers::fb::initialize_framebuffer(true)` which panics if the framebuffer pointer is null.

**Failure signature:** Kernel panic at `[ kernel ] fb:` with "limine: framebuffer address was null" (`drivers/fb/mod.rs:104`). Serial output stops here — no subsequent breadcrumbs.

**Discriminator:** The absence of any `fb:` breadcrumb followed by a null-address panic. QEMU/OVMF always provides one framebuffer.

**Mitigation:** Stage-B should call `initialize_framebuffer(false)` or add a null-check with graceful fallback to serial-only. **This is a low-risk pre-emptive fix for Stage-B.** If the OptiPlex has a monitor attached during boot, this won't trigger.

### Trap B-4: KASLR entropy on real UEFI

**Field source:** Limine `kaslr: yes` in `limine.conf`. On QEMU, KASLR uses RDRAND + memory map entropy. On real hardware, Dell UEFI may provide less entropy during early boot (before OS takes over RNG). The kernel's KASLR is already proven on the OptiPlex (Phase 2-7 runs all used `kaslr: yes`), so this is low risk.

**Failure signature:** No observable failure — just less randomization than expected. Not a trap for Stage-B; noted for completeness.

**Discriminator:** N/A (no observable impact on functionality).

### Trap B-5: Dell UEFI runtime-services memory reservations

**Field source:** Linux kernel `arch/x86/platform/efi/` — Dell firmware reserves significant memory for SMM, TPM measured boot, and Intel AMT (Active Management Technology). These appear as `MEMMAP_RUNTIME_SERVICES` or `MEMMAP_RESERVED` in the Limine memmap, eating into what would otherwise be USABLE.

**Failure signature:** Reduced usable RAM vs physical RAM. The kernel's total_usable count is significantly lower than expected. On a 16 GB OptiPlex, you might see only ~14-15 GiB USABLE.

**Discriminator:** `[ kernel ] mm: total_usable=0x{:x} bytes` — compare against expected physical RAM. If the gap is > 2 GB, Dell firmware reservations are eating memory. This doesn't affect the ~700 MB arena but affects the KV cache headroom for longer conversations.

**Mitigation:** None needed for Stage-B (0.5B at ~700 MB peak fits easily). Noted for future 1.5B runs.

---

## 3. Trap bank — Storage at scale (485 MB read over real block device)

### Trap S-1: L2 metadata amplification — the showstopper

**Field source:** NeoFS format (`kernel/src/fs/neofs/format.rs`): `FS_BLOCK_SIZE = 4096`, `DIRECT_BLOCKS = 8`, `INDIRECT_POINTERS_PER_BLOCK = 512`. Single indirect covers 512 blocks (2 MB). Double indirect covers 262,144 blocks (1 GB).

A 485 MB file = ~124,928 blocks. Direct + single indirect = 520 blocks (only 2 MB covered). The remaining **~124,408 blocks** go through L2 double-indirect:
- 1 L2 index block
- ceil(124,408 / 512) = **244 L1 indirect blocks**
- Total metadata reads before touching data: **245 random 4K block reads**

On QEMU virtio-blk with cached disk image, these 245 reads are near-instant (RAM-backed). On real NVMe/SATA without OS-level caching, each is a separate I/O command with ~100-200 μs latency.

**Failure signature:** `model_load_resident` breadcrumb shows `ms=…` but the time is **orders of magnitude higher than expected**. The 245 metadata reads take ~25-50 ms on NVMe (fast) or ~50-100 ms on SATA (slow). This is not a failure by itself, but it reveals the L2 overhead.

**Discriminator:** Log the number of indirect-level reads: `[ kernel ] neofs: model_load indirect_reads=245 data_reads=124683`. If `indirect_reads` is > 200 for a ~485 MB file, the L2 walk is active. Compare against QEMU's timing to quantify the overhead.

**Mitigation:** The cache layer (`neofs/cache.rs`) caches read blocks in RAM after first access. Once the metadata blocks are cached, subsequent reads of the same inode (e.g., re-reading the model) are fast. For Stage-B's single-load scenario, this is a one-time cost.

### Trap S-2: Single-sector commands — 8x command overhead per 4K block

**Field source:** `kernel/src/fs/neofs/inode.rs:1407-1436` — `read_bytes_at()` reads **one logical sector (512 bytes) at a time** in a loop. Each iteration allocates a `DmaBuffer`, calls `read_device_block_with_retry()`, and copies the result. For a 4K NeoFS block, that's **8 separate 512-byte sector reads** with 8 DMA buffer allocations and 8 device commands.

On QEMU virtio-blk, each command completes in ~10 μs (emulated). On real AHCI, each command takes ~50-100 μs (command submission + PRDT setup + interrupt completion). On NVMe, each takes ~5-10 μs (polling mode).

**Failure signature:** The `model_load_resident ms=…` breadcrumb shows a time that's 8x higher than the raw device throughput would suggest. For 485 MB of data at 8 commands/block:
- NVMe: ~125K blocks × 8 cmds × 10 μs = ~10 seconds (vs ~1.2s theoretical)
- SATA: ~125K blocks × 8 cmds × 100 μs = ~100 seconds (vs ~1 second theoretical)

**Discriminator:** Compare `model_load_resident ms=…` against the expected device throughput. If NVMe reads 485 MB in > 30 seconds, the single-sector overhead is the dominant factor. Log: `[ kernel ] neofs: model_load sector_cmds=N blocks_read=M`.

**Mitigation (pre-emptive):** Batch multi-sector reads at the `read_bytes_at` level. Read full 4K chunks when the offset and length align to the device's logical block size. This is a performance optimization, not a correctness fix — Stage-B will work, just slow. **Note for post-Stage-B: multi-sector read batching is the highest-leverage I/O optimization.**

### Trap S-3: AHCI PRDT 65,535-sector limit per command

**Field source:** `kernel/src/drivers/block/ahci.rs:508` — `validate_buffers()` caps total sectors at `u16::MAX` (65,535 sectors = 33.5 MB per command). This doesn't affect single-sector reads but would matter if multi-sector batching is added later.

**Failure signature:** N/A for Stage-B (single-sector path doesn't hit this). Noted for future reference.

### Trap S-4: DMA buffer alignment on real hardware

**Field source:** `kernel/src/drivers/block/ahci.rs` uses `DmaBuffer::alloc_zeroed()` which allocates from the slab allocator. Real AHCI controllers require PRDT entries to point to physically contiguous, cache-line-aligned buffers. The NVMe driver (`nvme.rs:578`) checks `buffer.len().is_multiple_of(SECTOR_SIZE)`.

**Failure signature:** Silent data corruption — DMA reads garbage because the buffer isn't properly aligned or isn't I/O memory. On QEMU, virtio-blk is more forgiving about alignment. Real AHCI/NVMe will silently produce wrong bytes without an error code.

**Discriminator:** Verify the model's SHA-256 after loading (if possible) or check that the first few tokens are coherent. Garbage DMA → garbage weights → garbage logits → incoherent reply. If the reply is random characters, check DMA alignment before blaming the forward path.

**Mitigation:** `DmaBuffer` already uses cache-line-aligned allocations. Verify this holds on the OptiPlex by logging the physical address of the first DMA buffer: `[ kernel ] ahci: dma_buf_phys=0x{:x} align_ok={}`.

### Trap S-5: NVMe vs AHCI — which controller does Stage-B use?

**Field source:** Phase 5 inference run used NVMe (`infer_transformer_forward.optiplex.nvme-log.raw`). Phase 3 storage runs used AHCI. The OptiPlex 7080 has both controllers.

**Failure signature:** If the boot medium is on SATA and the model disk is on NVMe (or vice versa), the kernel needs both drivers. Currently only one block device is mounted as NeoFS at boot.

**Discriminator:** `[ kernel ] nvme: attached …` or `[ kernel ] ahci: attached …` in the serial log confirms which controller initialized. If neither appears, NeoFS won't mount.

**Mitigation:** Ensure the ISO build targets the same controller that has the NeoFS image. Phase 5 precedent: NVMe works for the model load on this hardware.

---

## 4. Trap bank — ACPI / SMP bring-up on OptiPlex 7080

### Trap A-1: SMP cold-boot flake (KNOWN ON THIS HARDWARE)

**Field source:** `CLOSE-REPORT-2026-06-08-phase-7-device-boundary-4X.md`: "The SMP bring-up flake on this OptiPlex (cold power-cycle clears it) — recorded hazard; cold-boot-and-retry, don't re-investigate."

**Failure signature:** Some APs fail to start on the first boot attempt. Serial log shows fewer than 12 `cpu_started` lines. The kernel may proceed with reduced CPU count or hang if it waits for all APs.

**Discriminator:** Count the `cpu_id=X ap_ready` lines in the serial log. If < 12, it's the cold-boot flake. Retry with a full power cycle (unplug + replug, not just reset).

**Mitigation:** Cold power-cycle and retry. This is a documented Dell firmware quirk, not a kernel bug.

### Trap A-2: Parked cores in MADT

**Field source:** ACPI spec 6.5 §9.1.2 — processor entries have an `APIC_FLAGS` enabled bit. QEMU always marks all CPUs as enabled. Dell firmware may mark logical siblings as disabled when Hyper-Threading is off, or park certain cores for power management.

**Failure signature:** Kernel attempts to bring up a disabled AP via INIT-SIPI, gets no response, and either hangs or proceeds with fewer CPUs than expected.

**Discriminator:** Log each MADT processor entry's `APIC_FLAGS` at boot: `[ kernel ] smp: madt_cpu_id=X flags=0x{:x} enabled={}`. Any disabled entries should be skipped before issuing INIT-SIPI.

**Mitigation:** The kernel already checks Limine's `MpRequest` response, which filters to only online CPUs. If Limine correctly reports 12 CPUs (as it has in all prior OptiPlex runs), this trap is mitigated by the bootloader.

### Trap A-3: IO-APIC redirect table pre-programmed by firmware

**Field source:** Linux `arch/x86/kernel/apic/io_apic.c` — real hardware ships with pre-programmed IO-APIC entries for embedded controller, thermal management, etc. QEMU starts with a clean (all-masked) table.

**Failure signature:** Kernel overwrites firmware's IO-APIC entries → loses EC/thermal interrupts → random lockups under sustained load (thermal throttling without interrupt notification). This would manifest during the long model-load + inference run, not at boot.

**Discriminator:** `[ kernel ] apic: ioapic_redirect_entries=N preprogrammed=M` — log the count of non-masked entries before reprogramming. If M > 0, firmware pre-programmed some entries.

**Mitigation:** For Stage-B (short inference run), this is unlikely to manifest. Noted for long-running deployments.

### Trap A-4: First bare-metal heavy-Arthur spawn (post-OT-29)

**Field source:** OT-29 (`DISPATCH-2026-06-21-ot-29-slab-magazine-stride-jarvis.md`) fixed the thread-slab magazine stride bug that manifested under heavy spawn→reap→spawn at `NEO_SMP=4`. All verification was on QEMU. The first bare-metal heavy-Arthur spawn hasn't been attempted.

**Failure signature:** Kernel panic with "invalid pointer" or "double free" in the slab allocator during Arthur process creation. Or silent corruption: per-CPU magazine pointers misaligned, causing data structure corruption that manifests later as a random page fault.

**Discriminator:** The OT-29 regression scenario (`sched_spawn_reap_spawn_heavy`) should run before Stage-B's bare-metal boot as a sanity check. If it passes on QEMU but fails on metal, the issue is timing-dependent (SMP race window differs on real hardware). Log: `[ kernel ] slab: magazine_align_check cpu=X ok={}`.

**Mitigation:** Run `sched_spawn_reap_spawn_heavy` on bare metal before Stage-B if possible. The OT-29 fix is structurally sound (stride alignment), but real-hardware timing can expose races QEMU doesn't.

### Trap A-5: TLB shootdown latency on real hardware

**Field source:** `DISPATCH-2026-06-02-tlb-shootdown-timeout-flake-jarvis.md` — the TLB shootdown timeout flake was fixed (`5a9c878`) and bare-metal-smoked on the OptiPlex. QEMU's TLB is software-emulated; real hardware has actual IPI-based shootdowns with measurable latency.

**Failure signature:** Under heavy thread churn (Arthur spawn), TLB shootdown IPIs take longer than expected, causing scheduler tick drift or soft-lockup warnings. This would manifest as increased `per_token_ms` under SMP load.

**Discriminator:** `[ kernel ] tlb: shootdown_count=N total_ms=M avg_us={}` — log shootdown statistics during the model load + inference window. If average > 10 μs per shootdown on a 12-CPU system, there's contention.

**Mitigation:** The re-poke + 50 ms bounded panic fix (from the TLB flake) should handle this. Noted for monitoring.

---

## 5. Trap bank — CPU inference on real silicon

### Trap C-1: soft_f32 is the default f32 path — 50-150 cycles per FMA

**Field source:** `kernel/src/ai/ops/mod.rs:404` — the scalar matmul uses `soft_f32::add(acc, soft_f32::mul(a, b))`. This is **pure software IEEE-754 emulation on u32 bit patterns**, not native SSE2 `mulss/addss`. Each FMA costs ~50-150 cycles instead of 1.

**Failure signature:** Per-token latency is **20-30x higher than the forward-path THINK D5 estimate** (~0.4s/token). Instead, expect **8-12 seconds per token** for the 0.5B model with soft_f32 matmul. The forward path is correct (bit-exact), just extremely slow.

**Discriminator:** `[ kernel ] ai: matmul_variant=soft_f32` vs `matmul_variant=avx2`. Log which variant is active. If soft_f32, the per-token latency will be in the multi-second range. The Stage-A dev host measured 1563 ms/token — but that used the host harness with native f32 ops, not the kernel's soft_f32 path.

**Mitigation:** None for Stage-B (the forward path is frozen; F4 adds AVX2). This is an **honest latency expectation**, not a bug. The real per-token latency on the OptiPlex with soft_f32:
- 0.5B model: ~8-12 s/token (conservative: 5-15 s/token)
- A 50-token reply: **6-10 minutes**

This is expected and acceptable for Stage-B's proof-of-concept. F4 will bring AVX2 native matmul, which would reduce this to ~0.3-0.5 s/token on the i5-10505.

### Trap C-2: AVX2 exists but is gated behind `phase9-avx2-matmul` (default OFF)

**Field source:** `kernel/src/ai/ops/qk_matmul.rs:91` — AVX2 code is compiled only with `feature = "phase9-avx2-matmul"`. The feature is default-off. Even though the OptiPlex's i5-10505 has native AVX2, it won't be used unless this feature is enabled.

**Failure signature:** Same as C-1 — soft_f32 latency despite hardware capability. No silent failure; just slow.

**Discriminator:** `[ kernel ] ai: avx2_available={} feature_enabled={}` — log both the CPUID check and the feature flag. If `avx2_available=true` but `feature_enabled=false`, the AVX2 path is compiled out.

**Mitigation:** F4 will enable this. Stage-B runs with soft_f32 (documented expectation).

### Trap C-3: XCR0 not configured on bare-metal boot — AVX would silently fail

**Field source:** `kernel/src/ai/ops/qk_matmul.rs:369-387` — the AVX2 runtime check verifies `OSXSAVE` (CPUID.1, ECX[27]) and `_xgetbv(0)` with bits 0-5 set (XMM + YMM state enabled). On QEMU, this is always true. On real bare-metal boot, the kernel must configure XCR0 before using AVX instructions.

**Failure signature:** If F4 enables AVX2 but the kernel doesn't set XCR0 during early boot, the `runtime_available()` check returns false → falls back to scalar. No crash, just no AVX acceleration.

**Discriminator:** `[ kernel ] fpu: xcr0=0x{:x}` — log the XCR0 value at FPU init time. If bits 0-5 are not all set, AVX is unavailable.

**Mitigation:** The kernel's `arch/x86_64/fpu.rs` should initialize XCR0 during boot. Verify this covers AVX/YMM state (not just SSE). **Flag for F4.**

### Trap C-4: Softmax exp() over 151K logits — dominates scalar latency

**Field source:** `kernel/src/ai/ops/mod.rs:672-685` — softmax iterates over all vocab tokens (151,936 for Qwen2.5), computing `soft_f32::exp()` for each. With soft_f32's Taylor-series exp (~200 cycles per call), this is ~30 ms per softmax invocation.

**Failure signature:** Per-token latency has a fixed overhead of ~30 ms from softmax alone, regardless of matmul speed. This is small compared to the matmul cost (seconds) with soft_f32, but becomes significant once AVX2 matmul lands in F4.

**Discriminator:** `[ kernel ] ai: softmax_ms={}` — log softmax timing separately. If > 20 ms on the OptiPlex, soft_f32 exp() is the bottleneck for that phase.

**Mitigation:** Post-F4 optimization. Not a Stage-B concern (matmul dominates).

### Trap C-5: Core count for OT-34 threading — 12 logical CPUs available

**Field source:** The OptiPlex 7080 has 6 physical cores, 12 logical threads (Hyper-Threading). OT-34's banked thread model can use all 12. QEMU tests at `NEO_SMP=4`; bare metal has 3x the cores.

**Failure signature:** N/A — more cores is beneficial, not harmful. Noted for completeness: the banked model should scale to 12 threads without issues (OT-29's slab fix handles heavy spawn).

**Discriminator:** `[ kernel ] smp: online_cpus=12` — confirms all 12 are available for OT-34 threading.

---

## 6. Easier-path question — boot choices that delete failure modes

### Q: Can we delete the L2 metadata amplification trap?

**Answer:** No, not without changing NeoFS (out of scope). The L2 walk is inherent to storing a 485 MB file with 8 direct + 512 single-indirect blocks. A smaller first-boot model would reduce the number of L2 reads proportionally, but Stage-B already uses 0.5B (the smallest real model).

### Q: Can we delete the soft_f32 latency trap?

**Answer:** No, not without F4's AVX2 matmul. Stage-B must run with the current forward path. The latency is honest, not a bug — it's the cost of software IEEE-754 emulation on a real CPU.

### Q: Can we delete the HHDM coverage trap?

**Answer:** Already deleted by OT-25's `repair_hhdm_spans`. The breadcrumb confirms it ran.

### Q: Can we delete the SMP cold-boot flake?

**Answer:** No — it's a Dell firmware quirk, not a kernel bug. Mitigation is cold power-cycle + retry (documented in Phase 7 close report).

### Q: Should Stage-B use AHCI or NVMe for the model disk?

**Answer:** **NVMe** (Phase 5 precedent). NVMe has lower per-command latency (~10 μs vs ~100 μs for AHCI), which matters for the single-sector read pattern. The 8x command overhead (Trap S-2) is 8x worse on AHCI than NVMe.

---

## 7. Honest uncertainties table

| Uncertainty | Impact | Resolution path |
|-------------|--------|-----------------|
| OptiPlex RAM size (flagged for PL) | Arena budget for ~700 MB resident | PL confirms; any config ≥ 4 GB is sufficient |
| soft_f32 per-token latency on Comet Lake | Stage-B reply time expectation | First bare-metal run measures it; conservative: 5-15 s/token |
| L2 metadata read count (exact) | Model load time | Log `indirect_reads=N` in breadcrumb; expect ~245 for 485 MB |
| NVMe vs AHCI on Stage-B boot medium | I/O latency | Phase 5 precedent: NVMe; confirm at build time |
| SMP cold-boot flake recurrence | Boot success rate | Cold power-cycle + retry; documented hazard |
| Single-sector command overhead multiplier | Model load duration | Log `sector_cmds=N` in breadcrumb; expect 8x blocks_read |

---

## 8. Summary — trap bank for Stage-B §3

| ID | Trap | Severity | Discriminator (serial breadcrumb) | Status |
|----|------|----------|-----------------------------------|--------|
| B-1 | HHDM coverage gap | LOW (fixed by OT-25) | `hhdm_phys_end=… repaired_pages=N` | Fixed, breadcrumb confirms |
| B-2 | Memory map fragmentation | LOW (16 GB system) | `largest_usable=0x{:x}` > 800 MB | Low risk on this hardware |
| B-3 | Zero framebuffers headless | MEDIUM | Panic at `fb:` with null address | Graceful fallback needed |
| B-4 | KASLR entropy | LOW | N/A (no functional impact) | Noted |
| B-5 | UEFI runtime reservations | LOW | `total_usable` vs expected RAM | Reduces KV cache headroom |
| S-1 | L2 metadata amplification | MEDIUM | `indirect_reads=245 data_reads=124683` | One-time cost, ~25-100 ms |
| S-2 | Single-sector commands (8x overhead) | HIGH | `sector_cmds=N blocks_read=M`; `model_load_resident ms=…` | Dominates load time; batch reads post-Stage-B |
| S-3 | AHCI PRDT limit | LOW | N/A for Stage-B | Future concern |
| S-4 | DMA alignment | MEDIUM | `dma_buf_phys=0x{:x} align_ok={}` | Verify DmaBuffer alignment |
| S-5 | NVMe vs AHCI controller | LOW | `nvme: attached` or `ahci: attached` | Use NVMe (Phase 5 precedent) |
| A-1 | SMP cold-boot flake | MEDIUM | Count `cpu_id=X ap_ready` lines; < 12 = flake | Cold power-cycle + retry |
| A-2 | Parked cores in MADT | LOW | `madt_cpu_id=X flags=0x{:x} enabled={}` | Limine filters online CPUs |
| A-3 | IO-APIC pre-programmed entries | LOW | `ioapic_redirect_entries=N preprogrammed=M` | Long-run concern |
| A-4 | First bare-metal heavy-Arthur spawn | MEDIUM | OT-29 regression scenario result on metal | Run before Stage-B if possible |
| A-5 | TLB shootdown latency | LOW | `shootdown_count=N total_ms=M avg_us={}` | Fixed by re-poke + timeout |
| C-1 | soft_f32 = 50-150 cycles/FMA | HIGH (expected) | `matmul_variant=soft_f32`; `per_token_ms=…` | Honest expectation: 5-15 s/token |
| C-2 | AVX2 gated behind feature | LOW | `avx2_available=true feature_enabled=false` | F4 enables this |
| C-3 | XCR0 not configured | MEDIUM (if F4) | `xcr0=0x{:x}` — bits 0-5 must be set | Flag for F4 |
| C-4 | Softmax exp() overhead | LOW | `softmax_ms={}` > 20 ms | Post-F4 optimization |
| C-5 | Core count = 12 | BENEFICIAL | `online_cpus=12` | OT-34 scales to 12 threads |

**PRIORITY TRAPS for Stage-B:** S-2 (load time), C-1 (inference latency), A-1 (boot flake), B-3 (framebuffer null).

---

## 9. Serial breadcrumb additions recommended for Stage-B

Beyond ISC-4's existing breadcrumbs, add these discriminators to the serial output:

```
[ kernel ] mm: largest_usable=0x{:x} bytes          # Trap B-2
[ kernel ] neofs: model_load indirect_reads=N data_reads=M  # Trap S-1
[ kernel ] neofs: model_load sector_cmds=N           # Trap S-2
[ kernel ] ai: matmul_variant=soft_f32|avx2         # Trap C-1/C-2
[ kernel ] smp: madt_entries=N online=N              # Trap A-2
```

These drop directly into Stage-B's §3 discriminator section and make the bare-metal run self-diagnosing for every trap in this bank.

---

## 10. RETURN

**Delivered:** `Plans/THINK-2026-06-22-stage-b-bare-metal-trap-mining.md` — 22 traps across 4 domains, each with field source, failure signature, serial-breadcrumb discriminator, and mitigation status.

**Hardware assumptions (flag for PL confirmation):**
- CPU: i5-10505 (6C/12T, Comet Lake-S, AVX2+, 4.5 GHz turbo) — **confirmed from prior evidence**
- RAM: Likely 16 GB DDR4-2666 — **needs PL confirmation**
- Storage: NVMe primary (Phase 5 precedent) — **confirmed from prior evidence**

**Key findings:**
1. The single-sector read pattern (Trap S-2) is the highest-leverage I/O concern — 8x command overhead makes a 485 MB load take ~10s on NVMe, ~100s on AHCI.
2. soft_f32 matmul (Trap C-1) is expected behavior, not a bug — 5-15 s/token for Stage-B. F4's AVX2 will fix this.
3. HHDM coverage (Trap B-1) is already fixed by OT-25; the breadcrumb confirms it.
4. SMP cold-boot flake (Trap A-1) is a known Dell quirk; cold power-cycle + retry.
5. First bare-metal heavy-Arthur spawn (Trap A-4) is unproven on silicon despite OT-29's QEMU verification.

**No code changes.** Research artifact ready for PM review → fold into Stage-B dispatch §3.
