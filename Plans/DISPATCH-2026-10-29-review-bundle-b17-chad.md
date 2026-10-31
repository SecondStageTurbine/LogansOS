<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW BUNDLE (relays #158–#159) → Chad: the socket teardown fix and the Install chip

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. A bundle, so one model throughout (CLAUDE.md bundle rule). On its own,
#159 would be GPT-6-Luna high.

Both merged after batch 17 (run `40855721787`, green at `e29f21d0`). Review each squash on `main`:
`git show <sha> --stat` and `git show <sha>`. One pushed RETURN per relay, each a plain checklist (pass or fail, the
enforcing `file:line`, a concrete input for any fail): `Plans/RETURN-2026-10-29-<slug>-review-chad.md`.

## #158: #202 `34c6c132`, sockets close when their last handle leaves. Slug `socket-teardown`

Muse does the security review (#157). Yours is the concurrency cross-check.
1. **Lock order.** At every release point (`Detached::release`, exit, kill, revoke), what is held? The PR says no
   handle-table lock and no `CAP_GRAPH`, enforced by a `neo-test` witness; IF on for exit, kill and close, off for
   revoke. `close()` takes the socket registry lock and the TCP maps. Check CP-1, CP-37, CP-56 and CP-33 against the
   code.
2. **The last-handle race.** Can a grant or derive on one CPU and a release on another both see a count that makes
   the socket close while a handle still exists, or never close it?
3. **Exit and kill** call `close_net` before `complete_process_waiters`. Can a waiter observe the process gone while
   its sockets are still bound?
4. **The scenario** `socket_teardown` (control, exit, kill parked in recv, revoke per kind, table sizes, a kernel
   listener). Does each falsifier fail at its own check?

## #159: #203 `1c1df371`, OT-87 R4.4b, the Install Spine chip. Slug `install-chip`

The PR's checklist:
1. `chip_shown()` (`userland/neo-init/src/installer_view.rs`) shows the chip only with the cap and a row not refused
   `MEDIUM`, and LIST runs once, at `start`.
2. **The Tab order** (`desktop_session.rs`, Home's chip arms) is Arthur → Lens → Install → field, and the Lens's arm
   doesn't fire from the Install chip. Esc and Tab off the chip clear its selection.
3. **Activation.** Enter on the selected chip, and a click inside `chip3`, open `installer_view::run`. A press
   elsewhere doesn't. A press during `poll_control` (no focus) is dropped.
4. **The kill-switch.** With `neo-atrium`'s `install-chip` off, no field, method, panel arm or label path of the chip
   remains (`layout.rs`, `scene.rs`, `icons.rs`), and the `.text` table holds.
5. **The oracle** (`scene_oracle.py` `install-fixture`) is independent of the Rust scene, and its digests cover glass
   and RT.
6. **The scenarios' needles** are each printed only by the leg that claims them. The negative scenarios check every
   row's refusal and the target hash.

```
MODEL: GPT-6.1-Sol. EFFORT: high. (A bundle: one model throughout, per the bundle rule.)
WORKTREE: E:\AI Automation\workers\chad   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/review-bundle-2026-10-29c-chad
          git fetch origin && git checkout -B codex/review-bundle-2026-10-29c-chad origin/main

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-review-bundle-b17-chad.md
```

RAG health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that and read the files
directly. Push both RETURNs. Before ending, run `git status --porcelain` and `git log @{u}..`; both must be empty.
