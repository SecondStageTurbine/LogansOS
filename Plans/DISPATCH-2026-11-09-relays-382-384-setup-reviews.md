<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relays 382-384: cross-family reviews of the setup-menu rungs S4, S5 and S6a (2026-11-09)

All three are Claude-authored, so each goes to a Codex-family reviewer. Read the code at the stated head, from the
repo, not from a summary. Answer each item pass/fail with the enforcing `file:line`, and a concrete input for any
fail. RETURNs go on the named `codex/*` branch, pushed.

The shared design is `docs/THINK-2026-11-02-setup-menu.md`: §3 and the rung's §5 section.

## Section 382: Janus, #100 (setup S4, the Network page, `SYS_NET_IF_STATUS`). **MODEL** GPT-6.1-Sol, high.
Why: it's a new kernel syscall reading driver-owned state. Head `67be4ba2`, branch `claude/setup-s4-net-info`,
dispatch `Plans/DISPATCH-2026-11-09-seat-i-setup-s4-net-info.md`.
- [ ] The syscall only reads: no kernel state is written, and a short user buffer gives `EINVAL` before any copy.
  Pass/fail plus line.
- [ ] The snapshot can't tear: the fields come from one consistent read under the same lock (or seqlock) the writer
  uses. Name the lock and the writer's line.
- [ ] Lock order and context: the syscall takes no lock the NIC ISR or DHCP path takes in the opposite order, takes no
  AI-governance lock (I-14), and never blocks on the driver. Pass/fail plus line.
- [ ] Byte oracle: a hand-laid `NetIfStatusV1` example exists independent of the writer and reader, and the host KAT
  asserts it field by field. Pass/fail plus line.
- [ ] Kill-switch `net-info`: off means `ENOSYS`, the page shows "Network status is unavailable" and Continue, and the
  rest of the boot is unchanged. Pass/fail plus line.
- [ ] Gates: `setup_menu_net` (link=up, cdc-ecm, 10.0.2.15) and its no-NIC control assert what the dispatch says.
  Pass/fail plus line.
- [ ] Prior art: the PR carries the concurrency pitfall-bank table, and its claims hold. Pass/fail.
RETURN: `Plans/RETURN-2026-11-09-pr100-review-janus.md` on `codex/pr100-review-janus`.

## Section 383: Jarvis, #99 (setup S5, keyboard layouts US + Spanish). **MODEL** GPT-6-Luna, high.
Why: contained userland (one subsystem, no SMP core, no new syscall). Head `8e407c15`, branch
`claude/setup-s5-keyboard-layouts`, dispatch `Plans/DISPATCH-2026-11-09-seat-e-setup-s5-keyboard-layouts.md` (scope
changed to Spanish; see the PM comment on #99).
- [ ] The Spanish (Spain, ISO) table matches a raw source (xkb `symbols/es` or CLDR es) for every key × Shift ×
  AltGr; spot-check at least 15 keys, including ñ, ç, the AltGr row and the two dead-key positions. Pass/fail plus line.
- [ ] Dead keys: acute and diaeresis compose correctly; a dead key followed by a non-composing key emits both; a dead
  key followed by itself or space behaves as the source says. Pass/fail plus line.
- [ ] US by id is byte-identical to today's decoder for every input (the kill-switch). Pass/fail plus line.
- [ ] Home and Arthur get only the ASCII part of the layout: a non-ASCII key is dropped before Arthur's intent path,
  never giving `Err(61)` or ending his session (the ñ leg in `setup_menu_keys`). Pass/fail plus line.
- [ ] The layout id is stored in keys 6/7 and reaches the WM and Arthur at spawn; a restart keeps it. Pass/fail.
- [ ] Prior art: the survey table names its sources with licence and date, and no table rows were copied from an
  incompatible licence. Pass/fail.
RETURN: `Plans/RETURN-2026-11-09-pr99-review-jarvis.md` on `codex/pr99-review-jarvis`.

## Section 384: Chad, #101 (setup S6a, the i18n catalog, host-only). **MODEL** GPT-5.6-Luna, medium.
Why: a host-only crate nothing links yet, plus a wording read. Head `b33788db`, branch
`claude/setup-s6a-i18n-catalog`, dispatch `Plans/DISPATCH-2026-11-09-seat-c-setup-s6a-i18n-catalog.md`.
- [ ] Nothing in image A links `i18n-core`, and the PR shows image A unchanged. Pass/fail.
- [ ] The English table equals today's literals byte for byte (the drift KAT), and every id exists in both tables.
  Pass/fail plus line.
- [ ] The font-coverage KAT reads the shipped font's real coverage, not an assumed range. Pass/fail plus line.
- [ ] Spanish wording (`TRANSLATION-NOTES.md`): natural, neutral Latin American Spanish, the same short imperative
  tone, and no meaning changes. List any line you'd change, with your suggestion. Pass/fail.
- [ ] Prior art: the survey table (fluent, rust-i18n, gettext-rs, ICU4X) has licences and dates, and the choice of
  our own table is justified. Pass/fail.
RETURN: `Plans/RETURN-2026-11-09-pr101-review-chad.md` on `codex/pr101-review-chad`.
