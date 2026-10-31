# HANDOFF (ROUND 2) — Phase 3 BUILD-5b AHCI log-sink DMA bug → Jarvis — KERNEL FIX REQUIRED

**Date:** 2026-05-28
**From:** PM (Cerebro)
**To:** Jarvis
**Supersedes scope of:** [HANDOFF-2026-05-28-phase3-build5b-ahci-log-sink-dma-bug-to-jarvis.md](HANDOFF-2026-05-28-phase3-build5b-ahci-log-sink-dma-bug-to-jarvis.md) (Round 1)
**Disposition on your last commit:** [PM-DISPOSITION-2026-05-28-phase3-build5b-ahci-dma-coverage-commit-reviewed-blocker-stays-open.md](PM-DISPOSITION-2026-05-28-phase3-build5b-ahci-dma-coverage-commit-reviewed-blocker-stays-open.md)
**BLOCKER (still OPEN):** [BLOCKER-2026-05-28-phase3-build5b-ahci-log-sink-dma-wrong-lba-and-rodata-leak.md](BLOCKER-2026-05-28-phase3-build5b-ahci-log-sink-dma-wrong-lba-and-rodata-leak.md)

## Read this first — what happened to Round 1

Your commit `87a49d1` ("tools: gate bare-metal log sink qemu layout") was **accepted as a standalone QEMU-coverage improvement and kept on `origin/main`.** Closing that coverage gap was real, useful work and it is not being reverted. Thank you for it, and thank you for the transparent commit message.

But it **did not close the BLOCKER**, and the BLOCKER is still open. Here is the precise reason, because it matters for how you approach Round 2:

- Your diff changed only `tools/xtask/src/main.rs`. No kernel code changed. The defect is in the kernel's AHCI log-sink **write submission**, so it is by-construction untouched.
- You concluded "not a reproduced AHCI DMA defect" because old-code QEMU with the bare-metal topology wrote/extracted `NEO_LOG_v1` correctly at `0x10000000`. **That inference is wrong.** QEMU not reproducing a bug that bare metal *did* reproduce is confirmation that QEMU's AHCI/SATA emulation masks it — open-question possibility #1 — not evidence the kernel is innocent.
- The bare-metal evidence is confound-free. The Samsung SATA was freshly `dd`'d and byte-for-byte `cmp`-clean before the run; the post-run `NEO_LOG_v1` + ~500 bytes of `.rodata` at `0x434A51` was provably kernel-written (the raw has no such magic). The kernel wrote its own `.rodata` to a clean disk at the wrong offset. That is the defect. It is real and it is on real hardware.
- Consequence: your new QEMU gate **passes the exact path bare metal fails.** It cannot catch this defect. Do not rely on it as your validation signal for Round 2.

## The single most important instruction for Round 2

**Your validation gate is BARE METAL, not QEMU.** QEMU is proven blind to this class. You will likely not be able to reproduce this bug in QEMU at all. That means: **fix by code inspection, not by reproduce-iterate.** PL's bare-metal rerun is the proof. Build a fix you can defend by reading the code, and state your confidence basis explicitly.


## The method we want — side-by-side driver comparison

This is the step the Round-1 HANDOFF asked for and that the harness work skipped. The forensic evidence narrows the defect to one intersection: **AHCI driver × log-sink write submission.** Three adjacent paths are proven working and are your reference oracles:

- AHCI-driver NeoFS scenario write (`create_write_fsync_readback=ok` on Samsung SATA) — AHCI driver writes correctly *for the FS path*.
- NVMe-driver NeoFS scenario write — works.
- **NVMe-driver log-sink write** — works (AHCI lane extracted 9202 clean bytes at the correct `0x10000000`).

So the *log-sink write* job is done correctly by the NVMe driver, and the AHCI driver writes correctly for the *FS* job. The bug is where the AHCI driver's log-sink write path diverged from the NVMe driver's log-sink write path. Put the two implementations next to each other and find the divergence. Concretely:

```
rg -n "bare_metal_log_sink|log_sink_install|init_block_log_sink|NEO_LOG_v1" kernel/src
rg -n "fn write_block|fn write_blocks|fn submit_write|fn issue_write" kernel/src/drivers/ahci kernel/src/drivers/nvme kernel/src/drivers/block
```

Then read the AHCI write submission and the NVMe write submission for the log-sink caller, line by line, against each other.

## The two bugs — both must be fixed

### Bug 1 — destination LBA arithmetic
Declared `offset=0x10000000`; data landed at `0x434A51`. Not a clean integer factor. Likely: bytes-vs-sectors unit confusion in the LBA computation, a base offset added/subtracted wrong, a 28-bit/48-bit LBA truncation, or a stale/uninitialized FIS LBA field. The NVMe driver computes the same `0x10000000` correctly — diff against it.

### Bug 2 — DMA source pointer + length (the security half)
The on-disk bytes after the magic are verbatim kernel `.rodata` (Rust source paths, panic/assert format strings, crate registry paths). Hypothesis: the write passes the **address of the `NEO_LOG_v1` magic constant in `.rodata`** as the DMA source, with a length far exceeding the 10-byte magic + intended header, so the DMA walks into surrounding `.rodata`. The source must be a properly-sized log-entry buffer (heap or per-CPU log-staging area) and the length must equal the actual entry size.

**A "fix" that only relocates the write to `0x10000000` but still over-DMAs is NOT acceptable** — it would close the BLOCKER from the extractor's view while still leaking kernel memory to a new disk location. Bound the source/length. This is a confidentiality boundary, not just a correctness bug.

## Return contract (match the `72a0a41` format)

```
Root cause: <which file/lines in the AHCI driver diverged from the NVMe driver, for BOTH the LBA arithmetic and the DMA source/length>

Commit(s):
<full-sha> <subject>   (kernel fix; name the BLOCKER in the body)
[<full-sha> <subject>  — if you split the LBA fix from the length/source fix]

Confidence basis (REQUIRED, since QEMU can't validate this):
<why you believe the fix is correct from code inspection — e.g., "AHCI now uses the same sector-unit conversion as nvme::write_blocks at line X; DMA source is now the heap log buffer bounded to entry_len, matching nvme path">

Validation:
- cargo k: PASS
- neo-extract-log tests: PASS
- QEMU AHCI / NVMe (coverage only — note explicitly that QEMU cannot prove this defect fixed)
- AHCI + NVMe bare-metal build-only: PASS

Restaged SHA256: <all 7 entries>

PL bare-metal rerun instruction: <NVMe lane; if you touched AHCI-side code that the AHCI lane also exercises, request a BOTH-lane rerun>
```

## Do NOT

- Do **not** treat a green QEMU run as proof the defect is fixed. It is proven blind to this class. Your evidence is code inspection + PL's bare-metal rerun.
- Do **not** touch anything outside the AHCI driver + log-sink write path + minimal interface needed. Do not refactor the block-device trait, do not "improve" the NVMe driver (it's your reference oracle — leave it), do not revisit the APIC ordering or the `87a49d1` harness work.
- Do **not** half-fix: both the LBA and the DMA source/length must land before PM accepts.
- Do **not** commit on detached HEAD; `git checkout main` first.

## Acceptance gates PM will check on return

1. Kernel diff present and touches the AHCI log-sink write submission (this time there must be kernel code).
2. Both LBA arithmetic AND DMA source/length addressed; the `.rodata`-leak half is fixed, not deferred.
3. Diff matches stated root cause — PM inspects every changed file and diffs the AHCI path against the NVMe reference.
4. Commit(s) on `origin/main`, branch `main`, not detached; BLOCKER named in body.
5. Confidence-basis section present and defensible by code reading.
6. All 7 staging hashes re-verified independently (this gate runs in Round 2 — we will be greenlighting a bare-metal rerun).
7. Honest acknowledgment that QEMU cannot validate this; bare metal is the gate.

After PM acceptance, PL reruns the NVMe lane on the OptiPlex 7080 (Samsung SATA forensic evidence is preserved until then). Gate ledger: framebuffer PASS, extracted PASS marker, Captured bytes > 0, no Degradation block, AND — new for this BLOCKER — a magic-scan of `/dev/sda` showing `NEO_LOG_v1` at the correct `0x10000000` with a valid header (not `.rodata`) following it, and no stray magic elsewhere. On that, Phase 3 BUILD-5b formally closes.

Standing by for your Round 2 report.
