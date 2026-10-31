<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: DS-1 — the Home/Spine shell (keyboard only)

**Date:** 2026-10-18. **Source of scope:** `docs/THINK-2026-07-23-desktop-ux-SYNTHESIS.md` §"Synthesized ladder"
DS-1 (intent field, Intent Spine chips — the launch/switch/hold loop, `LaunchCap` via a userland session broker,
the Threshold backdrop on live glass, one instrument bloomed from a real Arthur action). **PM decisions:**
2026-10-18 (recorded below). Pointer + SAK are **DI-1**, decided separately.

## Slices

| Slice | What | Status |
|---|---|---|
| **DS-1a** | Home: the intent field on the WM's own focus; Enter launches Arthur with the intent; Esc in Arthur → `OP_YIELD_FOCUS` → Home holds the live Arthur; the next intent (or Enter on its Spine chip) resumes it; Tab moves the keyboard between field and chip | merged #39 |
| **DS-1b** | Session broker + `LaunchCap` (**ADVISORY** — see below); the Spine chips as the launch list | merged #41 |
| **DS-1c** | One instrument bloomed from a real, read-only Arthur action: the Lens, and a visible second Spine chip that switches to it (keyboard; a click with DI-1a's pointer). Design below | this PR |

Each slice has its own PR, its own gate slot (asked of the PM), a 1920×1080 `run-desktop` screenshot, and keeps the
kill-switch: **DS-1 off (`desktop-home` not built) ≡ the S3 session, bit-for-bit.** DS-1c has its own on top:
**`desktop-lens` off ≡ the DS-1b / DI-1a session** (below).

## The focus model (what the WM may read)

ADR-074 routes every key to exactly one focus object, the active one. Until DS-1 the WM held only clients'
ACTIVATE-only handles, so it could never read a key. DS-1 gives the WM **one focus object of its own** (Home's):

- The WM reads keys **only** from that object, and only while it is the active route — i.e. only keys the user
  typed into Home. A client's keys go to the client's object; the WM's handles on clients stay ACTIVATE-only
  (the boot test still proves `recv` on them is `EPERM`).
- Route changes are single atomic swaps in the kernel (`kernel/src/atrium/focus.rs`); activating Arthur's object
  deactivates Home's in the same step.
- Type-ahead typed between Enter and Arthur's focus going live lands on Home's queue; Home drains it when it next
  takes the keyboard back (after re-activation — `recv` on an inactive route is `EPERM`).

## The return path: `OP_YIELD_FOCUS` (PM decision 1)

- **Wire:** ADR-073 request opcode **7**, payload 32 bytes = the identity (namespace, surface id, surface-cap
  provenance; 24 bytes) + a non-zero little-endian `yield_serial` (bytes 48..56). Reply: the standard 56-byte
  reply, status 0. Hand-laid byte oracle: `atrium-logic-tests` `yield_focus_byte_oracle`.
- **INV-WM-1:** the verb names **only the caller's own surface**, and is honoured only on that surface's own
  channel (`SurfaceRegistry::yield_focus_on_channel` → `CapRequired` for a peer's identity on the caller's channel,
  `WrongOwner` for a foreign namespace, `SurfaceGone` after destroy; KAT `registry_yield_is_channel_bound`). No
  verb can name, enumerate, restack or sample a peer.
- **Client helper:** `libneo::atrium_client::yield_focus` (feature `atrium-yield`).
- **Semantics:** on yield the WM replies, shows Home (the chip shows Arthur *held*), and takes the keyboard. The
  client blocks on its session channel; the WM resumes it by re-activating its focus and sending `b"I"` + the next
  intent (possibly empty).

### KNOWN LIMITATION — a wedged or malicious client can trap focus until DI-1

`OP_YIELD_FOCUS` is **client-cooperative**. A client that never sends it — because it is hung, or hostile — keeps
the keyboard, and with DS-1 alone the user has no way back to Home. This is accepted for DS-1 (PM decision
2026-10-18) and is **closed only by DI-1's kernel secure-attention key**, which lands under its own cross-model
check. DS-1 does not pull a kernel SAK forward.

## `LaunchCap` (PM decision 2) — ADVISORY in DS-1b

DS-1b ships the userland session broker and `LaunchCap`. **It is advisory:** the kernel still lets any process
`sys_spawn` any module by name. The code, this doc and the PR say so, and a test **pins today's status** — a direct
`sys_spawn` from a non-broker *succeeds*, asserted — so that the later kernel `SpawnCap` gate (a separate rung
under OT-66, with a pitfall-bank pass and a cross-family AAG) flips a known test rather than an unstated assumption.

### DS-1b design (under strict alternation)

There is no readiness multiplex (OT-22), so the WM can wait on exactly one channel: the focused client's surface
channel. A broker that listened on a separate LaunchCap channel could not hear it while serving that client. So
the LaunchCap follows the ImportAttach pattern instead:

- **Minting.** The broker (the WM, already the session's only intended spawner) mints a LaunchCap for a client
  at launch: a dedicated channel whose client end it grants over the session channel, recorded against the
  client's surface together with an **allowlist** of module ids (enumerated, never a free-form name — no name
  injection). Arthur's DS-1b allowlist holds only a harmless probe module.
- **Presenting.** To launch, the client sends `OP_LAUNCH { identity, module_id, launch_token }` on its own
  surface channel, then sends the same token on its LaunchCap endpoint. IPC is synchronous rendezvous (ADR-004),
  so the broker, having just received that client's request, receives the token on its **own** end of the cap
  channel. Possession is proven by the channel — only a holder of the client end can send there; the token
  (chosen by the client) only ties that send to this request. The broker never receives on a
  handle a client gave it; waiting on its own end right after the client's request is the same exposure as
  waiting on the client's surface channel (the recorded focus-trap class). It then checks that the request names
  the surface whose channel carried it (INV-WM-1), that the token matches, and that `module_id` is on that cap's
  allowlist. Only then does it `sys_spawn`. Anything else is refused with a reply status (7: no cap / no
  matching proof; 10: not permitted; 11: the broker's spawn failed). If the client closes or exits instead of
  sending its proof, the broker's receive ends with `EPIPE` and the launch is refused (7).
- **The Spine launch list** renders the allowlist the Home itself holds (Home is the root holder).
- **ADVISORY, pinned.** `sys_spawn` is still ungated, so a client can bypass the broker by spawning directly.
  The boot test asserts that this *succeeds* today (`DESKTOP launchcap advisory direct_spawn=allowed`); the
  kernel SpawnCap rung (OT-66) flips exactly that assertion.

## Codex fix-forward (Chad on DS-1a, Janus on DS-1b)

**F1 — the keyboard handoff (Chad, HIGH).** DS-1a switched the *view* before the *route*. ADR-074 keeps a focus
object's queue while it is inactive. So a key typed at a view already on screen was queued for the previous owner
and handed to it on its next turn; a stale Enter could submit an unintended intent. Every transition now runs
`libneo::atrium_handoff::HANDOFF`:

1. **Swap.** The route moves to the new owner.
2. **Discard.** The new owner's queue is emptied of anything older: keys from its previous turn, or typed in the
   instant of the swap. This can only happen once it *is* the route, since only the active route can be received
   from. Home is drained by the WM (`home::discard_queued`). Arthur drains its own queue on receiving the message
   that hands it the turn: the launch or resume intent (`ArthurSurface::discard_queued`). It then acknowledges
   (`D`), and the WM presents only after the acknowledgement, so the order does not depend on the scheduler.
3. **Present.** The new owner's view is shown.

This applies to launch, hold, resume, return to Home, and DI-1a's escape and refocus. The refocus discards nothing:
an escape is not a turn boundary for Arthur, whose window and session stay. Keys typed to Arthur before the SAK are
its own, and keys typed while escaped went to the WM's focus.

What this guarantees: no key crosses into a later turn of any owner. Once the new view is on screen, every key
reaches its owner. A key typed after the swap but before the present belongs to the new owner, or is discarded,
never to the old one.

The order is the discriminator's subject. `tools/atrium-logic-tests` `handoff_kats` models routing and injects a key
at every step boundary of a Home → Arthur → Home → Arthur round trip. The production order passes. DS-1a's order
(present, swap), route-first without the discard, discard-before-swap, and present-before-swap all fail. In QEMU,
`desktop_home_boot` types `zz⌫⌫` immediately after Esc, during the hold handoff: none of those keys may reach Arthur
after it resumes, and each handoff to Home logs `DESKTOP handoff to=home via=… discarded=N`. The kernel-side twin is
#48's activation epoch: a HID batch staged under an earlier epoch is dropped before it reaches any queue. The two
compose. The kernel stops stale *staged* input from crossing a swap, and `HANDOFF` empties what already sits in the
new owner's *queue*. This fix stays WM-side and leaves `kernel/src/atrium/focus.rs` unchanged.

**F2 — the field, exact (Chad, MEDIUM).** The Home oracle hashed the field's whole text box as zero, so deleting
the placeholder, the typed intent or the caret left it green. Spec v1.3.1 states the field rules exactly: origin,
baseline, the scroll rule `room = T.w − 4`, and the caret's position and visibility.
`scripts/desktop/field_oracle.py` → `fixtures/desktop-ds1/field_text.rs` covers four states:
- the empty field, focused (placeholder and caret);
- a typed intent, focused and unfocused;
- a long intent, scrolled left.

For each state the oracle gives a tight window with exact-area per-pixel bounds over the scene oracle's own glass
background (the caret exact). The state's Home digest masks only that window and the bar's and chip's label
boxes. The host KAT `field_matches_exact_oracle_and_mutations_fail` checks every state. Its negative controls must
fail: no caret, no placeholder, one wrong letter, a run shifted by a space, a different ending, the long intent
unscrolled. A code mutation that deletes the placeholder's `draw_line` fails too. The QEMU readback checks the
session's final Home view, the empty focused field, against the same fixture.

**DS-1b — the withheld proof (Janus, HIGH).** On current main the WM polls Arthur's surface, so DI-1a's SAK and
pointer stay live. `launch::serve` still parked in `sys_recv` on the LaunchCap, though, so a client that sent
`OP_LAUNCH` and withheld the proof re-trapped the WM. The proof is now polled (`SYS_TRY_RECV`) inside the same loop
that services the SAK and the pointer. If no proof arrives within `PROOF_DEADLINE_NS` (3 s), or the cap is
closed, the request is refused with status 7. A proof that missed the deadline also **revokes the client's LaunchCap for the session**: the
broker closes its end. A late proof send then fails with `EPIPE` instead of blocking the client short of
receiving the refusal, which would otherwise park the WM in the reply's rendezvous; the client library always
receives the reply. This needs the pointer build (`SYS_TRY_RECV` and the SAK are DI-1a's). The kernel now also compiles
`SYS_CLOCK_GETTIME` (the deadline) and `SYS_SCHED_YIELD` (the poll yields, so the client — woken onto the WM's CPU
by the receive — can send) under `atrium-pointer-forward`, not only `browser-runtime`. The first run without the
clock refused an honest proof after one poll: `clock_gettime` returned `ENOSYS`, so the deadline was already past. Without it, the WM
already blocks on the client's surface channel (the recorded DS-1a focus trap) and awaits the proof the same way.
A client that never receives a reply at all is the general rendezvous class, closed by the readiness multiplex
(OT-22). Arthur's boot probes now include a withheld proof. In
`desktop_pointer_boot` the script sends Super+Escape while the broker waits (`DESKTOP launchcap awaiting-proof`):
the escape must be logged before `proof-timeout`, and Enter then refocuses Arthur. Janus's API note is folded in:
`LaunchBroker::judge` refuses a zero token itself.
## DS-1c design — the Lens: one read-only instrument bloomed from a real Arthur action

**The action.** A listing of `/` is a read-only action that needs no model. Arthur recognises it with
`showcase_logic::wants_root_lens`: a line that says "list", "ls" or "directory" (the same words as Arthur's A1
action mapper) and names `/` and no other path; naming `/` keeps ordinary sentences with the model. In
`desktop-home`, Arthur *blooms an instrument* for it instead of printing the listing. It asks the
broker for `OP_LAUNCH { module: MODULE_LENS }` (module 3), and the broker grants it from Arthur's LaunchCap
allowlist, which DS-1c widens to `{probe, lens}`.

**The instrument is a client, not WM chrome.** Per the vision (`THINK-2026-07-08-gui-vision-jarvis.md` §1.4: "an
instrument is a client surface borrowed into the current intent under caps"), the Lens is its own process,
the `neo-worker` binary in its `desktop-lens` role, with its own surface. **Its authority is exactly what the broker hands it at spawn (inherited
handles):** its session channel and **one directory handle opened `LOOKUP | READDIR` (no READ, no WRITE)**. The
directory sits at index 2 behind a closed placeholder channel at index 1, because file I/O reserves fd 1 (stdout)
for file caps and a directory installed there reads as `EBADF` — the first boot run found exactly that. The
Lens lists the directory with `sys_readdir` and draws the names. It holds no input focus: it never receives a key.
It is the `neo-worker` binary in its `desktop-lens` role (the repo's multi-role child), which is also why the
DS-1b probe (the same binary spawned with no handles) still exits 1 at once.
(Like `LaunchCap`, this is the cap-native shape, not yet a kernel guarantee — `sys_open` on the temporary root is
still ambient for any process; the same OT-66 rung closes it.)

**Strict alternation still holds.** The broker serves the Lens *inside* Arthur's `OP_LAUNCH`, while Arthur is
blocked waiting for the reply. It spawns the Lens and serves its create → grant → import/attach → one commit on
the Lens's channels. Only then does it reply 0 to Arthur. After its commit the Lens blocks on its session channel,
so it is static by construction and the WM composites its last committed buffer. When Arthur's session ends, the
broker closes the Lens's session channel. The Lens sees `EPIPE`, exits, and is reaped. The Lens never commits
again, so the WM never needs to multiplex two clients (OT-22 still open).

**Scene (spec v1.4).** The Lens frame is a second glass window, top-aligned with Arthur's and placed to its right
with a 32 px gap. It has a title band reading "Lens · read-only", an opaque client area, and a **read-only Cap Halo**: a
hairline ring outside the frame, dashed 6 on / 4 off. The dashes make the cue hue-independent. The halo carries
structure, not colour, as in S3. A **second Spine chip** ("L" / "Lens") appears below Arthur's. The Lens frame's
blur region is computed once at scene build with the rest, so `blur_regions_total` becomes 6 and typing still
never re-blurs. If the output has no room for the Lens beside Arthur, the broker refuses the bloom with 11 (spawn
failed). Nothing is overlapped.

**Switching (PM scope 2026-10-18: "a click on the chip switches", keyboard without the pointer).** The Lens holds no
input focus, so switching to it moves the *selection*, not a keyboard route: the window view shows Arthur unfocused,
the Lens's chip wears the chip focus light (spec v1.5), and the keyboard is on the WM's own focus.
- *Keyboard* (any build): Esc holds Arthur (DS-1a). In Home, Tab moves the keyboard from the field to Arthur's chip,
  then — while a Lens is shown — to the Lens's chip, then back to the field. Enter on the Lens's chip shows the Lens
  selected (`DESKTOP lens view shown selected=lens …`); there Tab or Enter switches to Arthur (it resumes, the empty
  intent) and Esc goes back to Home.
- *Pointer* (`desktop-pointer`): in the window view a click on the Lens's chip selects it (the keyboard goes to the
  WM's own focus, as for DI-1a's escape); a click on Arthur's chip, on his frame, Enter, or Super+Tab switches back.
  Home stays keyboard-driven (DI-1a). The click is consumed by the WM, never forwarded.

**Gate (two scenarios; `desktop_home_boot` and `desktop_pointer_boot` keep main's scripts and expectations
unchanged, built without `desktop-lens` — the kill-switch witness).**
- `desktop_lens_boot` (keyboard only): `list /` as the resume intent blooms the Lens. The capture is taken with both
  surfaces up, Arthur focused. Then the keyboard switches: Esc, Tab, Tab, Enter (the Lens view; `stale` and
  `residue` witnesses), Tab back to Arthur, `q`.
- `desktop_lens_pointer_boot`: the same bloom, then a click on the Lens's chip (captured, selected) and one on
  Arthur's chip. `DESKTOP pointer press at=X,Y` logs where each press landed, so a click that switched nothing
  explains itself.
- **Two-surface scene oracle:** each capture (QEMU's own screendump) must equal `scene_oracle.py lens-fixture`
  outside the mask boxes (five labels, both client areas, and the sprite): the Arthur-focused digest for the keyboard
  capture, the Lens-selected one for the click capture.
- **Instrument content byte oracle:** every one of the Lens client's 144,000 pixels must match
  `scripts/desktop/lens_content_oracle.py` → `fixtures/desktop-ds1/lens_content.rs`. Inside each drawn line's glyph
  window, a pixel must lie within per-pixel [lo, hi] bounds from exact-area coverage (the label oracle's machinery:
  fontTools, f64 clipping, neo-text's specified stem darkening, ±2/255). Everywhere else it must equal the content
  floor exactly. The listing is the test boot's temporary root in readdir order, as `kernel/src/fs/mod.rs` seeds it.
  A missing, extra, reordered or misspelt name fails, and so does any stray ink.

A failed bloom logs the stage and the Lens's own exit code (`DESKTOP lens failed stage=… lens_exit=…`; 200+errno
for a failed `readdir`). The session still ends on the Home digest unchanged, because the Lens dies with Arthur's
session.

**Kill-switch (`desktop-lens`, neo-init and neo-arthur).** Off: the scene is built with `BaseScene::with_home`, which
drops the Lens layout entirely (five blurs, no Lens region in any repaint). The LaunchCap allowlist is the probe
only (`allow=probe`), Arthur sends `list /` to the model as before, and the worker has no Lens role. This is the
DS-1b / DI-1a session, proven by their unchanged scenarios. The host KAT `lens_bloom_damage_and_kill_switch` pins
the scene half. `desktop-home` off ≡ S3, and the DS-1a/1b Home digests stay pinned.

**After the fix-forward (#47).** DS-1c is rebased onto `HANDOFF`.
- The Lens's launch is served inside the polled, deadline-bounded proof path.
- A pointer click on the Lens chip is an escape through `HANDOFF` (Home's queue is discarded) that also lights the
  Lens chip. Every escape and refocus sets the selection explicitly, so none can leave a stale one.
- Resuming Arthur from the Lens view uses #47's resume: Arthur discards its queue and acknowledges, then the window
  view is shown.

**AAG (Claude, three lenses) — what changed.**
- *Real finding.* A pointer selection racing Arthur's yield could carry the selection into Home, where the next
  typed intent was taken as "switch to the Lens" and lost. Fixed two ways: Home always starts with the selection on
  Arthur's chip (`show_home` clears it), and only the empty intent (Enter on the Lens chip) enters the Lens view.
- *The Lens labels are now exact.* Before, the digest masked the Lens title and the second chip's label box, so a
  missing or wrong read-only cue passed. `lens_content_oracle.py labels` →
  `fixtures/desktop-ds1/lens_labels.rs` holds "Lens", "· read-only", "L" and "Lens" to exact-area bounds over the
  scene oracle's background, in both captures; the sprite's pixels are excluded. Each of these mutations fails
  against a real capture: "· read-write" (139 px), "Lena" (43 px), "Lenz" (29 px).
- *Other changes.* The Lens exits 0 only when the broker ends it (`EPIPE`); any other receive error is its own exit
  code. CI now lints both Lens boot-test builds. Arthur skips the withheld-proof probe in Lens builds, because a late
  proof revokes the LaunchCap the Lens needs; `desktop_pointer_boot` keeps that probe.

### DS-1c known limitations

- **A hung Lens freezes the desktop.** The broker serves the Lens inside Arthur's request with no timeout, so a
  Lens that is alive but stuck before its first commit (a filesystem whose `readdir` never ends, say) blocks the
  WM. A Lens that crashes is safe: its exit closes its channels, the broker sees `EPIPE`, refuses the bloom (11)
  and logs the Lens's exit code. Same class as the recorded focus trap; closed with the readiness multiplex (OT-22).
- **One 576 KiB read-only mapping per bloom** stays mapped (no unmap syscall yet), as for Arthur's buffer.
- **A Lens that exits badly after its commit** is logged at session end (`DESKTOP lens failed stage=118 …`) and is
  never fatal to the shell.
- **With the pointer, a selected Lens hands back on Enter only.** While escaped onto the Lens, the WM's own keys are
  DI-1a's escaped keys: Enter, Super+Tab or a click on Arthur's chip or frame returns to Arthur. Tab and Esc do
  nothing there; they are the keyboard Lens view's keys, reached from Home.
- **Home is keyboard-only for the switch.** With the pointer, clicks on the chips switch in the window view; in Home
  (Arthur held) the pointer stays DI-1a's idle drain, so the Lens's chip there is reached with Tab. Pointer
  handling in Home is DI-1's next rung.
- **The selection is a cue, not a route.** "Switched to the Lens" lights its chip and unfocuses Arthur; the Lens
  receives nothing (it is static by construction). An instrument that takes input needs the readiness multiplex
  (OT-22) and is out of DS-1's scope.
- **One instrument, one directory.** The Lens is lent `/` only, and `wants_root_lens` recognises only a listing of
  `/`. `ls <dir>` for other directories (a path argument through the broker) is a later instrument rung.

## Chad re-checks (2026-10-19): the launch, the failed bloom, the parse

**DS-1ab F1 (was still open): the initial launch.**
- **Before.** The launch showed and presented the window view in `run()`, but the route moved to Arthur only later,
  inside `arthur_session`'s `HANDOFF`. A key typed once the window was visible went to Home's queue and was lost.
- **Now.**
  - Home stays on screen, its route live, while Arthur is spawned and handshaken.
  - Arthur's first commit, arriving while Home shows, is copied but not presented.
  - The window view is presented only in the launch `HANDOFF`'s present step, after the swap and after Arthur's
    discard ack.
- **Who gets a key.** One typed once the window shows is Arthur's. One typed while Home still shows lands in Home's
  queue; Home is the owner on screen then. Nothing reads that queue until Home's next turn, which discards it. That
  is the same rule as a hold.
- **Logging.** Arthur logs its intent line before the ack, so the WM's view-switch lines never interleave with it.
- **Discriminator.** `desktop_home_boot` types `x` and Backspace as the window appears. Both must reach Arthur.

**DS-1c F1: a failed bloom leaked, and could be retried without bound.**
- **The leak.** The kernel pins a mapped buffer for the life of the mapper's address space; `SYS_VM_UNMAP` is
  browser-runtime only. `SYS_CLOSE` refuses Process handles. So each failed bloom left the WM a read-only mapping and
  a process handle.
- **One lent buffer.** The WM now owns one Lens buffer, made at its start: read-only mapped once, plus one copy
  plane.
  - It lends that buffer to each Lens, with write, query and commit rights and no grant.
  - The Lens's derived handle and its mapping go with its process.
  - The process handle is revoked.
  - A Lens that cannot be reaped poisons the store: no later Lens is lent a buffer it may still hold. `ESRCH` after a
    kill counts as reaped.
- **Attempt bound.** At most `MAX_LENS_ATTEMPTS` blooms per client session (`LensAttempts`, host KAT). A failure
  spends its attempt.
- **Discriminator.** `desktop_lens_fault_boot` fails two blooms late, after the Lens committed. Before the third,
  two probes must be back at baseline:
  - the first 32 free handle slots (the allocator is first-free);
  - the mmap cursor stride (it never reuses an address).
- **Mutation check.** Without the revoke, the leak check goes red.
- **Remaining WM growth.** The WM still keeps one mapping, plus the buffer handle, of each *Arthur* session's buffer.
  This predates DS-1c and cannot be released until the kernel can unmap a borrowed buffer.

**DS-1c F2: strict parse.** `wants_root_lens` accepts exactly `ls /`, `list /`, `list directory /` and `list the
directory /`. Any other path or extra word stays with the model.

## Constraints carried

- **Strict alternation.** Channel `recv` blocks and there is no readiness multiplex yet (OT-22), so the WM serves
  exactly one thing at a time: Home's own keys while Home is focused, the focused client's channel otherwise. A
  held client is blocked on its channel by construction, so it cannot commit while unseen. Home's idle loop spins
  on `EAGAIN` (as Arthur's does) until OT-22.
- **Damage discipline.** Every view switch is checked twice in the boot test: `stale_pixels` (what was presented
  equals the base) and `residue_pixels` (the base equals the target view recomputed per pixel). Each witness is
  proven by a mutation that it catches.
