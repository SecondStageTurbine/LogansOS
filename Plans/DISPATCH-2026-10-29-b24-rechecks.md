<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RELAYS #183 → Chad and #184 → Jarvis: post-merge re-checks of batch 24

Batch 24 (`claude/pm-handoff-document-ypgky3` @ `477a68f3`) gates #227–#230. The Claude AAG found no blocking item.
These re-checks run on the squashes once they're on `main` (`git log origin/main --oneline | grep -E '#22[7-9]|#230'`).
Each item: CLOSED or STILL OPEN, the enforcing `file:line`, and a concrete input for anything open.

## #183 → Chad: your #148, #165 and #170. Bundle, so **MODEL:** GPT-6.1-Sol. **EFFORT:** high.

1. **#227 (your #148, `Plans/RETURN-2026-10-29-ot87-r4.4-ui-review-chad.md`).**
   - Does each button's hit box equal its drawn label, with a dead zone? Note the paint path is Inter at
     `CARD_TEXT_PX`, not the 15 px Plex Mono your review assumed.
   - Does the decoder reject every unknown class, action, refusal value and flag bit?
   - Does the scenario click the drawn Back label's centre on both screens?
2. **#229 (your #165, `Plans/RETURN-2026-10-29-ot61-c2.2b-review-chad.md`).**
   - Do a table's slots stay charged until drained or dropped, with no double release?
   - Is the backlog-scaled listener weight released on close?
   - The AAG's coverage note: the scenario exercises only `charge_next` plus a channel install. Name any path you
     think needs its own leg.
3. **#230 (your #170, `Plans/RETURN-2026-10-29-tcp-listen-bounds-review-chad.md`).**
   - Can a SYN admitted concurrently with close leave a half-open or accepted connection no listener references,
     with two real CPUs?
   - Is `retired` checked under the same lock admission uses?
   - The AAG's note: `RACE_CLOSE` is test-only, but is it taken at mixed IF states (CP-37)?

One pushed RETURN: `Plans/RETURN-2026-10-29-b24-recheck-chad.md`. Paste the SHA.

## #184 → Jarvis: your #143 on #228. **MODEL:** GPT-6.1-Sol. **EFFORT:** medium.

Your review: `Plans/RETURN-2026-10-29-ot61-c2.2-review-jarvis.md`, the #143 item.
1. Does a refused map free its leaf frame on every early return?
2. Is a replaced leaf freed once, after the shootdown?
3. Does the discriminator count real frames (`FRAMES_IN_USE`), and does it fail without the fix? The AAG notes the
   count is global, so another CPU allocating during the leg could fail it falsely. Is that a flake risk under
   `NEO_SMP=4`?

Commit `Plans/RETURN-2026-10-29-mem-quota-leaf-recheck-jarvis.md`, push it, and paste the SHA.
