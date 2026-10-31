<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: mt7601u ch7 RF_FREQPLAN MCU wedge — diagnose + fix the channel-switch RF-program path → Jarvis

**Date:** 2026-07-27
**From:** PM (Claude)
**To:** Jarvis (Codex family — built `f4a0940`, owns the mt7601u driver + the post-fw diagnostics lane `d7d5a83c`)
**Status:** DISPATCHED. The `f4a0940` bare-metal silicon run **FAILED conclusively** (not a hang) — the discriminator bank localized it precisely. This is a real driver gap in the RF frequency-plan programming across a channel switch.

## The evidence (already captured — you have it)
- **Serial:** `C:/Users/<user>/neoplutarch-serial.wifi-f4a0940-20260726.log`
- **Sink:** `E:/AI Automation/AltimitNeo-run-wifi-f4a0940/target/neo-test/mt7601u_assoc_baremetal.optiplex-wifi-20260726.nvme-sink.raw`
- **Signature:** scan/survey progressed **ch1–6, found bss=11** (RF/antenna/FW-upload/survey all good), then **ch7 wedged at `RF_FREQPLAN_RF17_20_RANDOM_WRITE`** → `mcu response timed out` → terminal `CHANNEL_COMMAND_PATH_UNUSABLE`, `association=false dhcp=false link_up=false`, `fence_refusals_total=0`. Final: `NEO_TEST event=FAIL name=mt7601u_assoc_baremetal reason=mt7601u_bind_incomplete_see_stage_breadcrumb`.

## The tell — this is channel-switch-specific, not a generic RF-write bug
ch1–6 programmed fine; the wedge is at the ch7 switch. Candidates, in priority order:
1. **Accumulated MCU state / missing inter-hop recalibration.** The MCU may need a calibration or state-reset step between channel hops that the driver skips, so state degrades until the Nth switch wedges. Directly related to the **deferred CAL_BW / `MCU_CAL_BW`-in-`set_bw_filter` wedge** already in the discriminator bank (NOTE-A class).
2. **Channel-group boundary at ch7.** MT7601U RF freqplan tables are grouped; ch7 may cross into a group whose RF17–20 write sequence or synth parameters differ from ch1–6. A wrong/duplicated write for the new group can hang the MCU.
3. **MCU handshake/timeout too tight on the RF17–20 random-write burst.** The "random write" burst to RF17–20 may need a per-write MCU-ready poll (or a longer settle) that the driver doesn't wait for at the channel-switch path (vs the initial-tune path that worked).

## OUR OWN prior art — this is a KNOWN recurring wedge (read FIRST)
This is **not a new failure.** The prior PM hit the *identical* ch7 `RF_FREQPLAN` MCU timeout on 2026-07-24 (scan ch1–6 fine, found 8 BSSIDs that time / 11 tonight, then ch7 wedge). Read it before re-deriving:
```
git show origin/main:Plans/FIX-FORWARD-2026-07-24-mt7601u-channel7-recovery-warm-proof-jarvis.md
git show origin/main:Plans/DISPATCH-2026-07-23-mt7601u-warm-postfw-fix-jarvis.md
```
Its established boundaries (treat as hypotheses to prove/reject, not gospel):
1. **The wedge is in ch7 `RF_FREQPLAN`, NOT deferred `CAL_BW`** — deferring `CAL_BW` did **not** prevent it (already tried). So don't re-chase CAL_BW; the RF freqplan register program on the channel switch is the locus.
2. **The recovery sequence is DESTRUCTIVE (prime suspect):** after the MCU timeout the driver clears host/device endpoint state and **forces a cold FW reload — which then stalls at ILM byte 0** (inband bulk-out timeout), so recovery *destroys* forward progress. **Progress since:** tonight `f4a0940` reached a *bounded* terminal (`CHANNEL_COMMAND_PATH_UNUSABLE`) instead of the old ILM-0 blind stall — so the terminal is now honest, but the underlying ch7 wedge is unfixed. The open question: bound/skip the wedge to a truthful WEDGED result **and** get past it, not just terminate cleanly.
3. Sink-install-after-USB-init timing (tonight's OptiPlex sink captured, so likely improved — confirm).

## Prior-art trap-mining pass (driver-dispatch standing rule — do this BEFORE coding)
Mine the **Linux `mt7601u`** driver for the exact channel-switch / RF-programming sequence — read for *principle*, never copy GPL code into our MPL kernel:
- How does `mt7601u` sequence RF_CSR / RF17–20 writes on a channel change vs initial tune? Is there an MCU calibration (`mt7601u_mcu_calibrate` / `MCU_CAL_*`) or a `vco`/`rxdcoc`/`bw` recal fired **between** hops?
- Is there a per-write MCU-ready poll or a mandated settle delay on the RF register burst?
- Are channels grouped (a per-group freqplan table), and does the group change at/near ch7?
- Pull the raw driver source directly (`git`/`curl` the upstream tree), not via a summarizing fetch (byte-oracle acquisition rule).
Seed each trap you find into the failure-mode bank below with its discriminator **before** the next bare-metal run.

## Governance fault-containment analysis (I-14 — mandatory for connectivity dispatches)
- **(a) Lock isolation:** the RF-program/MCU-timeout path acquires **no** AI-governance lock (`INFLIGHT`/`RECENCY`/`ARENA`/reservation/`TABLE`). Assert it stays so — an MCU timeout must not stall governance.
- **(b) Liveness degradation:** an MCU wedge must **fail the bind** (as it did — `CHANNEL_COMMAND_PATH_UNUSABLE`), never spin a CPU holding a governance-relevant lock; the residency daemon's P4b backstop is unaffected. Confirm the timeout path unwinds cleanly (it terminated the scenario — good; keep it bounded).
- **(c) Memory isolation:** `fence_refusals_total=0` on the failed run shows I-14(c) DMA containment held through the failure. Any new DMA buffer added for a recal step routes through `dma_arena::fence(...)`.

## Blind-hardware failure-mode / discriminator bank (the NEXT run must be conclusive pass OR fail)
| Expected next failure | Discriminator to add now |
|---|---|
| Recal added, ch7 still wedges | log the MCU state/heartbeat + the exact RF17–20 write index that times out (which register in the burst) |
| Wedge moves to a later channel (ch8+) | confirms accumulated-state hypothesis — log a per-hop MCU-ready + recal-fired breadcrumb |
| ch7 programs but assoc still fails | the wedge was masking the assoc path — the NOTE-A CAL-retry chain takes over |
| MCU-ready poll never satisfied | distinguish "MCU dead" vs "poll condition wrong" — log the MCU status register value, not just timeout |
Capture-before-you-need-it: if the fix needs the failing RF-write **index** or the **MCU status register**, add that logging in THIS dispatch — don't discover the gap on the next failing boot.

## Close-gate + merge
`scripts/worker-close-gate.sh` (rebase-fresh on `origin/main`), green push-CI, and — since this is kernel/driver-touching — the **4-shard `workflow_dispatch` QEMU gate** on the exact SHA (the device-less mt7601u fixture scenarios must stay green). The **real proof is the next bare-metal silicon run** (JOINT, Jarvis + PL, OptiPlex) reaching LINK-UP per `Plans/RUNSHEET-2026-07-26-wifi-f4a0940-silicon-linkup.md` — but land the diagnosed fix through the gate first. Branch: `codex/mt7601u-ch7-rf-freqplan-fix-jarvis`. Disk-reclaim after PM confirms merge.

## Relay — what to tell Jarvis
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-27-mt7601u-ch7-rf-freqplan-mcu-wedge-jarvis.md`. You already hold the serial + sink from tonight's run.
- **One-line:** Your `f4a0940` silicon run FAILED conclusively at **ch7 `RF_FREQPLAN_RF17_20_RANDOM_WRITE` → MCU timeout → `CHANNEL_COMMAND_PATH_UNUSABLE`** (scan found 11 APs ch1–6 first, so RF/FW/survey are good; containment held, `fence_refusals=0`; security clean). It's channel-switch-specific — accumulated MCU state / missing inter-hop recal / ch7 channel-group boundary / too-tight MCU-ready poll on the RF17–20 burst. **Trap-mine the Linux mt7601u channel-switch RF-program + MCU-calibrate sequence first** (read for principle, don't copy GPL), seed the traps into the bank, add the MCU-status + failing-RF-index logging (capture-before-you-need-it), then fix. I-14 containment analysis in the dispatch. Close-gate + push-CI + 4-shard QEMU on the exact SHA; the real proof is the next JOINT bare-metal LINK-UP.
- **RAG** (PowerShell) health-check first; e.g. `mt7601u set_channel RF_FREQPLAN RF17 random write mcu`, `mt7601u mcu calibrate CAL_BW set_bw_filter channel switch`, `mt7601u rf_csr program group table`. If down, note it + grep/Read.
