<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE: `reservation::guarded_clone` under force-kill (assessment only)

**Lane:** CP-66 follow-ups, item 5 (PM, after #76 merged as `8d3b146d`).
**Branch:** `claude/cp66-followups`.
**Outcome:** no fix in this build. The leak is real in shape but has no reachable mover outside test
builds. The guard is written down here for whichever build wires the mover into production.

## 1. The shape

`guarded_clone(slot)` mints an `Arc<Tensor>` from an arena slot. If a mover holds the tensor's
reservation (the slot is `Reserved`), it token-parks on the reservation table until the mover's commit
wakes it, then re-resolves (`kernel/src/ai/reservation.rs`, `guarded_clone`).

`guarded_clone` itself owns nothing across that park: the slot is borrowed, and the slot guard is
dropped before parking. Its **callers'** frames are the problem. Every tensor use goes through
`arena::mint`, and the syscall callers hold `Arc`s while they mint:
- `sys_infer_run_impl` holds the model `Arc` and the input tensors already minted, then mints the next;
- `tensor_lookup` serves `SYS_TENSOR_QUERY` / `_SHARE` / `_WRITE`;
- `Model::weight` / `exec` mint weights in the middle of a forward.

A token-parked user thread is visible to the force-kill token scan.
`zombify_parked_thread_by_pid` takes it raw, so everything its frames own leaks (CP-30). It does not
wedge: the mover's later wake finds a stale token and ignores it by design. The leak would be bounded
by what one syscall holds, which can be a whole model's last count.

CP-66's predicate does not cover it. `admission_section` is set only inside `run_scheduled`, after
the inputs are minted. B1c's `inference_lifecycle` covers the inventory route, whose lookups do park
here, but it is the generate / inventory marker, not a general one.

## 2. Reachability: none outside test builds

A park needs a `Reserved` slot, and only `reservation::try_migrate` makes one. Its callers:

| Caller | Built in |
|---|---|
| `residency_daemon` (demote, prefetch, rebalance) | only once `residency_daemon::start()` runs, and **nothing outside `test_harness.rs` calls it** (the function carries `#[allow(dead_code)]`, "feature-gated scenario consumer") |
| `arena::debug_migrate` | test seam (`#[allow(dead_code)]`, "race-gate probe seam") |

`exec_dispatch` only calls `residency_daemon::note_need` / `kick`, and `kick` on a never-started
daemon wakes no one. So in `product`, `desktop` and every default build, no slot is ever
`Reserved`, `guarded_clone` never parks, and the kill path cannot find a thread there.

## 3. The guard for when the mover ships

Whichever build starts the residency daemon outside a scenario (O1b's successor, or anything that
wires `try_migrate` into production) must close this first. The fix has the same shape as CP-66:
- **Mark the park.** Set a per-thread flag around the `block_current_with_token_and_guard` call in
  `guarded_clone`, and add it to `scheduler::kill_protected`, so neither kill scan zombifies a
  thread parked there.
- **Cancel it cooperatively.** Terminate must remove the dying pid's tokens from that tensor's
  reservation `waiters` and wake them. Each woken thread returns `None` (or a cancel result) to its
  minting caller, which unwinds.
- **Exit at the boundary.** Each syscall that can reach `mint` needs the torn-down check
  `sys_infer_run_syscall` already has. Today only the inference arms have one, so the tensor
  syscalls would need a general boundary check, which is the larger part of the work.

A simpler alternative, if the mover's hold is provably short and always ends in a commit: treat the
reservation park as unkillable (marked, no cancel). The waiter then wakes on commit and exits at the
boundary. This still needs the boundary check, and that window is what has to be proven bounded.

**Discriminator for that build:** have the mover hold a reservation, and let a stand-in caller park in
`SYS_TENSOR_QUERY` behind it. Terminate the caller and release the mover. Assert that the tensor's
`Arc` count returns to baseline, and that the caller exits without running the rest of the syscall.
The control is today's rule, which leaks one tensor count.

## 4. Why not fix it now

"Fix only if trivial" (PM). The marker-plus-cancel is the trivial half. The boundary check across
every minting syscall is not, and no production path can exercise it yet. A guard with no reachable
trigger would also leave its discriminator testing a configuration that never ships. Banked here
instead, with the trigger condition named so the mover's build picks it up.
