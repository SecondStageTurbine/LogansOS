<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: #98 terminal cross-family code review (Relay 392)

**Reviewer:** Jarvis  
**Reviewed head:** `76aa36d7bf48fb7b34ea7c86758927d377f6b29b` (`claude/l1a3-terminal`)  
**Scope:** Section 392 of `Plans/DISPATCH-2026-11-12-relay-392-pr98-terminal-review.md`.

## Review

1. **Child stdin**
   - **PASS — Only discipline output for focused keys and answerbacks are written.** `Session::key` passes encoded bytes through `Discipline::key` and `apply`, which writes only `fx.to_child`; terminal parser replies go through `write_child` in `parse` (`userland/neo-terminal/src/session.rs:114-126,172-180,201-215`). The app reads press events from its own focus and invokes that path (`userland/neo-worker/src/terminal_app.rs:419-442`). The child receives only the stdin read end, stdout write end, and term page at spawn (`userland/neo-worker/src/terminal_app.rs:524-539`).
   - **PASS — A full pipe drops the key and rings the bell.** A short nonblocking write increments `dropped` and sets `bell` (`userland/neo-terminal/src/session.rs:218-230`); `EAGAIN` maps to a zero-byte write (`userland/neo-worker/src/terminal_app.rs:102-115`).
   - **PASS — After EPIPE, no more writes are attempted.** `Write::Gone` sets `stdin_gone`; every later `write_child` returns before calling the sink (`userland/neo-terminal/src/session.rs:218-230`).
   - **PASS — Ctrl+Shift+C is never sent to the child.** It returns `EndProcess` before encoding or discipline processing (`userland/neo-terminal/src/session.rs:114-126`); the app handles that action with `sys_kill` (`userland/neo-worker/src/terminal_app.rs:428-435`).

2. **Rendering**
   - **PASS — `draw_all` and `draw_with` stay inside the width × height window when stride exceeds width.** `draw_all` fills only each `[row .. row + width]` slice (`userland/neo-terminal/src/render.rs:113-125`); `draw_with` constructs a `Canvas` with window width/height and the supplied stride (`userland/neo-terminal/src/render.rs:135-142`). Canvas bounds every fill and glyph clip to the window, while indexing rows using stride (`userland/neo-text/src/canvas.rs:98-111,122-160`). The test checks the top-left window against a tight buffer and verifies every other pixel remains unchanged (`userland/neo-terminal/src/render.rs:362-401`).
   - **PASS — The bounds check covers `stride*(h-1)+w`.** Renderer checks that exact minimum extent (`userland/neo-terminal/src/render.rs:113-116`); `Canvas::new` also uses checked multiply/add for the same extent (`userland/neo-text/src/canvas.rs:98-111`). The test supplies a buffer shorter than that extent and expects rejection (`userland/neo-terminal/src/render.rs:394-400`).

3. **The app**
   - **PASS — Each wake has bounded output and focus work.** Output reads stop at `READ_BUDGET` (16 KiB) (`userland/neo-worker/src/terminal_app.rs:89-95,390-415`); the focus loop processes at most `KEYS_PER_WAKE` (64) events (`userland/neo-worker/src/terminal_app.rs:419-458`).
   - **PASS — The child is killed on every app exit path after spawn unless already exited.** All loop exits call `end_child` (including turn/protocol failures, input errors, commit failure, and session EPIPE); `end_child` issues `sys_kill` for a live child (`userland/neo-worker/src/terminal_app.rs:299-330,453-456,502-520,542-547`). Failures before a successful spawn have no child process to kill (`userland/neo-worker/src/terminal_app.rs:284-287,524-539`).
   - **PASS — Buffer dimensions match the WM with and without `desktop-resize`.** The terminal selects 640×400 normally and 1280×800 with `desktop-resize` (`userland/neo-worker/src/terminal_app.rs:64-79`); the WM uses its matching 640×400 constants normally and `MAX_CLIENT_W/H` (1280×800) under the same feature (`userland/neo-init/src/desktop_session.rs:74-95`; `userland/neo-init/Cargo.toml:120-124`).

4. **The session turns (`arthur_session_turns`)**
   - **PASS — With `atrium-terminal` and `login`, `pointer_tick` does not run while locked.** That cfg path calls `lock::service` first and calls `pointer_tick` only on `Ok(false)`; `Ok(true)` skips it (`userland/neo-init/src/desktop_session.rs:2080-2103`).
   - **PASS — Without both features, this change leaves the code path as main's.** The original direct `pointer_tick` call remains under `#[cfg(not(all(feature = "atrium-terminal", feature = "login")))]`; the replacement service path is compiled only under the complementary cfg (`userland/neo-init/src/desktop_session.rs:2080-2103`).

5. **Prior art**
   - **PASS — The stated prior art and licensing checks hold.** The THINK identifies vendored `doy/vt100-rust` as MIT and records upstream commit `fb0f54ec7222f4805f33cd3ca530229e0a0fe6ad`; the vendored manifest and license agree (`third_party/vt100/VENDORED.md:5-18`, `third_party/vt100/Cargo.toml:1-4`, `third_party/vt100/LICENSE:1-3`). IBM Plex Mono is recorded as SIL OFL-1.1 with its licence text and provenance (`assets/fonts/README.md:7-18,20-35`, `assets/fonts/OFL-IBM-Plex.txt:1-4`). `termion` is described as prior art for raw-mode APIs, with its Unix backend's termios expectation noted; L1a links none of it (`docs/THINK-2026-10-31-l1a-terminal-window.md:515-530`).

6. **Concurrency**
   - **PASS — CP-14 holds.** The terminal's parked configuration waits on focus/pipe with a deadline, while the default path polls and yields; output and focus work are bounded (`userland/neo-worker/src/terminal_app.rs:345-363,390-415,419-458`). No new park/unpark protocol is introduced by the session or renderer.
   - **PASS — CP-33 holds for this app.** Every live-child teardown path calls `end_child`, which kills the process (`userland/neo-worker/src/terminal_app.rs:311-330,453-456,502-520,542-547`). The terminal adds no custom process-wait queue or kill teardown path.
   - **PASS — CP-57 holds.** The child receives no WM/session channel, only its three stdio selections (`userland/neo-worker/src/terminal_app.rs:5-11,524-539`). The terminal is a client: its synchronous surface requests are paired with replies (`userland/neo-worker/src/terminal_app.rs:586-612`); it adds no unsolicited server send or configure wait.
   - **PASS — CP-97 holds.** Input isolation while locked is enforced by the kernel lock route and lock epoch, with the WM servicing its lock focus; the terminal consumes input only through its own focus (`docs/THINK-2026-10-31-l1a-terminal-window.md:421-437`, `userland/neo-worker/src/terminal_app.rs:419-458`). The WM's resume-discard step sends a turn only after successful unlock handoff and ends the client if discard does not complete (`userland/neo-init/src/desktop_session/lock.rs:470-523`, `userland/neo-init/src/desktop_session.rs:3721-3729,3969-4009`).

**Result: PASS — all Section 392 review items.** The supplied dispatch reports the four-shard gate green at this head; this return records a code review and does not rerun it.
