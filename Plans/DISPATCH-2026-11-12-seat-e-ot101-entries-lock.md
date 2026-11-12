<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Seat E, OT-101 — NeoFS stops holding its namespace spin lock across device waits (2026-11-12)

**Seat:** Seat E (cloud), after F0 (#106) lands.
**Branch:** `claude/ot101-entries-lock`, cut from `origin/main` once F0 is on it.
**Thread:** OT-101 in `docs/OPEN-THREADS.md`.
**Finding:** Chad's F2 in relay 396, `Plans/RETURN-2026-11-12-pr106-review-chad.md`, which includes a concrete input
and the virtio-blk park points.

## The problem

`NeoFs.entries` is a `SwitchMutex` (`kernel/src/sync/switch_mutex.rs:180`): it spins, it doesn't park. NeoFS paths that
existed before F0 hold it through cache-miss block reads and through dirty throttling (`inode.rs:975`, a whole drain).
So any other writer on the volume spins for the full device wait, and a preempted holder strands every spinner (CP-63).
Chad found no ABBA, so there is no deadlock, but the contention is unbounded.

F0's own four operations already avoid this: they read outside `entries` optimistically, revalidate a version, and
return `EAGAIN` after 8 tries, with pinned superblock copies (#106, `a0572f1f`). OT-101 covers every other holder.

## Do this

1. **Inventory every `entries.lock()` site** in `kernel/src/fs/neofs/` on main (about fifteen as of #106: `inode.rs`
   around :331, :373, :452, :636, :698, :884, :896, :999, :1014, :1096, :1259, :1313, :1325, :1395, :1450). For each,
   say in a table in the PR:
   - whether it can reach a device read or a wait while it holds the lock (cache miss, throttle, drain);
   - its caller's context: syscall thread, writeback kthread, or boot.
2. **Choose the fix, and say why, at the top of the PR:**
   - **(a) A parking lock** for `entries`, if every acquirer runs in thread context where it may sleep. It's the
     smaller change, and it fixes the inherited paths in one place. Show that no acquirer is in IRQ context or under
     another spin lock, and keep the lock order of #106's CP-1 row.
   - **(b) F0's pattern:** read outside the lock and revalidate, at each site that can reach a device wait.
   - A mix is allowed; say which site gets which.
3. **Dirty throttling** (`inode.rs:975`) must not hold `entries` while it waits for the drain, whichever option you
   choose.
4. **Keep F0's guarantees:** its zero-held-read counters, the pinned copies and `EAGAIN` stay as they are, and every
   F0 scenario stays green.

## Gate

- **Kernel-touching:** the 4-shard gate (full tier, batch of one) and `scripts/worker-close-gate.sh`.
- **Discriminator (Chad's F2):** a scenario that holds a device completion during a cache-miss read under `entries`,
  then has a second writer on the same volume (`mkdir_at` or a create) and shows that it does not spin on `entries`.
  It parks, or it gets `EAGAIN` and retries. A counter (spin rounds on `entries`, or held-across-device-wait events)
  must read 0 with the fix, and non-zero with the fix reverted. Prove the second half with a test-only switch, or
  with a recorded run on the parent commit.
- **The dirty-throttle leg:** a writer throttled behind a held drain doesn't keep `entries` held.
- **Kill-switch:** state whether this is a pure correctness fix with no switch (allowed for a fix of an existing
  defect), or behind a feature. If it has a feature, off must equal today, bit for bit.

## Prior-art and pitfall banks

- **Concurrency pitfall bank (`docs/concurrency-pitfalls.md`), required:** a CP table in the PR for at least CP-1
  (lock order), CP-14 (lost wakeup, if the lock parks), CP-63 (a long call under a spin lock), CP-82 (writeback
  ordering) and CP-113 (R3's read-only flip, if #110 has landed). For each one, say whether the change introduces,
  guards or leaves it, and name the discriminator.
- **Prior art:** a sleeping lock around a filesystem namespace is standard (Linux `i_rwsem`, BSD vnode locks). Read
  them for the principle only; never copy GPL code. No survey is needed unless you borrow code.

## Review

When the PR is green, the PM queues:
- the AAG with a **cross-family lens** (Codex, GPT-6.1-Sol, high) on the lock change and every site in the table;
- a Muse read only if a site touches the write guard. Say so in the PR if one does.

## Close

Commit and push at every checkpoint. Post `PM:` on the PR with the run URL.

**Disk reclaim:** once the PM confirms the merge, run `cargo clean` in the worktree and remove it.
