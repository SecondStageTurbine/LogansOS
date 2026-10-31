<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: DI-1 pointer + SAK decision cross-check (Jarvis / Codex)

**Reviewed:** `origin/claude/desktop-di1-pointer-think` @ `23bf58968ff175721500244ef77bc67e3db3bbc5`  
**Code base checked:** `7deb10f26bc4bc838ce51ce1e3e184a6cd9e66a6`  
**Verdict:** **DISAGREE → B-ref**, after specifying its outstanding-event lifecycle.  
**RAG:** healthy (`ollama=true`, `qdrant=true`) at review time.

The common fixed SAK check is still required, but its delivery cannot use the
best-effort `DELIVERY.try_lock()` path proposed here. For pointer routing I prefer
the smaller userland-policy design: a kernel-minted, consume-once pointer record
forwarded by the trusted WM. Option A's two advertised advantages are real but
too weak to justify putting cursor integration, hit-testing, coordinate
translation, publication, and multi-button grab state in ring 0: a wedged WM has
already frozen every pixel and all chrome actions, while the WM already owns
geometry, scanout, RO client pixels, and cursor position.

## Findings

### 1. HIGH — the proposed SAK can be swallowed and then dropped

**Where:** `docs/THINK-2026-10-18-desktop-di1-pointer-decision.md:76-82,117`; current drop contract at
`kernel/src/atrium/focus.rs:125-130`.

The THINK promises an always-available escape but routes the matched chord with
"the same `try_lock`" discipline. A concurrent region-table publish holds
`DELIVERY.lock()`. If a SAK press arrives then, `try_lock` fails. The keyboard
matcher has already classified/swallowed the press (and later its release), but
the system route receives nothing. This is a real lost-escape execution, not a
queue-overflow hypothetical, and it affects both A and B.

Required change: reserve SAK outside the best-effort input queue, e.g. a bounded
coalescing atomic command latch or a dedicated queue whose publish cannot fail
because `DELIVERY` is busy. Add a negative control that deliberately holds
`DELIVERY` across the real-HID chord and proves exactly one pending system action.

### 2. HIGH — `try_lock` can lose button-up and leave A's grab stale

**Where:** THINK `:96,117,123-124,321`; current producer behavior
`kernel/src/atrium/focus.rs:125-130`.

A's grab ends when the observed button bitmap becomes zero, but a producer-side
contention drop occurs before queue overflow/coalescing policy can protect the
transition. Repro: press in A, begin grab, let a table publish hold `DELIVERY`,
release the last button during that critical section, then press in B. The lost
release leaves the old grab active, so B's new press can route to A. The proposed
"motion coalescing never drops a button transition" KAT covers queue pressure,
not this pre-queue drop.

Required change if A proceeds: update a bounded, non-droppable raw-button/grab
reconciliation latch even when delivery contention drops the event, and test a
dropped last-up, target death, table republish, SAK cancellation, and multiple
buttons. Clearing must happen before routing the next down.

### 3. HIGH — the named system-route authority gate is fail-open and the one-shot has no recovery owner

**Where:** THINK `:105`; `kernel/src/atrium/present_backend.rs:136-164`;
`kernel/src/sched/process.rs:312-319,1072-1080`.

`scanout_authorized(false)` returns true whenever `GPU_SUBMIT_ENFORCE` is off;
that toggle defaults off. It is therefore unsuitable as an authorization gate
for `SYS_INPUT_SYSTEM_ROUTE_CREATE` or table publication. A build/configuration
that enables the input feature without arming scanout enforcement would let the
first unprivileged caller win the one-shot system route. Even when enforcement
is armed, the current full `GpuSubmit` root has full meta rights and may be
derived/granted, so it is not identity-equivalent to "the WM".

The one-shot also has no stated recovery rule: if the WM drops its RECEIVE root
or is restarted, the kernel-owned route remains created but the replacement WM
cannot claim it. Use an unconditional, exact capability check (preferably a
dedicated non-delegable/session-generation input-system authority), bind the
route to that generation, and define revoke/reclaim behavior. Do not call the
scanout kill-switch truth table for input authorization.

### 4. HIGH — A's claimed routing bound excludes the uncapped target lookup

**Where:** THINK `:52,95-98,113-118,246-252`; current registry at
`kernel/src/atrium/focus.rs:34-43,69-79,125-134`.

The 16-entry hit-test is bounded, but each region contains only a `focus_id`.
Delivering to that id using today's structure requires a linear search of the
uncapped `Vec<RouteEntry>`. A grab that stores a focus id needs the same lookup
on every motion. Thus "≤16 compares" and "O(1) work per motion" do not cover the
whole ring-0 route. Dead weak entries are pruned on activation, but live
activator handles can keep an arbitrary number of entries alive.

If A proceeds, cap the live route registry to the same admitted surface bound or
resolve each published region into one of a fixed set of lifetime-safe delivery
slots. `FT-DI1` must count all target-resolution work, not just rectangle
comparisons.

### 5. MEDIUM — "chord in either modifier order" conflicts with the live decoder

**Where:** THINK `:76-80,319-324`; `kernel/src/drivers/usb/hid_logic.rs:145-171`.

`BootKeyEventDecoder` emits events only when a non-modifier keycode changes; a
Super-only transition emits nothing. If Tab is pressed first, its down event is
already delivered to the client. Pressing Super second either cannot trigger the
matcher at all, or a new raw-report matcher triggers after the client has already
seen half the chord. Swallowing Tab's later release would then leave the client
with a stuck key. The stated KAT cannot pass with the stated isolation semantics
without additional policy.

Choose and document one: modifier-first chords only; delay/buffer candidate key
presses; or explicitly send a cancellation/release before taking the SAK. Test
key-first, modifier-first, Super-first release, candidate-key-first release, and
rollover reports through the actual decoder.

### 6. MEDIUM — A's coordinate and grab-generation semantics are incomplete

**Where:** THINK `:95-99,106,133-134,283,321`.

An implicit grab legitimately produces surface-local coordinates outside the
target rectangle, including negative values. The proposed `i16` fields overflow
for a framebuffer coordinate or grabbed offset outside `[-32768,32767]`; the
framebuffer query and buffer descriptors do not impose that bound. A table
republish while grabbed also leaves unspecified whether translation uses the
press-time rectangle or the new rectangle/generation. Clamping would silently
change drag semantics; wrapping would be a security bug.

Specify signed out-of-bounds behavior, reject unsupported framebuffer/surface
dimensions or widen the wire fields, and pin either the press-time origin or a
well-defined table generation for the grab. Add edge tests beyond the target,
negative coordinates, maximum dimensions, resize/republish mid-grab, and target
death.

### 7. MEDIUM — B-ref preserves origin identity only after a missing retention protocol is supplied

**Where:** THINK `:160-169,188-193`.

After the WM dequeues an event, the kernel must retain enough authoritative
state to validate `seq`; otherwise consume-once forwarding cannot work. The
THINK does not bound that outstanding set, define expiry/drop acknowledgement,
say what happens if the WM dies, or fail closed on counter wrap. It also
overstates "anti-forge": the kernel can preserve the original buttons/phase,
but the WM still chooses the target and local coordinates, so semantic
misrouting/re-coordination remains trusted policy.

Before implementing B-ref, define a fixed outstanding ring keyed by
`{route_generation, seq}`, explicit `forward`/`drop` consumption, replay
rejection, overflow behavior, WM-restart invalidation, and fail-closed wrap.
Resolve the target handle at forward time and reject dead/revoked targets. This
is still a smaller and more evolvable ring-0 mechanism than A.

### 8. LOW — fd-0 does not pump the mouse path

**Where:** THINK `:35-43`; `kernel/src/arch/x86_64/syscall.rs:4572-4611,6292-6295`.

fd-0 calls `hid_keyboard::try_read_byte()`, whose drain can route keyboard
events, but it does not call `hid_mouse::pump_routed_events()`. Only
`SYS_INPUT_FOCUS_RECV` pumps both devices. The broader conclusion (a bystander's
valid RECV can advance everyone before its handle check) is correct; the prose
should not imply that fd-0 advances pointer input.

## Dispatch checklist

1. **Ground truth.** xHCI ISR work ends in bounded per-endpoint queues;
   HID decode and `route_event` occur under process-side device locks. RECV
   pumps keyboard then mouse before handle lookup, so a caller with a valid user
   output buffer and a bogus/unauthorized handle advances global input. The
   shared 64-entry drop-oldest queue makes mouse-driven key eviction reachable.
   The fd-0 qualification is Finding 8.

2. **SAK under both.** Under the existing ADR-074 model, a keyboard-blind WM
   and an untrusted focused client leave no userland observer that can guarantee
   escape. A separate trusted userland input broker could recognize the chord,
   but it would receive every key and require a forwarding protocol, discarding
   the very anti-keylogger/anti-synthesis shape being preserved. A small
   below-client SAK recognizer is therefore common to A and B; Finding 1 changes
   its delivery, not that conclusion.

3. **Option A seam.** The current lock direction is
   `USB_DEVICE(29) → DELIVERY`; a publish that copies/validates/resolves handles
   before taking only `DELIVERY` has no inverse USB edge. The implementation
   must also drop the process handle-table lock before `DELIVERY` to avoid a new
   inverse edge through handle destruction. Stale focus ids clean-miss because
   ids are monotonic and `InputFocus::drop` prunes the route, but the table and
   grab still need explicit cleanup/reconciliation (Findings 2, 4, 6). The
   present-authority gate is wrong as stated (Finding 3). A tight trusted
   publisher can repeatedly hold `DELIVERY`, causing both pointer and keyboard/
   SAK producer drops; "16 entries per call" is not a rate bound.

4. **Option B-ref.** A bare 64-bit sequence is not enough. Wrap must fail
   closed; each record must be consumed once; duplicate forwards must fail; a
   dead/restarted WM invalidates its route generation; delayed records need an
   expiry/order rule; and target liveness must be checked at forward time.
   Focus loss alone cannot be the rejection rule because click-to-focus needs
   delivery to an initially unfocused target. WM-chosen coordinates remain
   trusted and must be range-checked. B's material advantages are the smaller
   kernel state machine, userland evolution to touch/tablet, conventional
   compositor semantics, and easier crash containment.

5. **A over B.** A does hide direct client-region button bits from the WM, but
   the WM still sees the cursor path, owns every geometry and the display, and
   can infer many clicks from UI changes. That is incremental privacy, not a
   trusted-path boundary. A's wedged-WM advantage is similarly narrow: pixels,
   chrome focus changes, title moves, and SAK handling are frozen; only clicks
   against a stale table reach still-polling clients. Those gains do not justify
   the extra ring-0 state machine in this rung.

6. **FT-DI1.** It can catch allocation, ordinary misdelivery, compare-count
   regressions, and an accidental dependency on WM polling. It cannot catch a
   SAK or last-button-up lost specifically to `DELIVERY` contention, an
   uncapped post-hit target lookup unless that work is instrumented, a
   fail-open creator gate, route-root loss on WM restart, i16 overflow, or
   table-republish/target-death grab semantics. Those must be named legs if A
   remains a candidate.

7. **DI-1a rung.** Compile-time feature-off can preserve the old single queue
   only if queue layout, decoder state, cursor integration, and chord
   interception are all absent on that branch; the recorded-HID differential
   is appropriate. Empty-table runtime fallback is not itself bit-for-bit
   because feature-on queue/cursor/SAK state remains. Real `usb-kbd`/
   `usb-mouse` HMP discriminators avoid the test-inject seam, but anti-snoop
   needs a positive same-run proof that the client received the click plus
   instrumentation of the WM queue; merely observing an empty WM queue is
   vacuous. Add pitfall entries for best-effort control-event loss, dropped
   release/stale grab, one-shot authority orphaning, and uncapped secondary
   lookup.

8. **Prior art.** The stated high-level lessons are directionally fair.
   Wayland's published protocol model explicitly puts surface-local pointer
   delivery and implicit grabs in the compositor, supporting B as the mature
   default. Microsoft's public Winlogon documentation supports a protected SAS
   below applications. The broad Windows/macOS/Fuchsia/Genode comparisons do
   not prove that this kernel should own hit-testing; doing so would require a
   threat-model argument, not copying or inspecting GPL implementation code.
   No GPL source was read for this review.

9. **Cross-family structural concern.** The proposal treats `try_lock` drops as
   a liveness virtue even for state transitions and control events. It is safe
   for lossy motion, but not for SAK delivery or the transition that terminates
   a grab. The architecture needs two reliability classes, not one inherited
   producer discipline.

## Recommendation and strongest counter-argument

Adopt **B-ref + the corrected kernel SAK latch**. Keep raw pointer ownership and
hit-testing in the trusted userland WM; let the kernel attest a bounded,
consume-once event identity and preserve original button/phase fields. This
keeps scene semantics, coordinate evolution, implicit grabs, touch/tablet
growth, and crashable policy out of ring 0.

The strongest counter-argument is substantial: B makes the WM a pointer
keylogger and a single availability hop. It can observe clicks on an on-screen
keyboard or Arthur confirmation UI, and a wedged WM kills pointer delivery.
That residual must be stated in the ADR. For a later high-assurance path, add a
separately ratified confidential system-surface/input mode; do not pre-pay for a
general kernel window hit-tester in DI-1a.

## Verification

- Direct source reads covered `focus.rs`, both HID drivers/decoder, xHCI ISR
  draining, the RECV and fd-0 syscall paths, focus and GpuSubmit capability
  rights, present authorization, desktop-session routing, ADR-073/074, and the
  CP-bank entries named by the dispatch.
- No build was run: this dispatch changes documentation only.

