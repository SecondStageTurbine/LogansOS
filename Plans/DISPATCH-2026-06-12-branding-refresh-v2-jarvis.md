<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Branding refresh v2 — new LogansOS logo + palette re-center (blue/silver) → Jarvis (worker tree)

**Date:** 2026-06-12
**From:** PM/Cerebro
**To:** Jarvis (worker tree on the Windows box — it has the source files in `C:\Users\<user>\Downloads\` and can run/edit `generate.py`; new branch off `origin/main` `77623a0`: `codex/branding-refresh-v2-jarvis`)
**Status:** DISPATCHED. **Local build + QEMU boot screenshot; no silicon. PL-ratified new identity + the brand-forward/function-preserving palette mapping below.**
**Supersedes the look from:** the 2026-06-06 branding work (`Plans/DISPATCH-2026-06-06-logansos-branding-assets-prometheus.md`, ADR-…) — the circular cyan/magenta "L" badge is retired; the new identity is a blue lens-flare star + broken concentric ring arcs + the "LogansOS" wordmark, navy/electric-blue/silver.
**Coordination:** disjoint from the active 7.5-B branch (USB drivers). This touches `assets/`, `tools/branding/`, `docs/`, `kernel/src/drivers/fb/mod.rs`, `kernel/src/init/boot_frame.rs`, `kernel/assets/` — none overlap `kernel/src/drivers/usb/`. Land it on its own branch; PM sequences the merges.

## Source files (on the PL's machine)
- **Black-background canonical:** `C:\Users\<user>\Downloads\ChatGPT Image Jun 7, 2026, 11_47_34 PM.png`
- **Transparent:** `C:\Users\<user>\Downloads\LoganOSTransparent.png`

---

## Part A — the logo assets (the design changed shape, so the pipeline changes)
The old logo *filled a circle*, so `generate.py` derived the transparent by a circular corner-mask. **The new logo is not a circular badge** (centered star + arcs + a wordmark below, with irregular negative space), so the circular mask is wrong — **the PL's hand-made transparent is the source of truth, not a derived artifact.**

- **ISC-A1 — Place both sources.** Copy the black-bg → `assets/branding/LogansOS.png` (canonical); copy the transparent → `assets/branding/logansos-transparent.png`. These are now **two inputs**, not one input + one derived.
- **ISC-A2 — Rework `tools/branding/generate.py`.** It must no longer apply the circular mask or derive the transparent. New derivation:
  - `logansos-transparent.png` = the PL's transparent, used as-is (the *source* for the ladder).
  - `logansos-logo-512/256/128.png` (and `logansos-icon-128.png`) = deterministic resizes of the **transparent** (preserve alpha; the ladder is transparent-backed). Stdlib-only (`zlib`+`struct`), as today.
  - `kernel/assets/logansos_boot_logo_160.rgba` = a 160×160 RGBA8888 raw blob (exactly `160*160*4 = 102400` bytes) derived from the **black-bg canonical** (the framebuffer clears to black and blits opaque, so a black-backed crest is seamless). **Crop note:** the boot blob is the *badge/ring/star region only* — NOT the full composition-with-wordmark (the framebuffer draws the "LogansOS" wordmark as text separately, see Part B); pick a square crop of the ring/star so it reads at 160px, then resize. Record the crop box + commands in the branding README.
  - Keep the `assert len == 102400` in the script (the byte oracle), and `boot_logo.rs`'s `const _: () = assert!(WIDTH*HEIGHT*4 == RGBA.len())` must still compile.
- **ISC-A3 — Update `assets/branding/README.md`.** Describe the new identity (blue lens-flare star, broken concentric silver/white ring arcs, "LogansOS" wordmark, navy field) and the **new derivation rules** (transparent is a source; ladder from transparent; boot blob = cropped canonical → 160²; the exact `generate.py` command + crop box). Drop the cyan/magenta "circular badge" description.
- **ISC-A4 — Reference + orphan sweep.** README crest already points at `logansos-transparent.png` (no change needed). Grep for any stale references to the *old retired* assets and the prior design language; enumerate the result in the close (literal, not "all clear"). No orphaned image files.

## Part B — the palette re-center (brand-forward, function-preserving)
The new identity is **navy / electric-blue / silver**. Re-center the brand on it **without destroying the functional color semantics** (success/warning/danger must stay distinguishable). **Sample exact hexes from the new artwork** — do not eyeball; read the pixels (the bright star core, the blue glow, the silver arcs, the field black).

- **ISC-B1 — `docs/design-system.md` (the canonical palette).** Apply this structural mapping; fill exact values by sampling the logo:
  - **Void / background:** keep `#0a0e27` (already a deep blue-black; matches the new field). Optionally deepen toward the logo's measured black — your call, record it.
  - **Primary accent (was Cyan `#00ffff`):** → **electric blue** (sample the star/glow; ~`#3a8eff`±). Rename the role to the new hero blue; update the "System info / primary accent" usage row.
  - **Silver (NEW):** add the ring/wordmark silver (sample; ~`#c2cad6`±) as a named structural neutral / secondary text.
  - **White `#ffffff`:** keep (primary text/focus + star core).
  - **PRESERVE functional semantics:** **Lime `#00ff41`** (success) and **Amber `#ffaa00`** (warning) stay — they encode meaning, not brand.
  - **Magenta `#ff00ff`:** **demote** from "secondary brand accent" to **Alert/fatal only** (its `panic.rs` role). It is no longer a brand color — keep it strictly as the high-contrast "something is very wrong" signal. Update the table + the CSS vars (`--neo-color-*`) accordingly (the new blue gets a var; magenta stays as the alert var).
- **ISC-B2 — `kernel/src/drivers/fb/mod.rs` (the code constants).** Re-value (and rename for honesty — a const named `CYAN` must not hold blue): `NEON_CYAN` → `NEON_BLUE` (sampled electric blue, e.g. `rgb(0x3a,0x8e,0xff)`); `DIM_CYAN` → `DIM_BLUE` (a dim blue for rules/feed); `NEON_MAGENTA` stays (alert/panic). Update the ~handful of consumers: `boot_frame.rs` (title/version/rules + the feed lines) and `panic.rs` (unchanged — keeps magenta). Keep the constant SET minimal; no new colors beyond Silver if you add one for text-secondary.
- **ISC-B3 — Prose surfaces.** `docs/design.md:45` ("cyan-magenta neon accents") and `docs/ADR-024` (§ primary `#00ffff` / secondary `#ff00ff`) → update to the new language (electric-blue/silver hero, magenta reserved for alerts). Do **not** rewrite normative ADR decisions beyond the color values — just the palette facts. The AltimitNeo/.hack visual lineage prose can stay (it's history); only the live palette changes.
- **ISC-B4 — Boot renders correctly (the gate).** The boot splash = the new 160² crest blob blitted above the text wordmark, with the re-valued accents. **Boot in QEMU and screenshot** — the crest reads, the wordmark is legible, the accents are the new blue, the panic strip (if exercised) is still the alert color. The screenshot is the visual oracle (paired with the 102400-byte structural oracle from Part A).

## Standards
- SPDX on any new file; **no new `unsafe`**; `cargo fmt`; `clippy -D warnings`; the kernel **builds and boots green** (the color-constant rename must update every consumer — a missed one fails to compile, which is the point); the byte oracle holds. Commit as **two commits** (A: assets; B: palette) on the one branch. **CI:** the assets commit is `[skip ci]`-clean (no `.rs`); the palette commit touches `fb/mod.rs` (code) — your local build + boot screenshot is the gate; tag the push `[skip ci]` (a color re-value doesn't need the 45-min QEMU family) unless PM says otherwise.

## Scope guard
The logo + palette only. **Do NOT** change layout/typography, add UI beyond the existing boot frame, touch the functional semantics' *meanings* (only their not-changing is the point), alter the `.hack`/AltimitNeo lineage prose, or touch anything under `kernel/src/drivers/usb/` (7.5-B). If sampling reveals the new logo needs a functional color you can't place, BLOCKER to PM rather than inventing palette.

## Return
RETURN with: the new asset set + ladder + boot blob (byte oracle green), the `generate.py` rework, the palette diff across `design-system.md`/`fb/mod.rs`/`ADR-024`/`design.md`/branding-README, the **QEMU boot screenshot**, and the orphan/old-language sweep (literal enumeration). PM reviews the screenshot + merges (the close is a visual + byte-oracle gate, no QEMU family needed).

---

## Relay — what to tell Jarvis
- **Dispatch:** `Plans/DISPATCH-2026-06-12-branding-refresh-v2-jarvis.md`
- **Branch:** `codex/branding-refresh-v2-jarvis` off `origin/main` `77623a0`. Disjoint from 7.5-B.
- **One-line:** New LogansOS identity (blue lens-flare star + silver ring arcs + wordmark) replaces the cyan/magenta circular badge. **Part A (assets):** place both PL sources (black-bg→`LogansOS.png`, transparent→`logansos-transparent.png`); rework `generate.py` — transparent is now a *source* (no circular mask), ladder resizes from the transparent, the 160² boot blob is a *cropped* black-bg canonical (102400-byte oracle, `boot_logo.rs` compile-assert holds); update branding README + orphan sweep. **Part B (palette, brand-forward/function-preserving):** sample exact hexes from the artwork; primary accent Cyan→electric-blue, add Silver, keep Void, **keep Lime/Amber functional semantics**, demote Magenta to alert/fatal-only; re-value+rename `fb/mod.rs` constants (`NEON_CYAN`→`NEON_BLUE`, `DIM_CYAN`→`DIM_BLUE`, magenta stays) and update boot_frame/panic consumers; fix `design-system.md`/`ADR-024`/`design.md` prose. Gate: kernel builds+boots green + **QEMU boot screenshot** (visual oracle) + the byte oracle. Two commits (A/B); `[skip ci]`. No layout/typography/USB changes.
