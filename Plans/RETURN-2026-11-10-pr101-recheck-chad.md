<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: PR #101 narrow re-check (Chad, relay 386)

**Dispatch:** `Plans/DISPATCH-2026-11-10-relay-386-pr101-recheck.md`.
**Reviewed head:** `24b59a3472e9a0ca006fb6d073b6e47d57ac43a1` (`claude/setup-s6a-i18n-catalog`).
**Fix commit:** `f5dbe7bee6f028291651cf1b12e5eedfe8f13489`.
**RETURN branch:** `codex/pr101-recheck-chad`.
**Verdict:** PASS for the three dispatched checks.

1. **PASS — prior item 4, wording, notes, width and glyphs.** `IntroModelAbsent` now reads `Te oigo, pero aún no tengo mi modelo. Arranca el USB de LogansOS que vino con el equipo.` (`userland/i18n-core/src/lib.rs:206-207`), preserving “it came with.” `SetupBodyComfort` uses `Haz que la pantalla sea más tranquila, si quieres.` (`:151-152`); `HelpChips` uses `puedes acceder a ellos con Tab` (`:197-198`). `IntroGreeting` and `IntroName` retain Arthur's English expansion as its name (`:173-176`); I accept the explicit acronym decision and width rationale at `TRANSLATION-NOTES.md:64-65` for this host catalog. The final persona choice remains with J.c.; no Spanish gloss is claimed. The notes match the changed strings at `TRANSLATION-NOTES.md:54,76,80`, including the restored USB provenance and the documented shortening. `cargo test -p i18n-core` at this head: **7 passed, 0 failed, 1 intentionally ignored** (the optional width report). In particular, `every_row_fits_the_card_in_both_languages` (`src/tests.rs:702`) and `every_character_is_in_the_face_the_text_engine_draws_with` (`:578`) pass; these measure the actual card/font and inspect the shipped font's glyph map.

2. **PASS — prior item 5, survey.** `userland/i18n-core/PRIOR-ART.md:14-20` covers Fluent/fluent-bundle, rust-i18n, gettext-rs, ICU4X and the local `include_str!`/const-table approach. Each external candidate has a licence, last-commit date and full SHA, and `no_std` assessment; the local approach has our MPL-2.0 licence and `no_std` fit, with an inapplicable upstream commit shown as a dash. Lines 22-27 justify the dependency-free enum-indexed tables for the current fixed strings and positional arguments, and identify when to reconsider ICU4X.

   Two dates were independently checked on 2026-11-10 by shallow-fetching the exact SHAs from the raw upstream Git repositories into temporary bare repositories, then running `git show -s --format=%H%nAuthorDate:%aI%nCommitDate:%cI FETCH_HEAD`:

   | Raw repository | SHA | Commit date from Git | Survey |
   |---|---|---|---|
   | https://github.com/projectfluent/fluent-rs.git | `0f87afbbc3fe7d20c61a3fc75cb5d0e123aefdda` | `2026-09-27T13:31:38+03:00` | `2026-09-27` — matches |
   | https://github.com/gettext-rs/gettext-rs.git | `8a96df9da502b3a66411e07451670e47fe48bb54` | `2026-10-17T12:11:50+03:00` | `2026-10-17` — matches |

   Fluent's author date is `2026-09-26T01:07:59+03:00` (September 25 UTC); the survey correctly matches its **committer** date. Gettext's author and committer dates are identical. GitHub's commit API independently returned the same SHAs and corresponding UTC dates. This is the requested two-row spot-check, not a fresh audit of every upstream claim.

3. **PASS — no PR change touches image A; the crate remains unlinked.** The fix changes only `userland/i18n-core/{src/lib.rs,TRANSLATION-NOTES.md,PRIOR-ART.md}`. Relative to the merged main parent (`24b59a34^2`, also the merge base with fetched `origin/main`), the full PR changes only `Cargo.toml`, `Cargo.lock` and the five host-catalog files. The workspace manifest adds only the member (`Cargo.toml:19`), and the lockfile adds only its package entry. No image-producing source, dependency manifest or build recipe changes in that PR diff. A tracked-source search finds no runtime reference to `i18n-core`/`i18n_core`; outside the catalog and Plans, only workspace membership and existing design prose mention it. Its normal dependency section is empty (`userland/i18n-core/Cargo.toml:9`), and `cargo tree -p i18n-core --edges normal` prints only the crate itself; `neo-atrium` and `neo-text` remain dev-dependencies (`:11-14`). The head's merge also incorporates main's unrelated work; this verdict concerns #101's contribution against that main baseline, not a claim that main's image sources were frozen since relay 384. No image rebuild or binary-identity claim is made.
