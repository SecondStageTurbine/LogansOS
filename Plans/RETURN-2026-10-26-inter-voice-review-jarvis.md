<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay #85: Inter voice post-merge review (#145)

Reviewed SHA: `149431a185ebd8c5d3fe0a0bd3539a3e72bc38e1`.
Comparison base: `373a83f5`. Review-only; no production changes.
Reviewer: Jarvis / Codex-family. Scope: the userland text stack, bundled font,
goldens, licence staging and associated wording. No SMP or syscall work.

## Verdict

**PASS.** All eight checklist items pass. No confirmed defect or revert request.
The wider mean bound is supported by independent measurements, every retained
golden reproduces, and loaded binary growth closely tracks the font delta.
Validation limits: no full ISO rebuild or QEMU/bare-metal boot in this review.

## Checklist

1. **PASS — upstream, unmodified static font.** `assets/fonts/README.md:6` and
   `assets/fonts/README.md:28` describe the actual bytes. Downloaded the release
   directly with `curl.exe -L --fail` from
   `https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip`.
   Archive: 33,707,794 bytes, SHA-256
   `9883fdd4a49d4fb66bd8177ba6625ef9a64aa45899767dde3d36aa425756b11e`.
   Extracted `extras/ttf/Inter-Regular.ttf` compares byte-for-byte equal to
   `assets/fonts/Inter-Regular.ttf`: 411,640 bytes, SHA-256
   `40d692fce188e4471e2b3cba937be967878f631ad3ebbbdcd587687c7ebe0c82`.
   `userland/neo-text/src/tests/font_integrity.rs:73`
   (`shipped_fonts_are_canonical_sfnts`) passes, including the whole-sfnt
   checksum and expected name/copyright/licence records for both shipped faces.

2. **PASS — licence text and staging.** `assets/fonts/OFL-Inter.txt:1` has
   Inter's copyright and `:8` identifies OFL 1.1. The entire file equals the
   downloaded archive's `LICENSE.txt`: 4,380 bytes, SHA-256
   `262481e844521b326f5ecd053e59b98c8b2da78c8ee1bdbb6e8174305e54935a`.
   `tools/xtask/src/main.rs:88` lists Inter, Plex and NOTICE; `:6649` copies
   each into the staging tree's `licenses/`. `NOTICE:49` names Inter and
   `NOTICE:52` retains Plex Mono; `NOTICE:61` names both staged OFLs.
   `cargo test -p xtask --locked staged_licence_files_exist` passes.

3. **PASS — proportional face only.** `userland/neo-text/Cargo.toml:30`
   defines `inter`; `userland/neo-text/src/fonts.rs:10` embeds Inter and `:15`
   still embeds the same Plex Mono bytes. All five feature consumers migrated:
   `userland/neo-text/Cargo.toml:34`, `userland/neo-arthur/Cargo.toml:28`,
   `userland/neo-atrium/Cargo.toml:20`, `userland/neo-worker/Cargo.toml:23`,
   `tools/arthur-logic-tests/Cargo.toml:13`.
   `userland/neo-arthur/src/showcase_text.rs:136` selects Inter while `:137`
   still selects Plex Mono. Exact searches across userland/tools/scripts/assets
   find no executable `PLEX_SANS_REGULAR`, `plex-sans` or old TTF references;
   only the explicit historical explanation in `assets/fonts/README.md:48`
   remains. The old TTF is deleted. The kernel diff is confined to the test
   harness's font description/serial oracle (`kernel/src/test_harness.rs:36521`);
   the kernel console font and Plex Mono asset have no diff.

4. **PASS — relaxed exact-area mean bound is justified.** Independently ran
   a scratch host executable using the real `Font::rasterize`, aligned both
   bitmaps by their recorded bounds and summed absolute byte differences over
   the same union rectangle as `userland/neo-text/src/tests/mod.rs:173`.

   | Case | Sum / pixels | Mean | Max | Differing pixels |
   |---|---:|---:|---:|---:|
   | `a`, 13 px, offset .75 | 23 / 63 | 0.36507936507936506 | 1 | 23 |
   | `g`, 20 px, offset .5 | 39 / 187 | 0.20855614973262032 | 1 | 39 |
   | `o`, 24 px, offset 0 | 54 / 195 | 0.27692307692307694 | 1 | 54 |
   | `S`, 32 px, offset .25 | 65 / 425 | 0.15294117647058825 | 1 | 65 |

   Only `a` fails the former 0.35 bound. The 0.4 limit at
   `userland/neo-text/src/tests/mod.rs:233` is the tight rounded one-decimal
   bound; it is not the mathematical minimum (0.37 would also pass). It allows
   an integer sum of at most 25 here, only two more than measured. All measured
   differences are one byte value; the max-2 guard at `:232` remains. The
   rasterizer and its 0.005 px flattening tolerance
   (`userland/neo-text/src/raster.rs:20`) are unchanged. These results support
   ordinary curve-flattening/rounding error, not a hidden rasterizer regression.

5. **PASS — independently regenerated goldens; no tests dropped.** Actual
   `cargo test ... -- --list` output was collected at both revisions in this
   same worktree and compared: the test names and counts are identical.
   Neo-text: 28; neo-atrium: 65; arthur-logic-tests: 81. No new ignore attrs or
   removed test names. The normal post-change run passes 28 + 63 + 81 tests,
   with Atrium's existing `perf_native_1080p` and `preview` ignores
   (`userland/neo-atrium/src/tests.rs:207`, `:237`). Both were also run explicitly
   using `cargo test -p neo-atrium --locked -- --ignored` and passed. This
   includes the exact/FreeType/HarfBuzz cases in
   `userland/neo-text/src/tests/mod.rs:238`, `:284`, `:344`, the desktop pixel
   tests (`userland/neo-atrium/src/tests.rs:350`, `:411`, `:757`, `:815`) and
   Arthur's title oracle. Ran `scripts/desktop/check_fixtures.sh` in Ubuntu with
   a worktree-local Python environment: all 16 desktop fixture files reproduce
   byte-for-byte. Separately regenerated `fixtures/arthur-s2/title_window.rs:5`,
   all four `exact_*` tables (`userland/neo-text/src/tests/exact_a_13.rs:2` and
   its g/o/S peers), all four `ft_*` tables (`:2` in each), and `hb_kern_sans`.
   Arthur, exact-area and HarfBuzz outputs match in full. FreeType's only
   difference is its generated version comment (system 2.14.2 versus recorded
   2.13.2); bounds, dimensions and every coverage byte match. This establishes
   reproducibility from the independent generators, rather than relying on
   the author's account of how the tables were edited.

6. **PASS — independent HarfBuzz kerning.**
   `scripts/fonts/harfbuzz_kern_oracle.py:44` disables GSUB features so the
   independent shaper measures the same cmap glyphs as neo-text; `:87` shapes
   every printable ASCII pair. The committed table
   (`userland/neo-text/src/tests/hb_kern_sans.rs:4`) records 1,048 nonzero pairs,
   zero skipped. `every_ascii_pair_kerns_like_harfbuzz`
   (`userland/neo-text/src/tests/mod.rs:344`) passes all 9,025 pairs, including
   zeros. Running the oracle against Inter reproduces the committed Rust
   fixture exactly, including all 1,048 nonzero pairs and zero skipped pairs.

7. **PASS — footprint.** The raw proportional-font delta is
   411,640 - 200,500 = **211,140 bytes per linked copy**
   (`assets/fonts/README.md:6`, `:48`). Inter is embedded separately in Arthur,
   the WM (neo-atrium is a library linked into neo-init), and a Lens-enabled
   neo-worker; the total image delta depends on enabled roles, plus the new
   4,380-byte OFL and NOTICE changes. It must not be interpreted as exactly
   one font delta for every image configuration. Built the same custom-target,
   dev-profile configurations at each revision, with the pinned toolchain:
   `cargo build -Zjson-target-spec --bin neo-arthur --features desktop-session --locked`
   from Arthur's directory, and the corresponding `--bin neo-init --features
   desktop-glass` from init's directory. Neo-atrium has no separate executable.

   | Executable / quantity | Before | After | Delta |
   |---|---:|---:|---:|
   | Arthur ELF file | 30,346,968 | 30,555,864 | +208,896 |
   | Arthur sum of PT_LOAD file sizes | 767,823 | 978,983 | +211,160 |
   | Arthur sum of PT_LOAD memory sizes | 770,911 | 982,071 | +211,160 |
   | Atrium-enabled init ELF file | 28,309,624 | 28,522,544 | +212,920 |
   | Atrium-enabled init sum of PT_LOAD file sizes | 546,551 | 757,663 | +211,112 |
   | Atrium-enabled init sum of PT_LOAD memory sizes | 549,647 | 760,759 | +211,112 |

   Parsed the ELF64 program headers directly to separate loaded bytes from
   debug information. Load growth is within +20/-28 bytes of one font delta
   in each executable; nothing else balloons. The two measured loaded-file
   deltas sum to 422,272 bytes, consistent with two embedded font copies.
   A complete ISO was not rebuilt or booted; total image growth above is a
   configuration-dependent expectation, not a claimed ISO measurement.

8. **PASS — wording and design language.** The exact line is:
   "I can hear you, but my model isn't on this system yet. Boot the LogansOS USB it came with."
   It agrees at `userland/neo-arthur/src/model_state_logic.rs:49`,
   `userland/neo-atrium/src/intro.rs:81`, `tools/xtask/src/main.rs:16403` and
   `docs/THINK-2026-10-19-arthur-first-boot-model.md:296`. The introductory
   state/wording KATs pass (`userland/neo-atrium/src/tests.rs:2016`, `:2247`),
   including the real-font row-fit KAT (`:1950`). Design language section 6.1
   names Inter at `docs/THINK-2026-07-10-atrium-design-language.md:146`, `:152`
   and retains Plex Mono at `:147`.

## Reproduction and environment

- RAG health: healthy, Ollama true, Qdrant true; queried the dispatch's example.
  RAG results were discovery hints; conclusions use this worktree's pinned files.
- Ran `graft.cmd ask "post-merge review PR 145 Arthur voice Inter userland text stack font goldens licence staging" --source`
  before exact searches. Its graph was older than the reviewed change; did not
  use its Plex-era licence-test snippet as current evidence.
- Windows host Rust: cargo 1.97.0-nightly, repository-pinned nightly-2026-04-16.
  `cargo test -p neo-text -p neo-atrium -p arthur-logic-tests --locked` passes.
- Initial Ubuntu oracle attempts lacked fontTools/NumPy. Installed them only
  into ignored `target/inter-review-venv` (fontTools 4.67.0, NumPy 2.5.4);
  scratch review tools and outputs
  stay under `target/`. No dependency or fixture source edits.
- Pre-existing `.gitignore`/`.ignore` edits blocked the initial checkout. They
  were preserved in stash `jarvis-pre-review-local-ignore` (on the previous
  seat branch). The assigned revision already contains the same ignore rules.

## Closeout

Deliverable: only this RETURN, on `codex/inter-voice-review-jarvis`. No source,
asset, fixture, or dependency changes. Commit SHA is supplied in the handoff
after pushing. Final checks required: `git status --porcelain` empty and
`git log '@{u}..'` empty.
