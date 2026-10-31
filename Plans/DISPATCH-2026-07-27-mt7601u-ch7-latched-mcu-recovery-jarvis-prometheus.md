<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: mt7601u ch7 — latched MCU state + non-destructive recovery + retry terminalization → Jarvis (fix) + Prometheus (dig)

**Date:** 2026-07-27
**From:** PM (Claude)
**Status:** DISPATCHED. WiFi bare-metal iteration N+1. The `cea89e6a` retest (systemic ~500ms budget) FAILED at ch7 — but conclusively, and it **disproves "just widen the timeout."** This is normal blind-hardware progress: the last run revealed the next layer.

## This run's evidence (`cea89e6a`, OptiPlex, credential-local)
- ch1–6 `RF_FREQPLAN_RF17_20_RANDOM_WRITE` all completed OK at `budget=100000` → **the widen fixed the ch1–6 latency**, necessary but not sufficient.
- ch7: `set_channel(ch=7, Survey) begin; MCU-CMD before RF_FREQPLAN_RF17_20_RANDOM_WRITE` → wedge.
- `CHANNEL-PROBE enabled=1 reported_ok=true command_path_proven_usable=0 destructive_reload=0 com_reg0_write=0` → the endpoint probe says the **endpoint** is usable, but the **MCU command path is still latched**.
- `CHANNEL-PROBE retry ch=7 initial_phase=RF_FREQPLAN_RF17_20 reported_ok=true begin` → `set_channel(ch=7) begin` **again** → **HANG. No PASS, no FAIL, no terminal.**
- `fence_refusals=0` through the observed path (I-14(c) DMA containment held). Security cleanup verified: passphrase absent from serial/sink/strings; creds reverted to `b""`/`CHANNEL=0`; worktree clean. Artifacts: `target\baremetal-captures\mt7601u-2026-07-27\`.

## Refined diagnosis (two distinct bugs)
1. **Root: ch7 is a genuine MCU wedge, not latency.** A ~500ms budget still wedges at the 7th consecutive channel-switch command sequence, and the retry **re-issues the identical RF17-20 write against a latched MCU** and re-wedges. Since ch7 is structurally identical to ch1/ch4 (Prometheus, confirmed), the differentiator is **accumulated MCU state over 6 prior hops** — pointing to a **missing between-hop MCU drain/reset/recalibration** that upstream does implicitly, so our MCU latches by ch7. A retry of the same write cannot clear a latched command path; it needs a state-clearing sequence first.
2. **Our recovery is the wrong kind + doesn't terminalize.** Per Prometheus Q5 (`RESEARCH-2026-07-26-…FINDINGS.md`): our `recover_chip()` (mod.rs:1905-1962) does a **destructive cold FW reload** that (prior-art #2, FIX-FORWARD-2026-07-24) **stalls at ILM byte 0** when the MCU is wedged. Upstream instead uses `usb_reset_device` (endpoint/device reset, **no FW reload**, resident firmware stays live). And the retry path lost the bounded-terminal guarantee — it **hangs** instead of reaching a FAIL terminal.

## Prometheus — focused second dig (no build, no gate; you own the two open research gaps)
Read `git show origin/main:Plans/RESEARCH-2026-07-26-mt7601u-ch7-rf-freqplan-prior-art-FINDINGS.md` (your own) + `…:Plans/FIX-FORWARD-2026-07-24-mt7601u-channel7-recovery-warm-proof-jarvis.md`. Pull raw upstream source directly (git/curl, never WebFetch). Two questions, with file:line:
- **Q-A (prevention — between-hop MCU state):** what does upstream do to the MCU **between** channel hops that prevents accumulated wedge — a queue drain, an MCU response-slot reset, a `mt7601u_mcu_calibrate`/DPD/BW recal, a mailbox/seq-number reset? Is there a per-hop teardown we skip? This is the "why does upstream survive 14 hops but we wedge at 7" answer.
- **Q-B (non-destructive recovery):** the exact upstream `usb_reset_device` sequence + what MCU state it does/doesn't clear, and whether a **latched command path** (our `command_path_proven_usable=0` after endpoint reset) needs an MCU-specific reset (not just USB endpoint reset) to clear. What's the minimal non-destructive sequence that re-proves the command path without a cold FW reload?
Deliver an addendum THINK; hand to Jarvis.

## Jarvis — fix (own the driver change)
Three parts, in priority order:
1. **Prevent the wedge (best fix):** add the between-hop MCU drain/reset/recal upstream does (Prometheus Q-A) so the MCU never latches by ch7. If prevention holds, the survey reaches ch7→ch14 clean.
2. **Non-destructive recovery (fallback):** replace the cold-FW-reload `recover_chip()` on the channel-switch path with the lighter endpoint/MCU reset (Prometheus Q-B) that clears the latched command path with resident firmware — and **do NOT retry the identical write against a still-latched MCU**; re-prove `command_path_proven_usable` first.
3. **Terminalize the retry (robustness — mandatory regardless):** the retry-re-wedge MUST reach a bounded FAIL terminal (`CHANNEL_COMMAND_PATH_UNUSABLE` with the MCU/RF snapshot), never hang. Restore the bounded-terminal guarantee to cover the retry path — a hang is strictly worse than the clean FAIL `f4a0940` produced.
Keep the destructive cold reload rejected. Keep `fence_refusals=0` (I-14(c)).

## Prometheus's Q-A/Q-B answers + ⚠ VERIFY-FIRST gate (do NOT implement Q-A blind)
Prometheus (addendum `prometheus/mt7601u-ch7-qab-findings` @ `9972f447`) returned:
- **Q-A:** MCU **response-FIFO saturation** — upstream runs CAL_BW (2 `wait_resp=true` MCU cmds) *every* hop (`phy.c:424`); we skip CAL_BW in survey (`wedge_logic.rs:26`), so ~12 unprocessed slots accumulate by ch7 and the FIFO blocks. Proposed fix: run CAL_BW per-hop during survey.
- **Q-B:** minimal non-destructive recovery = endpoint recovery (already in CB-2) → **MCU queue drain** (N no-op `CMD_FUN_SET_OP` `wait_resp=true`, one per prior hop) → **command-path probe (verify `CMD_DONE`) before retrying `set_channel`** → NEVER cold FW reload. The `cea89e6a` hang = re-issuing RF17-20 against a still-latched MCU without draining/proving first.

**⚠ CONTRADICTION — resolve before implementing Q-A.** This Q-A **contradicts Prometheus's OWN first dig** (`RESEARCH-2026-07-26-…FINDINGS.md`: *"our driver matches upstream exactly — survey hops skip CAL_BW"*) **and the prior FIX-FORWARD** (*"CAL_BW defer matches upstream; deferring CAL_BW did not prevent this; the wedge is on RF_FREQPLAN not CAL_BW"*). Two prior sources say upstream **skips** CAL_BW in survey; the new dig says it **runs** it. The whole "run CAL_BW per-hop" fix hinges on which is true. **Jarvis (Codex) verifies the linchpin against RAW upstream source before coding:** does upstream mt7601u actually issue CAL_BW (the 2 `wait_resp` cmds) on **every scan/survey hop**, or skip it during scan? Read `phy.c` directly (git/curl, not WebFetch). **AND falsify the FIFO model against our own serial:** count the accumulated `wait_resp` MCU commands before the ch7 wedge — does it match a ~12-deep FIFO (and does starting survey later shift the wedge)? Confirm → implement CAL_BW-per-hop (or the no-op drain, whichever the source supports). Refute → the root cause is still open; do NOT ship a CAL_BW change on a wrong model.

**Implement REGARDLESS of the Q-A verdict (these stand on their own):** (a) never re-issue the identical RF17-20 write against a still-latched MCU — probe/prove the command path first (Q-B step 3); (b) keep the cold FW reload rejected (stalls at ILM 0); (c) terminalize the retry to a bounded FAIL, never hang. These fix the `cea89e6a` hang independent of the root-cause model.

## Discriminator bank update (blind-hardware — next run must be conclusive)
| Next-run observation | Meaning | Discriminator |
|---|---|---|
| Prevention added, ch7 survives → ch14 | root cause was accumulated MCU state | log the per-hop drain/recal fired |
| ch7 wedges but recovery re-proves path + continues | wedge real, non-destructive recovery works | `command_path_proven_usable` flips 0→1 after the light reset |
| ch7 wedges, recovery can't clear the latch | genuine hardware/firmware wedge | bounded FAIL terminal + MCU/RF snapshot (NOT a hang) |
| Wedge moves to ch8+ | prevention partial, threshold shifted | per-hop MCU state depth counter |

## Close-gate + merge
`scripts/worker-close-gate.sh`, green push-CI, 4-shard `workflow_dispatch` (kernel/driver). The device-less QEMU fixture cannot reproduce the MCU wedge — the **real proof is the next JOINT OptiPlex LINK-UP** (survey reaches ch7→ch14, WPA/GTK, DORA, LINK UP, `fence_refusals=0`). Branch: `codex/mt7601u-post-qselect-terminal-jarvis` (continue). Disk-reclaim after merge.

## Relays
**Jarvis** — `git show origin/main:Plans/DISPATCH-2026-07-27-mt7601u-ch7-latched-mcu-recovery-jarvis-prometheus.md`. **One-line:** the ~500ms widen fixed ch1–6 but ch7 is a **genuine MCU wedge, not latency** — the retry re-issues the identical RF17-20 write against a **latched** MCU (`command_path_proven_usable=0` after endpoint probe) and then **hangs, no terminal**. Fix in priority order: (1) add the between-hop MCU drain/recal upstream does so it never latches by ch7 (Prometheus is digging the exact sequence — Q-A), (2) replace the destructive cold-FW-reload recovery with a light endpoint/MCU reset that clears the latch + re-prove the path before any retry (Q-B), (3) **mandatory:** terminalize the retry to a bounded FAIL, never hang. Keep destructive reload rejected, `fence_refusals=0`. Next JOINT LINK-UP is the real proof. RAG: `mt7601u recover_chip usb_reset_device mcu latch channel switch`, `mt7601u mcu calibrate between hop drain recal`.
**Prometheus** — same dispatch path. **One-line:** focused second dig, no build: **Q-A** what upstream does to the MCU **between** channel hops that stops accumulated wedge (drain/recal/reset — why it survives 14 hops, we wedge at 7); **Q-B** the exact non-destructive `usb_reset_device`/MCU-reset sequence that clears a **latched command path** without a cold FW reload. Raw upstream source only (no WebFetch). File:line; addendum THINK to Jarvis. RAG: `mt7601u_mcu_calibrate between channel hop`, `usb_reset_device mcu state clear latch`.
