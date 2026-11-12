<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: PR #101 five S5 strings (Chad, relay 390)

**Dispatch:** `Plans/DISPATCH-2026-11-11-relay-390-pr101-s5-strings.md`.
**Reviewed commit only:** `3d994252aa491bc84a69454c587f009da6459157` (`claude/setup-s6a-i18n-catalog`).
**RETURN branch:** `codex/pr101-recheck2-chad`.
**Verdict:** PASS for all four dispatched checks. No changes requested.

1. **PASS — five English strings match byte for byte; drift KAT reads the real sources.** Catalog entries at `userland/i18n-core/src/lib.rs:139-148` match the following shipped literals:

   | Id | English | Source at reviewed commit |
   |---|---|---|
   | `SetupKeyboardBodyChoose` | `Choose the layout printed on your keys.` | `userland/setup-core/src/keyboard.rs:89` |
   | `SetupKeyboardTryIt` | `Try it: {}_` | `userland/setup-core/src/keyboard.rs:90` |
   | `SetupSummaryKeyboard` | `Keyboard: {}` | `userland/setup-core/src/keyboard.rs:95` |
   | `LayoutEnglishUs` | `English (US)` | `kernel/src/drivers/keymap_layout_logic.rs:34` |
   | `LayoutSpanishSpain` | `Spanish (Spain)` | `kernel/src/drivers/keymap_layout_logic.rs:34` |

   `src/tests.rs:89-90` includes those two real source files; `:181-190` binds all five new ids to them. The existing `SetupBodyKeyboard` binding now uses `KEYBOARD` (`:175-179`), matching its literal at `setup-core/src/keyboard.rs:36`. The drift KAT (`i18n-core/src/tests.rs:558-584`) checks source literal presence, English equality, and source coverage for every id; it passes.

2. **PASS — natural, neutral Latin American Spanish, with meaning and short imperative tone preserved.** `Elige la distribución impresa en tus teclas.` (`src/lib.rs:140`) gives the same instruction in familiar singular address; `Pruébalo: {}_` (`:142`) is a natural short invitation to test the keyboard and preserves the field and cursor. `Teclado: {}` (`:144`) preserves the summary label. `Inglés (EE. UU.)` (`:147`) and `Español (España)` (`:148`) are natural Spanish layout names, with the regions correctly distinguished: the Spanish option is Spain's layout, even for an es-419 interface. The accents and spacing in `EE. UU.` are correct. **Lines I would change: none.**

3. **PASS — width and glyph KATs cover all five new ids and pass.** Both iterate `Msg::ALL` in both languages: glyph coverage at `src/tests.rs:590-610` checks the shipped Inter Regular glyph map (`:600-604`); width coverage at `:726-759` measures the card at 1920×1080 and 1280×800 (`:733-738`). The new field uses 24 `W`s (`:694-696`), matching `setup-core/src/keyboard.rs:64`; the summary fills a layout name (`i18n-core/src/tests.rs:697`), and layout options include the focus and checkbox prefix (`:714-717`). At the exact reviewed head, `cargo test -p i18n-core -- --nocapture` finished with **7 passed, 0 failed, 1 intentionally ignored** (the optional width report), including the drift, width and glyph KATs. The reported widest row remains Spanish `IntroModelAbsent`, 653.4 of 676 px (96.7%).

4. **PASS — nothing in this commit touches image A.** `git diff-tree --no-commit-id --name-only -r 3d994252` lists only `userland/i18n-core/{PRIOR-ART.md,TRANSLATION-NOTES.md,src/lib.rs,src/tests.rs}`. No kernel, runtime consumer, image recipe, dependency manifest or lockfile changes occur in this commit. The catalog remains unlinked: a tracked Rust/manifest search finds no runtime consumer of `i18n-core`/`i18n_core`, only its package declaration and workspace membership. This is a source-diff verdict for the dispatched commit; no image rebuild or binary identity comparison was performed.
