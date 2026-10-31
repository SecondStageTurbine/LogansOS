<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW BUNDLE (relays #175–#177) → Chad: the net tick, NeoFS grow-write, the pin charge

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. A bundle, so one model throughout (CLAUDE.md bundle rule). On their own,
#175 would be Sol high (lock discipline), #176 Luna high, #177 Luna high.

#175 and #176 review squashes already on `main` (batch 20, run `40898712909`, green at `942f20e0`). #177 reviews an
open PR before it merges. Read each with `git show <sha> --stat` and `git show <sha>` (for #177,
`git fetch origin +pull/221/head:pr221 && git diff origin/main...pr221`). One pushed RETURN per relay, each a plain
checklist (pass or fail, the enforcing `file:line`, a concrete input for any fail):
`Plans/RETURN-2026-10-29-<slug>-review-chad.md`.

Every relay also checks the PR's prior-art section (CLAUDE.md): the CP table for #175 and #177. A missing section is
a FAIL of that item.

## #175: #215 `46dcda0e`, OT-95 the periodic net tick. Slug `net-tick`

1. Does the net-tick thread take every net lock with interrupts off, and does every other taker of the same locks
   (syscalls, `net-rx`, `release_net`, the harness) also take them IF-off, or through an `IrqSafeMutex`? List any
   IF-on taker you find (CP-37, both directions).
2. Are the drivers/net device table and the identity registry `IrqSafeMutex` now, and is nothing printed, allocated
   or parked while either is held?
3. Does the tick run the TCP retransmit, ARP/NDP aging and DHCP timers without holding one net lock across a call
   that takes another in the opposite order elsewhere?
4. Is the thread bounded per tick (no unbounded loop over a table an outside peer can grow)?
5. Kill-switch: with `net-tick` off, is the kernel unchanged from before?
6. `net_tick_retransmit`: does it fail with the tick disabled or with retransmit removed?

## #176: #212 `e3d88fcd`, NeoFS records rebuilt after a grow take the grown size. Slug `neofs-grow-write`

1. Every place that builds a new inode record from an old one (write, truncate, and any other copy-on-write path):
   does each set `fs_block_count` to the volume's current size? Name any that don't.
   **Note:** #209's `SYS_FILE_REPLACE` (`commit_regular_replace`, not yet merged) had the same gap; the PM added the
   refresh in the batch 21 gate (`8a7366c8`). Check it there too.
2. Does `neofs_grow_write` fail with the fix reverted (the AAG's concern: first-fit could reuse blocks freed below
   the old end)?
3. Does the host post-check (`host_check_neofs_grow_write`) verify the grown volume independently of the kernel's
   reader?

## #177: #221 `38f8de07` (open), OT-61 C2.2b, the buffer-map pin charge. Slug `ot61-pin-charge`

Muse #163 found the original gap (an uncharged, unbounded pin per map).
1. Is each pin charged before the pin is pushed, under the same lock, so two CPUs mapping at once can't both pass
   the budget check?
2. Is the charge released exactly once on every path: each error after the charge, a force-kill mid-syscall, and the
   Pmap drop?
3. Is `MAX_BORROWED_PINS` enforced in every build, and is a refused map refused before any state changes?
4. The stated deviation: pins aren't released on unmap. Is there a userland caller that maps repeatedly in a long-lived
   process and would now hit the cap? Name it, or pass.
5. Does the scenario fail with the weight set to 0?
