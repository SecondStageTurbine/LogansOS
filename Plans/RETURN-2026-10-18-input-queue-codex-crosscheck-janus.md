<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: input-queue split cross-check — Janus (Codex)

**Verdict: BOUNCE**

Reviewed the merged change on `origin/main` (merge `13287c58839633235c0274867282c6393f29ddbb`, plus the subsequent input-path changes now present at main). One deterministic correctness finding survives re-verification.

## Finding

### HIGH — a key overflow can evict the only unsuperseded button transition

**Location:** `kernel/src/atrium/input_queue_logic.rs:145-152`

When the queue is full, an incoming non-pointer event first looks for a motion-only event, then calls `oldest_superseded_transition(false)`. That helper intentionally excludes the newest queued pointer event. If that pointer event is at index zero and keys follow it, the final fallback nevertheless calls `self.events.pop_front()`, removing the protected pointer event while reporting `Enqueued::EvictedKey`.

Deterministic repro against the real module, with capacity 3:

1. Enqueue button-down and dequeue it (the client now believes the button is down).
2. Enqueue button-up, then two keys. The release is the sole/newest pointer event at the queue head.
3. Enqueue a third key.
4. `oldest_superseded_transition(false)` returns `None`; line 151 removes the release. Draining yields only keys, so the client remains stuck in the down state.

I mounted this sequence as a temporary host KAT. It failed with `the only queued release was evicted by the key overflow`; the temporary test was then removed. The committed random property does not pin this ordering: its hand-laid case places the release after 63 keys, so `pop_front` happens to remove a key (`tools/atrium-logic-tests/src/input_queue_kats.rs:231-250`).

This contradicts both the stated invariant that the newest pointer event is never evicted by a key and the final-button-state guarantee. At the production capacity, the fallback can select the oldest queued key explicitly rather than blindly selecting the queue head.

## Adversarial checks

1. **Ordering.** Tail-only coalescing and indexed removal preserve the relative order of retained key/key, key/click, and click/click events. The finding above is a loss, not a reorder: it removes an unsuperseded release and can leave the receiver's button state stale.

2. **Motion-only classification.** `last_buttons` is per queue and is updated for every routed pointer report, including coalesced or dropped motion. A fresh route starts at the protocol's neutral bitmap, and draining correctly does not reset source history. Superseded transitions are safe to remove because a later full bitmap remains. The fallback at line 151 breaks that reasoning by removing the one transition that is *not* superseded.

3. **Bounds and I-14.** Production queues are preallocated at 64 entries and never push above that logical bound. Tail coalescing is O(1); the scan and `VecDeque::remove` are O(64), bounded, and do not grow the deque. The longer critical section can increase producer `try_lock` misses, but the work bound is small; moreover current main's later S1-F1 staging makes `DELIVERY` contention retryable rather than immediately destructive. I found no second blocker here.

4. **Pitfall bank.** The split adds no lock, wake, IF-off spin, or route-word publication. Queue state, including `last_buttons`, remains under the single `DELIVERY` critical section. The CP-37 process-context/IF discipline is unchanged by this policy; current main additionally wraps staged flush in `without_interrupts`. No pitfall-bank claim was refuted beyond the queue-state finding.

5. **Kill-switch.** `enqueue_legacy` implements the old full-queue `pop_front` followed by `push_back`. Its outcome increments `RouteEntry::dropped` once exactly when the old implementation did. The key-only KAT compares each outcome and final bytes against both `enqueue_legacy` and an independent drop-oldest reference; it covers lengths below, at, and far above capacity.

6. **Discriminator.** `input_queue_flood` uses real `usb-kbd` and `usb-mouse`, waits for all keyboard reports, allows one second for trailing motion, requires at least 60 mouse reports and a nonzero queue-drop count, then compares all decoded key presses in order. Xtask selects `enqueue_legacy` through `NEO_TEST_EXTRA_FEATURES=atrium-input-queue-legacy`. It is a valid discriminator for pointer-flood key eviction, but it does not exercise the head-transition/key-overflow sequence in this finding.

7. **Named residuals.** At the reviewed merge, producer `try_lock` loss and the uncapped route table were pre-existing, scoped residuals rather than blockers to this policy. Current main has since repaired the direct contention-loss path with bounded per-device staging; the route-table bound remains separate from this change.

8. **Cross-family check.** The `Enqueued::EvictedKey` return value is not evidence of what was removed: the fallback does not inspect the head's class. That mismatch hid the failing state from the dropped counter and existing policy tests, which validate counts rather than the evicted object's class in this arrangement.

## Verification

- RAG health: `healthy` (`ollama=true`, `qdrant=true`); semantic discovery was followed by direct source inspection.
- `cargo test -p atrium-logic-tests input_queue`: 9 passed.
- Temporary deterministic KAT described above: failed against the current implementation; removed after reproduction.
- `NEO_SMP=4 NEO_TEST_ATTEMPTS=1 cargo xtask neo-test input_queue_flood`: all userspace and scenario-kernel builds passed. Execution stopped before QEMU because `/home/neo/limine` is absent in this Windows checkout.

