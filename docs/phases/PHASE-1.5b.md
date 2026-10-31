# Phase 1.5b — Userspace Foundation: IPC, Heap, Termination

| | |
|---|---|
| **Status** | Complete |
| **Started** | 2026-04 (week of) |
| **Closed** | 2026-05-03 |
| **ADRs landed** | 004 (IPC channels) · 005 (ring-transition prologue) · 006 (heap + mmap region) · 007 (process termination via ProcessHandle) |
| **Commits** | `67a46be` (1.5a) · `939d446` (B3 IPC) · `7825b23` (B4 heap) · `3d7f0e8` (B5 sys_kill) · `e9675c4` (B6 e2e + this doc) |
| **Follow-on** | Phase 2 — capability hardening + SMP + buddy/slab allocators |

---

## What Phase 1.5b Built

After Phase 1.5b, NEO can spawn multiple userspace processes, communicate between them via capability-mediated IPC channels (synchronous rendezvous, anonymous handles, deterministic-ordering inheritance), allocate dynamic memory in userspace via an mmap-style region primitive backed by a per-process linked-list allocator, and forcibly terminate child processes via ProcessHandle capabilities returned at spawn time. The kernel reclaims all process resources deterministically via the Drop chain — verified by an e2e leak detector that asserts the live frame count returns to baseline after every user process reaps. Both natural exit and forced kill share the same cleanup machinery via the `close_channel_peers` helper extracted in BUILD-5.

Phase 1.5b is the first phase where multi-process userspace runs end-to-end with capability discipline preserved across the IPC + spawn + kill surface — the foundation Phase 2's capability hardening, SMP, and buddy/slab allocators all extend.

---

## Original Criteria (from Phase 1.5 plan)

The Phase 1.5 plan distinguished 1.5a (multi-process spawn + Pmap::Drop) from 1.5b (the userspace primitives proper). The 1.5b ISC at planning time:

- **ISC-1.5b.1** — Synchronous IPC primitive with capability-shaped handles. (BUILD-3, ADR-004)
- **ISC-1.5b.2** — Userspace dynamic memory via a kernel growth primitive. (BUILD-4, ADR-006)
- **ISC-1.5b.3** — Process termination beyond suicide-only `sys_exit`. (BUILD-5, ADR-007)
- **ISC-1.5b.4** — End-to-end leak audit covering all of the above. (BUILD-6)
- **Anti-criterion ISC-1.5b.A1** — No POSIX-style ambient authority. PIDs are not authority-bearing.
- **Anti-criterion ISC-1.5b.A2** — No retrofit cost when Phase 2 capability discipline lands. Handles in 1.5b ARE the Phase 2 capability shape.

---

## Verification Log

Per-criterion pass/fail with commit references and trace evidence.

### ISC-1.5b.1 — IPC primitive ✓ PASS (commit `939d446`)

Boot trace shows clean ping/pong across two processes:
- `sys_chan_create` returns two handles in the calling process's table.
- `sys_send` blocks when no peer; `sys_recv` rendezvous wakes both with correct bytes.
- Truncation: send(16) vs recv(max=8) returns 8, no fault.
- Process exits while peer is blocked → blocked peer wakes with -EPIPE.
- Cross-process buffer copy via `Pmap::translate` (lock-free read path).
- ADR-004 §Q1 + Q2 + Q3 implemented as decided; ABI shape is forward-compatible with Phase 2 capability bits.

Course-correction: BUILD-3 phantom-win simplify finding revealed clone-cleanup-remove ordering in `on_thread_reap` is load-bearing (deadlock if restructured to remove-first). Documented inline + saved as a reusable lesson (`feedback_kernel_simplify_discipline.md` memory entry).

### ISC-1.5b.2 — Userspace heap ✓ PASS (commit `7825b23`)

Boot trace shows heap-built `Vec<u8>` IPC payloads round-tripping cleanly:
- `sys_map_region(size)` returns kernel-chosen user VA at the per-Pmap mmap cursor (`USER_HEAP_BASE = 0x4000_0000_0000`).
- `LibneoHeap` (libneo's `GlobalAlloc`) bootstraps lazily on first allocation (256 KiB initial region) and extends via `Heap::extend` on growth.
- `Pmap` interior mutability via `Mutex<PmapState>`; `translate(&self)` stays lock-free per ADR-006 §Q4 — verified by reading the implementation, NOT by trusting prose.
- Per-process frame reclaim at exit: 108 frames + 1 PML4 freed per process, deterministically.

Course-correction: typed-`MutexGuard`-parameter pattern surfaced via simplify (BUILD-4 Quality #2). Replaces `unsafe fn map_user_locked` + SAFETY-comment with `&mut MutexGuard<PmapState>` parameter — type-system proof of lock-held precondition. Saved as a reusable lesson (`feedback_typed_guard_pattern.md` memory entry); ADR-004 § Lessons cross-references.

### ISC-1.5b.3 — Process termination ✓ PASS (commit `3d7f0e8`)

Boot trace shows `sys_kill(ProcessHandle)` cleanly terminating the target:
- `sys_spawn` widened to return `(pid, ProcessHandle)` packed in i64; libneo `Result<(u32, ProcessHandle), i64>`.
- `Handle` struct converted to enum (`Channel { … } | Process { target }`); EBADF gating on wrong variant via `ipc_lookup` and `process_handle_lookup`.
- `terminate(pid)` follows registry-removed-FIRST ordering with `debug_assert!` per ADR-007 §Implementation note 3.
- Self-kill guard returns -EINVAL with explicit check at `sys_kill_impl` (Phase 4 reachability tagged via `PHASE-4-FOLLOWUP` comment in `process.rs`).
- Forced kill shares the cleanup path with natural exit via `close_channel_peers(&drained)`.

Course-correction: simplify pass extracted `close_channel_peers` and `process_handle_lookup` helpers; trimmed task-narration comment prefixes. Lesson: shared cleanup paths prevent drift — captured in ADR-004 § Lessons.

### ISC-1.5b.4 — End-to-end leak audit ✓ PASS (commit `e9675c4`)

Single boot exercises the full surface in one trace:
- spawn → IPC happy round-trip (heap-built payloads) → `go-block` synchronization → kill-on-Blocked → EINVAL → unblock → kill-on-Ready → 0 → double-kill → ESRCH → exit
- `mm::frame_audit` snapshots at each phase boundary (`baseline-pre-spawn`, `post-spawn`, `post-kill`, `post-natural-exit`, `final-post-reap`) emit absolute used + delta-since-previous.
- `debug_assert!` at end asserts `final == baseline`. Trace observed: `baseline=1026 final=1026 (delta=+0)`. **Leak audit clean.**

### Anti-criterion ISC-1.5b.A1 — No ambient authority ✓ PASS

PIDs are not authority-bearing. `sys_kill` requires a `ProcessHandle` capability returned by `sys_spawn`, NOT a pid lookup. POSIX `sys_kill(pid, signo)` was rejected in ADR-007 §Q1 with rationale.

### Anti-criterion ISC-1.5b.A2 — No Phase 2 retrofit cost ✓ PASS

Handles ARE the Phase 2 capability shape. `Handle` enum (Channel + Process variants) is the Phase 2 extension point — new variants (Tensor, Model, InferenceTicket, File per ANSPEC-001 §3.7) slot in as siblings, NOT as parallel handle tables. EBADF-on-wrong-variant gating in `ipc_lookup` / `process_handle_lookup` is what makes the enum useful — Phase 2 inherits it for free. Documented inline at the enum definition.

---

## Course Corrections (deviations from original plan)

- **BUILD-2 surfaced ADR-005** — the ring-transition timer prologue. Not in the original Phase 1.5 plan; surfaced when sustained user-process timer preemption was first attempted and CS.RPL handling was incorrect. Result: ADR-005 lands in Phase 1.5a, becomes load-bearing for every Phase 1.5b BUILD that involves blocking syscalls.
- **BUILD-3 phantom-win** — clone-cleanup-remove in `on_thread_reap` reverted from a "simpler" remove-first restructure that deadlocked `BTreeMap::remove`. Saved as the kernel-engineering Chesterton's Fence lesson (memory: `feedback_kernel_simplify_discipline.md`).
- **BUILD-4 typed `MutexGuard` parameter** — surfaced via simplify; replaces `unsafe fn` + SAFETY prose with type-system-proven precondition. Saved as a reusable kernel pattern (memory: `feedback_typed_guard_pattern.md`).
- **BUILD-5 `Handle` enum refactor** — should arguably have happened in BUILD-3 when channel handles were first introduced; the trigger was BUILD-5 needing a sibling Process variant. Deviation acknowledged; the Phase 2 capability extension point is documented at the enum definition so it doesn't get flattened back.
- **BUILD-5 ESRCH semantics surfaced** — original BUILD-5 spec had double-kill returning EINVAL. BUILD-6 review identified that as misleading (POSIX-shaped wrappers will assume ESRCH); ESRCH added at the standard POSIX number (-3) and `terminate()` updated. Documented in ADR-007 §Implementation note 6.
- **BUILD-6 e2e smoke design** — ping-pong-only worker (BUILD-5) extended to two-channel synchronization pattern so the kill-on-Blocked test is deterministic on single-CPU rather than racy.

---

## Anti-criteria (deliberate non-deliveries — Phase 2 work)

- **Blocked-thread kill** — `sys_kill` returns EINVAL when target is parked inside an `EndpointState`. Lifting requires walking every channel handle the target holds and yanking the parked `Box<Thread>` out of the matching variant. Phase 2 SMP work has the right hooks; defer until then.
- **`KillResult` enum** — `kill_thread_by_pid` returns bool today, collapsing two distinct future cases (NotFound vs Blocked). Defer enum introduction until Phase 2 lifts the Blocked-only restriction; cheaper to refactor one caller then than to introduce the enum prematurely.
- **Cooperative shutdown / SIGTERM-equivalent** — ADR-007 §Q2 deferred to Phase 4 service supervision when there's an actual use case. Today's `sys_kill` is forced-only.
- **`sys_wait(handle)`** — parent learns child exited via the handle's existence (today) or via Phase 2 `sys_wait` over a ProcessHandle (tomorrow). No use case in 1.5b.
- **Self-kill via passed handle test** — tagged `PHASE-4-FOLLOWUP: cap-passing-over-IPC required to construct this case` in `kernel/src/sched/process.rs`. The guard at `sys_kill_impl` is correct today; the runtime test becomes constructable when Phase 4 capability-grant IPC lands.
- **PID recycling** — `NEXT_PID` is monotonic in 1.5b; stale `ProcessHandle` cannot accidentally hit a freshly-spawned process. Phase 2+ may add recycling at which point handle revocation becomes load-bearing (ADR-007 §Implementation note 6).
- **`HandleTable::drain` Vec → iterator** — pre-existing helper, micro-cleanup deferred.
- **`libneo::print()` helper** — wraps the repeated `sys_write(1, MSG.as_ptr(), MSG.len())` pattern; deferred to Phase 1.6 cleanup pass.
- **Inner `without_interrupts` wrap removal** in `kill_thread_by_pid` — convention is "wrap your critical section"; cost is one instruction pair on a slow path. Defer.

---

## Lessons applicable to future phases

- **Pace discipline.** Phase 1.5b was built in approximately three days at high cadence with strong AI-assisted review. The pace was sustainable because every BUILD ran a /simplify gate, premortem, and boot verification before commit. Future phases should preserve this loop; speed without the loop is the failure mode. Phase 2 specifically needs a longer THINK stage — the capability system has design decisions that calcify worse than IPC's did, and SMP changes the threading model assumptions in nearly every file written so far. That's correct; resist the urge to compress THINK to match 1.5b's pace.

- **Apply simplify findings INDIVIDUALLY with boot tests between high-risk refactors.** Worked exactly as intended on BUILD-4 and BUILD-5; the discipline catches one regression per refactor before it compounds with the next. The opposite — batching all simplify findings into one commit — would have masked which change broke what.

- **Chesterton's Fence applies to working code's "redundant" structure.** BUILD-3's clone-cleanup-remove ordering looked like simplification candy; it was load-bearing. Before deleting anything that "looks redundant" in working code, find the test that proved it necessary. /simplify catches local quality issues but cannot infer multi-Drop ordering invariants; that's plan-agent territory. The split is real and useful: simplify for shape, plan-agent for ordering invariants.

- **Type-system-as-discipline beats `unsafe` + SAFETY comments.** When a function's `unsafe` precondition is purely "caller holds lock X", the typed `MutexGuard` parameter is strictly stronger than reviewable prose. Adopt by default for any new lock-held-precondition helper. Future locking work (Phase 5 tensor-context lock, Phase 2 SMP per-CPU primitives) should default to this pattern unless there's a specific reason not to.

- **Shared cleanup paths prevent drift.** When two distinct entry points need the same cleanup machinery, extract the shared helper at the moment the second caller appears — not later. The two-implementations-drifting bug shows up months later as "X leaks but Y doesn't, why?" Caught early via simplify; preempt at the design stage when possible.

- **Single-CPU rendezvous semantics are deterministic — exploit them.** BUILD-6's kill-on-Blocked test uses the property that on single-CPU, when a sender returns from `sys_send` after a rendezvous, the receiver has already woken AND continued to its next blocking syscall (otherwise the scheduler would not have switched back). This makes "is the peer Blocked yet?" answerable without explicit synchronization primitives. Phase 2 SMP breaks this; tests that depend on it will need explicit barriers (or rewrite to be timing-independent).

---

## Phase 2 transition notes

After BUILD-6 lands, **take a beat before Phase 2**. Phase 2 is bigger in scope than 1.5b (capability system + SMP + buddy/slab allocators is three substantial subsystems, any one of which is larger than all of 1.5b's BUILDs combined). The discipline that made 1.5b work in three days will need to flex differently for a phase that probably can't be done in three days regardless of pace.

Specifically:
- **Capability system design conversation BEFORE BUILD-1.** The capability shape calcifies worse than IPC's did; permission bits, revocation semantics, handle-passing-over-IPC all interact. Get the ADR right before code lands.
- **SMP changes the threading model assumptions in nearly every file written so far.** `without_interrupts` becomes insufficient on multi-CPU; lock-ordering becomes load-bearing across modules; the BUILD-3 phantom-win lesson about Arc lifetime ordering becomes much more important. The retrofit isn't a single ADR — it's a cross-cutting concern that touches `sched`, `mm`, `ipc`, `loader`.
- **Buddy/slab allocator** is the most self-contained of the three; could land first as a learning piece for the bigger items.

The Phase 1.5b ISC criteria + this verification log + the four ADRs are the artifact future-Phase-2-you reads to remember what was assumed. Keep this template; populate it for every future phase per Bible Article 18.2.
