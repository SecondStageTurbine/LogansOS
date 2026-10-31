<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: PR #88 write-guard follow-ups review — Muse (Gwen)

> Banked by the PM from the PL's paste (2026-10-21, ~00:00 UTC 10-22).

**Head reviewed:** `726c0d2188f5db6c3cd16c031e2cfdea4d092fa3` (d19fb29f + 726c0d21) · **Base:** main `659ad73a` · **Reviewer:** Muse (Gwen), cross-family vs Claude.
**Verdict: PASS on all five items.** KATs read, not run.

1. Blank-start sink rule (R1-7) — PASS. `write_guard_logic.rs:640-655` (drop LogSink claims on a table-less disk with a non-blank start); `start_is_blank` :554, `BLANK_START_BYTES` = 1 MiB :550; read error = not blank. KAT `r1_7_sink_needs_a_blank_start_on_a_table_less_disk`.
2. Extra whole-disk signatures — PASS. `whole_disk_contents` :378, probe table :401 (15 entries: XFS, btrfs, HFS+/HFSX, APFS, md 1.1/1.2, f2fs, ZFS, swap x2, ReFS), hit => `opaque-filesystem` :429. KAT `r1_7_more_whole_disk_signatures`.
3. PANIC_CPU id validated — PASS. `progress_logic.rs:127` `panic_cpu_id`; caller `progress.rs:86`. KAT `panic_cpu_id_validates_leaf_0xb`.
4. Panic-path sink bounded — PASS. `serial.rs:81` -> `write_fmt_panicking` `block_sink.rs:548`; `try_lock_bounded` :534 (`PANIC_LOCK_SPINS` = 1_000_000, :530); `panic_flush` :594; wired at `panic.rs:54`.
5. No widening — PASS (whole-diff audit, 11 files).

Observations (not fails): "start" = first 1 MiB by design; md v0.90/v1.0 end-of-device superblocks not probed; the item-4 bound is a spin count, not a wall-clock deadline.
