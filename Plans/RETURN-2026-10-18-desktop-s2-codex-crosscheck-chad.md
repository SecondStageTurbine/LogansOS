<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Desktop S2 cross-family adversarial review (Chad)

**Verdict:** FIX-FORWARD (BOUNCE-equivalent; PR #31 is already merged as `7deb10f2`).

**Reviewed:** the merged S2 change and its current descendants on `origin/main` at `1a840e54`, including neo-text, Arthur integration, both compositor paths, font assets/converter/licensing, host and kernel oracles, and kill-switch. The semantic index was healthy and used for discovery; final evidence was checked directly.

## Findings

### 1. HIGH (release integrity) — the generated sfnt files have invalid whole-font checksums, so the “unmodified conversion” / Reserved Font Name basis is not established

**Evidence:** `scripts/fonts/woff_to_sfnt.py:1-48`; `assets/fonts/README.md:21-36`; `assets/fonts/OFL-IBM-Plex.txt:39-42,56-65`; the `head.checkSumAdjustment` fields in both committed fonts.

The converter copies the WOFF tables and constructs a new sfnt directory but does not recompute `head.checkSumAdjustment`. Recomputing the OpenType whole-font checksum over the committed files gives:

- Plex Sans: `0x6D69BC6D`
- Plex Mono: `0xB1B2A332`
- required magic checksum: `0xB1B0AFBA`

Thus both generated sfnts are structurally checksum-invalid even though `ttf-parser` accepts them. This also undercuts the repository's categorical statement that the unwrap is an unmodified font. The bundled OFL text defines changing formats as a Modified Version and forbids a Modified Version from using the Reserved Font Name without permission. The OFL FAQ supplies a narrow WOFF exception when the original font data and metadata remain unchanged, but the malformed reconstructed sfnt is not adequate evidence that this exception was met: <https://openfontlicense.org/ofl-faq/> (2.2–2.2.2).

There is a second distribution risk: the boot image embeds the fonts in `neo-arthur` but does not stage the full OFL text. The name records contain a short licence sentence and URL, not the full text, and the OS exposes no ordinary font-metadata viewer. OFL §2 requires each redistributed copy to contain the notice and licence in a standalone, human-readable, or easily viewable machine-readable form. This is release-integrity review, not legal advice; the current artefact should not be represented as clearly compliant without an owner decision.

**Fix direction:** use an upstream canonical TTF whose provenance/hash is pinned, or produce a spec-valid sfnt and obtain an explicit licensing determination for the RFN. Stage `OFL-IBM-Plex.txt` (and the relevant third-party notices) into the shipped image in a user-readable location. Add a build test that sums every shipped sfnt to `0xB1B0AFBA` and checks the expected licence/name records.

### 2. MEDIUM — S2's band damage is discarded by the production desktop WM, so the advertised damage optimization does not exist there

**Evidence:** `userland/neo-arthur/src/showcase_surface.rs:239-282`; `userland/libneo/src/atrium_protocol.rs:131-134`; `userland/neo-init/src/desktop_session.rs:573-584`; `kernel/src/atrium/buffer.rs:357-364,420-424`.

Arthur publishes the text band's damage into its client `SharedPixelBuffer`, then sends an Atrium `Commit` message containing only identity and serial. The standalone compositor calls `sys_fb_present` on that same client buffer, so the kernel consumes the pending damage there. The desktop WM does not present Arthur's buffer: it maps the buffer RO, receives the separate protocol message, and always calls `scene.present(Some(client), geo.client)`. The client buffer's pending damage is never consumed by this path, and the WM recomposes and commits the entire 640×400 client rectangle on every typed byte/token.

This does not leave stale pixels—the full repaint is conservative—and synchronous request/reply prevents two protocol commits from overtaking one present. It does mean S2's “band-sized damage” claim is false for the production desktop and S3's per-typing cost includes a full client rectangle. Existing S2 damage tests exercise `TextSurface::take_damage` and the direct-present compositor, not propagation through the S1 WM.

**Fix direction:** carry the validated damage rectangle in the surface protocol (or add a compositor-readable consume/query operation tied to the commit serial), translate it to output coordinates, and make the WM union it with any chrome/focus damage. Add a desktop-session host/QEMU assertion that one keystroke commits only the translated line band.

## Dispatch checklist

1. **Rasterizer / flow safety:** line endpoints are clamped before indexing; `width + 2` covers the right-edge deposits; NaN casts collapse to index zero rather than escaping the allocation, and infinities clamp to the bitmap edge. Font coordinates and sizes are trusted/fixed in Arthur. Canvas clips before casting negative coordinates to `usize`; scrolling clamps `dy`. `String::pop`, `char_indices`, and the ranges returned by `line_break` preserve UTF-8 boundaries (including U+00B7); tab/control recursion is bounded; a narrower-than-one-glyph line still advances. Generic public constructors still rely on sane dimensions (`Rasterizer::new(width + 2)` and i32 rectangle arithmetic are not overflow-hardened), but no shipped input reaches pathological values.
2. **`&'static mut [u32]`:** within Arthur there is one owner and all writes are bounded. `raw3` omits `nomem`, so the inline `asm!` is a compiler memory barrier; stores cannot legally sink past the commit syscall or be erased as unobservable. The cross-process ordinary-reference issue is the same shared-memory abstraction concern reported in the S1 RETURN, not a new S2 regression.
3. **Damage:** `TextFlow` unions every touched band; wrap redraws old and carried lines, scroll marks the entire used region, and backspace redraws its current band. Direct-present commits are synchronous, so pending damage is not overwritten before present. Found the production-WM propagation omission above.
4. **Oracle independence:** the exact curved-glyph fixtures use a different f64 polygon clipper, FreeType provides a second sanity reference, and HarfBuzz exhausts printable ASCII kerning. The 28×20 kernel window cannot see kerning, but the all-pairs HarfBuzz host test closes that gap for shipped printable text. The OpenType spec requires moving to the next subtable when a glyph is absent from Coverage; it does not clearly require the class-0/zero-record fall-through, so that rule is HarfBuzz-compatibility rather than spec-derived. The exhaustive HB output oracle makes it acceptable for these faces. Re-derived first `H` row: left stem areas `0.89×0.96` and `0.79×0.96` round to 218 and 193; right stem fragments `0.15×0.96`, `1×0.96`, `0.53×0.96` round to 37, 245, 130, matching `H_20PX_QUARTER`. Overlapping/opposite contours remain a coverage gap, but no shipped-font failure was found.
5. **Licence / OFL:** finding 1. `ttf-parser` is MIT/Apache-2.0; `core_maths` and `libm` are MIT as recorded. Their licences should be included in the eventual third-party notice bundle, but no incompatible code licence was found.
6. **Image / stack budget:** the fonts reside in ELF read-only load data, not on the user stack; no loader size ceiling near 370 KiB was found. The stack fix optimizes `ttf-parser` and `neo-text`, but no high-water measurement proves margin. A cheap close is to paint the 64 KiB stack before `TextSurface::new`, first 22 px glyph, first GPOS lookup, and move into `ArthurSurface`, then report the deepest overwritten word; preserve a guard page below it.
7. **Kill-switch:** the legacy renderer's byte behavior matches its pre-S2 volatile reference over wrapping, scrolling, control bytes, case folding, and backspace. Plain stores are ordered by the syscall memory clobber. The host KAT is strong for output equality, but there remains no end-to-end QEMU kill-switch run; add one when touching this seam again.
8. **Other:** finding 2 is the cross-feature blind spot. No additional panic, stale-pixel, loader, or shaping defect survived re-verification.

## Validation performed

- `cargo test -p neo-text` — PASS, 26/26.
- `cargo test -p arthur-logic-tests s2` — PASS, 7/7 selected.
- Parsed both fonts with fontTools `ttx`; independently recomputed their whole-file checksums as reported above.
- Static review of the OpenType lookup rules and the project's bundled OFL plus the OFL FAQ.

QEMU was not run.
