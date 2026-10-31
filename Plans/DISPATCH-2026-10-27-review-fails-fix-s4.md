<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: fix the post-merge review FAILs on K1a-0 (#166), then OT-61 C1.2 (#150) and C1.3b (#161) → S4

## 0. FIRST, WITH A DEADLINE: K1a-0 (#166). RETURN: `Plans/RETURN-2026-10-27-k1a0-cap-graph-review-janus.md`

Janus's review (GPT-6.1-Sol high) FAILs #166. **`CAP_GRAPH` is not feature-gated, so this is in the product image.**
The PM re-checked F1 and F2 at `149d4a19`, and both hold.

This is the cross-family divergence the SMP rule exists for: Muse's PASS (#108, item 7) said nothing parks or waits
under `CAP_GRAPH`.

**Fix-forward. Push a gate-ready PR, `claude/k1a0-review-fixes`, by 18:00 UTC (2:00 pm EDT) today, or the PM reverts
#168 and #166.** This comes before everything else below. Commit and push any work in progress first.

- **F1: a snapshot `Arc<Process>` can be the last owner, and its drop runs a shootdown under `CAP_GRAPH`.** The Arcs
  come from `processes_snapshot()` in `retire` (`process.rs:1735`) and in `cap_is_live` (`:1780`), and from
  `revoke_handle`'s vector. The natural reaper takes `PROCESSES` out at `:2872` and drops its Arc at `:2880`, outside
  `CAP_GRAPH`. So a concurrent scan can be left holding the last Arc. Its drop runs `Pmap::drop`, which calls
  `pte_write_fence_and_shootdown` (`mm/pmap.rs:~1082`). That shootdown waits on a CPU spinning for `CAP_GRAPH` with
  IF=0, and after 50 ms it times out and panics.

  **Requirement:** nothing drops a `Process` or `Pmap` while `CAP_GRAPH` is held. Possible fixes:
  - hand the snapshot vectors back to the caller to drop after unlock;
  - or take the reaper's `PROCESSES` removal into its `CAP_GRAPH` section, so a scan that sees P always ends before
    the reaper drops it. Check the terminate path the same way.

  Choose one and justify it. Add Janus's discriminator: pause the reap after teardown, have a graph scan pin P, finish
  the reaper's release, put a second CPU into contention, then release the scan. Require no shootdown under the lock.
- **F2: the last tensor release can park under `CAP_GRAPH`.** The chain is `HandleTable::drain` / `remove` (`:1435`,
  `:1374`, `:1421`) → `release_tensor` → `arena::release` → `drain` → `reservation::guarded_clone`. That parks when
  the slot is Reserved (`reservation.rs:450`).

  **Requirement:** no tensor release under `CAP_GRAPH`. Defer `release_tensor` to the caller's after-unlock drop of the
  returned handles: close, revoke and teardown already return them. Add a discriminator: close or drain the last
  Tensor handle while its slot is Reserved, and assert that no park happens with `CAP_GRAPH` held. A try-lock witness
  in the park path works.
- **F3: `TicketMutexGuard` is `Sync` for `T: Send`** (`sync/ticket_mutex.rs:28–30`). Make the guard `Sync` only when
  `T: Sync`, using a marker field or an explicit bound. Add a compile-fail or `static_assertions`-style KAT.
- **Not blocking, but fix in the same PR if it's cheap:**
  - Item 4: make the HAS_CHILDREN wording say "may have children".
  - Item 1: CAP_GRAPH is not literally outermost, because `Channel.state → CAP_GRAPH`. ADR-010 already says so, so
    make the comments agree.
  - Item 6: the race hook defers instead of contending, so it can't catch a split grant section. Note it as a follow-up
    discriminator in the PR, not here.
- **CP bank:** add the two new classes. (a) A last `Arc` dropped under a global IF-off lock runs a shootdown
  (CP-56's hidden-Drop lesson, at a new site). (b) A park under a ticket lock. Correct CP-87/88's "nothing under it
  waits on another CPU" (`docs/concurrency-pitfalls.md:3951`).
- Kill-switch: none. This is a fix to an ungated path. Give the `.text` delta and the full 45-scenario sweep again.
  The post-merge cross-family review (Janus, GPT-6.1-Sol medium, narrow re-check of F1–F3) is required.

Then sections 1 and 2 below (no deadline, `browser-runtime` only).

**Why.** Jarvis's reviews (GPT-6.1-Sol high) FAIL C1.2, C1.3 and C1.3b. The PM re-checked them at `origin/main`
`91320d01`. Every finding below holds except C1.2 item 5.

All of this code is under `browser-runtime`, which is in no product image. So this is fix-forward. **These fixes
come before C2.2 merges**, and no further `browser-runtime` PR merges until they land. Commit and push your C2.2 work
in progress to its branch first, then do this as one PR: `claude/ot61-c1-review-fixes`.

## 1. C1.2: FS.base cache vs a user selector load. RETURN: `Plans/RETURN-2026-10-27-ot61-c1.2-tls-review-jarvis.md` items 2, 3, 7

- **The finding.** Ring 3 can run `mov ax, 0x23; mov fs, ax`, loading the flat user data selector from `gdt.rs:205`.
  That sets the hardware FS.base to 0 even with FSGSBASE off (SDM Vol. 3A §3.4.4). The per-CPU cache
  (`scheduler.rs:1019`) never reads the hardware, so two things go wrong:
  - a later switch-in of any thread whose `fs_base` equals the cached value skips the WRMSR. That can be a thread in
    another process, since layouts repeat;
  - `thread_set_tls(B)` returns 0 while the hardware still holds 0.
- **The fix requirement.** Whenever the kernel returns to ring 3, the hardware FS.base equals the thread's field,
  whatever the user loaded into FS. Possible fixes:
  - drop the cache, so switch-in and the setter always write the MSR;
  - on switch-out, invalidate the cache whenever the FS selector is non-zero;
  - zero the FS selector and invalidate on every switch-in.

  Choose one and justify it, including the WRMSR cost you measured or cite.
- **A discriminator in the gate.** A thread sets TLS B, loads selector `0x23` into FS, yields to a kernel thread, then
  reads `fs:[0]` and sees its key block. Also: the setter called again after a selector load takes effect.
- **CP-83.** Correct its premise (`docs/concurrency-pitfalls.md:3767`) and the `thread.rs:181` contract. Selector
  loads change FS.base too, and so would a future WRFSBASE.
- **Item 5 (the final-ELF gate doesn't flag `fs:` instructions): PM ruling, no change.**
  - The build dispatch asked the gate to catch native TLS. Native TLS always leaves one of the four marks the gate
    checks (`ot61_tls_gate.rs:1–22` explains this).
  - The `%fs:` clause was in the PM's review checklist, not in the build's contract.
  - A hand-written `fs:` access isn't native TLS. After fix 1, it can't hurt any thread but its own.
  - The std-real ELFs already get the instruction scan (C0.5 A4, `main.rs:6220`).

## 2. C1.3b: two ways a sibling survives exit_group. RETURN: `Plans/RETURN-2026-10-27-ot61-c1.3b-review-jarvis.md`

(This also covers C1.3 item 2, in `Plans/RETURN-2026-10-27-ot61-c1.3-review-jarvis.md`. C1.3b's futex refusal turned
that late park into finding 1.)

- **Finding 1: a refused kernel join spins forever.** `thread_join_for_current` (`process.rs:3272`) discards
  `wait_on_key`'s ESRCH with `let _ =` and loops on a done-word nobody will set. It loops in CPL0, with IF=0 from
  SFMASK, so it never reaches the syscall-return retire check. PM-checked.
  - Fix: on ESRCH, leave the loop and return to the syscall boundary, which then retires the thread. Check every
    other kernel caller of `wait_on_key` for the same discard.
  - Add the missing discriminator: a late `SYS_THREAD_JOIN`, not a raw futex wait, held past the purge. Role 3 only
    covers the raw futex wait.
- **Finding 2: the retirement IPI can be used up after the last check.** Some syscalls return with IF=1 (`spawn_user`
  through `LocalShootdownQuiesce::resume`, `tlb_shootdown.rs:49`). The retire check runs at `syscall.rs:1275`, but the
  `cli` doesn't come until `:806` or `:909`. An IPI that arrives in between EOIs and skips the retire, because it
  interrupted CPL0. Then IRETQ runs the thread's ring-3 loop forever, on an AP with no tick. PM-checked: the window is
  there, and the comment at `:800` says IF=1 returns exist.
  - Fix: run the last retire check with interrupts off, so nothing can be missed between it and IRETQ. Two ways:
    - `cli`, then check, then return, with the check able to retire;
    - or a per-CPU pending-retire latch that the IPI sets and the IF=0 tail re-reads.

    Name the CP entry (this is the classic "check flags with IRQs off before returning to user").
  - Add a discriminator: a syscall that returns with IF=1 while a hold delivers the IPI in the tail.
- **CP-86:** mark it OPEN again until both discriminators pass, and correct `:132` and `:3894ff`. Item 6's wording
  note: the swap publishes the flag before the counter increment, so say that in the comment and the bank.

## Rules

- Pitfall-bank table for every class this touches: CP-15/21/26/30/37/50/83/86, plus any new entries.
- Kill-switch: with `browser-runtime` off, `.text` and `.rodata` are identical (`NEO_BUILD_ID` pinned).
- `scripts/worker-close-gate.sh`. Send the PM the head SHA from `git ls-remote`.
- The review after merge is cross-family and required (SMP core): Jarvis re-checks items 2/3/7 and findings 1/2
  (GPT-6.1-Sol medium, narrow re-check).

```
WORKTREE: cloud session S4 (your own clone)
BRANCH:   claude/k1a0-review-fixes (section 0), then claude/ot61-c1-review-fixes
          git fetch origin && git checkout -B claude/k1a0-review-fixes origin/main

git show origin/main:Plans/DISPATCH-2026-10-27-review-fails-fix-s4.md
git show origin/main:Plans/RETURN-2026-10-27-k1a0-cap-graph-review-janus.md
git show origin/main:Plans/RETURN-2026-10-27-ot61-c1.2-tls-review-jarvis.md
git show origin/main:Plans/RETURN-2026-10-27-ot61-c1.3-review-jarvis.md
git show origin/main:Plans/RETURN-2026-10-27-ot61-c1.3b-review-jarvis.md
```

RAG isn't available in the cloud session. Cite code by `file:line`. Before ending, `git status --porcelain` and
`git log @{u}..` are both empty.
