<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relays 227-230: cross-family reviews and Lane 2b (the always-on relay box)

The coders on the always-on box work in `~/workers/<coder>` (yours alone), on this public repo. RAG is not available
there: use grep and direct reads. **Everything you push is public:** keep Windows user paths, LAN addresses, MACs,
hostnames and email addresses out of what you write.

**Delivery for a review:** commit the RETURN file named in your section on a new branch off `origin/main`, push it,
and reply with the commit SHA. Never push to `main`. Answer as a plain checklist: PASS or FAIL, the enforcing
`file:line`, and a concrete input for any FAIL. Name the head SHA you reviewed.

## Section 227: Chad, review of #13 (a faulting user copy returns EFAULT)

**MODEL** GPT-6.1-Sol, **EFFORT** high: it changes how the kernel's core memory path handles a fault.

`git fetch origin +pull/13/head:pr13 && git diff origin/main...pr13` (head `e5958fe7` when queued; review the head
you fetch and name it).

1. Every syscall path that copies to or from user memory goes through the new fault-fixup copy. List any that still
   use a range check alone.
2. A fault inside the copy returns EFAULT and leaves no lock held, no partial kernel state, and SMAP (AC) cleared.
3. A fault anywhere else in kernel mode still halts as before: the fixup applies only to addresses inside the copy
   routine (check the fixup-table lookup and its bounds).
4. The copy is correct for lengths 0, 1, a page boundary crossing, and a fault on the last byte.
5. The test scenarios cover a bad pointer for read, for write, and a partially valid buffer, and the control build
   shows the old halt.
6. The concurrency-pitfall table in the PR (CP rows) is present, and its claims hold.

RETURN: `Plans/RETURN-2026-11-01-pr13-user-copy-review-chad.md`, branch `codex/pr13-review-chad`.

## Section 228: Janus, review of #6 (OT-90 P3b-1: stop the other CPUs, mask interrupt sources)

**MODEL** GPT-6.1-Sol, **EFFORT** high: new cross-CPU shutdown code (SMP core).

`git fetch origin +pull/6/head:pr6 && git diff origin/main...pr6` (head `3212ad13`).

1. Stopping the other CPUs: an AP that is in an interrupt handler, holding a lock, or in the middle of a TLB
   shootdown when the stop IPI arrives. Does the stopping CPU wait forever, or deadlock on that lock?
2. Each AP acknowledges the stop before the BSP continues, and the wait is bounded with a defined outcome on timeout.
3. Masking interrupt sources (IOAPIC/MSI/LAPIC timer): nothing can still deliver to a stopped CPU, and the order
   (stop first, then mask, or the reverse) can't lose or wedge an interrupt the shutdown path needs.
4. The kill-switch: the feature off is the previous shutdown path, bit for bit.
5. The concurrency-pitfall table (CP rows) is present and its claims hold.

RETURN: `Plans/RETURN-2026-11-01-pr6-stop-cpus-review-janus.md`, branch `codex/pr6-review-janus`.

## Section 229: Jarvis, review of #23 (USB mass storage logic: Bulk-Only Transport + SCSI)

**MODEL** GPT-6-Luna, **EFFORT** high: one contained subsystem, no SMP core, no new syscall.

`git fetch origin +pull/23/head:pr23 && git diff origin/main...pr23` (head `0de729f2`).

1. The CBW and CSW byte layouts match the USB Mass Storage BOT spec, checked against the PR's hand-laid byte
   fixtures (signature, tag, data-transfer length, flags, LUN, CB length).
2. A CSW with a wrong signature, a mismatched tag, or a residue larger than requested is rejected, not trusted.
3. The SCSI READ CAPACITY(10)/(16) and READ(10) parsing bounds-checks every length and block count (a device
   that reports block size 0 or a huge capacity).
4. The reset-recovery sequence on a phase error follows the spec order.
5. The prior-art section (trap mining from Linux usb-storage, read for principle only) is present and holds.

RETURN: `Plans/RETURN-2026-11-01-pr23-msc-review-jarvis.md`, branch `codex/pr23-review-jarvis`.

## Section 230: Dexter, Lane 2b (PNG and JPEG decoding)

**MODEL** DeepSeek, high: a bounded host-only build, the next step after your Lane 2a (PR for
`dexter/l2a-miniz-oxide`).

Read `docs/THINK-2026-10-31-lived-in-os-lanes.md` section 4. Vendor `png` (image-rs/image-png, MIT OR Apache-2.0)
and `zune-jpeg` (MIT OR Apache-2.0 OR Zlib) under `third_party/`, with licence files, upstream URL and commit
recorded (re-check the head and licence on the day). Add one small decode API in `libneo`: bytes in, RGBA and size
out, with a size cap (width x height x 4 bounded) so a hostile image can't exhaust memory. Host KATs: a few images
fetched raw with `git` or `curl`, asserting a hash of the decoded pixels. No kernel change.

- BRANCH: `dexter/l2b-image-decode`, from `origin/dexter/l2a-miniz-oxide` (it builds on your 2a).
- Close: `scripts/worker-close-gate.sh`; push; RETURN `Plans/RETURN-2026-11-01-l2b-image-decode-dexter.md` on
  your branch, naming the head SHA.
- Disk reclaim: after the PM confirms the merge, `cargo clean` in your worktree.
