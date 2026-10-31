# PM-DISPOSITION — block-sink degradation visibility dispatch close consumption

**Date:** 2026-05-23
**Recorder:** PM
**Consumes:** `Plans/CLOSE-REPORT-2026-05-23-block-sink-degradation-visibility.md`
**Closes:** `Plans/DISPATCH-2026-05-23-block-sink-degradation-visibility-jarvis.md`
**Status:** CLOSED — dispatch transitions ACTIVE → CLOSED on this disposition

---

## 1. Disposition

PM accepts the close-report. All seven dispatch acceptance criteria (C1–C7) are PASS per Codex's verification. Code committed at `2223617` with attribution to Codex's dispatch.

## 2. Per-criterion ratification

| C | Substance | Disposition |
|---|-----------|-------------|
| C1 | Degradation reason recorded out-of-band | RATIFIED. `write_degradation_marker` constructs a full 4096-byte header and writes it via `device.write_block` directly — no staging buffer, no read-modify-write, no recursion through `_print`. The pre-v2 readers will see header_version=0 in old captures, degradation block absent; v2 readers parse the new fields when present. Backwards-compatible. |
| C2 | Boot-time selftest in bare-metal mode | RATIFIED. `bare_metal_log_sink_selftest` in `kernel/src/test_harness.rs:694` exercises emit-sentinel → flush → reread-header → cursor>0 → payload-contains-sentinel. Failure path strict-halts with reason `"log sink selftest failed"`, mapped to framebuffer state `FAIL LOG SELFTEST` per C4. Self-test is `#[cfg(feature = "neo-test-bare-metal-mode")]` scoped — does not run in QEMU. |
| C3 | Extractor prints degradation fields | RATIFIED. `neo-extract-log` prints a degradation block only when `degradation_reason != 0`. Existing captures (with reason=0 or header_version=0) remain quiet — manifest format unchanged for non-degraded runs. |
| C4 | Framebuffer self-test FAIL state | RATIFIED. `phase_indicator::fail_text` adds the single-line mapping `"log sink selftest failed" → "FAIL LOG SELFTEST"`. |
| C5 | No QEMU regression | RATIFIED. AHCI and NVMe NeoFS read/write QEMU scenarios PASS without `--bare-metal-mode`. `cargo test -p neo-extract-log` 6 tests PASS. |
| C6 | Bare-metal-mode ISOs rebuild cleanly | RATIFIED. AHCI and NVMe `--bare-metal-mode --build-only` produced ISOs (24,750,080 bytes each, +60 KB / 0.2% growth from added code), 128 MiB scenario raws (unchanged), 288 MiB log-sink raws (unchanged). Sizes match pre-dispatch within ±5% tolerance from dispatch §3 C6. |
| C7 | Close-report filed | RATIFIED. Close-report present at expected path with header layout table, file change inventory, verification command list, runtime caveat. |

## 3. Runtime caveat acknowledgment

Codex correctly notes (close-report §Runtime Caveat) that **C1, C2, and C4 runtime triggers cannot be exercised in QEMU** because the captured_bytes=0 failure manifests only on real Intel E6100p NVMe hardware. PM accepts this caveat — it is identical to the predecessor dispatch's C2/C4 runtime deferral, and the next BUILD-5b execution on the OptiPlex 7080 Micro is the runtime verification gate. Authority: Bible Article 14 /simplify + Article 1.2 correctness-before-performance.

PM does not require a second QEMU pass to trigger the strict-halt path artificially (e.g., via a fault-injection feature flag). That would expand scope without changing what we can learn — the real hardware run will either:

1. Show **`FAIL LOG SELFTEST`** on the framebuffer, indicating the self-test caught a real first-payload-flush failure. The on-disk header v2 degradation fields then tell us which primitive failed and at what LBA. We then dispatch a fix targeting that primitive.
2. Show the existing `FAIL` text (now with `header_version=2` and possibly non-zero degradation fields if a later flush failed). Extraction prints the degradation block. We dispatch accordingly.
3. PASS — possible if the prior FAIL was an intermittent issue unrelated to the log path. Less likely but not ruled out.

In all three outcomes, we have actionable diagnostic data we did not have on the 2026-05-22 attempt.

## 4. What this disposition unblocks

- **PM-side:** restage `BUILD-5b-staging/` with the new ISOs + raws (PM action item, done same-day per §6 below).
- **PL-side:** BUILD-5b re-execution is unblocked. Operator execution guide at `Plans/BUILD-5b-EXECUTION-GUIDE-2026-05-22-optiplex-7080.md` is still current; same dd-target paths, same procedure.

## 5. What this disposition does NOT do

- Does NOT close BUILD-5b. Runtime verification still pending.
- Does NOT close Phase 3. Phase 3 formal close gates on BUILD-5b runtime evidence + PM-DISPOSITION.
- Does NOT open Phase 4 PLAN drafting. Still gated on Phase 3 close per Q10.
- Does NOT propose Bible amendment or ADR. Header v2 is an internal evolution of the BlockDeviceLogSink, not an external interface contract; no documentation precedent invoked.

## 6. PM action items (same-day)

- [x] Commit Codex's kernel + tools changes at `2223617`.
- [x] Commit close-report.
- [x] File this PM-DISPOSITION.
- [ ] Restage `BUILD-5b-staging/` with the new ISOs + raws from `target/neo-test/vmware/`.
- [ ] Update INDEX.md.
- [ ] Notify PL that re-staged files are ready and the next OptiPlex run is unblocked.

## 7. Lane status after this disposition

- **PM:** clearing restage + INDEX, then standby for BUILD-5b runtime evidence.
- **Codex (Jarvis):** idle. Dispatch closed.
- **Cerebro:** idle.
- **Prometheus:** idle.
- **PL:** holds (a) BUILD-5b hardware session scheduling, (b) optional Q10 reverse to open Phase 4 PLAN drafting in parallel.

---

**End of disposition.**

*The dispatch bought observability, not a fix. Next hardware run buys us the answer to "what's actually failing." The dispatch after that buys the fix itself.*
