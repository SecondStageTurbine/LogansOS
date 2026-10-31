<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# EVIDENCE FIX-FORWARD: OT-61 C1.1 candidate `a061bd6f`

**Date:** 2026-07-25
**From:** PM
**To:** Cerebro
**Returned tip:** `61d50a9951472b876389cf53b525d5222f7ac764`
**Provisional code candidate:** `a061bd6fd92476c8c5fd24079546b18516458ce4`
**Base:** `87d4244fba43d936892eb3307b7155dc303a449c`
**Candidate push CI:** `30173847819` — exact SHA, attempt 1, green
**Verdict:** **EVIDENCE/SCOPE BOUNCE. No new code commit, push, workflow
dispatch, merge, PR, hardware action, or additional coder.**

## PM identity and scope disposition

The PM independently verified:

- the returned remote branch resolves to `61d50a99...`;
- its merge-base with the authorized base is exact;
- the three replay pairs are content-identical by stable patch ID;
- `a061bd6f...` contains the complete C1.1 repair and is exact-SHA push-CI
  green on attempt 1;
- `61d50a99...` differs from `a061bd6f...` only by a 57-line change to
  `scripts/worker-close-gate.sh`;
- the returned tip's push CI failed on attempt 1 in the pre-existing
  non-hermetic `lo_isc1_decide_is_o_devices` timing assertion and passed only
  on attempt 2.

The resume dispatch did not authorize a global close-gate implementation
change. The timeout hardening is a useful banked finding, but it is not part of
the browser C1.1 candidate. Do not amend or rewrite either SHA. The PM selects
`a061bd6f...` as the provisional code candidate and excludes
`61d50a99...` from C1.1 review.

The selected candidate own-stat is:

```text
21 files changed, +4010/-87
```

Automatic push CI:

```text
https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/30173847819
```

Host tests/format/lint, SPDX, and monolithic identity all passed on attempt 1.
QEMU correctly skipped because no workflow token existed.

## Correct the replay record

The RETURN's printed patch-ID values are incorrect even though pairwise
equality is true. Use the independently recomputed values:

```text
807b7293 ↔ 6afeb183
  df53af96e233883a911f65fad923e1471e04e1ae

375b1ac6 ↔ 7d5a2e9d
  b81604c651b265e9ca0e2513ecd63d141990fde3

b1b969c5 ↔ 604406ab
  472f973e2b521fe2054eb57198c646da80fd0d62
```

Return the exact commands used and all six old/new values.

## Evidence still owed

Complete the following without editing or committing the candidate.

### 1. Rejected-SHA red proof

The original AAG fix-forward explicitly requires the rejected candidate to
fail the new blocker-A and blocker-B discriminators. Structural reasoning is
not a substitute.

Use isolated detached worktrees for:

```text
rejected: d5e79fc7ce0f66ad8fb2e804b696b7c9a8c0d6e1
fixed:    a061bd6fd92476c8c5fd24079546b18516458ce4
```

Apply only the minimum test harness needed to exercise the same discriminator
against each tree. Do not commit the transplanted tests. Return:

- blocker A: rejected reaches the ready-ring panic/ownership failure; fixed
  retains exactly one waiter and resumes it;
- blocker B: rejected shows linear live-process stack/mapping growth; fixed
  reclaims all 73 stacks across all three routes to the warmed baseline;
- exact commands, terminal excerpts, and artifact hashes;
- clean detached-worktree status after removing the temporary test changes.

### 2. Real OFF equality

The selected candidate must not ship a hidden runtime delta when
`browser-runtime` is disabled. `.text`/`.rodata` equality plus anonymous
`.data` divergence is not the required normalized artifact proof, and source
line movement is not an unavoidable blocker.

Build baseline and candidate in isolated target directories with:

- `CARGO_INCREMENTAL=0`;
- a fixed `NEO_BUILD_ID`;
- `-Z location-detail=none`;
- identical pinned toolchain and environment.

Normalize the resulting real ELF with an ELF-preserving
`llvm-objcopy --strip-all`. Prove with `file`/`readelf` that the outputs remain
ELFs and that every `SHF_ALLOC` section is retained. Return paired sizes,
SHA-256 values, and `cmp` verdicts for:

- default;
- `usb-host` with `!browser-runtime`;
- `usb-hid` with `!browser-runtime`.

### 3. Close-gate transcript

Return one uninterrupted successful
`scripts/worker-close-gate.sh` transcript executed at exact
`a061bd6f...`, using the script from that SHA. Do not substitute the excluded
timeout-script tip.

The known `lo_isc1_decide_is_o_devices` benchmark flake must be identified
plainly if it fires. A failed run followed by a re-fire is not the same as one
uninterrupted close-gate pass.

### 4. Worktree truth

The shared worker tree is not clean. The PM observed five untracked paths:

```text
3dvon8gkkn57fwbhz3y7wwibj (# Delete conflict 2026-07-24 hycu7qC #).o
7u3cdk43kz5wufxfa7krlvtsz (# Delete conflict 2026-07-24 ypvf1cC #).o
Plans/FOLLOWUP-2026-07-22-deterministic-iso-tworebuild-proof-owed.md
Worktrees/
docs/research/vello-recording-ir-inventory.md
```

Do not delete, move, stage, stash, or modify them. Report the shared tree as
dirty-but-pre-existing and use clean detached worktrees for evidence. Never
claim an empty porcelain unless it is actually empty.

## Held findings

- The close-gate timeout hardening is banked for a separate governance/tooling
  review; it is not rejected on substance, only excluded from this candidate.
- `lo_isc1_decide_is_o_devices` is a pre-existing non-hermetic benchmark and
  needs a separate owner later.
- The reported MT7601U host-test wedge is relayed to the active WiFi review
  lane as diagnostic evidence; do not edit WiFi code from this assignment.

## Hold boundary

After the evidence addendum, the PM will decide whether
`a061bd6fd92476c8c5fd24079546b18516458ce4` is ready for independent C1.1
review and an exact-SHA four-shard token.

No code change, new commit, push, workflow dispatch, hardware action, merge,
PR, or new coder delegation is authorized.
