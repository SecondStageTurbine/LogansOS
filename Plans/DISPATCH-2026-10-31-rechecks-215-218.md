<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Re-checks of the batch 38/39 bounces (run by the PC relay runner)

Each seat fixed the findings in your earlier RETURN. Check only those findings (and anything the fix broke). Fetch with
`git fetch origin +pull/<N>/head:pr<N>` and read `git diff origin/main...pr<N>`. Answer as a plain checklist: PASS or
FAIL, the enforcing `file:line`, and a concrete input for any FAIL.

**Delivery:** commit the RETURN file named in your section on a new branch named after it, off `origin/main`. Push
it and reply with the commit SHA. Never push to main.

## Section 215: Chad, re-check of #267 (head `cd3385ff`) and #241 (head `8c7bdf9e`)
**MODEL** GPT-6.1-Sol, **EFFORT** high: a bundle runs on one model.
1. #267 (your 3b): is the log sink closed before the power path, so nothing after `Done` can wait on the disk? Do
   the new stuck-sink legs cover both power-off and restart, and would they fail on the old code?
2. #241 (your 1b): is the interface snapshot now bounded (what happens when the table keeps growing)?
3. #241 (your 1c and 4): is the 512 now called an admission threshold everywhere (code, PR, CP-54), or are
   in-flight segments counted against it?

RETURN: `Plans/RETURN-2026-10-31-b38-recheck2-chad.md`.

## Section 216: Janus, re-check of #250 (head `572fba15`)
**MODEL** GPT-6.1-Sol, **EFFORT** medium: a narrow re-check of your own findings.
1. Your item 4: is off (`neofs-cache-evict` disabled) now the old behaviour bit for bit, including `forget_range`?
2. Your item 5: is the dirty bound now true, or is the CP-64 row corrected to the real bound?
3. The partial-write retry: is it bounded now, and what does a caller see at the bound?

RETURN: `Plans/RETURN-2026-10-31-ot92-cache-recheck-janus.md`.

## Section 217: Jarvis, re-check of #254 (head `040d0e62`)
**MODEL** GPT-6.1-Sol, **EFFORT** medium: a narrow re-check of your own findings.
1. Your item 1: is the "from power-on" claim now established, or honestly labelled, and are out-of-order times
   refused?
2. Your item 4: is an unrecorded firmware field omitted instead of printed as 0.00 s? Is an estimated TSC rate
   marked?
3. Your item 2 (a bad user pointer stops the kernel) is kernel-wide and goes to a separate lane. Just confirm #254
   makes it no worse.

RETURN: `Plans/RETURN-2026-10-31-l71-boot-time-recheck-jarvis.md`.

## Section 218: Dexter, re-check of #247 (head `cf96e17f`) and review of #253 (head `93cbc705`)
1. #247 F1: is the Lens narrowed, or is a browse-only root justified, and is THINK line 90 corrected?
2. #247 item 5: are the path-scoping prior-art rows present, with licenses and dates, and do they hold?
3. #247: is the new `..` leg a real escape test?
4. #253: is your 204 F1 (the intent read) now polled and bounded, with a stall leg? Are the test gaps from your
   213 re-check (record-full Always, identity mismatch, escape during the wait) covered?

RETURN: `Plans/RETURN-2026-10-31-k1b-l21-recheck-dexter.md`.
