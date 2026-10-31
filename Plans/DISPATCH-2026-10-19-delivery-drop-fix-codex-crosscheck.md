<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family check of the `InputFocus::drop` IF fix (#48 bounce) → Jarvis (Codex)

**Type:** review only. **Deliverable:** a pushed RETURN file. **Why you:** you have reviewed this lock family
twice (#44 F1/F2, the #48 fix-forward). The PM's #48 bounce found a path both reviews missed:
`InputFocus::drop` took `DELIVERY` with IF=1 during process teardown. The change is SMP / concurrency-core,
so a different-family lens is required before merge. This fix is on the escaped-defect clock
(fix-forward window ends ~06:15 UTC 2026-10-19, then the PM reverts).

## What is under review

Branch `claude/delivery-drop-fix` (from `origin/main` 2acfa308); the PM names the head SHA in the relay.
Diff: `git diff origin/main...origin/claude/delivery-drop-fix`.

1. **The structural fix (CP-56, Drop/teardown blind spot).**
   - `kernel/src/sync/irqsafe_mutex.rs` gains `new_ordered` and a control-only `lock_as_caller`.
   - `DELIVERY` (`kernel/src/atrium/focus.rs`) is now an `IrqSafeMutex`. Its guard masks interrupts and
     restores the caller's IF on release, so no caller (syscall, HID pump, or a `Drop` run by teardown at
     IF=1) can hold it with IF=1.
   - Every acquisition goes through `delivery()` / `try_delivery()`, which feed the neo-test witness
     `debug_delivery_if_on_acquisitions`.
   - Both HID `PRIMARY` statics and both HID device locks (`hid_keyboard.rs`, `hid_mouse.rs`) are
     `IrqSafeMutex` too. The device locks keep their `USB_DEVICE` lock-order level.
2. **The discriminator.** The `hid_contention` teardown leg (`hid_teardown_kill_focus_holder` in
   `kernel/src/test_harness.rs`):
   - A registered process's handle is the last reference to the ACTIVE focus, which holds 8 queued key
     events.
   - A contender on another CPU keeps pumping both real HID devices and taking `DELIVERY`.
   - `process::terminate` is called with interrupts ON, which is the shape of the drop after
     `without_interrupts` in `terminate` and the natural reap.
   - Required: `kill_rc=0 focus_dropped=true route_cleared=true delivery_if_on=0`.
   - Control: `neo-test-delivery-drop-control` restores the pre-fix `Drop`. It must FAIL with
     `reason=delivery_taken_with_interrupts_enabled_in_teardown` (it shows `delivery_if_on=1`).
3. **#53 LOW nit.**
   - `userland/libneo/src/atrium_configure.rs`: serials live in `1..=0x7fff_ffff` and `next_serial` wraps
     to 1, skipping 0. A client applies a configure whose serial *differs* from the last applied (`is_new`),
     no longer "greater than".
   - The WM (`neo-init/src/desktop_session.rs`) and Arthur (`neo-arthur/src/showcase_surface.rs`) use them.
   - KAT: `the_serial_wraps_past_the_boundary_skipping_zero`.

## Refute it

1. Is there any acquisition of `DELIVERY`, `PRIMARY` or a HID device lock that can still run with IF=1?
   Check every feature combination, every `Drop`, and every teardown or handle-close path: `terminate`,
   natural reap, `sys_close`, revoke, `install_cap` replacing a handle, the `Arc` clones that syscall
   lookups drop on return, and test seams.
2. Does `IrqSafeGuard` change behaviour anywhere the old `SwitchMutex` guard was relied on? Consider:
   - IF being restored at guard drop *inside* an enclosing `without_interrupts`;
   - the `try_lock` failure path restoring IF;
   - lock-order (`new_ordered`) tracking on the device locks;
   - guard lifetimes that now span code which expected IF=1.
3. Can `InputFocus::drop` run while the same CPU already holds `DELIVERY` (a self-deadlock)? Check
   `insert_route`'s `retain` (drops `Weak` only), `claim_pointer_route`'s error path drop order, and
   `forward_record`.
4. Is the teardown leg vacuous? Is the focus really dropped *by the kill*, at IF=1, with the route active?
   Does the control faithfully reproduce main's `Drop`?
5. Is the witness placed so that a future raw acquisition would be caught? Is there any way to reach the
   inner `SwitchMutex` outside the control features?
6. Serial wrap: can a client skip or re-apply a configure across the wrap? Can a stale ack be honoured?
   Is `seq` ever 0 or odd for a complete publish?
7. Anything else a Claude reviewer would structurally miss.

A finding counts only if it survives your own re-verification; uncertain items are notes. "No findings"
must say what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/delivery-drop-fix-crosscheck-jarvis   (git fetch origin && git checkout -B codex/delivery-drop-fix-crosscheck-jarvis origin/claude/delivery-drop-fix)
```

```
git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-19-delivery-drop-fix-codex-crosscheck.md
#   (until the PR merges: git show origin/claude/delivery-drop-fix:Plans/DISPATCH-2026-10-19-delivery-drop-fix-codex-crosscheck.md)
git diff origin/main...origin/claude/delivery-drop-fix
```

Optional runs, at `NEO_SMP=4`:
- `cargo xtask neo-test hid_contention` should PASS.
- The same with `NEO_TEST_EXTRA_FEATURES=neo-test-delivery-drop-control` should FAIL with
  `reason=delivery_taken_with_interrupts_enabled_in_teardown`.
- The same with `NEO_TEST_EXTRA_FEATURES=neo-test-hid-mixed-if-control` should FAIL with
  `reason=hid_lock_taken_with_interrupts_enabled`.

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?"
questions; grep still wins for exact symbols. Health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`, expecting `{"status":"healthy",...}`. If it is down or
degraded, the PL manages it: note that in your RETURN and continue with direct reads.
Optional self-serve start:
`powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"`, then
`cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
Example query: `$body=@{query='InputFocus drop process teardown handle drain interrupts'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-19-delivery-drop-fix-codex-crosscheck-jarvis.md` to
`codex/delivery-drop-fix-crosscheck-jarvis` and push it. Contents:
- a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
- each finding with severity, file:line and a repro;
- what you checked for each numbered item.

Paste the pushed SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree; never delete the
worktree.
