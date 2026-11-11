<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Seat F, OT-100 R2 — the USB mass-storage write path (2026-11-10)

**Seat:** Seat F (cloud). **Branch:** `claude/ot100-r2-msc-write` off `origin/main`.
**Design (approved):** `docs/THINK-2026-11-10-ot100-usb-persistence.md`, merged as #103. Dexter passed it in relay
388 (`Plans/RETURN-2026-11-10-pr103-recheck-dexter.md`). R2 is the row in §10, with §3 (the write path), §3.5 (the
post-guard probe) and §9 (I-14).

## Build exactly R2's row

Behind feature `usb-storage-write`, which implies `usb-storage`:
- the (opcode, direction) allowlist;
- the bulk-OUT data-out stage (§3.2);
- `next_write`;
- WRITE(10);
- SYNCHRONIZE CACHE(10);
- the MODE SENSE(6) header;
- the `write_protected`, `supports_flush` and `flush` overrides;
- the post-guard probe (§3.5);
- TSC deadlines;
- sense mapping;
- the 32-bit read-edge refusal.

**Nothing from R3** (the area, the mount, the GPT edit) goes in this PR.

## Gate

- **Host KATs** in `msc_logic`, including **`usb_msc_write_cbw_direction`**: CBW byte 12 is `0x00`, the data stage is
  a bulk-OUT of exactly the pattern bytes, and the CSW residue is 0. Each new command's CBW and CDB bytes are checked
  against a hand-laid oracle that is independent of the builder (the byte-oracle rule).
- **The QEMU scenario `usb_msc_write`:**
  - a `usb-storage` disk on `qemu-xhci`, with a sealed scratch sector;
  - write a pattern across the 64-block and 128-block chunk boundaries, flush it and read it back;
  - **xtask hashes the raw image after QEMU exits**;
  - a second, unmarked `usb-storage` disk must refuse writes (`WRITE REFUSED`), and QEMU's SCSI trace must show it
    received no opcode outside today's six.
- **Kill-switch:** with `usb-storage-write` off, the allowlist and `command()` are today's, shown by a KAT on the table
  and on a READ(10) CBW's bytes. `write_block` returns `Unsupported` and `supports_flush` is false.
- The full 4-shard tier and `scripts/worker-close-gate.sh`. The PR body carries the THINK's trap bank (each trap and
  its discriminator), the I-14 statement and the concurrency pitfall-bank table for the touched classes.

## Review

This is a kernel driver, so when it's green the PM queues a **cross-family review** (Codex, GPT-6.1-Sol high). The
change touches the write guard's path, so it also gets a **Muse security read**: the guard is Claude-authored code,
per the CLAUDE.md routing.

## Safety (the standing rules)

No path in R2 may write any device the guard hasn't armed, and the Zenbook's internal NVMe is never written.
Guard-control features stay unbuildable with bare-metal, product and fb-progress-console.

## Close

Commit and push at every checkpoint. Post PM: with the 4-shard run URL. Then R3 is dispatched.
