<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: L2, Arthur shows its work (the action log, "What Arthur did", the inspector, memory export)

**Seat B, 2026-10-30.** Dispatch: `Plans/DISPATCH-2026-10-30-lanes-L1-L2-L7.md` (Seat B, L2.1–L2.4). Builds on
L1.3a (#248; THINK `docs/THINK-2026-10-30-l1.3-arthur-actions-prompt.md` D9). This fixes the log's format and who
writes it, because the view (L2.2), the inspector (L2.3) and the audit trail all read it.

## 1. Who writes it

**The WM, alone.** The log lives in the WM-only store (`/neofs/.system/actions.log`, the `system-store` cap, #224),
so Arthur can neither write nor erase its own record. Every entry is made where the decision is: the WM's
`permission` module sees every request, every answer, every lend from the record, every refusal and every
`OP_PERMISSION_DONE` (the action's outcome). Arthur's part is to name the **intent**: the line it acted on, sent with
the request (D3 below). L2.3's revokes and L1.3b's undos are WM actions too, logged the same way.

## 2. The record, `NAL1` (byte oracle: `action-log-core`)

Little-endian, one record after another, nothing between them.

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | magic `NAL1` |
| 4 | 2 | version, 1 |
| 6 | 1 | kind: 1 asked, 2 allowed once, 3 allowed always (recorded), 4 denied, 5 lent from the record, 6 refused, 7 done, 8 grant revoked, 9 undone |
| 7 | 1 | action: 0 none, 1 read, 2 list, 3 write |
| 8 | 8 | sequence number, from 0, no gaps |
| 16 | 8 | wall-clock seconds (`CLOCK_REALTIME`; 0 when unknown; the boot test pins 0 so its bytes are fixed) |
| 24 | 4 | detail: a refusal's reason (1 path, 2 read-only, 3 denied this session), a denial's (0 the user, 4 the lock took the prompt down), or a done's errno magnitude |
| 28 | 2 | path length (≤ 255) |
| 30 | 2 | intent length (≤ 255) |
| 32 | 16 | **prev**: the first 16 bytes of the previous record's BLAKE3; zero for record 0 |
| 48 | 16 | reserved, zero |
| 64 | n | the path, then the intent (UTF-8) |

A record's hash is BLAKE3 over its whole bytes; the next record's `prev` is that hash's first 16 bytes. **Verify**
walks the file: magic, version, reserved zero, lengths in range, sequence contiguous, each `prev` matching. It returns
the record count and the head hash, or the index of the first bad record. The chain makes an edit or a removal in the
middle show at the next record; it doesn't stop the WM itself rewriting the whole file (nothing can, short of an
external anchor; noted for later, e.g. the head hash shown in the inspector).

**Appending.** There is no append flag or seek: the WM opens the log, reads to its end (which also gives it the head
and the next sequence number, verifying as it goes), and writes the record there, then fsyncs. *As built after the
#253 bounce:*
- **The record comes first.** A lend whose record can't be appended isn't made, so no action runs unrecorded.
- **A broken chain** is cut back to its last good record. The bytes cut off are kept in `.system/actions.broken`
  (up to 64 KiB), and a record of kind 6 (detail 0xFFFF_FFFF, "chain broken") goes first, so the break is itself on
  the record. Records after a break can't be vouched for by the chain, which is why they're kept aside, not trusted.
- **Records dropped off the end** while the WM runs are caught against the head it last left (kind 6, detail
  0xFFFF_FFFD). Across a reboot only an anchor outside the file could catch that (the inspector showing the head).
- **Rotation.** Past the cap (1 MiB) the oldest half is dropped behind a **rotation marker** (kind 6, detail
  0xFFFF_FFFE). The marker is the file's first record and stands outside the chain: its `seq` and `prev` are those
  the first kept record carries, so the kept bytes and every later link are unchanged, and `verify` starts its walk
  from the marker.
- **Rewrites** (a repair, a rotation) are 4 KiB writes (the kernel's `MAX_FILE_IO`), then a truncate and an fsync.
- **Also logged:** a derive that fails (kind 6, detail 5) and a lent cap revoked with no done from the client (kind
  7, detail 0xFFFF_FFFF). Non-printable intent bytes are kept as `?`.
- **Not logged:** a request or a done naming another surface. It is refused (`CapRequired`) before its path is
  checked, so it is a protocol error, not an action; the WM's serial log names it (`reason=identity`).
- **While the WM waits for a path or an intent** it serves the pointer and the SAK chords as its session loop does,
  so a Super+Escape pressed meanwhile takes the keyboard, and the prompt that follows keeps it on the WM (Dexter's
  #248 re-check, item 2).

## 3. The intent

**D3.** `OP_PERMISSION`'s reserved `u32` at 52 becomes `intent_len: u16` at 52 (≤ 255) and a reserved `u16` at 54;
the intent follows the path as a second raw message. A zero length (L1.3a's clients) means no intent, so the change
is compatible on the wire. The WM logs it verbatim, printable ASCII only (anything else becomes `?`).

## 4. L2.2–L2.4 in one paragraph each

- **L2.2, "What Arthur did"** is a Lens view the WM draws from the log: newest first, one line per action (time,
  verb, path, the answer, the intent), with **Undo** on a write that has a pre-image (L1.3b). The view is the WM's;
  Arthur can't draw into it.
- **L2.3, the permission inspector** lists the record (`arthur-grants.json`) by app and, for each, the caps the WM
  holds lent right now; **Revoke** removes the record entry and revokes any lent cap, and logs kind 8.
- **L2.4, memory export** writes Arthur's memory as a documented, versioned file the user can take away (its own
  byte oracle); export is a button in the inspector, so the WM, not Arthur, performs it.

## 5. Prior art (design only, no code borrowed)

Checked 2026-10-30.

| Project | What | Take | License | Last commit |
|---|---|---|---|---|
| Certificate Transparency (RFC 6962) | Append-only Merkle log with signed tree heads | The anchor idea for later (publish the head) | IETF RFC (spec) | n/a |
| systemd journal (Forward Secure Sealing) | Sealed append-only log | Sealing is the step past a chain; not now | LGPL-2.1+ (read for principle) | n/a (not cloned) |
| A plain hash chain (git's object chain) | Each entry names its parent's hash | The whole design of §2 | GPL-2.0 (principle only) | n/a |

Writing it ourselves is the right size: the format is 64 bytes plus two strings, and the store is ours.
