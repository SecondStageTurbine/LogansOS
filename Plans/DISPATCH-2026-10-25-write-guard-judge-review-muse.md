<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #57): security review of the write-guard judgement wait (#120) → Muse (Gwen)

**MODEL:** Muse (Gwen). **Why:** this changes the disk write guard, the code that keeps LogansOS from writing a disk it
must not touch (on the Zenbook, the internal Windows NVMe). Security reviews of Claude-authored write-guard code go to
Muse (CLAUDE.md), as for PR #85 and #88.

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** a
RETURN. Muse can't push, so the PL pastes or uploads it and the PM saves it as
`Plans/RETURN-2026-10-25-write-guard-judge-review-muse.md`.

**Reviewed SHA:** `0275b2f3` on `main` (the squash of PR #120; gate run `40182595694`, green). Read the files at that
SHA from the repo, not a summary:
- `kernel/src/drivers/block/mod.rs` (`GuardedDevice::arming`, `wait_for_judgement`, `judge`, `write_block`,
  `read_block`, `GuardReader`);
- `kernel/src/drivers/block/write_guard_logic.rs`;
- `kernel/src/test_harness.rs` (the `block_write_guard` scenario, the judge-contention leg);
- `kernel/Cargo.toml` (`neo-test-write-guard-judge-spin-control`);
- `Plans/NOTE-2026-10-25-write-guard-judge-hang.md`;
- `docs/concurrency-pitfalls.md` CP-82;
- the lock audit on PR #120: https://github.com/SecondStageTurbine/LogansOS/pull/120#issuecomment-6396227883

**What changed.** Before, every caller of `arming()` entered `spin::Once::call_once`. A caller running with interrupts
off would spin on an unfinished judgement while the judging thread, doing block I/O, sat on the same CPU and never
ran again, so the machine hung. Now only the caller that wins a compare-and-swap on `judging` runs the judgement. Every
other caller waits for `arming.get()` with interrupts on, yielding each round.

## Checklist

Answer each item PASS or FAIL, with the `file:line` that enforces it. For a FAIL, give a concrete sequence of calls.

1. **The judgement runs exactly once per device.** Only the `judging` compare-and-swap winner calls `call_once`, and
   no path can run `judge()` twice or skip it.
2. **No write before the decision.** No caller's `write_block` reaches the inner device before that device's arming
   decision is complete, including a caller that arrives while the judgement is in progress.
3. **Refusals still hold.** A write outside the allowed extent, or to an unmarked disk, is still refused during the
   judgement, after it, and when the judgement itself fails (a read error, an allocation failure). The refusal counter
   and the first-refusal log behave as before.
4. **The judgement itself reads, never writes.** `judge()` and `GuardReader` issue only reads to the device.
5. **The interrupts-on wait is safe for every caller.** Check S4's lock audit against the code. No caller that can
   reach `arming()` holds an interrupt-safe lock or any lock an interrupt handler takes. The caller's interrupt state
   is restored before the inner device is used.
6. **The test-only items stay out of the product.** The judge pause hook, the wait counters and
   `neo-test-write-guard-judge-spin-control` compile only under their `neo-test-*` features. None of them is in the
   product feature set (`PRODUCT_IMAGE_KERNEL_FEATURES` in `tools/xtask/src/main.rs`).
7. **Coverage.** The `block_write_guard` judge-contention leg would fail if a second caller spun with interrupts off.
   Name anything the gate doesn't cover, e.g. AHCI/NVMe on real hardware.

## Close condition

Give the RETURN to the PL with PASS/FAIL per item, `file:line`, and an overall verdict (PASS or FAIL). The PM saves it
under `Plans/`.
