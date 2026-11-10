<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: the first-boot setup menu (a "Try or Install" wizard), Ubuntu-style

**Date:** 2026-11-02 · **Author:** Dexter · **Dispatch:**
`Plans/DISPATCH-2026-11-02-setup-menu-think-dexter.md` (relay 282, F11) · **Status:** revision 2 (relay 285),
design only, no code.

**The ask (the PL, 2026-11-02):** a menu like Ubuntu's, where you choose your Wi-Fi or internet, put in the
time, the keyboard, and the rest, for the "try it before you install it" sessions.

**Code and docs are cited at `origin/main` `2cf6af300`** (2026-11-02), unless a line names another ref. The two
keyboard PRs are now merged there (#53 `b03abfbab`, #54 `cab959819`), after revision 1 was written on their
branches.

**The one-breath answer.**
- **One wizard, drawn by the WM**, as a pre-desktop view in `neo-init` (the login, lock and installer views are the
  precedent). Not a separate app: the WM is the only settings writer, and the WM is the authority the kernel's clock
  confirm call is defined to accept.
- **Screens:** Welcome, Language, Keyboard, Network, Time zone, Comfort, then **Try or Install**.
- **Try:** every choice lives in memory and is lost at shutdown. The wizard writes nothing, because the Try
  volume is read-only (OT-87 §6.3); the store gains a volatile mode.
- **Installed:** one `NEOSET1` record is written once, by the same wizard on the installed system's first boot.
  The installer does not carry the record into the target in v1; that follows OT-66 §7's precedent and is named
  as the convergence rung (§5, S7b).
- **Today:** Comfort works end to end; Time zone works once Seat I's time-zone build lands (PR #51 is design
  only); **Network has no page until S4** — the link and lease exist in the kernel, but no userland call exposes
  them, so the wizard makes no network claim and the Wi-Fi half is a stub; Language and Keyboard are hidden until
  they can do something, with their rungs specified; Try or Install needs OT-87 R5 and, until the medium is
  readable, its Install action is a restart.
- **Seven build rungs + one convergence**, each with a QEMU scenario, a serial-script key walk, a frame oracle
  and a kill-switch. The whole wizard off ⇒ today's boot, bit for bit.

---

## 1. Prior-art survey (CLAUDE.md, "unfamiliar features")

Every row's licence and last commit were read from the repository's `HEAD` through the GitHub API on 2026-11-02.
All five are GPL-family, so they are **read for design and nothing is copied**; the tables in §2 are our own.

| Project | What it is | Licence | Last commit (HEAD) | What we take |
|---|---|---|---|---|
| **ubuntu-desktop-provision** (`canonical/ubuntu-desktop-provision`) | Ubuntu Desktop's installer front end: a Flutter wizard (`apps/ubuntu_bootstrap`, `packages/ubuntu_provision`) over Subiquity | GPL-3.0 | `a64330a621ec` 2026-10-26 | **The screen order, the in-live-session choice, and the "no network" option.** Also the anti-pattern: its Try path never reaches identity/time zone, so a Try session gets none of them. |
| **Subiquity** (`canonical/subiquity`) | Ubuntu Server's installer; the wizard collects an autoinstall config, curtin executes the disk work | AGPL-3.0 (`subiquity/`), GPL-3.0 (rest) | `c948f9bc97a3` 2026-10-29 | **The UI/executor split.** Our wizard decides; the kernel's installer executes; neither trusts the other's verdict. |
| **GNOME Initial Setup** (`GNOME/gnome-initial-setup`) | GNOME's first-boot tool on an already-installed system | GPL-2.0-or-later | `7ee9a4dd6769` 2026-10-19 | **Hiding inapplicable pages via a skip list; a skippable network page; the summary page.** |
| **elementary Installer** (`elementary/installer`) | elementary OS's GTK installer, run from the live session | GPL-3.0-or-later | `17ec614c481f` 2026-10-19 | **One "Try or Install" screen with an explicit, no-default choice**; Try launches a demo session and logs out. Also: it has **no network page at all** — the live session owns networking. |
| **initial-setup** (`rhinstaller/initial-setup`) | Fedora's post-install first-boot tool: a hub of spokes (EULA, plus distro spokes), run only when something needs input | GPL-2.0-or-later | `350247997b20` 2026-10-13 | **Conditional first boot**: the wizard runs because a setting is missing, not on a timer, and exits silently when nothing is needed. |

### 1.1 What each does, screen by screen

**Ubuntu Desktop (24.04-era).** The wizard's step order is an enum in
`apps/ubuntu_bootstrap/lib/app/installation_step.dart`: `loading → locale → accessibility → rst → keyboard →
network → refresh → tryOrInstall → autoinstall → … → sourceSelection → codecsAndDrivers → secureBoot → storage →
identity → activeDirectory → timezone → confirm → install → recoveryKey → done`. The order is the design:
language, then keyboard, then network, are universal and early. **`tryOrInstall` sits after network and before
storage**, so the Try/Install decision is made *inside* the live session, not at a boot menu. Choosing **Try**
closes the installer window (`YaruWindow.of(context).close`) and leaves the live desktop running; there is no
reboot. Because `identity` and `timezone` come *after* `tryOrInstall`, a Try session never sets either.

The network page offers Ethernet, Wi-Fi, hidden Wi-Fi, and a **"no network"** radio (`NoConnectView`,
`networkNoneOption`); Continue is allowed without a connection, because the payload is on the medium. The
`refresh` page is hideable (`allowedToHide`), so it appears only when it has something to offer. Keyboard has a
"detect layout" affordance that asks the user to press keys. Every page is a Flutter widget and the language page retranslates the wizard immediately.

**Subiquity** is the executor behind that UI: the wizard produces a config and curtin applies it in a fixed stage
order. The lesson is the seam, not the screens.

**GNOME Initial Setup.** Its page table is in `gnome-initial-setup/gnome-initial-setup.c`:
`welcome → language → keyboard → network → privacy → timezone (new user only) → software → account → password →
parental controls → summary`. Pages are skipped from a vendor config (`[pages] skip=…`), and a page can be
`new_user_only` or `existing_user_only`. The network page is **skippable** (`gis_page_set_skippable(page, TRUE)`),
shows "Wireless networking is disabled" when the radio is off, and **hides itself entirely when no device
exists** (`"No network device found, hiding network page"`). A summary page ends the run. There is no Try: it is
a post-install tool, and it is not part of the installer.

**elementary Installer.** `src/MainWindow.vala` pushes `LanguageView → KeyboardLayoutView → TryInstallView →
(CheckView) → DiskView → EncryptView → DriversView → ProgressView → SuccessView`. `TryInstallView` is one screen
with three options — "Try Demo Mode", "Erase Disk and Install", "Custom Install (Advanced)" — and a hidden
pre-selected radio, so the user must make a **conscious** choice before Next enables. "Try Demo Mode" calls
`Utils.demo_mode()`, which triggers a lightdm demo session and logs out; it does not merely close a window. No
network page: the live session's network panel is the only network UI, and the installer does not gate on it.

**initial-setup.** A hub of spokes (`initial_setup/gui/hubs/…`, `initial_setup/tui/…`) with only EULA in this
repo; a distro adds time zone and user spokes. It runs from a systemd unit after install and finishes without a
screen when every spoke already has a value. The pattern is the predicate: **show the wizard only when something
is actually unset.**

### 1.2 Our reading of the survey

1. **The order language → keyboard → network is universal**, and it is right: each screen makes the next usable.
   We keep it.
2. **Time zone belongs before the Try/Install choice, not after it.** Both Ubuntu and GNOME ask it late (it is an
   install-time or first-boot setting). The PL's ask is precisely that a **Try** session gets the right clock, so
   we move it earlier and diverge from Ubuntu deliberately. This is the one place where following the ask beats
   following the prior art.
3. **The Try/Install choice belongs at the end of the wizard, in the live session** (Ubuntu, elementary), not
   only in the bootloader. Our bootloader still offers it (§2.3) because of a limitation, not by preference.
4. **The network screen must never block.** Ubuntu, GNOME and elementary all let the user continue offline. The
   payload is local; nothing in our install needs a network.
5. **Hide a page that cannot do anything** (GNOME's skip list). A language picker that cannot change a word is
   worse than no picker. This is why two screens are hidden in v1 (§3.1, §3.2).
6. **A first-boot wizard is conditional** (initial-setup). Ours runs because a setting is missing (installed) or
   because a Try session has nowhere to persist (every Try boot), never on a timer.

---

## 2. Our flow

### 2.1 The screens, in order

Every screen is a WM card (§4.5). The row budget is eight (`CARD_MAX_ROWS`,
`userland/neo-atrium/src/layout.rs:95`), so each screen is a title, a short body, its options and a hint. The
Home field is reused as the time-zone search box. Up/Down/Tab move, Enter/Space select, Esc goes back.

| # | Screen | What it asks | What it sets | Shown in v1? |
|---|---|---|---|---|
| 0 | **Welcome** | "Set up LogansOS. A few short screens; you can change any of these later." **Begin** / **Skip setup** | nothing | yes |
| 1 | **Language** | which language the desktop uses | `Language` (catalog id) | **hidden until a second language exists** (§3.1) |
| 2 | **Keyboard** | which layout the keys type, with a test line | `KeyboardLayout` (table id) | **hidden until a second layout exists** (§3.2) |
| 3 | **Network** | the wired link's status; Wi-Fi is coming; **Continue without a network** | nothing persisted; the lease is kernel state | **hidden until S4** (no userland status call on `main`) |
| 4 | **Time zone** | city or IANA name, with a live clock preview | `TimeZone` (zone id), and one clock confirm | yes, once Seat I's build lands |
| 5 | **Comfort** | Reduce motion, Reduce transparency | `ReduceMotion`, `ReduceTransparency` | **yes — works today** |
| 6 | **Try or Install** | a summary of the choices, then **Try LogansOS** / **Install LogansOS** | nothing; it is an action | yes, once OT-87 R5 lands |
| — | *(installed first boot)* | the same 0–5, then **Finish**; no screen 6 | as above; one `NEOSET1` record | yes |

**Why a Welcome screen first.** The wizard can appear on a USB boot with no context. Ubuntu starts at language
because its boot menu already said "Try or Install Ubuntu"; we start with one sentence that says what this is and
offers Skip, then the PL's screens.

**Why no account screen.** OT-66 §7 decides account creation is a **first-boot screen on the installed system**,
not an installer or live-session step, because the installer's write path is the most reviewed code in OT-87 and
the live medium is read-only. We keep that. On the installed first boot the order is wizard → account creation →
the session; later boots go login → the session; on Try there is no account and no login.

**Where the hook goes.** The wizard runs between `clock::start` (`userland/neo-init/src/desktop_session.rs:1074`)
and `login::gate` (`:1077`): the RTC has been read, so the Time zone preview has an instant, and the account
screen follows the wizard rather than preceding it. `settings::load` (`:1047`) has already chosen the store mode
from the Try flag.

**Why no disk screen.** The disk choice already lives in the installer view (`userland/neo-init/src/installer_view.rs`,
OT-87 R4.4), with the kernel deciding every verdict. The wizard hands off to it; it does not duplicate it.

**Why no third-party-software screen.** There is no package manager, no proprietary-driver channel and no codec
offer; the closest thing (Wi-Fi firmware) is a boot module. Out of scope, stated rather than faked.

### 2.2 What each screen sets, and where it lives

The installed store is `NEOSET1` in `/neofs/.system/settings.a` and `.b` (two copies, the valid one with the
higher generation wins; a write goes to the file not holding the record). `settings-core` is the pure codec and
the range check (`userland/settings-core/src/lib.rs`); the WM is the only writer
(`userland/neo-init/src/desktop_session/settings.rs`). Keys are `u16`, values `u32`; `MAX_ENTRIES` is 16 and
there are 3 keys today (`:48`, `:72-88`).

| Screen | Key | Id | Type | Try session | Installed system |
|---|---|---|---|---|---|
| Language | `Language` | **6** (new) | `Choice` | RAM only | `NEOSET1` `.a`/`.b` |
| Keyboard | `KeyboardLayout` | **7** (new) | `Choice` | RAM only | `NEOSET1` |
| Network | *(none)* | — | — | the DHCP lease is kernel state; a Wi-Fi join (later) is session-only | a Wi-Fi PSK is a **separate credential record**, never `NEOSET1` |
| Time zone | `TimeZone` | **4** | `Choice` | RAM only | `NEOSET1` |
| *(not asked; default)* | `RtcLocal` | **5** | `Bool` | RAM only | `NEOSET1` |
| Comfort | `ReduceMotion` | 2 | `Choice` | RAM only | `NEOSET1` |
| Comfort | `ReduceTransparency` | 1 | `Bool` | RAM only | `NEOSET1` |
| Try or Install | *(none)* | — | — | an action, logged | an action, logged |

Keys 4 and 5 are the time-zone THINK's (`docs/THINK-2026-11-02-time-zones.md` §2.3); 6 and 7 are new here.
`settings-core` cannot depend on `libneo` or the kernel, so the counts come from generated files, the pattern the
time-zone THINK already sets for `TZ_ZONE_COUNT`: `LANGUAGE_COUNT` and `LAYOUT_COUNT` are written by their
generators and the range check accepts only ids below the count. Seven keys of sixteen is within the record's
budget; the record grows by 8 bytes per key, so this is 56 bytes of entries plus the 36-byte header and checksum.

**The Try store is a volatile overlay, not a write to a read-only mount.** Today `settings::load()` reads the two
files and, with neither, migrates the presence files and writes a first record. In a Try session `/neofs` is either
absent (no installed volume) or mounted read-only (an installed disk attached, OT-87 §6.3); either way the write
fails. The wizard needs a second mode:

- `StoreMode::Volatile`: start from `Settings::default()`, never read the installed disk's record (a Try session
  is anonymous — it does not inherit the installed user's language, motion or zone), apply writes in memory, log
  `writes=0`, and return them from `current()` for the rest of the session.
- The mode is selected by the Try flag (OT-87 R5) before `settings::load()` at
  `userland/neo-init/src/desktop_session.rs:1047`, so the rest of the session (glass material, motion) sees the
  wizard's choices exactly as an installed session would.

**The time-zone write is one kernel call, when the Time zone page is left — not at the end of the wizard.** The
clock contract is `CLOCK_REALTIME` is UTC, and `SYS_CLOCK_CONFIRM_REALTIME(offset_secs)` is **write-once** (a
second call returns `EALREADY`) (time-zone THINK §2.1). Revision 1 deferred that call to the end of the wizard, so
every action-log record written before it carried the RTC's local wall time as if it were UTC
(`userland/neo-init/src/desktop_session/action_log.rs:75-82`), and an appended, hash-chained record cannot be
corrected afterwards. The confirm therefore moves to the moment the zone becomes a real choice:

- the Time zone page computes a **preview** in userspace (`libneo::tz`): the RTC's wall reading with the chosen
  zone's abbreviation and offset, from `local_to_utc`'s inverse direction — no syscall;
- **leaving the Time zone page confirms the clock once**, with the chosen zone's offset. From that instant
  `CLOCK_REALTIME` is UTC, so every record appended after it is right. If `SYS_CLOCK_REALTIME_STATUS` already says
  confirmed (a later refactor, or a session that knew its zone at start), the page is review-only and makes no
  second call;
- **the zone is a one-way commit for the session.** Back from Comfort or Try/Install reaches the page in review
  mode — it shows the confirmed zone and offers no re-selection, because write-once forbids a second confirm and
  the wizard will not store a zone the clock is not using. A changed mind is Home's `timezone <name>` after setup
  (the time-zone lane's verb; whether that re-confirms is §7 q. 8), or a reboot;
- **the confirm is ordered store-first, and after a failure the durable record is the truth** (PM decision on
  #92, 2026-11-09, after Chad's relays 369, 373 and 377). The kernel's confirm is write-once and can't be undone,
  so leaving the page:
  1. writes `TimeZone` and `RtcLocal` together, in one `NEOSET1` record, and reads it back (the write goes to the
     file not holding the record, so a write that fails before landing leaves the old record in force); if that
     fails, the kernel isn't asked;
  2. asks the kernel to confirm; a refusal writes the previous record back;
  3. on success, makes the zone and mode the clock's and shows them.

  On **any** failure in those steps (including a write that lands but reports failure, and a rollback that itself
  fails), the WM re-reads the store, puts exactly the record it selects (the valid one with the higher generation)
  in force for the zone, the RTC mode and the clock, and resolves the clock from it as a boot would (confirming if
  that record places the RTC reading). The next page says "Couldn't save the time zone. In use: <zone>." The live
  state therefore always equals what the next boot builds; it is never "unchanged" by assumption.

  If the re-read itself fails (a copy can't be read), what the next boot selects is unknown, so the confirm is
  **unresolved** (Chad's relay 379): nothing is put in force, the clock stays exactly as it was, the wizard stays
  on the Time zone page, and its notice names no stored zone: "Couldn't save the time zone. Restart to check it."
  If both copies are read and neither is a valid record, the re-read takes the boot path's rule (migrate the
  presence files and write that as the first record), so the confirm and the next boot agree.

  Reconciliation covers the confirm's own failures. A display failure (`Scene::present` returning an error) is
  not one of them: it ends the session, and the next boot reads the store, which is already the truth by then;
- **no action-log record is written before the confirm.** The wizard holds its page choices in a small in-memory
  queue and appends them, in order, once the confirm succeeds; each record's `time_s` is the event's monotonic
  instant mapped onto the confirmed clock (`confirmed_utc − (m_confirm − m_event)`), not the RTC reading. Nothing
  is rewritten and the chain is untouched (§4.4);
- if the user skips the zone page or the wizard, no confirm happens and the clock stays unconfirmed (Home shows
  the RTC wall time with "zone not set", time-zone THINK §2.7). The queued records are then appended with the
  time-zone lane's "unconfirmed" flag and the RTC reading, so they are marked rather than silently wrong. On a
  Try boot the append itself fails (the mount is read-only, OT-87 §6.3) and the queue is dropped: Try writes
  nothing.

**`RtcLocal` is not a wizard question.** The default is 1 (the RTC holds local time, Windows' convention, right
for the dual-boot target); `rtc utc` fixes a UTC machine (time-zone THINK §2.3). Asking a first-boot user "does
your firmware clock hold UTC or local time?" is a question they cannot answer; the wizard leaves it at the default
and the open question is recorded in §7.

### 2.3 Try vs Install, precisely

**Try.**
- The wizard runs on **every** Try boot, because nothing it sets persists. The visible screens (Welcome, Time
  zone, Comfort, Try/Install) are cheap;
  the alternative (a write to the boot medium) is not possible (read-only) and would be a surprise on a shared USB.
- Settings are the volatile overlay (§2.2). The clock confirm is real for the session but not persisted.
- It writes **nothing** to any disk. `/neofs` is absent or mounted read-only (OT-87 §6.3); the wizard does not
  take exception. The gate's oracle includes the whole-disk hash before/after (S2/S7).
- No account, no login, and the intro card follows the wizard with the chosen zone's time in its greeting.

**Install.**
- The wizard runs once per boot, before the installer view. Its choices are in memory for the installer's own UI
  and for the action log's timestamps; they do **not** reach the target as bytes in v1 (§2.4).
- Picking **Install** opens `installer_view::run` (the same view `install` at Home opens). The kernel's own
  `SYS_INSTALL_LIST`/`SYS_INSTALL_RUN` verdicts decide everything; the wizard has no install authority of its own.
- Picking **Try** on an Install boot leaves the wizard and opens the desktop. Where the install bundle was loaded
  by that boot but the user chose Try, nothing is written.

**The boot-menu reality (v1).** OT-87 R5 makes Try/Install two Limine entries, Try the default, with a short
timeout (`boot/limine.conf:8-9` is `timeout: 0`, one entry, today; `docs/THINK-2026-10-27-ot87-installer.md` §6.1).
The Install entry is the only one that loads the install bundle as a Limine module (§4.3), and the bundle is large
(the ESP templates alone), so a Try boot cannot switch to Install at runtime. Therefore, in v1:

- the wizard's **Install** action on a Try boot explains that the installer needs its own boot and offers
  **Restart**, which returns to Limine where the user picks Install LogansOS;
- the wizard's **Try** action on an Install boot explains that the installation was not started and offers
  **Restart**.

That is the one place the design bends. The rung that removes it is the one that makes the medium readable
(L3u USB mass storage plus a FAT/ISO9660 reader), after which one boot can load nothing and the wizard's last
screen is the real choice, Ubuntu-style. The restart is a confirmation, never a silent reboot, and the action log
records both the choice and the restart.

### 2.4 What is carried into Install

**In memory, yes.** The wizard and the installer view are the same process (`neo-init`), so the final `Settings`
and the chosen zone are live when the installer runs. The installer uses them for its own language and for the
action log's timestamps (which are UTC + zone, time-zone THINK §2.7).

**As durable bytes, no — in v1, deliberately.** The target's NeoFS is written from a host-built seed image
(OT-87 §4.3), and the installer's write path is raw blocks, not files: writing `/neofs/.system/settings.a` into a
fresh NeoFS volume would mean a metadata write (inode, content hash, root, superblock) inside the most
security-reviewed code in the tree. OT-66 §7 already declined the analogous move for the account record: *the
installer does not create it; the installed system's first boot does.* The same rule applies here:

- the installed system's **first boot runs the same wizard once** (predicate: no valid `NEOSET1` record on a
  writable `/neofs`), writes the record, and never shows it again;
- the Try session's choices are therefore not literally carried; the user may see the same screens twice (once
  in Try, once on the installed first boot).

**The convergence (S7b).** Carry the record so the installed first boot skips the wizard. Two shapes, both
needing their own review: (a) patch the pre-created settings file inside the RAM copy of `neofs-seed.img` before
W4, recomputing its BLAKE3 and the root, with a hand-laid fixture for the patch; or (b) write a fixed-size
first-boot record to a preallocated ESP file and have the installed kernel pass it to the WM. (a) keeps NeoFS the
single authority; (b) avoids a NeoFS metadata write but adds a FAT reader. I recommend (a) when the installer's
seed path is otherwise stable, and only after S7.

---

## 3. What exists today, and what each screen plugs into

| Screen | Today | Plugs into | v1 status (works / stub / new rung) |
|---|---|---|---|
| Language | no i18n at all: every string is a `&'static str`; no `gettext`, no catalog, no locale in the kernel or userland | a new catalog + `Language` key + runtime page text | **hidden stub**, rung S6 |
| Keyboard | one US table (`keymap_us_logic.rs`, PRs #53/#54), no selector; the WM, Arthur and the kernel each decode | the shared table + a layout selector passed to clients | **hidden stub**, rung S5 |
| Network | wired DHCP at boot (RTL8125 and USB CDC-ECM); **no per-interface status call reachable from userland** (`SYS_NET_STATS` is per socket); Wi-Fi driver exists but is not product-wired | `SYS_NET_IF_STATUS` + the view | **kernel state exists, the wizard cannot read it — no page until S4**; **Wi-Fi stub** |
| Time zone | design merged (PR #51), build is Seat I's next; no zone key on `main` yet | `TimeZone`, `libneo::tz`, the confirm call | **works when the TZ build lands**, rung S3 |
| Comfort | keys 1/2 exist and apply live | `settings::set_wm` + the scene | **works today** |
| Try or Install | OT-87 R4.4 installer view exists; the Try flag and the second Limine entry are R5, not built | R5's flag + `installer_view::run` | **needs R5**, rung S7 |

### 3.1 Language

**There is no internationalisation layer.** A search for `gettext`/`i18n`/`localization` in `userland/` and
`kernel/src/` finds nothing; every label is a compiled `&'static str`, and the intro card's wording is checked by
a persona KAT (`userland/neo-atrium/src/intro.rs`). Ubuntu's language page retranslates the wizard instantly
because the Flutter app ships dozens of localisations; we cannot.

A real language screen therefore needs, in order:
1. a catalog (`setup-core` or a small `i18n-core`: message ids, one table per language, a fallback to English);
2. every string the wizard shows moved from a literal to a catalog id (the page titles, the bodies, the option
   labels, the hints);
3. the intro card and Home's verbs in the same catalog, or the language choice changes the wizard and nothing
   else — which would be worse than English only;
4. a `Language` key and its generated count;
5. later, shaping and BiDi (the text engine is Latin-only at T0/T1: `docs/THINK-2026-07-10-atrium-text-engine-research.md`).

**v1 recommendation: hide the page until steps 1–3 exist.** The key can land first (S2) and round-trip, but the
page is compiled only under a test feature (`setup-stub-pages`) so the full walk is still gated.
Showing a one-item picker would be a promise the OS cannot keep, and the PL's ask is about clocks and keyboards,
not translation.

### 3.2 Keyboard layout

The table is `kernel/src/drivers/keymap_us_logic.rs` (PR #53, `587def1ed`, branch `origin/claude/kbd-keypad`;
shifted punctuation is PR #54, `ebf82c642`, branch `origin/claude/kbd-shifted-punct`). It is one US table from a
USB HID usage to ASCII, `const fn printable(usage, modifiers, locks)`, plus Caps Lock and Num Lock
(`kernel/src/drivers/key_locks.rs`). Its own header says every path that turns a key into text reads it: the
kernel's legacy byte path, the WM's Home decoder and Arthur's decoder, by `include!`. PR #53/#54 unify what are
three separate decoders on `main` today (`userland/neo-atrium/src/keys.rs` has its own `printable`;
`userland/neo-arthur/src/showcase_logic.rs` has another; the kernel's `usb/hid_logic.rs` has a third).

**Why there is no selector.** A layout is not a runtime value anywhere: the table is a `const fn` compiled into
each decoder. To add one:
1. a table per layout (`keymap_layout_logic.rs`: an id, a name, the same `printable`/`effective_usage` surface);
2. a selection that reaches **every** decoder: the WM's Home, Arthur's showcase, and any future client. The clean
   shape is the one the time-zone THINK already uses for the zone: the WM passes the layout id in the client's
   session setup, and each decoder selects its table by id. The WM's own decoder selects the same id.
3. the console's raw byte path (`hid_logic`) still uses the kernel's table: either it keeps US (and the screen
   says so), or a `SYS_KEYMAP_SELECT` syscall changes the kernel's table for the session. The desktop is the
   requirement; the boot console staying US is acceptable and cheaper.
4. a test field on the wizard page that types through the **WM's** decoder, so the user sees the effect before
   leaving the screen.

**v1 recommendation: hide the page until two tables exist.** The first non-US table should be one whose keycaps
differ visibly from US (UK is the smallest honest choice: `"`/`@`, `£`/`#`, `\`/`~`), hand-written from the
keycap legends with the same all-keys KAT the US table has (`tools/xtask/src/keymap_tests.rs` is the pattern).
Licensing is not an issue for a layout table written from the standard; no upstream file is copied.

### 3.3 Network

**What works.** The product image compiles both wired paths:
- **RTL8125** (`kernel/src/drivers/net/rtl8125.rs`, product-image features `rtl8125` +
  `rtl8125-dma-without-iommu`, `tools/xtask/src/main.rs:6312-6319`), polled, with a boot DHCP exchange;
- **USB CDC-ECM** through `boot-eth0` (`kernel/src/drivers/usb/boot_eth0.rs`, part of `product`,
  `kernel/Cargo.toml:145-160`), also with DHCP.
The DHCP client is a **single global** bound to one interface (`kernel/src/net/dhcpv4.rs:23`). The RTL8125
bring-up skips DHCP when another interface already holds the boot lease (`rtl8125.rs:1224`); the USB CDC-ECM path
is the other bring-up. The lease is **queryable inside the kernel**: `dhcpv4::snapshot()` (`:365`) returns the
state, the offered and bound `Lease` (ip, mask, router, dns, t1, t2, expiry), and
`identity::snapshot_links()` (`kernel/src/net/identity.rs:231`) returns `(ifindex, up)`.
`ipv4::preferred_address(ifindex)` gives the address. All of that is kernel state; **no userland call exposes the
link, the lease or the address today**.

**What today's `main` cannot do — and why the page waits on S4.** There is no per-interface status call reachable
from userland. `SYS_NET_STATS` is **per socket** (`syscall.rs:5932`, it looks up a socket handle) and returns
packet/byte counters, not link or lease. So a wizard on `main` cannot show a lease and must not claim to: **the
Network page is not compiled in v1** (the hide rule, §1.2 item 5) and **S1's walk has no Network step**. It
appears with S4, when the read-only opcode lands:

```
SYS_NET_IF_STATUS(out: *mut NetIfStatusV1, len: usize) -> 0 | -errno
NetIfStatusV1 { version, flags, ifindex, driver[12], mac[6], ipv4[4], prefix,
                 router[4], dns[4], lease_secs_left }   // fixed size, I-12 byte oracle
```

- `flags`: bit 0 link up, bit 1 lease bound, bit 2 carrier.
- It reads `identity::snapshot_links()`, `ipv4::preferred_address()`, `dhcpv4::snapshot()` under the net locks.
  The syscall body runs IF=0, like `SYS_NET_STATS`; the net locks are plain spin locks today and the CP-37 rule
  is that no IF-on context takes them (OT-97). A read-only syscall adds no new taker class.
- **No new capability** is proposed: it exposes the default interface's link and address to any caller, which is
  not an authority worth a cap (the same information is on the wire). If the PM prefers symmetry with
  `net-dial-cap`, a `NetInfoCap` minted to neo-init is a one-line addition; I recommend not adding a cap for a
  read-only status call.

**What the S4 page shows, and what it must not promise.**
- one row: `Wired: connected — 10.0.2.15` or `Wired: no link` or `Network status is unavailable`;
- one row: the driver (`rtl8125`, `cdc-ecm`) and, if a lease exists, the remaining time;
- one row: `Wi-Fi: coming` (disabled, no list);
- **Continue without a network** always enabled; no screen blocks on a connection.

**Before S4 there is no Network page at all** — not even a Continue-only stub. The kernel has the facts but no
call hands them over, so a page could only say "unavailable" or promise a status it cannot read; the design's own
rule (§1.2 item 5) is to hide a page that cannot do anything. A Continue-only stub would also be honest, but it is
a page the user cannot use; hiding also keeps a dead page out of the S1 gate.

**Why Wi-Fi is a stub, not a list.** The driver exists (`kernel/src/drivers/usb/mt7601u/`, feature `mt7601u`,
not in `product`/`desktop`), with a scan, a WPA2/WPA3-SAE connect path and a boot DHCP loop. But there is **no
scan API, no connect request and no supplicant state exposed to userland**, and no product build enables it. A
list needs a kernel scan-result query, a connect syscall taking a passphrase (a new credential path), and a
supplicant state machine the WM can drive. That is a lane of its own, not a screen. The honest screen says
"Wi-Fi is coming" and points at wired. The hardware history (OT-39's channel-switch wedge) is also why a first
product UI should not promise a working join.

**Why no "test connection" button.** OT-96 R1/R2 (a wire receive path) is not merged; after bring-up only ARP
and DHCP are routed, and nothing accepts an inbound IPv4 packet. A "test" could only dial out (egress exists),
and a failed dial would be blamed on the wizard. Show the lease; do not invent a probe.

**The renewal gap, stated.** `net-tick` is not in image A (OT-95), and renewal is OT-96 R3. A lease shown at boot
can expire while a Try session sits open. The screen's job is the moment of setup; the session's network can die
later. This belongs in the open-threads board, not in the wizard's copy.

### 3.4 Time zone

The design is complete and merged (PR #51, `docs/THINK-2026-11-02-time-zones.md`); the build is Seat I's next. The
wizard plugs into four pieces, not new ones:

| Piece | Where | What the wizard does |
|---|---|---|
| `TimeZone` (4), `RtcLocal` (5) | `settings-core` (time-zone THINK §2.3) | writes 4 with `set_wm`; leaves 5 at its default |
| zone table + `libneo::tz` | `userland/libneo/src/tz.rs` + generated table (§2.2-2.4) | search by name/prefix; `utc_to_local` for the preview |
| `SYS_CLOCK_CONFIRM_REALTIME(offset)` | kernel, feature `rtc-offset` (§2.1) | **one** call when the Time zone page is left |
| `SYS_CLOCK_REALTIME_STATUS()` | kernel, same feature | read once to log whether the clock is confirmed |

The page is a short list first (UTC and a handful of common zones, pointer-pickable) with the Home field as the
search box: typing `new y` filters to `America/New_York`; Enter selects; the card shows the IANA name, the
abbreviation and the offset now (the same information the `timezone` Home card shows). The preview uses the RTC
wall reading, because the clock is unconfirmed until the page is left; it does not claim to be UTC. Leaving the
page is the commit point: it confirms the clock and makes the zone review-only for the rest of the session
(§2.2), and the page-choice records queued before it are appended after it with corrected timestamps (§4.4).

**Dependency.** If `rtc-offset` is off (the kill-switch), the page is hidden and the clock is OT-85's: the wizard
does not fake a zone it cannot apply.

### 3.5 Comfort

The only screen that works end to end today. `ReduceTransparency` and `ReduceMotion` are `settings-core` keys 1
and 2, written by the WM (`settings::set_wm`) and applied live to the base scene's material and motion profile
(`userland/neo-init/src/desktop_session/settings.rs`, the module doc's "Apply"). The wizard's toggles call the
same path, so the frame after the toggle is already in the chosen mode.

This is also the accessibility screen. There is no screen reader, no magnifier and no high-contrast setting; the
design system's floor is WCAG AAA contrast (7:1) and the reduce-transparency material is the legibility floor
(`docs/design-system.md:110-130`, ADR-024's reduce-transparency invariant). The wizard uses those defaults and
offers the two switches it actually has.

### 3.6 Try or Install

The installer exists as a WM view (OT-87 R4.4; `userland/neo-init/src/installer_view.rs`), opened by typing
`install` at Home, with the Install chip shown where `chip_shown()` decides (`:279`). The Try boot flag, the
second Limine entry and the install-mode kernel command line are **R5**, not built. The wizard's screen is
therefore R5-gated.

- **Try** (on a Try boot): end the wizard, show the intro then Home. No writes.
- **Install** (on an Install boot): call `installer_view::run(scene, events)` with the wizard's own focus; it
  owns the list, the confirmation and `SYS_INSTALL_RUN`. The wizard does not compose with it; it ends, and the
  installer's card takes the slot.
- **The other path** is the restart in §2.3, with a confirmation that names why ("the installer needs its own
  boot"), plus **Back** and **Try**.

The R5 rewire point is already named: `chip_shown()` in `installer_view.rs:279`. The wizard's screen and the chip
must agree; the chip is Home's, the screen is the wizard's, and both read the same Try flag.

---

## 4. The UI

### 4.1 Which process draws it

**The WM (`neo-init`), as a view, like the login, lock and installer views.** Five reasons, in order of weight:

1. **The settings store has exactly one writer: the WM.** `settings-core` is pure; `desktop_session/settings.rs`
   does the file I/O and is the only code that writes `.system/settings.{a,b}`. A separate client would need a
   new `OP_SETUP` request path and a policy for what it may set, and it could lie about the choice it reports.
2. **The clock confirm is authority-checked to the WM.** The time-zone THINK §2.1 defines the caller as "the
   session's input-system authority", the same check the input syscalls use. A client is not that authority.
3. **It must run before any client exists.** On an installed first boot, the wizard precedes the account screen
   and the session. There is no client to be a client of.
4. **The focus gate makes it unspoofable.** The WM's own focus is the route while the wizard runs; no client can
   receive a key or draw over it (ADR-074, `kernel/src/atrium/focus.rs`). A client-hosted wizard could be covered
   by another client.
5. **Precedent and size.** The login, lock and installer views are already cards in the WM with the same
   up/down/enter/esc and pointer path. The wizard adds a page machine, not a process.

**The cost is real:** `neo-init` is already 23 MB and the WM's trust surface grows. To keep the diff small and
host-testable, the page machine lives in a new pure crate, `userland/setup-core` (no syscalls, no allocation
beyond `Vec` where the WM already uses one): the page enum, the order, the option lists, the key-to-action map,
the summary rows and the times/search logic. The WM adapter (`desktop_session/setup.rs`) only draws, reads keys
and calls the three sinks (settings, clock, installer). The host KATs run the whole machine without QEMU; the
QEMU scenario proves the pixels and the settings bytes.

**Rejected: a separate `neo-setup` client of the WM.** It cannot write the store, cannot confirm the clock, and
would need a new protocol for both, with the WM as a policy proxy. It buys process isolation the wizard does not
need (it runs before any untrusted code) at the cost of two new surfaces.

**Rejected: kernel-drawn (the boot frame).** No settings store, no zone table, no input beyond the boot console,
and it would put UI policy in ring 0.

### 4.2 Keyboard-only, and the key map

The wizard is operable with **no pointer at all**, and with **no keyboard at all** (the PL's silent test: every
core flow completable with either alone, `docs/OPEN-THREADS.md` OT-52 row). Both are true on every page:

| Action | Keyboard | Pointer |
|---|---|---|
| move | Up/Down, Tab/Shift+Tab | hover |
| choose / toggle | Enter or Space | click the row |
| back | Esc (on Welcome: a Skip confirmation) | the Back button |
| pick a zone | Up/Down through the short list, or type and Enter | click a zone in the short list, or click the field and type |
| finish | the focused button, Enter | click the button |

- Up/Down and Tab move through the same list; the focus ring is the design system's 2 px Electric Blue
  (`docs/design-system.md` focus rule), and the focused row also carries a `>` mark so the state is not
  colour-only (the installer's `> `/`  `/`x ` marks are the precedent, `installer_view.rs`).
- Esc never silently discards: on Welcome it opens a one-row confirmation ("Skip setup and use the defaults?")
  with **Continue** / **Back**.
- No action is pointer-only and none is keyboard-only. The Time zone page therefore offers a **short list** (UTC
  plus a handful of common zones) that the pointer alone can pick from; the field is the fast path, not the only
  path. No time limit, no auto-advance, no animation that carries meaning (Reduce Motion turns the transitions off
  and the final frame is identical).
- The key decoding is the shared table (`neo_atrium::keys`), after PR #53/#54; Caps Lock and Num Lock behave as
  everywhere else, and a keypad key in navigation mode acts as its arrow.

### 4.3 Accessible by construction

- **Type:** page titles at Headline 2 (18 px), bodies at Body large (14 px), never the 10 px caption size for
  anything a choice depends on. The design system's base is 12 px (`docs/design-system.md:104-130`); the wizard
  chooses the larger size because a first-boot screen is read once, under time pressure.
- **Contrast:** body text at ≥ 7:1 (White or Silver on Void, not Electric Blue for body copy); the selected row's
  mark and the focus ring are shapes as well as colour.
- **No colour-only state:** selected is `>` + text, focus is a ring + text; the installer's marks set the pattern.
- **No motion requirement:** the page change is a cut with Reduce Motion on; nothing waits for an animation.
- **No transparency requirement:** with Reduce Transparency the card is the opaque neutral surface; the text
  contrast is unchanged.
- **No hidden text:** every row fits the card at its size; a KAT measures the longest label against the text box
  (the intro's row-fit KAT is the pattern, `scripts/desktop/intro_oracle.py`).
- **No screen reader exists** (there is no accessibility tree in the WM or the kernel). The wizard is text-first
  so a future AT can read the same rows, but v1 must say so rather than claim access it does not have.
- **No password fields** (the account screen is OT-66's, after the wizard), so no echo or masking question here.

### 4.4 Lock and permission rules

The wizard holds **no new capability**, derives none, grants none and revokes none.

| It may | How | Why it is bounded |
|---|---|---|
| write `NEOSET1` `.a`/`.b` | the WM's `settings::set_wm` | the same store and checksum the WM already writes; on Try, the volatile overlay writes nothing |
| confirm the clock once | `SYS_CLOCK_CONFIRM_REALTIME` | the kernel checks the caller is the session's input authority; a second call is `EALREADY` |
| read network status | `SYS_NET_IF_STATUS` | read-only, fixed-size, no cap |
| list disks (only via the installer view) | `SYS_INSTALL_LIST` | read-only; each preflight drops its claim with no write (`claimed_after=0`, OT-87 §4.5) |
| run the install (only via the installer view) | `SYS_INSTALL_RUN` | the kernel's own verdicts and the typed confirmation; the wizard has no install path of its own |

It **does not**: open `/neofs` read-write; read `.system/account.cred*`; mount or unmount anything; write the boot
medium; take an install claim; call `SYS_INSTALL_RUN` directly; spawn a client; or set a key outside the wizard's
three (`Language`, `KeyboardLayout`, `TimeZone`) plus the Comfort pair; `RtcLocal` is left at its default.

**Client reachability.** A client cannot open the wizard. `OP_SETTING` is the only client write path, and the new
keys are classified **WM-only** in the same place the idle-lock key already is (the PM's #222 bounce: a setting
that changes the machine's behaviour is not a client's to set). A time zone silently mislabels every action-log
record; a keyboard layout can make a password the user set impossible to reproduce. They become client-settable
only with a Settings-app
policy of their own.

**While the wizard runs:** no client is spawned; the WM's own focus is the input route; the wizard paints only
its own frames; the intro and Home are not shown until it ends. If a client were spawned by some future path, the
kernel's focus gate means it receives no key, and `compose`/`commit_present` show only the wizard's frames (the
lock screen's two rules, `lock.rs`'s module doc, are the precedent).

**The action log, and the confirm boundary.** Each page choice and the final action are logged
(`setup page=<name> choice=<value>`, `setup done action=try|install`), never a secret. No credentials exist in the
wizard; when Wi-Fi join lands, its passphrase must never reach the action log (the same rule as the account
password).

The log's timestamp is `CLOCK_REALTIME` (`action_log.rs:75-82`), which is the RTC's local wall time until the
confirm; an appended record cannot be rewritten because the chain links every byte. So **the wizard writes no
record before the confirm**: the page choices go into a small in-memory queue (in `setup-core`) and are appended
in order at the confirm, each with `time_s` set from the confirmed clock and the event's monotonic instant
(`confirmed_utc − (m_confirm − m_event)`). The chain then holds the truth and nothing is rewritten. The queue is
at most a handful of rows (one per page choice); if the confirm never happens it is flushed with the time-zone
lane's "unconfirmed" flag and the RTC reading (§2.2), and on a Try boot the append fails on the read-only mount
and the queue is dropped. This needs one small `action_log` seam: an append carrying an explicit `time_s`
instead of calling `now_s()` (`action_log.rs:152`, `:186`).

### 4.5 The frames and the eight-row budget

- Every page is a card in the Home slot (`layout.card(rows)`, eight rows maximum). The title is row 0; the body
  is one or two rows; the options take the middle; the hint is the last row. A page that needs more than eight
  rows is split, not scrolled.
- The page indicator is text (e.g. `2 of 4`, counting only the pages this build shows), not dots: it is
  readable and it is testable.
- The wizard has its own frames, not the intro's: `scene.chrome.set_card(rows)` with the wizard's rows, and
  `scene.present` of the card rect. The intro card is set only when the wizard ends.
- **The frame oracle:** a new `scripts/desktop/setup_oracle.py`, written from the scene specification and the
  wizard's wording (the `intro_oracle.py` pattern), renders each page independently and emits a digest; the
  kernel's scenario reads back the card slot and compares. The label boxes are masked and checked by the column
  pattern from `scripts/fonts/area_oracle.py`, so a wrong word fails, not just a wrong rectangle.
- The final Try/Install page's summary rows are computed by `setup-core` from the same `Settings` the store will
  hold, so the summary cannot disagree with what is written.

---

## 5. Rungs, gates and kill-switches

Dependency shape: **S1 and S2 are independent of the kernel** (the page machine and the store; the store's
volatile mode). **S3** needs Seat I's time-zone build (`rtc-offset` + `libneo::tz` + keys 4/5). **S4** needs a new
read-only kernel opcode. **S5** and **S6** are userland content lanes. **S7** needs OT-87 R5. **S7b** needs R4
and is the convergence. S1 must land before any of them; S2 before S3/S5/S6 (they write keys).

Every rung's scenario asserts **both** the frames it shows and the settings it leaves; where a rung changes only
one, the other is the previous rung's pinned result, so no rung regresses the other axis.

Each rung names its oracle and its kill-switch. The template is the repo's: a `neo-test` QEMU scenario, a serial
script of `<prompt, keys>` steps (`tools/xtask/src/main.rs`'s `SerialScriptStep` pattern, the desktop scenarios),
and a frame digest. The setup menu's WM view reads keys from its own focus, so the test build injects at the
router seam the desktop tests already use (`sys_input_focus_test_inject`, test-only), or through the serial
script's bytes; either way the wizard cannot tell the difference.

### S0 — this THINK and a cross-family read

No code. **Gate:** PM merge after the read. **Kill-switch:** n/a.

### S1 — the wizard shell: pages, navigation, frames

`userland/setup-core` (page enum, order, key map, summary, time-zone search) + `desktop_session/setup.rs` (the
view) + the page renderings, with every value at its default and the Language/Keyboard stub pages compiled behind
`setup-stub-pages` (the Network page does not exist until S4). No store writes, no clock call, no network call, no
install action (Try only), and no action-log write: the page walk is asserted from the serial log, and the log
queue arrives with S3's confirm.

**Gate `setup_menu_walk`:** a serial script walks Begin → Time zone (UTC, Continue) → Comfort (toggle both) →
Try → **Home**. It asserts
- each page's `DESKTOP setup page=<name>` line,
- that no `DESKTOP setup page=network` line exists (the page is absent, not a stub, §3.3),
- the final `DESKTOP setup done action=try`,
- the page digests from `setup_oracle.py` (card slot, labels masked and column-checked),
- that the intro follows (`DESKTOP intro …`) and Home is ready.
**Control:** Esc on Welcome → Skip → Home with no `setup` lines beyond the skip; **and** the `setup-menu`-off
build's `desktop_home_boot`/`desktop_glass_boot` digests are unchanged.

**Kill-switch `setup-menu`:** off ⇒ no hook in `run()`, no new frames, the same first frame and log as today.

### S2 — the store: two keys, the volatile overlay, WM-only

`settings-core` keys 6/7, `LANGUAGE_COUNT`/`LAYOUT_COUNT` generated, the range checks, the WM's `set_wm` path, the
`StoreMode::Volatile` overlay, the Try-flag selection at `desktop_session.rs:1047`, and the WM-only classification.

**Gate `setup_menu_store`:** on a **writable** volume, the test build enables the stub pages, sets keyboard=1 and
Comfort rm=1, finishes, and the scenario reads `settings.a` back through the next boot's `select()` and logs
`setup persisted gen=1 keys=…`; the host recomputes the record with `settings-core`'s host KAT and compares the
BLAKE3. On a **read-only** volume (a Try boot with an installed disk attached), the same walk logs `writes=0`
and the scenario's whole-disk BLAKE3 is identical before/after.

**Host KATs:** the two new keys round-trip; an unknown id is refused; a record lacking them reads defaults; the
record with all seven keys is a hand-laid byte fixture (the format rule: an independent byte example, not just a
round-trip).

**Kill-switch `setup-store`:** off ⇒ the new keys are not compiled and the record is byte-identical to today's
three-key one; the wizard's store calls are no-ops.

### S3 — the Time zone page and the one confirm

Depends on the time-zone build. The page: the field search, the IANA/abbreviation/offset rows, the preview from
the RTC wall reading, the **single confirm when the page is left**, the review-only state after it, and the
action-log queue flush (§2.2, §4.4).

**Gate `setup_menu_tz`:** Jarvis's discriminator — `-rtc base=2026-11-01T01:59:00` with `RtcLocal=1` (a local-time
RTC in New York, in the repeated fall-back hour; the earlier instant is taken, time-zone THINK §2.1). Walk
Welcome (Begin) → Time zone, type `new y`, choose `America/New_York`, leave the page, finish, and assert
- `DESKTOP setup tz=America/New_York`;
- the clock reads `01:59 EDT` and, a minute later, `01:00 EST` (the fall-back KAT from the time-zone THINK §4);
- `SYS_CLOCK_REALTIME_STATUS` is 1; the record's `TimeZone` is the New York id;
- **the action-log ordering and time:** the Welcome record is appended **after** the confirm, and its `time_s`
  lands within a few seconds of `1793512740` (the confirmed UTC, 05:59 — the walk's monotonic delta puts it
  slightly earlier), not `1793498340` (01:59 read as UTC, four hours behind); no record in the log carries
  `1793498340`; and the record chain verifies end to end;
- a host KAT: a second confirm in the same boot returns `EALREADY`, and the wizard's status check never makes
  that call (the page is review-only once confirmed).
**Control:** skip the wizard → status 0, Home shows "zone not set"; the queued records, if any, carry the
time-zone lane's "unconfirmed" flag and the RTC reading.

**Kill-switch `rtc-offset`:** off ⇒ the page is hidden, no confirm, the clock is OT-85's bit for bit.

### S4 — the Network page: `SYS_NET_IF_STATUS`

A read-only kernel opcode with a fixed `NetIfStatusV1` (I-12 hand-laid byte oracle) and the view. Kernel-touching
⇒ the 4-shard gate and an AAG lens on the net-lock/IF rule. **The page is not compiled before this rung**
(§3.3): S4 lands the call and the page together, so there is never a wizard page that cannot read its status.

**Gate `setup_menu_net`:** QEMU with `usb-net` on slirp (the CDC-ECM product path) → the page logs
`setup net link=up driver=cdc-ecm ipv4=10.0.2.15`; **control** with no NIC → `link=down`, Continue still enabled,
Home follows. Host KAT: the fixed-size struct field by field; a `snapshot()` with no lease yields `lease=none`;
the `EINVAL` on a short buffer.

**Kill-switch `net-info`:** off ⇒ the syscall is `ENOSYS`, the page shows "Network status is unavailable" and
Continue, and the rest of the boot is unchanged.

**As built (Seat I, 2026-11-09).** Where the build settles what §3.3 left open:
- **The number is `0x191`**, after `SYS_GETRANDOM` (the network block 0x180–0x18F was full).
- **`NetIfStatusV1` is 56 bytes, version 1**, little-endian: `version`, `flags`, `ifindex` (u32 each), `driver`
  [12], `mac` [6], `ipv4` [4], `prefix` u8, a zero pad byte, `router` [4], `dns` [4], a zero pad u32, and
  `lease_secs_left` u64 (`u64::MAX` without a lease). The hand-laid oracle is `tools/net-if-status-abi-tests`; it
  holds the kernel's struct and libneo's independent byte decoder to the same records.
- **One flag more than §3.3 named: bit 3, `PRESENT`** (a network device exists). It tells "no NIC" from "a NIC with
  no link" without a sentinel `ifindex`.
- **The default interface**: the DHCP client's when it holds a lease, else the first with link up, else the first
  registered. A lease is reported only on its own interface.
- **The lease's time left is on the DHCP client's clock**, whose ticks are the lease's seconds. It advances only
  while something calls `net::tick` (the boot's bring-up; `net-tick` where present), so on image A it is the
  lease's length less the bring-up's ticks. The renewal gap above stands.
- **One coherent record (Janus, #100 relay 382).** The sources live in separate consistency domains (the DHCP
  client, the device and identity registries, the IPv4 table, each driver's carrier), so reading them one after
  another could return a mix no state ever held (an old lease with a new link-down). A **network-status
  generation** (`kernel/src/net/status_gen.rs`) covers them: every writer of a source bumps it **inside its own
  lock, after the write** (a DHCP state, lease or interface change; a device or interface registration; a
  link-state change; an IPv4 address added or expired). The reader loads it, reads, and loads it again; on a
  change it reads again, at most 4 times, then returns the last record with bit 4, `STALE`, set. It never spins
  unbounded. The carriers have no lock, so they are read first, right after the first load: the record is the
  state at that instant. Since the reader takes the writers' locks, a write it saw had bumped before the read,
  and a write in its window moves the generation. The page shows nothing it can't trust: a `STALE` record reads
  "Network status is unavailable." The host discriminator (`tools/net-if-status-abi-tests`) runs a writer in
  every gap between the reader's reads, and shows a mixed record without the generation and a held-or-flagged
  one with it.
- **No new lock order.** Each network subsystem lock is taken alone and released before the next, and
  `CLIENT`'s hold is unchanged. The syscall body runs IF=0, like every other net-lock taker (`net-tick` runs its
  step IF-off), so it adds no new taker class (CP-37). None of them is an AI-governance lock (I-14).
- **`net-info` is not in `desktop` yet**: the setup menu isn't in the product image. Its gate kernel is
  `neo-test-setup-net-boot` (the intro kernel plus `net-info`). That makes the kill-switch a QEMU control:
  `setup_menu_net_off` boots the plain intro kernel, where the number is `ENOSYS` and the page says the status is
  unavailable. `setup_menu_net_nonic` is the no-NIC control on the `net-info` kernel.
- **The page's words**: "Wired: connected, 10.0.2.15" (or "no address yet"), "Device: cdc-ecm. Lease: 23 h 53 min
  left.", "Wired: no link.", "Network status is unavailable.", and "Wi-Fi: coming in a later version." Continue
  reads "Continue" with an address and "Continue without a network" otherwise. The Try summary doesn't name the
  network: it isn't a choice.

### S5 — keyboard layouts: a second table and a selector

`keymap_layout_logic.rs` with US + UK, the WM's decoder selecting by id, the id passed to Arthur at spawn (and to
any future client), the test field, and keys 6/7's layout half.

**Gate `setup_menu_keys`:** the test build enables the page; choose UK, type `"` `@` `#` `\` into the test field,
and the logged characters are the UK ones; then Home uses UK, and a control leaves US and gets the US ones. Host
KATs: every key × Shift × Caps × Num for both tables, written from the keycap legends; the two tables agree except
where the legends differ.

**Kill-switch `keyboard-layouts`:** off ⇒ one US table, the decoder identical, the page hidden.

### S6 — Language: the catalog and a second language

`i18n-core` (message ids, the English table, one translated table), the page strings moved to ids, the intro and
Home's verbs in the same catalog, the `Language` key enabled, the page shown when `LANGUAGE_COUNT > 1`.

**Gate `setup_menu_lang`:** choose the second language; every wizard page's strings and the intro card change
(frame digests), the record holds `Language=1`; a control English. Host KAT: every message id present in both
tables, no fallback taken on the wizard's own strings, the longest row fits.

**Kill-switch `i18n-catalog`:** off ⇒ English constants, the page hidden, the build byte-identical.

### S7 — Try or Install, with the R5 flag

The R5 flag, the second Limine entry, the wizard's screen, the installer handoff, and the restart confirmation
(§2.3).

**Gate `setup_menu_choice`:** Try → Home, and an attached installed disk is bit-identical before/after (OT-87
R5's hash). Install (Install boot) → the installer view lists the disks and a blank virtio disk installs. Control:
no bundle → Install explains and offers Try.

**Kill-switch `installer`:** off ⇒ the screen's Install row is absent, Try is the only action; the boot menu
remains the low-level chooser.

### S7b — carry the record into the installed system (convergence)

Either the seed patch or the first-boot record (§2.4). **Gate:** an installed disk's first boot reads the record
(gen 1) and does not show the wizard; the values match the wizard's choices; a control with no record shows the
wizard. **Kill-switch `installer-seed-settings`:** off ⇒ the installed first boot runs the wizard, as S2.

### 5.1 The kill-switch matrix

| Rung | Feature | Off ⇒ |
|---|---|---|
| S1 | `setup-menu` | no hook, no frames, today's `desktop_*` digests unchanged |
| S2 | `setup-store` | keys absent, today's 3-key record byte-identical |
| S3 | `rtc-offset` (the TZ lane's) | page hidden, clock OT-85 |
| S4 | `net-info` | `ENOSYS`, page says unavailable, boot unchanged |
| S5 | `keyboard-layouts` | US only, decoders identical |
| S6 | `i18n-catalog` | English constants, page hidden |
| S7 | `installer` | no Install action; boot menu only |
| S7b | `installer-seed-settings` | installed first boot asks, as S2 |

Every rung's gate pairs the rung-on scenario with a rung-off control at the same SHA; the `setup-menu`-off control
is the existing golden desktop scenario, not a new one, so the "menu off = today's boot" claim is the same
evidence the desktop already trusts.

---

## 6. What would prove this design wrong

Each row names the check that settles it, and what changes if the check fails. A design that cannot name its
falsifier is not a design.

| # | If this is true | Then | The check |
|---|---|---|---|
| 1 | The WM cannot host a pre-desktop view with a volatile store without touching every `settings::set` call site, coupling the wizard to glass/motion/login | the wizard is a **separate client** with a WM-mediated `OP_SETUP`, and the store writes stay in the WM; the cost is a new protocol | Count the `settings::` call sites and prototype S1/S2; if the diff reaches the scene, change the shape |
| 2 | The pre-confirm records cannot be timestamped correctly (the log only knows `now_s()` from `CLOCK_REALTIME`, `action_log.rs:75-82`, and the chain forbids rewriting) | fall back to **tagging** them "unconfirmed" at append (the time-zone lane's flag) instead of correcting them; the wizard then records the RTC reading plus the flag, and S3 must ship that flag first | Jarvis's discriminator: `RtcLocal=1`, RTC at `2026-11-01 01:59` New York, make a page choice before the Time zone page, then set New York and finish; the page record must land within a few seconds of `1793512740` (05:59 UTC, the earlier ambiguous instant), not `1793498340` (01:59 read as UTC, four hours behind) |
| 3 | A layout cannot be selected per session for every decoder (the console, Arthur and the WM disagree) | the keyboard page promises text it cannot deliver; move layout selection to a pre-boot chooser, or accept US on the console and say so | S5's first build: type the same keys in Home, Arthur and the console |
| 4 | `SYS_NET_IF_STATUS` cannot report a lease without new kernel bookkeeping (the DHCP snapshot is not per interface, or the locks forbid the read) | the network page becomes an unconditional Continue; drop it rather than show "unknown" on every boot | S4's spike: read `dhcpv4::snapshot()` and `identity::snapshot_links()` from a syscall body |
| 5 | The install bundle cannot be loaded at runtime (as today), **and** the PL rejects a restart at the Try/Install screen | the wizard has no Try/Install screen; the Limine menu is the only chooser, and the wizard ends with "Continue" | R5 + L3u; ask the PL which cost is acceptable |
| 6 | The serial script cannot drive a focus-gated WM view deterministically | the gate asserts logs and settings, not frames; the frame oracle moves to a host-only render test | S1's spike: one page, one scripted key, one digest |
| 7 | Users skip the wizard, or the defaults are wrong on most machines, so it adds a boot delay and the clock stays wrong | the zone must be guessed (RTC offset? no geo) or the screen made mandatory; the wizard loses its "optional" claim | Measure the skip rate on the preview; the PL's own run is the first data point |

Each load-bearing choice has a named check that can break it: the WM view (1), the single confirm (2), the
per-session layout (3), the status call (4), the Try/Install screen (5), the gate mechanism (6), and the whole
wizard's value (7).

---

## 7. Open questions (PM / PL)

1. **Hide or show the Language and Keyboard screens in v1?** I recommend hide (GNOME's rule): a picker that
   changes nothing is worse than no picker. The page code still lands behind `setup-stub-pages`, so the full walk
   is gated.
2. **The installed first boot asks again (v1).** I recommend accepting that, following OT-66 §7's precedent, and
   making S7b the convergence; the alternative is a NeoFS metadata write in the installer's reviewed path.
3. **The Try/Install screen costs a restart until the medium is readable.** I recommend keeping the screen (the
   PL asked for the choice) with an explicit Restart, and the boot menu as the low-level path, until L3u.
4. **`RtcLocal`.** I recommend leaving it out of the wizard and at its default 1 (the dual-boot convention), with
   `rtc utc` for UTC machines. A wizard row would ask a question most users cannot answer and would make the
   clock wrong in a new way on the machines that guess wrong.
5. **Does the wizard run on every Try boot?** I recommend yes (nothing persists; the visible screens are
   Welcome, Time zone, Comfort and Try/Install, §2.3), but it is a product call the PL should confirm on the
   preview.
6. **Is the network screen worth its rung?** The survey says it is optional everywhere, and elementary does not
   even have one. The PL asked for it, and it costs one read-only syscall; I recommend landing it, but it must
   never block and its Wi-Fi half waits for the Wi-Fi lane. It has no page until S4 (§3.3).
7. **A "test connection" button?** Not in v1: no wire RX (OT-96 R1/R2). Revisit when OT-96 lands.
8. **A zone change after the confirm.** The wizard's zone is a one-way commit for the session (§2.2): write-once
   forbids a second confirm. Home's `timezone <name>` after setup (time-zone THINK §2.5) must define whether a
   post-confirm change re-confirms (a delta call, with its own concurrency KAT) or takes effect at the next boot.
   The same question governs the wizard's Back into its own Time zone page. I recommend the delta re-confirm, but
   it changes the time-zone lane's write-once contract, so it is the PM's call.

---

## Revision 2 — relay 285 (Jarvis's two FAILs)

**Reviewed:** `origin/dexter/setup-menu-think` `a514ea631`, relay 284
(`Plans/RETURN-2026-11-02-setup-menu-review-jarvis.md`). PASS: the security model (§4.1, §4.4); Try vs Install
persistence (§2.3, §2.4), keeping the S2 ordering note — `StoreMode::Volatile` is chosen **before**
`settings::load`, which writes an initial record when none exists; and the rungs/kill-switches (§5). Two FAILs,
fixed here.

1. **Network readiness was overstated (§3).** Revision 1's table said "wired read-only works", which is false
   until S4: `main` has no per-interface status call reachable from userland (`SYS_NET_STATS` is per socket). The
   table now says the kernel state exists but the wizard cannot read it, and **the Network page is not in v1 at
   all** — S4 lands the opcode and the page together. There is no Continue-only stub (a page that cannot say
   anything breaks the design's own hide rule, §1.2 item 5); S1's walk loses its Network step, §3.3 and §5 S1/S4
   say so.
2. **§6 falsifier #2 was already true.** Revision 1 confirmed the clock at the end of the wizard, so every
   page-choice record written before it took `CLOCK_REALTIME`'s RTC-local reading as UTC
   (`action_log.rs:75-82`) and stayed wrong after the confirm — a hash-chained record cannot be rewritten. Fixed
   three ways, together:
   - **the confirm moves to the moment the Time zone page is left** (§2.2, §3.4), so every record after that
     point is correct;
   - **the wizard writes no record before the confirm**: its page choices queue in memory and are appended after
     the confirm with `time_s` from the event's monotonic instant (§4.4), so the pre-confirm records are
     corrected, not tagged;
   - **the zone is a one-way commit for the session**; a later change is Home's `timezone` verb (new open
     question §7 q. 8).
   The discriminator is Jarvis's: `RtcLocal=1`, RTC at `2026-11-01 01:59` New York (the repeated fall-back hour;
   the earlier instant is taken). The pre-confirm page record must land within a few seconds of `1793512740`
   (05:59 UTC), not `1793498340` (01:59 read as UTC, four hours behind), and the chain must verify.

Also refreshed at this revision: the keyboard citations now point at the merged commits on `main` (#53
`b03abfbab`, #54 `cab959819`), not their branches, and §3.2's KAT path is cited from `main`.
