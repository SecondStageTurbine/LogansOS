# PM-DECISION: Phase 3 BUILD-1 1B scheduler lifetime blocker

| | |
|---|---|
| **Date** | 2026-05-12 |
| **Author** | PM (NeoPlutarch Phase 3 cadence) |
| **Type** | PM decision; consumes Jarvis 1A/1B stop report |
| **Inputs** | `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1A-VERIFICATION-2026-05-12.md`; `Plans/FAILURE-2026-05-12-phase-3-build-1-jarvis.md`; commits `982b16d`, `7453f69` |

---

## 1. Decision

Jarvis was correct to stop before sub-commit 1B.

The current scheduler parks blocked threads by moving an owning `Box<Thread>` into caller-owned wait structures. That works for IPC, where the endpoint owns the blocked waiter and process-exit cleanup can extract and zombify it. It does not satisfy the Phase 3 storage requirement that I/O completion hold only non-owning wake authority over the issuing thread.

PM ratifies the blocker and selects the scheduler-owned weak-token design.

Do not migrate the whole scheduler to `Arc<Thread>` for BUILD-1. That is larger than the storage unblock, touches current/ready/zombie queues, and risks reopening Phase 2 scheduler closure.

Instead, introduce a scheduler-owned parked-thread table plus a non-owning generation-checked wake token. This is the concrete NeoPlutarch equivalent of ADR-021's `Weak<Thread>` intent.

---

## 2. 1A status

Commit `982b16d` is accepted as the BUILD-1 1A scaffold commit:

- block substrate;
- `Arc<DmaBuffer>` identity-DMA abstraction;
- PCI storage walker;
- modern virtio-blk probe skeleton;
- QEMU raw disk scenario plumbing;
- `phase3_pci_virtio_blk_probe_smoke`.

Verification debt remains because local QEMU gates were blocked by missing Limine ISO tooling:

- `cargo neo-test phase3_pci_virtio_blk_probe_smoke`: BLOCKED (`/home/neo/limine` / `LIMINE_DIR`);
- `cargo neo-test all`: BLOCKED for same reason;
- `cargo fmt --check`: BLOCKED, rustfmt absent for pinned nightly.

This does not close BUILD-1. It is acceptable as a build-passing scaffold with environment-blocked runtime verification.

---

## 3. Concrete unblock

Add a scheduler substrate before resuming virtio-blk I/O:

```rust
pub struct ThreadWakeToken {
    cpu_id: u32,
    slot: u32,
    generation: u32,
}
```

Names are Jarvis's discretion; semantics are not.

Required semantics:

- The token is non-owning. Holding it must not keep a `Thread` alive.
- The scheduler owns parked `Box<Thread>` values in a scheduler-owned table or per-CPU table.
- Parking is atomic with release of the caller's `SwitchMutexGuard`: install blocked thread in scheduler-owned parked table, write the token into the caller's rendezvous state, release caller guard and scheduler guard, then context-switch.
- Completion uses the token to try to extract and wake the parked thread.
- If process exit or kill has already removed/zombified the parked thread, token wake fails cleanly and storage completion skips wake.
- Token generation prevents stale wake after slot reuse.
- DMA buffers remain owned by the storage rendezvous through `Arc<DmaBuffer>`; the scheduler table owns only the parked thread.

This preserves the load-bearing Cerebro invariant: storage completion has weak wake authority and cannot extend process/thread lifetime.

---

## 4. ADR-021 refinement

ADR-021 is amended from a literal `Option<Weak<Thread>>` sketch to a weak scheduler wait-token sketch.

The previous `Weak<Thread>` wording was accepted as a semantic requirement, not a mandate to force the current Box-based scheduler through a full Arc migration during BUILD-1.

The acceptance gate remains:

- abandoned-I/O test proves storage completion does not leak DMA buffers and does not keep an exited thread alive.

---

## 5. Next cadence

File a focused Jarvis dispatch:

1. Sub-commit 1B0: scheduler-owned weak wait-token substrate.
2. Verification: token wake, token abandon, stale-token generation, existing IPC/cap/SMP regressions.
3. Resume original 1B virtio-blk read/write work only after 1B0 passes.

No new Cerebro dispatch is required. This is an implementation-surface correction inside the ADR-021 frame.

---

**End of PM-DECISION.**
