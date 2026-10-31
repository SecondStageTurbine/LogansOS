<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW (relay #144) → Chad: the OT-88 app-model THINK (#191)

**MODEL:** GPT-6-Luna. **EFFORT:** high. A design doc, docs only. It's a cross-family read of a Claude-authored THINK
before it merges.

The THINK is `docs/THINK-2026-10-29-ot88-app-model.md`, merged to `main` as `ff145c6e` (PR #191). J.c. changed D1 to
side-by-side floating windows with snap; review §2.3 and §7 as reworked for that. The dispatch was `Plans/DISPATCH-2026-10-29-ot88-apps-think-s1.md`. It designs how a second app besides Arthur is
launched and given focus and input, then the terminal, files and settings apps, and where the editor fits.

Check every claim it makes about today's code against the code itself. One pushed RETURN, a plain checklist (pass or
fail, the enforcing `file:line`, a concrete input or scenario for any fail):
`Plans/RETURN-2026-10-29-ot88-think-review-chad.md`.

1. **§1, today's tree.** Are its descriptions of launch, focus, keys, the pointer, surfaces, the Lens and faults right?
   Spot-check at least six of the `file:line` citations in `userland/neo-init/src/desktop_session.rs`,
   `kernel/src/atrium/focus.rs` and `userland/neo-arthur/src/showcase_surface.rs`.
2. **§2.3, Design B.** Is "kernel changes for the app model: none" true?
   - the routes, handoffs, the Super+Space and Super+Escape chords, and pointer forwarding to an arbitrary activator;
   - the claim that a hung app can't trap the user because the chords are kernel-side.
3. **§2.3 item 7, death is local.** Does the design close every path where one app's death or fault ends the WM?
   Compare it with #189's per-arm table, now on main.
4. **§2.3's pitfall pass** (CP-52, CP-54, CP-57, CP-61). Is each N/A or guard right? Is any class missing for an
   N-client WM loop that parks on N channels?
5. **§3.2, the kernel pipe (T-a).** Does T-b really need a kernel wake for a blocking read? Is the pipe's pitfall table
   complete (CP-33, CP-61, CP-67, CP-85, EOF/EPIPE), with a discriminator for each? Is the stdio convention (handles 0
   and 1 as pipes) a real kill-switch for every existing scenario?
6. **§4.1, NeoFS today.** Is each limit right on main after #180 and #187? The limits: no rename, delete, userland
   mkdir, stat or times; 32 KiB ordinary writes; 64 entries per directory; 47-byte names.
7. **§5, settings.** A WM-owned `NEOSET1` store with the WM as its only writer. Does this format need a byte oracle,
   and does the THINK commit to one?
8. **§7, the rungs.** Do the dependencies hold (A0 → A1 → A2, then T1 → T2, K1b → F0 → F1, S1 → S2)? Is each kernel rung
   (T1, K1b, F0) marked for the 4-shard gate and an AAG?
9. **§9, prior art.** Does every row give its licence, head date and SHA, and is every GPL or AGPL row principle only?
10. **§8, decisions D1–D5.** Is each one a real product call, not a hidden design gap? Is anything J.c. should decide
    missing?

```
MODEL: GPT-6-Luna. EFFORT: high. (A design doc: a cross-family read.)
WORKTREE: E:\AI Automation\workers\chad   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot88-think-review-chad
          git fetch origin && git checkout -B codex/ot88-think-review-chad origin/main

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-ot88-think-review-chad.md
git show origin/main:docs/THINK-2026-10-29-ot88-app-model.md
```

RAG health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that and read the files
directly. Push the RETURN. Before ending, run `git status --porcelain` and `git log @{u}..`; both must be empty.
