<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #38): narrow re-check of PR #101 F1–F3 → Janus

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. This is a narrow re-check of your own three findings from relay #34
(RETURN `18ddd2f0`).

**Type:** review only. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `1d2af6eb` (pinned). It is one commit
on top of the `612c0283` you reviewed.

## Setup

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/rtl8125-recheck-janus
          git fetch origin && git checkout -B codex/rtl8125-recheck-janus 1d2af6eb88a786dec904dd7b69545d9180d4b4a7

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-23-rtl8125-recheck-janus.md      # this file
git show 18ddd2f0:Plans/RETURN-2026-10-23-rtl8125-review-janus.md            # your RETURN
git diff 612c0283 1d2af6eb                                                   # the fixes
```

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete input or sequence.

1. **F3 closed.** Check that an RX completion whose length is larger than the armed buffer capacity (your inputs:
   opts1 `0x30003fff`, and wire length 2053 → 2049), or shorter than an Ethernet header, is rejected as an error
   **before** any `RxFrame` is built. Check that its buffer is recycled, and that no path from `reap` to
   `as_slice()` can slice past the buffer. Check that the KATs cover both of your inputs and would fail on the old
   classifier.
2. **F2 closed.** Check that `pump_once` drains a bounded batch per call (budget 24), so continuously replenished RX
   cannot keep it from returning. Check that the boot DHCP/ARP loops therefore advance, and that a model test with
   endless RX shows it returning.
3. **F1 closed.** Check that a buffer the fence refuses in `refill_rx` is kept and counted (`unfenced=`), and is
   never published to a descriptor or silently dropped.
4. **Nothing else moved.** Check that `git diff 612c0283 1d2af6eb` touches only `rtl8125.rs`, `rtl8125_logic.rs`,
   `tools/rtl8125-logic-tests` and `docs/concurrency-pitfalls.md`, and only for F1–F3.

A FAIL is reported only after you have re-checked it yourself.

## Close condition

- Commit `Plans/RETURN-2026-10-23-rtl8125-recheck-janus.md` on your branch and **push it**, with the verdict
  (CLOSED or STILL OPEN) and each item's result. Paste the pushed SHA to the PL.
- If you build, `cargo clean`. Never write any machine's internal disk.
