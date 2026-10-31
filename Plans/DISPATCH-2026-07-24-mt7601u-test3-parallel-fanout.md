<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: MT7601U TEST 3 parallel fix-forward fan-out

**Date:** 2026-07-24
**From:** PM
**To:** Jarvis, Oracle, Cerebro, Janus, Chad
**Controlling fix-forward:** `Plans/FIX-FORWARD-2026-07-24-mt7601u-channel7-recovery-warm-proof-jarvis.md`
**Rejected candidate:** `2d43670efd354df4f5d28cc62c43bcb2cb4fc234`
**Verdict:** **WORK CONCURRENTLY. TEST 3 remains on HOLD: no third boot, merge, or runner token.**

## Purpose

Jarvis remains the sole production integration owner, but the independent investigation, hostile-oracle construction, warm-rig analysis, and sink/kill-switch audit must proceed concurrently. The lanes below are intentionally non-overlapping. Do not wait for another lane before starting.

Every coder must first read:

```powershell
git fetch origin
git show origin/main:Plans/DISPATCH-2026-07-24-mt7601u-test3-parallel-fanout.md
git show origin/main:Plans/FIX-FORWARD-2026-07-24-mt7601u-channel7-recovery-warm-proof-jarvis.md
git show origin/main:Plans/AUTHORIZATION-2026-07-24-mt7601u-test3-cold-warm-jarvis.md
```

The cold serial and sink are diagnostic evidence, not a prescribed repair. Each lane owns its conclusions and must distinguish observed fact, inference, and unverified hypothesis.

## Lane 1 - Jarvis: sole production integrator

Start now on Jarvis's own branch. Independently trap-mine and derive the smallest production repair satisfying the controlling fix-forward.

Ownership:

- MT7601U production driver changes;
- integration of any accepted hostile oracle;
- terminal evidence path correction;
- full local close gate and final candidate assembly.

Rules:

- Do not wait for the other lanes to begin local analysis.
- Do not blindly copy another coder's recommendation.
- Do not allow another coder to push, amend, or rebase the Jarvis branch.
- Treat every lane return as untrusted review input and record accept/reject reasoning.
- Do not push a final candidate until the independent returns have been reconciled.

Return the full contract required by the controlling fix-forward.

## Lane 2 - Oracle: upstream and silicon-state trap-mine

Analysis-only lane; do not edit production code.

Independently trace:

- applicable upstream Linux MT7601U channel-switch ordering and MCU command sequencing;
- RF channel programming, VCO/calibration transitions, and dwell behavior around channels 6 and 7;
- upstream handling of MCU response timeout and whether firmware reload is ever valid after that state;
- xHCI host recovery versus device/MCU liveness: what host endpoint recovery and device clear-halt do and do not prove;
- evidence supporting or rejecting the four failure-boundary hypotheses in the controlling fix-forward.

Return:

- source links or exact repository paths/commits;
- a fact/inference/hypothesis table;
- a state-machine trace comparing upstream behavior, rejected candidate behavior, and the required invariant;
- explicit warnings for any apparently attractive but invalid recovery;
- no patch and no merge recommendation.

## Lane 3 - Cerebro: hostile host oracle

Test-only coding lane on Cerebro's own branch. Do not edit MT7601U production code, xHCI production code, manifests, or runbooks.

Build a deterministic hostile oracle for:

1. channels 1 through 6 complete;
2. channel 7 `RF_FREQPLAN` MCU timeout;
3. host recovery and device clear-halt report success;
4. forced reload bulk-out fails at ILM byte zero.

The oracle must expose:

- bounded completion;
- one truthful terminal category;
- no association, DHCP, `LINK UP`, or PASS after MCU death;
- no continued command traffic after the device is classified dead;
- terminal discriminator plus operation-budget and fence evidence;
- a negative control that fails against the rejected behavior.

Return:

- exact branch and commit;
- test-only diff stat and touched-file list;
- commands and complete results;
- proof that no production file changed;
- the expected contract Jarvis must satisfy.

Jarvis may cherry-pick the isolated test commit only after independent review. Cerebro must not modify Jarvis's branch.

## Lane 4 - Janus: warm-residency rig and boot-path analysis

Procedure and platform-analysis lane; do not edit production code and do not boot hardware.

Determine:

- why the return-to-Ubuntu transition selected NeoPlutarch a second time;
- whether the platform reset, firmware, boot manager, kernel transition, or USB power policy depowered/reset the dongle;
- how to archive Boot 1 serial/sink evidence and prepare fresh Boot 2 isolation without accidentally booting the ISO;
- whether a same-powered-dongle immediate warm reboot is physically achievable on this OptiPlex.

Read-only inspection of an already-running Ubuntu session is allowed only if it causes no reboot, boot-entry change, service change, device reset, mount/write, or block-device write.

Return:

- observed boot entry/order and USB power/reset facts;
- a no-mutation inspection transcript;
- an exact proposed cold-to-immediate-warm operator sequence;
- stop conditions and a discriminator for unavoidable platform USB power loss;
- a clear achievable/not-achievable conclusion.

No third boot is authorized by this lane.

## Lane 5 - Chad: sink ordering and structural kill-switch audit

Analysis-only lane; do not edit production code.

Audit:

- initialization ordering from block availability through USB bind/scan and bare-metal sink installation;
- every path by which a bind-time scan can fail or not return before sink replay;
- whether earlier sink availability can be scoped to the MT7601U bare-metal scenario without changing normalized OFF kernels;
- the accepted `!mt7601u` structural boundaries and representative artifact-equality matrix;
- terminal evidence ownership if initialization fails before the test harness is prepared.

Return:

- exact file/line map;
- minimum safe design options ranked by risk;
- a kill-switch checklist for Jarvis's candidate;
- required negative tests;
- explicit rejection criteria for a solution that still permits serial/nonempty and sink/all-zero disagreement.

## Coordination contract

- Work only on each coder's existing isolated worktree and branch.
- Do not share a mutable branch.
- Do not commit credentials, filled placeholders, serial logs, sink dumps, ISOs, or other hardware evidence.
- Do not stash credential-filled source.
- No coder may fire `workflow_dispatch`, merge, or perform a hardware boot.
- Send interim discoveries to the PM and Jarvis, but do not wait for synchronous approval to continue within the assigned lane.
- If two lanes discover contradictory facts, preserve both with evidence; the PM will adjudicate.
- Jarvis is the only production integrator. Oracle, Janus, and Chad are analysis-only. Cerebro owns test-only code.

## PM convergence gate

The PM will not consider a runner token until:

1. all four independent lane returns are present;
2. Jarvis records disposition of each material finding;
3. the final production candidate and hostile oracle are on Jarvis's exact SHA;
4. normalized OFF artifact equality, close gate, credentials, manifest, and clean-worktree checks pass;
5. exact-SHA push CI is green with a URL.

The four-shard gate still requires a new exact-SHA PM/PL token. A green gate will not authorize hardware automatically. No third boot and no merge.
