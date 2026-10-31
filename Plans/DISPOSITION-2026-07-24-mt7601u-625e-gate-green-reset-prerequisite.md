<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPOSITION: MT7601U `625e531f` gate green; warm-reset prerequisite remains

**Date:** 2026-07-24
**From:** PM
**WiFi candidate:** `625e531f353b417498a968dfa0b74f1d45c0e592`
**WiFi branch:** `codex/mt7601u-aag-oracle-binding-jarvis`
**Four-shard run:** `https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/30135801447`
**Warm-reset input:** `5ea8cc8eaeeb83c743ea322d2883574e7ddaee14`
**Verdict:** **Software gate accepted. Freeze the WiFi candidate. ISO and silicon remain on HOLD pending a fresh-base warm-reset qualification return.**

## Independent gate verification

The PM independently inspected GitHub Actions run `30135801447`:

- event `workflow_dispatch`;
- attempt 1;
- head SHA exactly `625e531f353b417498a968dfa0b74f1d45c0e592`;
- conclusion `success`;
- QEMU integration shards 1, 2, 3, and 4 all `success`;
- host tests/format/lint, SPDX, and monolithic identity all `success`.

The remote branch still resolves exactly to the authorized SHA. The AAG driver,
contract-binding, replay-parity, and normalized-OFF reviews are accepted. This
closes the software gate for the WiFi repair; it does not authorize a merge,
credential-filled build, rig staging, reset, or boot.

## Why the ISO hold remains

The accepted WiFi candidate does not contain the separately developed
FADT-derived reset actuator. The previous return-to-Ubuntu transition performed
a cold firmware upload on the next NeoPlutarch boot and therefore disproved warm
residency for that procedure.

Building a credential-filled ISO from `625e531f...` now would produce an image
that cannot execute the qualified PASS-only reset transition. It would either
repeat the invalid transition or be replaced after actuator integration. Do not
build, copy, burn, or boot that ISO.

## Lane R1 — Chad: fresh-base validation recut

Read:

```powershell
git fetch origin
git show origin/main:Plans/DISPOSITION-2026-07-24-mt7601u-625e-gate-green-reset-prerequisite.md
git show origin/main:Plans/DISPATCH-2026-07-24-mt7601u-warm-reset-qualification-chad.md
git show origin/main:Plans/FIX-FORWARD-2026-07-24-mt7601u-warm-reset-qualification-chad.md
```

Preserve `2fb648ed8ae688ec65bff6ec5c81c3b05d3b92ec` and
`5ea8cc8eaeeb83c743ea322d2883574e7ddaee14` unchanged. Create a new branch from
current `origin/main` and replay those two commits without semantic changes.
Do not amend, rebase, force-push, or delete the reviewed branch.

Return:

1. old-to-new commit map, stable patch IDs, tree/blob parity, base relation,
   own-stat, exact touched paths, remote parity, and a clean worktree;
2. the focused FADT parser/discovery test command and exact passing count;
3. the complete seven-row normalized candidate-OFF equality matrix required by
   the controlling fix-forward—explicitly name every row and feature set, and
   provide baseline/candidate raw and normalized paths/sizes, six reproducibility
   inputs, normalizer identity/command, paired SHA-256 values, and `cmp` result;
4. one uninterrupted successful `scripts/worker-close-gate.sh` transcript;
5. exact-SHA automatic push-CI URL.

Use clean isolated targets, the pinned toolchain, `CARGO_INCREMENTAL=0`, one
fixed `NEO_BUILD_ID`, and the same normalization command for both sides. A
symbol/string absence scan is corroboration only and cannot replace byte
equality. If any replay changes a production patch ID or blob, stop and return
the divergence rather than repairing it silently.

No workflow-dispatch token is granted. No hardware, reset, boot, credential
build, PR, merge, or Jarvis-branch edit is authorized.

## Lane R2 — Janus: exact-return review

After Chad returns an immutable fresh-base SHA, independently review that exact
commit. Reconfirm:

- production discovery exercises unrelated XSDT and RSDT siblings before FADT;
- the live descriptor is fully validated;
- the actuator is reachable only under the exact three-feature qualification;
- only an authorized terminal PASS can invoke it;
- the final breadcrumb and evidence paths are flushed before exactly one
  FADT-derived port write;
- there is no fallback or hard-coded machine tuple;
- all seven normalized OFF pairs are reproducible and byte-identical;
- the complete close gate and exact-SHA push CI are green.

Return `TOKEN-OK` or `BOUNCE` with exact evidence. Do not touch hardware or a
mutable coder branch.

## Jarvis boundary

Keep `625e531f353b417498a968dfa0b74f1d45c0e592` immutable. Do not integrate
Chad's current or future branch without a new PM convergence dispatch. Do not
build a credential-filled ISO yet.

If the reset lane is accepted, the PM will dispatch a new integration child
from the frozen WiFi candidate, require a new host/AAG review and exact-SHA
four-shard gate, and only then authorize one local-credential ISO recut. The
subsequent silicon authorization will name that exact ISO and a bounded
cold-to-immediate-warm procedure.

No merge is authorized.
