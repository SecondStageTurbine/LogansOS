<!-- SPDX-License-Identifier: MPL-2.0 -->
# i18n-core: prior-art survey

The survey the unfamiliar-feature rule (CLAUDE.md) and the S6a dispatch
(`Plans/DISPATCH-2026-11-09-seat-c-setup-s6a-i18n-catalog.md`) ask for, before writing a catalog of our own. It is
also in PR #101's body.

**How it was checked (2026-11-09):**
- **Licence and latest version:** the crates.io API.
- **Last commit:** a shallow fetch of each repository's default branch (`git fetch --depth 1`), with its SHA and
  commit date.
- **`no_std`:** read from the crate root and its `Cargo.toml` at that commit.

| Candidate | What it is | Licence | Last commit (date, SHA) | `no_std`? | Fit |
|---|---|---|---|---|---|
| `fluent-bundle` 0.16.0 ([projectfluent/fluent-rs](https://github.com/projectfluent/fluent-rs)) | Mozilla's Fluent: `.ftl` resources, selectors, plurals, a runtime parser | Apache-2.0 OR MIT | 2026-09-27, `0f87afbbc3fe7d20c61a3fc75cb5d0e123aefdda` | No: std, with `intl-memoizer` and `unic-langid` | Built for plurals and grammar we don't have; parses its resources at runtime |
| `rust-i18n` 4.2.4 ([longbridge/rust-i18n](https://github.com/longbridge/rust-i18n)) | YAML/JSON/TOML files compiled behind a `t!` macro | MIT | 2026-09-29, `5f3ea5ba03395f2f2908e3587e01788cfdfd3555` | No: std, serde, YAML at build time | A string-keyed map that needs std; no compile-time check that every key exists in every language |
| `gettext-rs` 0.8.0 ([gettext-rs/gettext-rs](https://github.com/gettext-rs/gettext-rs)) | Bindings to GNU gettext (`.po`/`.mo`) | MIT for the bindings; links GNU libintl (LGPL) | 2026-10-17, `8a96df9da502b3a66411e07451670e47fe48bb54` | No: C FFI | A C library, an LGPL runtime and a locale environment; doesn't fit a `no_std` ring-3 desktop |
| ICU4X `icu_experimental` 0.7.0, MessageFormat 2 ([unicode-org/icu4x](https://github.com/unicode-org/icu4x)) | Unicode's message formatting with CLDR data | Unicode-3.0 | 2026-11-05, `0a73cef7590cac66795c16f6300fceea29204c77` | Yes (`no_std` + alloc) | The right long-term home for plurals and locale dates, but still experimental, with a large data footprint, for 120 strings with positional arguments |
| `include_str!` / `const` tables of our own | An enum id indexes one `&'static str` array per language | MPL-2.0 (ours) | — | Yes | **Chosen** |

**Why our own table:** we have no plurals and no locale formatting yet, only 120 fixed strings with positional
arguments. A `const` array per language indexed by an enum costs no dependency. A missing translation is a compile
error (the arrays' length) or a KAT failure (an empty entry). `fill` covers the only formatting the strings use.

**Nothing is borrowed or vendored.** If S6b or later needs plurals or locale date formats, ICU4X's `icu_plurals` and
`icu_datetime` (Unicode-3.0, allowed by CLAUDE.md since 2026-10-31) are the candidates to revisit.
