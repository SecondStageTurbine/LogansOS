# ADR-004: IPC Channels — Anonymous Endpoints, Explicit Pass-at-Spawn, Sync Rendezvous

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-05-03 |
| **Authority** | Project Lead |
| **Supersedes** | — |
| **Superseded by** | — |
| **Related** | Bible Article 1.4 (POSIX disagreement) · Article 4.1 (monolithic IPC ethos) · Article 12.1 (capability discipline, Phase 2) · Article 12.2 (no ambient authority) · Article 13 (Phase 1.5 scope) · ADR-001 (monolithic kernel) · ADR-005 (ring-transition prologue — first sustained ring-3 test under blocking syscalls) · ADR-006 (interior mutability + Pmap::Drop chain this composes with) · ADR-007 (process termination — shares the `close_channel_peers` cleanup path with this ADR's "Process-exit cleanup" section) · ANSPEC-001 §3.7 (capability-handle pattern) |

---

## Context

Phase 1.5 ships "basic IPC" per Bible Article 13. Phase 2 introduces capability-based security per Bible Article 12.1, in which every kernel object is referenced by a per-process integer handle that the kernel maps to `(object, permissions)`. Phase 1.5b's IPC primitive must therefore be:

1. **Forward-compatible** with Phase 2's capability-handle pattern. Handles in 1.5b should *be* the same data type Phase 2 will extend with permission bits; no migration churn.
2. **Cheap.** Bible Article 4.1 cites microkernel IPC overhead as the reason NeoPlutarch is monolithic. The IPC primitive cannot itself become the overhead it argues against.
3. **Bible-coherent.** No ambient authority (Article 12.2). No POSIX-style inheritance defaults that 2008's `O_CLOEXEC` retrofit had to undo.

Three orthogonal design axes drive the implementation. Each is decided independently below. The combined choice is **A + A.2 + Sync rendezvous**.

## Decision

### Q1 — Topology: anonymous channel with two endpoints

`sys_chan_create() -> (h_a, h_b)` returns two handles (a packed `i64`) installed in the calling process's handle table. The handles are anonymous (no name, no global registry). The caller distributes them.

| Alternative | Rejected because |
|---|---|
| Per-process inbox keyed by pid | Anyone can send to any pid — the ambient-authority pattern Bible 12.2 explicitly rejects. Phase 2's capability-grant model would have to delete and replace this whole-cloth. Bible 1.3 violation. |
| Spawn-creates-fixed-channel-at-handle-0 | "Handle 0 is magic" is the same POSIX `stdin` pattern Bible 1.4 frees us to disagree with. Doesn't scale to multi-channel patterns Phase 2 will want (a process holding capabilities to several services simultaneously). |

The chosen shape — anonymous handles owned by the creating process, distributable on send / pass-at-spawn — *is already the Phase 2 capability shape*, minus the permission-bits-and-table-indirection machinery. Phase 2 extends `Handle` with permission bits; it does not replace handles with capabilities. That's the desired migration path.

### Q2 — Handle delivery: explicit pass-at-spawn with deterministic ordering

`sys_spawn` is extended to take a slice of handles to give the child:

```
sys_spawn(name, handles: &[Handle]) -> ProcessId
```

The child receives those handles at indices `0..handles.len()` in its own handle table — **the i-th handle in `handles` lands at child handle index `i`. This is part of the ABI, not a convention.** The child's startup code can write `let parent = Handle(0);` and be correct by construction.

| Alternative | Rejected because |
|---|---|
| POSIX-style inheritance flag (`sys_chan_inherit(h, true)` then `sys_spawn`) | Adds a syscall (`sys_chan_inherit`); displaces verbosity rather than eliminating it; relies on ambient state ("which handles got marked") at spawn time; Phase 2 capability-grant deletes and replaces this. POSIX itself retrofitted `O_CLOEXEC` in 2008 because "everything inherits unless you remember to mark it" was the wrong default. Linux's `posix_spawn` exists partly because POSIX inheritance proved error-prone. NeoPlutarch declines to repeat the experiment. |
| Named channels (`sys_chan_create("worker-control")`) | Introduces a global string namespace and rendezvous-by-name semantics that don't exist anywhere else in the kernel; defers the "who's allowed to open this name?" question until Phase 2 has to retrofit ACLs. Wrong layer. |

A.2 is already shaped like Phase 2 capability-grant: sender names recipient (the child being spawned), sender names cap (the handle), kernel transfers ownership/access. The Phase 2 migration turns `&[Handle]` into `&[Capability]` and the call site is unchanged structurally.

The **deterministic ordering** clause is what makes A.2 ergonomically equivalent to A.1 for the common case. The parent doesn't need to communicate handle indices to the child via argv or an early message — the index is structural.

### Q3 — Send semantics: synchronous rendezvous

`sys_send(handle, buf, len)` blocks until a matching `sys_recv(handle, buf, max_len)` arrives on the peer endpoint. No kernel buffering. Symmetric: either side can arrive first; the second arrival completes both. The kernel performs a checked user→user copy at rendezvous time.

| Alternative | Rejected because |
|---|---|
| Buffered (bounded ring) | Adds queue allocator, size limits, flow-control, back-pressure, kill-time cleanup of in-flight messages, fairness across multiple senders. Each is a design decision with security implications. The "common case" of "queue not full" lets you avoid the blocking-syscall path entirely — but landing the blocking-syscall machinery is the *point* of Phase 1.5b. Sync forces it to be tested under real load. |
| Buffered + zero-copy via shared pages | Solves the "data is large" half of Bible 4.1 — but Bible 4.1's "data is large and frequent" argument is about *inference data*, which goes through the AI subsystem with kernel-resident tensor objects (ANSPEC-001 §5), not through this IPC path. This primitive is for control-plane: capability passing, request/response, supervisor-to-worker coordination. Small messages, latency-sensitive. Sync is correct for this workload. |

A subtle but load-bearing semantic clarification: **send and receive are symmetric.** The endpoint state machine has `Idle`, `SendBlocked { … }`, `RecvBlocked { … }`, `Closed`. Either operation can arrive first; the second completes both. Wrappers must not assume a "sender goes first" ordering that the primitive does not enforce.

### Truncation policy

If the receiver's `max_len` is smaller than the sender's payload, the kernel truncates and returns `bytes_copied = max_len`. The receiver compares `bytes_copied` against `max_len` — if equal, truncation may have occurred and the caller can retry with a larger buffer if it cares. This is the smallest API that doesn't lose information; explicit `MSG_TRUNC`-style flags are deferred until a real caller needs them.

## Consequences

### Positive

- **Phase 2 migration is a lift, not a rewrite.** The handle table, the pass-at-spawn machinery, and the channel object all extend cleanly with permission bits and capability-grant syscalls.
- **Scheduler-block primitive lands and is tested.** Every future blocking syscall (futex, file I/O, inference deadlines, signal wait) consumes the same primitive. Phase 1.5b is the right phase to land it.
- **Bible-coherent.** No ambient authority; no POSIX inheritance defaults; no global namespaces.
- **First sustained test of ADR-005.** The blocking-syscall path interacts with the timer ring-3 prologue under sustained ring-3 load. If ADR-005's prologue/epilogue have a latent bug, this commit surfaces it.

### Negative

- **No capability passing yet.** A handle passed at spawn is the only inter-process handle delivery mechanism. A process cannot grant a handle to a *running* peer until Phase 2 capability-grant lands. Acceptable for Phase 1.5b's scope.
- **Synchronous-only blocks the sender on a slow receiver.** A misbehaving or paused peer halts forward progress. Phase 2's signal/kill machinery + per-channel timeouts mitigate; for Phase 1.5b's ping-pong control-plane shape, the constraint is acceptable.
- **`sys_spawn` ABI grows.** From 2 args to 4 (name_ptr, name_len, handles_ptr, handles_count). Within SysV-RDI/RSI/RDX/R10 capacity already wired by the syscall path; no second-syscall workaround needed.

### Neutral

- **Truncation as the receiver's responsibility.** Phase 1.5b callers are kernel-internal-or-handcrafted; they handle this trivially. Future libneo may add a `recv_message` wrapper that loops + grows the buffer.

## Implementation

This ADR is implemented by Phase 1.5b BUILD-3 landing alongside it.

### Kernel objects

```rust
// kernel/src/ipc/mod.rs
pub struct Channel {
    state: SpinMutex<[EndpointState; 2]>,
}

enum EndpointState {
    Idle,
    SendBlocked {
        buf_user_va: u64, len: usize,
        sender_pmap: Arc<Pmap>,        // for translate-at-rendezvous
        waiter: Box<Thread>,           // parked here for the duration
    },
    RecvBlocked {
        buf_user_va: u64, max_len: usize,
        receiver_pmap: Arc<Pmap>,
        waiter: Box<Thread>,
    },
    Closed,                            // peer process exited
}
```

Endpoint indices: side 0 and side 1. Each handle records `(Arc<Channel>, side)`. A send on side 0 matches a recv on side 1, and vice versa.

### Handle table

```rust
// kernel/src/sched/process.rs
pub struct Handle { channel: Arc<Channel>, side: u8 }
pub struct HandleTable { entries: Vec<Option<Handle>> }
// Process gains: handles: SpinMutex<HandleTable>.
```

Allocation strategy: lowest-free-index, so the first handle installed at spawn lands at index 0 (the deterministic-ordering ABI clause).

### Scheduler block primitive

```rust
// kernel/src/sched/scheduler.rs
pub fn block_current_into<F>(installer: F)
where F: FnOnce(Box<Thread>);
```

Atomically: pops next ready, swaps with current, marks prev `Blocked`, hands prev to `installer` (which moves the Box into a channel endpoint while still holding any caller-side locks), drops SCHED lock, calls `context_switch`. The caller wraps the whole thing in `without_interrupts` and holds the channel lock across the `installer` callback; the `installer` is the moment the channel's endpoint state transitions to `SendBlocked`/`RecvBlocked` with the parked Box.

(SMP retrofit point: Phase 2 will need the lock-drop-on-new-stack pattern via `ManuallyDrop` + asm release, so the matching peer on another CPU cannot complete the rendezvous before the blocker has finished switching stacks. Phase 1.5b is single-CPU; `without_interrupts` + immediate-drop suffices.)

#### Phase 1.5c BUILD-1 amendment: single-CPU handoff

BUILD-1's integration harness exposed that "single-CPU + `without_interrupts`"
was not enough to enforce the documented handoff semantics used by
`neo-worker`: the syscall that completed a rendezvous could queue the blocked
peer as Ready and still return to userspace first. The implementation now
front-queues the blocked peer and immediately yields, so on the single-CPU
Phase 1.5 line the peer is scheduled before the completing syscall returns.
Phase 2 SMP must replace this with an explicit barrier or direct-handoff
primitive; do not treat the current yield as SMP-safe.

BUILD-2 tightened the test discipline around this invariant: the handoff window
must not contain slow diagnostic syscalls before the peer reaches its next
blocking operation. A serial write there can be preempted by the timer, making
the peer visible as Ready and invalidating a kill-on-Blocked assertion. Tests
that need a deterministic `RecvBlocked` target must transition directly from
the rendezvous return into the blocking syscall.

BUILD-4 production boot tightened it again: even without serial in that window,
host/QEMU timing can deliver a stale PIT tick between the receiver's userspace
return and its follow-on blocking syscall. IPC front-wakes now grant the
receiver a bounded handoff quantum, cleared on the receiver's next syscall entry
or when it blocks. This is still a single-CPU Phase 1.5 mechanism, not an SMP
barrier; Phase 2 must replace it with explicit rendezvous barriers or a real
direct-handoff primitive.

### Process-exit cleanup

`sched::exit` (and `on_thread_reap` for the user-process-deregister path) walks the exiting process's handle table. For each handle:
1. Mark the **peer** endpoint state. If peer is `Idle`, transition to `Closed` (no thread to wake). If peer is `SendBlocked` or `RecvBlocked`, **yank the parked Box out**, reset the waiter's syscall return value to `-EPIPE`, mark waiter `Ready`, push to ready queue, then mark the endpoint `Closed`.
2. Drop the local handle (the `Arc<Channel>` refcount falls; if both sides are gone, the Channel itself drops).

A `debug_assert!(self.handles.is_empty())` after the cleanup loop guards the ordering invariant.

**Forced-kill shares this path.** ADR-007's `sys_kill` enters the same cleanup machinery via the shared `close_channel_peers(&drained)` helper (BUILD-5 simplify finding) — natural exit (`on_thread_reap`) and forced kill (`process::terminate`) both drain the handle table and signal channel peers identically. If you change the close_peer semantics here, update both callers; they MUST stay in lockstep. The "killed-recv" non-path (the killed process's own thread blocked in recv on its own channel doesn't return — it's gone, not woken) is the only asymmetry, and it's documented in ADR-007 §Implementation note 2.

### User→user copy

At rendezvous time, the matching syscall (the one that arrived second) holds the channel lock, sees the SendBlocked/RecvBlocked variant, and performs:

```
let src_phys = sender_pmap.translate(VirtAddr::new(send_buf))?;        // sender's VA
let dst_phys = receiver_pmap.translate(VirtAddr::new(recv_buf))?;      // receiver's VA
let n = min(send_len, recv_max);
let src_kern = phys_to_virt(src_phys).as_ptr::<u8>();
let dst_kern = phys_to_virt(dst_phys).as_mut_ptr::<u8>();
core::ptr::copy_nonoverlapping(src_kern, dst_kern, n);
```

Per-page translation is required for any copy that crosses a 4 KiB boundary. Phase 1.5b's small control messages typically fit in one page; the page-crossing loop is implemented but uncommon-cased.

If either translate returns `None`, the rendezvous aborts: the **first arrival** (the one that blocked) is woken with `-EFAULT`; the **second arrival** (which detected the fault) returns `-EFAULT` directly. No partial copies.

### Why ADR-005 cross-reference

This is the first kernel feature where ring-3 user threads sustainedly block in syscall context, get unblocked by another thread's syscall, and resume — repeatedly, under timer preemption. ADR-005's CS.RPL-branched timer prologue is exercised in every blocking iteration. If the prologue's GS handling or ring-transition has a latent edge case, this commit's smoke test surfaces it. Future Phase plans should treat blocking-syscall coverage as a required ADR-005 regression test.

## Verification (filled in as the BUILD-3 commit lands)

- [x] `sys_chan_create` returns two distinct handles in the calling process's table.
- [x] `sys_send` with no waiting peer parks the sender; `sys_recv` arriving second completes both with the correct bytes copied.
- [x] `sys_recv` with no waiting peer parks the receiver; `sys_send` arriving second completes both.
- [x] Truncation: `sys_recv(max=8)` against `sys_send(len=16)` returns 8, no kernel fault.
- [x] Process exits while peer is blocked → blocked peer wakes with `-EPIPE`, no deadlock.
- [x] Phase 1.3 ping/pong/hog smoke still passes (anti-regression).
- [x] Phase 1.5a Pmap::Drop walker still freed for both processes after IPC round-trip.
- [x] ADR-005 prologue exercised across many blocking iterations with no faults.

## Lessons

Three lessons surfaced during BUILD-3 → BUILD-5 that generalize beyond IPC. Future ADRs should follow the same `## Lessons` template; ADR-006 and ADR-007 may retroactively gain their own.

- **Chesterton's Fence on Arc lifetime ordering** (BUILD-3 `on_thread_reap`). The seemingly-redundant clone-cleanup-remove pattern in `on_thread_reap` is load-bearing: a one-pass remove-then-cleanup restructure caused a deadlock at `BTreeMap::remove` because the registry's `Arc` was the only remaining strong reference to the `Process`, and the rendezvous Drop chain became order-sensitive when that Arc transferred into a local. The lesson: when working code has a structure that looks redundant, find the test that proved it necessary BEFORE deleting it. /simplify catches local quality issues but cannot infer multi-Drop ordering invariants — that's plan-agent territory.

- **Type-system-as-discipline beats unsafe + SAFETY comments** (BUILD-4 `MutexGuard` parameter pattern). When a kernel function's `unsafe` precondition is purely "caller holds lock X", encode it as `&mut MutexGuard<…>` parameter type instead of `unsafe fn` + SAFETY prose. The MutexGuard cannot be constructed without taking the lock — the type system proves the precondition statically. Static proof is strictly stronger than reviewable prose; adopt this pattern by default for any new lock-held-precondition helper.

- **Shared cleanup paths prevent drift** (BUILD-5 `close_channel_peers` extraction). When two distinct entry points (natural exit `on_thread_reap` and forced kill `process::terminate`) need the same cleanup machinery, extract the shared helper at the moment the second caller appears — not later. Two parallel implementations of "drain handles, close peers" would inevitably drift as one path adds error handling or logging the other doesn't get; the bug surfaces months later as "kill leaks channels but exit doesn't, why?" /simplify caught this on the first review pass; future ADRs should preempt the duplication by extracting at the design stage.

---

**Decision recorded.** Per Bible Article 27, this ADR is amendable by superseding ADR.
