<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 6 BUILD-4a — Intel Gen9 finalizer toolchain + real EU kernels → Prometheus

**Date:** 2026-06-06
**From:** PM/Cerebro
**To:** Prometheus (worker tree, feature branch — `prometheus/phase-6-build-4a-intel-finalizer`)
**Dispatch (authoritative, on main):** `Plans/DISPATCH-2026-06-06-phase-6-build-4a-intel-finalizer-toolchain-prometheus.md`
**Status:** DISPATCHED — parallel to Jarvis iter6 (`codex/phase-6-build-4a-intel-gpu-bringup-jarvis`). Scopes are disjoint by region (§7); merge order in §8.
**Builds on:** Jarvis iter5 (`3c7a9d7`) bisect = **SUSPECT A (dispatch setup)** — the hand-laid EOT-only no-op hung byte-identically to the Add.

---

## 0. READ FIRST — fetch-first gotcha (applies to every PM dispatch)

I land dispatches on `origin/main` from the cloud (PM) side; your worker tree
won't see them until you fetch. The **EU-kernel source + the Jarvis BUILD-4a
driver** you'll be editing live on the Jarvis branch, not main — so branch off
*that*, not main, for this one:

```
git fetch origin
git checkout -b prometheus/phase-6-build-4a-intel-finalizer origin/codex/phase-6-build-4a-intel-gpu-bringup-jarvis
```

If you can't see this dispatch, read it straight from the ref to prove the fetch worked:

```
git show origin/main:Plans/DISPATCH-2026-06-06-phase-6-build-4a-intel-finalizer-toolchain-prometheus.md
```

You branch off the Jarvis BUILD-4a head (not main) because the whole Intel
driver, the vISA source, and the blob consts you are replacing only exist
there — BUILD-4a has not merged to main (it's gated on the Stage-B `[4,6]`
PASS). Pull the dispatch text from `origin/main`, pull the *code* from
`origin/codex/phase-6-build-4a-intel-gpu-bringup-jarvis`.

## 1. Why this lane exists now

iter5's bisect came back **SUSPECT A**: the hand-laid EOT-only no-op
(`NOOP_KERNEL_GEN9`, built from named bitfields in `intel_gen9_4a.rs`) hung
**byte-identically** to the Add — `RING_HEAD=ACTHD=0x50`, `seqno=0`,
`INSTDONE=0xffdeffff`, a row-0 thread dispatched (bit 16) but instruction
fetch stuck (bit 21). Jarvis's iter6 (parallel) fixes the prime cause on the
driver side: a **missing `STATE_BASE_ADDRESS`** so the EU can resolve the
kernel-fetch pointer. But iter6 leaves a **second, independent** unknown that
only this lane can close:

> Both embedded EU kernels are currently **un-finalized**. `ADD_F32X2_KERNEL_GEN9`
> is an ASCII *placeholder* blob (it was never compiled by an Intel finalizer —
> the standard Neo container has no IGC/IGA/ocloc), and `NOOP_KERNEL_GEN9` is
> **hand-laid** directly from the EU ISA, with its register-region DWords flagged
> in the kernel README as "the residual uncertainty… Stage-B silicon is the
> authority; if the no-op *also* hangs, iter6's real IGA/IGC finalizer
> regenerates a confirmed blob."

iter5 *is* that "no-op also hangs" case. So even after iter6 fixes
STATE_BASE_ADDRESS, we cannot distinguish "dispatch still broken" from "the
kernel bytes are malformed" until at least one **finalizer-confirmed, indep-
endently-disassembled** kernel exists. **This dispatch produces that ground
truth**, and the real Add blob that any genuine `[4,6]` PASS requires.

Anchor: ADR-032 **D4** — "EU kernels are compiled offline… with the open Intel
toolchain (`intel-graphics-compiler`/IGA/vISA)… The kernel *source*, the *exact
pinned compiler version*, and the *build recipe* are checked in so a blob's
provenance is reproducible (not an opaque artifact)." This is the dispatch that
makes D4 real instead of aspirational.

## 2. Objective

A **pinned, reproducible** Intel Gen9 finalizer toolchain, checked into the
repo as a build-time tool (not a kernel runtime dep — Article 4.3), that
compiles the checked-in vISA/ISA sources into real Gen9.5 EU binaries; and the
two embedded blobs (`ADD_F32X2_KERNEL_GEN9`, `NOOP_KERNEL_GEN9`) replaced with
**finalizer-built, independently-disassembled** bytes whose provenance (tool
version + exact command line + input sha256 + output sha256 + disassembly) is
checked in.

## 3. Implementation Success Criteria

- **ISC-1 — Pinned finalizer toolchain.** Stand up the Intel finalizer as a
  containerized build tool under `tools/intel-gen9-finalizer/`:
  `Dockerfile` pinning **`igc-1.0.17193`** (the README's pinned version) plus
  `ocloc`/`iga64`/the vISA finalizer, and a driver script
  (`build-kernels.sh`, SPDX `MPL-2.0`) that takes a `.visa`/`.cl`/`.asm`
  source and emits the Gen9 (`gen9`/`skl`/`kbl`) binary deterministically.
  If `igc-1.0.17193` is genuinely unobtainable, pin the **nearest reproducible
  release**, record the substitution + why in the README, and FLAG it to PM in
  the close report — do not silently drift the pin.
- **ISC-2 — Real Add kernel.** Compile `kernel/src/drivers/accel/intel_gen9_kernels/add_f32x2.visa`
  → Gen9 binary → replace the bytes of `ADD_F32X2_KERNEL_GEN9` in
  `kernel/src/drivers/accel/intel_gen9_4a.rs`. If the real binary's length is
  not 64 bytes, update the array length + `ADD_F32X2_KERNEL_EOT_TAIL_BYTES` /
  `add_kernel_has_pinned_eot_tail()` to match the real EOT tail the finalizer
  emitted (do **not** keep the placeholder tail if the real one differs).
- **ISC-3 — Real no-op kernel.** Provide an EU-ISA source for the EOT-only
  no-op (add `intel_gen9_kernels/noop_eot.asm` — the canonical IGT/Mesa form
  `send(8) null g0 0x82000010 { EOT }` is acceptable and preferred over the
  two-instruction g127 form if the finalizer accepts it), compile it, and
  replace `NOOP_KERNEL_GEN9` with the finalizer output. Keep the
  named-bitfield builder *as a documented derivation comment*, but the embedded
  bytes must now be the **finalizer's**, with the hand-laid bytes recorded
  alongside for the diff (this is exactly the "register-region DWords are the
  residual uncertainty" question the README raised — resolve it with silicon-
  grade provenance).
- **ISC-4 — Independent disassembly = the byte oracle.** For **each** blob,
  run the finalizer's disassembler (`iga64 -d` / `ocloc disasm`) and check the
  disassembly text in under `intel_gen9_kernels/` (`add_f32x2.gen9.asm`,
  `noop_eot.gen9.asm`). The disassembly is the **independent oracle** required
  by CLAUDE.md for a byte layout: it is produced by a *different* tool than the
  one that emitted the bytes, and a human can read the EOT send + opcodes off
  it. Confirm in the README that the disassembly shows a terminating
  **End-Of-Thread send** for both. (This is what the kernel README explicitly
  asked for: "replace the blob only with an independently reviewed EOT send.")
- **ISC-5 — Oracle sync.** Update the EU-kernel blob/tail assertions in
  `tools/ai-ops-tests` (the `ADD_F32X2_KERNEL_EOT_TAIL_BYTES`,
  `noop_kernel_has_eot`, `NOOP_KERNEL_SEND_DW0/DW3` checks) to the real
  finalizer bytes so the host suite asserts the *shipped* kernels. **Do not**
  touch the command-byte oracle for the *command stream* (`ADD_BATCH_BYTES` /
  the MEDIA_VFE_STATE/GPGPU_WALKER packet bytes) — that oracle belongs to
  Jarvis's iter6 and will move when he adds STATE_BASE_ADDRESS (§7).
- **ISC-6 — Provenance, fully pinned.** Rewrite
  `kernel/src/drivers/accel/intel_gen9_kernels/README.md` so the recipe is
  byte-reproducible: exact container build, exact `build-kernels.sh` invocation
  per source, the **sha256 of each input source and each output blob**, the
  tool version actually used, and a one-command "regenerate + diff" path. Drop
  the "could not regenerate in the standard Neo container" caveat — it's now
  regenerable in `tools/intel-gen9-finalizer/`.
- **ISC-7 — Standards + green.** SPDX on every new file (`MPL-2.0` for
  scripts/Dockerfile/Rust, `CC-BY-4.0` for `.asm`/`.visa` source + README +
  disassembly text). `cargo xtask` kernel build green with the new blobs;
  `cargo fmt --all --check` + clippy clean; host suite (`tools/ai-ops-tests`)
  + `shard-coverage 4` green with the feature **off**. ADR-032 D4 gets a
  ledger line that the finalizer toolchain now exists and the blobs are real
  (one line — the substantive recipe lives in the kernel README, don't dupe).

## 4. Two-stage gate — your half is Stage A

- **Stage A (you):** toolchain builds and emits the blobs **deterministically**
  (same input → same output sha256 across two runs); both disassemblies show a
  terminating EOT send; kernel builds with the real blobs; host oracle + CI
  green feature-off; fmt/clippy clean.
- **Stage B (PL, with Jarvis iter6):** OptiPlex run. This lane does **not**
  merge on its own and does **not** itself prove silicon — it removes the
  "are the kernel bytes even valid?" unknown so that *Jarvis's* iter6 no-op
  result is interpretable: with a finalizer-confirmed no-op + iter6's
  STATE_BASE_ADDRESS, a landed `noop-kernel-seqno` proves dispatch is fixed,
  and a landed `[4,6]` from the real Add is the BUILD-4a merge trigger.

## 5. Verification reality

The disassembly is the oracle, not "ocloc exited 0." Read each
`*.gen9.asm` and confirm with your own eyes that (a) the Add does a SIMD
elementwise f32 add of the two source regions into the dest, and (b) **both**
kernels terminate in an EOT send. Determinism is load-bearing: build each
blob twice and assert identical sha256 — a non-reproducible toolchain fails
ISC-1 even if it produces a working blob once. Paste both sha256 pairs into
the close report.

## 6. The reference surface

- Source: `kernel/src/drivers/accel/intel_gen9_kernels/add_f32x2.visa` (exists),
  `noop_eot.asm` (NEW), README (rewrite).
- Blob consts: `kernel/src/drivers/accel/intel_gen9_4a.rs`
  — `ADD_F32X2_KERNEL_GEN9` (line ~62), `ADD_F32X2_KERNEL_EOT_TAIL_BYTES`/
  `add_kernel_has_pinned_eot_tail` (~68–73), `NOOP_KERNEL_GEN9` + the
  `NOOP_*_DW*` builders and `NOOP_KERNEL_*` exports (~105–175). **These const
  definitions only** — see §7.
- Oracle: the EU-kernel assertions in `tools/ai-ops-tests` (NOT the
  command-stream `ADD_BATCH` bytes).
- Toolchain: `tools/intel-gen9-finalizer/` (NEW).

## 7. Scope guard — the one shared file (`intel_gen9_4a.rs`)

You and Jarvis both touch `intel_gen9_4a.rs`, but **disjoint regions**, and the
split is hard:

- **Yours (Prometheus):** the EU-**kernel-blob** region only — the blob byte
  arrays, their EOT-tail/DWord builder consts, and the `noop_kernel_has_eot` /
  `add_kernel_has_pinned_eot_tail` helpers. Plus `intel_gen9_kernels/**`,
  `tools/intel-gen9-finalizer/**`, and the EU-kernel oracle in `tools/ai-ops-tests`.
- **Jarvis's (do NOT touch):** the **command-encoder** region — `MEDIA_VFE_STATE_9DW`,
  `MEDIA_INTERFACE_DESCRIPTOR_LOAD_4DW`, `GPGPU_WALKER_13DW`, `ADD_BATCH_DWORDS`/
  `ADD_BATCH_BYTES`, the `GGTT_*_BASE` consts, `AddBatchLayout`/`BUILD4A_LAYOUT`,
  and anything STATE_BASE_ADDRESS. He grows the command stream; you do not.

If you find you *must* change a const in his region to make a blob fit (e.g.
`GGTT_KERNEL_BASE` / `KERNEL_BYTES` if the real Add exceeds one page), **stop and
flag PM** — don't edit it; we'll sequence it into iter6 instead.

## 8. Close gate + merge order

- `git fetch origin && git rebase origin/codex/phase-6-build-4a-intel-gpu-bringup-jarvis`
  (rebase onto the **Jarvis** branch, since that's your base), then re-run the
  Stage-A gate after the rebase.
- Determinism proof (two-run sha256 match per blob), both disassemblies
  showing EOT, kernel build + host oracle + fmt/clippy green feature-off.
- Single squashed commit; push to your **feature branch
  `prometheus/phase-6-build-4a-intel-finalizer`, not main**.
- **Merge order (PM runs this):** PM merges your finalizer branch **into the
  Jarvis BUILD-4a branch first** (real blobs are a prerequisite for
  interpreting iter6's no-op result), via
  `scripts/pm-merge-check.sh origin/prometheus/phase-6-build-4a-intel-finalizer`
  then an `--ff-only`/cherry-pick onto `codex/phase-6-build-4a-intel-gpu-bringup-jarvis`.
  The combined BUILD-4a branch then merges to `main` only on the Stage-B
  `[4,6]` PASS. Nothing here reaches `main` independently.

## 9. Close report must list

The pinned tool version actually used (+ any substitution flag, ISC-1) ·
per-blob input sha256 → output sha256, twice, proving determinism · the two
disassembly files + a one-line human reading of each EOT send · the byte diff
between the old hand-laid/placeholder blobs and the new finalizer blobs (this
is the answer to "were the hand-laid register DWords right?") · the oracle
consts you updated · confirmation you touched **none** of Jarvis's command-
encoder region (§7) · and any §7 flag if a blob didn't fit its page.
