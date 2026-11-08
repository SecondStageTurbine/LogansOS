<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: time zones and DST for the desktop clock (2026-11-02)

**Status:** revision 2. It answers the design read (Dexter, relay 270, branch `dexter/pr51-review`) and the PM's
decision on it (PR #51). Section 7 maps each finding to its fix.
**Author:** Seat I.
**Trigger:** J.c. tried the preview (QEMU on Windows) on 2026-11-02. Home showed **6:29 AM** when it was **1:29 AM in New York**
(EST, UTC-5, the day after DST ended on 2026-11-01).
**Builds on:** OT-85 (`kernel/src/arch/x86_64/rtc_logic.rs`, `userland/libneo/src/civil.rs`, `docs/OPEN-THREADS.md` row
OT-85, open item (c)), the settings record (`userland/settings-core`, `NEOSET1`).

## 0. One breath

- **The kernel contract is `CLOCK_REALTIME` is UTC, always** (PM decision, PR #51). The kernel holds no zone data and
  does no zone math.
- An RTC that keeps local time (the Zenbook dual-boots Windows) seeds an **unconfirmed** clock.
  - Early in the session, the WM, which has the zone, computes the UTC offset.
  - It hands the offset to the kernel once, through a small privileged call.
  - From then on the clock is UTC. NTP replaces this step when it lands.
- The user picks an IANA zone (`timezone America/New_York` on Home); it's stored as a stable zone id in `NEOSET1`.
- `libneo` converts UTC to local time with each zone's current POSIX TZ rule, which covers DST in both hemispheres.
- Home shows the zone next to the time (`1:29 AM EST`), or `UTC` when no zone is set. While the clock is still
  unconfirmed, it shows the RTC's wall time with a "zone not set" marker rather than a wrong UTC.

## 1. Why the preview showed UTC

OT-85 decided that the RTC holds local wall time, because the Zenbook dual-boots Windows and Windows keeps the RTC in
local time. The kernel stores the RTC reading as the epoch base and every consumer formats it with no offset
(`rtc_logic.rs`, the "RTC is local time" note).

QEMU's RTC defaults to `-rtc base=utc`, and `scripts/preview/preview.py` does not change that. So the preview's RTC
held UTC, the kernel took it as local, and Home showed 06:29. Nothing is wrong with the arithmetic; the RTC's
convention and the kernel's assumption disagree, and the user has no way to say which one their machine uses or which
zone they're in.

Two facts follow:

1. A zone setting alone doesn't fix this. With the RTC read as local time, the zone would only label the time. The
   clock also needs to know whether the RTC holds UTC or local time.
2. The two conventions really do differ by machine. Linux, QEMU and most VMs keep the RTC in UTC. Windows keeps it in
   local time unless `RealTimeIsUniversal` is set.

## 2. Design

### 2.1 The kernel: `CLOCK_REALTIME` is UTC (new rung `rtc-offset`)

The PM chose Dexter's option 3 for the RTC-mode contradiction in revision 1 (§7). **Every consumer can read
`CLOCK_REALTIME` as UTC**, and an instant the kernel can't vouch for is marked, never guessed.

**The rung**, kernel feature `rtc-offset`, is a small addition to OT-85's `rtc-epoch`:

1. **At boot, nothing changes in the read.** The kernel seeds realtime from the RTC exactly as OT-85 does. It doesn't
   know whether the RTC holds UTC or local time (only the user's settings know), so the clock starts **unconfirmed**.
   An invalid RTC read (OT-85's fallback) also leaves it unconfirmed.
2. **One write-once call**, `SYS_CLOCK_CONFIRM_REALTIME(offset_secs: i64)`:
   - The kernel subtracts `offset_secs` from the realtime base once (local wall time = UTC + offset, so UTC = reading
     - offset) and marks the clock **confirmed**.
   - `offset_secs` is 0 when the RTC holds UTC.
   - Limits:
     - Only the session's input-system authority may make the call (the WM, the same authority check the input
       syscalls use); anyone else gets `EPERM`.
     - The offset must be within ±24 h (`EINVAL`).
     - A second call gets `EALREADY`.
   - Monotonic time is untouched, so nothing that measures intervals moves.
3. **One read-only call**, `SYS_CLOCK_REALTIME_STATUS() -> 0 | 1` (unconfirmed / confirmed), lets a consumer tell
   "UTC" from "an RTC reading not yet confirmed".
4. **Kill-switch:** with `rtc-offset` off, the two calls are `ENOSYS` and realtime is OT-85's value, bit for bit. A KAT
   checks that the clock arithmetic matches the off state when the offset is 0, and a QEMU control scenario proves the
   off state.
5. **Review:** this adds a syscall and changes a kernel clock, so it gets the Codex cross-family review.
   - Concurrency: the base and the status flag are published together, either with one atomic store of a packed
     word, or by storing the base before a release store of the flag that every reader loads with acquire.
   - The concurrency-pitfall bank consult for that publish is part of the build's dispatch.

**Who confirms, and with what:** the WM, right after it reads `NEOSET1` at session start (§2.3):

- **`RtcLocal = 0`** (the RTC holds UTC): confirm with offset 0.
- **`RtcLocal = 1` and a zone is set:**
  - Compute `local_to_utc(reading, zone)` (§2.2) and confirm with the difference.
  - **Ambiguous** (a boot during the repeated autumn hour): take the earlier instant. That's what Windows does, and
    the displayed wall time is right either way.
  - **Gap** (a reading inside the skipped spring hour, which a correct RTC can't produce): take the offset in force
    after the gap.
- **`RtcLocal = 1` and no zone:** don't confirm. The clock stays unconfirmed and Home shows the RTC's wall time with
  "zone not set" (§2.7). That's correct for the wall time, honest about UTC, and fixed by `timezone <name>`, which
  confirms at once.

**After the confirm (Janus, #60 review).** Realtime is UTC from then on, and only display code applies the zone:
- a later `timezone <name>` changes what Home shows, never the instant;
- a later `rtc local`/`rtc utc`, or a zone that would have read the RTC differently, is saved, and the card says
  "Restart to read this computer's clock this way". The confirm is write-once, so a restart applies it.
- With a kernel that lacks `rtc-offset` (the status call fails), Home shows realtime exactly as before: no zone, no
  marker.

When NTP lands, it sets realtime from a UTC source and marks it confirmed, and this step becomes a fallback.

### 2.2 The conversion lives in `libneo`

A new `libneo::tz` module, pure (`no_std`, no allocation, no syscall), next to `civil`:

- `utc_to_local(utc_secs, zone) -> (local_secs, Offset)`: an `Offset` is seconds east of UTC plus an abbreviation
  (`EST`, `EDT`, `+03`) and an `is_dst` flag.
- `local_to_utc(local_secs, zone) -> LocalResult`: either `Single`, `Ambiguous(earlier, later)` (the repeated hour in
  the fall) or `Gap(before, after)` (the skipped hour in the spring). RTC-local mode uses this.
- `zone_by_name("America/New_York") -> Option<ZoneId>` and `zone_name(id)`.

Home's clock and the intro card call it, Arthur calls it (§2.6), and the probe calls it. `civil::from_realtime_ns` then
formats the local seconds unchanged.

### 2.3 The settings

There are two new `NEOSET1` keys. Record values are `u32`, so the zone is stored as an id, not as a string:

| Key | Id | Type | Values | Default |
|---|---|---|---|---|
| `TimeZone` | 4 | `Choice` | a stable zone id (§2.4); 0 = not set | 0 |
| `RtcLocal` | 5 | `Bool` | 1 = the RTC holds local time (Windows' convention) | 1 |

`settings-core`'s range check accepts only ids in the compiled table, so an unknown id refuses the record, like any other
bad value. The WM stays the only writer.

**Defaults.** The PM's rationale is "nothing for J.c. to change in Windows", so `RtcLocal` defaults to 1, which is right
for the Zenbook. For the preview to match, `scripts/preview/preview.py` passes `-rtc base=localtime`, so a VM's RTC
holds the host's local time like the real target's. The gate scenarios keep their pinned `-rtc base=<datetime>`. Then
J.c. only ever types the zone.

`rtc utc` is there for machines that keep the RTC in UTC (Linux-only boxes, VMs without `base=localtime`).

What the clock shows:

| RTC holds | `RtcLocal` | Zone | Clock | Home shows |
|---|---|---|---|---|
| local (Zenbook, preview) | 1 | set | confirmed: UTC = reading - offset | `1:29 AM EST` |
| local | 1 | not set | unconfirmed | `1:29 AM` + "zone not set" |
| UTC (Linux, plain VM) | 0 | set | confirmed, offset 0 | `1:29 AM EST` |
| UTC | 0 | not set | confirmed, offset 0 | `6:29 AM UTC` |

### 2.4 The zone table

The table is generated from the IANA tz database and checked in as a Rust table: one row per canonical zone holding the
name and the zone's **final POSIX TZ rule**: the footer `zic` writes at the end of every TZif v2+ file, which governs
every time after the release's last explicit transition. For example
`America/New_York` → `EST5EDT,M3.2.0,M11.1.0`, `Asia/Kolkata` → `IST-5:30`, `Europe/Dublin` → `IST-1GMT0,M10.5.0,M3.5.0/1`.

- **Generator:** `cargo xtask tzdata --src <tz checkout>` reads a tz release as a `git` checkout of the release tag
  (`https://github.com/eggert/tz`, the tz maintainers' repository; `data.iana.org` is refused by the build hosts'
  egress proxy). It's never fetched through a summarizing fetch.
  - It runs the host's `zic`, takes each zone's footer line and parses it **at generation time**.
  - Its output is `userland/libneo/src/tz_table.rs`: the rules as Rust constants, plus the release tag and commit,
    which a test pins.
- **Ids are stable and append-only:** a zone keeps its id across tzdata updates; a zone that's been removed keeps its id
  as an alias of its replacement. Id 0 is `UTC`.
- **Aliases** (`US/Eastern`, `Asia/Calcutta`) resolve to their canonical zone by name, but never get their own id.
- **Refresh: cadence and owner.** Each IANA release (a few a year, announced on the `tz-announce` list) gets one
  regeneration PR. The owner is whichever seat the PM names for the standing item; the build proposes adding it to the
  open-threads board.
  - The generator refuses a checkout that isn't exactly at a release tag (`git describe --tags --exact-match`), and
    records the tag and commit in the table. 2026e is tag `2026e`, commit `039ef27c`, and its signed tag object is
    `db07415c`; anyone can check that signature with `git tag -v 2026e`.
- **Settings validation:** the generator also writes `TZ_ZONE_COUNT` into `userland/settings-core/src/tz_ids.rs`, so
  `settings-core`, which doesn't depend on `libneo`, can range-check the `TimeZone` id itself. Ids are dense and
  append-only, so a count is enough.
- **Size:** about 340 canonical zones at roughly 40 bytes each, around 14 KiB, linked only into userland binaries that
  call it (the WM, and Arthur once §2.6 lands).

**The limitation, stated precisely:** the table is **exact for any time after its release's last explicit transition**
in a zone. That's true of the present when the table is generated from the current release (Dexter checked all 448 host
zones against `zoneinfo` at 2026-11-02 and found no disagreement).
- An earlier date converts with the final rule. New York before 2007, when DST ran April to October, is off by an hour on
  some dates, and so is `Africa/Casablanca` on 2026-06-01: +01 in fact, the footer `<+00>0` says +00. A KAT pins that
  case as the known limitation.
- A table left stale past a zone's next non-recurring change (Casablanca's Ramadan shifts) is wrong for the present
  until it's regenerated. That's why refreshing it has an owner.
- A past date in an Arthur answer can be off.
Option (b) in §3, which embeds slim TZif data, removes this limitation, and the API in §2.2 doesn't change if we take it
later.

### 2.5 Setting it: a Home command

Until the Settings app exists:

- `timezone` on Home: the WM shows a small status card (the same mechanism as the `help` and `install` cards; Arthur
  isn't involved) with the **IANA name**, the abbreviation and UTC offset now, the RTC mode, and whether the clock is
  confirmed. The name is there because an abbreviation alone doesn't identify a zone (`America/Chicago` and
  `America/Indiana/Knox` both show `CST`; London and Dublin both show `GMT` in winter).
- `timezone America/New_York` stores the zone; the clock updates at once.
- `timezone utc` resets to the default.
- An unknown name gets the same card, naming the nearest matches, found by case-insensitive prefix (`timezone new_y` →
  `America/New_York`), and nothing changes.
- `rtc local` and `rtc utc` set `RtcLocal`. Once the clock is confirmed, they take effect at the next restart (§2.1).

The WM handles these (it owns the store) before an intent goes to Arthur, the same as other Home verbs.

### 2.6 Arthur

Arthur has no time answers today (nothing in `neo-arthur` reads `CLOCK_REALTIME`). When they land, they call `libneo::tz`
with the zone the WM passes in Arthur's session setup, so Arthur never reads the settings store. This THINK only fixes
the API that build will use.

### 2.7 Display

Home's readout becomes `1:29 AM EST`. `libneo::civil::TimeText` is `Text<8>` today, which holds `1:29 AM`, so it grows
to `Text<16>` (the time, a space and an abbreviation of up to 6 bytes such as `+0530`, or `zone not set` on a second
line). The abbreviation comes from the rule: a numeric one such as `<+03>` shows as
`+03`. With no zone and a confirmed clock it reads `6:29 AM UTC`. With an unconfirmed clock it reads the RTC's wall time
and a "zone not set" marker. Either way a wrong or unset zone is visible. The intro card's
date line and the greeting (`clock.rs`, morning/afternoon/evening) use local time.

The minute-boundary refresh (`desktop_session.rs`, `same_minute`) already redraws every minute, and once the clock is
confirmed it reads UTC and formats with `utc_to_local`. A DST transition is then just a minute whose local time jumps,
and needs no special case. (Revision 1's RTC-local display, "realtime as read", would have stayed an hour off after a
transition, Dexter's F3; confirmed UTC removes that.) The KATs pin the minute before and the minute after.

**Other `CLOCK_REALTIME` consumers.** The only one today besides the clock is the action log's timestamp
(`neo-init/src/desktop_session/action_log.rs`, `now_s`). Under the new contract, a record written after confirmation
carries UTC. One written before confirmation (the boot screens, an unset zone) carries the RTC's reading, and the record
notes which (one flag from `SYS_CLOCK_REALTIME_STATUS`), so a reader never mistakes one for the other.

## 3. Prior-art survey (CLAUDE.md, unfamiliar features)

Last commits were checked on 2026-11-02 against each repository's `HEAD` (`git clone --depth 1`). Versions and licences
come from the crates.io API.

| Candidate | What it does | Fit for us | Licence | Last commit (HEAD) | Take |
|---|---|---|---|---|---|
| IANA tz database (`eggert/tz`) | The source data and `zic`. | Exactly the data we need; the generator runs `zic` on the host. | Public domain (data and most code; a few files BSD-3-Clause) | 2026-11-01, `c53fe556` | **The data**, through the generator. Release 2026e is on the build host. |
| `tz-rs` 0.7.3 | Parses TZif and POSIX TZ strings and converts UTC to local and back. `no_std` capable. | Small, no dependencies, the right shape. | MIT OR Apache-2.0 | 2026-06-19, `68fb9629` | **Option (a): a dependency for the POSIX-rule evaluator**, if its `no_std` + `alloc` build parses POSIX strings. That's the build's first check. Otherwise its design is read and we write the ~200-line evaluator. |
| `jiff` 0.2.38 | A complete date-time library with bundled tzdb (`jiff-tzdb`). | Correct and maintained, but large. Its `no_std` mode drops the tz database features we'd want. | Unlicense OR MIT | 2026-11-01, `9ea4c2b3` | Design reference only, for its `Ambiguous`/`Gap` API; §2.2 copies the idea, not the code. |
| `tzdb` 0.7.3 | The whole tz database compiled into static TZif blobs for `tz-rs`. | **Option (b)** (full history): about 100-200 KiB in every binary that links it. | Apache-2.0 | 2026-05-18, `f2710d78` | Not now. It's the upgrade path if the §2.4 limitation starts to matter. |
| `chrono-tz` 0.10.4 | tzdb compiled to Rust for `chrono`. | Pulls in `chrono`; heavy. | MIT OR Apache-2.0 | 2025-10-10, `64c3e4da` | Nothing. |

All the licences above are on the CLAUDE.md allow list (MIT, Apache-2.0, Unlicense, public domain). Nothing is GPL.

**Recommendation (revised at slice 1):** use the compiled POSIX-rule table (§2.4) with **our own evaluator**.
- `tz-rs`'s POSIX parser is `std`-only.
- Its `no_std` rule types would work, but once the rules are parsed at generation time, the runtime is ~150 lines of
  date arithmetic. That's less than the adapter code a dependency would need, and it adds no crate to every userland
  binary.
- `tz-rs` and `jiff` stay design references (read, not copied). Option (b) stays open behind the same API.

## 4. Tests (byte-exact, computed by hand)

The DST KATs use New York, 2026. The values below were worked out by hand, then cross-checked with the host's Python
`zoneinfo` (tzdata 2026e), which is independent of the code under test.

- 2026-01-01 is day 20454 since 1970-01-01: 56 years, 14 of them leap years (1972-2024).
- **Spring forward:** the second Sunday of March 2026 is the 8th (March 1 is a Sunday). Clocks jump at 02:00 EST, which
  is 07:00 UTC. That's day 20454 + 66 = 20520, so 20520 × 86400 + 25200 = **1772953200**.
  - 1772953199 → 2026-03-08 01:59:59 EST (UTC-5).
  - 1772953200 → 2026-03-08 03:00:00 EDT (UTC-4).
- **Fall back:** the first Sunday of November 2026 is the 1st. Clocks fall back at 02:00 EDT, which is 06:00 UTC.
  That's day 20454 + 304 = 20758, so 20758 × 86400 + 21600 = **1793512800**.
  - 1793512799 → 2026-11-01 01:59:59 EDT (UTC-4).
  - 1793512800 → 2026-11-01 01:00:00 EST (UTC-5).
- **J.c.'s reading:** 2026-11-02 06:29 UTC → 01:29 EST.
- **`local_to_utc` (RTC-local mode):**
  - 2026-11-01 01:30 local is `Ambiguous(1793511000 EDT, 1793514600 EST)`.
  - 2026-03-08 02:30 local is a `Gap`.
  - 2026-03-08 01:30 local is `Single(1772951400)`.
- **Southern hemisphere, Sydney 2026** (`AEST-10AEDT,M10.1.0,M4.1.0/3`; DST spans the new year, so the year starts
  in DST):
  - 2026-01-01 is a Thursday, so April 1 is a Wednesday and October 1 a Thursday. The first Sundays are 5 April and
    4 October.
  - **Fall back:** 2026-04-05 03:00 AEDT (UTC+11) is 2026-04-04 16:00 UTC. That's day 20454 + 93 = 20547, so
    20547 × 86400 + 57600 = **1775318400**.
    - 1775318399 → 2026-04-05 02:59:59 AEDT.
    - 1775318400 → 2026-04-05 02:00:00 AEST.
    - Local 02:30 on 5 April is `Ambiguous(1775316600 AEDT, 1775320200 AEST)`.
  - **Spring forward:** 2026-10-04 02:00 AEST (UTC+10) is 2026-10-03 16:00 UTC. That's day 20454 + 275 = 20729, so
    20729 × 86400 + 57600 = **1791043200**.
    - 1791043199 → 2026-10-04 01:59:59 AEST.
    - 1791043200 → 2026-10-04 03:00:00 AEDT.
    - Local 02:30 on 4 October is a `Gap`. Taking the offset after the gap (§2.1) gives 1791041400 (15:30 UTC).
  - Cross-checked with Python `zoneinfo` (tzdata 2026e) and the TZif footer in `/usr/share/zoneinfo/Australia/Sydney`.
- **Other rule shapes:** `Asia/Kolkata` (+05:30, no DST), `Europe/Dublin` (negative DST in the POSIX string), and a
  `<+03>`-style abbreviation.
- **The known limitation, pinned:** `Africa/Casablanca` at 2026-06-01 12:00 UTC converts with the footer's +00, not
  the real +01 (§2.4).
- **The kernel rung (`rtc-offset`):**
  - **Host KATs:** the confirm arithmetic (base minus offset, monotonic untouched); write-once; the ±24 h refusal;
    and "off ≡ OT-85" when the offset is 0.
  - **QEMU scenario:** `-rtc base=2026-11-01T00:59:00`, which a local-time RTC in New York shows during EDT. With
    `RtcLocal = 1` and New York set, the WM confirms an offset of -4 h. Realtime then reads 2026-11-01T04:59:00Z
    (1793509140), status 1, and a second confirm gets `EALREADY`.
  - **Its control:** with no zone set, the status stays 0 and Home shows "zone not set".
- **Settings:** `TimeZone` (checked against `TZ_ZONE_COUNT`) and `RtcLocal` round-trip through `NEOSET1` against a hand-laid record, an unknown zone id is
  refused, and a record without the keys reads the defaults.
- **The table:** the tzdata version is pinned; ids 0 (`UTC`) and the New York id are pinned; ids are unique and never
  reused.
- **QEMU scenario (display):** `-rtc base=2026-11-01T05:59:00` with `RtcLocal = 0` and the zone set to New York logs
  `01:59 EDT`, then `01:00 EST` a minute later. A control with no zone logs `05:59 UTC`.

## 5. Build slices

1. **`libneo::tz` and the generated table:** the evaluator (or `tz-rs`), the KATs above and `xtask tzdata`. Host-only, so
   a quick-tier gate.
2. **Kernel rung `rtc-offset`:** the two syscalls, the write-once publish, the kill-switch, host KATs and the QEMU
   scenario plus its control. Kernel-touching, so it needs the 4-shard gate and a Codex cross-family review.
3. **Settings keys and the Home commands:** `settings-core` keys 4 and 5, the WM's `timezone`/`rtc` verbs and the
   confirm call at session start, the readout with the abbreviation or "zone not set", `preview.py`'s
   `-rtc base=localtime`, and the display scenario. Userland, apart from the preview script.
4. **(Later, with Arthur's time answers):** pass the zone in Arthur's session setup.

This turns OT-85 (c) from "no time zone by design" into "UTC in the kernel, zones in userland". The RTC read itself
stays as it is.

## 6. Decisions for J.c.

1. **Accept the history limitation (§2.4) for now**, with option (b) as the upgrade path. I recommend yes: the clock
   shows the present.

The RTC-mode question from revision 1 was decided by the PM (§2.1, option 3). Its default (`RtcLocal = 1`, with the
preview switched to `-rtc base=localtime`) follows the PM's "nothing for J.c. to change in Windows".

## 7. Review findings (relay 270, Dexter) and what changed

| Finding | Verdict | Change |
|---|---|---|
| 1 (tz data source, licence, freshness, update) | PASS | — |
| 2 (New York + southern-hemisphere KATs) | FAIL | §4: Sydney 2026 KATs computed by hand, both transitions, with the ambiguous and the skipped local hour. |
| 3 (kernel on UTC: `CLOCK_REALTIME` wasn't UTC with `RtcLocal = 1`; §0 and §2.1 contradicted each other) | FAIL | PM decision (option 3). §0 and §2.1 now state one contract, "realtime is UTC, or marked unconfirmed". New kernel rung `rtc-offset` (two syscalls, write-once, kill-switch, Codex review); §2.3 defaults and the display table; §4 kernel KATs and scenario; §5 slice 2. |
| 4 (setting and showing the zone) | PASS | Notes fixed: N1, the reply is a WM status card, not Arthur (§2.5); N2, the card shows the IANA name (§2.5); N3, `TimeText` grows to `Text<16>` (§2.7); N4, superseded by confirmed UTC (§2.1). |
| F1 (footer is the final rule, not "now") | note | §2.4: wording made precise, refresh cadence and owner, the release pinned by tag and commit, Casablanca limitation KAT. |
| F2 (`settings-core` can't see the table) | note | §2.4: the generator writes `TZ_ZONE_COUNT` into `settings-core`. |
| F3 (RTC-local display drifts after a transition) | note | Gone with confirmed UTC; §2.7 says so. |
| Unchecked consumer (`action_log.rs`) | — | §2.7: classified; records carry the confirmed flag. |
