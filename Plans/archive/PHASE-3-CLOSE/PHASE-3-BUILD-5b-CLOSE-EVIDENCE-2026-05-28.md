# PHASE 3 BUILD-5b — BARE-METAL CLOSE EVIDENCE — 2026-05-28

**Status:** BUILD-5b **CLOSED** (bare-metal green, all gates discharged)
**Phase:** 3 (NeoFS, block layer, real disk I/O)
**Hardware:** Dell OptiPlex 7080 Micro (Service Tag D4SHKD3) — Intel NVMe `/dev/nvme0n1` (scenario), Samsung 860 EVO SATA/AHCI `/dev/sda` (log sink)
**Kernel commit validated:** `2fdb7cb1bfa25e8706d9a4fb78ca230b3b549488` — "kernel: publish ahci command table before doorbell"
**Discharges the gate that** [PM-DISPOSITION-2026-05-28-phase3-build5b-ahci-dma-2fdb7cb-fix-accepted-greenlight-baremetal.md](PM-DISPOSITION-2026-05-28-phase3-build5b-ahci-dma-2fdb7cb-fix-accepted-greenlight-baremetal.md) **was pending on.**
**Closes BLOCKER:** [BLOCKER-2026-05-28-phase3-build5b-ahci-log-sink-dma-wrong-lba-and-rodata-leak.md](BLOCKER-2026-05-28-phase3-build5b-ahci-log-sink-dma-wrong-lba-and-rodata-leak.md)
**Serial log (chain-of-custody):** `Plans/evidence/phase3-bare-metal-optiplex-7080-20260528-nvme-build5b-log-sink-rerun.serial.log`

## NVMe-lane rerun result — all four close gates GREEN

| Gate | Result | Evidence |
|------|--------|----------|
| 1. Framebuffer PASS (green) | ✅ | PL confirmed green; serial PASS at log line 178. (Framebuffer photo: outstanding artifact, non-blocking.) |
| 2. Extracted PASS marker + bytes > 0 + no Degradation | ✅ | `neo-extract-log` → `EXTRACT_STATUS=0`, Captured bytes **9191**, PASS marker present, no FAIL/Degradation/PANIC/BUG lines |
| 3. Extraction at default offset succeeds | ✅ | Header parsed at `0x10000000`; `/tmp/nvme-run-log.txt` written |
| 4. Deterministic leak check (the decisive gate) | ✅ | Magic-scan of `/dev/sda` first 512 MiB: `0:NEOPLUTARCH_PHASE` and `268435456:NEO_LOG_v1` (= `0x10000000`) **only** — no hit at `0x434A51`, no stray `NEOFSv1`, valid header (not `.rodata`) following |

## Why this is a genuine close and not a repeat of the deceptive PASS

The kernel-side serial signature of this run is byte-identical to the **failing** run (`bare_metal_log_sink_selftest=ok cursor=246`, `PASS ... bytes=30`) — the kernel selftest reads back from wherever it wrote, so it cannot distinguish correct from corrupt. The close therefore rests entirely on the **on-disk witness**: the `NEO_LOG_v1` magic now lands at the declared `0x10000000` with a valid, extractor-parseable header, and the old corruption signature at `0x434A51` (magic + ~500 bytes leaked kernel `.rodata`) is **absent**. Capture size 9191 bytes sits alongside the AHCI lane's known-good 9202 — a full clean log, not a truncated artifact.

## Root cause — confirmed by the fix's effect

The fix changed only the publication ordering (`fence(Release)` before the AHCI `PX_CI` doorbell, `fence(Acquire)` on completion; `kernel/src/drivers/block/ahci.rs`), touching neither the LBA arithmetic nor the PRDT source/length. Both symptoms — wrong destination LBA **and** the `.rodata` over-DMA — vanished together. That a pure publication fence resolves both proves the defect was a single publication-ordering bug: the HBA was fetching stale/unpublished command-table state (FIS LBA + PRDT DBA/DBC) before the freshly-written, already-bounded descriptor was visible. The in-memory descriptor was correct all along (PRDT bounded to `buffer.len()`, source = `buffer.physical_addr()`); it simply was not published before the doorbell. The security `.rodata`-leak concern is closed by the same fix.

## Scope / forward notes

- AHCI lane: green since `72a0a41` (unchanged). NVMe lane: now green. All four BUILD-5b storage paths validated on bare metal.
- The QEMU coverage gate added in `87a49d1` remains as a CI improvement, with the standing caveat that QEMU does not reproduce this hardware-visibility class — bare metal stays the arbiter for AHCI publication defects.
- Per `INDEX.md`, BUILD-5b "terminates Phase 3 formal close path." **Phase 3 formal close (squashed commit + phase tag + bare-metal verification, Bible Article 13) is a PL-tier action and is NOT performed by this evidence doc.** It is now unblocked.
- Samsung SATA forensic evidence from the failing run is now superseded by this clean run and may be reprovisioned freely.

---

**BUILD-5b CLOSED.** Evidence complete.
