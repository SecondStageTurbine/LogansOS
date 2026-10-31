<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Atrium app icons, relay #141 / #185

Reviewer: Jarvis. Worktree: `E:\AI Automation\workers\jarvis`.
Branch: `codex/review-bundle-2026-10-29b-jarvis`.
Read squash `40f60300`, the dispatch/design and PR evidence; current-source
references and host checks use main `87458b7e`. **PASS, with the placement
qualification in item 3.**

1. **PASS.** `tools/icon-raster/src/tests.rs:9` compares a 4x4 SVG against
   64 hand-laid bytes; :27 checks straight alpha. :147 checks all 21 committed
   BLAKE3 pins and :158 compares every blob to a fresh render. Together these
   catch both a changed committed blob and renderer drift. Demultiplication
   happens in `tools/icon-raster/src/lib.rs:60` before the blob is emitted.
2. **PASS for source exclusion; binary identity is author evidence.**
   `userland/neo-atrium/src/scene.rs:1230` retains the old letter/name drawing
   body under the off cfg, while icon painting (:922) and the alternate
   chip_text (:1266) are gated. The feature-off letter oracle still passes.
   #185's off-build table records equal neo-init/neo-atrium `.text`; those
   pairs were not rebuilt here. Product-on changes are intentional.
3. **PASS for the documented design, not literal vertical centering or an
   unchanged name baseline.** `userland/neo-atrium/src/icons.rs:36` places
   the 40x40 box at chip+(8,6): horizontally centered in 56px, **2px above
   vertical center**. NAME_BASELINE (:27) is 53 instead of the off path's 47.
   These are explicit design choices in #185, not identical label placement.
   Focus glow, selected-chip base rendering and opaque Reduce-Transparency
   material remain in the base pass. The icon blends over that result in the
   label pass (`scene.rs:920`). `icons.rs:41` uses the correctly rounded
   straight-alpha formula, and :54 clips destination writes to the label box.
   The clear two-pixel SVG margins make that clipping safe. Host checks cover
   both materials/focus states and a wrong-icon negative control.
4. **PASS by harness/source inspection plus recorded QEMU falsification.**
   The `40f60300` changes to `kernel/src/test_harness.rs` read the Arthur
   icon box and compare the independent pinned digest in
   `fixtures/desktop-ds1/icon_digests.rs:1`. Wrong Lens pixels produce a
   different digest. `tools/xtask/src/main.rs:15038` at the reviewed squash
   selects `desktop_scenario(scenario.name).capture` for the post-check,
   fixing the stale Home capture path. #185 records the wrong-blob scenario
   failure; this review did not rerun QEMU.
5. **PASS.** `tools/xtask/src/main.rs:6407` at current main includes
   `atrium-app-icons` in `desktop-with-model`'s neo-init feature list;
   `product_image_draws_the_app_icons` (:20404) asserts that list and the
   shared single image-A/product WM-build call. Its host test passes.

Executed on `87458b7e`: `cargo test -p icon-raster --locked --quiet`
(5 passed); `cargo test -p neo-atrium --features app-icons --locked --quiet`
(71 passed, 2 ignored); xtask `product_image_draws_the_app_icons` (1 passed).
Feature-off neo-atrium also passed (68 passed, 2 ignored) at `130a7ef3`.
[Batch 14b](https://github.com/SecondStageTurbine/LogansOS/actions/runs/40817887458)
was independently queried: success at `67f023bc99720804dc1e3a266cd50910c8325299`.
RAG healthy. No new binary comparison or QEMU execution claimed.
