<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# FIX-FORWARD: MT7601U AAG hostile-oracle binding and OFF reproducibility -> Jarvis

**Date:** 2026-07-24
**From:** PM
**Rejected runner candidate:** `00b45d418428680b0ede8d727664df27cda8c6ab`
**Verified push CI:** `https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/30120749427`
**Controlling review:** `Plans/REVIEW-DISPATCH-2026-07-24-mt7601u-candidate-00b45d4-aag.md`
**Verdict:** **AAG BOUNCE. Build one fresh-base validation recut. No workflow dispatch, ISO credential build, hardware, merge, or PR.**

## Panel disposition

### Lane A — Oracle: `TOKEN-OK`

Accepted:

- MCU response TD is armed before command OUT and retains exact ownership;
- abandoned-lap completions cannot become a current valid response;
- the endpoint probe and response-bearing retry are bounded;
- reported endpoint recovery is diagnostic, not liveness proof;
- no forced reload, forced `COM_REG0=0`, or post-terminal continuation exists;
- FCE upload evidence distinguishes channel commands from firmware upload;
- the terminal remains evidence-neutral and leaves the proximate cause open.

Non-blocking notes: theoretical 4-bit sequence-wrap alias and reliance on xHC transfer residue for a valid short RXFCE completion.

### Lane B — Cerebro: `BOUNCE`

The production path itself passed Cerebro's sink and terminal-truth inspection. The accepted hostile oracle is nevertheless not bound to it:

- `qualify_policy()` runs only reference/synthetic policies;
- no `WedgePolicy` backed by the production `channel_failure_action()` and `channel_fault_disposition()` reaches the qualification entry point;
- the smaller survey-control test initializes `destructive_reloads=0` and never derives or changes it;
- constant-returning terminal oracles declare no reload/no post-terminal traffic rather than deriving those properties from the hostile operation trace.

The oracle's own documentation states that the candidate remains unqualified until the production binding exists. A green reference model is not candidate proof.

### Lane C — Prometheus: source review accepted; `TOKEN-OK` rejected

Accepted at source level:

- operation-ledger code and call sites are structurally gated under `mt7601u`;
- the pure budget core is monotonic and non-replenishing;
- nested guards restore prior wait/operation state;
- no new IRQ-side lock, allocation, DMA surface, or governance-lock reachability was found;
- committed credentials are blank and `CHANNEL=0`;
- the close-gate `usb-host + !mt7601u` compile KAT is present.

The artifact verdict is incomplete. The review dispatch required paired normalized SHA-256 values against baseline `0be953de11695ced27ef86dfe9b4e06f2abfd18a`. Prometheus could not build and returned placeholders labeled "structural proof." It also audited the delta from `33030355...` while claiming equality to the older baseline. Structural inspection does not supersede the explicit artifact-equality gate.

## Required validation recut

Preserve the rejected branch and SHA. Do not amend, rebase, force-push, or delete it.

Create a new validation branch from current `origin/main`. Replay the seven candidate commits without semantic changes, then append the narrow Lane-B repair. Return mapping from every old commit to replayed commit and prove that all production/kernel blobs are identical to `00b45d4` after excluding the new governance documents and the hostile-oracle binding change.

### Production-bound hostile oracle

Bind the approved hostile oracle to the shipped production decision seams:

- implement its `WedgePolicy` contract using `channel_failure_action()` and `channel_fault_disposition()`;
- map the production one-probe/retry and terminal decisions into the oracle's action vocabulary;
- never synthesize `ForceColdReload` when the production disposition forbids destructive reload;
- call `qualify_policy()` with the production-backed adapter and require `qualified()`;
- derive the accepted trace's no-reload/no-post-terminal properties through the operation log and contract checker, not a never-mutated counter or constant-returning declaration.

Do not change production behavior to make the test green. Return the new hostile-oracle blob hash and the exact focused test list/count.

### Normalized OFF equality

Against baseline `0be953de11695ced27ef86dfe9b4e06f2abfd18a`, rebuild:

1. default kernel;
2. `usb-host + !mt7601u`;
3. `usb-hid + !mt7601u`.

Use isolated clean baseline/candidate worktrees, clean target directories, the same pinned toolchain, fixed build ID, and the same normalization tool and command. Return for every row:

- exact build command and feature set;
- raw and normalized artifact path and byte size;
- baseline normalized SHA-256;
- candidate normalized SHA-256;
- explicit byte-equality result.

Paired hashes are mandatory. A structural argument or single unpaired hash is insufficient.

## Gates and RETURN

Run:

- the focused production-bound hostile oracle;
- all MT7601U host tests;
- feature-on bare-metal association build with committed credentials blank;
- `scripts/worker-close-gate.sh` in one clean, successful sequence;
- automatic push CI on the exact final SHA.

Return:

- new branch, tip, parent, current-main base relation, and old-to-new commit map;
- own-stat and exact touched-file list;
- production/kernel blob-parity proof against `00b45d4`;
- focused and full host-test results;
- complete three-row normalized OFF equality table and reproducibility commands;
- credentials and `CHANNEL=0` proof;
- clean-worktree and remote-parity proof;
- exact-SHA push-CI URL.

After the recut, Cerebro re-reviews the binding, Prometheus independently reproduces the artifact equality using a build-capable environment, and Oracle confirms production blob parity. No four-shard token is granted here.

Chad's FADT-reset qualification remains a separate hardware prerequisite. No code from that lane is authorized for integration by this fix-forward.
