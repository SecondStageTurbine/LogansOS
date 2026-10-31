<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: fix three post-merge review FAILs on A9 (#162 wheel, #159 scrollback, #156 repetition guard) → S1

**Why.** The Codex review bundles came back on 2026-10-27. Three of S1's merged A9 PRs FAIL. All three features are
in `PRODUCT_IMAGE_KERNEL_FEATURES` or the product userland, so these defects are in the image J.c. boots. The PM
re-checked the findings at `origin/main` `91320d01`.

Under the escaped-defect contract this is **fix-forward with a deadline.** Gated fixes must be pushed by
**18:00 UTC (2:00 pm EDT) today**. Any PR whose fix hasn't been pushed by then is reverted. Reverting #159 also means
reverting #162, which scrolls through it. Do the wheel fix first.

## PR 1 — #162 wheel (`claude/a9-wheel-fix`). RETURN: `Plans/RETURN-2026-10-27-a9-arthur-wheel-review-chad.md`

- **F1: a forwarded wheel evicts a key.** `kernel/src/atrium/input_queue_logic.rs:38` (`is_pointer`) and `:48`
  (`carries_wheel`) don't know kind 5 (the WM-forwarded WHEEL). On a full client queue, a forwarded wheel therefore
  falls through to the key-eviction branch at `:173–:188`. PM-checked: the predicates match kind 2 and 3 only.

  Fix: an incoming kind-5 wheel with nothing cheaper to evict is dropped, the same as a raw wheel. Add Chad's F1 KAT
  exactly as given: 64 keys, then a kind-5 wheel. Expect `DroppedIncomingMotion`, with key 4 still at the head.
- **F2: press plus wheel outruns RESYNC.** `kernel/src/atrium/focus.rs:474` mints both records before either is
  enqueued. When the second enqueue evicts a live release, `note_lost` runs only at `:486`, after the press's flags
  are fixed. So a new press reaches the old grab without RESYNC.

  Fix: make the loss visible to the record that follows. Mint, enqueue and `note_lost` one record at a time, or put
  the wheel before the press, or have a lost release force RESYNC on the press. Choose one, and say why it keeps
  CP-52.

  Add Chad's combined ring+queue+`pointer_grab::decide` KAT exactly as given. Reproduce the failure first, then show
  it passing.
- **Wording:** fix the "never evicted" comment on raw wheels. The fallback at `:187` can evict one wheel from a queue
  that holds nothing but wheels.

## PR 2 — #159 scrollback (`claude/a9-scrollback-evict-fix`). RETURN: `Plans/RETURN-2026-10-27-a9-arthur-scrollback-review-chad.md`

- **Item 1: `view_top` goes stale after the 512-line cap evicts lines.** `TextSurface::settle` and `scroll`
  (`userland/neo-arthur/src/showcase_text.rs:319–337, 360–368`) keep the stored `view_top` even after `first_kept`
  has moved past it.

  Fix: clamp or advance the stored `view_top` when lines are evicted, not only while drawing.

  Add a KAT on the exact sequence Chad gives:
  1. Fill to 512 lines.
  2. Page up to the top.
  3. Append 10 lines while the view is held.
  4. Page down: the view lands on line 21, not 11.

## PR 3 — #156 repetition guard (`claude/a9-repetition-fix`). RETURN: `Plans/RETURN-2026-10-27-a9-repetition-guard-review-jarvis.md`

- **Item 1:** at `p == 1.0` the penalty must leave the logits untouched, bit for bit. Return early before touching
  them (`kernel/src/ai/repetition_logic.rs:36–52`). Add a KAT with a signalling NaN (`0x7f800001`).
- **Item 2, revised by the PM at 14:15 UTC.** `loop_end` (`:58–68`) treats overlapping 16-token windows as a loop.
  That cuts an 8-token phrase said three times, which contradicts the stated contract that a phrase shorter than 16
  tokens is never cut.
  - The first call was to keep the contract and require non-overlapping windows. S1's oracle re-run showed the cost: a
    one-token loop would then show 16 repeats before the cut, not one. **So the contract wording changes and the code
    doesn't.**
  - The new contract: the output is cut only after at least `ngram` (16) tokens of exact periodic repetition beyond the
    phrase's first occurrence (the last 16 + p tokens repeat with period p), and the cut keeps one copy.
  - Add KATs: `[0..8]×3` → `Some(8)` (one copy kept), and `[0..8]×2` → `None` (a phrase said twice is never cut).
  - `expected_guard_tokens.txt` is unchanged.

## PR 4, after the three above and with no deadline — #151 presents/liveness counters (`claude/a9-prefill-perf-window-fix`). RETURN: `Plans/RETURN-2026-10-27-a9-presents-liveness-review-janus.md`

This is a diagnostic overlay only. It never changes a reply, so the PM isn't reverting it.

- **Item 1/2.** `begin_generation` clears `GEN_END_TSC` before it clears `PTR_RX`
  (`kernel/src/ai/prefill_perf.rs:158–172`). `end_generation` snapshots the driver reports (`:228–230`) before it
  publishes the end (`:232`). So the WM's `note_pointer_rx` can count a dequeue that the input side doesn't, which
  gives `in=0 rx=1`. That reading is outside the three documented diagnoses.
- **Fix.** Make the window one published generation number, or order the stores so a reader can never see a
  half-open window. Then `rx` counts only the dequeues that `in` can see. Add a host KAT that interleaves a dequeue at
  each store boundary.
- **Item 5.** Add the missing pitfall-bank table to the PR body (CP-2, CP-3, CP-21, CP-25).

## All four

- One PR each, each rebased onto `origin/main`. Run `scripts/worker-close-gate.sh` on each.
- Kill-switch: with the feature off, `.text` is identical to `main` (`NEO_BUILD_ID` pinned). Give the hashes.
- Pitfall bank: PR 1 changes an IRQ-safe delivery path (the DELIVERY guard). Give a short CP table: CP-37, CP-52,
  CP-54 and CP-56.
- Send the PM each head SHA, read from `git ls-remote`, as soon as it's pushed. The PM gates them in the next batch.
- Reviews after merge: Chad re-checks F1/F2 and the scrollback item; Jarvis re-checks the guard items (narrow
  re-check, GPT-6.1-Sol medium).

```
WORKTREE: cloud session S1 (your own clone)
BRANCH:   claude/a9-wheel-fix, then claude/a9-scrollback-evict-fix, claude/a9-repetition-fix
          git fetch origin && git checkout -B claude/a9-wheel-fix origin/main

git show origin/main:Plans/DISPATCH-2026-10-27-review-fails-fix-s1.md
git show origin/main:Plans/RETURN-2026-10-27-a9-arthur-wheel-review-chad.md
git show origin/main:Plans/RETURN-2026-10-27-a9-arthur-scrollback-review-chad.md
git show origin/main:Plans/RETURN-2026-10-27-a9-repetition-guard-review-jarvis.md
```

RAG isn't available in the cloud session. Cite code by `file:line`. Before ending, `git status --porcelain` and
`git log @{u}..` are both empty.
