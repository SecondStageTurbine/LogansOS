<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (RESEARCH/DESIGN THINK): Phase-7.5 closeout — the ch6+ channel-switch wedge + the iwlwifi-vs-MT7601U scope call → Cerebro

**Date:** 2026-06-23
**From:** PM (Claude)
**To:** Cerebro (owns the W1/W2 MT7601U lane — the deepest context on this chip). **Research/design only — produce a THINK, NO code, NO QEMU.** New branch off `origin/main` HEAD: `codex/phase-7.5-closeout-ch6-wedge-cerebro`.
**Status:** DISPATCHED. One of **four parallel closeout-research lanes** (Cerebro/Prometheus/Oracle/Janus); the PM synthesizes the four THINKs into a single Phase-7.5-closeout PLAN + ordered dispatch sequence. **These are host-only design THINKs — they must not open a QEMU gate** (Stage-B owns the runners).

## Why this lane
The PL wants the *combined brain power* of the four free coders aimed at **how to fully close Phase 7.5** before any closeout build is dispatched. You own the two hardest, most chip-specific questions:

### Charter A — the ch6+ channel-switch wedge (OT-27(d)), the gate-blocking knot
`set_channel(6)` MCU-times-out under sustained channel-switch command load (`mcu response timed out` / `inband bulk-out timeout`); the forced-reload recovery re-upload then fails on the **bulk** pipe — *distinct* from the cold-upload **control** pipe you already fixed in BUILD-2 (`f378f12`), which the W2 build classifies cleanly as `EndpointStall unresponsive=false`. This wedge gates **full cold end-to-end association** (ch7–14 skipped today; 10 BSSIDs on ch1–6 is past the scan gate but short of the link). Deliver:
- **A root-cause hypothesis bank** for the bulk-pipe wedge — ranked, each with the *discriminator* (the serial breadcrumb / register read / state capture) that would confirm-or-refute it on the next bare-metal run. Apply the blind-hardware standing rule: design the next run to be **conclusive on pass AND fail**, and name any capture that must be added *before* the run (capture-before-you-need-it).
- The **bulk-pipe-reset / endpoint-recovery approach** (what the recovery sequence should do that it doesn't today) — but treat the deep prior-art on the exact reset/MCU-response-buffer pattern as **Prometheus's trap-mine lane** (Charter, below); cross-reference, don't duplicate. Where you need a specific Linux `mt7601u` byte sequence, name it as a dependency on Prometheus's findings.

### Charter B — the scope call: does MT7601U *close the WiFi phase*, or is iwlwifi in-scope? (PL-deferred to this research)
The bible (Art. 13) opens 7.5 as "Intel **iwlwifi** family"; the lane deliberately executed the **MT7601U** USB SoftMAC dongle (ADR-054), silicon-proven on the OptiPlex 7080. **The PL deferred the scope fork to this analysis** — your THINK must give the PL a *decision-grade* comparison:
- **What MT7601U already satisfies** vs the bible's WiFi acceptance intent (client mode, WPA2/WPA3, regulatory, kernel connection-management surface) — what's genuinely *done* vs what the open tail (b/c/d) still owes.
- **The honest cost of an iwlwifi driver** if it were in-scope: a *from-scratch* Intel WiFi driver is a FullMAC-vs-SoftMAC question — does iwlwifi's firmware run the MLME/supplicant for us (deleting the from-scratch SoftMAC work), or not? Estimate the build size against what MT7601U cost. Mine the easier-path question (the standing trap-mining rule): is there a chip/firmware class that does the hard part for us?
- **A recommendation** with the residual risk of each path, so the PL can rule "MT7601U closes it / iwlwifi required / hybrid." You recommend; the PL decides.

## References
```
git fetch origin
git show origin/main:docs/ADR-054-phase-7.5-mt7601u-driver.md            # W1 scan + the F1-part-2 ch6 wedge characterization (the bulk vs control pipe distinction)
git show origin/main:docs/ADR-057-kernel-csprng.md                       # W2 supplicant context (if needed)
git show origin/main:Plans/DISPATCH-2026-06-16-phase-7.5-build-W2-mlme-ccmp-cerebro.md   # §5 failure-mode note "F1-part-2 still wedges past ch6 → bulk-poll-budget + MCU-response-buffer"
git show origin/main:Plans/THINK-2026-06-20-w2-mt7601u-discriminator-bank-and-fault-containment.md   # the W2 discriminator bank (P2/P6 endpoint/MCU-boot)
git show origin/main:Plans/RESEARCH-2026-06-14-mt7601u-bringup-trap-mine-FINDINGS.md      # the 27-trap bank + the file:line bring-up sequence (Channel switching @ §)
git show origin/main:docs/OPEN-THREADS.md                                # OT-27(b)(c)(d) — the WiFi follow-on tail
git show origin/main:docs/bible.md                                       # Article 13 Phase 7.5 acceptance intent (the "iwlwifi family" wording)
# kernel/src/drivers/net/wifi/mt7601u/   — the live driver (set_channel, the bulk/control transfer paths, the recovery re-upload)
```

## Deliverable
A THINK on your branch: `Plans/THINK-2026-06-23-phase-7.5-closeout-ch6-wedge-and-scope-cerebro.md` (SPDX `CC-BY-4.0`). Charter A (the ranked hypothesis+discriminator bank for the bulk-pipe wedge + the recovery approach) and Charter B (the MT7601U-vs-iwlwifi decision-grade comparison + recommendation). RETURN the file path; the PM reads it on the branch (or you push the branch and the PM lands it to `main` after review).

## Scope guard
**Research/design only.** Do NOT write driver code, do NOT run QEMU, do NOT open a CI gate. No kernel change. If Charter A surfaces that the fix needs a capture the driver doesn't yet log, *name it as a build requirement* in the THINK (don't add it here). Cross-reference Prometheus's trap-mine for the exact Linux byte sequences rather than re-mining them.

---
## Relay — what to tell Cerebro
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-23-phase-7.5-closeout-ch6-wedge-cerebro.md` (+ the references above). **Branch:** `codex/phase-7.5-closeout-ch6-wedge-cerebro` off `origin/main`.
- **One-line:** **Research THINK, no code, no QEMU** — one of four parallel Phase-7.5-closeout brains. **(A)** Produce a ranked root-cause hypothesis bank + per-hypothesis discriminators for the **ch6+ channel-switch / bulk-pipe-reset wedge (OT-27(d))** that gates full cold association — conclusive-on-pass-AND-fail, naming any capture to add before the next bare-metal run; sketch the bulk-pipe/endpoint-recovery approach (defer the exact Linux byte sequences to Prometheus's trap-mine, cross-reference). **(B)** Give the PL a decision-grade **MT7601U-vs-iwlwifi scope call**: what MT7601U already satisfies vs the bible's WiFi intent, the honest cost of a from-scratch iwlwifi driver (FullMAC firmware-runs-MLME vs SoftMAC — easier-path question), and a recommendation with residual risk. **Deliver `Plans/THINK-2026-06-23-phase-7.5-closeout-ch6-wedge-and-scope-cerebro.md`; RETURN the path.** Host-only — do not open a QEMU gate (Stage-B owns the runners).
