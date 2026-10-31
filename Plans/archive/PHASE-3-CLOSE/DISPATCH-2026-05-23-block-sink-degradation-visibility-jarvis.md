# Dispatch — BlockDeviceLogSink degradation visibility + boot-time self-test

**Date filed:** 2026-05-23
**Lane:** Jarvis (Codex)
**Status:** ACTIVE
**Filed by:** PM
**PL authority:** verbal go-ahead 2026-05-23 ("yes" to dispatch the diagnostic visibility fix)
**Predecessor:** `DISPATCH-2026-05-22-block-device-log-sink-and-phase-indicator-jarvis.md` (CLOSED)
**Blocks:** BUILD-5b re-execution on OptiPlex 7080 Micro (Phase 3 formal close path)

---

## 1. Context

PL executed BUILD-5b AHCI scenario on OptiPlex 7080 Micro on 2026-05-22. Outcome:

- Framebuffer rendered plain **"FAIL"** (red).
- `neo-extract-log` against `/dev/nvme0n1` produced:
  ```
  Found NEO_LOG_v1 header at offset 0x10000000
  Scenario: phase3_ahci_neofs_mount_and_readwrite
  Start timestamp ns: 0
  Captured bytes: 0
  ```
- No kernel diagnostics survived. We have **zero observability** into what failed.

### 1.1 What we know

- The plain-text "FAIL" rules out `strict_bare_metal_halt`. Per `kernel/src/scenario/phase_indicator.rs::fail_text`, the strict-halt reasons ("no log sink device available", "log sink install failed", "log sink invalid") map to "FAIL NO LOG" or "FAIL LOG" on the framebuffer. Plain "FAIL" comes only from `debug_exit(EXIT_FAILURE)` after `crate::scenario::phase(ScenarioPhase::Fail("FAIL"))`. So:
  - `prepare_bare_metal_mode()` reached the `serial_println!` INFO line at `kernel/src/test_harness.rs:683-689`.
  - `install()` succeeded — the 4096-byte initial header was written (extraction confirms magic + scenario name).
  - The scenario itself ran and eventually called `debug_exit(EXIT_FAILURE)`.
  - `debug_exit` calls `crate::log::flush()` before halting (`test_harness.rs:10813-10825`).
- Despite this, `write_cursor` in the on-disk header is still 0 after the run. So **no payload flush ever updated the header**.

### 1.2 Most likely failure mechanism (hypothesis, not certainty)

`BlockDeviceLogSink::install()` writes the full 4096-byte header via `write_bytes_at()`. Because `take == block_size`, the read-modify-write branch is skipped — the device only sees a `write_block()` call. This works on real Intel E6100p NVMe.

Every subsequent flush of staged log content writes <4096 bytes, hits the read-modify-write branch in `write_bytes_at`, and calls `self.device.read_block(lba, ...)` before the `write_block`. If `read_block` returns `Err` (driver issue, queue issue, IRQ-vs-poll mismatch, alignment, timing — unknown) the flush returns `Err`, `flush_locked` marks the sink degraded, and `inner.staged = 0` discards the buffered log content. The sink then silently drops every subsequent byte; the `serial_println!` that would have reported the degradation goes through `_print` → `crate::log::write_fmt` → the now-dead sink. Nothing reaches disk except the original header.

**This hypothesis may be wrong.** Other candidates: first `write_block` returns `Err` even with the read removed; `rewrite_header` step inside `flush_locked` fails; `device.flush()` (the trailing supports_flush block) hangs or errors; a panic-without-flush path we haven't identified. The point of this dispatch is **not to commit to one hypothesis** — it is to add the observability that lets the next hardware run tell us which one is true.

## 2. Goal

After this dispatch lands, the next BUILD-5b execution either:

1. **Succeeds** because the underlying issue resolves once the self-test catches it early and the sink path is hardened, OR
2. **Fails with diagnostic information** that survives the failure — written by a path that does not depend on the same primitives that are failing. `neo-extract-log` prints actionable detail; the framebuffer shows a distinct failure state.

We do not commit, in this dispatch, to fixing the underlying read-modify-write or NVMe driver issue. We commit to making it visible.

## 3. Acceptance criteria

### C1 — Degradation reason recorded out-of-band

When `BlockDeviceLogSink` transitions to degraded state, persist the reason via a **single full-block write** to the on-disk header (no staging buffer, no read-modify-write, no recursion through `_print`). The header gains a degradation field carrying:

- `degradation_reason: u32` — enumerated (Io, InvalidRequest, Unsupported, Other), 0 = not degraded.
- `degradation_op: u32` — what we were doing (0 = idle, 1 = read_block_for_rmw, 2 = write_block_payload, 3 = write_block_header_rewrite, 4 = device_flush_after_write).
- `degradation_lba: u64` — the LBA the failing op targeted (0 if not applicable).
- `degradation_cursor: u64` — write_cursor at the time of degradation.

These fields live in the currently-reserved area (after `start_timestamp_ns` at offset 0x88, well under the 4096-byte header). Header serialization version field is added if it doesn't already exist; bump it.

The diagnostic-write path is a new function that constructs a full 4096-byte buffer holding the current header content (magic, region_size, scenario_name, build_id, etc.) PLUS the degradation fields, then issues `device.write_block` directly — no read first, no staging. If even this write fails, we have no path left; that's acceptable — at least we tried, and on the next attempt the sink is already known-degraded.

### C2 — Boot-time self-test in bare-metal mode

After `install()` succeeds in `prepare_bare_metal_mode()`, add a self-test that:

1. Emits a known sentinel via `serial_println!` — exact string: `NEO_TEST/1 event=INFO name={SCENARIO} bare_metal_log_sink_selftest=begin`.
2. Calls `crate::log::flush()`.
3. Re-reads the header from the device (via the sink's device handle and a fresh DmaBuffer, no caching).
4. Verifies `write_cursor > 0` AND the payload region at offset `HEADER_SIZE` (LBA-relative) contains the sentinel substring.

Outcomes:

- **Pass:** continue boot, emit `serial_println!("NEO_TEST/1 event=INFO name={SCENARIO} bare_metal_log_sink_selftest=ok cursor={}", cursor)`, proceed to scenario.
- **Fail (cursor still 0, or sentinel not found, or any error in the self-test path itself):** call `strict_bare_metal_halt` with a new reason `"log sink selftest failed"`. Mapped in `phase_indicator::fail_text` to a distinct framebuffer state: `"FAIL LOG SELFTEST"`.

The self-test runs only under `#[cfg(feature = "neo-test-bare-metal-mode")]`. QEMU scenarios are unaffected.

### C3 — Extraction tool prints degradation fields

`tools/neo-extract-log/src/main.rs`: when the header `degradation_reason != 0`, print after the existing header summary:

```
Degradation detected:
  reason: <name>
  op:     <name>
  lba:    <decimal>
  cursor: <decimal>
```

If `degradation_reason == 0`, no degradation block printed (backwards-compatible with existing manifest format).

### C4 — Framebuffer self-test FAIL state

`kernel/src/scenario/phase_indicator.rs::fail_text`: add `"log sink selftest failed"` → `"FAIL LOG SELFTEST"`. Five-line addition.

### C5 — No regression in existing QEMU scenarios

`docker compose run --rm neo cargo neo-test phase3_ahci_neofs_mount_and_readwrite` (without `--bare-metal-mode`) must still PASS in QEMU under the existing harness. Same for `phase3_nvme_neofs_mount_and_readwrite` and any other QEMU scenario the harness touches. Self-test path must not execute outside bare-metal-mode.

### C6 — Bare-metal-mode ISOs rebuild cleanly

`docker compose run --rm neo cargo neo-test --build-only phase3_ahci_neofs_mount_and_readwrite --bare-metal-mode` produces a fresh `.bare-metal.iso` + scenario raw + log-sink raw. Same for nvme. No new build warnings, no errors. Confirm ISO and raw sizes are within ±5% of the prior dispatch's outputs (24.7 MB ISO, 128 MiB scenario, 288 MiB log-sink).

### C7 — Document in the dispatch close-report

Provide a CLOSE-REPORT that:

- Confirms each acceptance criterion's status.
- Shows the new header layout (offsets table).
- Lists the new framebuffer state name.
- Lists files changed and approximate line counts.
- Notes that **runtime verification of C1/C2/C4 happens during the next BUILD-5b hardware run** — Codex cannot exercise the strict-halt or degradation path in QEMU because the bug only manifests on real hardware.

## 4. Implementation notes (suggestions, not prescriptions)

Codex has discretion within the acceptance criteria. The notes below are PM's working assumptions; deviate freely if there's a cleaner path.

- **Header layout:** add a `header_version: u32` at offset 0x08–0x0c (currently inside the magic padding). The current magic occupies bytes 0x00–0x0b (b"NEO_LOG_v1\0" is 11 bytes); a u32 at 0x0c won't conflict if you adjust the magic field length to 12 instead of 16. Alternatively, keep magic length 16 and put `header_version` at 0x90 alongside the new degradation fields.
- **Degradation write path:** add `fn write_degradation_marker(&self, reason: DegradationReason, op: DegradationOp, lba: u64, cursor: u64) -> BlockResult<()>` to `BlockDeviceLogSink`. This rebuilds the header buffer with new fields, calls `self.device.write_block(lba_of_header, ...)` directly. Do **not** call this from inside `flush_locked` while holding the inner lock — release the lock first or restructure so `mark_degraded` can call it without deadlock.
- **Self-test re-read:** the sink's device handle is reusable. Allocate a fresh DmaBuffer in `prepare_bare_metal_mode` (after install, before scenario dispatch). `device.read_block(lba_of_header, ...)` to confirm the cursor advanced. The cursor field is at the offset you defined (currently 0x018..0x020).
- **Sentinel string length:** keep the self-test message under 1024 bytes total so a single flush is sufficient. Match the existing `bare_metal_log_sink=installed` line for format.
- **Op enumeration in DegradationOp:** keep it small — the four states identified in C1 cover the four call sites in `flush_locked` and `write_bytes_at`. Don't try to enumerate every possible failure point; we just need to know which of those four was active.
- **Existing failure path through `mark_degraded`:** the current implementation calls `serial_println!` from inside `mark_degraded`. Keep that call (it's harmless on bare-metal because serial is absent) but **additionally** call the new diagnostic-write path. The serial call has its own re-entrancy guard via `IN_FLUSH`.
- **Don't auto-flush per `_print`:** that was a tempting fix but introduces latency throughout the kernel for every scenario. Keep the staging buffer behavior; the self-test gives us the assurance we need that the path works.

## 5. Out of scope

- **Fixing the underlying NVMe / AHCI / read-modify-write issue.** That's a follow-on dispatch after we see the diagnostic data.
- **Changing the staging buffer size or auto-flush threshold.** Defer.
- **Adding more granular phase indicator states.** The new "FAIL LOG SELFTEST" plus the existing set is sufficient.
- **Re-running on hardware.** PL re-runs after this dispatch closes and the new ISOs land.

## 6. Verification expectations

Codex should:

1. Run `docker compose run --rm neo cargo neo-test phase3_ahci_neofs_mount_and_readwrite` (QEMU, no bare-metal-mode). Must PASS unchanged.
2. Run `docker compose run --rm neo cargo neo-test phase3_nvme_neofs_mount_and_readwrite` (QEMU). Must PASS unchanged.
3. Run `docker compose run --rm neo cargo neo-test --build-only phase3_ahci_neofs_mount_and_readwrite --bare-metal-mode`. Must produce ISO + raws cleanly.
4. Run `docker compose run --rm neo cargo neo-test --build-only phase3_nvme_neofs_mount_and_readwrite --bare-metal-mode`. Same.
5. Run `cargo test -p neo-extract-log`. Extraction tool tests pass.
6. Optional but encouraged: write a unit test in `block_sink.rs` covering the header serialization with degradation fields populated. The existing `serialize_header` is testable on host.

Runtime self-test verification (C2's actual triggering) and runtime degradation-marker verification (C1's actual triggering) are **not expected** in Codex's verification — they require real-hardware behavior we cannot reproduce in QEMU. Compile-only verification of those paths is acceptable. PL will exercise them at next BUILD-5b run.

## 7. Files Codex is expected to touch

- `kernel/src/log/block_sink.rs` — degradation fields in header, diagnostic write path, new mark_degraded behavior. ~80–120 new lines.
- `kernel/src/test_harness.rs` — self-test call after install in `prepare_bare_metal_mode`. ~30–50 new lines, scoped to `#[cfg(feature = "neo-test-bare-metal-mode")]`.
- `kernel/src/scenario/phase_indicator.rs` — new fail_text mapping. ~3 line addition.
- `tools/neo-extract-log/src/main.rs` — read + print degradation fields. ~30 lines.
- `tools/neo-extract-log/Cargo.toml` — no changes expected.

Total: ~150–200 net new lines plus minor edits.

## 8. Hand-off contract

When Codex completes:

1. Code committed and pushed (Codex's normal pattern) OR returned to PM for PM-side commit (matches predecessor dispatch). Either works.
2. CLOSE-REPORT filed at `Plans/CLOSE-REPORT-2026-05-23-block-sink-degradation-visibility.md`.
3. PM consumes via PM-DISPOSITION.
4. PL re-stages BUILD-5b raws (the staged folder needs the new bare-metal-mode ISOs + raws; PM updates the staging folder and notifies PL).
5. PL re-runs the AHCI scenario on the OptiPlex. We learn what's actually broken.

---

**End of dispatch.**

*This dispatch buys observability, not a fix. The fix follows after we can see what's wrong.*
