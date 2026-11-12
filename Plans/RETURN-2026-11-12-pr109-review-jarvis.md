<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Jarvis review of #109 (S6b Language page and catalog)

Reviewed head `3855d53db5a59f48100f64304850f9d1569bb760` (`claude/setup-s6b-lang`) against Section 401 of `Plans/DISPATCH-2026-11-12-relays-400-402.md` and `Plans/DISPATCH-2026-11-12-seat-c-setup-s6b-language.md`.

1. **Kill-switch `i18n-catalog` — PASS.** `setup-core` applies `with_lang` only when the feature is enabled; the Language page is otherwise hidden in the production build (the legacy S1 stub can still be enabled explicitly with `stub_pages`) (`userland/setup-core/src/lib.rs:60-62,410-416,436-448`). With the feature off, the default is `Lang::En` (`userland/i18n-core/src/lib.rs:41-46`; `userland/neo-atrium/src/lang.rs:13-22`). The off-feature setup, time-zone, network and keyboard KAT modules pin the rows (`userland/setup-core/src/lib.rs:790-812`; `userland/setup-core/src/tests.rs:12-45,47-118`), and the PR records the unchanged neo-atrium and English civil KAT evidence. The English control checks today's Welcome, intro and help digests (`tools/xtask/src/main.rs:22353-22390,24663-24770`); the existing fixtures remain untouched. User-facing code reads the catalog rather than retaining cfg-gated duplicate wording (`userland/i18n-core/src/stray.rs:21-29,238-274`).

2. **Drift KAT — PASS.** The English table is compared byte for byte with `fixtures/en.txt` (`userland/i18n-core/src/tests.rs:121-151`). The stray-literal KAT scans setup wizard and keyboard, neo-init setup/zone-verb/clock, neo-atrium intro/clock/power and intro-text painter, plus `libneo::civil`; it checks each worded literal against the reasoned command, zone and log allow-list (`userland/i18n-core/src/stray.rs:21-80,238-274`).

3. **Catalog completeness — PASS.** Each message id is checked in both languages for nonempty text and no fallback, with argument counts checked (`userland/i18n-core/src/tests.rs:25-48`). The longest realistic rows are measured against both reference card sizes (`userland/i18n-core/src/tests.rs:206-215,327-360`). The S4 Network page strings, including lease forms, are in the catalog (`userland/i18n-core/src/lib.rs:203-220`).

4. **Text handling — PASS.** `card_buttons` uses `text.get(..at)` for its ASCII bracket offsets, so an invalid UTF-8 boundary returns no slice rather than panicking (`userland/neo-atrium/src/scene.rs:565-575`). Arthur's intent receiver rejects bytes outside printable ASCII before copying the intent (`userland/neo-arthur/src/main.rs:1656-1675`). English and Spanish date/time outputs, including the Spanish ordering and suffix, have explicit civil KATs (`tools/rtc-logic-tests/src/lib.rs:554-577`); the date formatter consumes each language's indexed pattern (`userland/libneo/src/civil.rs:221-239,242-255`).

5. **Settings — PASS.** The enabled `LANGUAGE_COUNT` is sourced from `i18n_core::LANGUAGE_COUNT`; the disabled count remains one (`userland/settings-core/src/setup_ids.rs:8-13`). `Key::Language` accepts only values below that count (`userland/settings-core/src/lib.rs:156-168`), and invalid stored values cause unpacking to fall back to `Settings::default()` (English) (`userland/neo-init/src/desktop_session/settings.rs:129-143`).

6. **Scenario oracles — PASS.** `setup_menu_lang` asserts the Spanish walk, stored `lang=1`, live language change, and absence of writes; `setup_menu_lang_en` asserts English remains stored and today's digests remain (`tools/xtask/src/main.rs:22306-22390`). Captured pages are compared against the Spanish and English oracle fixtures, including the control's unchanged Welcome, intro and help (`tools/xtask/src/main.rs:24661-24770`).

7. **Prior art — PASS.** The #109 PR body, under “Prior art,” says S6a's survey still applies and links `userland/i18n-core/PRIOR-ART.md`.

No tests were run; this was a source, KAT and PR-body review.
