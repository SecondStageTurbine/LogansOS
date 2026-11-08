<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: OT-88, the app model, and terminal, files and settings

**Date:** 2026-10-29 · **Author:** S1 · **Dispatch:** `Plans/DISPATCH-2026-10-29-ot88-apps-think-s1.md` ·
**Status:** design only, no code. A cross-family read (Codex, GPT-6-Luna high) comes before the merge.

OT-88 (terminal, file manager, settings) and OT-67 (the text editor) are MVP (0.5) items
(`Plans/DECISION-2026-10-27-release-tiers-mvp.md:10, :20-21`). All four are apps the user types into and points at.
Today only Arthur can be one. This THINK designs the missing piece first (§2), then each app against it (§3-§6), then
the rungs (§7) and J.c.'s decisions (§8).

Citations are to `origin/main` at `da8707a3`. "DS" means `userland/neo-init/src/desktop_session.rs`.

## 1. What the tree does today

### 1.1 Launch

- **Arthur is hard-wired into the WM's loop.** `run()` (DS:994) loops `home::read_intent` → `arthur_session` →
  `home::return_home` (DS:1081-1109). Every non-empty Home intent goes to Arthur. `spawn_arthur` (DS:1898-1922) calls
  `sys_spawn_client("arthur", …)`.
- **The LaunchCap is real but advisory.** `launch::mint` (DS:2149) hands a client a LaunchCap with an allowlist. The
  client sends `OP_LAUNCH {module_id, token}`; the broker checks the token, then `broker.judge`, then spawns (DS:2185-2254).
  - Module ids are an enumeration, never a name: `MODULE_ARTHUR = 1`, `MODULE_PROBE = 2`, `MODULE_LENS = 3`
    (`userland/libneo/src/atrium_launch.rs:20-45`).
  - The kernel's `sys_spawn` is still ambient until K2's `SpawnCap` (`docs/THINK-2026-10-27-ot66-login-session.md` §8).
- **Spawn takes a name, not a path.** The kernel finds a Limine module whose path ends in `/neo-<name>`
  (`kernel/src/sched/process.rs:3118-3133`). The image ships four: neo-init, neo-worker, neo-shell and neo-arthur
  (`boot/limine.conf:26-35`). Nothing loads an ELF from NeoFS.
- **Worker roles are cargo features.** `neo-worker`'s `_start` picks its role by `#[cfg(feature = …)]`
  (`userland/neo-worker/src/main.rs:412-422`); xtask picks the features per scenario (`tools/xtask/src/main.rs`
  `worker_features`). The Lens's "role comes from what the broker hands it" (`atrium_launch.rs:27-30`) is true only
  inside the Lens build: with no handles it acts as the probe.

### 1.2 Focus and keys

- **The kernel already has many routes, one active.** `kernel/src/atrium/focus.rs`:
  - one global `ACTIVE_INPUT_FOCUS` (:119), swapped in `activate` (:200-221) under `DELIVERY`;
  - `ACTIVATION_EPOCH` drops batches staged for an old owner (:125-131);
  - a 64-event queue per route (:37) and at most 32 live client routes (:43, CP-54);
  - `route_event` delivers HID events only to the active route (:310-337).
- **A client owns its focus; the WM only switches it.** Arthur creates its focus and an activator
  (`userland/neo-arthur/src/showcase_surface.rs:114-115`). It grants the WM an ACTIVATE-only activator (:207-210), and the
  WM proves it can't receive on it (DS:1249-1261). The WM has its own focus for Home (DS:3247-3254).
- **Handoffs are ordered.** `atrium_handoff::hand_over` swaps the route, discards the old owner's queue, then presents
  (launch DS:1276-1315, hold DS:3563-3598, resume DS:3476-3508, return DS:3605-3634).
- **The SAK chords are fixed kernel policy.** Super + Tab, Space or Escape (`kernel/src/atrium/sak_logic.rs:31`):
  "next window, summon the Spine, escape". The WM drains the latch (`take_sak`, DS:4579-4596).

### 1.3 Pointer

- The WM claims the pointer route (DS:4287-4306). `Ctx.activator` is `(!escaped).then_some(arthur)` (DS:436): **Arthur
  is the only possible target.**
- Records are forwarded only during a press-grab that started inside Arthur's window (`Grab::Client`, DS:4470-4520).
  There is no hover motion. The wheel goes to Arthur while the pointer is over his window (DS:4390-4407).
- The kernel side is generic: `forward_record` (`focus.rs:555-610`) sends to any route that has been activated.

### 1.4 Surfaces

The wire is designed for many principals (`userland/libneo/src/atrium_protocol.rs:4-7`; opcodes 1-10 at :18-44). The
WM's side is Arthur's:
- a fixed namespace (DS:70);
- `ROLE_TEXT` at 640×400 required (DS:1142-1149);
- control bytes `B/G/M/K/D/I`;
- one `arthur_session` with one surface.

The Lens reuses the opcodes through its own `SurfaceRegistry` and namespace (DS:5126-5209).

### 1.5 The Lens

The Lens is a second process with a surface but **no focus and no input** (`focus=none`, DS:5073). It lives inside
Arthur's `OP_LAUNCH` while Arthur blocks on the reply. "Switching" to it moves the WM's selection, and keys stay on the
WM's focus (DS:5232-5310). DS-1's THINK chose this on purpose: "strict alternation… the WM never needs to multiplex two
clients" (`docs/THINK-2026-10-18-ds1-home-spine-shell.md:173-178`).

### 1.6 Faults

- **Refuse-and-continue, from #182 (`bbba7381`) and #189 (`46d6ba1e`).** `enum Fault` and `refuse()` (DS:1857-1884)
  answer a bad request with a status and keep the loop going. They cover undecodable bytes, a commit fault, BUSY,
  CONFIGURE, DESTROY, YIELD and unknown opcodes (DS:1414-1691).
- **Still fatal to the whole WM:** a handshake failure (DS:1139-1271, codes 5-21), a dead channel (24, DS:1370), and a
  nonzero Arthur exit (30, DS:1726-1731). `run()` returns the code and neo-init exits.
- **Ring-3 exceptions are contained.** `contain_user_fault` (`kernel/src/arch/x86_64/idt.rs:598-625`) kills the faulting
  process for every vector, and `#PF` has its own kill path. (OT-83's row in `docs/OPEN-THREADS.md:94` still reads as
  open. The code says it is fixed; the row is stale.)

### 1.7 S3's installer

S3's installer avoided all of this by being a WM view (`installer_view.rs` on `claude/ot87-r4.4-ui`; ledger
`Plans/LEDGER-2026-07-29-logansos-release-ledger.md:533-539`). It is drawn in Home's card slot and reads keys from the
WM's own focus. Option B there was "runtime roles with pointer forwarding", deferred until a real second client needs
it. These four apps are that need.

## 2. The app model

### 2.1 What an app needs

1. **Launch:** from a Spine chip or a Home intent, through the broker. It gets only the caps its manifest names.
2. **A surface** of its own size and role, composited by the WM.
3. **Keys** while it is focused, and **none** while it is not. No key typed for one app may reach another (the F1
   discard rule).
4. **The pointer:** presses, drags, the wheel and hover motion inside its surface.
5. **Focus moves** between Arthur, the apps and Home: by the Spine, Super+Tab and Home intents.
6. **Death is that app's alone.** A crashing, hung or misbehaving app costs its own window. Never the session, and never
   Arthur.
7. **No ambient authority.** Directory caps from K1 only, the clipboard later through the WM, no root cap.

### 2.2 Three designs

**Design A: every app is a WM view** (the installer's way).
- Each app is code inside neo-init, drawn in the WM's plane, reading the WM's own focus.
- **For:** no protocol or kernel work. The pointer and keys are already the WM's.
- **Against:**
  - Every app runs with the WM's authority: `Handle::InputSystem`, the root directory cap, the installer cap. A parser
    bug in the terminal (escape sequences are attacker-shaped bytes) is a bug in the process that owns input.
  - A crash in any app is a crash of the session (§1.6).
  - The terminal hosts child processes. Hosting them inside the WM makes the WM a process supervisor too.
  - neo-init's `.text` and its review surface grow by four apps.
  - It contradicts the cap model the OT-66 THINK is building, where the WM derives per-client caps and revokes them at
    logout.

  It suits the installer, which needs the installer cap the WM already holds and must not hand out. It doesn't suit
  apps.

**Design B: one binary per app, launched by module id.**
- Each app is its own boot module: `neo-terminal`, `neo-files`, `neo-settings`, later `neo-editor`.
- A static **app manifest** in neo-init maps each module id to:
  - its binary name;
  - the caps the broker hands it at spawn (its directory caps; nothing else);
  - its surface role and size;
  - whether it takes focus.
- The WM's Arthur-only paths become a **client table** (§2.3). Each app does what Arthur does today: it creates its own
  focus, hands the WM an ACTIVATE-only activator, and creates a surface over the existing opcodes.
- **For:**
  - Each app holds only its manifest's caps. A fault kills one process, which the WM sees as EPIPE plus an exit code.
  - The kernel already has per-client routes (cap 32), ordered handoffs, pointer forwarding to any activated route, and
    "next window" in the SAK table.
  - It is the shape K2 expects: the WM derives a `SpawnCap` per client with an allowlist of module ids.
- **Against:**
  - Four more boot modules, so a `limine.conf` line and an xtask build each.
  - The WM's Arthur-specific code (§1.4) has to be made generic, which is the bulk of the work.

**Design C: one multi-role app binary, role chosen at run time from the caps it is handed.**
- `neo-worker` grows into a launcher of every role: the broker hands it, say, a tty handle and it is a terminal, or a
  directory cap and a surface and it is Files.
- **For:** one module; adding an app adds no `limine.conf` line.
- **Against:**
  - Every app's code is in every app's process.
  - The role is inferred from the *shape* of a handle list. One wrong handle quietly turns one app into another. The
    Lens's probe fallback (`userland/neo-worker/src/lens.rs:15-16`) is a small instance of that already.
  - It fights how roles are built today (cargo features). Making them all runtime-selectable compiles every role into
    one binary, so any role's dependencies (`vt100`, the editor's text engine) land in all of them.
  - It saves only the module list. Design B's per-app cost is a line and a build target.

### 2.3 Recommendation: Design B, with side-by-side floating windows

**Pick B.** What it needs:

1. **The client table.** In DS, `arthur_session`'s single client becomes `clients: [Client; N]`, with N = 6 (Arthur, the
   Lens, terminal, files, settings, editor). Each entry holds:
   - its module id;
   - its surface registry and namespace (the Lens already has one: DS:5126-5209);
   - its activator;
   - its LaunchCap;
   - its process handle;
   - its **frame**: where its window is, its size, and its place in the stacking order;
   - its state: `Shown`, `Minimised` or `Dead`.

   **The session loop gives each client its own turn.** Each turn drains that client's channel (a bounded number of
   requests) and answers it with the same `refuse()` path (§1.6), so a busy client can't starve another. Arthur stays
   entry 0, and his control bytes (`B/G/M/K/D/I`) stay his.
2. **Side-by-side windows (J.c., 2026-10-29, §8 D1).** Every `Shown` client's window is on screen at once, each in its
   own frame, overlapping in a stacking order.
   - **The WM composites every shown window**, bottom to top, with the focus light on the focused one only. A shown
     client's commits are presented at once, focused or not: a terminal keeps scrolling behind the editor.
   - **Minimised** windows are off screen and keep running; their commits are accepted and not presented. The Spine
     chip shows a running, minimised app. L2's locked state hides every window the same way (OT-66 THINK §5.2).
   - **Frosted glass under each window samples the backdrop, not the windows beneath it.** That keeps today's cached
     blur: one blur per frame per move or resize, never per commit of the window underneath. A frost that showed the
     windows beneath would re-blur whenever any of them committed, which the CPU path can't afford. Reduce
     Transparency draws opaque frames, as now.
   - **Layout: floating with snap (J.c., 2026-10-29, §8 D1).** Windows move by the title band and resize by edges
     and corners, as Arthur's window does today (DI-1b), now per window. **Dragging a window to the left or right edge
     of the work area snaps it to that half**; dragging it away restores its size. A snap is a resize like any other: the
     WM publishes the size in the client's configure mailbox, and the outline stays until the client acks it.
   - **The cost:** the WM maps each shown client's buffer read-only (up to 4 MiB each under `desktop-resize`, so 24 MiB
     for six) and composites N frames in a present instead of one. Damage stays per window: a commit repaints its
     window's rect, plus whatever lies above it in the stacking order.
3. **Focus: click to focus, then raise.** The keyboard goes to the focused window only. Every focus move is still
   `hand_over` (swap → discard → present) between two activators, whoever owns them:
   - **a click on an unfocused window** focuses it and raises it to the top. The WM consumes that click: the app
     doesn't receive it, so a click meant to focus never presses a button;
   - a Spine chip click or Enter on a chip: launches, or focuses and raises (un-minimising if needed);
   - **Super+Tab:** the next shown window, in stacking order, focused and raised;
   - **Super+Space:** the Spine;
   - **Super+Escape:** Home (both already chords);
   - `OP_YIELD_FOCUS`: app → Home.

   The F1 rule (no key typed for one owner reaches the next) holds per handoff, as now.
4. **The pointer hit-tests across windows.** The WM finds the topmost frame under the pointer:
   - **its title band and edges** are the WM's: move, resize and snap;
   - **its surface**, if the window is focused: presses, drags and the wheel go to that client, as Arthur's do today.
     **Hover motion** is forwarded too, as kind-3 records with no button change: the terminal needs it for mouse
     reporting, and Files for hover highlight;
   - **its surface, if the window isn't focused:** a press focuses and raises (item 3); hover is not forwarded;
   - **outside every window:** the WM's (the Spine, Home).

   A press-grab stays with the window it started in until the release, wherever the pointer goes, as `Grab::Client`
   does now. It resets if that window's client dies or is minimised (CP-52, below).
5. **Launch by manifest.** The broker's `judge` reads the manifest and calls `sys_spawn_client(binary, caps)` with
   exactly the listed caps. Then it runs the handshake Arthur's code does. Each spawn gets a LaunchCap only if its
   manifest says so: Files does ("open with"), the terminal doesn't. Module ids stay an enumeration:
   - `MODULE_TERMINAL = 4`
   - `MODULE_FILES = 5`
   - `MODULE_SETTINGS = 6`
   - `MODULE_EDITOR = 7`
6. **One instance per app** in the MVP. Launching a running app focuses and raises its window, un-minimising it.
7. **The WM never blocks on a client (Chad #144 item 2).** A kernel chord only latches: the WM has to *run* to act
   on Super+Space, Super+Escape or the Spine's End (DS:4579). Today three things can park it on one app:
   - a reply is a blocking `sys_send` (DS:2036-2049), so an app that sends a request and then never receives parks
     the WM in the reply;
   - the handshake blocks in `recv_request` and `recv_control` (DS:1139), so an app that never finishes `CREATE`
     parks the WM there;
   - the surface grant (`grant_surface`) and the `R` before the buffer grant are rendezvous sends too.

   So "kernel chords" alone don't keep the user out of a hung app's trap. The design:
   - **A non-parking send, grant and grant receive (rung A0k, kernel).** `SYS_TRY_SEND` and `SYS_TRY_GRANT` complete
     only if the peer is already parked in its receive on that channel, and `SYS_TRY_RECV_GRANT` only if the peer is
     already parked in its grant (the WM receives a client's buffer and activator by grant). Otherwise they return `EAGAIN` and change nothing. They
     never park, so they add no wait queue and no park-teardown class. A well-behaved client sends its request and
     parks in its receive for the reply, so the WM's try normally succeeds on the first turn after the request is
     taken.
   - **Per-client reply state.** Each client entry holds at most one pending reply (the protocol is one request in
     flight per client) and one pending grant. A pending reply blocks that client's next request (the WM doesn't
     take it), never the WM. Each loop turn retries every pending reply once. While any reply is pending, the loop's
     wait has a one-tick deadline (bounded latency, and no spinning when nothing is pending).
   - **A handshake state machine.** `CREATE`, the `B` control, the import's `R` and the buffer grant become steps of
     the client's entry, each taken with `SYS_TRY_RECV` or the try ops above, with a deadline (5 s from the spawn).
     A client that misses it is ended (item 8).
   - **Unresponsive clients.** A reply still pending 5 s after its request marks the client hung: its chip says so,
     and the Spine's End `sys_kill`s it. The WM never waits for it. (A1a ends it at the deadline; A2's chip shows it
     first.)
   - **The one message the WM sends first** is the route handoff's `K` (the discard step). A try lands only on a parked
     receiver, and two pollers never meet, so the client protocol gains a rule: **a client out of front parks in its
     session receive**. It has nothing else to receive then: no key reaches it, and its focus answers `EPERM`, which is
     how it knows. In front it polls, keys and the session channel both. The `K` try is bounded (1 s), and its failure
     ends that client. The one gap: a client switched away and back faster than it polls once is still polling when
     `K` comes, and is ended. Publishing the handoff state in the client's mailbox (as configure is) closes it; that
     rides to A1b.

   With that, the liveness promise is exact: **however an app behaves, the WM keeps running, so the chords and the
   Spine's End keep working.** The kernel chords are what makes the keys reach the WM; the non-parking ops are what
   keeps the WM free to read them. A1a's scenarios include an app that never receives its reply and one that never
   finishes its handshake.

   *Rejected:* a reply thread per client (the WM's threads need `browser-runtime`, which no product kernel has, and
   a second thread would undo the single-threaded WM that §2.3's pitfall pass leans on); reversing every handoff so
   the WM only ever receives (it would still need A0k's non-parking grant receive, and every client's protocol would
   change).

8. **Death is local, for every way a client can fail (Chad #144 item 3).** A dead or failing app's entry goes to
   `Dead`:
   - its channels are closed first, so anything the app has parked on them returns with an error;
   - its LaunchCap is revoked;
   - it is `sys_kill`ed if it hasn't exited, and reaped with `sys_wait(WAIT_FLAG_WNOHANG)` on later turns until it
     has; cleanup never waits on the app;
   - its chip shows it is not running;
   - its frame's rect is repainted from what lies beneath it;
   - if it was focused, focus goes to the next shown window in stacking order, or to Home through `return_home`'s path
     when none is left.

   Every current exit path, and what it becomes:

   | Today (DS at `da8707a3`) | Becomes |
   |---|---|
   | A handshake failure, codes 5-21 (DS:1139-1271) | **That client** ends |
   | A dead session or surface channel, 24 (DS:1370) | **That client** ends |
   | A nonzero exit, 30 (DS:1726-1731) | **That client** ends (it has already exited: reaped) |
   | A reply-send failure, 28/29/34 (DS:1423, :1555, :1597, :1627, :1642, :1682): the app closed its surface or exited between its request and the reply | **That client** ends. With the try ops it is `EAGAIN` (pending, item 7) or a dead channel (this row) |
   | A broker or launch error (DS:1614-1625): the target won't spawn, or the requester can't be answered | **The launch** fails: the requester gets a status, and only a requester that can't be answered ends |
   | A nested Lens error (DS:1662-1664) | **The Lens** closes and its owning client is told; nothing else ends |
   | `Fault::Wm` (a fault in the WM's own state: its registry, its own buffers, its present) | **The WM** exits, as now |

   Only the last row is WM-fatal. A1a's gate adds a disconnect-during-reply leg (the probe app closes its surface
   after a request, before the reply) and a Lens-failure leg, beside the handshake and nonzero-exit legs.
   - A hung app is ended from the Spine, which `sys_kill`s its process handle (`SYS_KILL = 7`; the WM holds the handle
     from the spawn). Item 7 is what keeps the Spine able to do it.
9. **The WM's loop: poll in A1, park later, with a budgeted wait set (Chad #144 item 4).** Image A's WM polls today
   with `SYS_TRY_RECV`, because `wait-multiplex` (OT-22 rung 1, `desktop-park`) is not among
   `PRODUCT_IMAGE_KERNEL_FEATURES`.
   - **The budget doesn't fit today's primitive.** Each client has two channels (session and surface). Six clients
     are 12 items; with the pointer route, Home's focus and the SAK latch, 15. `SYS_WAIT_ANY` takes at most
     `ITEMS_MAX = 8` (`kernel/src/sched/wait_any.rs:46`); a longer set is `EINVAL`.
   - **A1 polls.** Every turn try-receives each live client's two channels, in a rotating order (the first client
     served moves by one each turn), and takes at most one request per client per turn. That bounds a turn's work
     (six clients, two channels) and keeps one busy client from starving the others. Polling costs CPU while idle,
     not correctness.
   - **Parking is a later rung (A3p).** It raises `ITEMS_MAX` to 16 (15 items plus one spare), with a discriminator
     for the bound: a 16-item wait parks and wakes on its last item, and a 17-item wait is `EINVAL`. Only then does
     `desktop-park` join image A.

**Kernel changes for the app model: one small one, A0k** (Chad #144 item 2): `SYS_TRY_SEND`, `SYS_TRY_GRANT` and
`SYS_TRY_RECV_GRANT` (item 7). Everything else is in place: the routes, handoffs, chords and pointer forwarding. The rest of the work is in
neo-init and libneo. The parked loop's larger wait set (A3p, item 9) is a second, later one. K2 (`SpawnCap`) and K1b
(`open_at`) are separate OT-66 rungs this design uses but doesn't change.

**Pitfall-bank pass for §2.3 (`docs/concurrency-pitfalls.md`).** The WM is single-threaded. A0k is the one kernel
change: it takes a channel's state lock and hands a message to an already-parked receiver, the path `sys_send` already
runs, but never parks itself. A0k's own pass is the CP-19…CP-28 handoff and wake classes (it wakes a parked
receiver) and CP-67 (it holds the channel only inside the call); its gate is the AAG with a cross-family lens. The
classes the WM is nearest to, and why each is N-A or guarded:
- **CP-52, a stale pointer grab:** designed out by the B-ref (no kernel grab). With several windows, the WM holds one
  grab at a time, naming its window's table index. It must reset when that client dies or is minimised, and a move,
  resize or snap grab must reset the same way. A1 needs a scenario where an app dies mid-drag, with another window
  shown.
- **CP-57, a server-initiated rendezvous send:** the WM never sends first to an app. Each client's configure (a move
  that resizes, a snap) goes through that client's own seqlock mailbox, as Arthur's does today. That alone doesn't
  cover replies: a reply to an app that doesn't receive it parks the sender just the same (Chad #144 item 4). So every
  WM send and grant to a client is a try (item 7). *As built in A1a (PR #15, Chad's re-checks):* every reply, the
  probe app's handshake and handoff, and Arthur's permission lend and resume are tries against a deadline, with the
  other clients' turns and the WM's own input served between retries. Three waits remain, by design: Arthur's own
  handshake (`G`, `M`, the launch handoff) still blocks, before any other client exists; the launch proof is bounded
  (3 s) with the pointer served; and Home and the permission prompt wait on the user, not on a client. A1a's legs: an app that never receives its reply, and one that
  never finishes its handshake; the WM serves another client and the chords throughout.
- **CP-14, a lost wake between check and park:** applies once the loop parks (A3p). A client's request that arrives
  between the WM's last try-receive and its park must still wake it. `SYS_WAIT_ANY` checks every item's readiness
  after queuing itself, the recheck that closes this; A3p's discriminator sends on the last item in the window.
- **Fairness, bounded service:** a single-threaded WM can still starve a client by always serving another first.
  Item 9's rotating order and one request per client per turn bound it; A1a's legs include two clients committing
  at once, each served every turn.
- **CP-61, a wait naming a reused slot:** applies if `desktop-park` goes into image A. A dead client's handle slot must be
  removed from the WM's wait set before it is closed, so the slot can't be reused under it. Covered by the existing
  `wait-multiplex` identity check.
- **CP-54, unbounded lookups:** clients are capped at N = 6, under the kernel's 32 routes. Each loop turn takes at most
  one request per client (item 9), and a hit-test walks at most six frames.

### 2.4 What an app may hold

| Cap | Who | How |
|---|---|---|
| Its session channel and surface | every app | spawn, as Arthur's |
| Its own focus and activator | every app that takes focus | it creates them; the WM gets ACTIVATE-only |
| A directory cap | Files (the home, or `/neofs` before L3); Settings (none; it asks the WM, §5); the editor (none at launch; files arrive by grant, §6) | spawn, from the manifest; derived and narrowed (K1b) |
| A file handle | the editor, or the terminal's child | granted by the broker on "open with" (§4.3) |
| A LaunchCap | Files ("open with"); Arthur (as today) | the manifest |
| A `SpawnCap` | the terminal, for its shell only | K2: derived by the WM, allowlist `[shell]` |
| The clipboard | later | through the WM (§6.2) |

**No app holds:** the root directory cap, `Handle::InputSystem`, the installer cap, or another app's anything.

## 3. Terminal

### 3.1 The pieces

The terminal is four pieces (`Plans/SURVEY-2026-10-27-ot88-terminal-emulator.md`):
1. a parser;
2. a screen model;
3. a renderer;
4. a PTY seam, plus key encoding.

**Take the survey's leaning:** depend on or vendor **`vt100`** (MIT, `doy/vt100-rust` `fb0f54ec7222`, 2026-10-19). It
brings **`vte`** (Apache-2.0 OR MIT, `alacritty/vte` `89c9c22f5b8f`, 2026-10-25). The heads were re-checked on
2026-10-29 and are unchanged. `vt100` gives the parser and the screen, so ours are:
- **The renderer:** `vt100::Screen` cells in Plex Mono through neo-text onto the terminal's surface. Damage comes from
  `vt100`'s screen diff, sent with `OP_COMMIT_DAMAGE`.
- **Key encoding:** HID usage plus modifiers → bytes, as a table (§3.3).
- **The PTY seam** (§3.2).

Rung T2 checks the survey's three open questions first:
- `vt100` builds on our userland target with `std` off;
- its sequence coverage against the T1 shelf's crossterm output;
- its scrollback model.

### 3.2 The PTY seam

**Today's stdio is global.** fd 0 reads serial or USB-HID input, and fd 1 goes to serial and the framebuffer console
(`kernel/src/arch/x86_64/syscall.rs:6045-6066, :6183-6186`). There is no per-process stdio. Channels are **synchronous
rendezvous** (`kernel/src/ipc/mod.rs:2`, ADR-004), with `SYS_TRY_RECV` but no try-send.

The two designs:

| | **T-a: a kernel pipe object** | **T-b: userland rings in a shared buffer** |
|---|---|---|
| What | A new `Handle::Pipe`: a bounded byte buffer. A write blocks only when it is full, a read only when it is empty, and both are waitable with `SYS_WAIT_ANY`. Two per terminal: in and out. | Two single-producer rings in one `sys_buffer`, plus a doorbell. |
| CP-57 (rendezvous deadlock) | **Designed out:** neither side's write waits for the other to read, until the buffer is full. | The doorbell is the hazard. A rendezvous doorbell recreates CP-57; a polled one is CP-58's "rescuer" smell, and burns CPU while a TUI is idle. |
| Kernel work | Yes: a new handle type, its wait-queue integration, and teardown on kill. | None. |
| What std apps need | `std::io::stdin().read` blocks: a blocking pipe read. | A blocking read can use the futex, which already wakes across processes: waits and wakes are keyed by the shared word's **physical** address, not the pid (`kernel/src/sched/futex.rs:63`, :278, :293; the syscalls at `kernel/src/arch/x86_64/syscall.rs:6842`, :6869). Publish the ring's sequence with Release, `FUTEX_WAIT(expected)` on the consumer, `FUTEX_WAKE` on the producer; the futex's compare under its queue lock closes the lost-wake gap (Chad #144 item 5). What it doesn't give: EOF when the last writer goes, EPIPE, endpoint accounting, or `SYS_WAIT_ANY` integration. |

**Recommend T-a, the kernel pipe.** T-b can block and wake (the futex is cross-process), so that is no longer the reason.
T-a wins on what T-b would have to rebuild in userland: EOF when the last writer closes and EPIPE when the last reader
does, both through process death; endpoint and charge accounting; and a pipe item in `SYS_WAIT_ANY`, so the terminal
parks on its child's output beside its input. Those are kernel-side facts about endpoints, so they belong in the
kernel (rung T1).

Its pitfall-bank classes, each with the discriminator T1 must ship:

| Class | How a pipe meets it | The discriminator |
|---|---|---|
| **CP-33**: kill drains only some wait queues | A reader parked on an empty pipe whose process is killed | A scenario: kill a child parked in `read` and one parked in a full `write`; both reap, and no zombie stays |
| **CP-67**: an owned `Arc<Channel>` held across a park | The same shape with `Arc<Pipe>` | The CP-67 repair's pattern (`with_channel_anchor`), copied for the pipe, plus the per-kill leak counter |
| **CP-61**: a wait naming a reused slot | A pipe handle closed and reused under a `WAIT_ANY` | The identity re-check that `wait-multiplex` already does, extended to the pipe item |
| **CP-85**: exit retiring one thread | A threaded std child exiting with a reader parked | The C1.3 sibling drain, applied when the pipe's peer exits: a reader gets EOF, a writer gets EPIPE |
| **EOF and EPIPE ordering** | The terminal closes while the child writes | A KAT for the state machine: last writer closed → reader gets EOF after the buffer drains; last reader closed → writer gets EPIPE at once |
| **CP-14**: a lost wake at empty→non-empty and full→non-full | A reader checks empty, a writer writes and wakes, then the reader parks | The check and the enqueue on the wait queue are one critical section under the pipe's lock. Discriminator: a two-CPU leg with the write forced into that window (an armed point, as `lock_route_races` does); the reader wakes |
| **Wake on the last endpoint** | The last writer is a process killed while a reader is parked | Endpoint counts drop in the handle's teardown, and the last one wakes every waiter. Discriminator: kill the only writer while the reader is parked; the reader gets EOF |
| **Charge and endpoint counts across grants and threads** | An end granted to another process, or held by two threads | Counts are per endpoint handle, not per process. Discriminator: grant the write end away and close the original; the reader still waits; close the granted one and it gets EOF. The object charge returns to zero after both close |

**The line discipline lives in the terminal, in userland.** No kernel tty layer:
- **Raw mode** is the default: bytes go to the child as typed.
- **Cooked mode** (echo, backspace editing, Enter delivers the line) is the terminal's own small state machine, for
  children that want it. `neo-shell` does its own line editing already (`userland/neo-shell/src/main.rs:1144`).
- **Ctrl+C** sends `0x03`. The OS has no signals, so if the child doesn't stop, the terminal's **End process** (Ctrl+Shift+C,
  or a menu) `sys_kill`s the child it spawned and holds the handle for.
- **The window size** goes in a seqlock mailbox in a small shared buffer, the CP-57-free pattern resize already uses
  (DI-1b). The std overlay's terminal-size query reads it.

**How the child gets its stdio: chosen explicitly, never by slot (Chad #144 item 5).** A convention on handle slots 0
and 1 can't be the kill-switch: today's spawns already inherit **channels** in those slots (DS:1910), and `sys_write(1)`
doesn't consult the handle table at all (`syscall.rs:6043`). So:
- **The spawn names the stdio.** The terminal's spawn passes an explicit stdio selection (a spawn flag, or a stdio
  descriptor beside the inherited handles) naming which inherited handles are its stdin and stdout pipes.
- **The kernel routes by type.** fd 0 and fd 1 go to a pipe only when the process's stdio selection names a
  `Handle::Pipe`. Otherwise they keep today's global path. A channel inherited at slot 0 or 1 stays a channel.
- **The std overlay asks, not assumes.** `stdio_neoplutarch.rs:44` returns Unsupported for stdin today, so there is no
  "global stdin" for std apps. With a pipe selected, stdin and stdout read and write it; without one, stdout keeps
  today's path and stdin stays Unsupported.
- **The kill-switch leg.** With `pipe` off, and with it on for a spawn that selects no pipe, every existing scenario is
  unchanged, including the ones whose children inherit ordinary channels at 0 and 1 (Arthur's spawn).

### 3.3 Key encoding

- A table: HID usage (`InputEventV1` kind 1) plus modifiers → the bytes an xterm-compatible terminal sends.
  - Printable keys use the layout's ASCII.
  - Ctrl+letter is the C0 control.
  - The arrows, Home, End, PgUp, PgDn, Insert, Delete and F1-F12 use the xterm sequences, with modifier parameters.
- **Byte oracle:** a hand-laid table in T2's tests for every non-printable key, with and without each modifier, taken from
  xterm's `ctlseqs` **fetched raw** (the exact-bytes rule), never from memory or a summarizing fetch.
- **Application cursor mode** (DECCKM) switches the arrows between `CSI` and `SS3`. `vt100` tracks the mode; the encoder
  reads it.

### 3.4 What runs in it

1. **MVP: `neo-shell`.** Its commands are `help ls cat pwd cd echo status arthur spawn exit`
   (`userland/neo-shell/src/shell_logic.rs:8`).
   - Its stdio moves to the pipe convention.
   - It gains file commands once NeoFS has them (`rm`, `mv`, `mkdir`; §4.2).
   - It is a REPL, which suits cooked mode.
2. **After the MVP: the T1 shelf** (crossterm apps: Helix, gitui, yazi, any ratatui app;
   `Plans/RESEARCH-2026-07-11-rust-day1-apps-FINDINGS.md:17-43`). They need:
   - the std overlay's blocking stdio and terminal size (above);
   - loading an ELF that isn't a boot module. Spawn finds Limine modules only (§1.1), so a shelf app must either be a
     boot module or wait for disk-backed spawn, which nothing designs yet. This is a gap OT-88 does not close.

## 4. Files

### 4.1 What NeoFS can do today

NeoFS lives only in the kernel (`kernel/src/fs/neofs/`). Userland reaches it through `SYS_OPEN 12`, `READ 13`, `CLOSE 14`,
`FSYNC 15`, `READDIR 16` and `FTRUNCATE 23` (`syscall.rs:104-115`). Writes are `sys_write(fd, …)`.

| A file manager needs | NeoFS today | Where |
|---|---|---|
| List a folder | **Yes.** `readdir` gives a name, a kind (Directory, Regular, Symlink, Model) and a cursor | `libneo/src/lib.rs:1161-1194` |
| Sizes, dates | **No.** No `stat`; the inode format has no timestamps | `fs/neofs/format.rs:304-324` |
| Open a file | **Yes**, by path through the root form; K1b's `open_at` is unbuilt | `fs/mod.rs:1333-1345` |
| Create a file | **Yes**, `OPEN_CREATE_REGULAR`, not into the bare root | `fs/mod.rs:30-31, :1345` |
| Write a big file | **No.** An ordinary write is a whole-file copy-on-write capped at 8 direct blocks (32 KiB). Larger files come only from `create_file_from_bytes`, behind `neofs-large-write` | `fs/neofs/inode.rs:565-577, :1020-1028`; `mount.rs:134-158` |
| Rename | **No** | no syscall |
| Delete | **No** | no syscall |
| New folder | **No** from userland. `NeoFs::create_directory` exists behind the `installer` feature | `fs/neofs/mount.rs:161-174` |
| A folder of any size | **No.** At most 64 entries: one 4 KiB block of 64-byte dirents | `inode.rs:1402-1408`, `format.rs:15` |
| Long names | **No.** At most 47 bytes of UTF-8 | `format.rs:326-336` |
| Symlinks | Rejected on read | `format.rs:57` |

`mkdir` and `unlink` are already banked as an OT-67 prerequisite
(`Plans/QUEUE-2026-08-07-single-lane-priority-order.md:242-243`).

### 4.2 The NeoFS rung (F0)

A kernel rung, in K1b's directory-handle form, so each call is bounded by the handle it is given:

| Call | What | Rights it needs on the directory handle |
|---|---|---|
| `mkdir_at(dir, name)` | a new empty folder | WRITE |
| `unlink_at(dir, name)` | remove a file, or an **empty** folder | WRITE |
| `rename_at(dir, name, dir2, name2)` | move or rename within one volume | WRITE on both |
| `stat_at(dir, name)` | kind, size, and whether a folder is empty | LOOKUP |

- Each is one commit through #180's commit barrier (`c805f2b2`): the superblock is written last, and freed blocks are not
  reused until durable.
- `rename_at` over an existing name is **refused** in the MVP (no silent replace). The editor's atomic save no longer
  needs it: W1's `SYS_FILE_REPLACE` (§6.1) replaces a file's content in one commit. So F0 has no `replace` option.
- **Byte oracle:** a hand-laid directory block before and after each operation (the format rule).
- **Crash cuts:** a scenario cuts power after each write of each operation. The tree must be either before or after,
  never between, as #180's scenarios do.
- **Kill-switch:** `neofs-dir-ops`. Off means none of the four calls exists, and today's syscall table is unchanged.

**Not in F0:** timestamps (a format change; §8, D4), folders over 64 entries, names over 47 bytes, and writing files over
32 KiB through the ordinary path. W1's `SYS_FILE_REPLACE` (§6.1) covers large files for the editor, so this no longer
gates OT-67. Files' copy could use it too, once W1 is merged.

### 4.3 The MVP file manager

- **Browse:** from its directory cap, with `readdir` and `stat_at`. Folders first, then files, by name. The Lens's code
  (`neo-worker/src/lens.rs`, read-only, `LOOKUP | READDIR`) is the starting point.
- **Open:**
  - A folder opens in place.
  - A file is opened **by Files, and handed on by the broker** (Chad #144 item 10). A directory row is a name, not an
    open file, so someone must open it. Files does, from its own directory cap: it opens the selected name with the
    rights the target app's manifest asks for (READ, plus WRITE for the editor), and that file handle is the only
    thing it grants. The broker never holds a directory.
  - **`OP_OPEN_WITH`** is Files' request on its session channel: `{module_id: u16, rights: u32}`, followed by the
    grant of the one file handle. The broker checks `module_id` is a manifest app that accepts files, and that the
    handle's rights are no more than that manifest names. Then it launches or switches to that app and grants it the
    handle.
  - **Failures:** an unknown or non-file-taking `module_id`, or rights beyond the manifest's: the broker refuses with a
    status and drops the handle (Files keeps nothing either). The target can't be launched: refused, handle dropped. The
    target exits before the grant: the same. A grant the target never receives is pending (§2.3 item 7) and dropped
    with the target if it is ended. Files shows each refusal in its own UI.

  This is the powerbox pattern: the app opening a file never sees a directory. SerenityOS's FileSystemAccessServer is
  the same idea: an app asks, and a trusted service hands it one fd. The MVP's mapping is fixed:
  - text → the editor;
  - anything else → "no app opens this";
  - `.neom` → nothing; models are Arthur's.
- **Rename, delete, new folder:** with F0's calls, through Files' own directory cap.
  - Delete asks once, in a confirmation the app draws. There is no trash in the MVP.
  - A non-empty folder can't be deleted until it is emptied (F0's `unlink_at` refuses it).
- **Copy:** read and write through the 32 KiB path, so only small files copy in the MVP. Files says so instead of failing
  quietly.

**What Files can't do in the MVP:** dates, big-file copy, folders over 64 entries, long names, trash, search and
thumbnails.

### 4.4 Files and the Lens

The Lens is DS-1's read-only directory instrument, launched from inside Arthur. Files does what it does and more. Whether
Files replaces the Lens on the Spine is a product call (§8, D2).

## 5. Settings

### 5.1 What the MVP sets, and where it lives today

| Setting | Today | MVP |
|---|---|---|
| **Reduce Transparency** | A cargo feature, or the **presence of** `/neofs/atrium-reduce-transparency` at boot (DS:2462-2481) | A switch, applied live by the WM |
| **Reduce Motion** | A cargo feature, or the presence of `/neofs/atrium-reduce-motion` (DS:2901-2932); profiles Full / Reduced / Off (`neo-atrium/src/motion.rs:42-50`) | A three-way choice, applied live |
| **Idle lock time** | Not built. L2 builds the timer (default 10 minutes, J.c.'s call; OT-66 THINK :312, :441); **L4** is the settings hook | Off / 5 / 10 / 30 minutes, once L2 and L4 land |
| **Change password** | Not built: OT-66 L4 | A button that opens the WM's own password view (§5.3) |
| **Network** | eth0 and RTL8125 run DHCP at boot with fixed attempts (`drivers/usb/boot_eth0.rs:25-26, :103-110`; `rtl8125.rs:872`); no static IP; WiFi credentials are compile-time and test-gated | **Read-only:** the link, the address, the gateway. A "renew" button only if a kernel DHCP-renew call exists (it doesn't yet; §8, D3) |
| **Display** | No setting. The kernel takes Limine's framebuffer (`kernel/src/main.rs:527-545`); `limine.conf` has no resolution line | **Read-only:** the resolution. Changing it means writing the boot partition's `limine.conf`, which is FAT, not NeoFS, and is the installer's job (§8, D3) |

### 5.2 Where settings are stored

- **Today's store is presence files** at the `/neofs` root. OT-50's unified config substrate has a THINK
  (`docs/THINK-2026-07-10-ot50-unified-config-substrate.md`) and no build.
- **The MVP store:** one small typed key-value file per scope:
  - machine settings in `.system/settings` (OT-66 seeds `.system/`: THINK :359);
  - after L3, user settings in `home/<account>/settings`.

  The format is fixed-size records (`NEOSET1`): a magic, then `{key id u16, type u8, value}` entries. It gets a hand-laid
  byte oracle and the same torn-write cut as `NEOCRED1` (two copies, newer-valid wins).
- **This is OT-50's first slice**, scoped to the keys above, and built so its views rule holds: the settings app, Arthur
  and `neo>` are all views over the same store, never separate copies.
- **Migration:** the first boot that has the store reads the two presence files once, writes the store, and leaves the
  files. A later rung removes them.
- **As built (S1 host crate, `userland/settings-core`):** the layout is a 20-byte header (magic `NEOSET1\0`, version,
  entry count, generation), 8-byte entries (`key u16, type u8`, a reserved zero byte, `value u32`), then a 16-byte
  BLAKE3 checksum. The two copies are `.system/settings.a` and `.b`. Keys: 1 Reduce Transparency (bool), 2 Reduce
  Motion (Full, Reduced, Off), 3 idle-lock minutes (0, 5, 10, 30; default 10). An unknown key, a wrong type or an
  out-of-range value refuses the whole record, which then reads as the defaults (the idle lock on).
- **As built (S1 WM side, neo-init `atrium-settings-store`):** the WM reads the store at boot, before it builds the
  scene. With no record it migrates the presence files and writes the first one. `OP_SETTING` (opcode 11, libneo
  `atrium-settings`) is checked against the key table, written to the file not holding the record, read back, then
  applied live: Reduce Transparency through `BaseScene::set_mode`, Reduce Motion as the scene's profile, and the
  idle time read by L2's idle timer. A bad key or value is refused in the reply's status (20, 21; 22 if the write
  didn't land) and changes nothing.
- **Who may send it (the PM's #222 bounce):** only the client the WM launched as the settings app (module `settings`);
  any other gets `CapRequired` (7). Only Reduce Transparency and Reduce Motion are client-writable
  (`settings-core`'s `Key::client_writable`); the idle lock is WM-only (23): a client that could set it to 0 would
  turn auto-lock off for good. S2's idle-lock control is the WM's own view, like Change password. Client writes are
  rate-limited (a bucket of 4, one back a second; 24), and nothing is set under the lock screen (4). A write that
  landed but read back wrong is resolved by reading the store again, as the next boot will.
- **Gates:** `login_settings_store` (Arthur as the boot test's stand-in settings app: live apply, store digests, the
  refusals, the rate limit, the idle lock at the WM-set time, the record read after a reboot), `login_settings_refused`
  (Arthur as itself: both requests refused, nothing written, the idle lock at the default) and `login_settings_cut`
  (L1's cut on a settings write).

### 5.3 Who writes a setting

**The WM owns the store and is its only writer.** The keys it applies are Reduce Transparency, Reduce Motion and the
idle time, and the WM holds the `.system/` directory cap.
- The settings app holds **no directory cap**. It sends `OP_SETTING {key, value}` on its surface channel.
- The WM validates the value against the key's type and range, writes the store, applies the change live, and replies.
  A bad value is refused (refuse-and-continue).
- This is SerenityOS's ConfigServer shape: one service writes, and clients ask it.

**Change password never passes through the settings app.** The button sends `OP_SETTING {key: password, value: none}`.
The WM then opens its own password view, the same trusted view as the login screen (OT-66 §7), with the WM's own focus
and the settings app held. The app never sees the old or new password.

### 5.4 Settings as an app

Settings is a client like the others (Design B, `MODULE_SETTINGS = 6`): its own surface, keys and pointer, and no
directory cap. Whether the Spine has a Settings chip, or Settings is reached from Home, is part of §8, D2.

## 6. The editor (OT-67), and the clipboard

### 6.1 How the editor fits

The editor's own design is a later THINK, after the text engine's. This section is only its place in the app model:
- **A Design B client:** `MODULE_EDITOR = 7`, `neo-editor`, with a `ROLE_TEXT` surface.
- **Files reach it by grant.** "Open with" from Files hands it one file handle (§4.3).
  - **Save** writes through that handle.
  - **Save as** asks the broker for a save location: a WM-drawn picker over the user's home, which hands back one new
    file handle. That is the powerbox again; the editor holds no directory cap.
- **What it needs from NeoFS:** one call, W1's `SYS_FILE_REPLACE` (`docs/THINK-2026-10-29-ot67-editor.md` §5.2, #209).
  It replaces a file's whole content in one commit, through the indirect layout, so it covers files over 32 KiB and the
  atomic save together. No temporary file and no rename. It is a prerequisite of OT-67, not of OT-88.
- **Overwrites no longer leak** (#206, merged as `78991d10`): each commit frees the version it supersedes, so repeated
  saves keep the volume's used count constant.
- **Text engine:** the editor's MVP ships on T0 (neo-text) with stated limits: left to right, no shaping, every byte
  preserved (`docs/THINK-2026-10-29-ot67-editor.md` §1, §4.1). T2 (rustybuzz, BiDi and fallback;
  `docs/THINK-2026-07-10-atrium-text-engine-research.md:187-201`) replaces its text layer later, and is not a
  prerequisite (Janus #145 item 6).

### 6.2 The clipboard (after the MVP's first cut)

Nothing exists today. When it comes, it goes through the WM, which keeps the one clipboard:
- **Copy** sends `OP_CLIP_SET` with the bytes.
- **Paste** is `OP_CLIP_GET`, answered only on a paste key the WM itself saw delivered to that app (the F1 handoff gives
  it this). So an app in the background can't read what the user copied.

This is noted so A1's client table leaves room for it. It is no rung in this THINK.

## 7. Rungs

The units are dependencies, iteration cycles and gates (CLAUDE.md). Every build rung needs the 4-shard gate and its own
cross-family read. The rungs that touch kernel or security code also need the AAG and the pitfall-bank pass.

**Order:** A0 → A0k → A1a → A1b → A1c → A2. A3p (the parked loop's 16-item wait set) can follow A1a at any point.
- A1 is split three ways (J.c.'s D1: side by side, floating with snap). **A1a is the part that is the same whatever the
  layout:** the client table, a turn per client, per-client faults and routing. A1b puts N windows on screen; A1c makes
  them move, resize and snap.
- After A1c, three chains can run in parallel:
  - T1 → T2;
  - K1b → F0 → F1;
  - S1, which needs L1. Its idle-lock and password rows need L2 and L4.
- OT-67 (the editor) starts after A1c and F0, plus the text engine's THINK. Its big-write NeoFS change is W1
  (`SYS_FILE_REPLACE`, #209).

| Rung | What | Depends on | Cost | Close gate | Kill-switch | Kernel? | Muse? |
|---|---|---|---|---|---|---|---|
| **A0** | This THINK and a cross-family read | — | 1 doc, 1 review | PM merge after the review | — | — | — |
| **A0k** | `SYS_TRY_SEND`, `SYS_TRY_GRANT` and `SYS_TRY_RECV_GRANT` (§2.3, item 7): complete only with the peer already parked opposite, else `EAGAIN`, never parking (#232) | A0 | Low to moderate. Two syscalls on the existing handoff path | 4-shard gate: a try to a parked receiver delivers; a try to an idle or sending peer is `EAGAIN` and changes nothing; a try-grant installs exactly one handle or none; a two-CPU leg with the receiver parking as the try runs. **AAG with a cross-family lens** (the channel handoff and wake) | `ipc-try-send` (off ⇒ today's syscall table) | **Yes.** CP-19…CP-28, CP-67 | No |
| **A1a** | **The layout-independent part.** The client table (§2.3, item 1) with Arthur as entry 0; a turn per client in the session loop; per-client faults (§2.3, item 8: every communication, protocol and launch failure ends that client or that launch, not the WM; cleanup never waits on the client); the WM's sends and grants to clients through A0k's tries, with a per-client pending reply and a handshake state machine (§2.3, item 7); polling with a rotating order and one request per client per turn (§2.3, item 9); keyboard routing to the focused client by `hand_over`; the manifest and broker launch; `neo-probe-app`, a second focusable client. One window on screen at a time until A1b. Image A keeps polling (§2.3, item 9) | A0k | **The bulk of the generic work.** DS's Arthur paths made generic: the handshake's Arthur-specific bytes separated from the generic ones. Several scenario cycles | 4-shard gate: <br>- launch the probe app; keys reach only the focused client (each client's receive count is checked) <br>- Super+Tab between Arthur and the probe app with no stale key (F1) <br>- the probe app's handshake fails, or it exits nonzero: Arthur and the WM live on <br>- a hung probe app is killed from the Spine <br>- the probe app never receives its reply, and (a second leg) never finishes its handshake: the WM serves Arthur and the chords throughout <br>- the probe app closes its surface between a request and the reply: only it ends <br>- a Lens failure closes the Lens; its client and the WM live on <br>- two clients committing at once are each served every turn <br>- with the switch off, every desktop scenario's captures are unchanged | `atrium-apps` (off ⇒ today's Arthur-only WM) | No | No (no new authority) |
| **A1b** | **N windows on screen.** A frame per client and a stacking order; the WM composites every shown window, bottom to top, with the focus light on the focused one; a frost per window from the backdrop (§2.3, item 2); click to focus and raise (the focusing click consumed); hit-testing across frames, presses and hover to the focused window's surface; one grab at a time, reset on its client's death or minimise; minimise and restore from the Spine chip | A1a | Moderate to high: compositing and damage across overlapping frames, and the cost of N frames per present under TCG | 4-shard gate: <br>- Arthur and the probe app shown side by side, both committing; the capture shows both, the focused one lit <br>- a click on the unfocused window focuses and raises it, and the app does not receive that click <br>- keys reach only the focused window <br>- the probe app dies mid-drag with Arthur shown: the grab resets, Arthur's window is repainted whole, focus goes to Arthur <br>- a minimised app's commits are not presented <br>- frame-time witness for two windows against one | `atrium-apps` | No | No |
| **A1c** | **Floating with snap.** Move by the title band and resize by edges and corners per window (DI-1b, generalised from Arthur's one window); a drag to the left or right edge snaps the window to that half, and a drag away restores its size; each resize and snap through that client's configure mailbox | A1b | Moderate. DI-1b's paths made per window; the snap is new | 4-shard gate: <br>- move and resize each of two windows, read back by capture <br>- snap one left and one right: two halves, read back by capture and by each client's configure ack <br>- a drag away from a snap restores the size <br>- an overlapped window's frost is unchanged when the window above it moves (it samples the backdrop) | `atrium-apps` | No | No |
| **A3p** | The parked loop: `ITEMS_MAX` 8 → 16 in `wait_any`, then `desktop-park` into image A (§2.3, item 9) | A1a | Low to moderate. One bound in SMP wait code | 4-shard gate: a 16-item wait parks and wakes on its last item; a 17-item wait is `EINVAL`; a send in the window between the last try-receive and the park wakes the WM (CP-14). **AAG** (wait queues) | `desktop-park` (off ⇒ polling) | **Yes.** CP-14, CP-61 | No |
| **A2** | Launching from the Spine and Home: chips for the apps (§8, D2); Home intents `terminal`, `files`, `settings`; the chip's running, minimised and not-running looks | A1c | Low. WM and neo-atrium | 4-shard gate: each launch path; a chip for an app that isn't running launches it, one that is switches to it | `atrium-apps` | No | No |
| **T1** | The kernel pipe (§3.2), libneo `stdio::Tty`, and the std overlay's stdio mapping | A1a (for the scenario's terminal; the pipe itself depends on nothing) | Moderate to high. A new handle type and its teardown on kill | 4-shard gate: the §3.2 discriminators; a pipe round trip under SMP; EOF and EPIPE KATs. **AAG with a cross-family lens** (it touches teardown and wait queues) | `pipe` (off ⇒ no handle type, today's syscall table) | **Yes.** The pitfall-bank pass in §3.2 | No |
| **T2** | The terminal: `vt100` vendored or depended on (after the survey's three checks), the renderer, the key encoder with its byte oracle, cooked mode, End process, running `neo-shell` | T1, A2 | Moderate. The renderer and the encoder are mechanical; the vt100 build check is the unknown | 4-shard gate: a scenario that types into `neo-shell` in the terminal and reads the grid back; a scripted escape sequence draws the cells the oracle expects; Ctrl+C and End process | `neo-terminal` module (off ⇒ not in the image) | No | No |
| **K1b** | OT-66's: `open_at` and the port (OT-66 THINK §8) | K1a | High (OT-66's own estimate) | OT-66's | `cap-rooted-open` | Yes | Yes |
| **F0** | NeoFS `mkdir_at`, `unlink_at`, `rename_at`, `stat_at` (§4.2) | K1b | Moderate. Four operations, each one commit, with crash cuts | 4-shard gate: byte oracles; a crash cut after each write; rights checked on the handle (a handle without WRITE can't delete). **AAG** (kernel rung; Chad #144 item 8) | `neofs-dir-ops` | **Yes.** Pitfall-bank pass on the NeoFS commit path and handle rights | Yes (rights checks) |
| **F1** | Files (§4.3): browse, open with, rename, delete, new folder, small copy | A2, F0 | Moderate | 4-shard gate: each operation, and open with into a test app that receives exactly one handle | `neo-files` module | No | No |
| **S1** | The settings store `NEOSET1` (§5.2), the WM's `OP_SETTING`, and the settings app with Reduce Transparency, Reduce Motion, and network and display read-only | A2, L1 (for `.system/`) | Moderate. One byte oracle and the WM's apply-live paths | 4-shard gate: each switch applied live and read back by digest; the torn-write cut; a bad value refused; the presence-file migration | `atrium-settings-store` (off ⇒ the presence files, as today) | No | No |
| **S2** | Idle lock and Change password in Settings | S1, L2, L4 | Low | 4-shard gate: the lock fires at the chosen time; the password view is the WM's, and the app's receive count is unchanged while it is up | `login` | No | **Yes** (the password path) |

**What OT-88's MVP needs:** A1a, A1b, A1c, A2, T1, T2, K1b, F0, F1, S1 and S2. The machine-time cost is roughly eleven 4-shard gates
plus their bounce cycles. T1, K1b and F0 are the three kernel rungs, so they set the critical path and the AAG load.

## 8. Decisions for J.c.

Taste and product calls only. Each has a recommendation.

**Decided (J.c., 2026-10-29): D2–D5 as recommended. D1 changed: side-by-side windows in the MVP, floating with snap.**

- **D1. One app in front, or apps side by side?** **Decided (J.c., 2026-10-29): side by side in the MVP.**
  - The first recommendation was one in front: each app filling the window area, with the Spine and Super+Tab
    switching. J.c. chose side by side instead. §2.3 and §7 are reworked for it, and A1 is split into A1a (the part the
    two designs share), A1b (N windows) and A1c (move, resize, snap).
  - **Layout. Decided (J.c., 2026-10-29): floating windows with snap.** Windows move, resize and overlap, in the
    design language's classic-Mac manner. Dragging one to the left or right edge snaps it to that half. Tiling was the
    alternative: the WM placing every window in a grid, with no overlap. It was not chosen.
- **D2. What's on the Spine?** **Decided (J.c., 2026-10-29): the recommendation.** Today it has Arthur and the Lens. The icons exist for Terminal, Files and Settings (#185).
  **Recommend:**
  - Arthur, Terminal, Files and Settings on the Spine.
  - **Files takes the Lens's place.** The Lens was a read-only Files; with Files there, two folder views on the Spine is
    one too many.
  - The Lens's code becomes Files' starting point.
  - Arthur can still show a folder by opening Files at it.
- **D3. Network and display in Settings: read-only in the MVP?** **Recommend yes.** **Decided (J.c., 2026-10-29): the recommendation.**
  - Changing the resolution means rewriting `limine.conf` on the boot partition.
  - Static IP and WiFi aren't in the MVP (wired networking is).
  - Both show their state, and get controls in a later rung.
- **D4. Dates in Files?** **Decided (J.c., 2026-10-29): the recommendation.** NeoFS keeps no times. Adding a modified time is an on-disk format change: a version bump, a byte
  oracle, and handling of existing volumes. **Recommend no dates in the MVP.** Files shows name, kind and size. Times
  come with the format's next planned bump, not as a one-off.
- **D5. Delete: a trash, or ask once?** **Recommend ask once, and no trash in the MVP.** **Decided (J.c., 2026-10-29): the recommendation.** A trash needs its own folder per
  volume and an emptying policy. The confirmation names what will be deleted.

## 9. Prior art

How the facts were gathered:
- Each head date and SHA comes from `git clone --depth 1 --filter=blob:none --no-checkout` of the default branch, on
  2026-10-29.
- Each licence was read from the repository's licence file at that head.
- **Dates are US Eastern** (the local date of the read), not UTC. Smithay's and wayland-rs's heads are 2026-10-29 and
  2026-10-22 in UTC (Chad #144 item 9); the SHAs are the same.
- **Not reachable from this container** (the proxy answers 403): `gitlab.freedesktop.org` (Wayland), `gitlab.redox-os.org`
  (Orbital's home; its GitHub mirror was used), and `fuchsia.googlesource.com` (Fuchsia). The Wayland rows use Smithay
  and wayland-rs, which implement `wl_seat` focus in Rust, on GitHub. Fuchsia has no checked row.
- **No code was copied.** GPL and AGPL code was read for principle only.

### 9.1 The app model: focus and input routing

| Project | Licence | Head (date, SHA) | How it routes focus and input | What we take |
|---|---|---|---|---|
| **Redox Orbital** (`redox-os/orbital`, GitHub mirror) | MIT | 2026-09-23, `5cab3c69959b` | `window_order.rs` keeps a `focus_order` deque, and its front is the focused window (`focused()`). `scheme.rs` sends a `FocusEvent` on change; keys go to the focused window, and the mouse to the hovered one (`hover`) or a window that asked for a grab (`handle_window_mouse_grab`). Super+Tab cycles (`super_tab`). | The shape: a focus order with one front, focus events, hover versus grab. Our grab is WM-side only (CP-52), never asked for by an app. |
| **SerenityOS WindowServer** (`SerenityOS/serenity`) | BSD-2-Clause | 2026-10-28, `b5d4da532a8c` | `WindowManager.cpp`: keys go to the active input window (`process_key_event`). A press starts *automatic cursor tracking*, which sends every mouse event to that window until release (`deliver_mouse_event`, `:1288-1314`). The WM keeps Super and Alt+Tab for itself (`:1682-1760`). | The press-grab rule we already have, and the WM keeping the switch chords. Our chords are kernel-fixed (SAK), which is stronger. |
| **Genode nitpicker** (`genodelabs/genode`) | AGPL-3.0 (with a linking exception) | 2026-05-20, `0f275e7afaab` | Clients are grouped into *domains* by session label. Each domain has a layer and a focus policy: `none`, `click`, or `transient` (focus only while a button is held). Focus can come from an external policy component through a "focus" ROM (`nit_focus`). Hover motion goes only to the focused client unless a domain is set to receive it. (`repos/os/src/server/nitpicker/README`) | **Principle only.** Per-client focus policy: the Lens and the busy emblem are `none`, apps are `click`. Hover only to the front client, as §2.3 item 4 says. Focus decided by one policy owner (our WM), never by clients. |
| **Smithay** (`Smithay/smithay`) | MIT | 2026-10-28, `d8405602ebc0` | Wayland compositor building blocks. `src/input/keyboard/mod.rs`: `KeyboardHandle::set_focus` sends the old target `leave` and the new one `enter` with the keys currently held, each with a serial (`KeyboardTarget`, `:36-43`, `:1304`). | Enter and leave as events an app sees, so an app can drop its own key-repeat and modifier state on leave. A1's protocol gains `focus_in`/`focus_out` notes on the surface channel. |
| **wayland-rs** (`Smithay/wayland-rs`) | MIT | 2026-10-21, `ab2cc988d301` | The Rust bindings for the Wayland protocol, including `wl_seat`'s keyboard enter and leave and its serials. | The serial idea: our activation epoch (`focus.rs:125-131`) already is one. Nothing new. |
| **Fuchsia scenic** | — | not fetched (403) | Not checked from source here. | Nothing claimed. |

### 9.2 Files, settings and launching

| Project | Licence | Head (date, SHA) | What it does | What we take |
|---|---|---|---|---|
| **SerenityOS FileSystemAccessServer** | BSD-2-Clause | `b5d4da532a8c` (as above) | An app asks for a file; the server checks its approved list or prompts the user, then hands back an fd (`Userland/Services/FileSystemAccessServer/ConnectionFromClient.cpp:30-67`). | The powerbox: §4.3's "open with" and §6.1's "save as". Apps get one handle, never a directory. |
| **SerenityOS ConfigServer + LibConfig** | BSD-2-Clause | `b5d4da532a8c` | One service owns each config domain's file; clients read and write through it and are notified of changes. | §5.3: the WM is the only writer of its keys, and the settings app asks. |
| **SerenityOS LaunchServer** | BSD-2-Clause | `b5d4da532a8c` | Maps a file or URL to the app that opens it. | §4.3's fixed "open with" table. Ours is a static table in the broker, not a config file, in the MVP. |
| **SerenityOS FileManager, Settings, Terminal + LibVT** | BSD-2-Clause | `b5d4da532a8c` | The apps themselves (`Userland/Applications/{FileManager,Settings,Terminal}`, `Userland/Libraries/LibVT`). | Reference for scope: what a small OS's first file manager and settings app offer. No code (C++). |
| **COSMIC Files** (`pop-os/cosmic-files`) | GPL-3.0 | 2026-10-21, `4efe802ede04` | A full Rust file manager on iced. | **Principle only.** Reference for behaviour (folders first, confirm delete); far beyond the MVP. |
| **COSMIC Settings** (`pop-os/cosmic-settings`) | GPL-3.0 | 2026-10-26, `5793cbb5205a` | Settings pages for the COSMIC desktop. | **Principle only.** Grouping of pages. |

### 9.3 Terminal

From the survey, with the heads re-checked on 2026-10-29:

| Project | Licence | Head (date, SHA) | Use |
|---|---|---|---|
| `vt100` (`doy/vt100-rust`) 0.18.1 | MIT | 2026-10-19, `fb0f54ec7222` | **Depend or vendor** (T2), after the three checks in §3.1 |
| `vte` (`alacritty/vte`) 0.15.0 | Apache-2.0 OR MIT | 2026-10-25, `89c9c22f5b8f` | Comes with `vt100`. Its `Cargo.toml` has `default = ["std"]`, so `std` is off when depended on without defaults |

The other survey rows (alacritty_terminal, wezterm-escape-parser, rio-vt, libghostty-vt, libvterm) stay reference only.

**Licences of `vt100`'s other dependencies** (`itoa`, `unicode-width`, `arrayvec`, `memchr`) are still to be checked
against their own files at import, as the survey says.

## 10. Which prior-art rules apply

- **The survey rule:** yes (§9). The app model, Files, Settings and the terminal are all new to this codebase.
- **The concurrency pitfall bank:** yes, for A0k (the try ops, §2.3), A3p (the wait set), T1 (the pipe, §3.2) and F0
  (NeoFS operations). Each rung repeats its own pass at dispatch. A1 and A2 add no kernel concurrency (§2.3).
- **Driver trap-mining:** no; no device is brought up.
