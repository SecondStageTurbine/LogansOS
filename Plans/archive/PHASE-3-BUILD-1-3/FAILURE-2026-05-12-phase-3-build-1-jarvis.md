# FAILURE - Phase 3 BUILD-1 Sub-Commit 1B Hard Gate

Date: 2026-05-12  
Lane: Jarvis / Codex  
Branch: `codex/phase-3-build-1-jarvis`

## Status

STOPPED before implementing sub-commit 1B.

The 1A block substrate / DMA / PCI probe scaffold is implemented and builds, but the 1B virtio-blk read/write path cannot be implemented honestly against the current scheduler lifetime model while satisfying the dispatch hard gate:

```rust
struct IoRendezvous {
    done: AtomicBool,
    waiting_thread: Option<Weak<Thread>>,
    dma_buffers: Vec<Arc<DmaBuffer>>,
    result: IoResult,
}
```

## Triggered stop condition

The current scheduler has an atomic release-on-switch primitive for a `SwitchMutexGuard`, but it parks blocked threads by moving an owning `Box<Thread>` into the caller-managed wait structure:

- `kernel/src/sched/thread.rs`: `Thread::new`, `Thread::new_user`, and `Thread::adopt_current` allocate slab-backed `Box<Thread>`.
- `kernel/src/sched/scheduler.rs`: ready/current/zombie queues are `Box<Thread>`.
- `kernel/src/sched/scheduler.rs`: `block_current_into_with_guard` atomically releases the caller guard and scheduler guard, but its installer receives `Box<Thread>`.
- `kernel/src/sched/scheduler.rs`: wake APIs require an owning `Box<Thread>`.

That primitive is correct for the existing IPC rendezvous shape, but it is not the 1B storage shape. A storage `IoRendezvous` that stores only `Option<Weak<Thread>>` has no place to put the parked `Box<Thread>`, and a storage `IoRendezvous` that also owns the `Box<Thread>` would keep the thread alive until device completion, violating the abandoned-I/O lifetime gate.

## Why not patch around it

The obvious workaround would be to store `Box<Thread>` in the storage rendezvous and separately add a nominal weak-looking field. That would be dishonest: process exit would not be able to abandon the I/O without the completion object retaining the thread.

The other workaround is to migrate `Thread` to `Arc<Thread>` so `alloc::sync::Weak<Thread>` can be real. That is a scheduler-wide representation change touching current/ready/zombie queues, context-switch mutation of `rsp`, blocked return delivery, process reap, and kill/exit handling. It is larger than sub-commit 1B and would risk the Phase 2 scheduler closure gates if done inline.

## Smallest viable unblock

One of these scheduler substrates is needed before 1B can proceed:

1. A real `Arc<Thread>` scheduler model with interior mutability for mutable scheduler fields and wake APIs that accept upgraded thread references.
2. A scheduler-owned parked-thread table plus a real weak/liveness handle whose type and semantics are ratified as equivalent to `Weak<Thread>`.

Option 1 matches the dispatch text literally. Option 2 needs PM/ADR approval because the dispatch explicitly names `Option<Weak<Thread>>`.

## Verification state at stop

- `cargo k`: PASS.
- `cargo k --no-default-features`: PASS.
- `cargo neo-test phase3_pci_virtio_blk_probe_smoke`: BLOCKED by missing host Limine directory.
- `cargo neo-test all`: BLOCKED by missing host Limine directory.
- `cargo fmt --check`: BLOCKED because `rustfmt` is not installed for the pinned nightly.

## PM decision needed

Decide whether Jarvis should first land a scheduler lifetime sub-commit that migrates `Thread` to an Arc/Weak-capable model, or whether ADR-021 should permit a scheduler-owned parked-thread table with a weak liveness token as the storage rendezvous target.
