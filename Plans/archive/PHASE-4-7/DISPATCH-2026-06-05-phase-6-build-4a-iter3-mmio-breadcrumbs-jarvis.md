<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 6 BUILD-4a iteration 3 — locate the iter2 hard-hang + fix GT MMIO mapping → Jarvis

**Date:** 2026-06-05
**From:** PM/Cerebro
**To:** Jarvis (continue on `codex/phase-6-build-4a-intel-gpu-bringup-jarvis`, new commit on top of `503e092`)
**Status:** DISPATCHED — iter2 hard-hang follow-up; two-stage gate (Stage B PL-driven)
**Builds on:** BUILD-4a iter2 (`503e092`).

---

## 0. The iter2 Stage-B result (authoritative)

The OptiPlex run **hard-hung earlier than iter1**. The persisted NVMe log (218 bytes, sha256 `166045B0…`) contains only:
```
event=INFO ... bare_metal_log_sink=installed target_driver=nvme ...
event=INFO ... bare_metal_log_sink_selftest=begin
```
Serial got slightly farther but **never reached the new accel-intel forcewake/ring lines**. So the hang is between the log-sink selftest and the first forcewake breadcrumb — i.e. **in the new GT-register MMIO path**, and hard enough that nothing after `selftest=begin` flushed to NVMe.

**This is a regression from iter1** (which cleanly reached submit + timeout). The only iter1→iter2 delta is the forcewake/GT-register MMIO, so that path is faulting/hanging the CPU.

**Prime suspect:** the new **high-offset GT reads** — `FORCEWAKE_ACK_GT_GEN9 = 0x130044`, `GEN6_GT_CORE_STATUS = 0x138060` (~1.2 MiB into MMIO) — fall **outside the mapped MMIO window**, so the access page-faults → hang. iter1 only touched low offsets (ring registers near the base), which is why it didn't hang here.

## 1. Objective

First make the hang **locatable** (right now we can't see where it dies), then fix the most-likely cause (MMIO mapping). The next bare-metal run must leave a persisted breadcrumb naming the exact step that hangs — or PASS.

## 2. Implementation Success Criteria

- **ISC-1 — Breadcrumb-and-flush (observability; the unblock).** Add a helper that writes a short marker to the NVMe log sink **and flushes it to disk immediately** (so it survives a hard hang/fault). Emit a breadcrumb *before* each risky step, at minimum: `mmio-map`, `gt-bar-mapped`, `forcewake-write`, `forcewake-ack-poll-begin`/`-end`, `ring-mode-write`, `ring-state-verify`, each **named** status read (`read FORCEWAKE_ACK_GT`, `read GT_CORE_STATUS`, `read RING_FAULT`, `read RING_HEAD`), `submit`, `wait-begin`. The **last persisted breadcrumb = the hanging line.** (Per-breadcrumb NVMe flush is slow — acceptable, this is the one bring-up scenario.)
- **ISC-2 — Map the full GT MMIO BAR before any GT access.** Map the entire Gen9 GTTMMADR BAR (16 MiB) up front, and **bounds-check every GT register access** against the mapped size (log + skip, never fault, if an offset is somehow out of range). Confirm every offset in use (`0x130044`, `0x138060`, `0x4094`, `RENDER_RING_MMIO_BASE+0x29c`, the forcewake regs) is inside the mapping.
- **ISC-3 — No access can hard-hang.** Every forcewake-ACK / status poll is finite and **logs-and-continues** on timeout (never an unbounded or faulting read). A failed forcewake handshake degrades to a logged error + clean timeout (iter1-style), not a freeze.
- **ISC-4 — Oracle + standards.** Command-byte oracle (`intel_gen9_add_batch_matches_hand_byte_oracle`) stays green; `accel-intel` default-off; SPDX on new files; no new kernel dep; SAFETY + PRM citations on the MMIO unsafe; `cargo fmt --all --check`; host-checks + `shard-coverage 4` green; full sharded suite green with `accel_intel_add_bringup` hardware-only.
- **ISC-5 — Record it.** ADR-032: note the iter2 hard-hang, the full-BAR-mapping + bounds-check resolution, and the breadcrumb-flush observability approach.

## 3. Close gate (two-stage)

- **Stage A (you):** builds with `accel-intel` on; oracle green; CI suite + host-checks green with the feature off; fmt clean. Push; PM reviews the breadcrumb + mapping diff.
- **Stage B (PL):** fresh OptiPlex run; the NVMe log shows **either** `accel_intel_add_bringup` PASS (readback `== [4.0, 6.0]`) **or** a final breadcrumb that names the exact hanging MMIO step (+ whatever forcewake/ring/status it managed to read). **Merge only on Stage-B PASS;** a localized hang returns to PM for iteration 4.

## 4. Scope guard

Still the single `Add` bring-up — **observability + MMIO-mapping hardening only**. No op-set breadth, **no ELSP/execlist** (we haven't reached the submit path yet — don't change submission until the breadcrumbs prove we get there), no display/modeset. The deliverable is: the next run can't hang silently — it either passes or tells us precisely where the wall is.
