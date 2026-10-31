# ADR Proposal: Owner metadata on lock primitives as production-stable diagnostic

**Date:** 2026-05-10
**Author:** PM (drafting)
**Status:** Proposal. NOT ratified. Per Bible Article 27, amendments require project-lead approval; this draft sits in Plans/ awaiting that decision.
**Tentative ADR number on ratification:** ADR-020 (next available after ADR-019; verify at ratification time).
**Related:** ADR-018 and ADR-019 proposals (companion). This proposal is independent: it could ratify with or without ADR-018/019.
**Bible articles potentially affected:** None directly. May add a small note to Article 15 (Code Standards) or Article 16 (Testing and Verification) referencing the diagnostic discipline.

---

## Rationale

SC-3B's Phase D investigation added ownership-metadata instrumentation to `SwitchMutex` to capture the holder identity at NMI fire time. The instrumentation (cpu_id, thread_id, acquisition_site, seqlock-protected reads) was the key diagnostic signal for naming the Hpw mechanism — without it, the investigation would likely have required additional passes.

The instrumentation was reverted at Sub-step 5c (commit e2b3f4f) per the discipline that gate-passing code does not retain instrumentation. But the diagnostic value was real, and the cost (12 bytes per lock plus cache-line considerations) is small enough that production-stable owner metadata is a defensible engineering trade-off.

This proposal: ship owner metadata as a feature-gated (or always-on lightweight) field on `SwitchMutex` and possibly other lock types, with seqlock-protected reads from interrupt context for NMI-handler use.

## Decision context

### Current state

`SwitchMutex` (and `spin::Mutex`) carry no ownership information. When a deadlock or hang occurs, the NMI handler can observe lock state (locked/unlocked) but cannot identify which thread/CPU holds the lock or where the lock was acquired. Diagnostic instrumentation must be added ad-hoc for each investigation.

### Proposed state

`SwitchMutex` (and possibly `IrqSafeMutex` if ADR-019 ratifies) carries:

```rust
struct LockOwner {
    cpu: u32,                  // CPU id of the holder; u32::MAX if unowned
    thread_id: u32,            // thread id of the holder; u32::MAX if unowned or unknown
    acquisition_site: u32,     // hash of (file, line) of the acquisition; 0 if unowned
    seq: AtomicU32,            // seqlock counter for consistent reads from interrupt context
}
```

Updated atomically with lock state via seqlock discipline:
- Writer (the thread acquiring or releasing the lock) increments `seq` to an odd value before updating cpu/thread_id/acquisition_site, then increments to an even value after.
- Reader (typically NMI handler) reads `seq`, then the fields, then `seq` again; if `seq` changed or is odd, retry or treat as "inconsistent."

Total cost per lock: 16 bytes (3 × u32 + 1 × AtomicU32) plus possible cache-line padding to avoid false sharing with the lock itself.

### Feature gate semantics

Two configurations:

- **`kernel-diagnostic` cfg enabled:** Owner metadata is always present, always updated, always readable.
- **`kernel-diagnostic` cfg disabled:** Owner metadata is absent (size 0; field is `()` or behind a `cfg`-gated wrapper); no cost.

Default for development builds: `kernel-diagnostic` enabled.
Default for release builds: `kernel-diagnostic` enabled (the small cost is worth the always-on diagnostic).
Default for size-optimized embedded builds (future): `kernel-diagnostic` disabled.

The feature gate is a build-time choice, not a runtime one. Once chosen at build, the metadata is either present or absent for the entire kernel image.

## Concrete proposal

### API sketch

```rust
// kernel/src/sync/lock_owner.rs (new file)

#[cfg(feature = "kernel-diagnostic")]
pub struct LockOwner {
    cpu: AtomicU32,
    thread_id: AtomicU32,
    acquisition_site: AtomicU32,
    seq: AtomicU32,
}

#[cfg(not(feature = "kernel-diagnostic"))]
pub struct LockOwner;  // ZST

impl LockOwner {
    pub fn acquire(&self, cpu: u32, thread_id: u32, site: u32) {
        #[cfg(feature = "kernel-diagnostic")]
        {
            self.seq.fetch_add(1, Ordering::Release);  // odd
            self.cpu.store(cpu, Ordering::Relaxed);
            self.thread_id.store(thread_id, Ordering::Relaxed);
            self.acquisition_site.store(site, Ordering::Relaxed);
            self.seq.fetch_add(1, Ordering::Release);  // even
        }
    }

    pub fn release(&self) {
        #[cfg(feature = "kernel-diagnostic")]
        {
            self.seq.fetch_add(1, Ordering::Release);  // odd
            self.cpu.store(u32::MAX, Ordering::Relaxed);
            // ... clear other fields ...
            self.seq.fetch_add(1, Ordering::Release);  // even
        }
    }

    /// NMI-safe read. Returns Some(snapshot) if a consistent read was achieved,
    /// or None if the lock state was being updated during the read.
    pub fn read_consistent(&self) -> Option<LockOwnerSnapshot> {
        #[cfg(feature = "kernel-diagnostic")]
        {
            let seq1 = self.seq.load(Ordering::Acquire);
            if seq1 % 2 != 0 { return None; }
            let cpu = self.cpu.load(Ordering::Relaxed);
            let thread_id = self.thread_id.load(Ordering::Relaxed);
            let site = self.acquisition_site.load(Ordering::Relaxed);
            let seq2 = self.seq.load(Ordering::Acquire);
            if seq1 != seq2 { return None; }
            Some(LockOwnerSnapshot { cpu, thread_id, site })
        }
        #[cfg(not(feature = "kernel-diagnostic"))]
        { None }
    }
}
```

### Acquisition site encoding

`acquisition_site` is a u32 hash. Options:
- Hash of (file path, line number) — computed at compile time via a macro.
- Index into a static table of acquisition sites — also macro-generated.
- Symbol-table offset — requires DWARF or similar at NMI handler time, more complex.

PM lean: macro-generated hash of (file, line). Cheap to compute (compile-time constant), no runtime table lookup needed in NMI handler.

### Integration with SwitchMutex / IrqSafeMutex

Each `SwitchMutex` instance gains a `LockOwner` field. `lock()` calls `owner.acquire(current_cpu, current_thread_id, site_hash)`. Guard drop calls `owner.release()`. NMI handler reads `owner.read_consistent()`.

If ADR-019 ratifies, `IrqSafeMutex` also gains the field. The two share the `LockOwner` implementation.

## Cost-benefit summary

**Cost:**
- 16 bytes per lock instance plus possible cache-line padding (rounded up: probably 32-64 bytes effective per lock).
- Two atomic increments on every lock acquisition and release (small; not in the lock's critical-section cost).
- Macro for site-hash generation.

**Benefit:**
- Future similar bugs (Hpw, scheduler deadlocks, etc.) are diagnosable from the NMI handler in real time — no need for ad-hoc instrumentation passes.
- The 36-hour Phase A-through-D investigation arc that SC-3B required becomes ~6 hours when the diagnostic is already present.
- Tooling can be built on top: post-hang lockmap dumps, periodic lock-state telemetry, etc.

## Open questions for ratification

- **Q1:** Feature-gate or always-on? PM lean: always-on for development and release builds; cfg-gated only for size-optimized embedded.
- **Q2:** Apply to `spin::Mutex` too, or only `SwitchMutex`/`IrqSafeMutex`? PM lean: only the kernel-owned lock types; `spin::Mutex` is an external crate that we shouldn't fork.
- **Q3:** Should the diagnostic include a hold-time stat (e.g., timestamp at acquire, reported at NMI)? Useful for finding lock-holders that are taking too long. PM lean: defer to follow-up; this proposal lands the foundational metadata only.
- **Q4:** Seqlock vs RCU vs simple ordering for the consistent-read invariant? PM lean: seqlock — it's what Phase D's instrumentation used, and it's the canonical pattern for this case.
- **Q5:** Sequencing — independent of ADR-018/019, or bundled with one of them? PM lean: independent; this proposal can ratify on its own merits regardless of the other two.

## Cross-references

- `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` — SC-3B closure record; §8.1 declares this proposal as a carry-forward
- `Plans/SC-3B-PHASE-D-FINDINGS-2026-05-10.md` — Phase D capture; demonstrates the diagnostic value
- `Plans/ADR-PROPOSAL-2026-05-10-bible-8-5-4-preempt-while-holding-clarification.md` — companion proposal (declarative discipline)
- `Plans/ADR-PROPOSAL-2026-05-10-switchmutex-irq-off-by-construction.md` — companion proposal (structural enforcement)

## Project-lead decision required

Three options:
- **Ratify as drafted.** ADR-020 file lands in docs/; implementation begins.
- **Ratify with edits.** Project lead names changes; PM redrafts.
- **Reject.** Proposal stays in Plans/ as historical record; ad-hoc instrumentation remains the diagnostic mechanism.

Decision goes in the ratification commit message + the ADR-020 status field on ratification.

---

**End of proposal.**
