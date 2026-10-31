<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW DISPATCH: MT7601U post-Q_SELECT terminal child `f4a0940`

**Date:** 2026-07-25
**From:** PM
**Candidate:** `f4a0940872e865207128a240587f7cac036f0cf9`
**Branch:** `codex/mt7601u-post-qselect-terminal-jarvis`
**Parent/base:** `8ab6409fe711e531c744d0296f8855beab7b7b3a`
**Push CI:** `30159747107` — exact SHA, attempt 1, green
**Status:** **REVIEW ONLY. No workflow dispatch, hardware boot, merge, PR,
credential-filled build, amendment, rebase, force-push, or reset action.**

## PM identity verification

The PM independently verified:

- the remote branch resolves exactly to the candidate;
- the candidate is exactly one child of the required parent;
- the child own-stat is five files, `+602/-111`;
- the touched paths are exactly:
  - `kernel/src/drivers/usb/mt7601u/linux_parity_logic.rs`
  - `kernel/src/drivers/usb/mt7601u/mod.rs`
  - `kernel/src/test_harness.rs`
  - `tools/ai-ops-tests/src/lib.rs`
  - `tools/ai-ops-tests/tests/mt7601u_hostile_oracle.rs`
- `git diff --check` is clean;
- the Jarvis worktree is clean at the exact candidate;
- the SSID and passphrase placeholders are `b""`, `CHANNEL=0`, and neither
  local credential string occurs in the committed tree;
- automatic push CI run `30159747107` is exact-SHA green on attempt 1:
  host tests/format/lint, SPDX, and monolithic identity passed; QEMU was
  skipped because no workflow dispatch was authorized.

This establishes immutable review identity only.

## Claimed child repair

Jarvis claims:

- one non-replenishing 250,000-charge ledger covers MAC initialization
  through successful EEPROM acquisition;
- failures produce one evidence-neutral, stage-specific terminal with
  response ownership, packet-RX ownership, budget, and fence evidence;
- both MCU and direct-register traffic latch, block, and count after terminal;
- durable sink flushes bracket BBP, WCID, and EEPROM initialization;
- no reload, MCU-death claim, baked channel, reset integration, or accepted
  Linux-order regression was introduced.

The returned accounting basis reserves the prior 50,000 MAC allowance, up to
approximately 20,000 for MAC-status polling, and approximately 68,000 for
eFUSE polling, leaving more than 110,000 charges for BBP, WCID, responses,
bulk completion, and vendor retries.

## Lane B re-review — Cerebro

Review the exact child against the bounce and execute the focused negative
controls. Require:

1. no uncovered blocking device operation from successful post-`Q_SELECT`
   readiness through successful EEPROM acquisition;
2. a finite, genuinely non-replenishing ledger with no nested reset,
   replacement, or escape path;
3. deterministic bounded failure at BBP readiness, both BBP table writes,
   every WCID/key/attribute burst, intervening direct-register work, and
   EEPROM acquisition;
4. exactly one evidence-neutral terminal category containing the actual
   failing stage and complete accounting evidence;
5. zero post-terminal MCU and direct-register traffic, with blocked attempts
   counted;
6. durable sink flushes at the claimed stage boundaries and terminal;
7. production-bound tests whose observations come from the production policy
   or operation trace rather than hand-declared success fields;
8. bounded negative controls for removing the enclosing budget, terminal
   flush, and post-terminal gate.

Return `TOKEN-OK` or `BOUNCE` with exact commands, counts, and file/line
evidence. Review only; do not change the candidate.

## Lane A delta review — Oracle

Review only the child delta for regression of the parent’s accepted Linux
parity:

- resident firmware remains a mutation-free early return;
- response-TD ownership and sixteen packet-RX buffers remain established in
  Linux order before MAC traffic;
- MAC frames, chunk boundaries, sequence behavior, and final-chunk-only
  response policy remain unchanged;
- cold upload remains intact;
- no reset, reload, or invented recovery has reappeared.

Also ensure that the expanded budget/terminal layer does not change healthy
Linux operation ordering or command bytes. Return `TOKEN-OK` or `BOUNCE`.

## Lane C child review — Prometheus

Review the exact child and correct the prior patch-ID reporting. Require:

- the true frozen-to-replay pairwise stable patch IDs, including
  `9ff50e15... ↔ e71d1645... = 2852b75b...` and
  `625e531f... ↔ 22776a71... = fa91766d...`;
- exact child parent, five-path scope, and `+602/-111` own-stat;
- credentials blank and `CHANNEL=0`;
- structural absence of the child’s policy, ledger, terminal, and gate under
  `!mt7601u`;
- isolated normalized OFF equality against
  `0be953de11695ced27ef86dfe9b4e06f2abfd18a` for default, `usb-host`, and
  `usb-hid`, using a fixed build identity and a demonstrated ELF-preserving
  normalizer;
- blank ISO identity and the returned blank ISO size/hash if reproducible.

Return paired sizes and SHA-256 values plus `TOKEN-OK` or `BOUNCE`.

## Hold boundary

The candidate is frozen at `f4a0940872e865207128a240587f7cac036f0cf9`.
Reviewers must not substitute its parent or review a mutable branch tip.

Only after all three lanes return on the exact child will the PM decide
whether to grant an exact-SHA four-shard workflow token. Four-shard success
would remain separate from credential-filled build and hardware authorization.
