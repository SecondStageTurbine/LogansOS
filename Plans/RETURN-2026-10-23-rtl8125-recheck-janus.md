<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: PR #101 F1–F3 re-check — Janus

**Verdict: CLOSED. All four dispatch items PASS.**

Reviewed exactly `1d2af6eb88a786dec904dd7b69545d9180d4b4a7`, against prior
head `612c0283db3ff1cb07ea483bbf43a2e797236539` and original RETURN
`18ddd2f0:Plans/RETURN-2026-10-23-rtl8125-review-janus.md`.
Dispatch: `origin/main:Plans/DISPATCH-2026-10-23-rtl8125-recheck-janus.md`
(read after fetching origin). Branch: `codex/rtl8125-recheck-janus`.
Review only; no driver changes or wider PR acceptance decision.

Source references below are at the reviewed commit. `driver` means
`kernel/src/drivers/net/rtl8125.rs`, `logic` means
`kernel/src/drivers/net/rtl8125_logic.rs`, and `tests` means
`tools/rtl8125-logic-tests/src/lib.rs`.

## Checklist

1. **PASS — F3 closed.** `logic:495` rejects wire lengths over the armed
   capacity and post-FCS frames shorter than 14 bytes (`logic:509`); lengths
   of four bytes or less take the earlier error path. `logic:711` supplies
   the ring's buffer capacity to that classifier, and `logic:723` emits a
   `BadLength` event instead of a frame. The ring is armed for 2048 bytes
   (`driver:987`), matching the RX allocations (`driver:977`). Thus opts1
   `0x30003fff` and `0x30000805` (wire 2053, formerly frame 2049) are rejected
   before any `RxFrame` exists. `driver:492` takes the rejected slot buffer,
   counts it and returns it to the pool; `driver:500` refills the ring.
   The only frame path goes through the ready queue (`driver:483`) and
   `rx_take` (`driver:626`), whose independent `len > buffer.len()` guard
   (`driver:635`) recycles an invalid entry before construction. Accepted
   frames have offset zero (`driver:643`), so the slice at
   `kernel/src/net/device.rs:59` stays within the buffer.

   KATs at `tests:524` cover both original inputs, EOR, capacity boundaries,
   and a 13-byte/14-byte header boundary. The register-model regression at
   `tests:556` injects both oversized inputs, asserts two bad lengths and
   only one valid delivery, then verifies all four buffers are rearmed.
   These assertions discriminate the old classifier: it returned frames
   of 2049 and 16379 bytes for those inputs, producing zero bad-length
   events and three deliveries rather than the required two and one.
   This old-code counterfactual was checked against the original source,
   not executed as a mutation test.

2. **PASS — F2 closed.** `driver:1255` uses `drain_bounded` with
   `PUMP_FRAME_BUDGET = 24` (`logic:735`). Its loop at `logic:741` invokes
   the receive closure at most 24 times, even when every call succeeds;
   each handled frame is recycled (`driver:1271`). Consequently continuous
   RX no longer prevents return to the counted DHCP loop and tick
   (`driver:1190`, `driver:1192`) or the counted ARP loop (`driver:1240`).
   `tests:627` starts with 16 completed descriptors, replenishes one after
   every take/recycle, and asserts return after exactly 24 frames; it also
   checks an empty source returns zero. The test passed independently.

3. **PASS — F1 closed.** `driver:431` delegates refill to the shared
   `refill_from` helper (`logic:672`). Fence refusal takes only the reject
   branch (`logic:692`), which increments `RX_UNFENCED` and retains the
   actual `Arc<DmaBuffer>` in `rx_unfenced` (`driver:437`, `driver:438`).
   Descriptor publication is reachable only from the successful fence
   branch (`logic:686`, `logic:659`); refusal neither publishes nor silently
   drops the buffer. Both report formats expose `unfenced=`
   (`driver:747`, `driver:1281`). `tests:573` uses the original outside-arena
   address `0x200000`, checks retention, no publication and conservation;
   `tests:611` also checks termination when all six buffers are refused.
   Both tests passed independently.

4. **PASS — nothing else moved.** The complete
   `git diff 612c0283 1d2af6eb` touches exactly four files: the driver,
   logic, tests, and `docs/concurrency-pitfalls.md`. Every hunk concerns
   F1 retention/accounting, F2 bounded pumping, or F3 length validation,
   their tests, or documentation of the length trap
   (`docs/concurrency-pitfalls.md:3443`). No unrelated change was found.

## Independent validation

- Ran the unchanged host-test crate and shared logic copied from this
  worktree into a disposable container's tmpfs, using the existing
  `neoplutarch-dev:latest` image. Source mount and container root were
  read-only; networking and container logging were disabled. No other
  worktree or repository-root checkout was used.
- Command: `cargo test --offline --manifest-path tools/rtl8125-logic-tests/Cargo.toml --target x86_64-unknown-linux-gnu --target-dir /build/target`.
  **30 passed, 0 failed; doc-tests 0.**
- Ran `cargo clean` with the same manifest and target directory:
  **109 files, 18.4 MiB removed**, exit 0. Build output stayed in memory.
- No full kernel build, QEMU, silicon run, raw-disk operation or internal
  disk write was performed. Existing unrelated `.gitignore` and `.ignore`
  worktree changes were left out of this RETURN commit.
