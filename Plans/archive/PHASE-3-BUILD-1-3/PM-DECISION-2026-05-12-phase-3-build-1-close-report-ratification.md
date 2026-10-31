# PM DECISION - Phase 3 BUILD-1 close report ratification

**Date:** 2026-05-12
**Status:** RATIFIED
**Authority:** PM consumption of Prometheus close report
**Primary artifact:** `Plans/CLOSE-REPORT-2026-05-12-phase-3-build-1.md`

---

## 1. Decision

Phase 3 BUILD-1 is PM-ratified as **CLOSED**.

The close report is accepted as the final BUILD-1 evidence bundle. The branch
`codex/phase-3-build-1-jarvis` is accepted through commit `7f48c85`
(`kernel/storage: add aarch64 virtio barrier lane`) for BUILD-1 closure
purposes.

No BUILD-2 NeoFS work is authorized by this decision.

---

## 2. Acceptance Basis

The following BUILD-1 gates are satisfied:

- Block substrate, DMA buffer abstraction, and PCI/virtio probe substrate landed.
- Virtio-blk raw single-block read/write path landed.
- `IoRendezvous.waiting_thread` lifetime issue resolved via scheduler-owned
  `ThreadWakeToken { cpu_id, slot, generation }`.
- F_FLUSH is asserted as offered and deliberately not negotiated.
- CONFIG_WCE is deliberately not negotiated.
- Abandoned-I/O no-resurrection/no-leak scenario passes.
- Minimal VFS/testfs and file syscalls pass.
- BUILD-1 integration scenarios pass on x86_64.
- Reboot persistence smoke passes.
- aarch64 barrier-exposure runtime lane passes under QEMU aarch64 with
  `compile_only=false`, `event_idx_negotiated=true`,
  `publish_descriptors=2`, `before_notification_check=2`, and
  `data_integrity=ok`.

---

## 3. Course Corrections Accepted

The two BUILD-1 course corrections are accepted and closed:

1. **1B scheduler lifetime blocker.** The abandoned `Weak<Thread>` direction
   was replaced with scheduler-owned wake tokens. This preserves SC-4 process
   exit semantics and avoids keeping parked threads alive through storage I/O.
2. **1D aarch64 blocker.** The missing aarch64 kernel substrate was bypassed
   with a dedicated no_std QEMU `virt` + virtio-mmio barrier lane sharing the
   production barrier wrapper source.

Both corrections have verification artifacts and are included in the close
report.

---

## 4. Formatting Caveat

The formatting caveat is accepted as non-blocking style debt:

- Global `cargo fmt --check` now runs, but fails on broad pre-existing repo
  formatting drift.
- Scoped rustfmt for the new aarch64 harness and shared barrier wrapper passes.
- BUILD-1 closure must not be represented as global formatting PASS.

---

## 5. Follow-On Authorization

Jarvis is authorized to perform the post-close integration lane:

1. Create/update the normative Phase 3 status documentation.
2. Update `Plans/INDEX.md` for BUILD-1 artifacts.
3. Commit only the named closeout/doc artifacts.
4. Merge `codex/phase-3-build-1-jarvis` to `main` only after the docs/index
   commit and the specified verification pass.

The follow-on dispatch is:

`Plans/DISPATCH-2026-05-12-phase-3-build-1-post-close-docs-and-merge-jarvis.md`

---

## 6. Stop Conditions

Stop and file a blocker instead of merging if any of the following occur:

- The close report's CLOSED status or aarch64 PASS frame cannot be preserved.
- The docs update would need to weaken ADR-021 or invent BUILD-2 authority.
- `cargo k` or `cargo k --no-default-features` fails after the docs/index
  commit.
- The aarch64 barrier lane no longer passes.
- The merge produces conflicts outside the expected docs/index or already
  committed BUILD-1 implementation surface.
- Any untracked, unrelated plan/doc material would need to be added to complete
  the merge.

---

## 7. External Status

External project status after this decision:

**Phase 3 BUILD-1: CLOSED, pending post-close docs/index integration and merge.**

**Phase 3 BUILD-2: NOT AUTHORIZED.**
