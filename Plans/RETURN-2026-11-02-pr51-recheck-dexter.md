<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay #276, Section 276: re-check of #51 (time zones, revision 2)

**Seat:** Dexter · **Reviewed:** PR #51 head `0918aa3b45928e2cd3b430f6254a370d2eb1e78f` (branch `claude/tz-think`,
docs only: one file, +168/−58 against `490b16d8`; PR base `9eaa9e6f`) · **Branch:** `dexter/pr51-recheck`
**Base:** `origin/main` = `3e36788a86d1eed7a88ff0f96c0d75b4a6c5f807`
**Dispatch:** `Plans/DISPATCH-2026-11-02-relays-274-276.md` §276. **Earlier report:**
`Plans/RETURN-2026-11-02-pr51-tz-review-dexter.md` (relay 270).
**RAG:** not available on this box; grep and direct reads.
**Scope:** the two FAILs from relay 270, against the THINK at the head. I recomputed every KAT independently with the
host's Python `zoneinfo` (tzdata 2026e) and its TZif footers. No build or QEMU gate: the PR is docs only.
**Base note:** the PR base is 11 commits behind current `origin/main`; the code files cited below are unchanged
between the head and `origin/main` (a `git diff origin/main 0918aa3b --` on those files is empty), so the line
numbers hold at both.

## Verdict

| # | Item | Relay 270 | Now |
|---|---|---|---|
| 2 | Southern-hemisphere KATs (Sydney; ambiguous and skipped hours) | FAIL | **PASS** — §4 now has the Sydney 2026 values; I recomputed all four plus the ambiguous and gap results and they match (`docs/THINK-2026-11-02-time-zones.md:246-260`). |
| 3 | `CLOCK_REALTIME` = UTC; local-RTC correction | FAIL | **PASS** — §0 and §2.1 now state one contract and design the PM's option 3: boot seeds unconfirmed, the WM confirms once with the offset, status call, kill-switch, Codex review (`:14-19`, `:45-88`). |

Both prior FAILs are cleared. The relay-270 notes are also addressed: F2 is now `TZ_ZONE_COUNT` (`:150-152`), and
N1-N3 are the WM card / IANA name / `Text<16>` (`:172-194`). F1 and F3 leave small residual wording notes (6 and 3
below). The new notes are build-dispatch refinements; none blocks the THINK.

## 2. Sydney KATs — PASS

`docs/THINK-2026-11-02-time-zones.md:246-260` has the southern-hemisphere case. I recomputed each value with
`zoneinfo` (tzdata 2026e), independent of the THINK:

| Instant | Sydney local |
|---|---|
| 1775318399 | 2026-04-05 02:59:59 AEDT (+11) |
| 1775318400 | 2026-04-05 02:00:00 AEST (+10) |
| 1791043199 | 2026-10-04 01:59:59 AEST (+10) |
| 1791043200 | 2026-10-04 03:00:00 AEDT (+11) |

- The local 02:30 on 5 April is `Ambiguous(1775316600, 1775320200)`: 1775316600 is 02:30 AEDT and 1775320200 is
  02:30 AEST. Correct.
- The local 02:30 on 4 October is a `Gap`; taking the post-gap offset (+11) gives 1791041400 (2026-10-03 15:30 UTC).
  Correct under §2.1's stated rule.
- The hand arithmetic checks: 2026-01-01 is day 20454; 20547 × 86400 + 57600 = 1775318400 and
  20729 × 86400 + 57600 = 1791043200; 5 April and 4 October are the first Sundays. The footer
  `AEST-10AEDT,M10.1.0,M4.1.0/3` is the host's `Australia/Sydney` footer.
- The New York values (`:232-245`) are unchanged and still match: 1772953200, 1793512800, 1772951400, the two
  ambiguous instants, and J.c.'s 06:29Z → 01:29 EST.

That satisfies the dispatch's item 2.

## 3. `CLOCK_REALTIME` contract — PASS

**Contract.** `:14-19` says the kernel contract is `CLOCK_REALTIME` is UTC; a local RTC seeds an unconfirmed clock;
the WM computes the offset early in the session and hands it to the kernel once. `:45-48` repeats it: every consumer
reads UTC, and an instant the kernel can't vouch for is marked. `:124-129`'s display table now has the confirmed/UTC
rows (local RTC + zone = confirmed; local RTC + no zone = unconfirmed + "zone not set"; UTC RTC = confirmed offset 0;
no zone + confirmed = UTC). That is exactly the PM decision in the dispatch.

**Design.** `:52-88`:
- Boot read unchanged; the clock starts unconfirmed, including OT-85's invalid-read fallback (`:52-54`).
- `SYS_CLOCK_CONFIRM_REALTIME(offset_secs: i64)` subtracts the offset from the base once, marks confirmed, is
  write-once (`EALREADY`), is restricted to the input-system authority (`EPERM`), and refuses beyond ±24 h
  (`EINVAL`) (`:55-64`). The sign matches the tests: New York offset -4 h, reading 00:59 → 04:59Z.
- `SYS_CLOCK_REALTIME_STATUS() -> 0 | 1` (`:65-66`), kill-switch `ENOSYS` + OT-85 bits (`:67-69`), Codex review and
  the atomic/release-acquire publish (`:70-73`).
- The WM's confirm rules: `RtcLocal=0` offset 0; local + zone with `local_to_utc`, earlier instant for ambiguous,
  post-gap offset for gap; local + no zone stays unconfirmed (`:75-86`).
- `:204-207` classifies the action log and its pre/post-confirmation flag. `:265-277` adds host KATs and the QEMU
  scenario: 2026-11-01T00:59 local New York → offset -4 h → 1793509140 (2026-11-01T04:59:00Z), status 1, second call
  `EALREADY`; control stays 0. I verified 1793509140 is 04:59Z. The display scenario 05:59Z → 01:59 EDT and
  06:00Z → 01:00 EST is correct.
- `:283-284` puts the rung in slice 2 with the 4-shard gate and the Codex review. The code seam exists:
  `kernel/src/arch/x86_64/rtc.rs:49-56` is the one base, `kernel/src/arch/x86_64/syscall.rs:7345-7348` reads it, and
  `syscall.rs:9030-9036` is the input-system authority check the THINK cites.

The old contradiction is gone: §0's contract and §2.1's rung now agree, and the rung is the mechanism that makes the
contract true. Item 3 is PASS.

## New notes (non-blocking)

1. **The confirm call needs a guard for a failed RTC seed.** `:52-54` says an invalid read leaves the clock
   unconfirmed, but `:55-57` lets any (authorized) confirm mark it confirmed; OT-85's fallback stores base 0
   (`kernel/src/arch/x86_64/rtc.rs:49-56`). Concrete: boot with `rtc-epoch-force-invalid`, `RtcLocal=0`, the WM
   confirms 0 → status 1 and `CLOCK_REALTIME` = 1970 + uptime. Make the confirm refuse (or leave the status 0) when
   the seed fell back, and add the force-invalid boot to the KAT.
2. **A zone or `rtc` change after confirmation cannot re-anchor.** `:55-63` is write-once; `:176` and `:180` let
   `timezone`/`rtc` change the setting after session start. Concrete: `RtcLocal=1`, the saved zone is New York but
   the RTC is actually Los Angeles at 01:29; the session start confirms -5 h → 06:29Z, and
   `timezone America/Los_Angeles` only relabels it (22:29 the previous day) instead of 01:29. Either add a WM
   re-anchor call (the offset delta) or state that a zone change after confirmation is a label change and the base
   must be right at session start.
3. **The unconfirmed/no-zone path still drifts across a transition.** `:84-86` says `timezone <name>` "confirms at
   once"; `:199-202` says F3 is gone. Concrete: boot in New York at 2026-03-08 01:30 EST, no zone; 30 minutes later
   the unconfirmed clock reads 02:00 while the true local time is 03:00 EDT; `local_to_utc(02:00, NY)` is a `Gap`,
   and the post-gap offset gives 06:00Z (01:00 EST), an hour behind the true 07:00Z. Scope the "F3 gone" claim to
   the confirmed path, or re-read the RTC on a late zone set.
4. **Dublin's footer is not the negative-DST form on this `zic`.** `:136` gives
   `Europe/Dublin → IST-1GMT0,M10.5.0,M3.5.0/1`; the host's `zic` (glibc 2.39) emits
   `GMT0IST,M3.5.0/1,M10.5.0` for the same data. They are equivalent, but `:261`'s "negative DST in the POSIX
   string" KAT is then not what the generator gets. Pin the negative-DST case as a hand-written POSIX string, and
   pin whichever form `zic` emits.
5. **§3 is stale against the latest `PM:` comment.** `:217` and `:224-225` still make "does `tz-rs` parse POSIX
   strings `no_std`?" the build's first check. The PM's 2026-11-02T10:07 comment answers it: `from_posix_tz` is
   `std`-only, the parser is `pub(crate)`, and the plan is host-side footer parsing + `const TimeZoneRef` values
   with `find_local_time_type` at runtime. Fold that into §3/§5 before the build starts.
6. **F1 wording is still a little broad.** `:156` says "exact for any time after its release's last explicit
   transition"; `:162-163` then says a stale table is wrong at the present after Casablanca's next Ramadan change.
   Both cannot be exact for *any* time. Say "exact for the present at generation time; exact for a future present
   only until the next unincorporated transition." The owner is also still "whichever seat the PM names"
   (`:145-146`), not named.
7. **Id 0 is both "not set" and "UTC".** `:109` says `0 = not set`; `:142` says "Id 0 is `UTC`". Say 0 is the UTC id
   and the default, and that the confirmed flag (not the id) selects the unconfirmed "zone not set" display.
   `:22-23`'s "or UTC when no zone is set" should be scoped to a confirmed clock.

## What I did not check

- No build, QEMU, or gate: the PR is docs-only.
- I did not re-verify the crates.io versions/last-commit hashes or the `tz-rs` API beyond the PM's comment; the PM
  read the `tz-rs` source at HEAD.
- The kernel `rtc-offset` implementation does not exist yet, so the review is of the stated design and the code seam,
  not a diff.
- The stale-contract comments in the code (`kernel/src/arch/x86_64/rtc_logic.rs:27-32`, `rtc.rs:8-11`,
  `userland/neo-atrium/src/clock.rs:13-15`, `userland/neoplutarch-std-overlay/std/time_neoplutarch.rs:8-11`) will
  need updating in the build; `SystemTime`'s "a process cannot tell which" rationale is also weaker once
  `SYS_CLOCK_REALTIME_STATUS` exists.

## Bottom line

Both prior FAILs are **PASS** at `0918aa3b`: the Sydney 2026 KATs are present and correct, and the THINK now states
and designs the PM's `CLOCK_REALTIME` = UTC contract with the `rtc-offset` rung. The seven notes above should go into
the build dispatch; none is a blocker to starting it.
