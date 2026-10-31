<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: DI-1 — the pointer + escape-chord decision, prepared for PL ratification

**Status:** decided. The Codex cross-check (Jarvis) refuted Option A; DI-1 is **B-ref**, specified in §8, which
supersedes §4–§5 wherever they disagree. **Author seat:** Claude (S2 session). **Base:** `origin/main` @ `7deb10f2`
(S1 #25 + S2 #31 merged). **Needs before commitment:** the Codex cross-family check in
`Plans/DISPATCH-2026-10-18-di1-pointer-codex-crosscheck.md`. This recommendation *agrees with Oracle*, and Oracle
and this author are both Claude-family, so the agreement is weak evidence by CLAUDE.md §Cross-model diversity.

The question, from `docs/THINK-2026-07-23-desktop-ux-SYNTHESIS.md` §"The ONE real divergence":
- **Oracle "Strata"** (branch `origin/codex/desktop-ux-design-vision-oracle`, §3.3–3.4, §6) proposes:
  - a kernel cursor plus a kernel pointer hit-route against a WM-published region table;
  - a kernel Secure-Attention-Key (SAK) chord table.
- **Janus** (branch `origin/codex/desktop-ux-design-vision-janus`, §5) proposes an "InputFocus v2":
  - split keyboard from pointer;
  - then *either* a pointer-only, revocable WM hit-test route (userland forwarding) *or* a bounded kernel routing
    cache;
  - and explicitly "not grounds to move WM policy into the kernel".

---

## 0. Ground truth on `main` (what both options extend)

These facts correct or sharpen both source THINKs. All are verified on `7deb10f2`.

1. **Pointer events are already kernel-routed, but to the *keyboard* focus.** With `atrium-input-routing`,
   `hid_mouse::Inner::drain` calls `route_event(InputEventV1::pointer(dx, dy, buttons))`
   (`kernel/src/drivers/usb/hid_mouse.rs:213-222`). The event is relative motion, `kind = 2`
   (`kernel/src/atrium/input_logic.rs:34-42`, ADR-074 wire table), and goes into the one active `InputFocus`
   queue.
   - There is no cursor position, no hit-test, no absolute event and no enter/leave.
   - The wheel byte is dropped.
2. **Keys and pointer share one 64-entry, drop-oldest queue per client** (`kernel/src/atrium/focus.rs:34`,
   `route_event` :125-141). A moving mouse can evict keystrokes from Arthur's queue today; Arthur discards
   `kind != 1` (`userland/neo-arthur/src/showcase_logic.rs:36-44`). This is a **latent defect independent of
   DI-1**, and both options must fix it.
3. **Nothing runs in an ISR above the xHCI event ring.** `xhci::handle_interrupt` only moves completions into
   bounded `IrqSafeMutex` endpoint queues (`xhci.rs:827`, `EP_QUEUE_CAP = 64`). HID decoding and `route_event`
   run in **process context**, inside `pump_routed_events`. That pump is called from
   `SYS_INPUT_FOCUS_RECV` *before* its handle/permission check (`syscall.rs:6292-6295`) and from the fd-0 read.
   - **Correction to Oracle §3.4/§6:** "ISR-safe like `route_event`" is not the current shape. The real
     discipline is "bounded, no-alloc, `try_lock`, no wake, in the RECV pump", and both options inherit it.
   - **Consequence:** input advances only while *some* process calls RECV. A wedged focused client does not
     freeze input as long as another principal (the WM) keeps polling RECV. If every principal stops polling,
     no input moves under either option. No timer pump exists.
4. **The WM is keyboard-blind and pointer-blind today.**
   - It holds an ACTIVATE-only sibling (`compositor_input_focus_rights`, `sched/process.rs:297`).
   - It proves its own RECV is `EPERM` (`userland/neo-init/src/desktop_session.rs:356-360`).
   - Its loop handles only Commit and Destroy (:379-428).
5. **No chord is reserved anywhere.** The boot-keyboard decoder emits events only on keycode transitions; a
   modifier-only change is not an event (`hid_logic.rs:145-171`).
6. **`DELIVERY` is a plain `SwitchMutex` (unordered, not IRQ-safe)**, nested under the HID driver's `USB_DEVICE`
   (29) lock. Its `Vec<RouteEntry>` is pruned but not capped (`focus.rs:43, :70`). Any new table placed under
   it must keep CP-37 true: `DELIVERY` must never be reached with IF=0.
7. **The WM already owns the geometry needed to hit-test.** `userland/libneo/src/atrium_desktop.rs` `layout()`
   (:92-176) defines the `frame`, the `TITLE_H = 28` `title` band and the `client` rect. It is syscall-free and
   mounted in the kernel harness and host KATs.

---

## 1. The escape chord (SAK) is not actually contested — decide it first

The user must always be able to leave a hung or malicious focused client. On a keyboard-only machine (the Silent
Test's keyboard half, and the bare-metal box without a mouse), the only signal is a key chord. The WM is
**keyboard-blind by design**: that is ADR-074's anti-keylogger property, and both families want to keep it. So the
WM *cannot* see a chord, and only the kernel can recognise one.

**Janus's userland-forwarding option does not remove this ring-0 change.** It removes only the pointer half.
Option B in §2 therefore *also* needs the SAK table. That is the decision-relevant correction to the framing
"kernel vs. userland": both options put a fixed chord compare in the keyboard pump.

**SAK design (common to both options).**
- **Table:** a `const` table of ≤ 8 chords. Proposed:
  - `Super+Tab`: next window
  - `Super+Space`: summon the Spine
  - `Super+Escape`: escape (revoke client focus)
- **Where it is checked:** in `hid_keyboard`'s routed decode, before `route_event`. A matching press goes to a
  kernel-owned **system route** whose RECEIVE root the WM holds.
- **Swallow rule:** the release of any key whose press was swallowed is also swallowed (a 256-bit per-keycode
  bitmap), so a client never sees half a chord.
- **Fixed, not configurable:** widening the table is a kernel change through the AAG. A client cannot shadow it,
  and the WM cannot grow it into a keylogger.
- **Bounds:** ≤ 8 compares per key transition, no alloc, the same `try_lock`. I-14 clean.

---

## 2. The two pointer options, concretely against today's code

### Option A — kernel cursor + hit-route (Oracle §3.4, refined)

**Kernel changes**

| Change | Where |
|---|---|
| Cursor integration: relative dx/dy → clamped absolute `(x, y)` in framebuffer pixels | `hid_mouse` routed drain (pump, process context) |
| Region table: fixed `[Region; 16]` of `{rect, kind ∈ {client, chrome}, focus_id}` in z-order, plus a generation counter, stored inside the `DELIVERY` critical section | `kernel/src/atrium/focus.rs` (+ pure logic in `input_logic.rs`) |
| Hit-test on button-down: top-down walk; implicit grab until all buttons are up; motion during a grab goes to the grab target | `focus.rs` `route_pointer` (new), called from the mouse pump |
| System route: one kernel `RouteEntry` (id 1, never reused) receiving chrome-targeted pointer events, SAK chords and cursor-position updates | `focus.rs` |
| Separate pointer queue per `RouteEntry` (fixes §0.2), with motion coalescing | `focus.rs` |
| New wire kind 3 `POINTER_ABS`: `flags` = buttons, `code` = phase (down/move/up), `x, y` = surface-local i16 for `client` targets and screen coordinates for the system route | `input_logic.rs`, `libneo` mirror, ADR-074 amendment with a hand-laid byte oracle |

**New syscalls** (numbers provisional; 0x1B1–0x1B2 are free on `main` @ 7deb10f2)

| Number | Syscall | Purpose and gating |
|---|---|---|
| `0x1B1` | `SYS_INPUT_SYSTEM_ROUTE_CREATE` | One-shot. Returns a RECEIVE root for the system route. Gated on the compositor's present authority (the same authority `scanout_authorized` checks). |
| `0x1B2` | `SYS_INPUT_REGION_TABLE(entries, n ≤ 16)` | Process context. Copy-in and validate: rect inside the framebuffer; each `client` entry names an **activator handle the caller holds**, resolved to a `focus_id` at publish time. Then swap the table under `DELIVERY.lock()`. |

**Userland changes**
- The WM publishes the table after every stack or geometry change.
- An unfocused window's whole frame is registered `chrome`, so the first click goes to the WM (click-to-focus
  consumes the click, as Oracle proposes).
- The title band is always `chrome`.
- The WM draws the sprite from system-route position updates.

**ISR-shaped bounds.** Everything runs in the RECV pump (process context). There are ≤ 16 rect compares per
button transition, O(1) work per motion event, and no allocation (fixed arrays). The producer takes `DELIVERY`
via `try_lock` (drop on contention, counted). Publish takes `DELIVERY.lock()` in process context for a 16-entry
memcpy. Nothing is reached with IF=0.

**Wedged or malicious focused client**
- Clicks always route by geometry, never via the focused client.
- The escape is the SAK or a click on another window.
- A client cannot capture the pointer beyond an implicit grab, and a grab ends at button-up. A stuck button
  (hardware) is bounded by the SAK escape, which also cancels any grab.

**Wedged WM**
- Clicks still route to clients against the last published table, and SAK events queue on the system route.
- The screen is frozen, though, because the WM composes every pixel. The benefit is real (clients keep working
  and can still be closed via their own UI) but partial.

**Anti-keylogger and INV-WM-1**
- The WM receives keys only as SAK chords.
- Clients receive only pointer events aimed at their own region, plus their grab, in surface-local coordinates.
  They learn no screen position, so INV-WM-1's "no reply discloses geometry" holds.
- **Anti-synthesis holds:** no syscall lets any principal enqueue into another principal's queue; events are
  minted only by the HID path.
- **Residual:** to draw the sprite, the WM learns the cursor *position* everywhere, but never the button state
  over a `client` region. The hover path over a client remains visible to the WM. Removing that requires a
  hardware cursor plane (future, OT-63 accel), not this rung.

**Pitfall-bank classes touched (`docs/concurrency-pitfalls.md`)**

| Pitfall | How Option A treats it |
|---|---|
| CP-1 (lock order) | Publish takes `DELIVERY` without any USB lock; the pump takes `USB_DEVICE` → `DELIVERY`. No ABBA, as long as the publish path never takes a USB lock while holding `DELIVERY`. |
| CP-11/13 | The table lives entirely inside the `DELIVERY` critical section, so there is no lock-free publication and no torn read. |
| CP-12 | Focus ids are monotonic and never reused, so a stale entry naming a dead focus misses cleanly (the event is dropped and counted, never misdelivered). |
| CP-17 | The system route exists before the WM's first RECV; events published before that queue bounded. |
| CP-37 | `DELIVERY` stays out of IF=0 contexts. Assert it in debug builds. |
| CP-14/15/16 | Remain structurally absent: no park or wake is introduced. |

### Option B — WM pointer route + forwarding (Janus §5, made precise)

**Kernel changes**
- Split the route word: `ACTIVE_POINTER_FOCUS` alongside `ACTIVE_INPUT_FOCUS`. Pointer events route to the WM's
  own pointer queue, never to the keyboard focus.
- The same cursor integration as A is still needed: the WM needs positions, and doing it in userland from
  relative deltas works too, so it is optional here.
- **Plus the SAK table** (§1).
- Forwarding, in one of two variants:
  - **B-raw:** `SYS_INPUT_FOCUS_FORWARD(activator, event_bytes)`. The WM enqueues any pointer-kind event into a
    client queue. **This breaks anti-synthesis**: a compromised WM can mint clicks a client believes came from
    the user.
  - **B-ref (route-by-reference):** the kernel tags each pointer event it delivers to the WM with a per-event
    sequence number. `SYS_INPUT_FOCUS_FORWARD(activator, seq, local_x, local_y)` moves *that kernel-minted
    event* (consume-once) into the target queue, with WM-supplied local coordinates bounded by the target's
    published size. This preserves anti-synthesis against forging. The WM can still misroute, drop, delay or
    re-coordinate events, the same power a wrong region table has in A.

**New syscalls:** `SYS_INPUT_POINTER_ROUTE_CLAIM` (WM-only) and `SYS_INPUT_FOCUS_FORWARD`.

**Userland changes:** the WM hit-tests in userland using `atrium_desktop` geometry. It must **poll its pointer
queue continuously**, because the input path has no wake. That is a CPU cost A does not pay, since clients poll
anyway.

**ISR-shaped bounds:** O(1) kernel work per event. The forward syscall is process context with a bounded copy.
Hit-testing cost moves to userland.

**Wedged or malicious focused client:** handled. The pointer never goes to the client first, so click-to-focus
always works, and the keyboard escape is the SAK.

**Wedged WM:** the pointer is dead, since all pointer input waits behind the WM. The SAK still queues.

**Anti-keylogger and INV-WM-1**
- Keys stay WM-blind except SAK.
- **The WM sees every pointer event, including button state over client regions.** That re-creates part of the
  X11 snoop surface: clicks on an on-screen keyboard *are* keystrokes, and the trusted-path concern is real for
  Arthur's agentic confirmation UI.
- B-ref is anti-forge but not anti-snoop. B-raw is neither.

**Pitfall-bank classes:**
- CP-11/12 on the split route word (same pattern as today).
- Consume-once sequence numbers need CP-12 care: a never-reused 64-bit counter per route.
- Otherwise the same as A, minus the table.

### Side by side

| Property | A: kernel hit-route | B-ref: WM forward by reference |
|---|---|---|
| New ring-0 surface | Cursor + 16-entry table + grab + 2 syscalls (~250 LOC est.) | Split route + seq tags + forward + claim (~150 LOC est.) |
| SAK table needed | yes | yes |
| Escape from wedged/malicious client | yes | yes |
| Pointer when the WM is wedged | routes to clients (pixels frozen) | dead |
| WM sees client clicks | no | **yes** |
| WM sees cursor position | yes (sprite) | yes |
| Kernel-minted events only (anti-forge) | yes | yes (B-ref) / **no** (B-raw) |
| Extra hop per pointer event | 0 | +1, and the WM must poll |
| Policy in ring 0 | none; z-order *data* only | none |

---

## 3. Prior art (principle only; no code read for copying)

- **Wayland:** the compositor is the trusted input router. It receives all evdev input, hit-tests its own scene,
  delivers to surfaces with an implicit grab on button press, and clients cannot snoop each other. The
  compositor sees everything, and a wedged compositor wedges input. The escape (VT switch) is a kernel/logind
  path outside the compositor. Lesson: the reference design is B-like, and it still needs a kernel-side escape.
- **Fuchsia Scenic:** the scene manager (userland) hit-tests its view tree and delivers pointer streams per view
  over capability channels (mouse/touch sources), with focus as a separate focus chain. Lesson: hit-testing
  against a *published view tree* by a trusted component, with per-view channels, is the capability-native
  shape. A's region table is the kernel-sized version of that.
- **Windows win32k:** window manager, hit-testing and the raw input thread live in the kernel. The secure
  attention sequence (Ctrl+Alt+Del) is trapped below every app, and it is the archetype for our SAK. win32k is
  also the canonical CVE sink. Its bugs come from a huge stateful object model, **user-mode callbacks during
  kernel operations**, and parsing in ring 0 (menus, fonts, window objects), which motivated Win32k filtering
  and lockdown.
  - **Lesson:** the danger is policy, object models and callbacks in ring 0, not a bounded data table. A must
    stay data-only, with no callbacks and no parsing beyond fixed-size records, to stay out of that class.
- **macOS WindowServer:** a userland server does hit-testing and event routing to apps over IPC. Secure Event
  Input lets password fields block keyloggers, and Force Quit is served by the system, not the hung app. A
  wedged WindowServer freezes the UI and is restarted by a watchdog. Lesson: the B shape works in practice; its
  failure mode is the whole-UI wedge plus watchdog restart.
- **Genode nitpicker (on seL4/NOVA):** a tiny trusted GUI multiplexer. It owns hit-testing over client views,
  routes input to the hovered or focused session, and provides an **X-ray mode** (a secure attention key that
  labels every view with its true owner). Layout and decoration live in *untrusted* separate components
  (window manager, layouter, decorator).
  - **Lesson, and the closest analogue:** put the minimal trusted mechanism (hit-test over views, focus, SAK) in
    the smallest trusted component, and keep all policy out of it. In our system the smallest trusted component
    on the input path is the kernel focus router. That is the argument for A, provided A stays nitpicker-sized.

---

## 4. Recommendation

**Adopt the SAK table (§1) unconditionally.** For the pointer, **adopt Option A: kernel cursor plus a
bounded, data-only hit-route**, under these hard constraints. Violating any one of them flips the recommendation
to B-ref.

1. The region table is fixed-size (≤ 16 entries), fixed-record, single-writer and data-only. There are no
   callbacks, no parsing beyond fixed records, and no allocation on any routing path.
2. All routing runs in the RECV pump under the existing `try_lock` discipline. Nothing new runs with IF=0
   (CP-37), and no new lock level is added.
3. Focus *policy* (which window to focus, raise, move or restack) stays entirely in the WM. The kernel only
   routes events to the entry the WM published.
4. **Anti-synthesis is an ADR invariant:** no production syscall lets any principal enqueue an input event into
   another principal's queue. Test seams stay `cfg(neo-test-*)`.

**Why A over B-ref, in one line.** A gives the same escape guarantees plus two properties B structurally cannot
provide: no WM visibility of clicks on client surfaces, and pointer routing that survives a wedged WM. It does
this with a nitpicker-sized kernel delta that sits in the `route_event` discipline already ratified by ADR-074.

**What A costs.** Roughly 100 more kernel LOC than B-ref. Also, every geometry change is a table publish
(≤ 16 entries; process context).

### Named falsification test (the Sage rule: hypothesis + what would prove it wrong)

**Hypothesis H-DI1:** A's ring-0 delta can be built entirely inside the existing pump discipline (bounded, no
alloc, `try_lock`, no new lock level, no IF=0) and survives a cross-family adversarial review with no unfixable
finding.

**Falsification test `FT-DI1` (three legs; any failing leg falsifies H-DI1 and the recommendation reverts to
B-ref):**

1. **`di1_pointer_route_bounds`** (QEMU, NEO_SMP=4, real `usb-mouse` driven by HMP `mouse_move` /
   `mouse_button`, no test-inject):
   - 10 000 pointer events against a full 16-entry table must show an **allocation-counter delta of 0** across
     the pump.
   - The hit-test compare count must stay ≤ 16 per button transition.
   - Not one event may be misdelivered: each lands in its grab target or the hit entry, checked against a host
     model.
   - Needing an allocation, an unbounded walk or an IRQ-context path to pass falsifies H-DI1.
2. **`di1_wedged_wm_click`** (QEMU): the WM publishes a table, then stops polling and presenting. A client
   polling RECV must still receive a kind-3 click with exact surface-local coordinates.
   - If it cannot (for example, because routing silently depends on the WM's RECV), A's main advantage over B
     does not exist, and the cheaper B-ref wins.
3. **Codex cross-check** (`Plans/DISPATCH-2026-10-18-di1-pointer-codex-crosscheck.md`): a surviving finding on
   the publish/route seam or the grab lifecycle that cannot be fixed inside constraints 1–4 falsifies H-DI1.

If H-DI1 falls, the fallback B-ref is already specified (§2) and keeps every escape property. The cost is the
WM-sees-clicks residual, which must then be stated to the PL plainly.

---

## 5. The smallest buildable DI-1 rung under the recommendation (DI-1a)

**Scope.** The ring-0 work is limited to `kernel/src/atrium/{focus.rs, input_logic.rs}`,
`kernel/src/drivers/usb/{hid_keyboard.rs, hid_mouse.rs}` and two syscalls. The userland work is in
`desktop_session.rs`, `atrium_desktop.rs` and `libneo`.

1. **Queue split** (fixes §0.2): each `RouteEntry` gets a 64-entry key queue and a 32-entry pointer queue.
   Motion coalesces: when the pointer queue is full, consecutive moves merge instead of dropping a button event.
2. **SAK:** `Super+Tab` (cycle), `Super+Space` (Spine), `Super+Escape` (escape) go to the system route, with the
   swallow-release bitmap.
3. **Kernel cursor and region table:** 16 entries, with an implicit grab and kind-3 events, as in §2-A.
4. **WM:**
   - **Click-to-focus.** An unfocused window's frame is `chrome`; the first click activates that window's
     `InputFocus` and republishes the table.
   - **Pointer sprite.** A 12×18 arrow in `text-primary` with a 1 px `ground` outline. It is drawn in the WM's
     scene from system-route positions, and its damage is old rect ∪ new rect.
   - **Title-band move.** Button-down on the `title` (always `chrome`) starts a move. Grabbed motion moves the
     frame, clamped to the workfield. The table is republished on button-up.
   - **Always-available escape.** `Super+Escape` deactivates the client focus, returns focus to the Spine and
     shows an S3 glint. It works while a client spins or is wedged.

**Discriminators**

- **Host KATs** (`tools/atrium-logic-tests`):
  - A region hit-test byte oracle: a hand-laid table plus points mapped to expected entries, covering z-order,
    inclusive top-left and exclusive bottom-right edges, an empty table, and a stale `focus_id`.
  - Cursor clamp at all four edges.
  - Grab lifecycle: down in A, drag into B, up → every event goes to A.
  - The SAK matcher: chord in either modifier order, swallow-release, Super released first, non-SAK
    pass-through.
  - Kind-3 wire fixtures as hand-laid bytes (a format change, so it needs an independent byte oracle).
  - Motion coalescing never drops a button transition.
- **QEMU** (NEO_SMP=4; real `usb-kbd` and `usb-mouse` driven through HMP, with **no** test-inject in these
  builds):
  - **`di1_click_to_focus`:** two clients. The first click on unfocused B leaves B's queue empty and moves
    focus to B. The second click delivers kind 3 with exact local coordinates. The WM's queue never holds a
    client-region button event (anti-snoop).
  - **`di1_sak_wedged_client`:** the focused client spins without calling RECV, and a second, malicious client
    calls RECV in a tight loop. `Super+Tab` and `Super+Escape` reach the WM, and focus moves. No client queue
    ever holds Tab or Escape.
  - **`di1_title_move`:** drag the title band by (+100, +40). The framebuffer FNV digest must equal
    `scene_pixel` for the moved layout, with zero stale pixels. This reuses S1's oracle.
  - **`di1_pointer_route_bounds`** and **`di1_wedged_wm_click`:** the FT-DI1 legs above.
  - **Anti-synthesis:** a source check that no production `cfg` exposes an enqueue syscall (symbol and grep
    assertion in CI, the same pattern as the existing compile-out of test seams).

**Kill-switch**
- Kernel feature `atrium-pointer-route` (default off; `desktop` may enable it once gated) and `atrium-sak`.
- **Off ≡ ADR-074 today, bit-for-bit:** kind-2 relative pointer events to the keyboard focus, no chord
  interception. A host KAT drives one recorded HID stream through both builds and asserts identical routed
  event sequences.
- At runtime, an empty region table also falls back to the ADR-074 route.
- Queue split under the kill-switch: off keeps the single queue, preserving bit-for-bit equivalence. The
  eviction fix ships with the feature on.

**Governance (I-14).**
- (a) No AI-governance lock is reachable from either pump or from publish (grep plus lockdep).
- (b) A flooding mouse degrades only the pointer queue (coalesced and capped), and a flooding table publisher is
  bounded at 16 entries per call.
- (c) No DMA change.

**Concurrency.** This is SMP-touching: it changes the cross-CPU-visible routing state. The dispatch must
therefore carry the pitfall-bank consultation, and the merge-time AAG **requires a cross-family lens** (CLAUDE.md,
PL-ratified 2026-07-26).

---

## 6. PL summary (≤ 6 lines)

1. Either way, you get a mouse that focuses and moves windows, plus a key chord (Super+Escape) that always gets
   you out of a frozen or hostile app.
2. The chord needs a tiny fixed kernel check whichever we pick, because the window manager is deliberately
   blind to your typing.
3. **A (recommended):** the kernel delivers each click straight to the window under the pointer. The window
   manager never sees what you click inside apps, and clicks keep working if it freezes.
   **Risk:** about 250 lines of new kernel code on the input path.
4. **B:** the window manager receives every click and passes it on. That is a little less kernel code and is how
   Linux and macOS do it. **Risk:** the window manager sees all your clicks (on-screen keyboards, Arthur's
   "confirm" buttons), and the mouse dies if it freezes.
5. Before we commit, a different AI family (Codex) reviews the kernel design. If A can't stay small and
   bounded, we fall back to B automatically.

---

## 7. Sources

- `docs/THINK-2026-07-23-desktop-ux-SYNTHESIS.md`
- Oracle's THINK: `origin/codex/desktop-ux-design-vision-oracle:docs/THINK-2026-07-23-desktop-ux-oracle.md`
- Janus's THINK: `origin/codex/desktop-ux-design-vision-janus:docs/THINK-2026-07-23-desktop-ux-janus.md`
- `docs/ADR-073-atrium-surface-protocol-v1.md`
- `docs/ADR-074-atrium-kernel-routed-input.md`
- `docs/concurrency-pitfalls.md` (CP-1, 11–17, 37)
- Code: `kernel/src/atrium/{focus.rs, input_logic.rs, logic.rs}`, `kernel/src/drivers/usb/{hid_keyboard.rs,
  hid_mouse.rs, hid_logic.rs, xhci.rs}`, `kernel/src/arch/x86_64/syscall.rs` (0x1A6–0x1AC, 0x1B0),
  `userland/neo-init/src/desktop_session.rs`, `userland/libneo/src/atrium_desktop.rs`
- Prior art (Wayland, Fuchsia Scenic, win32k, WindowServer, Genode nitpicker): cited for principle only, with no
  source read for reuse.

---

## 8. Codex outcome → B-ref (2026-10-18)

**Verdict.** Jarvis's Codex cross-check (`origin/codex/di1-pointer-crosscheck-jarvis:Plans/RETURN-2026-10-18-di1-pointer-codex-crosscheck-jarvis.md`,
`5aa95c0a`): **DISAGREE → B-ref**. Two HIGH findings break §4's own hard constraints, so FT-DI1 leg 3 fires
and the pre-approved fallback applies (PM relay, 2026-10-18; PL: "if A can't stay small we fall back to B
automatically").
- **F4** breaks constraint 1 (bounded routing): every routed event needs an uncapped focus lookup.
- **F3** breaks the authority model: `scanout_authorized` is fail-open when enforcement is off, and the
  one-shot route has no recovery owner.

The measured A prototype (parked at `claude/di1a-option-a-wip` @ `156f4a32`, reference only) was already
~447 code lines against §2's ~250 estimate.

**What survives from §1–§5.**
- The SAK recognizer, reworked by F1/F5 below.
- The queue-eviction fix, merged in #38.
- The process-context pump discipline.
- The anti-synthesis ADR invariant.

**What is withdrawn.**
- Kernel cursor, region table, hit-test and kernel grab (all of §2-A).
- `SYS_INPUT_REGION_TABLE`.
- FT-DI1 legs 1–2.

**The residual B carries**, stated plainly as §6.4 already did: the WM sees every click, including on-screen
keyboards and Arthur's confirm UI, and a wedged WM stops pointer delivery. A confidential system-surface input
mode is the future answer (Jarvis's recommendation), not this rung.

### 8.1 Findings and where each is handled

| Jarvis | Severity | Disposition |
|---|---|---|
| F1: SAK can be swallowed, then dropped by `try_lock` | HIGH | Lock-free coalescing **command latch** (`focus::latch_sak`, atomic OR); never queued. Negative control: `DELIVERY` held across a real-HID Super+Escape → exactly one pending action. **#44.** |
| F2: contention-dropped last button-up leaves a stale grab | HIGH | Root cause removed in #44: decoded events are staged per device and never dropped for contention. A raw-button latch (`focus::raw_buttons`) is stored before staging. Under B-ref the grab lives in the WM, so the kernel's duty is "every record carries the full button bitmap, and a dropped transition forces a resync record" (§8.3). Negative control in #44. |
| F3: fail-open authority, orphaned one-shot | HIGH | §8.2: a dedicated, exact, non-delegable **InputSystem** capability bound to a session generation, with revoke/reclaim. `scanout_authorized` is not used. |
| F4: uncapped secondary lookup | HIGH | Moot under B-ref: no per-event kernel lookup by focus id. A forward resolves one handle the caller names (§8.3). |
| F5: "either modifier order" contradicts the decoder | MED | **Modifier-first only** (§8.4), tested through the real `BootKeyEventDecoder` (#44 KATs). |
| F6: i16 overflow, grab origin | MED | §8.3: forwarded coordinates outside i16 are **rejected** (`EINVAL`), never clamped or wrapped. The WM pins the grab origin at press time. |
| F7: B-ref needs a retention protocol | MED | §8.3: fixed outstanding ring keyed by `{route_generation, seq}`. |
| F8: fd-0 does not pump the mouse | LOW | Fixed in #44 (fd-0 pumps both devices; device locks held IF=0, CP-37). |

### 8.2 Authority: the InputSystem capability (F3)

- **Object.** A new kernel object, `InputSystem`, holding `session_generation: u64` (monotonic, never reused).
  The only handle kind is the root, with rights `USE` and **no `DERIVE`/`GRANT`**, so it cannot be delegated.
  It is minted once by the kernel and installed in neo-init at the same boot step that hands neo-init its
  scanout authority. No syscall mints one.
- **Exact check.** Every DI-1 syscall requires the caller to hold an `InputSystem` handle whose generation
  equals the live `SESSION_GENERATION`. No feature switch or enforcement toggle can open it: if the object
  is absent, the answer is `EPERM`.
- **Revoke and reclaim.**
  - `SYS_INPUT_SYSTEM_RECLAIM(authority)` bumps `SESSION_GENERATION` (a WM restart inside neo-init), installs
    a fresh generation in the caller's handle and invalidates the old pointer route (ring cleared, queue
    discarded).
  - If the holding process dies, its handle drops: the pointer route and SAK consumer are released, and the
    generation bumps on the next reclaim. If neo-init itself dies there is no WM, the same failure domain as
    the scanout cap; this is documented, not hidden.

### 8.3 B-ref pointer route (F7, F6, F2)

**Claim.** `SYS_INPUT_POINTER_CLAIM(authority)` returns a RECEIVE-only root for the one pointer route of the
current generation (`EEXIST` while one is live). While a route is claimed, mouse records go to it instead of
the keyboard focus. With no route, or with feature `atrium-pointer-forward` off, it is ADR-074's kind-2 route
bit-for-bit.

**Record minting.** Every decoded mouse report becomes a kernel-minted record:
- A kind-4 `POINTER_REC` event: `flags` bits 0–4 = the full button bitmap, bit 7 = `RESYNC`; `code` =
  seq & 0xffff; `x/y` = dx/dy.
- `seq` is a per-route u32 counter.
- The record is also stored in the outstanding ring.

**The ring.**
- **Shape:** a fixed `[Outstanding; 64]` with entries `{seq: u32, buttons, dx, dy, state: Live | Consumed}`,
  indexed by `seq % 64`.
- **Matching:** a forward or drop names `seq16`. It matches only if `ring[seq16 % 64]` is Live, has
  `seq & 0xffff == seq16`, and the route generation equals the caller's.
- **Ambiguity:** with 64 slots and a 16-bit tag, a match is ambiguous only if 65 536 records were minted
  between delivery and forward. By then the slot has been overwritten, so the match fails closed (`ESTALE`).
- **Overflow:** minting into a Live slot expires the old record (counted; a later forward gets `ESTALE`).
  If the expired record was a button transition (its bitmap differs from the previous record's), the next
  record is flagged `RESYNC` and carries the latched raw bitmap. A WM that lost a transition therefore
  re-learns the true state before acting on the next press (F2).
- **Wrap:** reaching `u32::MAX` bumps the route generation and clears the ring (fail closed). Replays of old
  seqs then miss.

**Consumption.** `SYS_INPUT_POINTER_FORWARD(route, seq16, target, x: i32, y: i32, op)`:
- `op = FORWARD`: `target` must be an ACTIVATE handle the caller holds, naming a live `InputFocus` that has a
  route entry (dead or revoked → `ESRCH`, resolved at forward time). The kernel enqueues a kind-3 event
  `{flags: record.buttons, code: phase, x, y}` into that focus's queue. The phase (down/move/up) is derived
  from the record's bitmap against the previous record's. The buttons and phase come from the record, never
  from the WM.
- `op = DROP`: consume without delivering.
- **Exactly once:** a second forward or drop of the same seq returns `EALREADY`.
- **Coordinates (F6):** the WM chooses `x, y` (trusted policy, as Jarvis notes: B is anti-forge, not
  anti-misroute). Values outside `i16` are `EINVAL`, never clamped. The WM pins the grab origin and target at
  press time and holds them until a record shows buttons == 0.
- **Not a rejection rule:** the target being unfocused does not by itself reject a forward. Click-to-focus
  activates first and then forwards, or consumes the click by policy.

**Bounds.** Mint and expire are O(1). A forward is one handle lookup plus one enqueue under `DELIVERY.lock()`
in process context (no USB lock held, and the handle-table lock is dropped before `DELIVERY`, per Jarvis
checklist 3).

### 8.4 SAK (F1, F5)

- **Policy:** modifier-first. Super, then Tab / Space / Escape. Key-first is not a chord, and the client
  sees the whole keystroke.
- **Swallow-release:** a chord's release is swallowed in any order.
- **Delivery:** `latch_sak` (atomic OR, coalescing). The WM takes it with `SYS_INPUT_SAK_TAKE(authority)`,
  which returns the pending bits and clears them.
- **Feature `atrium-sak`:** off means no interception.
- **Landed** with the latch and its real-decoder KATs in #44.

### 8.5 The WM loop (resolves the open question from the A attempt)

The WM's Arthur loop blocks in `sys_recv(surface)`, and B requires polling the pointer route.

- **Chosen: a non-blocking channel receive**, `SYS_TRY_RECV` (`0x1B4`, under `atrium-pointer-forward`).
  - It is the same rendezvous as `SYS_RECV`: when the peer is send-blocked it copies and wakes the sender.
    Otherwise it restores the endpoint state and returns `EAGAIN` instead of parking.
  - The WM stays single-threaded. Its loop polls the surface channel, the pointer route and the SAK latch,
    exactly as Home already polls its own focus queue.
  - It touches the IPC core, so the Codex lens covers it (CP-14/15/16: no new park or wake edge, since it
    only ever takes the existing send-blocked handoff).
- **Rejected after checking: a WM input thread.** `SYS_THREAD_CREATE` requires the `browser-runtime`
  feature and capability, which `desktop` does not enable. Taking it on would bring the browser VM
  primitives into the desktop image, plus a cross-thread scene lock, which is a far larger surface than one
  non-parking receive. (The first version of this section chose the thread before that gate was checked.)

### 8.6 DI-1a rung under B-ref

**Kernel**
- The InputSystem capability, plus reclaim.
- The pointer route claim, record minting and the outstanding ring.
- `FORWARD`/`DROP`.
- `SAK_TAKE`.
- `SYS_TRY_RECV` (§8.5).
- Feature `atrium-pointer-forward`, default off. Off is ADR-074 bit-for-bit: the mouse keeps its kind-2 route
  to the focus.

**Userland:** the WM's polling loop (§8.5), click-to-focus (the first click on an unfocused frame activates it and is
consumed), the sprite (damage = old ∪ new), title-band move, and `Super+Escape` returning focus to the Spine.

**Discriminators**
- **Host KATs:**
  - ring lifecycle: forward/drop once, replay → `EALREADY`, overwrite → `ESTALE`, wrap → generation bump,
    RESYNC after an expired transition;
  - a hand-laid kind-4 byte oracle;
  - phase derivation;
  - i16 rejection.
- **QEMU, real HID via HMP:**
  - **`di1_forward_click`:** the WM forwards a click; the client receives exact local coordinates and the
    record's buttons.
  - **`di1_forward_replay`:** a replayed or overwritten seq is refused.
  - **`di1_wm_restart`:** reclaim invalidates the old route.
  - **`di1_authority`:** a process without InputSystem gets `EPERM`, including with GPU-submit enforcement
    off.
  - **`di1_sak_wedged_client`:** with `DELIVERY` held, the latch already proven in #44.
  - **`di1_title_move`:** an FNV digest equal to `scene_pixel` for the moved layout.

**Kill-switch, pitfall bank and AAG:**
- **Kill-switch:** as above.
- **Pitfall bank (new entries, per Jarvis checklist 7):**
  - best-effort control-event loss;
  - a dropped release leaving a stale grab;
  - one-shot authority orphaning;
  - an uncapped secondary lookup.
- **AAG:** Claude lenses plus a Codex cross-family lens (SMP-core).
