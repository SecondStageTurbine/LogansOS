<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE: the intro's greeting with a wall clock (supersedes one line of NOTE-2026-10-20)

**Author:** S4 (Claude). **Date:** 2026-10-21. **Thread:** OT-85 (the wall-clock rung).
**Supersedes:** exactly one point of `Plans/NOTE-2026-10-20-arthur-first-boot-intro-s2.md`: §3 Q3 and the
table row that justify **"Good day"** ("the kernel has no wall clock, so a time of day could be wrong"). The rest
of that note, and the PL's decisions on it, stand unchanged.

## What changes, and only when

The change applies only when **both** hold:
- the WM is built with `desktop-clock` (in the product set), and
- the kernel actually has a wall-clock epoch. `rtc-epoch` seeded realtime from the RTC, and the WM checks that
  realtime − monotonic ≠ 0 inside one tick.

Otherwise the card is NOTE-2026-10-20's, word for word, with "Good day". A fallback boot (RTC invalid or absent)
therefore never greets with a time of day that could be wrong.

With a clock, line 1 of the approved variant A splits into two rows:

```
Good afternoon. It's 5:48 PM on Wednesday, October 21, 2026.
I'm Arthur: Assistant for Real-time Tasks, Home automation and User Requests.
```

Every row after these is unchanged. The USB-only card has eight rows, which is `CARD_MAX_ROWS`.

## Why two rows, not "Good afternoon. I'm Arthur: …"

That single row would be 93 bytes. NOTE-2026-10-20 raised the bound to 90 only for the variant-A greeting. The
brief for this rung says not to weaken the persona KAT; a needed exception is to be reported, not taken. Split into
two rows, each fits the ordinary bound for new rows (≤ 80):
- the greeting-and-time row is at most 63 bytes ("Good afternoon. It's 12:59 PM on Wednesday, September 30, 2026.");
- the name row is 77.

No bound moved.

## The words

- **Greeting by local hour**, with the boundaries stated in code (`neo_atrium::clock`):
  - morning 05:00–11:59;
  - afternoon 12:00–17:59;
  - evening 18:00–04:59.

  "Good night" is a farewell, so it is not used.
- **The time** is 12-hour with AM/PM, no seconds. **The date** is "Weekday, Month D, YYYY". Both come from
  `libneo::civil`, the same formatter the gate's probe uses.
- **Register** (`docs/VISION-arthur-persona.md`): one plain statement of fact, no exclamation, no emoji.
- **The persona KAT** (`neo-atrium` `clock_persona_kat_every_variant_every_hour`) runs over every model state ×
  USB-only × all 24 hours, plus the longest texts. It holds the original rules unchanged. The one rule that
  existed only because there was no clock ("no time-of-day word") becomes stricter: exactly one row carries a
  time-of-day word, the first, and the word must match the hour. The clock-less variants keep the original
  rule as it was.
- **While the card shows**, a new minute re-words row 1. The card never shows a stale time.

## Home readout (not part of the card)

- Above the intent field, two lines on the backdrop: "5:48 PM" (44 px, primary) over
  "Wednesday, October 21, 2026" (17 px, secondary).
- There is no panel and no bar. It is ambience, not a status strip; the desktop-UX synthesis rejected the
  taskbar metaphor.
- Masked from the digest oracles like the other text. `scripts/desktop/clock_oracle.py` proves it draws nothing
  outside its box.
