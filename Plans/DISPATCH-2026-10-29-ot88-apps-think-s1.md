<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-88 THINK, the app model plus terminal, files and settings → S1

**Why.** OT-88 (terminal, files, settings) and OT-67 (the text editor) are MVP (0.5) items
(`Plans/DECISION-2026-10-27-release-tiers-mvp.md`), and neither has a design yet. The terminal has a prior-art survey,
`Plans/SURVEY-2026-10-27-ot88-terminal-emulator.md`.

All four apps hit the same missing piece. Today **Arthur is the only client that receives input:**
- the WM forwards pointer events only to Arthur's activator;
- the Lens has no focus;
- worker roles are compile-time cargo features (S3's finding for R4.4, ledger 2026-10-29).

S3's installer avoided this by being a WM view. The PM deferred the general answer, "runtime roles, with pointer and
key forwarding to a second client", until a real second client needs it. These apps are that need.

## The THINK: `docs/THINK-2026-10-29-ot88-app-model.md`

1. **The app model (the core of the THINK):**
   - how a non-Arthur app is launched: from a Spine chip or Home intent, through the broker, with a LaunchCap;
   - how it gets a surface, focus, keys and pointer, and how focus moves between Arthur, the apps and Home;
   - what the WM forwards, and to whom;
   - how an app's role is chosen: runtime, from the caps the broker hands it, or one binary per app;
   - what an app may hold: directory caps from K1, the clipboard later, no ambient authority;
   - what happens when an app dies, using #182 and #189's refuse-and-continue as the base.

   Compare at least two designs and recommend one. Name the kernel changes, if any (for example the SAK table, or
   focus routing in `focus.rs`), with a pitfall-bank pass for any cross-CPU or interrupt path.
2. **Terminal:** the survey's leaning (`vt100` plus `vte`), the PTY seam, key encoding, and what runs in it (neo-shell,
   or the T1 shelf).
3. **Files:** browsing NeoFS through K1 directory caps, and what an MVP file manager does (open, rename, delete,
   create a folder, if NeoFS supports it). Say what NeoFS can't do today.
4. **Settings:** what the MVP needs to set (the idle lock time from OT-66 L4, the network, the display, Reduce
   Transparency and Reduce Motion), and where each setting is stored.
5. **The editor (OT-67):** only how it fits the app model. Its own design is a later THINK.
6. **The rung ladder:** dependencies, cycles, gates and kill-switches, in the no-calendar units (CLAUDE.md).
7. **Decisions for J.c.:** taste or product calls only, each with a recommendation.

## Prior art (the survey rule)

Add rows for the app model and for files and settings: how small systems do multi-client focus and input routing
(for example the Redox orbital, Fuchsia scenic and its focus chain, Genode's nitpicker, Wayland's
`wl_seat`/`xdg_shell` focus semantics, and SerenityOS's WindowServer). Give each row's licence, last commit date and
SHA (checked with `git`), and what you'd take. Use GPL code for principle only.

## Rules

- Docs only, so no gate. Push to `claude/ot88-app-model-think` and open a PR. The PM merges it after a cross-family
  read (Codex, GPT-6-Luna high).
- Commit and push at every checkpoint. Send the PM the head SHA from `git ls-remote`.
- Read the code, not only the docs: `userland/neo-init/src/desktop_session.rs` (the WM and broker),
  `userland/neo-atrium`, `kernel/src/atrium/focus.rs`, `userland/neo-arthur/src/showcase_surface.rs`, and
  `docs/THINK-2026-10-18-ds1-home-spine-shell.md`.

```
WORKTREE: cloud session S1 (your own clone)
BRANCH:   claude/ot88-app-model-think
          git fetch origin && git checkout -B claude/ot88-app-model-think origin/main
git show origin/main:Plans/DISPATCH-2026-10-29-ot88-apps-think-s1.md
git show origin/main:Plans/SURVEY-2026-10-27-ot88-terminal-emulator.md
```

RAG isn't available in the cloud session. Before ending, `git status --porcelain` and `git log @{u}..` are both empty.
