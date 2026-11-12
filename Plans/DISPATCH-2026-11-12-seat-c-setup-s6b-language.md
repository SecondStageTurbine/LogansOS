<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Seat C, setup S6b — the Language page, the catalog wired in (2026-11-12)

**Seat:** Seat C (cloud). **Starts when #100 (S4, the Network page) is on `main`.** Don't open a gate before then.
**Branch:** `claude/setup-s6b-lang`, cut from `origin/main` after #100 lands.
**Design:** `docs/THINK-2026-11-02-setup-menu.md` §3.1 and §5 S6. S6a (#101, `cee75016`) shipped `i18n-core`.
**Scope (J.c.):** English and Spanish only. Don't add a third language.

## Build: Seat C's plan as posted on #101 (2026-11-12 04:56 UTC), approved

- **The strings:** S4's Network-page strings and the Language page's own text go into `i18n-core`, with Spanish for each.
- **`setup-core`:** the `Wizard` carries a `Lang`, and every row comes from the catalog. The Language page is shown
  when `LANGUAGE_COUNT > 1`, and choosing a language redraws the wizard in it.
- **`settings-core`:** `LANGUAGE_COUNT` comes from `i18n-core` (2).
- **`neo-init`:** the choice is stored as `Language`.
- **`neo-atrium`:**
  - the intro, power and clock text and Home's verbs take the `Lang`;
  - the persona KAT's ASCII-only rule becomes "covered by the font";
  - `scene.rs` slices on a char boundary.
- **`libneo::civil`:** a date pattern per language.

Out of scope: Arthur's replies stay in English.

## Kill-switch `i18n-catalog`: the PM's answer to Seat C's question

**ELF byte-identity is not required.** The THINK's "the build byte-identical" is met by behavioural identity: with
`i18n-catalog` off, `Lang::En` is fixed and the page is hidden. Two things must prove the off build equals today's:
- host KATs showing every row is identical;
- the existing setup and intro scenarios' frame digests, unchanged.

Don't keep duplicate literals under `cfg` to make the binaries match. Note this reading in the PR body.

## The drift KAT, after the literals move

1. **Pinned English table:** the English table must equal a pinned snapshot of S6a's (`fixtures/en.txt`), so any
   English wording change is deliberate.
2. **No stray literals:** a source scan of the wizard, intro, power, clock and Home-clock code fails on any
   user-visible literal outside the catalog. Command words and serial log lines are allow-listed.

The THINK's host KATs also stay:
- every id is present in both tables;
- no fallback is taken on the wizard's own strings;
- the longest row fits.

## Gate

- **`setup_menu_lang`:**
  - Choose Spanish. Every wizard page's frame digest, the intro card's, and one Home verb card's must match the
    Spanish oracle renders (`scripts/desktop/setup_oracle.py` with a language argument).
  - The record must hold `Language=1`.
  - Control: the English walk must match today's digests.
- **Gate tier:** the full 4-shard tier and `scripts/worker-close-gate.sh`.
- **PR body:** the kill-switch evidence and the prior-art note (S6a's survey still applies; say so).

## Review

**Contained userland: GPT-6-Luna, high.** Then a Spanish wording read of the new strings by Chad, who read S6a's. The
PM queues both when the PR is green.

## Close

Commit and push at every checkpoint. Post PM: on the PR with the run URL. Then disk reclaim: once the PM has merged
and confirmed, `cargo clean` the worktree and remove it.
