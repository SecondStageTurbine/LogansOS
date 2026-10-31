<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Desktop S3 cross-family adversarial review (Chad)

**Verdict:** FIX-FORWARD (MERGE-WITH-NITS equivalent; PR #36 is already merged as `b3c4de55`).

**Reviewed:** the merged S3 change and its current descendants on `origin/main` at `1a840e54`: normative spec, Rust renderer, Python oracle, WM integration, kernel gates, RT/classic kill-switches, screenshots, accessibility and perf claims. The semantic index was healthy and used for discovery; final evidence was checked directly.

## Finding

### MEDIUM — the label masks make the trust subtitle and chip label invisible to every exact scene discriminator

**Evidence:** `fixtures/desktop-s3/scene_digests.rs:5-10`; `fixtures/desktop-s3/bar_label_rt.rs:5-6`; `fixtures/desktop-s3/title_label_rt.rs:5-6`; `kernel/src/test_harness.rs:31258-31332`; `userland/neo-atrium/src/scene.rs:402-502`.

The independent digest zeros three large boxes: bar 170×24, title 240×26, chip 50×46 (12,620 pixels total). The RT exact fixtures then restore coverage only for the 70×24 “Atrium” window and the 52×26 “Arthur” window. The remaining checks merely require eight bright pixels somewhere in each full box.

A mutation deleting the second title `draw_line` (`"· owned"`) passes: the whole title box remains masked, the exact `Arthur` window is unchanged, and `Arthur` supplies ample bright ink. Deleting/corrupting the chip's “Arthur” text also passes because its “A” supplies ink. A wrong material, hairline, or stray pixels inside the unused portions of any mask likewise evade the scene digest. In Glass mode there is no exact label-pixel check at all, only ink; the fact that the label code is shared with RT narrows but does not close background-dependent blend errors.

This is specifically important because `· owned` is part of the explicit trust communication, not decorative text. The hue-independent corner ticks survive, so this is not an accessibility failure in the shipped screenshot; it is a discriminator that can pass after losing one of the intended trust channels.

**Fix direction:** add independent fixtures for the full title (`Arthur · owned`) and both chip lines, in RT and at least one non-flat Glass background; mask only the exact glyph windows or independently assert every masked pixel. Add the deletion of the `· owned` draw call as a required negative control.

## Dispatch checklist

1. **Oracle independence:** the current v1.3 spec now forces the blur margin (`3R`), H-then-V order, `+R` rounding, Bayer threshold, truncation-toward-zero ring distance, half-up corner coverage, geometry, tint and draw order. The Python uses cumulative sums/f64 integrals while Rust uses sliding sums/libm, so the narrowed “algorithm, not design constants” claim is honest. The surviving non-independent convention is the broad label masking described above. Hand derivations: for a 3×3 impulse 900 with r=1, horizontal center is `(0+900+0+1)/3=300`, then vertical center is `(0+300+0+1)/3=100`; for r=1, the corner pixel area is `π/4`, and `floor(255π/4+0.5)=200`. Both match the Rust KATs.
2. **Typing never re-blurs:** `BaseScene::build` is the only caller that constructs the four cached region blurs (five with Home); `paint_rect`, `set_focused`, glow fill, and client composition only sample cached data. The counter baseline is taken after construction and focus withdrawal occurs before the destroy-time log, so current paths are covered. A log at 32 frames is only an interim witness (a hypothetical later reblur in an endless session would wait for destroy), but no rebuild/resize/RT-toggle path exists. `paint_rect` cost is independent of the blur algorithm.
3. **Memory safety / panics:** `Layout::new` proves the 800×538 minimum before every subtraction; odd modes merely floor centering. Production callers invoke `Blurred::at` only after the corresponding panel `contains` check. The maximum box sum is `17×65535=1,114,095`, far below u32; backdrop i64 arithmetic is safe at 8K and the session already rejects dimensions above u16. Damage is clipped before indexing. Public low-level helpers assume coherent `(plane,w,h)` and in-bounds blur rectangles, but no hostile runtime value reaches them.
4. **Kill-switches:** with `desktop-glass` off, painting still calls the S1 routines and `desktop_session_boot` pins the exact S1 framebuffer. The integration refactored the classic path through `Geometry`, so this is output equality, not proof of `.text` identity. RT constructs no `Blurred` values and panel interiors use pinned opaque tokens. Anti-aliased rounded-edge pixels still composite over the backdrop, as the shape spec requires; there is no blurred/backdrop sample in the opaque chrome interior.
5. **Accessibility:** from the independent oracle's brightest focused Glass backgrounds at 1920×1080, primary white has minimum contrast 8.95:1 in the title box and 8.01:1 on the chip; secondary `#C8CDD2` is 5.59:1 and 5.01:1 respectively, above 4.5:1 for the small text. The owned corner tick versus adjacent panel is about 3.48:1 in Glass and 4.88:1 in RT, and its L-shaped geometry plus hairline remains recognizable in greyscale. Hue is therefore not the only signal.
6. **Perf numbers:** TSC under TCG is useful only for within-run relative comparisons/path regressions, not wall-time or hardware frame-budget claims; `_rdtsc` is also not serializing. In addition, S2's client damage is not propagated through the desktop protocol, so a “typing damage frame” currently recomposes the full 640×400 client rectangle (reported separately in the S2 RETURN). The region blur already uses O(n) running sums. Plausible next savings are channel/SIMD fusion or a lower-resolution blur; a full-screen cache would compute substantially more area than these four selective regions.
7. **Visual quality:** inspected both committed 1920×1080 PNGs at native resolution. No visible banding, blur-margin seam, rounded-corner seam, glow clipping, or bad title baseline was found. The focus glow is visually stronger than the frost and the large rings read more as precise orbital lines than soft flare stars, but the result is coherent rather than broken; RT is visibly flatter and calmer.
8. **Other:** the exact-label blind spot is the main cross-family finding. The S1 input-loss and S2 damage/font findings are recorded in their own RETURNs and remain relevant to the composed desktop; no additional S3-specific concurrency issue survived review.

## Validation performed

- `cargo test -p neo-atrium` — PASS, 21 passed / 2 ignored.
- `python scripts/desktop/scene_oracle.py fixture` — logical text exactly reproduces `fixtures/desktop-s3/scene_digests.rs` (normalizing Windows stdout encoding/newlines only).
- Independently calculated Glass contrast over every pixel in the three label boxes and greyscale tick/panel contrast.
- Visually inspected `docs/screenshots/desktop-s3-glass.png` and `desktop-s3-rt.png` at 1920×1080.

QEMU was not run.
