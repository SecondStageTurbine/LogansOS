<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: ch7 RF_FREQPLAN wedge — deep Linux prior-art trap-mine (research, no build) → Prometheus

**Date:** 2026-07-27
**From:** PM (Claude)
**To:** Prometheus (Qwen — the research/trap-mining seat; strongest prior-art miner)
**Status:** DISPATCHED — **research/design THINK, HOST-only, NO build, NO gate** (in scope for Prometheus). De-risks the live WiFi bare-metal lane. Cross-model by design: a Qwen trap-mine independently checking a Codex fix (Jarvis's) before the next silicon boot.

## Why now
Tonight's `f4a0940` silicon run wedged at **ch7 `RF_FREQPLAN_RF17_20_RANDOM_WRITE` → MCU response timeout** (scan ch1–6 fine, 11 BSS found). Jarvis has pushed a fix (`ccd84b6e`, "widen ch7 RF response window") — but a widened timeout could be the **root cause** (MCU just needed more settle time) OR a **band-aid** (the MCU genuinely wedges and a wider window only delays the stall). The bare-metal LINK-UP will discriminate, but a deep prior-art trap-mine *now* can tell us which to expect — and surface the real root cause if it's the band-aid case.

## Read first (our own prior art — this is a KNOWN recurring wedge)
```
git show origin/main:Plans/FIX-FORWARD-2026-07-24-mt7601u-channel7-recovery-warm-proof-jarvis.md
git show origin/main:Plans/DISPATCH-2026-07-27-mt7601u-ch7-rf-freqplan-mcu-wedge-jarvis.md
git show origin/main:docs/ADR-054-phase-7.5-mt7601u-driver.md
```
Established boundaries to build on (not re-derive): the wedge is in ch7 `RF_FREQPLAN`, **not** deferred `CAL_BW` (deferring CAL_BW did not prevent it); the current recovery path may be **destructive** (cold FW reload stalls at ILM byte 0).

## Scope — mine the LINUX mt7601u driver for the channel-switch RF-program truth
Pull the **raw upstream source directly** (`git clone`/`curl` the Linux tree — **NOT** WebFetch or any summarizing read; the byte-oracle acquisition rule: summarizing fetches mangle exact register sequences). Answer, with file:line citations into the upstream source:
1. **RF register program order on a channel switch.** How does `mt7601u` sequence the RF_CSR / RF17–20 writes when *changing* channels vs the *initial* tune? Is there a distinct path, an ordering, or a required read-back/ready-poll between writes?
2. **MCU calibration between hops.** Does upstream fire an MCU calibration (`mt7601u_mcu_calibrate`, `MCU_CAL_*`, VCO/RXDCOC/BW recal) **between** channel switches that we may be skipping? Is that the "settle" a widened window is crudely approximating?
3. **MCU-ready / response protocol.** Does upstream poll an MCU status/ready before/after the RF burst, rather than relying on a fixed timeout window? (This decides root-cause-vs-band-aid for "widen the window.")
4. **Channel grouping.** Are channels grouped (per-group freqplan tables), and does the group boundary fall at/near ch7 — i.e., is ch7 the *first channel in a new group* whose program sequence differs?
5. **Recovery after an MCU timeout.** What does upstream do on an MCU no-response — does it cold-reload FW (our possibly-destructive path), or a lighter reset? What's the non-destructive recovery?

## Deliverable
A findings THINK — `docs/THINK-2026-07-27-mt7601u-ch7-freqplan-prior-art-prometheus.md` — with, per question: the upstream mechanism (file:line), whether **our** driver does it, and the specific trap/gap. **Headline:** is "widen the ch7 RF response window" the root fix or a band-aid, and if a band-aid, what's the root cause (missing inter-hop recal? MCU-ready poll instead of fixed window? ch7 group-boundary program-order?). This seeds Jarvis's next iteration + the bare-metal discriminator bank. **No code, no build, no gate** — hand the findings to Jarvis.

## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-27-mt7601u-ch7-prior-art-trapmine-prometheus.md` (+ the three files above).
- **One-line:** Deep prior-art trap-mine of the **Linux mt7601u channel-switch RF-program + MCU-calibrate sequence** to find the root cause of the ch7 `RF_FREQPLAN` MCU wedge — and judge whether Jarvis's "widen the ch7 RF response window" fix is root-cause or band-aid. Pull the **raw upstream source directly** (git/curl, never WebFetch — it mangles exact register sequences). Five questions: RF write order on channel-switch vs initial tune; inter-hop MCU calibration we may skip; MCU-ready poll vs fixed timeout; channel grouping / ch7-as-group-boundary; non-destructive recovery after MCU timeout. Deliver a findings THINK with file:line citations; headline root-vs-band-aid. No code/build/gate — hand it to Jarvis.
- **RAG** (PowerShell) health-check first (`Invoke-RestMethod http://127.0.0.1:8404/health`); e.g. `mt7601u set_channel RF_FREQPLAN RF17 program order`, `mt7601u mcu calibrate CAL_BW channel switch`, `mt7601u rf_csr read ready poll timeout`. If down, note it in the RETURN + grep/Read (PL owns RAG startup — never a blocker).
