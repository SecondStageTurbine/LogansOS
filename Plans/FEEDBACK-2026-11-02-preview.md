<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# J.c.'s feedback from the first preview (2026-11-02)

Every item J.c. reports from trying LogansOS goes here, with its owner and state. The PM updates it.

| # | What J.c. saw | Cause (as understood) | Owner | State |
|---|---|---|---|---|
| F1 | The launcher said QEMU wasn't installed (QEMU from MSYS2) | The .bat only looked in `C:\Program Files\qemu` and PATH | PM | Fixed (#49); next preview |
| F2 | Accelerated mode (`Start LogansOS.bat`, WHPX) freezes at boot: a kernel #GP after timer calibration | An unsupported MSR/feature access (to confirm) | Seat E | PR `claude/whpx-boot-gp` gating |
| F3 | Arthur's thinking symbol sits inline after "Arthur>" | Design: move it to the header, "…" on the reply line | Seat H | PR `claude/arthur-thinking-header` gating |
| F4 | The clock shows UTC (6:29 AM at 1:29 AM EST) | No time zone setting | Seat I (PR 3) | Queued after F5, F6 |
| F5 | Shift+; types `;`, not `:` | Shifted punctuation missing from the keymap | Seat I (PR 1) | PR `claude/kbd-shifted-punct` gating |
| F6 | The numeric keypad does nothing | Keypad scancodes/usages and Num Lock unhandled | Seat I (PR 2) | Queued |
| F7 | Dragging windows is laggy | Mostly compatibility mode (TCG, software CPU) with a software compositor; also full-frame redraws during a drag | Seat F (after #15) | Queued |
| F8 | Arthur answers "what time is it" with a model disclaimer | The model has no clock; Arthur should answer time/date questions from the system clock (with the user's zone, F4) | Seat I (with F4) | Queued |
