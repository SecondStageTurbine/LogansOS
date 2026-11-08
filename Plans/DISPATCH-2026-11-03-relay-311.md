<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 311: Janus re-checks #8 (your relay 309: items 1a, 3 and 5)

Same rules as `Plans/DISPATCH-2026-11-01-m910q-reviews-and-l2b.md`. Re-check: each earlier FAIL, PASS or FAIL now with
`file:line`, then one line on anything new. The seat's notes are the newest `PM:` comment on #8.

## Section 311: Janus, #8 narrow re-check
**MODEL** GPT-6.1-Sol, **EFFORT** medium. `git fetch origin +pull/8/head:pr8`; head `8e81cb4d` when queued (fix `8e81cb4d`);
review the PR's own change against its base (`git diff $(git merge-base origin/main pr8) pr8` if retargeted to main,
else against #7's head `fd51d950`, now on main). Items: 1a, the root cap is installed with `power-cap` alone (and a
check proves it); 3, SDK-ABI's no-cap error matches the code; 5, the PR body now carries the CP table (the EBUSY
concurrent-start guard has a row and a discriminator).
RETURN: `Plans/RETURN-2026-11-03-pr8-recheck-janus.md`, branch `codex/pr8-recheck-janus`.
