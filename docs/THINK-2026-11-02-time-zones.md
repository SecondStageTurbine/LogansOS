<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: time zones and DST for the desktop clock (2026-11-02)

**Status:** for the PM's read before the build.
**Author:** Seat I.
**Trigger:** J.c. tried the preview (QEMU on Windows) on 2026-11-02. Home showed **6:29 AM** when it was **1:29 AM in New York**
(EST, UTC-5, the day after DST ended on 2026-11-01).
**Builds on:** OT-85 (`kernel/src/arch/x86_64/rtc_logic.rs`, `userland/libneo/src/civil.rs`, `docs/OPEN-THREADS.md` row
OT-85, open item (c)), the settings record (`userland/settings-core`, `NEOSET1`).

## 0. One breath

- The kernel stays zone-free. `CLOCK_REALTIME` is UTC.
- How the RTC is read becomes a setting: **UTC**, the default, or **local** for a Windows dual-boot.
- The user picks an IANA zone (`timezone America/New_York` on Home). It's stored as a stable zone id in `NEOSET1`.
- `libneo` converts UTC to local time from a compiled table of the zone's current POSIX TZ rule, which covers DST.
- Home shows the zone's abbreviation next to the time ("1:29 AM EST"). With the default zone it shows "UTC", so a wrong
  zone is visible.

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

### 2.1 The kernel: no change to the clock code, one change in meaning

The kernel still seeds `CLOCK_REALTIME` once from the RTC (OT-85) and knows nothing about zones. What changes is the
contract: `CLOCK_REALTIME` is **the RTC's reading**, and userland decides what it means using the RTC-mode setting
(§2.3). The kernel needs no setting and no DST rule. This keeps the dispatch's "kernel stays on UTC" at the level that
matters: no time-zone code or data in the kernel.

When a UTC time source lands (NTP, OT-85 (c)), the kernel's realtime becomes true UTC on every machine, and RTC mode
only decides how the RTC seed is read.

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

There are two new `NEOSET1` keys. The record's values are `u32`, so the zone is stored as an id, not as a string:

| Key | Id | Type | Values | Default |
|---|---|---|---|---|
| `TimeZone` | 4 | `Choice` | a stable zone id (§2.4); 0 = UTC | 0 (UTC) |
| `RtcLocal` | 5 | `Bool` | 1 = the RTC holds local time (Windows dual-boot) | 0 (the RTC holds UTC) |

`settings-core`'s range check accepts only ids present in the compiled table, so an unknown id refuses the record, as
every other bad value does. The WM is still the only writer.

**One decision for J.c. (§6):** the `RtcLocal` default. Default 0 matches QEMU, VMs and Linux, and it fixes the
preview as soon as J.c. types the zone. Default 1 keeps OT-85's Zenbook behaviour with no setting, but then the
preview needs `-rtc base=localtime` too. The design works either way.

What each combination shows:

| RTC holds | `RtcLocal` | UTC instant | Local time shown |
|---|---|---|---|
| UTC (QEMU, Linux) | 0 | realtime | `utc_to_local(realtime, zone)` |
| local (Windows) | 1 | `local_to_utc(realtime, zone)` | realtime as read (exactly what Windows shows) |

In the second row, a boot during the repeated autumn hour gives an ambiguous UTC instant. We take the earlier one
(Windows' own behaviour) and the displayed wall time stays right either way. The UTC instant only matters to things
that need UTC, and none exist yet.

### 2.4 The zone table

The table is generated from the IANA tz database and checked in as a Rust table: one row per canonical zone holding the
name and the zone's **current POSIX TZ rule** (the footer `zic` writes at the end of every TZif v2+ file), for example
`America/New_York` → `EST5EDT,M3.2.0,M11.1.0`, `Asia/Kolkata` → `IST-5:30`, `Europe/Dublin` → `IST-1GMT0,M10.5.0,M3.5.0/1`.

- **Generator:** an `xtask tzdata` command reads a raw tzdata release (the `tzdata*.tar.gz` from IANA, fetched with
  `curl`, never through a summarizing fetch). It runs `zic` locally and takes each zone's footer line. Its output is
  `userland/libneo/src/tz_table.rs`, with the tzdata version in a header comment and a test that pins it.
- **Ids are stable and append-only:** a zone keeps its id across tzdata updates; a zone that's been removed keeps its id
  as an alias of its replacement. Id 0 is `UTC`.
- **Aliases** (`US/Eastern`, `Asia/Calcutta`) resolve to their canonical zone by name, but never get their own id.
- **Size:** about 340 canonical zones at roughly 40 bytes each, around 14 KiB, linked only into userland binaries that
  call it (the WM, and Arthur once §2.6 lands).

**The limitation, stated:** a POSIX rule describes the zone's rule *now*, so a date before the zone's last rule change
converts with today's rule. For example, New York before 2007, when DST ran April to October, is off by an hour on
some dates. The desktop clock always shows the present, so it's exact. A past date in an Arthur answer could be off.
Option (b) in §3, which embeds slim TZif data, removes this limitation, and the API in §2.2 doesn't change if we take it
later.

### 2.5 Setting it: a Home command

Until the Settings app exists:

- `timezone` on Home: Arthur's reply shows the current zone, its offset and the RTC mode.
- `timezone America/New_York` stores the zone; the clock updates at once.
- `timezone utc` resets to the default.
- An unknown name gets a reply that names the nearest matches, found by case-insensitive prefix (`timezone new_y` →
  `America/New_York`), and nothing changes.
- `rtc local` and `rtc utc` set `RtcLocal`.

The WM handles these (it owns the store) before an intent goes to Arthur, the same as other Home verbs.

### 2.6 Arthur

Arthur has no time answers today (nothing in `neo-arthur` reads `CLOCK_REALTIME`). When they land, they call `libneo::tz`
with the zone the WM passes in Arthur's session setup, so Arthur never reads the settings store. This THINK only fixes
the API that build will use.

### 2.7 Display

Home's readout becomes `1:29 AM EST`. The abbreviation comes from the rule: a numeric one such as `<+03>` shows as
`+03`. With zone 0 it reads `6:29 AM UTC`; that suffix is what makes a wrong or unset zone visible. The intro card's
date line and the greeting (`clock.rs`, morning/afternoon/evening) use local time.

The minute-boundary refresh (`desktop_session.rs`, `same_minute`) already redraws every minute. A DST transition is
just a minute whose local time jumps, so it needs no special case. The KATs pin the minute before and the minute after.

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

**Recommendation:** use the compiled POSIX-rule table (§2.4) with `tz-rs` as the evaluator if it builds `no_std` here,
and our own evaluator if not. Option (b) stays open behind the same API.

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
- **Other rule shapes:** `Asia/Kolkata` (+05:30, no DST), `Australia/Sydney` (southern hemisphere: DST spans the new
  year), `Europe/Dublin` (negative DST in the POSIX string), and a `<+03>`-style abbreviation.
- **Settings:** `TimeZone` and `RtcLocal` round-trip through `NEOSET1` against a hand-laid record, an unknown zone id is
  refused, and a record without the keys reads the defaults.
- **The table:** the tzdata version is pinned; ids 0 (`UTC`) and the New York id are pinned; ids are unique and never
  reused.
- **QEMU scenario:** the desktop clock probe with `-rtc base=2026-11-01T05:59:00` (UTC mode) and the zone set to New York
  logs `01:59 EDT`, then `01:00 EST` a minute later. A control with the zone left at UTC logs `05:59 UTC`.

## 5. Build slices

1. **`libneo::tz` and the generated table:** the evaluator (or `tz-rs`), the KATs above, and `xtask tzdata`.
   Host-only (no kernel change), so a quick-tier gate.
2. **Settings keys and the Home commands:** `settings-core` keys 4 and 5, the WM's `timezone` and `rtc` verbs, the
   readout with the abbreviation, and the QEMU scenario. Userland only.
3. **(With Arthur's time answers, later):** pass the zone in Arthur's session setup.

Nothing here changes the kernel. That moves OT-85 (c) from "no time zone by design" to "time zones in userland" and
leaves the RTC read itself untouched.

## 6. Decisions for J.c.

1. **The `RtcLocal` default** (§2.3). I recommend **0 (UTC)**: it's right for the preview and every VM, and the
   Zenbook needs one `rtc local` once. Choosing 1 keeps the Zenbook zero-touch, but then the preview script also needs
   `-rtc base=localtime`.
2. **Accept the history limitation (§2.4) for now**, with option (b) as the upgrade path. I recommend yes: the clock
   shows the present.
