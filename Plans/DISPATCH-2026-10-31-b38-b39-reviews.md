<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Cross-family pre-merge reviews for batches 38 and 39 (run by the PC relay runner)

Batch 38: #241, #247, #250, #265, #267. Batch 39: #254 (it conflicts with batch 38 only in the shared scenario
registries). Fetch each PR with `git fetch origin +pull/<N>/head:pr<N>` and review `git diff origin/main...pr<N>`.
Answer as a plain checklist: PASS or FAIL, the enforcing `file:line`, and a concrete input for any FAIL. Every
section's last item: the prior-art section the build needed (CP table for SMP/lock/kill code, survey for an unfamiliar
feature) is present and its claims hold.

**Delivery:** commit the RETURN file named in your section on a new branch named after it, off `origin/main`
(`codex/<slug>-<coder>` or `dexter/<slug>`). Push it and reply with the commit SHA. Never push to main.

## Section 209: Chad, re-check of your own three findings (#241, #265, #267)
**MODEL** GPT-6.1-Sol, **EFFORT** high: a bundle runs on one model, and #267 is shutdown code.
1. **#241 (your #189):** is the whole egress call bounded, including every retry and wait inside it? Is OUTBOUND
   capped, and what does a caller see at the cap?
2. **#265 (your #198):** does the refusal leg now prove nothing is left pending after a refused object quota?
3. **#267 (your #199 item 2):** does the deadline cover the whole clean shutdown? Specifically: a block request stuck
   in the device, `SINK.lock` held by a stalled writer, a worker that never joins, and `fsync_regular` on a dead
   device. When the deadline fires, what state is the disk left in, and does power-off or restart still happen?
4. The prior-art item.

RETURN: `Plans/RETURN-2026-10-31-b38-rechecks-chad.md`.

## Section 210: Janus, review of #250 (OT-92, NeoFS block cache eviction with a dirty throttle, head `f89cc0ca`)
**MODEL** GPT-6.1-Sol, **EFFORT** high: memory and lock code in the filesystem's hot path.
1. Can eviction drop a dirty block, or a block another CPU is reading or writing?
2. Lock order: which locks does eviction and the dirty throttle take, in what order, and can a writer that is
   throttled while holding a lock deadlock against the flusher?
3. Can the throttle hang forever (for example when the device fails, or with the cache full of pinned blocks)?
4. Off mode (`None`): is it the previous behaviour bit for bit?
5. The prior-art item (CP rows).

RETURN: `Plans/RETURN-2026-10-31-ot92-cache-review-janus.md`.

## Section 211: Dexter, security review of #247 (OT-66 K1b, scoped directory caps, head `02b1a35a`)
1. Can a holder of a scoped directory cap reach anything outside its subtree: `..`, symlinks, a mount point inside
   the subtree, a rename or hard link out of it, or an absolute-path syscall that ignores the cap?
2. Can a scoped cap be widened by derive, grant, spawn or IPC?
3. Does Arthur get only his own subtree in image A, and does anything else he held before still work?
4. Is the off mode today's behaviour bit for bit?
5. The prior-art item.

RETURN: `Plans/RETURN-2026-10-31-k1b-scoped-root-review-dexter.md`.

## Section 212: Jarvis, review of #254 (L7.1, a boot-time number from power-on, head `04bddd7a`)
**MODEL** GPT-6.1-Sol, **EFFORT** medium: one subsystem, but it adds a syscall (`SYS_BOOT_TIMES`).
1. Is the number actually measured from power-on (which firmware source, and what if it is missing)?
2. `SYS_BOOT_TIMES`: is the struct byte-oracled by hand, versioned, and copied out safely (length, bad pointer)?
3. Can any process read it, and does it leak anything it shouldn't?
4. Is it in image A, and is the Home row correct when the number is unavailable?
5. The prior-art item.

RETURN: `Plans/RETURN-2026-10-31-l71-boot-time-review-jarvis.md`.
