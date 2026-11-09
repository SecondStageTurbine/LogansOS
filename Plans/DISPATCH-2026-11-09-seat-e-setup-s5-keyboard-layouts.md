<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Seat E, setup menu S5 — keyboard layouts (2026-11-09)

**Scope change (J.c., 2026-11-09 ~21:00 UTC): two layouts only, US and Spanish (Spain, ISO), not UK.** The details are in
the PM comment on #99: Spanish dead keys for the acute accent and the diaeresis, and the gate types `ñ ¿ ¡ ç á`. Every
mention of UK below now means Spanish.

**Seat:** Seat E (cloud). **Branch:** `claude/setup-s5-keyboard-layouts` off `origin/main`.
**Design:** `docs/THINK-2026-11-02-setup-menu.md`: §3.2 (keyboard layout), §4.2 (keyboard-only, and the key map),
§5 (rungs and gates) and the **S5** section, plus the S2 store for keys 6/7.
**Feedback item:** F11 in `Plans/FEEDBACK-2026-11-02-preview.md`, J.c.'s Ubuntu-style first-boot setup menu.

## Why this, why now

S1 and S2 are on main (#70, #71). Seat I is building S3, the time zone, on #92. The THINK (§5) calls S5 a userland
content lane that depends only on S1 and S2, so it can run beside S3. Expect to rebase over #92's wizard changes
when S3 lands; keep your page code in its own module so that rebase stays small.

## What to build (the S5 section, as written)

1. **`keymap_layout_logic.rs`**, with US and UK tables. US is the table already in `keymap_us_logic.rs` (#53, #54),
   moved behind the layout id with no change in behaviour.
2. **The WM's decoder selects by layout id.** The id is passed to Arthur at spawn, and to any later client, so the
   WM, Arthur and the kernel agree on the layout.
3. **The Keyboard page:** the selector and the test field. Keys 6 and 7 hold the layout half in the S2 store.
4. **Kill-switch `keyboard-layouts`:** with it off, there's one US table, the decoder is identical and the page is
   hidden. Prove the decoder is identical with a host test that compares every key × modifier output against
   today's table, not by inspection.

## Oracles

- **Host KATs:** every key × Shift × Caps × Num for both tables, **written from the keycap legends of a UK ISO
  keyboard**, not derived from the code. The two tables must agree everywhere except where the legends differ.
  Take the legend source from the raw source (a standards document or a vendor layout chart fetched with
  curl or git), never through a summarizing fetch (the byte-oracle rule).
- **Gate `setup_menu_keys`:** enable the page in the test build. Choose UK, type `"` `@` `#` `\` into the test field,
  and the logged characters must be the UK ones. Home then uses UK. A control run leaves the layout on US and gets
  the US characters.
- The rung's scenario asserts both the frames it shows and the settings it leaves (§5).

## Prior art

Before writing the UK table, survey the existing keymap tables: the Linux `kbd` keymaps, xkeyboard-config's `gb`
symbols and the Unicode CLDR keyboard data. Give each its license and last commit date. These are data tables, so
take the **design and the legend facts** and write our table ourselves. Vendor only under an allowed license
(xkeyboard-config is MIT-style, CLDR is Unicode-3.0), and keep the upstream notice if you copy rows. The survey
table goes in the PR.

## Gate

`scripts/worker-close-gate.sh`, then a green 4-shard PR run at the head. Post the URL in a PM: comment. The
PM queues the cross-family review when it's green.

## Close

Commit and push at every checkpoint. After merge, `cargo clean` and remove the worktree.
