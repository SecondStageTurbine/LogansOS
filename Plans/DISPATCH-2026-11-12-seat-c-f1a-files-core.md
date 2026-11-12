<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Seat C, F1a — the Files app's core, host-tested (2026-11-12)

**Seat:** Seat C (cloud). S6b (#109) landed as `2cfc61eb`.
**Branch:** `claude/f1a-files-core`, cut from `origin/main`.
**Design:**
- `docs/THINK-2026-10-29-ot88-app-model.md`: §4.2 (F0's calls), §4.3 (the MVP file manager), §8 D2 and D4, and §9.2
  (the prior art for Files);
- `Plans/DECISION-2026-11-11-one-window-first.md` (critical path A2 → F1 → E2).

## Why this lane, and why now

F1 (Files) is on the critical path, and it needs A2 (#111, Seat B) and F0 (#106, landing as batch 106). Its **core**
needs neither: like `neo-term-core` (L1a.2) and `neo-edit-core` (E1), it is a host-tested crate with no syscalls and no
WM. Building it now leaves F1b as a thin wiring rung once A2 and F0 are on main.

It touches no file of A2 (WM and neo-atrium), F0 (the kernel), A3p (`wait_any`, the WM loop) or R3 (the boot-stick
mount). The only possible overlap is new string ids in `userland/i18n-core`, which at most needs a textual merge.

## Build: `userland/files-core`

A `no_std` + `alloc` crate, host-tested, **linked into no image**.

1. **A directory trait shaped like F0's calls.** Take the shapes from #106 (`userland/libneo/src/` at `7557b87e`, or
   from `main` once batch 106 lands): `readdir`, `stat_at`, `mkdir_at`, `unlink_at`, `rename_at`, and opening one file
   by name with given rights. Use **F0's error set** (stale, exists, not empty, no space, refused, name too long,
   folder full). F1b's adapter onto libneo must be thin, so mirror the real signatures; don't invent a richer API.
   Tests use a fake in-memory implementation of the trait.
2. **The listing:**
   - folders first, then files, each sorted by name, with kind and size and no dates (D4);
   - a defined, tested sort order (state it: e.g. case-folded ASCII, then bytes);
   - the 64-entry folder limit shown, not hidden.
3. **Navigation:**
   - a folder opens in place, and Up goes to its parent, never above the root cap;
   - the selection is kept by name across a refresh, and falls back to a neighbour when the name is gone.
4. **Operations, as an explicit state machine with a confirmation step where §4.3 says so:**
   - **new folder** ("New folder", then "New folder 2", and so on);
   - **rename** — validate the name first: not empty, at most 47 bytes of UTF-8, no `/`, not `.` or `..`. Renaming
     over an existing name is refused, as F0 refuses it;
   - **delete** — asks once. A non-empty folder is refused with its own message;
   - **small copy** — up to 32 KiB. Over that, Files says so and copies nothing.
5. **"Open with" mapping** (§4.3):
   - text → the editor (`MODULE_EDITOR`; define the constant's use, not the module). Use a stated extension list;
     state it and test it;
   - `.neom` → nothing; models are Arthur's;
   - anything else → "no app opens this".

   The core returns the request (`module_id` and the rights from the target's manifest: READ, plus WRITE for the
   editor). It never holds a directory for the target. F1b turns the request into `OP_OPEN_WITH` plus the grant of
   one file handle.
6. **Muse's F0 observation — a directory handle binds the PATH, not the folder's identity**
   (`Plans/RETURN-2026-11-12-pr106-security-muse.md`, observation 1). Files must not assume a folder's identity across
   operations:
   - **Rename by another process:** if the current folder (or an ancestor) is renamed, its handle goes stale. Files
     falls back to the nearest ancestor that still resolves, says so, and refreshes.
   - **Replacement:** if a folder is moved away and a new one is made at the old path, the old handle now acts on the
     new folder. So Files never caches a listing across an operation: it re-lists after every operation and after
     every stale error.
   - **Its own renames:** after Files renames a folder, it reopens it by the new name.

   KATs drive each of these through the fake: rename-away, rename-away plus mkdir at the old path, and an ancestor
   rename.
7. **Errors and strings:**
   - every error from step 1 maps to a user string through `i18n-core`, in **English and Spanish only**
     (J.c.'s decision: two languages);
   - add the new ids to both tables and to `fixtures/en.txt`, and keep S6b's drift KAT and no-stray-literals scan
     green, extended to `files-core`;
   - the Spanish follows S6a/S6b's tone. Mark the rows "F1a" in `TRANSLATION-NOTES.md`.
8. **A render model, not pixels:** rows, the selection, the confirmation and the message line, as plain data, so F1b
   paints them. Start from the Lens (`userland/neo-worker/src/lens.rs`, read-only) where it fits.

## Gate

- **Not kernel-touching:** push CI (host tests, fmt, clippy, SPDX) is the gate. Run `scripts/worker-close-gate.sh`.
- **Linked into no image:** show that Build Checks' product image size and `.text` are unchanged against main's
  latest push run.
- **KATs for every item above**, including:
  - a randomized operation sequence against a model of the tree;
  - the three path-binding cases in item 6.
- **Kill-switch:** the crate is in no image. F1b's `neo-files` module feature is its switch.
- **Prior art:** THINK §9.2 covers Files (SerenityOS FileSystemAccessServer for the powerbox; COSMIC Files for
  behaviour, principle only, GPL-3.0). State in the PR that it still applies. Survey anything new you borrow (license
  and last commit date).

## Review

When green, the PM queues two reviews:
- a cross-family code review (GPT-6-Luna, high): contained userland, with no SMP core and no syscall;
- a Spanish wording read of the F1a rows (GPT-5.6-Luna, medium).

## Close

Commit and push at every checkpoint. Post `PM:` on the PR with the run URL.

**Disk reclaim:** once the PM confirms the merge, run `cargo clean` in the worktree and remove it.

**Next for this seat:** F1b (the `neo-files` module, the Spine chip's switch from A2, and `OP_OPEN_WITH` in the broker)
once A2 and F0 are both on main.
