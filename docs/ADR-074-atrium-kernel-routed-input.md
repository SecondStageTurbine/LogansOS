<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-074: Atrium kernel-routed input

Status: accepted for G1 Slice 4 (2026-07-20).

## Decision

Keyboard and pointer events are delivered by the kernel directly to the active
`InputFocus` client. The client owns the RECEIVE-bearing root and explicitly
mints a disjoint ACTIVATE-only sibling for grant to the compositor. That sibling
has neither RECEIVE nor DERIVE, so compositor attenuation cannot manufacture
RECEIVE authority. The compositor owns the focus decision and calls `activate`;
it is not an event intermediary. Its focus-indicator signal is its own decision.

`atrium-input-routing` is a default-off subfeature. With it off, fd 0 continues
to call `hid_keyboard::try_read_byte()` through the original branch. With it on
and the Atrium runtime enabled, keyboard press/release and pointer events enter
the active focus queue. `SYS_INPUT_FOCUS_RECV` (0x1AA) is non-blocking and pumps
both HID completion drains, giving the pointer path a production caller.

## Wire layout (8 bytes, little-endian)

| Offset | Key event | Pointer event |
|---:|---|---|
| 0 | kind = 1 | kind = 2 |
| 1 | bit 0 = pressed | button bitmap |
| 2..3 | HID keycode (u16 LE) | reserved = 0 |
| 4..5 | modifiers (i16 LE) | dx (i16 LE) |
| 6..7 | reserved = 0 | dy (i16 LE) |

Hand-laid fixtures in the host KAT are `[01 01 04 00 22 00 00 00]` and
`[02 05 00 00 fb ff 07 00]`. They are independent of the decoder inputs.

## Atomicity and containment

The pre-existing route word retains CP-11/12/13: Release/Acquire publication,
never-reused identities, and one aligned `u64`. Slice 4 serializes route swap
and enqueue with one short delivery critical section. An event is therefore
entirely before or after a handoff; it cannot enter both queues or the old queue
after the new route is published.

Each client queue is fixed at 64 events and drops the oldest on overflow. The
producer uses `try_lock`; transient consumer contention drops that event instead
of spinning. The HID path never allocates, blocks, parks, wakes, or takes a scheduler lock. This
makes CP-14 (lost wake), CP-15 (stranded wake), and CP-16 (wake under a foreign
lock) structurally absent: receive returns `EAGAIN`, and clients retry. The
delivery lock is not an AI-governance lock and no `INFLIGHT`, `RECENCY`,
`ARENA`, reservation, or `TABLE` lock is reachable from the path.

For I-14: (a) delivery touches only the existing USB-device lock and the small
input router critical section; (b) a non-draining client loses its own oldest
events without stalling another client or governance; (c) Slice 4 adds no DMA
and leaves the existing arena-fenced USB report buffers unchanged.

## Falsifiable gates

The QEMU `atrium_input_routing` scenario uses real process cap tables and
syscalls. It asserts the client root cannot activate, the compositor cap cannot
derive RECEIVE, compositor receive returns `EPERM` without changing a sentinel
buffer, and the client receives the event. Removing the inversion or widening
the compositor cap makes the test red. It retains the atomic-handoff and bounded
queue checks. Host KATs cover wire fixtures, key transitions, and compiled-in /
runtime-off HID equality against the legacy ASCII decoder.

## Amendment (2026-10-18): pointer motion can never evict a keystroke

The single drop-oldest queue let a pointer flood evict keystrokes from a client
that drains slower than the mouse reports. That happens in a multi-window session,
where every client's RECV pumps the shared HID drains, and the DI-1 THINK §0.2 found
it. The queue stays one ordered stream of the same 8-byte events. Only the overflow
policy changes (`kernel/src/atrium/input_queue_logic.rs`):

- **Coalescing.** A motion-only pointer event merges into a motion-only tail with the
  same buttons. The deltas add, saturating at `i16`. "Motion-only" means the button
  state is unchanged since the previous pointer event routed to that queue. Merging
  into the tail never reorders events.
- **Eviction order on overflow.** The oldest motion-only event goes first. Next
  is the oldest button transition that a later pointer event supersedes, whether
  that later event is queued or incoming. Every pointer event carries the full
  button bitmap, so the client's final button state always arrives. Last, the
  oldest key goes.
- **Dropping the incoming event.** If nothing cheaper is queued, an incoming
  motion-only event is dropped itself; its buttons already match the current
  state.
- **When a key can still be lost.** Only to an incoming key, or to an incoming
  button change into a queue holding nothing but keys. Both are key overflows,
  as before, and never the result of pointer traffic.

Bounds are unchanged: 64 entries, no allocation, producer `try_lock`, no wake. The
producer's work under the delivery lock rises from O(1) to O(64). The kill-switch is
kernel feature `atrium-input-queue-legacy`, which restores drop-oldest. Host KATs
prove key-only streams are byte-identical under both rules. The QEMU discriminator
`input_queue_flood` drives a real usb-kbd and usb-mouse through HMP and requires
every keystroke, in order, through a pointer flood that overflows the queue. Under the
legacy rule it must fail, and that run is the control.

## Amendment (2026-10-18): reliable staging, the SAK latch, and B-ref pointer records (DI-1)

DI-1 settled pointer routing as **B-ref** (THINK §8, after the Codex cross-check). The kernel attests
pointer events; the WM, which already owns geometry, applies policy.

- **Staging (S1-F1).** HID drivers decode into a bounded per-device stage and flush it under one `try_lock`.
  Contention defers events instead of dropping them. Kill-switch: `atrium-input-stage-legacy`.
- **SAK (`atrium-sak`).** Super+Tab / Super+Space / Super+Escape, modifier first, are latched in a lock-free
  coalescing kernel latch. Neither the chord key's press nor its release reaches a client. The WM takes the
  latch with `SYS_INPUT_SAK_TAKE`.
- **InputSystem authority.** A capability kind only the kernel mints, into the session root, with rights
  `USE|SYSTEM` and no DERIVE/GRANT. Every DI-1 syscall checks it exactly.
- **Pointer route (`atrium-pointer-forward`).** `SYS_INPUT_POINTER_CLAIM` (`0x1B1`; flag 1 = reclaim)
  starts a session generation. While a route is claimed, each mouse report becomes a kind-4 record in the
  WM's queue; otherwise the mouse keeps its kind-2 route to the keyboard focus. `SYS_INPUT_POINTER_FORWARD`
  (`0x1B2`) consumes a record exactly once: it forwards a kind-3 event, with the record's buttons and phase
  and the WM's coordinates, to a focus the WM holds ACTIVATE on, or drops it. Replays get `EALREADY`;
  overwritten or unknown tags get `ESTALE`; coordinates outside i16 get `EINVAL`.
- **`SYS_TRY_RECV` (`0x1B4`).** `SYS_RECV` that returns `EAGAIN` instead of parking, so the
  single-threaded WM can poll its surface channel, the route and the latch in one loop.
- **Anti-synthesis holds.** No syscall enqueues an event the HID path did not mint; a forward moves a
  kernel-minted record. **The residual is stated plainly:** the WM sees every click (including on-screen
  keyboards and confirmation UI), and a wedged WM stops pointer delivery.

**Wire (8 bytes, little-endian, unchanged layout).**

| Kind | Name | `flags` | `code` | `x`, `y` |
|---|---|---|---|---|
| 3 | `POINTER_ABS` (to a client) | buttons | phase: 1 down, 2 move, 3 up | surface-local, WM-chosen |
| 4 | `POINTER_REC` (to the WM) | bits 0–4 buttons, bit 7 `RESYNC` | record tag (`seq & 0xffff`) | relative motion |

**Byte oracles (hand-laid, asserted in host KATs).**
- Kind 3, DOWN, button 1, (300, −15): `03 01 01 00 2c 01 f1 ff`.
- Kind 4, tag `0x0203`, button 1, dx −2, dy 300: `04 01 03 02 fe ff 2c 01`.

**Discriminators.** `hid_contention`, `di1_forward`, `desktop_pointer_boot` (QEMU, real HID via HMP), plus
the host KATs in `tools/atrium-logic-tests` (`sak_kats`, `pointer_route_kats`).

## Amendment (2026-10-18): staged input is bound to the activation that captured it

Jarvis's cross-check of S1-F1 (#44) found that a key decoded for one focus could be flushed into the
next one after a handoff, and that the keyboard's `PRIMARY` lock was still taken with interrupts on.

- **Activation epoch.** Every change of the active route (activate, deactivate, a dying active focus)
  bumps `ACTIVATION_EPOCH` while holding `DELIVERY`. Each HID stage carries the epoch its batch was
  decoded under. A batch from an earlier epoch is discarded, never delivered (CP-55). The WM's pointer
  records (DI-1a) are not epoch-bound: they always go to the pointer route.
- **Keyboard-state reset policy.** Losing the keyboard means every key is released: a client whose
  focus is deactivated, or another focus activated, must treat all keys as up. The kernel does not
  synthesize releases. A release discarded with its batch therefore leaves no stuck key, and a
  release the new owner receives for a key it never saw pressed is ignored.
- **IF discipline.** `PRIMARY` and every HID device lock are acquired only with interrupts off (CP-56).
- **Queue fallback.** A key overflow evicts the oldest queued key, never the head blindly, so the sole
  unsuperseded button transition survives (Janus, #38 RETURN).

## Amendment (2026-10-18): unambiguous pointer tags, route expiry, capped routes (DI-1a fixes)

Jarvis's cross-check of DI-1a (#46) found three defects.

- **Tags never alias.** A route mints sequences 1..=65 535 only, and a record's tag is its sequence.
  A retained tag therefore names one record for the route's whole life: before this, sequence 1 and
  sequence 65 537 shared a slot and a tag. When a route reaches the end of its sequence space it
  mints nothing more and sends one record with `flags` bit 6 (`ROUTE_EXPIRED`, tag 0, never
  minted). The WM then reclaims a fresh route (a new handle and a new ring) and revokes the old
  handle. Exhaustion no longer resets the ring under the same route handle.
- **RESYNC cancels a stale grab.** A RESYNC record with a button held cancels the WM's current grab;
  the WM consumes until every button is up, so the old target never gets that press. A RESYNC with
  every button up still reaches the grabbed client as the release it missed.
- **Capped client routes.** At most `INPUT_ROUTE_CAP` (32) live client routes exist, and activation
  past the cap returns `EBUSY`. Every id lookup under `DELIVERY` is therefore bounded. The pointer
  route is outside the cap.

