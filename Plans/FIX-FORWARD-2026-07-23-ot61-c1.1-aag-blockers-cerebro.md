<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# FIX-FORWARD: OT-61 C1.1 AAG blockers → Cerebro

**Date:** 2026-07-23  
**From:** PM  
**To:** Cerebro  
**Priority:** Release-blocking; build immediately. The 4-shard gate remains token-controlled behind the already-authorized Jarvis gate.  
**Rejected candidate:** `d5e79fc7ce0f66ad8fb2e804b696b7c9a8c0d6e1`  
**Rejected candidate CI:** `https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/30060896551` — exact SHA, host/SPDX and all four QEMU shards green  
**Verdict:** **AAG BLOCK — DO NOT MERGE**

## Read first

```powershell
git fetch origin
git show origin/main:Plans/FIX-FORWARD-2026-07-23-ot61-c1.1-aag-blockers-cerebro.md
git show origin/main:Plans/DISPATCH-2026-07-22-ot61-c1.1-threads-futex-cerebro.md
git show origin/main:Plans/RATIFY-2026-07-22-c1-abi-amended-freeze.md
git show origin/main:docs/concurrency-pitfalls.md
```

The green gate is real, but incomplete. It proved the implemented scenarios; it did not prove the production feature composition, capacity-pressure wake path, per-thread stack reclamation, all-queue process fault-kill, or two frozen lifecycle contracts. Fix the candidate; do not weaken the freeze or the tests.

Use the semantic-search RAG at `http://127.0.0.1:8404` for unfamiliar ownership paths. Exact symbol searches remain direct-source work.

## AAG blockers — all required

### A. P0 — token wake can panic on a full ready ring

The scheduler has a 64-slot ready ring plus 16 token-parking slots. Parking moves an owned `Box<Thread>` out of the ready ring, so ordinary admissions can refill the freed ring slot. `wake_thread_token` then consumes the parked token before calling the panicking ready enqueue path. A valid futex wake, timeout, or join completion can therefore panic the kernel and lose the extracted waiter.

Evidence in the rejected SHA:

- `kernel/src/sched/scheduler.rs:57-62` — separate ready/token capacities
- `kernel/src/sched/scheduler.rs:561-575` — panicking enqueue
- `kernel/src/sched/scheduler.rs:1568-1590` — token park
- `kernel/src/sched/scheduler.rs:1657-1674` — consume token, then panic-capable enqueue

Required property:

- a wake must never panic, drop, duplicate, or strand a valid parked thread under ready-ring backpressure;
- reserve re-entry capacity or make enqueue fallible while retaining/restoring the token;
- report `Woken` and increment the futex wake count only after the waiter is actually queued exactly once;
- apply the same invariant to explicit futex wake, timer expiry, join completion, and every other token-wake caller.

Required discriminator: deterministically park a token waiter, fill its CPU ready ring to the admitted limit, fire the wake, then open capacity. The kernel stays live; the waiter remains owned exactly once and eventually resumes; no false wake count is reported. The rejected candidate must red by reaching its panic path.

### B. P0 — successful created-thread stacks leak until process exit

Created user stacks map fresh frames, but normal join/detach/reap only removes registry metadata. `unmap_user_range` is used on creation rollback, not successful per-thread teardown. A long-lived browser can sequentially create and reap beyond the concurrency cap while leaking every completed stack's mappings and frames. The existing final frame audit is vacuous for this defect because dropping the whole process `Pmap` cleans the evidence after process exit.

Evidence:

- `kernel/src/sched/process.rs:2237-2284` — allocation plus rollback-only unmap
- `kernel/src/sched/process.rs:527-540` — registry removal without stack unmap
- `kernel/src/sched/process.rs:2213-2228` — whole-`Pmap` teardown only on last process thread
- `kernel/src/sched/scheduler.rs:2068-2085` — off-stack reap handoff
- `kernel/src/mm/pmap.rs:432-500` — map/unmap frame behavior

Required property:

- retain created-stack ownership until the target is safely off both user and kernel stacks;
- unmap/free each created stack exactly once at the correct join/detach/zombie collection boundary;
- never unmap the initial process stack or a live thread's stack;
- preserve the frozen `thread_get_info` validity interval through exit/reap;
- keep CP-30: no owned `Arc`/`Box`/guard may survive a blocking syscall on a stack that force-zombify can free raw.

Required discriminator: warm page-table overhead, capture a same-process frame/mapping baseline, then run substantially more than `MAX_THREADS_PER_PROCESS` sequential cycles covering spawn→join, spawn→detach, and zombie→later-join/detach. Reap between batches. Before the parent process exits, every retired created stack translates absent and frame usage returns to the warmed baseline (or an explicitly proven fixed page-table bound). The rejected candidate must show linear growth.

### C. P0 — ring-3 fault-kill misses sibling ownership queues

`fault_kill_current_user` purges futex state and handles Ready/token-parked siblings, but IPC parks owned `Box<Thread>` values in separate rendezvous queues and process-wait has its own ownership path. A sibling left in one of those queues keeps the process live count nonzero, so fault teardown never completes and the parent may never receive exit 139. The green guard test covers futex-park plus join-park only.

Evidence:

- `kernel/src/sched/process.rs:2494-2518` — rejected fault-kill coverage
- `kernel/src/ipc/mod.rs:195,252,315,381` — IPC-owned parked threads
- `kernel/src/ipc/mod.rs:455-462` — separate IPC extraction path used elsewhere

Required property:

- a user #PF retires every sibling regardless of Ready, token-parked, futex-parked, IPC-parked, process-wait-parked, or any other `block_current_into*` ownership site;
- each queue loses the target exactly once and stale generation/token wakes remain harmless;
- exception-frame work stays bounded; heavy teardown remains deferred to safe off-stack reap;
- parent observes exit 139 and the system remains schedulable.

Required discriminator: a child leaves one sibling blocked in IPC and another in a process-wait queue, then another thread faults its guarded stack. The parent must receive 139 within a bounded deadline, all parked ownership must drain exactly once, and a pre-exit frame audit must be clean. The test must prove the IPC/process-wait states were actually reached.

### D. P1 — BrowserRuntime authority is not compositional

The rejected candidate installs `BrowserRuntime` only when `net-dial-cap`, compositor, and GPU-root features are all absent. A production-relevant `browser-runtime + net-dial-cap` composition therefore gives `neo-init` only NetDial; `SYS_THREAD_CREATE` fails `EPERM`.

Evidence:

- `kernel/src/main.rs:323-335` — NetDial-only inherited-cap arm
- `kernel/src/main.rs:359-380` — BrowserRuntime arm excludes every other root-cap feature

Required property:

- assemble root capabilities compositionally and deterministically;
- enabling `browser-runtime` installs `BrowserRuntime` alongside every other enabled root authority, with a documented stable handle order;
- default-off installs no BrowserRuntime authority;
- preserve fail-closed behavior and no userspace self-mint path; children receive it only by explicit delegation/inheritance.

Required discriminator: an exact combined-feature scenario enables `browser-runtime + net-dial-cap`, proves both authorities are present, successfully creates a thread, and preserves the NetDial lookup. A child without delegated BrowserRuntime must still receive `EPERM`. Include a combination-shape compile/KAT that prevents a return to mutually exclusive cfg arrays.

### E. P1 frozen contract — bootstrap must initialize bounds/errno state before app entry

The freeze requires a TLS-free child bootstrap sequence: record the created thread's bounds, establish the C1.2 TCB/key-block placeholder and per-thread errno seam, then call the application entry. The rejected `ThreadStart` retains only the callback/argument/errno field, and `thread_bootstrap` calls the app directly. Its QEMU bounds test hides the omission by querying `thread_get_info` manually inside the test callback.

Evidence:

- `userland/libneo/src/nthread.rs:18-34` — direct app entry without bootstrap bounds initialization
- `userland/neo-init/src/main.rs:2022-2038` — test performs the missing query itself

Required property: the TLS-free trampoline must query and record self stack bounds plus initialize the reserved TCB/key/errno seam before any application function runs. Do not add native TLS or re-derive the C1.2 ABI.

Required discriminator: application entry asserts that bootstrap-populated state already exists, contains the live RSP within usable bounds, and exposes an isolated per-thread errno cell. A negative control bypassing initialization must red.

### F. P1 frozen contract — join must use the futex done word

The original dispatch freezes join/detach/exit status on a per-thread done word waited through P1 futex. The rejected candidate instead stores a private kernel `joiner: ThreadWakeToken` and blocks through `try_block_current_with_token_and_guard`, creating a second join wait protocol.

Evidence:

- `Plans/DISPATCH-2026-07-22-ot61-c1.1-threads-futex-cerebro.md` — frozen done-word/futex requirement
- `kernel/src/sched/process.rs:2325-2388` — private join-token implementation

Required property: converge delayed join on the ratified per-thread done-word plus P1 futex wait/wake mechanism. Preserve single-joiner, detach, exact exit-code, target-reap, and lost-wakeup semantics without an independent join-token queue.

Required discriminator: a delayed-target join must exercise the futex wait/wake path non-vacuously, survive a wake-between-check-and-park race, return the exact exit code, and leave no private join-token ownership or stale waiter state.

## Pitfall bank

Add the new traps to `docs/concurrency-pitfalls.md` only with accurate EXPOSED→fixed status and falsifiable discriminators:

- ready-ring re-entry capacity after off-ring token parking;
- per-thread resource leaks hidden by whole-process teardown audits;
- process-kill completeness across every owning wait queue;
- feature-exclusive root-cap assembly that passes single-feature tests but fails the composed product.

Do not mark any entry fixed until the new exact-SHA gates prove it.

## Required close gate and RETURN

1. Rebase the repair onto the new `origin/main` containing this dispatch; show exact merge-base and own-stat. No stale-base diff.
2. Keep the frozen `0x44–0x4D` ABI, physical futex key, absolute deadline, wake-count semantics, default-off kill switch, no-self-mint cap model, CP-30 discipline, and Slice-6 prerequisite intact.
3. Add all six non-vacuous discriminators above. Extend the shard battery so the capacity, leak, all-queue fault-kill, composed-feature, bootstrap, and futex-join paths actually execute.
4. Run host tests, format, workspace clippy, blocking kernel clippy, SPDX, and `scripts/worker-close-gate.sh`.
5. Build now, but do not fire the 4-shard `workflow_dispatch` until the PM/PL issues a new exact-SHA runner token. The already-authorized Jarvis gate owns the next slot; fixed C1.1 is next, Oracle follows.
6. Push the exact repair SHA. Return the push CI URL, but remember push CI skips QEMU.
7. On token, fire all four shards on that exact SHA. Return the exact workflow URL and prove every new scenario executed, not merely appeared in the shard inventory.
8. Return: branch/SHA/base, own-stat, blocker→repair table, new pitfall entries, host/KAT evidence, close-gate output, exact-SHA push and 4-shard URLs, and deletable worktree.

No merge. The PM will repeat independent own-stat, exact-SHA CI verification, and the full AAG panel.
