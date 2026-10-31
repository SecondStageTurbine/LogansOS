<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: LogansOS branding assets — finish the visual rebrand → Prometheus

**Date:** 2026-06-06
**From:** PM/Cerebro
**To:** Prometheus (worker tree, feature branch — `prometheus/logansos-branding-assets`)
**Dispatch (authoritative, on main):** `Plans/DISPATCH-2026-06-06-logansos-branding-assets-prometheus.md`
**Status:** DISPATCHED — branding/docs + a tightly-scoped kernel boot-string change. Parallel to BUILD-4a (Jarvis); collision risk low (§7).
**Reconciliation note:** this supersedes an earlier same-filename draft that scoped the work to image swaps only. It is kept superset: the rendered boot **wordmark** `"NEO  PLUTARCH"` is in scope (it is brand text shown at every boot — see ISC-4), and the source crest is delivered **on `origin/main`**, not hand-added in the worker tree (see §0).

---

## 0. READ FIRST — fetch-first gotcha (applies to every PM dispatch)

I land dispatches **and their source assets** on `origin/main` from the cloud (PM) side. Your local worker tree will not see them until you fetch. So **before you look for anything this dispatch references:**

```
git fetch origin
git checkout -b prometheus/logansos-branding-assets origin/main
```

Both this dispatch **and the canonical source image `assets/branding/LogansOS.png`** are committed on `origin/main` — after the fetch + branch-off they are present in your tree. If you still can't see the dispatch, prove the fetch worked by reading it straight from the ref:

```
git show origin/main:Plans/DISPATCH-2026-06-06-logansos-branding-assets-prometheus.md
```

This is a standing gotcha for all my dispatches — fetch before you hunt.

## 1. Context — the rebrand is half-done

The brand rename **NeoPlutarch → LogansOS** landed as a *text-only* pass (`2d792f4 rebrand: NeoPlutarch -> LogansOS (brand-only)`, lineage fixup `20c64dd`; named for the PL's son Logan — Bible footer, 2026-06-05). Docs, the Bible, AGENTS.md, and the RAG all say **LogansOS** now. Internal codenames are deliberately KEPT (Bible Article 25.2): the `neoplutarch-kernel` crate, the `neo-`/`libneo`/`neofs`/`neomc` family, and the `.neom`/NEOM model format do **not** change. This dispatch is about the **human-facing visual identity**, which was never migrated:

- `assets/branding/` still holds the old NeoPlutarch crests (philosopher-bust design) and a stale branding README that calls `neoplutarch-logo.png` "canonical."
- The kernel boot splash still embeds `assets/branding/plutarch-transparent.png` → `kernel/assets/plutarch_boot_logo_160.rgba`, and `boot_frame.rs` renders the wordmark `"NEO  PLUTARCH"`.
- A new canonical crest — `LogansOS.png` (circular neon badge, "L" monogram + LOGANSOS wordmark, same cyan/magenta language and "OS" sub-badge as the old crest) — is committed to `main` as the source of truth for this work.

The Bible's 2026-06-05 footer (`docs/bible.md:352`) records this exact open item: *"Pending design follow-up: regenerate the branding/boot-logo image assets (assets/branding/PlutarchTransparent.png, kernel/assets/plutarch_boot_logo_160.rgba), which still render the old crest."* **This dispatch closes that ledger item.**

## 2. Objective

Every brand surface — README crest, branding asset ladder, and the kernel framebuffer boot splash (logo **and** wordmark) — shows **LogansOS**, derived from `assets/branding/LogansOS.png`, with zero remaining references to Plutarch/NeoPlutarch **image assets**, and the kernel still builds and boots green.

## 3. Design constraint

The crest is canonical (already produced — do **not** redraw it), but anything you generate must stay on the project's design language: `docs/design-system.md` — cyberpunk neon on dark, **Cyan `#00ffff`**, **Magenta `#ff00ff`**, **Void `#0a0e27`** (non-pure-black background). Don't introduce off-palette colors when you remove backgrounds or downscale.

## 4. Implementation Success Criteria

- **ISC-1 — Canonical + transparent crest.** Treat `assets/branding/LogansOS.png` as the canonical full crest (already on main). Produce `assets/branding/logansos-transparent.png` — the same crest with the black background removed (the badge is circular, so the corners become transparent). Verify visually that the neon ring/wordmark are intact and the edges aren't haloed.
- **ISC-2 — Variant ladder.** From the canonical crest, generate the same ladder the old brand had — square, same aspect, deterministic resize (Pillow/ImageMagick in the container): `assets/branding/logansos-logo-512.png`, `logansos-logo-256.png`, `logansos-icon-128.png`. (These replace `neoplutarch-logo-512/256.png` and `neoplutarch-icon-128.png`.)
- **ISC-3 — Boot splash derivative (with a byte oracle).** Regenerate the framebuffer splash from `logansos-transparent.png`: a **160×160 RGBA8888 raw blob, no header**, at `kernel/assets/logansos_boot_logo_160.rgba`. It MUST be exactly `160 * 160 * 4 = 102400` bytes, same dimensions/pixel order as the current `plutarch_boot_logo_160.rgba` (confirm against `kernel/src/init/boot_logo.rs`: `WIDTH=160`, `HEIGHT=160`). **Record the exact generation command/script** in the branding README so it's reproducible — per the existing README's "record the runtime constraints" rule. This blob is an on-disk byte layout, so its byte-length is the independent oracle (ISC-4 asserts it in code), not "it looked right."
- **ISC-4 — Kernel wiring (logo + wordmark).**
  - `kernel/src/init/boot_logo.rs`: point `include_bytes!` at `logansos_boot_logo_160.rgba`; update the doc-comment source-PNG reference (`plutarch-transparent.png` → `logansos-transparent.png`). Add a `const` assertion that `WIDTH * HEIGHT * 4 == RGBA.len()` so a wrong-sized blob fails to **compile** (the byte oracle from ISC-3, made mechanical).
  - `kernel/src/init/boot_frame.rs`: this is the part the earlier draft scoped out — but it renders brand **text**, so it's in scope. Wordmark `"NEO  PLUTARCH"` → `"LOGANS OS"` (keep it readable in the 8px debug font; pick the spacing that centers cleanly and note your choice); doc comment "The NeoPlutarch boot sequence" → LogansOS; "Center the Plutarch crest" comment → LogansOS; diagnostic line `"[ 0.001 ] NEO kernel online"` → LogansOS. **Leave the `.hack`/Altimit lore lines intact** — `"welcome to THE WORLD"` and `"the world is listening."` are deliberate easter eggs, not brand strings. The stale `PHASE 1.5c` version string is **out of scope** here (flag it in your close report; don't fix it in this dispatch).
- **ISC-5 — Doc references.** `README.md`: `assets/branding/PlutarchTransparent.png` → `assets/branding/logansos-transparent.png` (the `alt` already says "LogansOS crest"). Rewrite `assets/branding/README.md` to describe the LogansOS canonical crest, the variant ladder, the transparent variant, the boot derivative, and the RGBA generation method. Check `docs/bible.md:352` and any other md/rs/toml hit for a stale **asset path** (see §6 grep) — fix asset paths; do not touch normative article text.
- **ISC-6 — Retire the Plutarch assets (no orphans).** After everything points at LogansOS, delete the superseded image files: `assets/branding/neoplutarch-logo.png`, `neoplutarch-logo-512.png`, `neoplutarch-logo-256.png`, `neoplutarch-icon-128.png`, `plutarch-transparent.png`, `PlutarchTransparent.png`, and `kernel/assets/plutarch_boot_logo_160.rgba`. The **textual** rename lineage (AltimitNeo → NeoPlutarch → LogansOS) stays in README/Bible prose; only the binary brand assets are removed, and `Plans/archive/**` history is left untouched (it's a record). Prove zero references remain with the §6 grep pasted into your close report (literal enumeration, not "all clear").
- **ISC-7 — Build + boot gate.** `cargo xtask` kernel build green; the const byte-length assertion compiles; the QEMU boot scenario that renders the splash passes (boot_frame renders the new crest + "LOGANS OS" wordmark without panic). `cargo fmt --all --check` and `clippy` clean. SPDX header on any new script you add (CC-BY for asset docs, MPL-2.0 for code/scripts).

## 5. Verification reality

The boot splash is **visual** — don't declare it done from the byte count alone. Boot the kernel in QEMU and confirm the framebuffer shows the LogansOS crest above the "LOGANS OS" wordmark (no hardware needed; the boot smoke / bare-metal-mode build is enough). Capture a screenshot via the existing test harness / QEMU and attach it to the close report. The 102400-byte assertion is the format oracle; the screenshot is the human confirmation. Eyeball each PNG variant too — especially the transparent one (background removal is where these go wrong).

## 6. The reference surface (grep these to zero)

Run this exact grep at close; the result for **asset paths** must be empty:

```
rg -i "plutarch-transparent|plutarch_boot_logo|neoplutarch-logo|neoplutarch-icon|PlutarchTransparent"
```

Known hits at dispatch time:
- `README.md:4` — `PlutarchTransparent.png`
- `assets/branding/README.md:3,7,8,9,10,16` — canonical + ladder + boot derivative
- `kernel/src/init/boot_logo.rs:5,11` — source PNG comment + `include_bytes!`
- `docs/bible.md:352` — the pending-item line names the two old asset paths; updating that footer to "regenerated" is a **PM/PL Bible amendment**, not your edit — flag it for the PM ledger (see §8), don't rewrite the Bible footer yourself.
- Plus the in-code brand strings in `kernel/src/init/boot_frame.rs` (wordmark/comments/diagnostic line per ISC-4).

## 7. Close gate

- `git fetch origin && git rebase origin/main`, then re-run the build/boot gate after the rebase.
- Kernel build + boot scenario green; const byte assertion in place; `fmt`/`clippy` clean.
- The §6 grep for asset paths returns empty — **paste the full grep output** in the close report (enumerate, don't summarize).
- Screenshot of the booted splash showing the LogansOS crest + "LOGANS OS" wordmark, plus the recorded RGBA generation command and the `102400`-byte confirmation.
- Single squashed commit; push to your **feature branch `prometheus/logansos-branding-assets`, not `main`** — PM runs `scripts/pm-merge-check.sh origin/prometheus/logansos-branding-assets` then `--ff-only` merges.

## 8. Close report must list

Files added · files removed (ISC-6) · the wordmark spacing you chose · the boot screenshot · the recorded RGBA generation method + byte count · the stale `PHASE 1.5c` note (ISC-4) · and a **flag to PM** that the Bible footer's "Pending design follow-up: regenerate the branding/boot-logo image assets" item (`docs/bible.md:352`) is now satisfied and wants a PM/PL ledger amendment.

## 9. Scope guard + coordination

Branding image assets, their references, and the tightly-scoped boot-string/wiring changes in `boot_logo.rs` / `boot_frame.rs` **only** — no other kernel behavior, no new kernel-crate dependency, no internal-codename changes (Article 25.2: `neoplutarch-kernel` crate and the `neo-`/`.neom` family stay), no version/phase-indicator changes. **Coordination:** BUILD-4a (Jarvis) is active on Intel GPU bring-up (`kernel/src/drivers/accel/*`, `tools/`); this dispatch touches `assets/`, `kernel/src/init/`, and docs — disjoint from 4a, so no merge collision is expected. If you discover an overlap, flag PM to sequence the merges. The source crest `LogansOS.png` and this dispatch both arrive on `origin/main` (§0) — branch off it, don't reintroduce the source image.
