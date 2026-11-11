<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 386: Chad, #101 narrow re-check (your relay 384) (2026-11-10)

**MODEL** GPT-6.1-Sol, medium. Why: a narrow re-check of your own earlier findings.
**Head:** `24b59a34` (`claude/setup-s6a-i18n-catalog`). **Fix commit:** `f5dbe7be`.
**RETURN:** `Plans/RETURN-2026-11-10-pr101-recheck-chad.md` on `codex/pr101-recheck-chad`, pushed.

- [ ] Your item 4: `IntroModelAbsent` keeps "it came with" (your suggested line, or equivalent), every other line you
  listed is fixed, `TRANSLATION-NOTES.md` matches, and the width and glyph KATs still pass. Pass/fail plus line.
- [ ] Your item 5: the prior-art survey (now `PRIOR-ART.md` and/or the PR body) covers fluent/fluent-bundle,
  rust-i18n, gettext-rs, ICU4X and the `include_str!` approach, each with licence, last-commit date and SHA, and
  `no_std` fit, plus a justified choice. Spot-check two dates against the raw repos. Pass/fail.
- [ ] Nothing else changed in a way that touches image A (the crate is still unlinked). Pass/fail.
