<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH — seats A, B and C, round 2

**Date:** 2026-10-29 (evening) · **From:** PM. The environment notes from round 1 still apply: QEMU at `NEO_SMP=4`,
`LIMINE_DIR=/opt/limine`, the pinned OVMF, and deleting `target/neo-test` images between runs.

**First, always:** a bounce on any of your round-1 PRs comes before the items below. The PM gates them in batches
(22: #204 + #219; 23: seat B's PRs; 24: seat C's PRs) and posts any finding on the PR.

## Seat A: the power-path fixes (one PR, off `origin/main`)

These are independent of #204/#219.
- **#140 (Janus, `Plans/RETURN-2026-10-29-ot90-p1-review-janus.md` item 4, F1):** an unsupported sleep-register GAS
  geometry (access width, bit offset, address space) must be refused fail-closed when the descriptor is validated,
  not used.
- **#154 (Jarvis, `Plans/RETURN-2026-10-29-ot90-p2-review-jarvis.md` items 2 and 4):**
  - F2: an accepted reset or sleep GAS whose physical address isn't HHDM-representable must be refused (an overflow
    check, not just the physical width);
  - F5: the judge must check the duration, and Jarvis's assertion (a RESET/SHUTDOWN event out of its window is
    refused) must pass.
- A KAT for each refusal, using the exact descriptor bytes from the reviews, and the kill-switch statement.

Then, once #204 has merged: **#171** (Chad b19, `Plans/RETURN-2026-10-29-exit-codes-bounded-review-chad.md`), its own
PR.

## Seat B: the app-model THINK, then A1a (#220)

1. **A docs PR amending `docs/THINK-2026-10-29-ot88-app-model.md`** per Chad #144
   (`Plans/RETURN-2026-10-29-ot88-think-review-chad.md`):
   - item 2: the hung-app escape claim. The WM must not block on a client's reply send;
   - item 3: "death is local" must classify every current exit, including reply-send failures;
   - item 4: CP-57, and the wait set (9 items vs the cap of 8);
   - item 5: cross-process futex wake exists (`futex.rs:63`), so T-b's rationale changes;
   - the stdio kill-switch and the open-with protocol;
   - item 8: F0's AAG made explicit.

   Also Janus #145's THINK corrections, if they're still open.
2. **Rework #220** (`claude/a1a-apps` @ `9e46d426`) to the amended THINK:
   - an async or bounded handshake and reply;
   - client-local comms, protocol and launch arms, with non-blocking cleanup;
   - the wait-set budget;
   - disconnect-during-reply and Lens-failure legs.

   S1's own verdict was that #220 is A1a's skeleton until #144's points land. The PR body states which of #144's
   items A1a solves and which ride to A1b/A2.
3. **#222** (settings WM): rebase once #216 merges.

## Seat C: #218 wire receive, the next round (old S4's lane)

`claude/ot96-wire-rx` @ `2c4a0f67`. The PM's AAG findings are fixed. Chad #173 (`Plans/RETURN-2026-10-29-ot96-aag-chad.md`)
is still open:
- **F1, IF-on takers:** `kernel/src/main.rs:542` `virtio_net::service_all` after neo-init's reap, and mt7601u's
  late-bind DORA loop. Take them IF-off, or route them through the net-rx thread.
- **F2, unbounded work:** `flush_wire_egress` under a concurrent producer, and virtio's uncapped used-index loop.
  Bound both per pass.
- **F3, listener retirement:** use your #230's flag. Rebase #218 onto `main` once #230 merges, or stack it and say so.
- **The CP-56 virtio TX-header row.**
- `wire-rx` must require `net-tick`; #215 is merged now.
- A falsifier for each, the close-gate, and a comment starting `RETURN (seat C):`. Chad's narrow re-check follows.

R3 (`claude/ot96-r3-dhcp-renew` @ `8a6ad8dc`, stacked on #218) re-stacks only once #218 has merged (CLAUDE.md:
re-stack on a final base).

## Close condition (each item)

- `scripts/worker-close-gate.sh` passes; its evidence block is in the PR.
- A comment starting `RETURN (seat X):` with the head SHA and the scenario results.
- Don't merge. Commit and push at every checkpoint. No model identifiers.
