<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# SYNTHESIS: the first app-icon set, S1 (Claude) vs Jarvis (Codex) — the PM's recommendation for J.c.

**Inputs.** Two independent passes, from different model families, on the same brief
(`Plans/DISPATCH-2026-10-29-icon-design-pass.md`):
- **A, S1 (Claude):** `docs/design/icons-2026-10-29/s1/`, branch head `96329773`.
- **B, Jarvis (Codex):** `docs/design/icons-2026-10-29/jarvis/`, branch head `deb4a55c`.

The side-by-side sheet is `docs/design/icons-2026-10-29/compare.png`, built from the seats' own SVGs by
`compare.js`. Each icon is shown at 40, 24 and 16 px, on the Void and on the frosted chip.

**J.c. decides.** Nothing ships until he picks.

## Where the two families converged (the strong signal)

**Four of the seven icons got the same object from both seats**, without either seeing the other:
- **Lens:** a magnifier.
- **Files:** a tabbed folder.
- **Install:** an arrow down into a tray.
- **Terminal:** the `>_` prompt.

They also agreed on the colour rule: Silver carries the shape, and Electric Blue is only a small accent or light.
Both seats also gave Browser a round, world-like shape.

Cross-family agreement like this is real evidence that these are the right objects. The open questions are weight and
detail, not the idea.

## Where they diverged (a real choice)

**Arthur, Settings and Browser.**

## Per icon, with the PM's recommendation

| Icon | A, S1 | B, Jarvis | Recommendation |
|---|---|---|---|
| **Arthur** | A bright four-point spark, an open arc and a live dot. It echoes the logo's star and ring without copying them. | A faceted diamond with a cross and small rays. | **A.** It reads instantly as Arthur, ties to the brand, and stays legible at 16 px. B reads more like a compass or a target. |
| **Lens** | A heavy magnifier. | A thinner magnifier with a centre dot. | **A**, for its weight at 16 px. B's centre dot is a nice "focus point" detail if J.c. wants it grafted. |
| **Terminal** | A bare `>_`, no frame. | `>_` in a window frame, drawn all in blue. | **B's framed shape**, redrawn in A's colours (a Silver frame, blue cursor only). The frame says "terminal" at a glance and sets it apart from the arrow shapes. B's all-blue stroke is only 2.8:1 on the chip, below the 3:1 floor. |
| **Files** | A bold folder with a dark fill. | A thin-stroke folder. | **A**, which is clearer at 16 px. |
| **Settings** | Three sliders. | A gear. | **J.c.'s call.** The gear is the universal sign. S1 chose sliders on purpose, under the distinctness rule, so the set doesn't have three round shapes (Lens, Browser, gear). The PM leans **sliders**, but it's a taste call. |
| **Install** | A bold white arrow into a tray. | A thin blue arrow into a tray. | **A.** It's stronger, and doesn't rely on blue. |
| **Browser** | A planet with an orbit. | A globe with meridians. | **B's globe shape**, drawn at A's weight. The globe is the sign everyone knows for "the web"; a planet can read as "space". |

## Set-wide

- **Use A's weight and colour discipline for the whole set:** heavier strokes, and Silver or White for every
  shape-carrying mark. Electric Blue is 2.8:1 on the frosted chip, which fails the 3:1 floor for any shape that
  depends on it. A obeys this throughout; B uses blue for whole outlines (Terminal, Install, Browser, Arthur's
  cross).
- **The five-year-old test:** both sets pass in their blurred rows. S1 also measured it, with centroid, elongation
  and axis per icon. Its closest pair is Arthur and Browser, which the globe recommendation moves further apart.
- **The design-system §8 replacement:** use S1's proposed text, which is more concrete about stroke and size rules.
  Add Jarvis's sentence that Magenta, lime and amber are never identity colours.
- **Licensing:** both sets are original drawings, and both prior-art tables record principle only. Jarvis's SVGs lack
  the SPDX line S1's carry; add it in the build lane.

## If J.c. takes the recommendation, the final set is

Arthur A · Lens A · Terminal B-shape in A colours · Files A · Settings (gear or sliders, J.c.) · Install A ·
Browser B-shape at A weight.

**Next:** a follow-up build lane, to one seat.
1. Redraw the two grafts in A's style.
2. Add SPDX lines.
3. Rasterise the chosen set to RGBA in xtask, the way the busy emblem is.
4. Draw them on the Spine chips behind a feature switch.
5. Replace `docs/design-system.md` §8.

## J.c.'s decision (2026-10-29)

**Arthur A · Lens A · Terminal A (the bare `>_`) · Files A · Settings A (the sliders) · Install B · Browser B (the
globe).**

The two B icons keep Jarvis's shapes, the thin arrow into a tray and the globe with meridians. They're redrawn at the
set's colour rule: Silver for the shape-carrying lines, Electric Blue only as the accent. As drawn, Jarvis's
all-blue outlines are 2.8:1 on the frosted chip, under the 3:1 floor. The PM told J.c. this, and he can ask for the
all-blue version back.

Build lane: `Plans/DISPATCH-2026-10-29-icons-build-s1.md`.
