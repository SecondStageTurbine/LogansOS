<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Seat I, A3p: the WM's parked loop (2026-11-12)

**Seat:** Seat I (cloud). Start after #100 (S4) lands.
**Branch:** `claude/a3p-parked-loop`, cut from `origin/main`.
**Design:** `docs/THINK-2026-10-29-ot88-app-model.md`. The A3p row is in §7. §2.3 covers item 9 and the
pitfall-bank pass.
**Why now:** `Plans/AUDIT-2026-11-11-mvp-app-chain.md` §3 lists A3p as buildable. Its dependency, A1a, has landed, and
it conflicts with no lane in flight:
- A2 waits on #98;
- F0 is #106;
- S6b is Seat C's;
- OT-100 is Seat F's.

It's off the MVP's critical path. It stops the WM from spending CPU while idle, and it gives the A1b/A1c window work
the wait set it needs.

## Build A3p's row exactly

1. **Raise `wait_any`'s bound.** `ITEMS_MAX` goes from 8 to 16 (`kernel/src/sched/wait_any.rs:46`). That's 15 items
   plus one spare.
   - Every array sized by it must still be bounded. Check the syscall's `keys`/`caps` copy-in at `syscall.rs:6931`.
   - Show the per-wait stack and heap cost at 16.
   - A 17-item wait must stay `EINVAL`, returned before any copy.
2. **Make `desktop-park` image A's default.** The WM's turn loop parks on `SYS_WAIT_ANY` over the session and surface
   channels of every live client, the pointer route, Home's focus and the SAK latch, instead of polling.
   - Keep item 9's fairness: a rotating order, and at most one request per client per turn.
   - Keep A1a's try-only sends.

## Gate (the A3p row, plus the kill-switch)

- **Wait-set scenarios (4-shard, full tier):**
  - a 16-item wait parks, and wakes on its last item;
  - a 17-item wait is `EINVAL`;
  - **CP-14:** a send in the window between the WM's last try-receive and its park still wakes the WM. The
    discriminator sends on the last item inside that window.
- **CP-61 scenario:** a client dies while the WM is parked, and its handle slot is reused before the WM's next wait.
  The WM must neither wake on a stale slot nor miss the new client.
- **Image A, parked by default:**
  - the existing desktop and login scenarios pass with the WM parked;
  - an idle-CPU witness shows the WM no longer busy-polls (`desktop_idle_parks` today).
- **Kill-switch:** with `desktop-park` off, the loop is today's polling loop, bit for bit. Prove it with the image-A
  size and digest comparison #98 used, or with unit equality where that can be expressed.
- **Close:** `scripts/worker-close-gate.sh`, then the personal-details scan.

## Required sections

- **Concurrency pitfall-bank table** (`docs/concurrency-pitfalls.md`). For each class, say whether this change
  introduces it, guards it or leaves it, and name the discriminator:
  - at least CP-14 (lost wake), CP-61 (reused slot), and the CP-19…28 park/wake hand-off classes;
  - any class the larger stack array touches.
- **Prior art:** none needed for a bound and a default. Say so in the PR.
- **I-14:** no driver path is touched. State it.

## Review

**Kernel, SMP wait core.** When the PR is green, the PM queues:
- the **AAG** with a **cross-family Codex lens** on the park/wake seam (GPT-6.1-Sol, high);
- the **CP table** check.

## Close

Commit and push at every checkpoint. Post PM: on the PR with the run URL.

**Disk reclaim:** once the PM has merged the PR and confirmed it is on `origin/main`, `cargo clean` the worktree and
remove it.
