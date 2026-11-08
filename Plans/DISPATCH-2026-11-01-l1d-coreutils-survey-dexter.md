<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 239: Dexter, L1d survey (uutils coreutils against our std target)

**MODEL** DeepSeek, high: a bounded research job with host builds; no kernel change.

Same rules as your earlier relays (public repo: no personal details; RAG not available).
Background: `docs/THINK-2026-10-31-lived-in-os-lanes.md` section 3, L1d (and section 1 on the `std` target:
`x86_64-unknown-neoplutarch`, the std overlay plus `neo-std-pal`).

Write `docs/THINK-2026-11-01-l1d-coreutils-on-logansos.md` that answers, for each of `ls`, `cat`, `cp`, `mv`,
`mkdir`, `rm`, `echo`, `wc`, `head`, `tail`:

1. Re-check uutils/coreutils on the day: head commit, date, licence (record them; the per-utility crates are
   `uu_<name>`).
2. Which `std` and `libc`/`nix`/`rustix` calls it makes (from its source and its dependency tree, `cargo tree`),
   and which of those our `neo-std-pal` implements today, stubs, or lacks. Cite `file:line` in our PAL.
3. Try building it for our target in a scratch branch (don't commit the vendored crates): record the exact errors.
4. A table: utility | blocking gaps | the PAL work each gap needs | an estimate in build cycles (no calendar time).
5. A recommended order and the first utility to land, with what its gate scenario would be.

- BRANCH: `dexter/l1d-coreutils-survey` from `origin/main`. Commit only the THINK and your RETURN.
- Close: push; RETURN `Plans/RETURN-2026-11-01-l1d-coreutils-survey-dexter.md` on your branch, naming the head SHA.
