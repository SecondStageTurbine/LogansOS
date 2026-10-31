<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: mt7601u — bound + instrument the post-firmware control plane (kill the pre-sink wedge) → Jarvis

**Date:** 2026-07-26
**From:** PM (Claude)
**To:** Jarvis (WiFi / mt7601u owner)
**Status:** DISPATCHED — **kernel-touching, QEMU-gated; feeds the WiFi silicon lane.** Rebuilds the intent of an orphaned PM-draft (`wip/pm-draft-mt7601u-warm-postfw-bound` @ `7b35d1d`) as a proper coder-owned change. **The draft is design-reference ONLY — do not cherry-pick/merge it (PM-authored, unreviewed); build it yourself with the full gate. I'll delete the draft branch once this is committed.**

## Why this matters (the silicon history demands it)
TEST-3 WiFi silicon showed the failure mode this fixes: a warm-resident dongle took the WARM-RESYNC path, then **"emitted nothing for nearly seven minutes before the first MAC-init `RANDOM_WRITE`."** Root cause the draft names: the **first post-firmware WPDMA read inherited xHCI's full default wait budget** (incl. Linux-style retries) and **wedged for minutes — before the NVMe sink was even installed**, so the wedge produced NO durable evidence. A blind-hardware run MUST be diagnostic on failure (standing rule); right now that path is a silent multi-minute stall. This makes the next WiFi silicon boot **self-diagnosing** on exactly the seam that has bitten us.

## Scope (the draft's intent, built properly)
1. **Bound the post-FW control plane.** Add a `POST_FW_WAIT_BUDGET` (draft: `50_000`, far above a valid handful-of-events read, far below a multi-minute wedge) applied via the xHCI wait-budget guard across the post-firmware phase (WPDMA-idle, ASIC-ready, CSR/BBP reset, USB-DMA init). A stale warm-resident device fails **fast with diagnostics** instead of stalling pre-sink.
2. **A retained discriminator + breadcrumbs.** `POST_FW_STATE` (NOT_RUN/WPDMA_WAIT/WPDMA_IDLE/WPDMA_TIMEOUT/DEVICE_UNRESPONSIVE/ASIC_READY/CONTROL_PLANE_READY) + `POST_FW_WPDMA_LAST`, with per-stage serial breadcrumbs (`POST-FW begin op=.. / result=..`) and a `#[cfg(feature="neo-test")] debug_post_firmware_breadcrumbs()` the bare-metal scenario replays into the sink. Use the bounded `cold_poll` → `Done/Unresponsive/Retry` (the draft's shape) so a busy-timeout vs an unresponsive device are **distinguished**, each with its own terminal + Err.
3. **Discriminator maps to a cause** (blind-hardware rule): `wpdma_timeout` = device alive but WPDMA never idled; `device_unresponsive` = read never completed (dead/detached); each localizes the next step instead of a blind 7-min silence.

## ⚠ Scope guard — do NOT perturb the frozen WiFi candidate
The active WiFi candidate `f4a0940` (`codex/mt7601u-post-qselect-terminal-jarvis`) is **frozen/immutable** (mid-gate + evidence trail). **Do not touch it.** Build this on a **fresh branch off current `origin/main`** as the diagnostic-hardening iteration. Coordinate the landing order with the WiFi lane: once the `arthur_reversibility` gate question resolves and `f4a0940` merges (or is superseded), this folds into the WiFi silicon-readiness path so the **next** boot has the instrumentation. If the PM decides the next silicon boot should carry it, it becomes part of that candidate — but never a mutation of the frozen `f4a0940`.

## Standing rules
- **I-14 / no new invariant / kill-switch:** behavior on the healthy path unchanged (a valid warm/cold device still reaches CONTROL_PLANE_READY); only the *bound + instrumentation* is new. No baked channel, no firmware-reload change, no MCU-death claim (the disproven ones). Confirm the bound never fires on a healthy device (valid reads complete in a handful of events, far under `50_000`).
- **Credential discipline unchanged:** this touches only the post-FW control plane; no credential surface.

## Close-gate + merge
`scripts/worker-close-gate.sh` (rebase-fresh), green 4-shard `workflow_dispatch` URL (a QEMU scenario exercises the bounded post-FW path + the `debug_post_firmware_breadcrumbs` replay), PM verify + AAG (I-14, the bound-never-fires-on-healthy proof, discriminator-maps-to-cause, no baked-channel/firmware-reload creep). Branch `codex/mt7601u-postfw-diagnostics-jarvis`. Disk-reclaim after merge.

## Relay — what to tell Jarvis
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-26-mt7601u-postfw-diagnostics-jarvis.md`. Design-reference (do NOT merge): `git show origin/wip/pm-draft-mt7601u-warm-postfw-bound:kernel/src/drivers/usb/mt7601u/mod.rs` — I'm deleting that branch; rebuild as your own. **Fresh branch off `origin/main`; do NOT touch the frozen `f4a0940`.**
- **One-line:** **Bound + instrument the mt7601u post-firmware control plane so the warm-path pre-sink wedge (the 7-min silence before MAC-init) fails fast with diagnostics instead of stalling.** Add `POST_FW_WAIT_BUDGET` (~50k, healthy reads are a handful of events) across WPDMA-idle/ASIC-ready/CSR-BBP/USB-DMA; a `POST_FW_STATE` discriminator (wpdma_idle/wpdma_timeout/device_unresponsive/asic_ready/control_plane_ready) + `POST_FW_WPDMA_LAST` + per-stage serial breadcrumbs + a `neo-test debug_post_firmware_breadcrumbs()` the bare-metal scenario replays to the sink; use a bounded `cold_poll`→Done/Unresponsive/Retry so busy-timeout vs unresponsive are distinguished, each localizing the cause. **Do NOT perturb the frozen `f4a0940`** — fresh branch, lands as the WiFi silicon-readiness iteration. Healthy-path behavior unchanged (bound never fires under `50_000`); no baked channel, no firmware-reload/MCU-death change. Close-gate + green 4-shard URL.
- **RAG** (PowerShell): health-check first; e.g. `mt7601u post firmware WPDMA ASIC ready xhci wait budget`, `cold_poll PollStep Done Unresponsive Retry`, `WaitBudgetGuard xhci`. If down, note it + grep/Read.
