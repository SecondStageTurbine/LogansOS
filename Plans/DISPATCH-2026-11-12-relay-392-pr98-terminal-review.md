<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 392 and a Muse read: reviews of #98, the terminal window (2026-11-12)

**PR:** #98, L1a.3, `neo-terminal`: the terminal window running `neo-shell`. Seat B wrote it, and Seat B is in the
Claude family.
**Head:** `76aa36d7`, branch `claude/l1a3-terminal`. Read the code at this head from the repo, not from a summary.
**Gate:** green at that head, run 43158436948 (all four QEMU shards). With the feature off, image A is byte-identical
to main.

For every item, answer pass or fail with the enforcing `file:line`. Give a concrete input for any fail.

## Section 392: the cross-family code review

**Reviewer:** Jarvis. **MODEL:** GPT-6-Luna. **EFFORT:** high.
Why this model: this is contained userland, one subsystem, with no SMP core and no new syscall. The only kernel change
is in `test_harness.rs`, behind a test feature.

1. **Child stdin** (`neo-terminal/src/session.rs`):
   - [ ] The only bytes written to the child's stdin are the discipline's output for focused keys, plus the
     answerbacks.
   - [ ] When the pipe is full, the key is dropped and the bell sounds.
   - [ ] After EPIPE, nothing more is written.
   - [ ] Ctrl+Shift+C (End process) is never sent to the child.
2. **Rendering** (`neo-terminal/src/render.rs`):
   - [ ] With `stride > width`, `draw_all` and `draw_with` touch no pixel outside the `width × height` window.
   - [ ] The bounds test covers `stride*(h-1)+w`.
3. **The app** (`neo-worker/src/terminal_app.rs`):
   - [ ] Each wake does a bounded amount of work: at most 16 KiB of output and 64 focus events.
   - [ ] The child is killed on every exit path.
   - [ ] `BUFFER_W`/`BUFFER_H` match the WM's import, both with and without `desktop-resize`.
4. **The session turns** (`neo-init/src/desktop_session.rs`, the `arthur_session_turns` change):
   - [ ] With `atrium-terminal` and `login`, `pointer_tick` never runs while the session is locked.
   - [ ] Without those features, the code is the same as main's.
5. **Prior art:**
   - [ ] The PR's prior-art section holds: the vendored `vt100` (check the licence and the upstream commit), IBM
     Plex Mono under OFL-1.1, and termios read for principle only.
6. **Concurrency:**
   - [ ] The PR's CP-table rows CP-14, CP-33, CP-57 and CP-97 hold against the code.

**RETURN:** `Plans/RETURN-2026-11-12-pr98-review-jarvis.md`, on branch `codex/pr98-review-jarvis`, pushed.

## The Muse security read (J.c. relays it; Muse can't push)

**Reviewer:** Muse (Gwen), a separate model family. Per CLAUDE.md, a security read of Claude-authored code goes to
Muse. This is THINK §6: the lock screen and the client protocol. Read the files at `76aa36d7` from the repo.

1. **The lock path:** `neo-init/src/desktop_session/lock.rs` (`RESUMED`, `take_resumed`, `note_arthur` and
   `end_arthur`), and in `desktop_session.rs`, `apps::resume_front` and the `turns` hook.
   - [ ] No key typed while the session is locked reaches the terminal or its child.
   - [ ] The resume step gives a client its turn back only after a successful unlock.
   - [ ] A client that doesn't answer its discard step is ended.
2. **Locked input:**
   - [ ] While the session is locked, no pointer or key event goes to any client.
   - [ ] When `lock::service` returns `Ok(true)`, the pointer tick is skipped.
3. **The client protocol** (`terminal_app.rs`):
   - [ ] The child gets only the stdin read end, the stdout write end and the term page. It gets no WM channel and no
     other handle.
   - [ ] The terminal writes no input of its own beyond the three answerbacks.
4. **Product builds:**
   - [ ] `end_arthur` is compiled only under `lock-terminal-test`, never in the product.

**RETURN:** J.c. pastes or uploads Muse's answer, and the PM banks it as
`Plans/RETURN-2026-11-12-pr98-security-muse.md`.

## Landing

#98 is full-alone. It lands as a batch of one once both reviews pass, or once each FAIL is fixed and re-checked by the
same reviewer.
