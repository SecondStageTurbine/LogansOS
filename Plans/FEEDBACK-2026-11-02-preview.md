<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# J.c.'s feedback from the first preview (2026-11-02)

Every item J.c. reports from trying LogansOS goes here, with its owner and state. The PM updates it.

| # | What J.c. saw | Cause (as understood) | Owner | State |
|---|---|---|---|---|
| F1 | The launcher said QEMU wasn't installed (QEMU from MSYS2) | The .bat only looked in `C:\Program Files\qemu` and PATH | PM | Fixed (#49); next preview |
| F2 | Accelerated mode (`Start LogansOS.bat`, WHPX) freezes at boot: a kernel #GP after timer calibration | An unsupported MSR/feature access (to confirm) | Seat E | **Fixed** (#52, `671fe846`): XSAVE area sized from the enabled components (CP-100); follow-ups in #57 |
| F3 | Arthur's thinking symbol sits inline after "Arthur>" | Design: move it to the header, "…" on the reply line | Seat H | **Fixed** (#50, `d3ee311d`) |
| F4 | The clock shows UTC (6:29 AM at 1:29 AM EST) | No time zone setting | Seat I | **Fixed, on main:** #59 (`f77b9310`) and #60 (`cb71ab69`); in the preview of 2026-11-03 6:36 AM Eastern (`cb71ab69`) |
| F5 | Shift+; types `;`, not `:` | Shifted punctuation missing from the keymap | Seat I | **Fixed** (#53, `b03abfba`) |
| F6 | The numeric keypad does nothing | Keypad scancodes/usages and Num Lock unhandled | Seat I | **Fixed** (#63, `cab95981`) |
| F7 | Dragging windows is laggy | Mostly compatibility mode (TCG, software CPU) with a software compositor; also full-frame redraws during a drag | Seat F | **Fixed, on main:** #58 (`1bbc05db`, 2026-11-08): the drag is measured, and only the damaged band recomposes; cross-window drag test (Dexter re-check PASS) |
| F8 | Arthur answers "what time is it" with a model disclaimer | The model has no clock; Arthur should answer time/date questions from the system clock (with the user's zone, F4) | Seat I (with F4) | **Fixed, on main:** #68 (`08a3f30f`); in the preview of 2026-11-03 2:23 PM Eastern |
| F9 | A wrong password at login is refused silently; J.c. wants "Incorrect password. Try again." | The login screen gives no failure feedback | Seat I | **Fixed, on main:** #62 (`d8217c05`); in the preview of 2026-11-03 6:36 AM Eastern |
| F10 | Holding Backspace (or any key) doesn't repeat | The i8042 driver drops typematic repeats on purpose (FINDINGS trap 9) expecting software auto-repeat, which was never built | Seat I | **Fixed, on main:** #61 (`504d582f`); in the preview of 2026-11-03 6:36 AM Eastern |

**2026-11-02 midday:** J.c. confirmed the accelerated launcher boots on his machine (F2 verified on real WHPX). F4/F8
(time zone, Arthur's time answers) are still open: only the design (#51) has merged; the build is Seat I's.
J.c. chose to **trim** the cloud seats (7-day usage warning): keep B, C, D, E, F, I; H and G are done; A retires
after the tiered-gate stack lands.

**J.c. (2026-11-02 midday):** he'll test F4/F8 (time zone), F10 (key repeat) and F9 (password message) together:
the PM tells him once all three are on `main` **and** in a published preview. No interim updates on these.

| F11 | J.c. wants an Ubuntu-style first-boot setup menu for Try/Install: language, keyboard, network (Wi-Fi), time zone | New feature (MVP: OT-87 Try or Install) | Dexter (THINK, relay 282, PR #64), then Seat I builds | **Design approved and merged** (#64, `dfa85c06`; Jarvis re-check relay 305 all PASS). Seat I builds it (rungs S1-S7b) after F8 (#68). **S1 on main** (#70, `858e1284`, 2026-11-08: the wizard shell); **S2 on main** (#71, `1b0b9097`, 2026-11-09: choices saved, volatile overlay when the store can't be written); S3 (time zone + clock confirm) in review (#92, Seat I); S5 (keyboard layouts, US + UK) dispatched to Seat E 2026-11-09 |


**Landing order (2026-11-03 03:45 UTC).** gate-plan puts #59, #62, #61 and #60 each in `full-alone` (they touch
xtask, `Cargo.*` or the kernel), so each lands as its own batch of one, about 100 minutes apiece, in that order.
The preview for J.c.'s test follows the fourth.
