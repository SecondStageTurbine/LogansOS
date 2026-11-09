<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay #270, Section 270: design read of #51 (time zones and DST)

**Seat:** Dexter · **Reviewed at:** PR #51 head `490b16d8de3c222e5cf2cacd541d1f5844c6924f` (the dispatch's
head) · **Branch:** `dexter/pr51-review` · **Base:** `origin/main` = `792f0bb4558b4b002bc8e0e4ecfbab6b2657c59c`
**PR base:** `9eaa9e6fef2d34ae31e0192a484dcaed4a49f06d` · **PR diff:** one file, +202
(`docs/THINK-2026-11-02-time-zones.md`); no code.
**Dispatch:** `Plans/DISPATCH-2026-11-02-relays-268-270.md` §270.
**RAG:** not available on this box; grep and direct reads.
**Scope:** a design read of the THINK against the PR tree and the OT-85 / settings / WM code at that tree. No build
or QEMU gate — there is no code to build. The code trees are identical between the PR base and `origin/main`
(`git diff --stat 9eaa9e6f 792f0bb4 -- kernel userland tools Cargo.toml Cargo.lock` is empty), so the code line
numbers below apply at both.

## Verdict

| # | Item | Verdict |
|---|---|---|
| 1 | tz data source, licence, freshness, update | **PASS** — all four are recorded; **F1 is a precision/cadence note**, not a blocking finding |
| 2 | 2026-03-08 / 2026-11-01 New York KATs and one southern-hemisphere zone | **FAIL** — the New York values are correct; the southern-hemisphere zone has no values |
| 3 | kernel stays on UTC; conversion in userland; Arthur uses it | **FAIL** — in `RtcLocal=1` `CLOCK_REALTIME` is local wall time, not UTC; §0's “UTC” is false, and the RTC-local display formula is wrong after a transition |
| 4 | set the zone before the Settings app; show the zone | **PASS** — Home verbs and the abbreviation suffix; N1-N4 are build notes |
| **F1** | The TZif footer is the rule after the last transition, not “now” | **note** — the 2026-11-02 present is correct for every zone I checked; the “always exact” claim needs a cadence and a precise guarantee |
| **F2** | `settings-core` has no zone table to range-check against | note |
| **F3** | RTC-local mode: “realtime as read” drifts by an hour after DST | note (part of item 3) |
| **N1-N4** | WM reply path; abbreviation uniqueness; `TimeText` capacity; RTC-local zone semantics | notes (item 4) |

**Verdict: two build blockers (items 2 and 3).** The record work in §1 and §2.4 is good, every New York 2026 value
checks out, and the 2026-11-02 present is correct for all the zones I checked (F1). But the southern-hemisphere
KATs are missing (item 2), and the kernel's `CLOCK_REALTIME` is not UTC in `RtcLocal=1` — the THINK's own contract
says so, while §0 claims otherwise — with the RTC-local display formula also wrong after a transition (item 3).
Those two need a revision before the build. Item 4's mechanism is sound; its four notes should be fixed in the same
pass. F1/F2 are non-blocking notes.

---

## 1. tz data source, licence, freshness and update — PASS for the record

**Source is recorded.** The table is generated from the IANA tz database (`eggert/tz`), with the generator reading
a raw `tzdata*.tar.gz` from IANA via `curl`, never a summarizing fetch
(`docs/THINK-2026-11-02-time-zones.md:92-97`; the survey row is `:146`). This follows the standing rule that exact
bytes come from the raw source.

**Licence is recorded.** The survey says public domain for the data and most code, with a few BSD-3-Clause files
(`:146`), and `:152` says every listed licence is on the CLAUDE.md allow list and nothing is GPL. The repo's
allow list does include BSD (`CLAUDE.md:824`). No GPL, no licence gap.

**Freshness is recorded.** The survey says HEADs were checked on 2026-11-02; the IANA row gives the last commit
2026-11-01 at `c53fe556`, and release 2026e on the build host (`:141-146`). The host's `tzdata.zi` reports version
2026e, which agrees.

**Update path is recorded.** `xtask tzdata` regenerates the table; the output is
`userland/libneo/src/tz_table.rs` with the tzdata version in a header comment and a test that pins it (`:96-98`).
Ids are append-only; a removed zone keeps its id as an alias of its replacement; aliases never get their own id
(`:99-101`). That is a coherent update story.

**Caveats, not fails.** There is no update cadence or owner (who regenerates, and when), and no exact IANA URL or
tarball checksum is pinned. The “how” is recorded; the “when/who” is not. Add both to the build slice — a pinned
version with no refresh trigger is a snapshot, not a maintenance process.

**F1 is the substantive problem, not the record.** The table takes the footer `zic` writes in a TZif v2+ file as
“the zone's current POSIX TZ rule” (`:92-94`, `:96-98`). A TZif footer is the rule for times after the file's last
explicit transition; it is not necessarily the zone's rule now. The failure is below.

## 2. DST KATs: New York correct, southern hemisphere missing — FAIL

### What is right (and worth keeping)

The New York 2026 values are correct. I recomputed them independently with the host's Python `zoneinfo`
(tzdata 2026e); every number matches:

- 2026-01-01 is day 20454 since 1970-01-01 (56 years, 14 leap years) — `:162`.
- Spring: 1772953199 → 2026-03-08 01:59:59 EST; 1772953200 → 2026-03-08 03:00:00 EDT — `:163-166`.
- Fall: 1793512799 → 2026-11-01 01:59:59 EDT; 1793512800 → 2026-11-01 01:00:00 EST — `:167-170`.
- J.c.'s reading: 2026-11-02 06:29 UTC → 01:29 EST — `:171`.
- `local_to_utc`: 2026-11-01 01:30 local = `Ambiguous(1793511000 EDT, 1793514600 EST)`; 2026-03-08 02:30 local
  is a `Gap`; 2026-03-08 01:30 local = `Single(1772951400)` — `:172-175`.

So the skipped and repeated New York hours are pinned, and the UTC instants are right. That is the hard half.

### What is missing

The dispatch asks for New York **and one southern-hemisphere zone**, with hand-computed values including the
repeated and skipped local hours. The THINK only names `Australia/Sydney` under “Other rule shapes”
(`docs/THINK-2026-11-02-time-zones.md:176-177`); there are no Sydney epochs, no transition instants, and no
`Ambiguous`/`Gap` values. The host KATs (`:159-181`) are all New York. This is a **FAIL** against the dispatch's
item 2.

**Concrete input to add** (I computed these with the same independent `zoneinfo` 2026e; they are the Sydney
counterparts of the New York vectors):

- **Fall back, 2026-04-05** (first Sunday of April; 03:00 AEDT → 02:00 AEST, at 2026-04-04 16:00 UTC):
  - 1775318399 → 2026-04-05 02:59:59 AEDT (+11).
  - 1775318400 → 2026-04-05 02:00:00 AEST (+10).
  - The local hour 02:00-02:59 is repeated.
  - `local_to_utc(2026-04-05 02:30, Australia/Sydney)` = `Ambiguous(1775316600, 1775320200)` (02:30 AEDT =
    15:30 UTC; 02:30 AEST = 16:30 UTC).
- **Spring forward, 2026-10-04** (first Sunday of October; 02:00 AEST → 03:00 AEDT, at 2026-10-03 16:00 UTC):
  - 1791043199 → 2026-10-04 01:59:59 AEST (+10).
  - 1791043200 → 2026-10-04 03:00:00 AEDT (+11).
  - The local hour 02:00-02:59 is skipped.
  - `local_to_utc(2026-10-04 02:30, Australia/Sydney)` = `Gap`.

The same gap applies to the other rule shapes named but not pinned at `:176-177`: `Europe/Dublin` (negative DST
in the main data form; the exact footer depends on `zic`'s data form) and the `<+03>` abbreviation have no KAT
values. The dispatch only requires one southern-hemisphere zone, so Sydney is the blocking omission; add Dublin
and `<+03>` in the same table while the KAT section is open.

The QEMU scenario (`:182-183`) is New York/UTC only. It does not exercise the southern-hemisphere rule or
`RtcLocal=1`. Add a host KAT for Sydney (and for the RTC-local path in item 3) rather than another QEMU boot.

## 3. Kernel UTC, userland conversion, Arthur — FAIL on the kernel claim

### What holds

- **Conversion is in userland.** `libneo::tz` is a pure `no_std`, no-allocation module next to `civil`
  (`docs/THINK-2026-11-02-time-zones.md:50-61`); the kernel is not changed (`:42-45`, `:193-194`). Good.
- **Arthur uses the same conversion.** `neo-arthur` has no `CLOCK_REALTIME` reader today — grep finds only
  `CLOCK_MONOTONIC` uses — and the THINK says Arthur will call `libneo::tz` with the zone the WM passes in its
  session setup, never reading the settings store (`:124-128`). That is the right seam. The exact session-setup
  ABI is not specified, but that is a build detail; the conversion call is the same.

### What fails

**`CLOCK_REALTIME` is not UTC.** §0 says “The kernel stays zone-free. `CLOCK_REALTIME` is UTC”
(`docs/THINK-2026-11-02-time-zones.md:13`). §2.1 then says “the contract: `CLOCK_REALTIME` is **the RTC's
reading**” (`:42-45`), and §2.3's table says the local-RTC row displays “realtime as read” (`:81-84`). Those
cannot all be true. The code agrees with §2.1, not §0: `kernel/src/arch/x86_64/rtc_logic.rs:27-32` says “The RTC
is local time … A `CLOCK_REALTIME` value is therefore local wall time counted as if it were UTC seconds since
1970”; `userland/libneo/src/civil.rs:7-10` repeats “No offset”; and the WM's clock module says the same
(`userland/neo-init/src/desktop_session.rs:5958-5960`).

**Concrete input.** On the Zenbook, RTC local, `RtcLocal=1`, zone `America/New_York`, at 2026-11-02 01:29 EST
(the exact preview case), the RTC reads 01:29. The kernel's `unix_seconds` treats those fields as UTC, so
`CLOCK_REALTIME = 1793582940` (01:29 UTC); the true UTC instant is 1793600940 (06:29 UTC). A consumer that follows
§0 (“CLOCK_REALTIME is UTC”) reads 01:29 UTC, five hours behind — a five-hour error of the same size as the preview
one, in the other direction. The THINK's own §2.3 table acknowledges the realtime is local by giving
`local_to_utc(realtime, zone)` as the “UTC instant” (`:84`), but §0 still claims UTC. The dispatch's item 3 asks
for a kernel that stays on UTC; this design leaves it on the RTC's convention and asks userland to remember which
one it is. Those are different contracts.

**The fix is a design decision, not a wording tweak.** Three coherent options:
1. **Keep `CLOCK_REALTIME` UTC.** Drop `RtcLocal`, or convert the RTC local reading to UTC before seeding — the
   latter needs zone data at boot, which contradicts “no time-zone code in the kernel.” The simplest form is:
   the RTC must be read as UTC, and a Windows dual-boot sets `RealTimeIsUniversal`; the preview already is UTC.
2. **Rename the contract.** Say `CLOCK_REALTIME` is the RTC's reading, add a separate `CLOCK_RTC`/“wall” reading
   if a true UTC clock is needed, and audit every consumer. Today the set is small (`action_log.rs:81` uses it for
   log seconds; `desktop_session.rs:6033` for the readout; `userland/neo-init/src/main.rs:2448` for the wall-clock
   probe), and `SystemTime` is deliberately unsupported (`userland/neoplutarch-std-overlay/std/time_neoplutarch.rs`).
   None is a hidden UTC consumer — but the THINK must say none may become one until NTP lands.
3. **Keep the kernel on UTC and do the RTC-mode correction in userland at boot.** Userland computes
   `base = realtime - monotonic` (it already does in `clock::sample`, `desktop_session.rs:6030-6040`), applies
   `local_to_utc(base, zone)` once for `RtcLocal=1`, and uses `U0 + monotonic` thereafter. That preserves a UTC
   `CLOCK_REALTIME` only if the kernel's seed is already UTC; if the base is local, the kernel clock is still
   local. So this option collapses into 1 or 2.

The PM/J.c. should choose 1 or 2 and the THINK should state it. As written, item 3 is **FAIL**. The §6
`RtcLocal` default (0 vs 1) is a separate decision and does not fix the contract: `RtcLocal=1` remains reachable,
and the THINK's own fixed table (`:81-84`) still has a local-realtime row.

### F3 (part of item 3): the RTC-local display formula is wrong after a transition

Even if the PM accepts “`CLOCK_REALTIME` is the RTC's reading”, the table's “local time shown = realtime as read”
(`:84`) is only true at boot. The kernel's realtime is `base + monotonic`, so after a DST transition it does not
jump with the RTC. The display must be `utc_to_local(U0 + elapsed, zone)`, where `U0 = local_to_utc(RTC_at_boot,
zone)` and `elapsed` is the monotonic time since boot — otherwise the clock is an hour off after the transition.
The THINK says “A DST transition is just a minute whose local time jumps, so it needs no special case”
(`:136-137`); that is true for a UTC-derived display, not for “realtime as read.”

**Concrete input.** Boot on the Zenbook at 2026-03-08 01:30 EST (06:30 UTC), `RtcLocal=1`, zone
`America/New_York`. The kernel's `unix_seconds` treats the RTC fields as UTC, so `CLOCK_REALTIME` at boot is
1772933400 (01:30 UTC); the true UTC instant is 1772951400 (06:30 UTC), which is `U0 = local_to_utc(01:30
local, zone)`. Thirty minutes later the actual local time is 03:00 EDT (07:00 UTC). `CLOCK_REALTIME` is
`1772933400 + 1800 = 1772935200`; “realtime as read” is 02:00, and `local_to_utc(1772935200, zone)` is a `Gap`
(02:00 does not exist that day). The correct value is `utc_to_local(U0 + 1800, zone)` =
`utc_to_local(1772953200, zone)` = 03:00 EDT. The same drift happens in reverse at the fall-back. This is a
build-blocking ambiguity in the RTC-local path; the THINK's API can express the fix, but the text must say it.

## 4. Setting and showing the zone before the Settings app — PASS, with notes

**Setting.** §2.5 gives the Home commands: `timezone` (current zone, offset, RTC mode), `timezone
America/New_York`, `timezone utc`, case-insensitive prefix suggestions for an unknown name, and `rtc local` /
`rtc utc` (`docs/THINK-2026-11-02-time-zones.md:113-120`). The WM owns the store and handles these before an
intent goes to Arthur, the same shape as the existing `help`/`install` verbs
(`userland/neo-init/src/desktop_session.rs:3789-3805`). That satisfies “before the Settings app exists.”

**Showing.** §2.7 makes the readout `1:29 AM EST`, with `6:29 AM UTC` for zone 0, so an unset or wrong zone has
a visible suffix (`:130-134`). The intro card and greeting use local time (`:133-134`). That satisfies the
visibility requirement in the common case.

**N1 — “Arthur's reply” contradicts “the WM handles these.”** `:115` says “`timezone` on Home: Arthur's reply
shows the current zone…”, but `:122` says the WM handles these before an intent goes to Arthur. The WM has no
Arthur-style reply path: `help` and `install` show WM cards, not an Arthur message
(`desktop_session.rs:3789-3805`). The build needs a named WM-side response (a small card, or the intro card's
status line) for the query and for the unknown-name suggestions; otherwise the documented reply has no
implementation. This is a design gap, not a setting gap — the value still stores.

**N2 — the abbreviation does not always make a wrong zone visible.** `America/Chicago` and
`America/Indiana/Knox` both print `CST` in winter; `Europe/London` and `Europe/Dublin` both print `GMT` in
winter. A wrong zone with the same abbreviation is invisible. Add the IANA name to the query reply and the
settings card (the readout can keep the abbreviation), or accept that the suffix is a hint rather than a
discriminator. The dispatch says “showing the zone so a wrong one is visible”; the abbreviation is necessary but
not sufficient.

**N3 — `TimeText` is too small for “1:29 AM EST.”** `userland/libneo/src/civil.rs:166` defines
`pub type TimeText = Text<8>;`, and `desktop_session.rs:6006`/`:6014` store the readout's time in that type.
`1:29 AM` is 7 bytes; adding ` EST` is 11. The build must widen `TimeText` (and its “longest 8 bytes” comment at
`civil.rs:165`) or carry the abbreviation separately. §5's slice 2 says “the readout with the abbreviation”
(`:189-190`) but does not name this type change; add it so the slice is buildable as written.

**N4 — `RtcLocal=1` is the mode where the zone label matters least.** The table's local-RTC row says the display
is “realtime as read” (`:84`), so at boot the time is the RTC's wall time and the selected zone only labels it.
If the selected zone is wrong, the time is still the machine's local time and only the label is wrong. With the
correct `U0 + elapsed` formula from F3 the zone does affect later DST transitions, but at the boot instant the
zone cannot correct a local RTC. Say that in §2.3: in RTC-local mode the zone is a label at boot and a DST rule
afterwards, not a correction of the RTC's base. It keeps the user's expectation honest.

## F1 (note): the TZif footer is the rule after the last transition, not “now”

The generator takes the footer line of each TZif (`docs/THINK-2026-11-02-time-zones.md:92-94`, `:96-98`). A TZif
footer is the POSIX rule for times **after the file's last explicit transition**, not a snapshot of the zone's rule
at every time. The THINK calls the footer “the zone's current POSIX TZ rule” (`:92-94`); that is only true when the
present is after the last transition in the release the table was generated from.

**What I checked.** I parsed the footer from every zone file on the host (448 zones) and compared the footer's
offset and abbreviation to `zoneinfo` at 2026-11-02 12:00 UTC: **zero disagreements**. So the preview date and the
current present are fine; this is not a present-day blocker.

**The concrete limitation the THINK already half-states.** `Africa/Casablanca` 2026e has its last explicit
transition on 2026-09-20; the footer is `<+00>0`. On 2026-11-02 the actual rule is +00, so the footer is correct
now. On 2026-06-01 (before the last transition) the actual rule is +01 and the footer gives +00; that is the
THINK's “a date before the zone's last rule change converts with today's rule” limitation (`:105-109`), so it is
disclosed, not a new failure. The same shape appears for `Asia/Gaza`'s footer (a POSIX recurrence approximating a
rule that is not a plain recurrence), though it matches 2026.

**Why it is still a note worth fixing.** The THINK's “The desktop clock always shows the present, so it's exact”
(`:105-107`) is only true if the table is regenerated from a current release and the present is after that
release's last transition. There is no update cadence or owner (item 1 caveat), so a stale table can be wrong at the
present after a transition. For example, the 2026e Casablanca footer is `<+00>0`; a 2027-04-01 present (actual +01
after Ramadan) would be wrong until the table is regenerated from a release containing the 2027 transitions. Add:
(a) a cadence/owner for `xtask tzdata`, (b) a precise guarantee (“exact for the present when generated from the
current release; a date before the release's last transition uses the final rule”), and (c) a KAT for one
non-recurring zone (Casablanca 2026-06-01) that pins the known limitation. If the PM wants exact history, option (b)
(full TZif) is the upgrade path the THINK already names (`:149`), at 100-200 KiB per binary.

The “current POSIX TZ rule” wording at `:92-94` and the “exact for the present” claim at `:105-107` should be
sharpened either way. This is a note, not a blocker for the first build.

## F2 (note): `settings-core` cannot range-check the zone table as described

§2.3 says “`settings-core`'s range check accepts only ids present in the compiled table” (`:72-73`). The table is
generated into `userland/libneo/src/tz_table.rs` (`:96-98`), but `settings-core` has no `libneo` dependency — its
only dependency is `blake3` (`userland/settings-core/Cargo.toml`) — and its `Key::valid` is a fixed match
(`userland/settings-core/src/lib.rs:112-119`). Either generate a `TZ_ZONE_COUNT`/id list into `settings-core` as
well, or move the “is this a known zone id” check to the WM (which links both). The former is simpler; say which
in build slice 2.

The new key ids 4 and 5 are append-only from the current 1-3 (`lib.rs:70-85`), and `MAX_ENTRIES` is 16
(`lib.rs:47-48`), so the record has room. No problem there.

## What I did not check

- No build or QEMU gate: the PR is docs-only, and the THINK is the deliverable.
- I did not verify the crates.io versions, licences or last-commit hashes in the prior-art table (`:144-150`);
  the box has no network. I did verify that the host's tzdata is 2026e, which supports the IANA row's freshness.
- I did not verify the `tz-rs` `no_std` + `alloc` POSIX-parsing claim (`:147`); that is correctly the build's first
  check.
- I did not audit every current `CLOCK_REALTIME` consumer beyond the grep above; the THINK's “none exist yet” at
  `:87-88` is true for UTC consumers, but `action_log.rs:81` is a wall-clock consumer that must be classified in
  the chosen contract.

## Bottom line

Keep the New York KATs and the §2.4 record work. Before the build, fix the two blockers:
1. **Item 2** — add the Sydney 2026-04-05 and 2026-10-04 KATs (values above), and the Dublin/`<+03>` shapes.
2. **Item 3** — choose whether `CLOCK_REALTIME` is UTC or “the RTC's reading”; make §0, §2.1, §2.3 and the
   consumers agree. In RTC-local mode, use `utc_to_local(U0 + elapsed, zone)`, not “realtime as read.”
3. **Item 4 notes** — name the WM reply path (N1), add the IANA name to the query/settings (N2), widen
   `TimeText` (N3), and state the RTC-local zone semantics (N4).
4. **F1/F2 notes** — add the `xtask tzdata` cadence/owner and the Casablanca limitation KAT, and say where the
   zone-id range check lives (`settings-core` or the WM).
