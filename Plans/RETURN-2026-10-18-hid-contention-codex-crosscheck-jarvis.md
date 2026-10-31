<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: HID contention cross-check — Jarvis

**Verdict: BOUNCE.** Per-device staging repairs the original transient `DELIVERY.try_lock()` loss,
and the SAK/button latches are sound at the driver boundary. However, staged keys can cross an
already-observed focus handoff into the wrong client, and the CP-37 repair leaves the keyboard
`PRIMARY` spinlock mixed between IF=1 and IF=0 in a shipped/reachable path.

Reviewed `origin/main` at `cfafff09`, with the HID change isolated as `968d4ca4..b4772d98` and its
DI-1a integration checked on main. RAG health was healthy; semantic discovery was followed by direct
reads of the drivers, queue, focus router, syscall paths, latches, and QEMU discriminator.
`cargo test -p atrium-logic-tests` passed all 95 tests.

## Findings

### 1. HIGH / blocker — a decoded key can cross a later focus handoff into the new client

**Files:** `kernel/src/atrium/focus.rs:111-120, 206-255`;
`kernel/src/drivers/usb/hid_keyboard.rs:193-242`.

The stage stores only `InputEventV1`; it does not capture the active focus id or a focus generation.
`flush` reads `ACTIVE_INPUT_FOCUS` only after it eventually acquires `DELIVERY`. Therefore this
non-overlapping sequence is possible:

1. Focus A is active and a key report is decoded into the keyboard stage.
2. `DELIVERY.try_lock()` is contended, so the event remains staged.
3. After that holder releases the lock, focus B wins `activate()` and publishes B.
4. The next pump flushes A's already-decoded key into B's queue.

This is not merely an event concurrent with the atomic swap: step 1 may precede step 3 by an
arbitrary number of failed pumps. It violates ADR-074's no-wrong-surface/anti-keylogger contract and
can disclose the tail of input intended for A to B. The old implementation lost this event, which
was an availability defect, but changing loss into cross-client disclosure is not an acceptable
repair.

**Required correction:** bind staged events/batches to the active focus generation observed at
decode, and fail closed on a generation mismatch (with an explicit keyboard-state reset policy so a
cross-handoff release cannot leave stale client state). Add a discriminator that stages a real key
for A, activates B before flush, and proves B receives none of A's event.

### 2. HIGH / blocker — CP-37 remains on the keyboard `PRIMARY` lock

**Files:** `kernel/src/drivers/usb/hid_keyboard.rs:143, 246-259, 263-268`;
`kernel/src/arch/x86_64/syscall.rs:4707-4747, 6421-6436`;
`kernel/src/sync/switch_mutex.rs:122-180`.

`try_read_byte()` clones `PRIMARY` with `PRIMARY.lock()` *before* entering its new
`without_interrupts` closure. The fd-0 blocking loop calls this inside `with_interrupts_enabled`, so
that `SwitchMutex` is acquired with IF=1. `SYS_INPUT_FOCUS_RECV` enters with IF masked and calls
`hid_keyboard::pump_routed_events()`, which acquires the same `PRIMARY` with IF=0.

Reachable deadlock:

1. Any user process calls ambient `read(0, ...)` and is preempted on CPU N while holding `PRIMARY`.
2. The scheduler runs the WM on CPU N; its focus-receive syscall calls the HID pump with IF=0.
3. `SwitchMutex::lock()` spins on `PRIMARY`; the timer cannot run and the preempted owner can never
   resume to release it.

This is reachable in the desktop feature set even if the intended UI does not normally read fd 0:
fd 0 is a syscall special case, not a capability, so an untrusted process can issue the read and
repeat it to widen the small preemption window. The inner `USB_DEVICE` lock was correctly moved to
IF=0; the outer `PRIMARY` lock was missed. Move the `PRIMARY` clone inside the same IF=0 window (as
the fd-0 mouse pump already does externally), and add a mixed fd-0/focus-receive SMP discriminator.

## Dispatch checklist

1. **Loss / duplication:** every successful keyboard decode is filtered as SAK or staged/directly
   routed before its report slot is reposted; mouse raw state is latched before stage; flush pops only
   after acquiring `DELIVERY` and enqueues once. I found no duplicate path. Stage overflow remains an
   intentional counted loss: once 64 entries contain only keys, an incoming key evicts the oldest
   key, so a release can still be lost under sustained contention.
2. **Ordering:** FIFO is preserved within each device. Keyboard and mouse still have no cross-device
   total-order guarantee, and independent batch flushes do not weaken a documented one. Finding 1
   breaks the security boundary across focus handoff.
3. **Bounds / I-14:** stage length is capped at 64 and the preallocated `VecDeque` does not grow past
   that logical cap. A flush performs at most 64 O(64) queue operations. That O(4096) section is
   bounded, but on integrated main it also includes the separately reported uncapped route-vector
   lookup. All-key overflow can evict a key as documented; there is no keyboard raw-state
   reconciliation equivalent to mouse buttons.
4. **Pitfall bank:** USB-device to `DELIVERY` remains one-way and nonblocking (`try_lock`), with no
   park/wake edge; route publication atomics are unchanged. `DELIVERY` itself is forced IF=0. Finding
   2 refutes the claimed CP-37 completeness one lock outward.
5. **Kill-switch:** `atrium-input-stage-legacy` restores per-event direct routing and leaves the empty
   stage flush as a no-op. SAK, raw-button, fd-0 mouse pumping, and the IF discipline are orthogonal
   fixes and intentionally remain active; equality is therefore for staging semantics, not the
   entire pre-PR binary.
6. **Discriminator:** `hid_contention` requires a different holder CPU, real report-count progress,
   exact 20-event order, and nonzero `FLUSH_CONTENDED`, so it is not vacuous for the original race.
   It never activates a second focus and never runs an IF=1 fd-0 reader, so it cannot catch either
   blocker above. It also does not fill the 64-entry stage.
7. **Latches / fd 0:** SAK modifier-first and swallow-release behavior matches the real decoder KATs;
   atomic OR prevents contention loss and intentionally coalesces repeats. `RAW_BUTTONS` records each
   decoded mouse bitmap and can legitimately be newer than the delivered stream. The fd-0 mouse pump
   is present. Production DI-1a userland does not consume the raw-button latch, which contributes to
   the stale-grab issue recorded in the DI-1a RETURN.
8. **Other:** the SAK latch preserves distinct bits, but DI-1a userland clears the full bitmask while
   applying only one prioritized action; that consumer-policy note is recorded in the DI-1a RETURN.

