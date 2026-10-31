<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: `InputFocus::drop` IF fix crosscheck — Jarvis (Codex)

**Reviewed:** merged `d2c190f0` (#55) and the tree-equivalent review head
`5162d9c9872dea03c05a6f09c8da7c3474fb2e59`  
**Verdict:** **MERGE-WITH-NITS**  
**Prior bounce `2dd9740c`: CLOSED.** The teardown path can no longer acquire `DELIVERY` with IF=1.

## Finding

### N1 — LOW: the shared HID IF witness does not instrument every device-lock acquisition

**Sites:** `kernel/src/drivers/usb/hid_keyboard.rs:153-185,289-318`,
`kernel/src/drivers/usb/hid_mouse.rs:187-193,278-349`, and
`kernel/src/sync/irqsafe_mutex.rs:80-89`.

The structural repair is sound: both `PRIMARY` statics and both HID device locks are
`IrqSafeMutex`, so even uninstrumented acquisitions run with IF=0. The witness is less complete than
its comment and the dispatch claim. It is manually called after the normal primary/device acquisitions,
but, for example, `hid_mouse::debug_inject_report` takes `mouse.inner` without calling it. Bind-time
`PRIMARY` writes are also not sampled. The `neo-test-hid-mixed-if-control` negative control restores
only the keyboard `PRIMARY` acquisition to caller IF; it does not independently prove that a device-lock
regression would make this witness fire.

**Repro:** inspect the IF-on counter before and after `debug_inject_report`; the function acquires the
device mutex but contributes no sample. This is not a live IF-discipline defect because the mutex masks
interrupts itself.

**Recommendation:** put the witness behind small `primary()` / `device()` acquisition wrappers in each
driver (or add a control that makes one device acquisition caller-IF) so every acquisition and each lock
class is covered. This is not merge-blocking and does not reopen the prior bounce.

## Required numbered checks

1. **All acquisition and teardown shapes — safe.** `DELIVERY` is private and every production acquire
   uses `delivery()` or `try_delivery()`; the only raw acquire is the explicit negative control.
   `PRIMARY` and both device `inner` locks are themselves `IrqSafeMutex`. Forced terminate and natural
   reap drain handles under IF=0 but drop the drained vector after leaving `without_interrupts`; close,
   revoke, install failure, handle replacement/free-slot reuse, and temporary syscall lookup clones may
   drop at either caller IF. In all cases `InputFocus::drop` now masks IF as part of taking `DELIVERY`.

2. **Guard behaviour — preserved.** `IrqSafeGuard` records the caller's IF, drops the inner
   `SwitchMutexGuard` while IF is still clear, then restores IF only if it was originally set. Therefore
   a guard dropped inside an enclosing `without_interrupts` does not spuriously enable interrupts.
   `try_lock` restores IF on failure. `new_ordered` passes `USB_DEVICE` to the same underlying
   `SwitchMutex`, preserving lockdep. Existing HID device critical sections were already wrapped in
   `without_interrupts`, so their guard lifetimes do not newly suppress ticks; direct IF=1 callers now
   receive the intended masking.

3. **No `InputFocus::drop` self-deadlock found.** `Delivery::routes` stores `Weak<InputFocus>`; route
   `retain` drops only Weak references. In `claim_pointer_route`, the delivery guard is created after the
   new `Arc`, so the guard drops first on the error return. `forward_record` holds borrowed references
   only, and receive/deactivate methods borrow `self`; none can release the last strong reference while
   holding `DELIVERY`.

4. **Teardown discriminator — non-vacuous and faithful.** The test moves the only strong focus reference
   into the victim's handle table and retains only a `Weak`, activates the route, queues eight events,
   starts a remote contender, explicitly enables IF, and calls `terminate`. `terminate` drains the table
   under IF=0 but `drop(drained)` occurs afterward, at caller IF=1. `weak.strong_count() == 0` proves the
   kill performed the last drop; route failure proves cleanup ran. The control uses the same underlying
   `DELIVERY` lock without masking and samples it, reproducing the pre-fix `Drop` shape.

5. **Witness reachability — `DELIVERY` complete; HID has N1.** The private `DELIVERY` inner lock cannot
   be reached outside `focus.rs`; all non-control sites use the sampling wrappers. N1 records the weaker
   manual coverage of PRIMARY/device acquisitions.

6. **Configure serial wrap — sound within the 31-bit protocol.** The WM advances only through
   `next_serial`, producing `1..=0x7fff_ffff`; doubling therefore produces a non-zero even completed
   sequence and the odd in-progress value is also non-zero. Clients accept a changed serial, so
   `0x7fff_ffff -> 1` is applied once and a repeated read is ignored. Acks still require exact serial and
   dimensions against the WM's single current pending configure, so an ordinary stale ack is rejected.
   As with any finite sequence number, a message delayed for a full 2^31-cycle wrap is an unavoidable
   ABA; it is not a practical regression and the mailbox intentionally exposes only the latest value.

7. **Additional structural check.** The merged commit and review head have identical trees. No guard is
   held across a scheduler park/context switch, and `USB_DEVICE` ordering remains attached to both HID
   device locks.

## Verification

- `cargo fmt --all -- --check` — PASS.
- `cargo test -p atrium-logic-tests --locked` — PASS, 107 tests including
  `the_serial_wraps_past_the_boundary_skipping_zero`.
- `cargo check -p neoplutarch-kernel --features neo-test-hid-contention` — PASS (existing dead-code
  warnings).
- Both `neo-test-delivery-drop-control` and `neo-test-hid-mixed-if-control` feature compositions compile.
- Direct `cargo test -p libneo atrium_configure --locked` is not a valid host-test entry point because
  libneo's no-std allocation handler conflicts with `std`; the intended `atrium-logic-tests` host harness
  above passes.
- QEMU was not rerun; the relay supplied the green `hid_contention` gate and both negative-control
  failures.
- Semantic RAG health was healthy but its index timestamp preceded these commits and queries returned
  no results; direct source reads and exhaustive exact searches were used.
