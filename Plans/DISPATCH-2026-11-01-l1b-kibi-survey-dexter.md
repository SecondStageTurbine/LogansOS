<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 240: Dexter, L1b survey (kibi, the editor, against our std target and the terminal)

**MODEL** DeepSeek, high: a bounded research job with host builds; no kernel change.

Same rules as your earlier relays (public repo: no personal details; RAG not available). Background:
`docs/THINK-2026-10-31-lived-in-os-lanes.md` section 3 (L1b), the terminal design in PR #12
(`git fetch origin +pull/12/head:pr12 && git show pr12:docs/THINK-2026-10-31-l1a-terminal-window.md`), and your own
L1d survey (PR #38) for the std PAL method.

Write `docs/THINK-2026-11-01-l1b-kibi-on-logansos.md`:

1. Re-check ilai-deutel/kibi on the day: head commit, date, licence, its MSRV and edition (the survey said Rust 1.99 /
   edition 2024; compare with our `rust-toolchain.toml`).
2. Its terminal layer: every termios/ioctl/signal call it makes (raw mode, window size, SIGWINCH) and the escape
   sequences it emits and expects as keys. Map each onto the terminal protocol PR #12 proposes; list what #12 lacks.
3. Its file and environment calls against our `neo-std-pal` (implemented, stubbed, missing), with `file:line`.
4. Try building it for our target in a scratch branch (don't commit it): record the exact errors.
5. A recommended port shape (a `neoplutarch` terminal backend in kibi's `unix`/`windows` module pattern) and the
   order of work in build cycles, plus what the L1b gate scenario would type and check.

- BRANCH: `dexter/l1b-kibi-survey` from `origin/main`. Commit only the THINK and your RETURN.
- Close: push; RETURN `Plans/RETURN-2026-11-01-l1b-kibi-survey-dexter.md` on your branch, naming the head SHA.
