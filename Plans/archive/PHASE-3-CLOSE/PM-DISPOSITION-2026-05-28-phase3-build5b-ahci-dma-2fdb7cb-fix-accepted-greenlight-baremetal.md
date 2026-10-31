# PM-DISPOSITION — Phase 3 BUILD-5b — Jarvis commit `2fdb7cb` (AHCI publish-barrier fix) ACCEPTED on review; greenlit for bare-metal validation

**Date:** 2026-05-28
**Phase:** 3
**Build:** BUILD-5b
**Reviewer:** PM (Cerebro / Claude)
**Subject commit:** `2fdb7cb1bfa25e8706d9a4fb78ca230b3b549488` — "kernel: publish ahci command table before doorbell"
**BLOCKER:** [BLOCKER-2026-05-28-phase3-build5b-ahci-log-sink-dma-wrong-lba-and-rodata-leak.md](BLOCKER-2026-05-28-phase3-build5b-ahci-log-sink-dma-wrong-lba-and-rodata-leak.md) — **remains OPEN; closes only on green bare-metal NVMe-lane rerun.**
**Round-1 disposition (for history):** [PM-DISPOSITION-2026-05-28-phase3-build5b-ahci-dma-coverage-commit-reviewed-blocker-stays-open.md](PM-DISPOSITION-2026-05-28-phase3-build5b-ahci-dma-coverage-commit-reviewed-blocker-stays-open.md)

## Verdict

Commit `2fdb7cb` is ACCEPTED on code review and greenlit for bare-metal validation. It is the genuine kernel fix Round 1 lacked: a release publication fence before the AHCI `PX_CI` doorbell and an acquire fence on completion, conforming AHCI to the NVMe submit/complete discipline. The BLOCKER does not formally close until PL's bare-metal NVMe-lane rerun passes the close gate below.

## Round-2 cadence ledger — all gates green

| # | Gate | Result | Evidence |
|---|------|--------|----------|
| 1 | Commit on `origin/main`, branch `main`, not detached | ✅ | `git log origin/main` top = `2fdb7cb`; `--pretty=fuller` shows author/commit dates |
| 2 | Commit message names the BLOCKER | ✅ | Body cites the BLOCKER path and frames LBA + `.rodata` leak as one publication-ordering bug |
| 3 | Kernel diff present, matches stated root cause | ✅ | `kernel/src/drivers/block/ahci.rs` +7/-1: `fence(Release)` between `write_prdt()` and `PX_CI` (line 423); `fence(Acquire)` on completion (line 447). Correct placement. |
| 4 | Security `.rodata`-leak half addressed | ✅ | `write_prdt` (line 600+) sources DMA from `buffer.physical_addr()`, sets DBC from `buffer.len()` with validation (`!=0`, `≤4MB`, sector-aligned). In-memory PRDT is bounded; no `.rodata` pointer or overshoot in source. Publication fix makes the device read that bounded descriptor. |
| 5 | Confidence basis defensible by code reading | ✅ | NVMe submit/complete uses identical `fence(Release)` (nvme.rs:487) / `fence(Acquire)` (nvme.rs:503); NVMe log-sink is proven correct on this OptiPlex. AHCI now matches. (Soft note: report gave root-cause prose rather than a labeled "Confidence basis" section; substance present.) |
| 6 | All 7 staging SHA256 independently re-verified | ✅ | `Get-FileHash` on `E:\...\BUILD-5b-staging\` — all 7 MATCH reported digests. Pattern consistent with kernel-only change (ISOs moved; raws + extractor unchanged). |
| 7 | Honest acknowledgment QEMU can't validate; bare metal is the gate | ✅ | Stated explicitly: "QEMU remains coverage-only; bare metal is the actual gate." |


## Residual caveat carried forward (not a blocker)

The descriptor writes in `write_prdt` are `write_volatile`, and on x86 write-back memory a `fence(Release)` is effectively a compiler barrier. So static analysis does not fully pin *why* bare metal failed while QEMU passed — most likely a write-combining/visibility window that the fence closes (the same mechanism under which NVMe's identical fence is empirically sufficient on this hardware). This is why:

- The fix is safe and well-justified (bounded descriptor + proven NVMe pattern), but
- **Bare metal is the genuine arbiter**, and a green result is conclusive only with the deterministic magic-scan in the close gate.
- **If the bare-metal rerun still fails**, the next investigation is the memory type (WB vs WC) of the AHCI command list/table/PRDT region and whether an actual `SFENCE` (which `fence(Release)` does not emit on x86) is required. That is a contingency, not a present blocker.

## Bare-metal NVMe-lane close gate (PL rerun)

A green rerun closes Phase 3 BUILD-5b. Required observations:

1. Framebuffer PASS (green) — **photograph it this time** (trip-wire #5; NVMe lane was not photographed last round).
2. Serial PASS marker: `NEO_TEST/1 event=PASS name=phase3_nvme_neofs_mount_and_readwrite ... create_write_fsync_readback=ok bytes=30`. (The `source=deviceN` index may differ from the prior run's `device1`; accept on marker substance, not the index.)
3. Extraction at default offset: `neo-extract-log --device /dev/sda` succeeds, Captured bytes > 0, no Degradation block.
4. **Deterministic leak check (new for this BLOCKER):** magic-scan of `/dev/sda` shows `NEO_LOG_v1` only at `0x10000000`, with a valid header (not `.rodata`) following, and no stray magic elsewhere.

On all four: file the closeout PM-DISPOSITION + index update; BLOCKER closes; BUILD-5b formally closes.

## Forensic-evidence note

This rerun re-`dd`s the Samsung SATA, overwriting the preserved forensic evidence at `0x4321A2` / `0x434A51`. That evidence has served its purpose — the BLOCKER and the NVMe-lane PARTIAL evidence preserve the full forensic chain on paper. Proceeding is approved. (If PL prefers a final `dd if=/dev/sda` capture before reprovisioning for belt-and-suspenders, that is cheap insurance but not required.)

---

**PM-DISPOSITION end.** Greenlit to PL for bare-metal NVMe-lane rerun.
