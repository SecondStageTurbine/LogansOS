<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Cross-family pre-merge reviews for batches 35 and 36 (run by the PC relay runner)

Each section is one relay. Fetch the PR with `git fetch origin +pull/<N>/head:pr<N>` and review
`git diff origin/main...pr<N>`. Answer as a plain checklist: PASS or FAIL, the enforcing `file:line`, and a concrete
input for any FAIL.

**Delivery:** commit the RETURN file named in your section on a new branch named after it, off `origin/main`
(`codex/<slug>-<coder>` or `dexter/<slug>`). Push it and reply with the commit SHA. Never push to main.

## Section 203: Dexter, security review of #248 (Arthur's actions behind the WM permission prompt, head `6f00f886`)
1. Can Arthur, or any client, perform a mutating action without a prompt that the user answered with their own
   input? Can a client forge the answer or the event?
2. Is "read-only by default" enforced by the WM or kernel, not just by Arthur's own code?
3. Does "Always" store its grant WM-only (under `.system/`), and can a client add one there?
4. Can a client draw a fake prompt, or can the prompt be answered while the lock screen is up?
5. Is the prior-art section (CP table and survey) present, and do its claims hold?

RETURN: `Plans/RETURN-2026-10-31-arthur-prompt-review-dexter.md`.

## Section 204: Dexter, security review of #253 (the hash-chained action log NAL1, head `124ea1e7`)
1. Is the record format byte-oracled, by hand?
2. Is the chain verified on read, and do truncation or tampering fail closed?
3. Is the log WM-only (`.system/`), and can a client write, truncate or replace it?
4. Is every grant, prompt answer, action and undo logged with its intent, and is growth bounded?

RETURN: `Plans/RETURN-2026-10-31-action-log-review-dexter.md`.

## Section 205: Chad, review of #245 (OT-61 C2.3b, the vendored html5ever + selectors closure, head `e3fe798a`)
**MODEL** GPT-6-Luna, **EFFORT** high: contained, with no kernel change.
1. Does every vendored crate record a compatible license (MIT, Apache, BSD, MPL, zlib or ISC), its upstream URL and
   its version?
2. Does any `build.rs` do something unexpected on the build host?
3. The `parking_lot_core` patch on SYS_FUTEX: are wait and wake correct, with no lost wakeup and spurious wakes
   handled?
4. The memchr AVX/XSAVE check.
5. Are the default and product builds unchanged?
6. Is the new CI build job bounded in time and disk?

RETURN: `Plans/RETURN-2026-10-31-ot61-c2.3b-review-chad.md`.

## Section 206: Chad, light review of #259 and #260 (Dexter's SDK-ABI refresh and CI size budgets, heads `fc663850`, `e0b8121e`)
**MODEL** GPT-5.6-Luna, **EFFORT** medium: docs, a host test and CI.
1. #259: does the drift test fail when a non-test syscall is missing from `docs/SDK-ABI.md`? Spot-check five entries
   against `kernel/src/arch/x86_64/syscall.rs`.
2. #260: does the CI step measure the right build and file (image A's kernel `.text`, the ISO), fail over budget,
   and stay deterministic?

RETURN: `Plans/RETURN-2026-10-31-sdk-abi-size-budgets-review-chad.md`.
