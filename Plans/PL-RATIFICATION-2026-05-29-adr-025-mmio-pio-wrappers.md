# PL-RATIFICATION: ADR-025 adopted (typed MMIO/PIO wrappers)

**Date:** 2026-05-29
**From:** PL (JC)
**To:** Project record

---

## Ratification

ADR-025 (Typed volatile MMIO and port-I/O wrappers for driver register access) is RATIFIED and adopted as `docs/ADR-025-mmio-pio-typed-wrappers.md`. The candidate in `Plans/` is removed (git rename to docs/).

## Scope of what was ratified

- Decision direction: typed volatile `Mmio<T>` (RO/WO variants) for memory-mapped registers; width-typed `Pio8/16/32` for x86 port I/O. Concentrates hardware-boundary `unsafe` into a small audited core (Bible 4.2; OSTD-style discipline).
- Explicit non-goal recorded and binding: these wrappers are NOT a synchronization primitive — volatile governs single-CPU/device access only and emits no cross-CPU barrier. Cross-CPU sync remains atomics/`Ordering` + `IrqSafeMutex` (ADR-019).
- Implementation is a forward-marker: incremental, new drivers first (Phase 6 accel/GPU), opportunistic refactor of virtio-net/AHCI; no big-bang rewrite. CI lint deferred.

## Open questions (implementation-time, non-blocking)

Q1 module home (lean `hal/`), Q2 derive-macro timing (lean hand-written first), Q3 CI lint timing (lean follow-up), Q4 per-device fence policy. These are resolved at implementation, not preconditions of adoption.

## Bible touchpoints

No amendment required for adoption. A future Article 9 clause MAY name typed register-access wrappers as the canonical hardware-access primitive; that would be a separate amendment if pursued.

---

**ADR-025 is binding project decision as of 2026-05-29.**