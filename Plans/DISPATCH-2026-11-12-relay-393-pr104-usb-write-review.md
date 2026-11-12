<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 393 and a Muse read: reviews of #104, OT-100 R2, the USB mass-storage write path (2026-11-12)

**PR:** #104, by Seat F (Claude family). **Head:** `2cf42002`, branch `claude/ot100-r2-msc-write`. Read the code at
this head from the repo, not from a summary.
**Gate:** green at this head, run 43158521142, on all four QEMU shards.
**Design:** `docs/THINK-2026-11-10-ot100-usb-persistence.md`. The rung is §10's R2 row, plus §3, §3.5 and §9.
**Dispatch:** `Plans/DISPATCH-2026-11-10-seat-f-ot100-r2-msc-write.md`.

Answer each item pass or fail, with the enforcing `file:line`. Give a concrete input for any fail.

## Section 393: cross-family code review

**Reviewer:** Janus. **MODEL:** GPT-6.1-Sol. **EFFORT:** high.
Why: this is a new kernel driver path (bulk-OUT DMA, waits bounded by deadlines) and it touches the write guard.

1. **Allowlist.**
   - [ ] `WRITE_PATH_COMMANDS` admits WRITE(10) only as Out, SYNCHRONIZE CACHE(10) only with no data, and MODE SENSE(6)
     only as In (`msc_logic.rs`).
   - [ ] A known opcode in the wrong direction is refused before any byte reaches the pipe.
   - [ ] With `usb-storage-write` off, `ALLOWED_OPCODES` and `command` are byte-for-byte today's.
2. **CBW, CDB and CSW bytes.**
   - [ ] CBW byte 12 is `0x80` only for In.
   - [ ] WRITE(10) never sets FUA.
   - [ ] The CSW residue can't claim more than was sent, and a passed write with a residue is `Failed`.
   - [ ] The host KATs check each of these against hand-laid bytes, not bytes produced by the builders.
3. **Data-out stage.**
   - [ ] Caller data is copied into the fenced bounce buffer before submit, and the device never gets caller memory.
   - [ ] A Release fence comes before the TRB is published (CP-77/78).
   - [ ] A stalled data-out clears the OUT halt and still reads the CSW.
4. **The guard.**
   - [ ] `GuardedDevice::write_probe` returns `None` for a device that isn't armed, so an unarmed stick is never sent
     MODE SENSE, SYNC or WRITE.
   - [ ] Every write still goes through `GuardedDevice::write_block`/`submit`.
   - [ ] No new path reaches `MscDisk::write_block` around the guard.
5. **Waits and locks.**
   - [ ] Every write-path wait has a tick deadline (10 s per chunk, 30 s per sync), with a poll-count backstop.
   - [ ] The only lock held across a wait is the `transport` `SwitchMutex`.
   - [ ] No AI-governance lock is taken (I-14a).
   - [ ] The CP table's rows (CP-1, CP-2/3, CP-18, CP-77/78, CP-80, CP-82) hold.
6. **The 32-bit edge and sense mapping.**
   - [ ] `next_write` refuses a chunk past 32 bits before the CBW is built.
   - [ ] A read past READ(10)'s range is refused, not copied short.
   - [ ] DATA PROTECT latches write-protected, and ILLEGAL REQUEST on SYNC latches flushing off.
7. **The scenario oracle.**
   - [ ] `check_usb_msc_write` checks SCSI commands only after the last `usb_xhci_reset`.
   - [ ] A trace with no reset fails.
   - [ ] After the reset, any opcode outside the kernel's set fails.
   - [ ] The traced WRITE/SYNC/MODE SENSE counts must equal the driver's, and the unmarked stick's driver counts must be
     0/0/0.
   - [ ] Moving the window past the firmware's probe can't hide a kernel-sent command. Show the line that would catch a
     command the kernel sends before its own reset, or say why none can happen.
8. **Prior art.**
   - [ ] The PR's trap-bank table (Linux `unusual_devs` quirks) gives a real handling and a real discriminator for each
     row.

**RETURN:** `Plans/RETURN-2026-11-12-pr104-review-janus.md` on `codex/pr104-review-janus`, pushed.

## The Muse security read (J.c. relays it; Muse can't push)

**Reviewer:** Muse (Gwen), a separate model family. Per CLAUDE.md, the write guard is Claude-authored security code.
Read the files at `2cf42002` from the repo.

1. [ ] No new code path writes, flushes or probes a block device the guard hasn't armed. Check
   `kernel/src/drivers/block/mod.rs` (`GuardedDevice`, `write_probe`, `block::write_probe`) and
   `kernel/src/drivers/block/partition.rs`.
2. [ ] `PartitionDevice` forwards `write_probe` and `write_protected` without widening what the guard admits.
3. [ ] With `usb-storage-write` off, none of the new trait methods or probe code is compiled.
4. [ ] No guard-control feature becomes buildable together with `bare-metal`, `product` or `fb-progress-console`.
5. [ ] Nothing in R2 can reach the Zenbook's internal NVMe. The write path is USB mass storage only, and it is behind
   the guard.

**RETURN:** J.c. pastes or uploads Muse's answer, and the PM banks it as
`Plans/RETURN-2026-11-12-pr104-security-muse.md`.

## Landing

#104 is full-alone. It lands as a batch of one once both reviews pass, or once each FAIL is fixed and re-checked by the
same reviewer. Then R3 is dispatched.
