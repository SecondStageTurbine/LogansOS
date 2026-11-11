<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Seat E, audit the MVP app chain and propose the next buildable rung (2026-11-11)

**Seat:** Seat E (cloud), free since #99 landed (`6bdce6c9`). **No branch for code.** The deliverable is one docs PR,
`Plans/AUDIT-2026-11-11-mvp-app-chain.md`.

## Why

The MVP (`Plans/DECISION-2026-10-27-release-tiers-mvp.md`) needs a terminal, a file manager, a text editor and a
settings app. Their designs are approved:
- `docs/THINK-2026-10-29-ot88-app-model.md` §7: A0k, A1a to A1c, A3p, A2, T1, T2, K1b, F0, F1, S1, S2;
- `docs/THINK-2026-10-29-ot67-editor.md` §7: E1, W1, C1, E2, E3;
- `docs/THINK-2026-11-08-file-viewer-prior-art.md` §6: V0 to V4.

The PM's records disagree on what has landed. ADR-010 says "since K1b (2026-10-29)", while the OT-66 THINK and the
ledger call K1b open, at private #247. The next rung has to be picked from facts.

## What to do

For each rung in those three tables, check `origin/main` and record its state: **landed** (cite the commit, and
the code or scenario that proves it), **partial**, or **not started**. Check the code, not the docs: features in
`kernel/Cargo.toml`, syscalls, crates under `userland/`, and scenario names in `tools/xtask`.

Then give the critical path to "Files opens a file in the editor and saves it", and name the **next one or two
buildable rungs**. Those are rungs whose dependencies are all landed and which don't conflict with lanes in flight:
- #98, the terminal;
- #100 / S4 and the setup menu;
- OT-100, the USB persistence work;
- the browser stack, #81 and #85.

For each, say whether it's kernel-touching (the 4-shard gate, a cross-family AAG) or host-only.

## Close

Post the audit as a docs PR, with a PM: comment summarising it. The PM merges it and dispatches the rung you
recommend, probably to you. No code in this item.
