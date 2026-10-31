<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: DI-1a B-ref cross-check — Jarvis

**Verdict: BOUNCE.** The authority split and nonblocking IPC primitive survive review, but the
consume-once token is reusable after only 65,536 reports, and the production WM does not actually
recover a stale client grab when `RESYNC` arrives with a button held. Both violate the findings this
change was intended to close.

Reviewed `origin/main` at `cfafff09` (DI-1a #46, stacked on HID #44). RAG health was healthy; I used
semantic discovery followed by direct reads of the complete ring, focus, syscall, IPC, capability,
driver, WM, and discriminator paths. `cargo test -p atrium-logic-tests` passed all 95 tests.

## Findings

### 1. HIGH / blocker — the 16-bit token aliases a different live record after 65,536 mints

**Files:** `kernel/src/atrium/pointer_route_logic.rs:14-18, 123-150, 156-179`;
`kernel/src/atrium/focus.rs:299-310, 367-404`.

The wire record and forward syscall carry only `seq as u16`. The ring stores a `u32`, indexes by
`seq % 64`, and accepts a token when `slot.seq as u16 == tag`. Therefore sequence 1 and sequence
65,537 name both the same slot and the same tag. A caller that retained tag 1 from the first record
can consume the record at sequence 65,537, supplying coordinates and a target chosen for the old
event. The source comment acknowledges the alias, but that is incompatible with the dispatch's
requirement that a stale record cannot forward after slot reuse.

Minimal pure reproduction: mint sequence 1 and retain its tag; mint 65,536 more records; call
`consume(1)`. It succeeds and consumes sequence 65,537 rather than returning `Stale`. The existing
property test stops at 20,000 steps, below the first alias.

The `u32::MAX` exhaustion path is not fail-closed either. `mint_records` resets `RecordRing` to
sequence 1 and increments `SESSION_GENERATION`, but keeps the same pointer-route object/handle.
`forward_record` never checks `SESSION_GENERATION`, so an old tag can alias the new ring as soon as
that tag is minted again.

**Required correction:** make the token unambiguous for the lifetime of the route. With the current
16-bit ABI, stop before tag reuse and require/perform a reclaim that gives the WM a new route handle;
alternatively widen the token and bind a generation into validation. Add KATs crossing 65,536 and
the production exhaustion/reset boundary.

### 2. HIGH / blocker — `RESYNC` suppresses a new press but leaves and uses the stale grab

**Files:** `userland/neo-init/src/desktop_session.rs:1882-1922`;
`kernel/src/atrium/focus.rs:288-315`; `kernel/src/atrium/input_queue_logic.rs:136-153`.

When `POINTER_RESYNC` is set, the WM correctly refuses to treat a held button as a fresh press, but
it does not invalidate `self.grab`. It immediately matches the old `Grab::Client` and forwards the
record to the old target. The grab is cleared only when a record reports `buttons == 0`.

Concrete sequence:

1. A left-down is processed and establishes `Grab::Client(A)`.
2. The corresponding up transition is expired from the record ring or evicted from the WM queue.
3. The user presses again; the next observable held-button record carries `RESYNC` (or a queue
   eviction marks the following report `RESYNC`).
4. `pressed` is false because of `RESYNC`, but `Grab::Client(A)` remains, so the record is forwarded
   to A as if the first press were still active.

This is the stale-grab/misroute from Jarvis F2. The raw-button latch does not repair it: production
userland has no raw-button syscall and never consults the latch. On `RESYNC`, the WM must cancel the
old grab before selecting a forward target (for example, enter `Consumed` until all buttons are up),
and a KAT must exercise lost-up followed by a new held/down state through the real WM state machine.

### 3. MEDIUM — CP-54 remains: pointer mint/forward perform uncapped route-vector scans under `DELIVERY`

**Files:** `kernel/src/atrium/focus.rs:45-56, 273-283, 288-293, 367-393`;
`kernel/src/arch/x86_64/syscall.rs:6290-6355`.

`Delivery.routes` is a `Vec<RouteEntry>` with no fixed cap. `mint_records` and `forward_record` both
use `iter_mut().find(...)` while holding `DELIVERY` with interrupts disabled. Resolving the target
handle before taking `DELIVERY` does not remove this second id-to-entry lookup.

The length is user-amplifiable: the Atrium focus create and activator-mint syscalls require no
InputSystem authority, so a process can create, mint, activate, and retain arbitrarily many live
focus objects; `insert_route` retains each live owner. This contradicts the claimed O(1) mint/forward
bound and recreates the CP-54 shape. It also lengthens the critical section that the HID stage must
race. Cap the route set, or make the resolved focus object directly name bounded delivery state so
forwarding does not rescan a global unbounded vector.

## Dispatch checklist

1. **Exactly once:** reclaim is serialized with mint by `DELIVERY`, and a reclaimed route fails the
   live-route id check. Replay before tag wrap returns `EALREADY`; overwrite before tag wrap returns
   `ESTALE`. Finding 1 breaks stale-token identity at 65,536 and after the reset path.
2. **Authority:** `InputSystem` is kernel-minted only into neo-init, carries `USE|SYSTEM` without
   `DERIVE|GRANT`, cannot be passed by `sys_spawn`, and is checked exactly on claim/forward/SAK take.
   Input authorization does not consult `scanout_authorized`. No finding.
3. **Forge/misroute:** syscall arguments cannot choose buttons or phase, and a target must resolve
   through an ACTIVATE handle. Delivery to a non-active focus is queued but remains unreadable until
   activation, which is acceptable trusted-WM policy. Finding 2 is an independent stale-target
   misroute in the production WM.
4. **`SYS_TRY_RECV`:** its `SendBlocked` arm matches `recv` under the same channel guard, restores all
   other states, never parks, drops the guard before continuing the sender, and preserves the same
   copy-fault result on both sides. I found no lost/double wake or endpoint corruption.
5. **Locks / IF:** forward resolves both handles before `DELIVERY`; claim, forward, route receive,
   mint/flush, and production handle-drop paths reach it with IF=0. The separate HID RETURN records a
   remaining CP-37 issue in the keyboard `PRIMARY` lookup.
6. **Bounds:** ring operations and queue enqueue are bounded and allocate nothing after bind. Finding
   3 refutes the claimed O(1) route lookup.
7. **Kill-switches:** with `atrium-pointer-forward` off, no claim/forward syscalls or pointer records
   are exposed and the mouse follows ADR-074; with `desktop-pointer` off, neo-init retains the blocking
   DS-1 loop. The shared structs grow, so this is semantic equality rather than binary identity.
8. **Discriminators:** `di1_forward` proves ordinary replay/drop/stale/reclaim using a handful of
   reports; it cannot reach the 65,536 alias. `desktop_pointer_boot` proves one normal click path but
   not queue/ring overflow, held-button `RESYNC`, userland reclaim, or multiple pending SAK bits. The
   authority negative is feature/enforcement dependent as documented.
9. **Other:** distinct SAK bits are atomically retained, but `pointer::take_sak` clears the whole
   bitmask and returns only one action. Simultaneous distinct chords therefore collapse by userland
   priority; document that policy or retain unhandled bits. This is a note, not a merge blocker.

