<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: PR #101 setup S6a i18n catalog review (Chad, relay 384)

**Reviewed head:** `b33788db8a763f384ef28017e9869d6b4415c503` (`claude/setup-s6a-i18n-catalog`).
**Review branch:** `codex/pr101-review-chad`.
**Validation:** `cargo test -p i18n-core` — 7 passed, 1 intentionally ignored.
**Verdict:** FAIL. The host-only catalog and KATs are present, but the Spanish wording has a documented meaning loss and the required prior-art survey is absent.

## Section 384 checks

1. **PASS — nothing in image A links `i18n-core`, and image A's ELFs are unchanged.** The crate is added as a workspace member at `Cargo.toml:19`; its only dependencies are test-only `neo-atrium` and `neo-text` at `userland/i18n-core/Cargo.toml:11-14`. No image-producing crate or target dependency was changed in the reviewed range `52c0ed0e..b33788db` (the range changes only workspace metadata, the host crate, its tests/notes, and the dispatch).

2. **PASS — English drift KAT and table completeness.** `Msg::ALL`, `MSG_COUNT`, and the two same-sized tables are generated together at `userland/i18n-core/src/lib.rs:77-99`; the test checks every id in both languages and rejects fallback/empty entries at `userland/i18n-core/src/tests.rs:25-46`. The English table is checked against the current setup, intro, clock, power, civil, and desktop source literals at `userland/i18n-core/src/tests.rs:88-94,545-573`, with named format arguments normalized to the catalog's positional `{}` form before comparing the bytes printed to users (`:517-543,556-560`).

3. **PASS — font coverage reads the shipped font's real cmap.** The KAT parses `neo_text::fonts::INTER_REGULAR` with the text engine's `Font::from_bytes`, then checks every character with `font.glyph_id`, reporting each missing code point (`userland/i18n-core/src/tests.rs:577-599`). It does not assume a Latin-1 range. The focused test passed.

4. **FAIL — Spanish wording is not fully natural/meaning-preserving.** The notes themselves identify a meaning change: `IntroModelAbsent` drops “it came with” (`userland/i18n-core/TRANSLATION-NOTES.md:80`), changing the concrete USB instruction. Suggested replacement: `Te escucho, pero mi modelo aún no está en este sistema. Arranca el USB de LogansOS que venía con esta computadora.` The acronym choice also leaves the English expansion embedded in Spanish (`:64-65`); either retain it explicitly as a named English acronym with a natural Spanish gloss, or use `Asistente para tareas en tiempo real, automatización del hogar y solicitudes de usuario` and document the acronym decision. I would also change `Tab los alcanza` (`:76`) to `Puedes acceder a ellos con Tab`, and `Haz la pantalla más tranquila` (`:54`) to `Haz que la pantalla sea más tranquila`.

5. **FAIL — prior-art survey missing.** The dispatch requires a table covering `fluent`/`fluent-bundle`, `rust-i18n`, `gettext-rs`, ICU4X, and `include_str!` approaches, with each license, last commit date/SHA, `no_std` fit, and justification for our own table. The reviewed PR range contains no such table or survey; `userland/i18n-core/Cargo.toml:1-14`, `src/lib.rs:1-25`, `src/tests.rs:1-12`, and `TRANSLATION-NOTES.md:1-5` contain no prior-art record. Add the required table to the PR before treating this check as pass.

## Notes

- No image or runtime test pass is claimed beyond the focused host KATs above; this is a host-only review at the dispatched head.
