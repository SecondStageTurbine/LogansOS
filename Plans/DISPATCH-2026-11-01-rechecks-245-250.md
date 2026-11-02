<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relays 245-250: narrow re-checks of fixed PRs, and Dexter's L2d design

Same rules as `Plans/DISPATCH-2026-11-01-m910q-reviews-and-l2b.md` (public repo: no personal details; RAG not
available; plain checklist, PASS or FAIL with `file:line` and a concrete input for any FAIL; name the head you read;
RETURN on a new branch off `origin/main`, push, reply with the commit SHA; never push to `main`). For a re-check:
each FAIL in your earlier report, PASS or FAIL now, then one line on anything new the fix introduced. The seat's
item-by-item notes are the latest `PM:` comment on each PR.

## Section 245: Chad, #13 (your relay 241 re-check: the three remaining FAILs)

**MODEL** GPT-6.1-Sol, **EFFORT** medium. Head `249ee9f9`. Earlier: `git show origin/codex/pr13-recheck-chad:Plans/RETURN-2026-11-01-pr13-recheck-chad.md`.
RETURN: `Plans/RETURN-2026-11-01-pr13-recheck2-chad.md`, branch `codex/pr13-recheck2-chad`.

## Section 246: Jarvis, #29 (your relay 233 review of the fetch broker, all five items)

**MODEL** GPT-6.1-Sol, **EFFORT** medium. Head `e4824383`. Earlier: `git show origin/codex/pr29-review-jarvis:Plans/RETURN-2026-11-01-pr29-fetch-broker-review-jarvis.md`.
RETURN: `Plans/RETURN-2026-11-01-pr29-recheck-jarvis.md`, branch `codex/pr29-recheck-jarvis`.

## Section 247: Jarvis, #30 hot-unplug and #23's changed code

**MODEL** GPT-6-Luna, **EFFORT** medium. #30 head `9c97b079` (your relay 238 FAIL: hot-unplug); #23 head `600f45a9` (its code
changed after your relay 229 PASS, for Chad's #20 review: re-check that your four PASS items still hold, and that the
trap-mining section now exists). Earlier reports on `codex/pr30-review-jarvis` and `codex/pr23-review-jarvis`.
RETURN: `Plans/RETURN-2026-11-01-pr30-pr23-recheck-jarvis.md`, branch `codex/pr30-pr23-recheck-jarvis`.

## Section 248: Jarvis, #12 terminal THINK revision 3 (your relay 236 design findings)

**MODEL** GPT-6-Luna, **EFFORT** medium. Head `95b59532`. Earlier: `git show origin/codex/pr12-review-jarvis:Plans/RETURN-2026-11-01-pr12-terminal-think-review-jarvis.md`.
Also check it now covers kibi's needs (`docs/THINK-2026-11-01-l1b-kibi-on-logansos.md`, on main).
RETURN: `Plans/RETURN-2026-11-01-pr12-recheck-jarvis.md`, branch `codex/pr12-recheck-jarvis`.

## Section 249: Chad, #9 and #20 THINK revisions (your relay 237 design findings)

**MODEL** GPT-6-Luna, **EFFORT** medium. Heads `12ada069` (#9) and `cfcdf7fb` (#20). Earlier: `git show origin/codex/pr9-pr20-review-chad:Plans/RETURN-2026-11-01-pr9-pr20-storage-thinks-review-chad.md`.
RETURN: `Plans/RETURN-2026-11-01-pr9-pr20-recheck-chad.md`, branch `codex/pr9-pr20-recheck-chad`.

## Section 250: Dexter, L2d design (images in the desktop)

**MODEL** DeepSeek, high: a design THINK, no code.

Your Lane 2 decode API is on `main` (`userland/neo-image-decode`, `libneo::image_decode`). Write
`docs/THINK-2026-11-01-l2d-images-in-the-desktop.md`: where the desktop would use decoded images first (wallpaper,
app icons, the browser's `<img>` path via OT-61), how a PNG reaches the compositor (file read through the
capability model, decode in the app's process, never in the WM or kernel), the memory budget per image and per
session, a cache and its eviction, and a first gate scenario (decode a PNG from NeoFS and show it; the frame hash as
the oracle). Name what in the WM or Atrium each option touches, with `file:line` from main, and recommend the first
build. RETURN: `Plans/RETURN-2026-11-01-l2d-images-dexter.md` on branch `dexter/l2d-images-think` (commit the
THINK and the RETURN only).
