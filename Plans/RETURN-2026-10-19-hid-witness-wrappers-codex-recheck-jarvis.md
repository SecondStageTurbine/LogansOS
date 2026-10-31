<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: HID witness-wrapper re-check — Jarvis (Codex)

**Reviewed:** `42ea1af3` (`claude/hid-if-witness-wrappers`)  
**Verdict:** **MERGE** — no findings.  
**Prior `33d476fe` N1:** **CLOSED.** Bind-time `PRIMARY` stores, every ordinary `primary()` lookup,
both device-lock paths, and mouse `debug_inject_report` now pass through wrappers that sample the
caller's IF before acquiring the lock.

## Numbered checks

1. **Completeness — closed.** Exact searches of both drivers found no production `PRIMARY` acquisition
   outside `primary_slot()` and no device `inner` acquisition outside `Keyboard::device()` /
   `Mouse::device()`. This includes bind, `is_bound`, normal reads, routed-event pumps, and the mouse
   injection path. Neither driver has a device/primary `try_lock` or a teardown acquisition. The only raw
   acquisitions are deliberate controls: keyboard `PRIMARY.lock_as_caller()` under
   `neo-test-hid-mixed-if-control`, and mouse `inner.lock_as_caller()` inside `Mouse::device()` under
   `neo-test-hid-device-if-control`; both are sampled.

2. **Production equivalence — preserved.** With control features off, `primary()` still executes within
   `without_interrupts`, obtains the same `IrqSafeGuard`, clones the same `Arc`, and drops the guard
   before the outer interrupt scope restores IF. Keyboard device sections retain their outer
   `without_interrupts` and the same IRQ-safe guard. Mouse `device_section` is the same outer scope, and
   the opaque `DerefMut` returned by `Mouse::device()` owns the same `IrqSafeGuard` through the closure;
   it is dropped before `without_interrupts` exits. Sampling is an atomic test-only side effect and is
   compiled out of production.

3. **Device control — sound and discriminating.** `neo-test-hid-device-if-control` implies
   `neo-test-hid-contention`, omits the mouse device section's outer interrupt mask, and selects
   `lock_as_caller`. The mixed-IF leg explicitly enables interrupts before 64 mouse reads. After
   `primary()` returns and restores caller IF, `Mouse::device()` samples IF before the raw acquisition,
   so the counter must increase and the scenario reaches
   `reason=hid_lock_taken_with_interrupts_enabled`. The holder used by the earlier contention phase has
   already stopped, and the witness increments before acquisition in any event, so a blind witness is
   not being disguised as the expected failure. `lock_as_caller` is available only when one of the
   explicit neo-test controls is enabled.

4. **Lock order — preserved.** Each device mutex is still constructed with
   `IrqSafeMutex::new_ordered(..., USB_DEVICE)`. Both `lock()` and control-only `lock_as_caller()` acquire
   that same underlying ordered `SwitchMutex`; the wrappers do not bypass lockdep or extend the guard
   beyond the original critical section.

5. **Coverage boundary — explicit and acceptable.** The witness covers every acquisition of the two
   HID `PRIMARY` locks and the two HID device locks. It does not independently prove the generic
   `IrqSafeMutex` implementation, detect code that re-enables interrupts after acquisition while still
   holding a guard, or cover unrelated USB locks. Those are outside N1's claim. There are no such IF
   changes inside these HID critical sections, and `IrqSafeGuard::drop` still releases the underlying
   guard before restoring the prior IF state.

## Verification

- `git diff --check origin/main...HEAD` — PASS.
- `cargo fmt --all -- --check` — PASS.
- `cargo check -p neoplutarch-kernel --features neo-test-hid-contention` — PASS (existing dead-code
  warnings).
- `cargo check -p neoplutarch-kernel --features neo-test-hid-device-if-control` — PASS (same warnings).
- `cargo check -p neoplutarch-kernel --features neo-test-hid-mixed-if-control` — PASS (same warnings).
- `cargo check -p neoplutarch-kernel --features neo-test-delivery-drop-control` — PASS (same warnings).
- QEMU was not rerun; the positive/control behavior supplied with the review was checked against the
  scenario's exact feature gates, IF transition, witness increment, and failure predicate.
- Semantic RAG health was healthy, but `last_indexed` was 2026-10-17 and therefore predates the reviewed
  commits; current source and exact-symbol searches were used for the final verification.
