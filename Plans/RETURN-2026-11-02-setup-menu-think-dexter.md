<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Relay 282: the first-boot setup menu ("Try or Install") THINK

**Seat:** Dexter. **Answers:** `Plans/DISPATCH-2026-11-02-setup-menu-think-dexter.md`, Section "Relay 282"
(F11: an Ubuntu-style setup menu for Try/Install).
**Branch:** `dexter/setup-menu-think`, cut from `origin/main` @ `88ec5e866`.
**Head (the THINK artifact; this RETURN's parent):** `35e13dfd7191f1b5d132f6b10edfa103f647b110`.
**Kernel-touching:** **no.** This relay changes no code. The deliverable is one design THINK and this RETURN.

## Delivered

| Item | Path | Notes |
|---|---|---|
| Design THINK | `docs/THINK-2026-11-02-setup-menu.md` | 725 lines. Answers the dispatch's six points: the prior-art survey (§1), our screen flow and storage split (§2), what exists today and what each screen plugs into (§3), the UI process/keyboard/accessibility/permission rules (§4), seven build rungs + one convergence with gates and kill-switches (§5), and what would prove the design wrong (§6) |

Only those two files are on the branch. No code, no fixtures, no crates, no generated files.

## The answer in brief

1. **One wizard, drawn by the WM.** It is a pre-desktop view in `neo-init`, like the login, lock and installer
   views — not a separate app. The WM is the only settings writer, it is the authority the kernel's clock-confirm
   call is defined to accept, and it runs before any client exists. The page machine lives in a new pure crate
   (`userland/setup-core`) so the state machine is host-tested and the WM adapter only draws and reads keys.
2. **The screens:** Welcome → Language → Keyboard → Network → Time zone → Comfort → **Try or Install**. Language
   and Keyboard are hidden in v1 until a second language/layout exists (GNOME's skip-list rule); their page code
   lands behind a test feature so the full walk is still gated. Comfort works today; Time zone works when Seat I's
   time-zone build lands; Network is wired-read-only with a Wi-Fi stub; Try/Install needs OT-87 R5.
3. **Try vs installed storage.** Try uses a new `StoreMode::Volatile`: defaults, in-memory writes, the installed
   disk's record is not read, nothing is written (the Try volume is read-only, OT-87 §6.3). Installed writes one
   `NEOSET1` record (keys 1/2/4/6/7; `RtcLocal` stays at its default) on the installed system's first boot.
4. **Two new keys.** `Language` = 6 and `KeyboardLayout` = 7, `Choice` with generated `LANGUAGE_COUNT` /
   `LAYOUT_COUNT`, range-checked by `settings-core`; all four new behaviour-changing keys are WM-only for
   `OP_SETTING` (the PM's #222 rule). The record stays within `MAX_ENTRIES` (7 of 16).
5. **One clock confirm, at the end.** The Time zone page only previews (from the RTC wall reading); the single
   `SYS_CLOCK_CONFIRM_REALTIME` runs when the wizard leaves the last settings page, so Back-and-change survives the
   write-once contract. Skipping the wizard leaves the clock unconfirmed ("zone not set"), never wrong.
6. **Carried into Install: in memory yes, durable bytes no (v1).** The installer's seed is host-built and its write
   path is raw blocks; writing a settings file into a fresh NeoFS volume would be a metadata write in OT-87's most
   reviewed code. This follows OT-66 §7's precedent (the installer does not create the account; the installed
   first boot does). The installed first boot runs the same wizard once; S7b (the seed patch, or a first-boot ESP
   record) is the convergence.
7. **The one bend:** until the install medium is readable (L3u + FAT/ISO9660), the Install bundle must be loaded at
   boot, so the Try/Install screen's "other" action is an explicit **Restart** into the other Limine entry, never a
   silent reboot. Once the medium is readable, one boot can load nothing and the wizard's last screen is the real
   choice, Ubuntu-style.
8. **The gate shape.** Each rung has a `neo-test` QEMU scenario, a serial script of `<prompt, keys>` steps, a new
   `scripts/desktop/setup_oracle.py` frame digest (the `intro_oracle.py` pattern), and a kill-switch. Every rung's
   scenario asserts both the frames and the settings it leaves; `setup-menu` off reproduces today's golden
   `desktop_home_boot`/`desktop_glass_boot` digests.
9. **The new kernel surface is one read-only opcode,** `SYS_NET_IF_STATUS`, returning a fixed `NetIfStatusV1`
   (link, driver, MAC, IPv4/prefix, router, DNS, lease remaining) built on `identity::snapshot_links()`,
   `ipv4::preferred_address()` and `dhcpv4::snapshot()`, with an I-12 hand-laid byte oracle. S4 is kernel-touching
   and takes the 4-shard gate + AAG. No new capability.
10. **The falsifiers are explicit (§6):** WM-view coupling (1), confirm timing (2), per-session layout (3), the
    status call's realizability (4), the Try/Install screen's cost (5), the scripted gate (6), and the wizard's
    value if users skip it (7).

## Checklist, in the dispatch's order

| # | Dispatch item | Where | State |
|---|---|---|---|
| 1 | Prior-art survey (Ubuntu `ubuntu-desktop-provision`/Subiquity, GNOME Initial Setup, one more), licences, last commits, screens/order/skippable/no-network/Try-vs-Install | §1, §1.1, §1.2 | Done. Five repos, all GPL-family (read-only). Ubuntu's exact step enum and Try/Install page, GNOME's page table + hide/skip rules, elementary's `TryInstallView`, Fedora `initial-setup`'s conditional first boot |
| 2 | Our flow, in order; what each sets; Try vs installed storage; what is carried into Install | §2.1–§2.4 | Done, with the key/id table, the volatile overlay, the one-confirm rule, and the S7b convergence |
| 3 | What exists today and what each screen plugs into (time zone, keymap, wired, Wi-Fi, installer, WM); works/stub/new rung | §3, §3.1–§3.6 | Done, with `file:line` and branch SHAs for the unmerged keymap PRs |
| 4 | The UI: which process draws it, keyboard-only, accessibility, lock/permission rules | §4.1–§4.5 | Done; the WM view is argued and the two alternatives are rejected |
| 5 | Rungs in build order, each with a gate oracle (injected keys, settings, frames) and a kill-switch (menu off ⇒ today's boot) | §5, §5.1 | Done: S0–S7 + S7b |
| 6 | What would prove the design wrong | §6 | Done: seven falsifiers, each with its check |

## Verification performed

- **Base and citations:** read at `origin/main` `88ec5e866` (the commit that carries the dispatch). Every code
  reference was read from that tree or, for the two keyboard PRs, from `origin/claude/kbd-shifted-punct`
  (`ebf82c642`) and `origin/claude/kbd-keypad` (`587def1ed`), because they are not on `main` yet.
- **Prior art:** five repositories read through the GitHub API and raw files on 2026-11-02; licences and last
  commits are in the §1 table (`a64330a621ec`, `c948f9bc97a3`, `7ee9a4dd6769`, `17ec614c481f`, `350247997b20`).
  All are GPL-family: design read only, nothing copied. The window/row tables and the screen copy are our own.
- **Nothing was built or run.** There is no code change to build. The gate oracles in §5 are designs, not runs;
  each names its dependency so it cannot be mistaken for a green gate.
- **Working tree:** only the two files (`git show --stat` on the two commits: one THINK, one RETURN).
- **SPDX:** the THINK and this RETURN both carry `<!-- SPDX-License-Identifier: CC-BY-4.0 -->` as line 1.
- **No personal details** beyond the repo's existing "the PL" convention; no accounts, credentials or paths from
  the PL's machine.

## Decisions requested from the PM (also §7 of the THINK)

1. **Hide or show the Language/Keyboard screens in v1.** I recommend hide, with the code behind `setup-stub-pages`
   until a second language/layout exists.
2. **Accept the installed first boot asking again (v1),** following OT-66 §7, with S7b as the convergence? The
   alternative is a NeoFS metadata write in the installer's reviewed path.
3. **Keep the Try/Install screen at the cost of an explicit Restart** until the medium is readable, or drop it to
   the Limine menu until L3u?
4. **`RtcLocal` stays out of the wizard** (default 1, `rtc utc` for UTC machines)? A wizard row is a question most
   users cannot answer.
5. **Does the wizard run on every Try boot?** I recommend yes (nothing persists; four screens after Welcome).
6. **Land the Network screen's read-only status opcode** (S4), or defer the screen? It costs one syscall and
   never blocks; the Wi-Fi half waits for the Wi-Fi lane.

## Recommended first build

**S1 + S2 as one lane** (the page machine/view and the store) — a wizard that cannot persist is a demo, and the
two are the only rungs with no external dependency. Then **S3** the moment Seat I's time-zone build is on `main`.
**S4** in parallel if a kernel window opens (it is the only kernel-touching rung and needs the 4-shard gate + AAG).
**S5** and **S6** are userland content lanes; **S7** waits for OT-87 R5; **S7b** waits for R4.

No merge is requested for this branch beyond the THINK's design review; the PM may merge the two docs on the
usual docs path.
