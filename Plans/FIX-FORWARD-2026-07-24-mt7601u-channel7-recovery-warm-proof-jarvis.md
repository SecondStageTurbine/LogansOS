<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# FIX-FORWARD: MT7601U channel-7 wedge, terminal evidence, and warm-proof boundary -> Jarvis

**Date:** 2026-07-24
**From:** PM
**To:** Jarvis
**Candidate under test:** `2d43670efd354df4f5d28cc62c43bcb2cb4fc234`
**Frozen base:** `5c990a9b2e934b79b32cc350c4da4cc4344d5ad3`
**Rejected TEST 3 ISO SHA-256:** `eab064bcf5a372e60990cd30c20715fd788486f4015e17652dd22bc41bd96973`
**Verdict:** **HOLD - TEST 3 is RED. No third boot, no merge, and no runner token.**

## Read first

```powershell
git fetch origin
git show origin/main:Plans/FIX-FORWARD-2026-07-24-mt7601u-channel7-recovery-warm-proof-jarvis.md
git show origin/main:Plans/AUTHORIZATION-2026-07-24-mt7601u-test3-cold-warm-jarvis.md
git show origin/main:Plans/DISPATCH-2026-07-23-mt7601u-warm-postfw-fix-jarvis.md
git show origin/main:docs/runbook-wlan0-dora-optiplex.md
```

The PM evidence and hypotheses below are untrusted diagnostic inputs only. Independently trap-mine the driver, upstream Linux behavior, xHCI semantics, and boot procedure; derive and own the repair. Do not implement a PM-authored patch mechanically.

## Independent TEST 3 verdict

The authorized two-boot sequence stopped at its boundary. There is no authorization for an identical rerun or a third boot.

### Cold boot - RED

The dongle enumerated cleanly at high speed as `148f:7601`. Cold firmware upload and post-firmware setup completed. Survey channels 1 through 6 completed and found eight BSSIDs.

Channel 7 then failed during `RF_FREQPLAN` with an MCU response timeout. Host endpoint recovery and device `ClearFeature(ENDPOINT_HALT)` both reported success, but the forced firmware reload failed at ILM byte `0/45380` with an inband bulk-out timeout. The scan never reached a truthful terminal discriminator, association, DHCP, `LINK UP`, or PASS.

This run did not reach NOTE-A. NOTE-A remains reserved for the existing condition: the target AP is found but auth/association fails before deferred post-association calibration.

### Immediate second boot - RED / invalid warm proof

A second NeoPlutarch boot occurred unintentionally during the return-to-Ubuntu transition without fresh capture and sink isolation. It performed a cold `45412`-byte firmware upload. It emitted neither `WARM-RESYNC` nor the resident-firmware/upload-skipped path.

The capture is not an acceptance boot, but the cold upload conclusively disproves firmware residency across that transition. Do not count it as the authorized warm boot and do not repeat the same transition as proof.

## Preserved evidence

Evidence root:

`Workers/jarvis/target/neo-test/baremetal-evidence/mt7601u-test3-2d43670-20260724`

- `cold-putty.raw.log`
  - SHA-256: `1413a6d32b92d8c7ae1276cd1c144c257cf884bb44257806058f38ff837e329d`
- `cold-sink.raw`
  - SHA-256: `080acf35a507ac9849cfcba47dc2ad83e01b75663a516279c8b9d243b719643e`
  - size: 16 MiB
  - content: all zero; `strings` output empty

The sink and serial therefore disagree. SSID grep count was zero; passphrase grep count was zero. `fence_refusals` was not emitted and must not be reported as zero. `/dev/sda` was untouched. The coder worktree was clean at the tested candidate; no merge occurred.

## Failure boundaries to trap-mine

Treat these as hypotheses to prove or reject:

1. The channel-switch wedge occurs in channel 7 `RF_FREQPLAN`, not in deferred `CAL_BW`. Deferring `CAL_BW` did not prevent this failure.
2. After the MCU timeout, the current recovery sequence clears host/device endpoint state and then forces a cold reload. On this silicon the reload stalls before its first ILM byte, so the recovery attempt destroys forward progress and prevents the intended bounded WEDGED/skip or terminal result.
3. The NVMe evidence sink is installed after USB initialization. The MT7601U bind-time scan fails before sink installation and never returns to the later replay point, leaving a zero sink despite substantial serial evidence.
4. Firmware residency was not preserved by the observed return/reboot path. This is a separate platform/procedure boundary from the cold-scan driver failure.

## Required repair properties

### A. Avoid or bound the channel-switch wedge

- Independently compare the channel-programming order, MCU command sequencing, calibration state, dwell transitions, and recovery rules with the applicable upstream Linux MT7601U path.
- Preserve `CHANNEL=0` survey semantics. No baked channel, target-channel constant, target-bearing-channel omission, or runtime shortcut may become shipping proof.
- The configured AP must remain discoverable by a genuine survey.
- If the MCU dies during a channel transition, do not blindly force a reload that is already known to fail at ILM byte zero and strand the scan.
- Either prove a safe recovery path on the modeled state machine or produce a bounded, truthful terminal/skip disposition without attempting continued use of a dead MCU.
- Preserve operation-wide bounds, endpoint-fence invariants, and the existing fail-hard DHCP/LINK-UP success contract.

### B. Guarantee terminal evidence

- A bind-time scan failure must reach both serial and the raw evidence sink, including the final discriminator and operation-budget/fence summary.
- The current post-USB sink installation plus later replay is insufficient when bind never returns.
- Independently choose the smallest structurally safe correction: earlier bare-metal sink availability, a guaranteed pre-sink terminal/replay mechanism, or another demonstrated design.
- Preserve non-MT7601U scenarios and all structural kill switches. With `mt7601u` disabled, the required normalized OFF artifacts must remain byte-identical to their accepted baseline.
- A zero sink paired with a nonempty serial log is RED.

### C. Add hostile host oracles

Add deterministic coverage for at least this trace:

1. channels 1 through 6 complete;
2. channel 7 `RF_FREQPLAN` times out;
3. host recovery and device clear-halt report success;
4. attempted reload bulk-out fails at byte zero.

The oracle must prove:

- bounded completion;
- exactly one truthful terminal category;
- no association, DHCP, `LINK UP`, or PASS after MCU death;
- no continued command traffic against a device classified dead;
- the terminal discriminator and budget/fence accounting remain available to the evidence path.

Include a negative control that demonstrates the rejected behavior loses terminal/sink evidence or attempts the destructive reload. A happy-path scan alone is insufficient.

### D. Requalify the warm procedure separately

Before requesting another silicon token, identify how the return-to-Ubuntu transition selected the NeoPlutarch ISO again and whether the platform or boot path reset/depowered the USB device.

Return a step-by-step cold-to-immediate-warm procedure that:

- keeps the dongle physically powered and in the same port;
- prevents an accidental second ISO boot before Boot 1 evidence is archived;
- provides fresh PuTTY capture and a freshly cleared sink for each authorized boot;
- one-shot boots the exact ISO only after isolation is ready;
- can distinguish platform USB power loss from driver failure.

If this platform cannot preserve dongle firmware across the required transition, report that constraint for PM disposition. Do not weaken the resident-firmware requirement and do not relabel a cold upload as warm.

## Frozen constraints

- Source credentials remain `SSID=b""` and `PASSPHRASE=b""`; `CHANNEL=0`.
- Real credentials may be filled only in the local unstaged build, never added, committed, pushed, or stashed. Restore and verify blank placeholders immediately after the ISO build.
- No baked-channel fallback.
- NOTE-A remains the CAL-retry discriminator only when scan finds the AP and association fails.
- No weakening of terminal PASS, DHCP truth, LINK-UP truth, operation budgets, AAG bounds, or fence accounting.
- `/dev/sda` remains out of scope.
- No merge and no hardware boot.

## Build, close gate, and RETURN

1. Rebase the coder branch onto current `origin/main` containing this dispatch.
2. Return a trap-mine table mapping each failure boundary to upstream evidence, local code, proposed invariant, and hostile oracle.
3. Implement the smallest independently derived repair on the coder branch.
4. Run focused host/KAT oracles, formatting, host tests, workspace clippy, blocking kernel clippy, SPDX/identity, all normalized OFF artifact comparisons, and `scripts/worker-close-gate.sh`.
5. Commit and push. Return the exact SHA, parent, frozen-base relation, own-stat, test outputs, OFF-artifact hash table, clean-worktree proof, and exact push-CI URL.
6. Do not fire the four-shard `workflow_dispatch` without a new PM/PL exact-SHA runner token.
7. After a four-shard green and independent PM verification, recut a credential-local ISO from that exact SHA and return its size, SHA-256, manifest, and placeholder-restoration proof.
8. A new silicon authorization, if any, will name the exact ISO and a corrected cold-to-immediate-warm procedure. Until then, do not boot it.

Green without an exact CI URL is not done. A gate green is not merge authorization. No third boot and no merge.
