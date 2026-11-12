<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: PR #103 OT-100 THINK re-check (Dexter, relay 388)

**Dispatch:** `Plans/DISPATCH-2026-11-10-relays-387-388-rechecks.md`. **That file is not on `origin/main` at
`b638b2ab5`** (`git show origin/main:Plans/DISPATCH-2026-11-10-relays-387-388-rechecks.md` fails; the only record of
relay 388 is `Plans/RELAY-QUEUE.md:209`). This review therefore runs the Section 388 task from the queue line and from
relay 385's published checklist (`Plans/DISPATCH-2026-11-10-relay-385-ot100-think-read.md`). The PM should push the
dispatch (and 387's) so the two relays have their authoritative text.
**Reviewed head:** `9a2c97dd45df3c4c71eb0fb315fbc9fb81daffa9` (`claude/ot100-usb-persistence-think`, PR #103), the
author's revision 1.
**Previous read:** `a94a5e60` / `Plans/RETURN-2026-11-10-pr103-ot100-think-read-dexter.md`.
**Review branch:** `dexter/pr103-recheck`, based directly on the reviewed head.
**Design:** `docs/THINK-2026-11-10-ot100-usb-persistence.md` at the reviewed head.

**Verdict: PASS** for the prior FAIL items (the layout, the MSC write path, the Tails date) and for the six
high/medium findings F1-F6. Three medium notes and three small notes below are for R3; none reopens a prior FAIL. The
THINK is now detailed enough to build R2 and R3 from, provided the F2 ownership detail in N1 and the two-entry Limine
detail in N2 are settled in the rung's own dispatch.

## Method and limits

- Read the reviewed THINK (550 lines) and diffed it against the `a94a5e60` text (290 insertions, 133 deletions; only
  the THINK changed in `9a2c97dd`).
- Re-read the code the revision cites: `kernel/src/drivers/usb/msc.rs`, `msc_logic.rs`,
  `kernel/src/drivers/block/mod.rs`, `write_guard_logic.rs`, `gpt_logic.rs`, `partition.rs`,
  `kernel/src/fs/neofs/{inode,cache,mount,format}.rs`, `kernel/src/fs/mod.rs`, `kernel/src/main.rs`,
  `kernel/src/installer.rs`, `kernel/src/limine_file_logic.rs`, `tools/xtask/src/main.rs`, `tools/xtask/src/qmp.rs`,
  `tools/xtask/src/vm_image.rs`, `boot/limine.conf`, and the Zenbook runbook.
- No host test, QEMU run or bare-metal run: the head is documentation only, and no code was changed by this review.
  The existing read-only MSC/guard suites were already run at the previous head; this re-check makes no new code
  claim. No close-gate/4-shard evidence is claimed.
- The PR is based on `5ef9f7f44`, two docs/queue commits behind `b638b2ab5`; the code tree is identical for this
  review. The head's own change is the THINK only (`git show --stat 9a2c97dd`).

## Prior FAIL items

### 1. Item 2(a), the layout and both GPT copies — **PASS**

The revision replaces "verify in R3" with a concrete host-side edit and an independent oracle
(`docs/THINK-2026-11-10-ot100-usb-persistence.md:107-142`):

- If xorriso does not accept a GPT type GUID, xtask edits the type GUID in **both** entry arrays (primary at LBA 2;
  backup before the backup header at the image's last LBA), then both entry-array CRCs, then both header CRCs
  (`:109-115`). That is the complete four-CRC set the prior finding asked for.
- The oracle is a separate GPT parser written from the UEFI byte layout, checking both headers' signatures/sizes/CRCs/
  `my_lba`/`alternate_lba`/disk GUID, both entry arrays byte-identical, the new entry in both, no overlap, and the
  superblock (`:116-127`).
- The guard KAT runs `judge_with_partitions` over the built image and over a "repaired" image whose backup moved to a
  larger disk's last LBA, with CRCs redone (`:128-131`). The code supports this: `gpt_logic::validate` accepts a
  primary whose `alternate <= last_lba` and keeps the usable range clear of both copies (`gpt_logic.rs:170-172`), and
  `partition_claims` drops any entry overlapping `metadata_lbas` (`write_guard_logic.rs:468-486`).

The remaining precision point is N4 (the oracle's summary says "the 32 LBAs before it", but the backup array starts at
`last_lba - 33`).

### 2. Item 2(b), `persist=off` is wired — **PASS**

The revision gives the image two Limine entries with `cmdline: persist=on` / `persist=off`, `timeout: 3` on the stick
image only, a kernel `ExecutableCmdlineRequest`, and a host-tested `persist_logic::parse_cmdline` whose default is
**Off** (`THINK:143-164`). "Try" now flips the current boot read-only through the same switch as §4.2, with no hidden
next-boot state (`:158-160`).

The pieces exist: `boot/limine.conf` has one entry and four `module_path:` lines (`boot/limine.conf:8,17,28-35`), and
`write_limine_conf` is at `tools/xtask/src/main.rs:24453`. The pinned `limine` 0.6.3 does offer
`ExecutableCmdlineRequest` (`Cargo.lock:1062-1066`; `limine-0.6.3/src/request.rs:122`). The default-Off rule is the
right safety default: a boot with no command line never writes the stick.

The remaining implementation trap is N2 (the existing module-injection anchor and the global module count).

### 3. Item 2(c), the boot-device match cannot guess — **PASS**

The fallback is gone. The boot device is identified only when Limine's GPT disk GUID is non-zero and exactly one
registered block device's primary GPT carries it; zero, none, or two or more matches mean **not identified** and no USB
device is mounted read-write (`THINK:296-305`). The identified device must also carry the partition (`:306-312`). A
non-boot USB LogansOS volume is never RW; an installed internal LogansOS keeps today's rule; on the Zenbook (Windows
NVMe only) nothing but the stick's area is writable (`:313-318`). That closes the prior "first LogansOS partition on any
device" hole (`fs/mod.rs:1000-1095`; the product mount is read-write at `main.rs:921`).

### 4. Item 3 / F1, the MSC write path — **PASS**

The revision adds the missing data-out stage in the protocol module, not just in the driver:

- `Data::{None, In, Out}`; `bmCBWFlags` is `0x80` for In and `0x00` for Out and None; the CBW's data length is the
  slice length; `command` stops hard-coding `true` (`THINK:191-205`; today's hard-coded `encode_cbw(..., true, ...)` is
  `msc_logic.rs:391`).
- The Out stage copies the caller's bytes into the fenced 64 KiB bounce and submits an OUT TRB; the 31-byte CBW keeps
  its own `small` buffer. Short writes, stalls (`clear_halt_out` then CSW), errors, and the CSW residue rule for writes
  are each named (`THINK:206-224`).
- `next_write` mirrors `next_read` and refuses a chunk past `u32::MAX` before the CBW; the read path's 32-bit edge gets
  the same explicit refusal (`THINK:225-235`; today `next_read` returns `None` there, `msc_logic.rs:302-311`, and
  `read_block` then copies a short `out` into full caller buffers, `msc.rs:311-356`).
- The allowlist becomes `(opcode, direction)` pairs (`THINK:170-190`), closing the wrong-direction class.
- The named KAT `usb_msc_write_cbw_direction` checks byte 12, the length, the bulk-OUT bytes and the CSW
  (`THINK:229-241`), and CP-77/78 is corrected from "leave" to "introduce → guard" (`THINK:395`).

This is the concrete data-out path the prior read found absent; R2 can be built from §3.2.

### 5. F2, the read-only flip — **PASS, subject to N1**

The design now has the right safety contract (`THINK:324-353`): `writable` becomes an atomic, a fault code is stored,
the drain worker classifies a fatal error, clears `writable` with `Release`, prints the once-only
`persist: volume lost (...); read-only - <n> unsaved blocks dropped` line, does **not** write a superblock, and exits
`Failed`; mutations racing the flip may enter the cache but never commit; `fsync` is to return the fault; the user
notice and the `usb_persist_unplug_midwrite` R3 scenario plus the classifier KAT are named. The current code confirms
the starting point: `writable` is a plain `bool` (`inode.rs:111`), and the worker exits `Failed(2)` on the first drain
error (`cache.rs:710-712`).

N1 is the one implementation seam the revision does not name: the worker only gets `Arc<NeoFsBlockCache>`
(`inode.rs:200`, `cache.rs:691`), and the cache has no writable/fault field (`cache.rs:153-185`), so it cannot clear
`NeoFsState`'s flag as written. The design needs a shared `Arc<MountHealth>` (or the atomics on the cache) passed to
`spawn_writeback_worker`, plus error-variant preservation (see N1). This is an ownership detail, not a different
design; the contract and tests are right.

### 6. Item 8, the Tails date — **PASS**

The Tails row is now release-dated: Tails 7.12, 2026-09-03, with every git host that was tried and refused listed,
7.15.1 noted as current, and 7.11 (2026-08-19) as the fix for Persistent Storage activation
(`THINK:425`). The dispatch asks for licences and dates; a pinned release plus date satisfies that, and the row is
explicit that it is release-dated rather than commit-dated. The two-week cadence is internally consistent with 7.11 →
7.12 → 7.15.1. No commit SHA is claimed, so there is nothing to spot-check against a raw repository.

## Other prior findings

- **F3 (state the mount rule needs) — PASS.** `BlockDevice::write_protected() -> Option<bool>` (default `None`) is
  added (`THINK:242-257`), `PartitionDevice` forwards it alongside `supports_flush`/`flush`
  (`partition.rs:133-139`), and `block::arming_report` exposes `claims`/`table` (`THINK:296-323`). The current
  `BlockDevice` trait has no write-protect query (`block/mod.rs:139-145`) and `writes_allowed` returns only a bool
  (`block/mod.rs:651-657`); `Arming` already carries `claims` and `table` (`write_guard_logic.rs:120-127`). The RW rule
  requiring exactly one `neofs` claim equal to the partition's span is the right strengthening.
- **F4 (probe after the guard) — PASS.** Bind stays INQUIRY/TEST UNIT READY/READ CAPACITY (`msc.rs:383-569`; no
  SYNCHRONIZE CACHE at bind), and the probe moves to the identified, armed boot device from the mount
  (`THINK:274-293`). The unarmed-disk QEMU assertion is the right test (mechanism: N3).
- **F5 (vm-image layouts) — PASS.** `xtask vm-image` always builds without `--persist-mib` and refuses a persistent ISO
  as its boot disk, with a unit test (`THINK:314-319`), keeping `check_vm_layout`, `check_one_disk` and the installer
  invariants.
- **F6 / item 6 (no-area boot) — PASS.** With no area nothing arms, the identified boot device has no partition, no
  other USB volume is RW, and the boot carries on read-only (`THINK:296-323`, `:354-362`); the two-stick and
  same-GUID cases are explicit R3 scenarios. The prior "second LogansOS volume mounted RW" contradiction is gone.
- **Item 7 (kill-switch and R4 tier) — PASS.** R3's off-state is a byte-identical image on the same commit/inputs, with
  xorriso dates pinned first (`THINK:508`); R4 is now full tier because it adds a kthread and a file-write path
  (`:509`). See N5 for the remaining reproducibility detail.
- **Small notes — PASS.** `mod.rs:383-427` corrected (`THINK:30-32`); "read-only as today" now distinguishes the
  no-volume case from the product's RW mount (`:23-26`, `:56-58`); `persist=off` now says "never issues a write", not
  "never calls `writes_allowed()`" (`:158-160`); the 32-bit read edge is fixed in R2 (`:228-235`); the runbook update
  is assigned to R3 (`:548`); the ELF-string proxy is dropped (`:549`); the governance check is a module-path check,
  not a string match on `ARENA` (`:488`).

## R3 notes (none reopens a prior FAIL)

**N1 (medium, F2 reachability and error kinds).** `NeoFsState.writable`/`fault` need a handle the drain worker can
reach. Today the worker is spawned with `Arc<NeoFsBlockCache>` only (`inode.rs:200`; `cache.rs:691`), and
`NeoFsBlockCache` has no such fields (`cache.rs:153-185`). Put the atomics in an `Arc<MountHealth>` held by both, or on
the cache and passed to `spawn_writeback_worker`. Separately, the classifier cannot see the driver's error kind:
`retry_busy_block_io` maps every non-`Unsupported` `BlockError` to `NeoFsError::BlockDevice` (`inode.rs:2814-2828`),
and `flush_device_with_retry` maps every flush error to `NeoFsError::BlockDevice` (`cache.rs:1126-1140`);
`BlockError` has no `Timeout` variant today (`block/mod.rs:84-92`). Since the THINK says "not fatal: none in v1", the
latch may safely treat any drain error as fatal; but the named reason and the classifier KAT need `WriteRefused` /
`DeviceGone` / `Timeout` preserved (a new `NeoFsError` variant or a cache-level `BlockError`). Also,
`fsync_regular` currently returns `Ok(())` when not writable (`inode.rs:972-974`), which contradicts "fsync returns
the fault"; and the nine direct `if !self.writable` checks (`inode.rs:262,343,420,607,665,713,792,895,973`) need the
atomic load, not only the four VFS trait methods the THINK cites (`:327-328`). Concrete input: a host KAT that flips
the shared flag under a running drain and asserts the worker's line and the next mutation's `ReadOnly`, plus a
`fsync` after the flip that returns the fault.

**N2 (medium, the two Limine entries and the module injector).** `inject_module_line` inserts after the **last**
`module_path:` in the whole file (`tools/xtask/src/main.rs:24528-24544`), and `check_boot_module_count` counts every
`module_path:` in the file against `MAX_BOOT_MODULES = 8` (`:24553-24561`, `:8192`). `boot/limine.conf` has four
modules today (`:28-35`). If the second entry is emitted before the dynamic injections, every firmware/model/install
module lands only in the **try** entry; duplicating all four into both entries doubles the count (8 today, over the
limit as soon as one dynamic module is staged). Fix: run all injections on the default entry, then duplicate the
finished entry (with its modules and `cmdline`) to make the try entry, and count modules per entry (or exempt the try
entry). Concrete input: `xtask iso --persist-mib` with `--install`/a model module staged; assert both entries carry the
same module list and `check_boot_module_count` accepts the file.

**N3 (medium, the QEMU mechanisms R2/R3 name).** The R2 test "by QEMU's SCSI trace" (`THINK:507`) is not a facility
the harness has: `tools/xtask/src/qmp.rs` records QMP events and sends only `cont` at start (`qmp.rs:1-15`, `:143`);
there is no SCSI trace collection in `tools/xtask`. Name the mechanism (`-trace usb_msc_*`/`scsi_* -D <file>`, parsed
by xtask) or drop the trace assertion and rely on the host-side image hash. Likewise, the R3 `device_del` mid-write
test (`THINK:348`) needs a post-boot QMP command channel; today's recorder consumes the stream in its own thread, so
`device_del` cannot be sent. Concrete input: a small QMP control thread that sends `device_del` after the guest prints
the write-start line, with the recorder still collecting events.

**N4 (small, the backup entry array's first sector).** `THINK:116` says the oracle reads "the last LBA and the 32 LBAs
before it". The backup entry array is 32 LBAs at `last_lba - 33 .. last_lba - 2`, so a literal last-32 window misses
its first sector. The header at the last LBA carries `partition_entry_lba`; read the array from that field (and assert
it is `last_lba - 33`) rather than assuming a fixed 32-LBA offset.

**N5 (small, byte-identical image reproducibility).** `THINK:508` pins xorriso's **dates** for the R3 off-state. The
GPT disk GUID (and any xorriso-generated volume identifiers) must also be pinned, or the comparison will differ even
with identical dates; xtask already edits the GPT for the type GUID, so pinning the disk GUID in that same host edit is
the natural place. Concrete input: build the no-area image twice from the same commit/inputs and assert the full-file
hashes match, not only the ISO payload.

**N6 (small, the §5 "write to every block device" scenario).** `THINK:376` says the kernel tries a write to every block
device, while §5 attaches the persistent image as the boot stick (`:365-370`). On that stick the persistence partition
is armed, so "every block device" would write the stick too. Target the NVMe only, or attempt a write outside the
claimed extent, and assert the stick image is unchanged as well as the NVMe image.

## Verification

- Reviewed head `9a2c97dd45df3c4c71eb0fb315fbc9fb81daffa9`; `git show --stat` is the THINK only; the PR vs
  `origin/main@b638b2ab5` is `docs/THINK-...` (+550), `docs/OPEN-THREADS.md` (+1) and two base-relative queue lines,
  no code.
- No host/QEMU/bare-metal run; no code changes; no close-gate/4-shard claim. The findings above are source-level
  design/implementation points, not failures of existing code.
