<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay #129: A9 pointer window

Worktree: `E:\AI Automation\workers\janus`. Branch: `codex/review-bundle-2026-10-29-janus`.
Reviewed squash `de8547c1736669a7627ef3dcaff532c8bae2c17f` (#179). The two pointer modules are unchanged on current-main `87458b7e`, where the reproduction ran. **Overall: FAIL — a reader can still display rx > in.** RAG healthy; no implementation fix.

1. **FAIL F1 — single-word open/close does not make a counter snapshot atomic.** `kernel/src/ai/pointer_window_logic.rs:62` publishes last then open; `:69` closes with one store. Those fix the old multi-store reset window. But `counts` at `:101` loads minted before rx separately, and `kernel/src/ai/prefill_perf.rs:424` calls `PTR.counts().0` and then `PTR.counts().1` in separate argument evaluations. Concrete interleaving with one unchanged open ID: reader obtains in=0; a DELIVERY holder mints tag 8 (minted=1) and the WM dequeues it (rx=1); reader obtains rx=1. The displayed pair is (0,1), despite every individual writer state satisfying rx<=in. No weak-memory behavior or window transition is needed. The same interleaving can occur between the two loads inside a single counts call.

   Independently executed a host probe against the actual unchanged module:

   ```rust
   w.open();
   let input = w.counts().0; // timeline's first argument
   w.note_minted(8);         // another CPU runs between reader loads
   w.note_rx(8);
   let rx = w.counts().1;    // timeline's second argument
   assert!(rx <= input);
   ```

   `rustc +stable --edition=2024 target/review-janus/pointer_split.rs` compiled; execution printed `production split read: in=0 rx=1; invariant=false` and failed the assertion. The probe is an ignored local artifact; only this RETURN is committed. `rx_never_exceeds_in_at_any_store_boundary` (`pointer_window_logic.rs:163`) permutes complete sequential Mint/Rx calls and reads only at the end of each script (`:156`). It does not interleave reader loads, so its success does not establish the advertised reader invariant. Use one coherent reader snapshot and add a reader/writer interleaving discriminator; merely switching the call site to one existing counts call leaves the internal two-load window.

2. **PASS for ordinary serialized writers, with an epoch-wrap limitation — packed counts do not numerically carry.** `pointer_window_logic.rs:107` saturates the low 32 bits, tags every bump, and restarts at 1 on an ID mismatch. Mint/dequeue writers both hold DELIVERY in `kernel/src/atrium/focus.rs:259` and `:417`; so an old callback cannot arrive after a later callback has installed a newer ID in that same count. Open itself does not clear count cells. This addresses ordinary next-open carry. IDs deliberately wrap after u32::MAX opens (`pointer_window_logic.rs:57`): a count cell untouched for the entire cycle can alias an ancient ID and expose its old count. That is a stated-width lifetime limit, not proof of unlimited epoch uniqueness. It is separate from F1, which occurs on the first generation.

3. **PASS — pitfall table is present in PR #179 and its scoped N/A judgments hold.** CP-2/3 add no blocking resource or cross-CPU wait; CP-21 adds no IPI or scheduling behavior; CP-25 adds no partition-loop work. Writer CAS contention is serialized by DELIVERY rather than a fixed retry budget inherent in fetch_update. The table's 'bounded fetch_update' wording should be read with that caller condition. This does not establish a coherent reader snapshot.

4. **PASS for compile-time gating; .text hash not independently rebuilt.** `kernel/src/ai/mod.rs:118` gates the module, and all pointer hooks in focus are prefill-perf gated. The removed touchpad getter had only the removed diagnostic caller. PR #179 records identical product-minus-prefill-perf .text SHA-256 `44720c5f0252cc7703a0ed1a9a93853ad710dc57de0a020c646fa0d0827be985` in matched build conditions. The diff is consistent with that claim; this review did not repeat that kernel binary comparison.

Validation: the deterministic split-reader probe above reproduces F1. `docker compose run --rm --no-deps neo cargo test -p ai-ops-tests pointer_window_logic` passed all 3 existing pointer-window tests (306 other unit tests filtered out). Their success alongside this reproduction demonstrates the missing reader-interleaving coverage; no QEMU result is inferred from them.
