<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 282: Dexter, THINK: the first-boot setup menu ("Try or Install"), Ubuntu-style

**Asked by J.c. (2026-11-02):** "Let's do a menu like Ubuntu has, where you get to choose your Wi-Fi or internet,
where you get to put in the time, keyboard, and all that kind of stuff for the 'Try it before you install it'
sessions."

**MODEL** DeepSeek, high: a design THINK, no code. Same rules as your earlier relays (public repo: no personal details).

Write `docs/THINK-2026-11-02-setup-menu.md`:

1. **Prior-art survey** (CLAUDE.md rule; licences and last-commit dates, read for design, nothing copied unless the
   licence allows): Ubuntu's desktop installer (the Flutter `ubuntu-desktop-provision` / Subiquity flow), GNOME Initial
   Setup, and one more (e.g. elementary or Fedora's first-boot). What screens, in what order, what's skippable, how
   they handle no network, and how "Try" differs from "Install".
2. **Our flow:** the screens in order (language, keyboard layout, network, time zone, then Try / Install; anything
   else the survey says belongs), what each sets, where each setting is stored for a Try session (in memory, lost at
   shutdown) vs an installed system (persisted), and what's carried into Install.
3. **What exists today and what each screen plugs into:** read main: the time-zone design
   (`docs/THINK-2026-11-02-time-zones.md`, PR #51), the keymap (`kernel/src/drivers/keymap_us_logic.rs`, PRs #53/#54),
   wired networking (RTL8125, USB-Ethernet; DHCP), Wi-Fi (`mt7601u`, not product-wired), the install design
   (OT-87: grep docs for it), and the desktop session/WM. For each screen: works now / stub with a clear message
   ("Wi-Fi is coming") / needs a new rung.
4. **The UI**: drawn by which process (the WM's session, or its own client of the WM), keyboard-only operable,
   accessible (readable sizes, focus visible), and the lock/permission rules (nothing in the menu can grant itself
   more than the session has).
5. **Rungs** in build order, each with its gate oracle (a scenario that steps through the menu with injected keys and
   checks the stored settings and the frames), and a kill-switch (the menu off ⇒ today's boot, unchanged).
6. What would prove the design wrong.

- BRANCH: `dexter/setup-menu-think` from `origin/main`. Commit only the THINK and your RETURN.
- RETURN: `Plans/RETURN-2026-11-02-setup-menu-think-dexter.md` on that branch, naming the head SHA.
