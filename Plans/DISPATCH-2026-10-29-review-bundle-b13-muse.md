<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW BUNDLE (relays #132–#134) → Muse: R3 and R4.1 re-checks, and the NeoFS commit barrier

Three security reviews of Claude-authored code. All three merged after batch 13 (run `40799093977`, green at
`0395982f`). **Read the files at each squash SHA from the repo, not a summary.** The PL pastes your RETURNs back, and
the PM banks them. Write each as a plain checklist: pass or fail, the enforcing `file:line`, and a concrete input for
any fail.

## #132: #172 `cf438ab8`, the R3 GPT fixes (your R3 item 5). Banked as `Plans/RETURN-2026-10-29-ot87-r3-recheck-muse.md`

Your RETURN: `Plans/RETURN-2026-10-27-ot87-r3-guard-review-muse.md`. Janus re-checks its own item 7 on the same PR.
Files: `kernel/src/drivers/block/gpt_logic.rs` (`validate`, `any_overlap`, `metadata_lbas`),
`kernel/src/drivers/block/write_guard_logic.rs` (`partition_claims`), and
`tools/block-guard-logic-tests/src/partition_guard_tests.rs`.
1. Your layout (the ESP and a NeoFS entry over the same span, with a sealed superblock at LBA 2048) arms nothing on
   `judge()` or `judge_with_partitions()`, and a write to LBA 2048 is refused.
2. A one-sector overlap at either end, at S=512 and S=4096, arms nothing, while entries that only touch still arm.
3. When the primary overlaps and the backup is clean, the backup is judged on its own.
4. The two named differential divergences from the legacy guard each only remove write permission.

## #133: #180 `c805f2b2`, the NeoFS commit barrier and deferred reuse. Banked as `Plans/RETURN-2026-10-29-neofs-barrier-review-muse.md`

Storage durability in the product path (no kill-switch: it fixes an ungated path). Files:
`kernel/src/fs/neofs/{commit_logic.rs,cache.rs,inode.rs,mount.rs}` and `docs/specs/neofs-format.md` §3.4–§3.5.
1. **The barrier.** No superblock copy is written while a non-superblock write since the last flush may still be
   volatile. The order is flush, then the copies, then flush.
2. **A flush error** stops the drain with the superblocks unwritten, so the old copies stay valid.
3. **Drain serialization.** The worker only `try_lock`s the drain lock, and fsync's `drain_all` takes it. Can two
   drains interleave and break the barrier's "flushed since the last write" claim?
4. **Deferred reuse.** Blocks freed by a truncate are not handed out until every present superblock copy holds the
   freeing generation or later, proven by a completed flush. Is the deferral recorded before the freeing commit
   allocates its own free map?
5. **No flush, no writable mount.** `supports_flush() == false` refuses a writable mount (`NoDurableFlush`). Can any
   path still reach the flush helper on such a device and skip silently?
6. Could a crafted on-disk volume make deferred reuse grow without bound, or hand out a block the durable generation
   still names?

## #134: #181 `ee629498`, NVMe namespace notices and reset detection (your R4.1 item 4). Banked as `Plans/RETURN-2026-10-29-ot87-r4.1-recheck-muse.md`

Your RETURN: `Plans/RETURN-2026-10-27-ot87-r4.1-claim-review-muse.md`. Behind `installer`, in no product image.
Files: `kernel/src/drivers/block/{nvme.rs,nvme_event_logic.rs,claim_logic.rs,install_claim.rs}`,
`tools/install-claim-logic-tests/src/{nvme_tests.rs,tests.rs}`.
1. **Your sequence** (a destructive claim; namespace 1 detached and a twin attached; the controller ready throughout)
   stops the claim before the twin is written: `MediaChangedBefore`.
2. **A twin with a different NGUID/EUI64** fails the identity at claim time and at every step boundary. An all-zero
   namespace refuses a destructive claim.
3. **Resets.** Does every reset kind (controller, function-level, NVM subsystem) stop the next write? Is the stated
   limit (a controller reset re-enabled by another agent with the driver's exact CC) the only gap?
4. **The poll.** AER completions are serviced under the admin queue lock in `media_state()`, and the ISR is
   unchanged. Can an admin command lose its own completion to the AER, or the reverse?
5. **The spec rows S1–S13** in the PR body are marked "to confirm". Check each against the NVMe Base Specification
   PDF: pass, fail, or not checkable.
6. **Item 1 ride-along.** A claim that wrote anything moves its slot to a new generation when it drops, so its
   enumeration is `StaleToken`.

The cross-family Codex read of #180's cut KAT and #181's spec rows is a separate relay (#135–#136, Chad).
