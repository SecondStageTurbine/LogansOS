<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 340: #71 first review (setup menu S2: the store)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:** `Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md`
§340, running §328 of `Plans/DISPATCH-2026-11-08-relays-325-328.md` as written.

**Reviewed:** PR #71 head `50ca36e20` ("Setup menu S2 (4/n): the store gates"), stacked on #70 `1c036bf25`;
`git diff refs/pr/70...refs/pr/71` is 12 files, +665/−9. `origin/main` at review time is `0ecb5b117`; #70's tip
is its own mainline merge at `1c036bf2`. The four S2 commits are `cc739d4ca` (keys), `4a6ac8b11` (WM store +
overlay), `c7a00a43b` (wizard gating) and `50ca36e20` (gates). The PR body was read from the GitHub API.

**Overall: PASS**, all eight items. No blocking finding. I recomputed all six hand-laid BLAKE3 checksums
independently (Python's `blake3`), ran `cargo test -p settings-core` (12 five-key tests),
`cargo test -p settings-core --features setup-keys` (4 setup-key tests) and the xtask `record7` KAT — all green.
The two QEMU scenarios were not run in this review, so their evidence is the PR run; this review is the static
read plus the host KATs. Four non-blocking notes at the end.

## Checklist, in the dispatch's order

| # | Dispatch item | Verdict | Enforcing refs |
|---|---|---|---|
| 1 | Off ≡ main: with `setup-keys` off, the record is byte-identical to main's five-key record | **PASS** | `userland/settings-core/src/lib.rs:96-103,240-254,350-366`; `userland/settings-core/src/setup_keys_tests.rs:62-72`; ran both host suites |
| 2 | The hand-laid records and xtask's `record7` are independent of the writer; the KAT holds `record7` to `DEFAULTS7_GEN1`'s checksum | **PASS** | `userland/settings-core/src/setup_keys_tests.rs:21-72,81-94`; `tools/xtask/src/login_scenarios.rs:1683-1704,2243-2252`; checksums independently recomputed |
| 3 | A 5-key record reads keys 6/7 as defaults; an out-of-range or mistyped 6/7 is refused, not clamped | **PASS** | `userland/settings-core/src/lib.rs:164-167,281-298,395-411`; `setup_keys_tests.rs:41-60,96-130`; suite green |
| 4 | Volatile overlay: `load()` never reads the installed record and no commit reaches disk | **PASS** | `userland/neo-init/src/desktop_session/settings.rs:258-270,300-308,313` (note on the write-probe read) |
| 5 | The wizard runs only when the store is volatile or no valid record was found; Skip writes nothing | **PASS** | `userland/neo-init/src/desktop_session.rs:1140-1143,5121-5129`; `desktop_session/settings.rs:50-52,273-274`; `desktop_session/setup.rs:66-70,80-104` |
| 6 | `login_setup_store` boot 2 fails on any `setup page=`/write; `setup_menu_store_try` fails on any `SETTINGS written\|loaded\|migrated` | **PASS** | `tools/xtask/src/login_scenarios.rs:1793-1805`; `tools/xtask/src/main.rs:20184-20199`; `login_scenarios.rs:470-485` |
| 7 | CI and `scripts/worker-close-gate.sh` run the `setup-keys` suite and the three new clippy rows | **PASS** | `.github/workflows/ci.yml:938-940,1199-1202`; `scripts/worker-close-gate.sh:88,110-122` (regex checked against the new rows) |
| 8 | Prior art: a new store mode on our own format needs no survey | **PASS / confirmed** | `docs/THINK-2026-11-02-setup-menu.md:35-49,101-104` |

## Detail

### 1. Off ≡ main

With `setup-keys` off, `KEYS` is the same five keys in the same order (`settings-core/src/lib.rs:96-103`),
`Settings` has the same five fields and defaults (`:240-254`), and `encode` is byte-for-byte the main version
(`:350-366`; `git diff origin/main refs/pr/71 -- lib.rs` shows no change inside `encode`). `decode` is the same
`select`/`with` path (`:370-416`). The off build's `tests.rs` still runs (`:498-499`), and its `DEFAULTS_GEN1` is
the unchanged oracle; `cargo test -p settings-core` is 12/12 green. The PR's new `DEFAULTS5_GEN1` fixture
(`setup_keys_tests.rs:62-72`) is byte-identical to main's `DEFAULTS_GEN1` (same bytes, same checksum
`d52a55082e67d1b1c93fb73b207eb9bf`), which I recomputed. Off and main therefore emit identical records.

### 2. Byte oracle

`setup_keys_tests.rs:21-72` lays the six records out by hand (header, entry table, checksum), with the checksum
originating outside the crate (Python `blake3` per the module doc); the test at `:81-94` compares `encode` to them
and `decode` back. xtask's `record7` (`login_scenarios.rs:1683-1704`) builds the same layout itself — magic,
version, count `7`, generation, the five `settings_core::KEYS` entries, then keys 6/7 appended by hand — and never
calls `encode`. The KAT at `:2243-2252` checks `len() == 92`, the 12-byte header, and that bytes 76.. are
`1ca373c0c135b0d880ac427a1540a232`, which is `DEFAULTS7_GEN1`'s checksum (`setup_keys_tests.rs:21-29`).
I recomputed all six checksums from the fixture bytes and each matches its stored checksum; the KAT passes.
The KAT is correct for the build CI uses (xtask's `settings-core` without `setup-keys`); see note 4 for the one
configuration where that assumption is not true.

### 3. Five-key records and refusal, not clamping

`decode` starts from `Settings::default()` (`lib.rs:395`), so a five-key record's missing 6/7 read as default 0;
the test at `setup_keys_tests.rs:96-105` decodes `DEFAULTS5_GEN1` to `Settings::default()` and passes. For a
present but bad key, `Settings::with` runs `Key::from_id` then `key.valid(value)` and returns `Err(SetError::Value)`
before assigning (`lib.rs:281-285`); `decode` maps that to `RecordError::Value` (`:408-410`), and a wrong type is
caught before it at `:405-407`. `valid` uses strict `<` against `LANGUAGE_COUNT`/`LAYOUT_COUNT` (`:164-167`), both
1 (`setup_ids.rs:8-10`), so `LANGUAGE_1`/`LAYOUT_1` (value 1) are refused and `LAYOUT_BOOL` (type 1) is
`RecordError::Type`; `Settings::with(6,1)`/`(7,1)` are `Err(Value)` (`setup_keys_tests.rs:107-130`). The
`setup-keys` suite is 4/4 green.

### 4. Volatile overlay

`load()` (`desktop_session/settings.rs:258`) evaluates `store_is_volatile()` first; on true it sets `VOLATILE`,
stores the defaults in memory and returns (`:259-270`) before the `read_small`/`select` load path at `:271-273`. So
no installed record is selected or put in force. `commit()` returns in its volatile branch (`:300-308`) before
`plan`/`write_whole` (`:310-313`), and `set()` reaches commit only for a changed value (`:425-431`). The only
disk-write calls are `login::write_whole` at `:313` and the probe's `OPEN_CREATE_REGULAR` at `:79`.

One literal-read caveat: `store_is_volatile()` reads both copies (`:74-75`) to choose which file to open for
write (`:76-81`). Those bytes are discarded; they are not used to populate the session. If "never reads" is meant
literally rather than "never inherits", that is the one read. It cannot write in the volatile case: an absent
`.system/` returns true at `:68-73`; a read-only mount fails the write open and returns true at `:82-88`; the
create at `:79` runs only after `.system/` opened and a write-open returned `ENOENT`, and if it succeeds the
function returns `false` (`:82-86`), so the normal (non-volatile) load/write path follows, not the overlay.

### 5. Wizard gating; Skip

`desktop_session.rs:1140-1143` runs `setup::run` only when `setup_menu_due()` is true (with `setup-store` off,
`setup_menu_due` is constant true at `:5131-5134`, preserving the S1 build). `setup_menu_due` (`:5122-5129`) calls
`settings::setup_due()`: `volatile() || FOUND == 0` (`settings.rs:50-52`), and `FOUND` is set only in the branch
where `select` found a valid record (`:273-274`). So an installed system with a valid record logs
`DESKTOP setup skipped reason=stored` and does not run the wizard; a volatile store or no valid record does.
Skip writes nothing: `setup.rs:47-70` calls `store` only under `if finish == Finish::Try` (`:66-69`), and
`Finish::Skipped` goes straight to `end` (`:70`); `store` (`:80-104`) is the only setup write path.

### 6. The two gates

`login_setup_store` boot 2's expected sequence is `login_scenarios.rs:1780-1791`; the absent loop at `:1793-1805`
rejects `DESKTOP setup page=`, `SETTINGS written` and `SETTINGS migrated`, using the plain substring `absent`
helper at `:470-485`. `setup_menu_store_try`'s block is in `require_observed_pass` (`main.rs:20167-20199`), which
the desktop scenario runner calls; it rejects `SETTINGS written`, `SETTINGS loaded` and `SETTINGS migrated`, plus
`DESKTOP setup skipped` (`:20184-20199`). Boot 1 also counts exactly three `SETTINGS written` lines
(`login_scenarios.rs:1772-1779`), matching the one first-record write plus the two changed Comfort keys.

### 7. CI and the close-gate

CI adds the feature-on suite at `ci.yml:938-940` and the three blocking clippy configurations at `:1199-1202`
(`setup-store`, `setup-store-boot-test`, and `setup-store-try-boot-test,setup-store-volatile-test`). The
close-gate itself runs `cargo test -p settings-core --features setup-keys` at `scripts/worker-close-gate.sh:88`,
and its userland-clippy step (`:103-125`) reads every `(cd userland/... && cargo clippy ...)` line from `ci.yml`
and runs it (`run_check` at `:121`). I applied that step's `SHAPE` regex (`:111`) to the current `ci.yml`: all
three new rows match, so the close-gate runs them without another edit.

### 8. Prior art

The THINK already carries the survey (`docs/THINK-2026-11-02-setup-menu.md:35-49`), and §1.2 item 6 (`:101-104`)
already names the conditional-first-boot pattern (initial-setup). This PR adds a mode to our own `NEOSET1`
record — no new external format, protocol or UI archetype — so there is nothing a new survey would cover.
Confirmed: no survey needed.

## Non-blocking notes

1. **Literal "never reads".** The write-probe in `store_is_volatile` (`settings.rs:74-75`) reads both installed
   copies to choose the file to test. It discards them and the overlay still starts from the defaults; the
   THINK's concern (§2.2, `:170-181`) is inheritance, and nothing is inherited. Not a fail.
2. **Boot 2's absent list is success-write only.** It rejects `SETTINGS written` but not `SETTINGS write=failed`
   / `SETTINGS resync` (`login_scenarios.rs:1793-1805`). Boot 2 has no commit path today (its `load()` returns at
   the `select` hit and the wizard is skipped), so this is a gap only if a future regression adds a write attempt
   that fails.
3. **THINK §S2 text is stale on two points.** `docs/THINK-2026-11-02-setup-menu.md:625-640` still says "today's
   three-key record" (the base has five keys, 4/5 from the time-zone work) and its gate says "sets keyboard=1",
   which `LAYOUT_COUNT = 1` makes invalid. The PR writes keys 6/7 as 0 and records that decision in its body
   ("Open questions"). Plan text, not this branch's code.
4. **`record7`'s count is a fixture, not derived.** It hardcodes `7` and appends 6/7
   (`login_scenarios.rs:1683-1704`), which is correct while xtask's `settings-core` build keeps `setup-keys` off
   (the comment at `:1678-1682` says so). A workspace build that unifies `setup-keys` into xtask would make the
   loop emit seven keys and then append 6/7 again; CI and the close-gate do not do that. Optional hardening:
   append only when `settings_core::KEYS.len() == 5`.

## Evidence

- `python3` + `blake3`: recomputed all six `setup_keys_tests.rs` checksums; each equals its stored checksum, and
  `DEFAULTS5_GEN1` equals main's `DEFAULTS_GEN1`.
- `cargo test -p settings-core` — 12 passed, 0 failed.
- `cargo test -p settings-core --features setup-keys` — 4 passed, 0 failed.
- `cargo test -p xtask record7_is_the_seven_key_oracle` — 1 passed, 0 failed.
- Shell check of `worker-close-gate.sh`'s `SHAPE` against `ci.yml`: the three new clippy rows match.
- Not run in this review: `login_setup_store`, `setup_menu_store_try`; their gate evidence is the PR's CI run.
