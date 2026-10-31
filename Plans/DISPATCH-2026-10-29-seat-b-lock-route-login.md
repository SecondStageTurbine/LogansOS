<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH — Seat B: the lock route (#216), then product login

**Date:** 2026-10-29 · **From:** PM · **Seat:** B (a new cloud coding session; it replaces the old S2)

Read, in order: `Plans/HANDOFF-2026-10-29-pm.md` §3–§4 (the S2 rows), `Plans/RETURN-2026-10-29-l2b-aag-jarvis.md`
(#160), `Plans/RETURN-2026-10-29-ot66-l1-review-muse.md` (#152), and `docs/concurrency-pitfalls.md` CP-57, CP-97.

## Environment

This container has QEMU and Limine. Run scenarios as CI does:
`LIMINE_DIR=/opt/limine NEO_SMP=4 cargo neo-test <scenario>`. The local QEMU is 8.2 (CI uses 10.2).

## Work, in order

### 1. #216 (`claude/ot66-l2b-lock-route` @ `948979d9`): Jarvis #160's items 1, 6, 7 and 10

- **Items 6 and 1, input in flight across the chord:** forwarding checks the lock state before acquiring DELIVERY,
  and the receive-side check is separate from the pop. Make each forwarded record epoch-stamped against the lock
  state, checked under DELIVERY, so a record that crossed the chord is never delivered to the unlocked client.
- **Item 7, the drop order:** the drop publishes LOCK=0 before clearing ACTIVE, briefly reopening lock-free reads.
  Fix the order: clear ACTIVE, then LOCK.
- **Item 10:** CP-97's text overstates the guard; qualify it to what the code enforces.
- A discriminator leg for each item, each falsified by reverting its fix. Then run the close-gate and push.

### 2. Product login: open the PR

`claude/ot66-l1-product-login` @ `3a6e86d0`. Muse #152 passed. Rebase it on `origin/main`, run the close-gate, and open
the PR. In the body, name `kernel-rng` + `sak-lock` + the WM's `login` and the pairing test, and cite Muse #152. It's
kernel-touching (image features), so the PM gates it in a batch. `sak-lock-route` joins after #216 merges.

### 3. Then, as separate PRs off `main`

- **#127:** a grant recipient's last Process Arc is dropped under CAP_GRAPH, plus the deferred-drain gap.
- **#153:** an immutable tensor's write is copied before EPERM is returned.
- **#222** (`claude/ot88-s1-settings-wm` @ `1de213d2`): rebase once #216 merges.

## Close condition

- Pushed; `scripts/worker-close-gate.sh` passes; the evidence block is in each PR.
- A comment on each PR, starting `RETURN (seat B):`, with the head SHA and the scenario results.
- **Don't merge;** the PM gates and merges. Jarvis does a narrow re-check of #216.
- Commit and push at every checkpoint. No model identifiers in commits.
