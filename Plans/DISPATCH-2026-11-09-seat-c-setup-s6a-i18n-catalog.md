<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Seat C, setup menu S6a — the i18n catalog, host-only (2026-11-09)

**Seat:** Seat C (cloud). **Branch:** `claude/setup-s6a-i18n-catalog` off `origin/main` (#94 landed as `52c0ed0e`).
**Design:** `docs/THINK-2026-11-02-setup-menu.md`: §3.1 (Language) and the **S6** section in §5.
**Feedback item:** F11 in `Plans/FEEDBACK-2026-11-02-preview.md`.

## Why split S6

S6 moves every wizard string to a catalog id. S4 (Seat I, the Network page) and S5 (Seat E, #99, keyboard layouts)
are adding wizard pages right now, so moving the strings today would conflict with both. **S6a is the catalog only:
a host crate that no wizard code links yet.** S6b moves the strings, enables the page and adds the
`setup_menu_lang` gate, and it is dispatched after S4 and S5 land.

## What to build (S6a)

1. **`i18n-core`**, a `no_std` host crate. It holds:
   - message ids, as an enum or consts, one per string the wizard, the intro card and Home's verbs show today
     (collect them from `userland/setup-core`, `neo-init`'s setup and intro, and `neo-atrium`'s Home);
   - the **English** table, word for word what ships today;
   - a **Spanish** table, the second language. The PM picked Spanish because it's Latin-script, which the text
     engine supports, and J.c. can change the choice;
   - a lookup with a fallback to English, and `LANGUAGE_COUNT`.
2. **Host KATs:**
   - every message id is present in both tables, with no fallback taken on any wizard string;
   - every character in the Spanish table is covered by the shipped font. Read the font's coverage from the
     font data the text engine uses; don't assume Latin-1. Name any missing glyph;
   - the longest row of each page fits the page's width budget, with the budget taken from the scene spec, in
     both languages;
   - the English table equals today's literals byte for byte. This test guards against drift until S6b swaps the
     literals for ids.
3. **No change to any image.** Nothing links the crate yet, so no kill-switch is needed, the same as L1a.2. State in
   the PR that image A's ELFs are unchanged.

## Translation quality

Write the Spanish as plain, neutral Latin American Spanish, matching the English's short imperative style. Add
`i18n-core/TRANSLATION-NOTES.md`, which lists each id, its English text, its Spanish text and any choice a reviewer
should check. The PM will route a review of the wording.

## Prior art (unfamiliar-feature rule)

Survey Rust i18n crates before writing: `fluent`/`fluent-bundle`, `rust-i18n`, `gettext-rs`, `icu_*`/ICU4X message
formatting, and `include_str!`-table approaches. For each, give its license, its last commit date and SHA, and
whether it fits `no_std`. Expect the answer to be a small table of our own, since we have no plurals or arguments
yet. The survey table goes in the PR.

## Gate

`scripts/worker-close-gate.sh`, then a green PR run at the head; this should be the quick tier, since it's
host-only. Post the URL in a PM: comment.

## Close

Commit and push at every checkpoint. After merge, `cargo clean` and remove the worktree.
