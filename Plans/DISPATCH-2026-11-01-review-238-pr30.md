<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 238: Jarvis, review of #30 (L3u-2: bind USB mass storage as a read-only block device)

**MODEL** GPT-6-Luna, **EFFORT** high: one driver subsystem, no SMP core, no new syscall.

Same rules as `Plans/DISPATCH-2026-11-01-m910q-reviews-and-l2b.md` (public repo: no personal details; RAG not
available; plain checklist, PASS or FAIL with `file:line` and a concrete input for any FAIL; name the head SHA; RETURN
on a new branch off `origin/main`, push, reply with the commit SHA; never push to `main`).

`git fetch origin +pull/30/head:pr30 && git diff origin/main...pr30` (head `6792c984`). The BOT/SCSI logic it uses
(#23) already passed your review; this is the binding into the USB stack and the block layer.

1. **Read-only and the Zenbook rule:** the block device it registers refuses every write, and is registered behind
   the write guard (`GuardedDevice`); list each path a write could take and the check that stops it.
2. **DMA containment (I-14):** every transfer buffer is a fenced DMA buffer; the xHCI ISR path takes no governance
   lock; a device that never completes a transfer can't hang the caller forever (bounded waits).
3. **Hot-unplug:** removing the stick mid-read returns an error to the reader and unregisters the device; no stale
   handle reaches freed buffers.
4. **Binding:** only interface class 0x08, subclass 0x06 (SCSI), protocol 0x50 (BOT) binds; others are left alone.
5. **The kill-switch:** feature off leaves the interface unbound and the default image unchanged.

RETURN: `Plans/RETURN-2026-11-01-pr30-msc-bind-review-jarvis.md`, branch `codex/pr30-review-jarvis`.
