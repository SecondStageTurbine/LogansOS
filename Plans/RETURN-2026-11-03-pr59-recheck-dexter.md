<!-- SPDX-License-Identifier: CC-BY-4.0 -->

Reviewed PR #59 at head `3e5b0df08acdce2d30a0c69d4e2bb6507a83af26` (`claude/tz-libneo`). The dispatch named
`415f5e4b`; that is the pre-fix rebase: its four tz files are byte-identical to `000e7c64f`, the rebased
slice-1 commit that is `3e5b0df08`'s parent, so both FAILs below are still present at `415f5e4b`. The branch
was force-pushed to `3e5b0df08` ("tz: defined at the i64 extremes; the generator checks its provenance") while
this relay ran; per "review the head you fetch and name it", this re-check is of `3e5b0df08`. **Merge
`3e5b0df08`, not `415f5e4b`.** Its base `c4170635a` is an ancestor of `origin/main` at `b75c81ae`, and
`origin/main...3e5b0df08` is only the nine tz files listed below. All `file:line` references name the reviewed
head.

RETURN branch: `dexter/pr59-recheck`, from `origin/main` at `b75c81ae85babff749812753c53b2e110175e6ea`.
Review only; no changes to the reviewed code.

- PASS — **1: the New York and Sydney 2026 KATs, including the repeated and skipped local hours, still hold.**
  `tools/tz-logic-tests/src/lib.rs:78-97` pins New York's spring/fall instants and wall times, `:99-126` pins
  `local_to_utc` `Ambiguous`/`Gap`, and `:129-153` pins Sydney's April fall-back and October spring-forward,
  including the ambiguous 02:30 and the skipped 02:30. `cargo test -p tz-logic-tests` passes 9/9 at
  `3e5b0df08`; the new extremes KAT (`:204-241`) leaves these values untouched.

- PASS — **2: the provenance FAIL is fixed; a dirty checkout and a non-release tag are both refused.**
  `tools/xtask/src/tzdata.rs:340-373` (`provenance`) requires every one of the nine `SOURCES` to be tracked
  (`:341-347`, `SOURCES` at `:24-34`), refuses any tracked modification/stage with
  `git status --porcelain --untracked-files=no` (`:348-353`), restricts `git describe` to `20[0-9][0-9][a-z]*`
  (`:353-361`) and re-checks the name with `is_release_tag` (`:325-332`), and records `HEAD` after asserting the
  tag resolves to that same commit (`:365-372`). `cmd_tzdata` takes the pair from `provenance` (`:389`).
  Independent checks at this head:
  - the 2026e clone (`039ef27cc5f062a2055cb67435d6d71adbefd27d`) succeeds: 344 zones, 253 links, and the written
    `userland/libneo/src/tz_table.rs` is byte-identical to the checked-in file (sha256
    `54d8a2a150ddd0767c592c1755b16fe1d04c2f457d4f5559ec4b629254fa2d0a`, `git status` clean);
  - the same clone with `africa` modified at tag `2026e` exits 1: "the tz checkout has changes, so it isn't the
    release", and leaves the table untouched;
  - a scratch checkout whose only tag is `not-a-release` exits 1: "the tz checkout must be at a release tag";
  - `cargo test -p xtask tzdata` passes 4/4, including `release_tags` (`:463-481`) and
    `provenance_refuses_a_dirty_tree_and_other_tags` (`:484-518`).
  `--untracked-files=no` is the right scope: untracked files do not change what `zic` compiles, and an untracked
  *source* is still refused by the `ls-files --error-unmatch` check. Residual (not a FAIL): tag authenticity is
  not checked, so a locally fabricated release-shaped tag on an otherwise clean commit would pass; the PM's ask
  was to record and verify the tag's commit against `HEAD`, which is done.

- PASS — **3: the `i64`-bounds FAIL is fixed; the functions are defined and overflow-free for any `i64`.**
  `userland/libneo/src/tz.rs:175-186` bounds the evaluated instant to `±2^40` s and `offset_at` applies the clamp
  first (`:190`); `utc_to_local` saturates its add (`:232`); `local_to_utc` saturates both offset subtractions
  (`:239-243`), the gap-probe subtraction (`:252`) and both `Gap` results (`:259-260`). Every remaining product
  is bounded by the clamped year (~`1.1e12`), and the old panic sites at `:164`/`:177` (pre-fix numbering) are
  gone. `tools/tz-logic-tests/src/lib.rs:204-241` is a real KAT: under the debug/test profile, where overflow
  checks are on, it feeds `i64::MAX`, `i64::MIN` and their neighbours to `utc_to_local`/`local_to_utc` for UTC,
  New York, Sydney and `u16::MAX`, and asserts the saturated results, including New York's `MIN → MIN` and
  Sydney's `MAX → MAX`. `cargo test -p tz-logic-tests` passes 9/9. In-range results are unchanged (`:240` still
  reads EST at the 2026 instant). Outside `±2^40` the offset is the bound year's and the result saturates, as
  documented at `tz.rs:171-176` — a defined clamp, which the PM's instruction allowed.

- PASS — **3 (the other half): no allocation, and a bad zone id is still safe.** `tz.rs` remains `no_std` with no
  `alloc`/`Vec`/`String`; `zone(id)` is `Option`-returning (`:97-101`) and the callers fall back to `ZONES[0]`
  (`:230`, `:237`), including for `u16::MAX`, which the extremes KAT now exercises at `:212`.

- NOTE — the earlier Dublin note is closed: `Offset::is_dst` now documents that it means the rule's DST part is
  in force, not summer time, with Dublin's negative DST as the example (`userland/libneo/src/tz.rs:63-66`).

- NOTE — two earlier non-blocking gaps remain: `zone_name(id)` (THINK §2.2) is still absent, and the generator
  still writes only `userland/libneo/src/tz_table.rs`; it does not write `TZ_ZONE_COUNT` into
  `userland/settings-core/src/tz_ids.rs` (`tzdata.rs:389` onward). Slice 3 needs that count for the settings
  range check, so it is still an unfinished piece of the generator contract, not a slice-1 regression.

- NOTE — the checked-in table is unchanged by the fix (`git diff 415f5e4b 3e5b0df08 -- tz_table.rs` is empty)
  and still records `TZDATA_VERSION = "2026e"` / `TZDATA_COMMIT = "039ef27c..."` (`tz_table.rs:5-6`), matching
  the clean-checkout regeneration.

- NOTE — gate state: the PM's green run `41458281253` was on the pre-fix `c04ca616`, not this head. At review
  time CI run `41561062947` on `3e5b0df08` was still running (Host/format/lint, build checks, four QEMU shards
  and decoder fuzz in progress; only the skip check had completed). A green gate on `3e5b0df08` is still needed
  before merge.

**Overall: both FAILs from relay 286 are fixed at `3e5b0df08`: item 1 PASS; item 2 PASS (clean-checkout
reproduction and byte-identical table; dirty tree and non-release tag refused); item 3 PASS (bounded, saturating
arithmetic with an extremes KAT; allocation and bad-zone-id behaviour unchanged). The dispatch's `415f5e4b` is
the pre-fix head and must not be the merge SHA.**
