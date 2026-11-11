<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: OT-100, Tails-style persistence on the boot stick

**Date:** 2026-11-10. **Seat:** F. **Status:** step 1 of `Plans/DISPATCH-2026-11-10-seat-f-ot100-usb-persistence.md`,
**revision 1**. This is the design only. No kernel code changes in this step. Revision 1 answers Dexter's relay-385 read
(`Plans/RETURN-2026-11-10-pr103-ot100-think-read-dexter.md`, FAIL on layout, the MSC write path and the Tails date);
§12 maps each finding to its change. The narrow re-check comes next, then the PM dispatches the rungs.
**PL decision (J.c., 2026-11-10):** "Yes tails style." LogansOS saves to a persistence area on the boot stick itself
and never writes the laptop's internal drive.
**Builds on:**
- the write guard: I-17, `kernel/src/drivers/block/write_guard_logic.rs`, PR #85/#88;
- OT-87 R3's partition rule (`partition_claims`) and R2's NeoFS boot mount (`fs/mod.rs`);
- the read-only USB mass-storage driver, L3u (`kernel/src/drivers/usb/msc.rs`, `msc_logic.rs`);
- the Zenbook runbook (`docs/runbook-baremetal-zenbook-s14.md`).

## 0. The answer in one paragraph

`cargo xtask iso` appends a small, already-formatted NeoFS partition to the boot image. It is a GPT entry of the
LogansOS NeoFS type, and its first block is a sealed NEOFSv1 superblock. Rufus DD-writes the image to the stick as it
does today. The partition is then exactly what OT-87 R3's partition rule already arms, so **the write guard does not
change**. The USB mass-storage driver gains WRITE(10), SYNCHRONIZE CACHE(10) and a write-protect check, behind a new
feature. At boot, when the boot device is identified for certain as that stick and the user didn't pick the "try"
entry, NeoFS mounts the partition read-write as the product volume. If the stick can't be identified, nothing on it is
written. A partition on any other USB stick is never mounted read-write. If the stick fails mid-boot (write-protected,
pulled, a failed flush), the mount flips to read-only and says so. Growing the area to fill the stick, and encrypting
it, are named later steps. Both need decisions this v1 doesn't have to make.

## 1. What exists today

- **The guard** wraps every block device, with no production off switch (`block/mod.rs:705-731`).
  - It arms lazily, on the first write or on `writes_allowed()` (`mod.rs:383-427`: `arming` and `writes_allowed`).
  - It admits a write only inside an extent claimed by a sealed marker, on space no partition table allocates
    (I-17).
  - Under `neofs-partition` it also arms a GPT partition when all five of these hold
    (`write_guard_logic.rs:453-511`):
    1. the GPT validates;
    2. the entry has the LogansOS NeoFS type `EAA7A9D0-75C1-4385-9664-B2A74FC2622D`;
    3. INCOMPLETE (bit 48) is clear;
    4. the partition's first block is a sealed NEOFSv1 superblock;
    5. that volume fits the partition.
  - A claim never covers GPT metadata.
- **The boot ISO** is an xorriso hybrid with a protective MBR, a GPT and an ESP (`tools/xtask/src/main.rs:10637-10658`).
  - On a stick bigger than the image, the GPT's backup header sits at the image's end, not the stick's.
  - `gpt_logic::validate` accepts that: the primary needs `alternate <= last_lba` (`gpt_logic.rs:170-172` says so for
    "our own ISO on a USB stick").
  - The guard leaves the stick's tail past the backup header unallocated (`gpt_extents`). Nothing claims it, though.
- **The USB mass-storage driver is read-only three ways:**
  - the module says so (`msc.rs:2`);
  - the opcode allowlist is `[0x12, 0x00, 0x03, 0x25, 0x9e, 0x28]`, enforced before any byte reaches the device
    (`msc_logic.rs:134`, `:385-390`);
  - `write_block` returns `Unsupported` (`msc.rs:360-363`), and `supports_flush` keeps its default `false`.
- **A writable NeoFS mount requires `supports_flush()`.** Without it, the mount logs "refusing a writable mount: the
  device cannot flush its write cache" (`fs/neofs/mount.rs:62-70`).
  - The boot mount (`boot-neofs`, in `product`) takes the first LogansOS partition on **any** device
    (`fs/mod.rs:1000-1095`).
  - With no volume, the boot carries on and logs why (`:1166-1183`). A volume it does find is mounted **read-write**
    (`main.rs:921`); "read-only as today" in this THINK means only the no-volume case.
- **The MSC protocol module has a data-in stage only.** `msc_logic::command` hard-codes the CBW direction to
  device-to-host (`encode_cbw(tag, data_len, true, …)`, `msc_logic.rs:391`) and runs `pipe.bulk_in(data)` for the data
  stage (`:402-413`). The `Pipe` trait's `bulk_out` carries only the 31-byte CBW today, through the `small` buffer
  (`msc.rs:209-219`). There is no bulk-OUT data stage anywhere.
- **A NeoFS mount can't change its mind.** `NeoFsState.writable` is a plain `bool` set at mount (`fs/neofs/inode.rs:111`),
  and the writeback kthread exits `Failed(2)` on the first drain error (`fs/neofs/cache.rs:710-712`), leaving the
  mount "writable" with no worker.
- **The kernel has no command-line parser.** `boot/limine.conf` has one entry (`:17`); the `limine` crate (0.6.3)
  offers `ExecutableCmdlineRequest`, which nothing requests yet.
- **The boot medium's identity:** `installer::boot_disk_guid()` gives its GPT disk GUID, from Limine's
  executable-file response (`installer.rs:172`). It is only built with `installer`.
- **The installer claim** (OT-87 R4) is the one path past the guard. It refuses the boot disk (`BootDisk`,
  `claim_logic.rs:408-409`) and a device that cannot flush, and it calls an xorriso stick Foreign. Rightly, it is no
  use here.

## 2. The stick layout (dispatch item 1)

### 2.1 Options

| | Where the area lives | Created | Guard change | Cost |
|---|---|---|---|---|
| **A (chosen)** | A GPT partition appended to the image by `xtask iso`, LogansOS NeoFS type, pre-formatted | At image build time | **None.** R3's rule arms it as is. | The image grows by the area's size. The area has a fixed size. |
| B | A GPT partition added at first boot in the stick's free tail, with consent (Tails' flow) | First boot | **Yes:** writing the boot medium's GPT. That relaxes I-17 (metadata never written) and the installer's `BootDisk` refusal. | One bug leaves the stick unbootable. It needs its own relax-and-re-prove claim. |
| C | An extent past the image's end, claimed by a new sealed marker and outside the partition table | First boot (write the marker) | **Yes:** a new marker kind. Also, the first marker write has no claim to arm it (a chicken-and-egg). | Invisible to every other OS, and a fourth marker kind to prove. |
| D | The existing log-sink or whole-disk NeoFS markers | — | — | Doesn't fit: the image occupies offset 0 and 256 MiB, and a table allocates both. |

**A is v1.** It needs no change to the guard, writes no partition table at run time, and creates nothing on the
user's machine. The area exists from the moment the stick is written, empty and sealed. B is how Tails does it (§7)
and the right v2 for "use all the free space". It's named in §10 as a later step, with the guard change it needs.

### 2.2 Layout of the v1 image

```
LBA 0          protective MBR (0xEE)                       } xorriso, as today
LBA 1..        GPT primary header + entry array            }
               ISO 9660 / HFS+ / ESP (the Limine tree)     } as today
P_start        LogansOS NeoFS partition (new)              } xtask: -append_partition, GPT type EAA7A9D0-…
  +0             sealed NEOFSv1 superblock (copy 0)          neofs-mkfs, built on the host
  +1 MiB         superblock copy 1
  +64 MiB        superblock copy 2
  … empty volume …
P_end
image end - 33 GPT backup entry array + header             } xorriso
---- stick end: untouched, never claimed (the guard leaves it unallocated and nothing claims it)
```

- **Size:** 512 MiB in v1, a build parameter (`--persist-mib`). NeoFS's third superblock copy sits at 64 MiB, so
  the floor is just above 64 MiB.
  - The timing ISO was 1,313,275,904 B, so the image grows to about 1.8 GB. That still fits the 30 GB Kingston.
- **How:** `xorriso … -append_partition 3 <type> persist.neofs -appended_part_as_gpt`, on the host, in `xtask iso`.
  - `persist.neofs` comes from `tools/neofs-mkfs`, the same tool as `vm_image`'s data disk.
  - Whether the dev container's xorriso takes a GPT type GUID for an appended partition is checked first in R3
    (build the image, dump it). If it takes only an MBR type byte, xtask edits the image after xorriso: the type
    GUID of partition 3 in **both** entry arrays (primary at LBA 2, backup just before the backup header at the
    image's last LBA), then both entry-array CRCs, then both header CRCs. That is a host-side file edit. **No GPT
    byte is ever written by the kernel** (revision 1: the PM's "one write outside the claimed extent" does not exist
    in option A; the only GPT writer is xtask, on the host, before the image is published).
- **The image oracle (R3), independent of xorriso and of the guard code:** a host check with its own GPT parser,
  written from the UEFI spec's byte layout, reads the built image's LBA 0, LBA 1, LBA 2.., the last LBA and the 32
  LBAs before it, and asserts:
  1. both headers' signatures, sizes, CRC32s, `my_lba`/`alternate_lba` pairs, and the same disk GUID;
  2. both entry arrays byte-identical, with matching array CRCs;
  3. partition 3 present in both, consecutive with the others, type `EAA7A9D0-…`, attributes 0 (INCOMPLETE clear),
     `first_lba` 1 MiB-aligned, `last_lba` before `last_usable`;
  4. no other entry overlaps it, and the ESP and the ISO entries are unchanged from the image without the area;
  5. the partition's first 512 bytes are a sealed NEOFSv1 superblock whose volume fits, checked against a
     hand-laid expectation of its fields (magic, version, block size log2, block count) and a BLAKE3 recomputed
     by the oracle.
  The guard KAT then runs `write_guard_logic::judge_with_partitions` over the same image twice: as built, and with
  the backup copy moved to a larger disk's last LBA and its CRCs redone (what a firmware or Windows "repair" does).
  Both must arm exactly partition 3's span.
- **The R3 rule, checked against this layout:**
  1. The GPT validates, including on a larger stick.
  2. The new entry has the LogansOS type.
  3. INCOMPLETE is clear.
  4. The first block is a sealed superblock (neofs-mkfs seals it).
  5. The volume fits.

  Only that entry arms. The ISO data and the ESP have other types and never arm. The GPT metadata never arms.
- **The Apple Partition Map** that `-hfsplus -apm-block-size 2048` adds:
  - The guard reads only the MBR and the GPT.
  - xorriso maps appended partitions into the APM only with `-appended_part_as_apm`, which we don't pass.
  - The R3 oracle checks that the APM doesn't cover the appended range. If it does, R3 drops `-hfsplus` from the
    persistent image, because nothing here boots through it.

### 2.3 Consent and "Try": a real mechanism

Tails asks before creating the area and asks again, for a passphrase, each boot. v1 creates the area at build time,
so the consent left is per boot: **do we write this time?**
- **The menu.** The persistent image's `limine.conf` gets two entries and `timeout: 3` (the stick image only; today's
  ISO keeps `timeout: 0`):
  - `/LogansOS` (the default): `cmdline: persist=on`;
  - `/LogansOS — try (nothing is saved)`: `cmdline: persist=off`.
  `write_limine_conf` (`xtask main.rs:24452`) learns to emit the second entry with the same modules. The key name is
  checked against the pinned Limine's CONFIG.md in R3 (`cmdline:`; older releases spelled it `kernel_cmdline:`).
- **The kernel side.** R3 adds an `ExecutableCmdlineRequest` and a pure parser, `persist_logic::parse_cmdline`
  (host-tested): it returns `On` only for the exact token `persist=on`; `persist=off`, no token, an unknown value, a
  duplicate or a missing response all give `Off`. **Off is the default.** A boot with no command line never writes
  the stick.
- **What `Off` does.** The boot mount takes the persistence partition read-only (or skips it), and the kernel never
  issues a write to the stick. The guard census may still judge the stick (reads only).
- **The setup menu's "Try" button** acts on the current boot: it calls the same switch as §4.2's read-only flip
  (`persist: switched to read-only by the user`), after a final drain. Nothing has to be stored for the next boot;
  the next boot asks Limine's menu again. That is the honest form of "Try" for v1: no hidden one-shot state.
- **Test** (R3, Dexter's input): boot the try entry with an area present; assert the stick image's hash is unchanged
  after QEMU exits **and** the mount reports read-only.

## 3. The USB mass-storage write path (dispatch item 2)

The design is ours. The traps are from Linux `usb-storage`/`sd` (read for the principle only, GPL-2.0). The
fallback shapes are from Haiku's `usb_disk` (MIT; design borrowed, no code). See §7.

### 3.1 Commands, and the direction each may run in (feature `usb-storage-write`)

| Op | Command | CDB (byte oracle in R2) | Direction | Why |
|---|---|---|---|---|
| `0x2a` | WRITE(10) | `2a 00 LBA[4, BE] 00 LEN[2, BE] 00` | **Out only** | The write. FUA (byte 1 bit 3) stays **0** (Linux `BROKEN_FUA`). |
| `0x35` | SYNCHRONIZE CACHE(10) | `35 00 00000000 00 0000 00` (the whole medium) | None | `flush()`. |
| `0x1a` | MODE SENSE(6) | `1a 08 3f 00 04 00` (DBD, all pages, 4-byte header only) | In | Write-protect: header byte 2 bit 7. |

- The allowlist becomes a table of **(opcode, direction)** pairs. Today's six opcodes keep their direction (In, or None
  for TEST UNIT READY). A CDB with an allowed opcode but the wrong direction is refused like an unknown opcode
  (`ResetReason::NotAllowed`, nothing sent). That closes a class: a WRITE can never be issued with a data-in stage,
  nor a READ with a data-out stage.
- **Not added:** WRITE(16), READ(16), PREVENT/ALLOW MEDIUM REMOVAL, START STOP UNIT, WRITE SAME. The allowlist doc's
  warning still holds for them.
- **MODE SENSE asks for the 4-byte header only.** Linux's `sd` does the same for USB: devices that crash on page
  `0x3f` with a large allocation length are why it has `NO_WP_DETECT` and the "use 192 bytes" heuristics. A stall or
  a failure reads as "write-protect unknown"; the first WRITE's DATA PROTECT sense is then the fallback (Haiku's
  shape).
- **The feature off** keeps today's six-opcode list, today's `command()` signature and today's `Unsupported`, bit for
  bit (§10, R2's kill-switch).

### 3.2 The data-out stage (revision 1, Dexter's F1)

`msc_logic` gains a direction and a bulk-OUT data stage. Sketch (R2 owns the code):

```rust
pub enum Data<'a> { None, In(&'a mut [u8]), Out(&'a [u8]) }

pub trait Pipe {
    fn bulk_out(&mut self, data: &[u8]) -> Xfer;      // CBW (today) and, new, the data-out stage
    fn bulk_in(&mut self, buf: &mut [u8]) -> Xfer;
    fn clear_halt_in(&mut self) -> bool;
    fn clear_halt_out(&mut self) -> bool;
    fn mass_storage_reset(&mut self) -> bool;
}

pub fn command(pipe: &mut impl Pipe, tag: u32, lun: u8, cdb: &[u8], data: Data<'_>) -> Outcome
```

- **CBW.** `dCBWDataTransferLength` = the slice's length; `bmCBWFlags` = `0x80` for `In`, **`0x00` for `Out` and
  `None`** (BOT §5.1). `encode_cbw` already takes the direction as a parameter; `command` stops hard-coding `true`.
- **Data stage, `Out`.** `pipe.bulk_out(data)`. In the driver this is a new path: the caller's bytes are first copied
  into the fenced 64 KiB `bounce` buffer, and the OUT Normal TRB points at the bounce's `FencedDmaAddr`, never at the
  caller's memory (§9 c). The 31-byte CBW keeps its own `small` buffer.
  - `Done(n)` with `n == len`: go to the CSW.
  - `Done(n)` with `n < len`: a short write; the CSW still follows, and its residue must equal `len - n`, else
    `ResidueMismatch` → reset recovery.
  - `Stall`: BOT §6.7.3 (host expects to send, device stalls bulk-out): `clear_halt_out`, then read the CSW. If the
    clear fails, reset recovery.
  - `Error`: reset recovery (`ResetReason::Data`).
- **CSW** (unchanged rules, now applied to writes): signature `USBS`, tag equal to the CBW's, then:
  - status 0 with residue 0: `Passed { len }`;
  - status 0 with a residue: for a **write** this counts as a failure (`Failed`), printed with the residue; a stick
    that reports a bogus residue on writes that read back correctly is then a quirk to add (Linux `IGNORE_RESIDUE`);
  - status 1: `Failed` → REQUEST SENSE (§3.4);
  - status 2, or an invalid CSW: reset recovery.
- **Chunking.** A new pure helper, `next_write(lba, blocks, max_blocks) -> Result<(u32, u16), Range>`, mirrors
  `next_read` and **refuses before the CBW** any chunk whose last block is past `u32::MAX` (`Err(Range)`, and
  `write_block` returns `Err(BlockError::OutOfRange)` with a line). The read path's 32-bit edge gets the same explicit
  refusal: today `next_read` returns `None` there and `read_block` copies a short buffer into full caller buffers
  (`msc.rs:311-356`), which would panic. Unreachable for a 512 MiB area, but R2 fixes it rather than carry it.
- **Host KATs (R2), with hand-laid bytes:**
  - **`usb_msc_write_cbw_direction`** (Dexter's): a recording `Pipe` runs a WRITE(10) of a known pattern and asserts
    (i) CBW byte 12 (`bmCBWFlags`) is `0x00`, not `0x80`; (ii) the CBW's bytes 8-11 are the pattern length, LE;
    (iii) the data stage is one bulk-OUT of exactly the pattern; (iv) a CSW with residue 0 gives `Passed`.
  - a READ(10) through the new signature produces today's CBW bytes exactly (no regression);
  - wrong-direction refusals (WRITE as `In`, READ as `Out`) send nothing;
  - the stall-on-OUT path clears the OUT halt, then reads the CSW;
  - `next_write` at `u32::MAX - 1` with 2 blocks is refused.
  - The existing KAT that WRITE(10) is refused (`tools/usb-msc-logic-tests/src/lib.rs:560-563`) stays, under the
    feature off.

### 3.3 `write_block`, `supports_flush`, `flush`, `write_protected`

- **`write_block(lba, buffers)`:** copy into the bounce, `command(… Data::Out …)` per chunk under the `transport`
  mutex (CP-18), check each `Outcome`.
- **The maximum transfer stays 64 KiB (128 × 512 B).** If R5 shows big writes failing and small ones passing (Linux
  `MAX_SECTORS_64`), the chunk drops to 64 blocks; F10 tells which.
- **`MscDisk` overrides `supports_flush` and `flush`** (today they're the trait defaults, `block/mod.rs:139-145`).
  `PartitionDevice` already forwards both (`block/partition.rs:133-139`); R2's KAT checks the forwarding.
  - `supports_flush`: true only after the write probe's SYNCHRONIZE CACHE succeeded (§3.5). Until then it's false and
    the NeoFS writable mount refuses, as today.
  - `flush`: SYNCHRONIZE CACHE(10). ILLEGAL REQUEST (sense key 5, ASC 0x20) latches `sync_unsupported` (Haiku's
    `sync_support = 0`). Any other failure retries twice, then `Err`.
- **A new trait method, `write_protected() -> Option<bool>`** (default `None`). `MscDisk` returns the probe's
  answer, or `Some(true)` once a DATA PROTECT sense is seen. `PartitionDevice` forwards it. Dexter's F3: the mount
  rule needs it to be observable.

### 3.4 Errors and sense

- **Status 1 (FAIL):** REQUEST SENSE, then:
  - UNIT ATTENTION (6): retry once;
  - NOT READY (2): retry after 250 ms, at most 4 times;
  - DATA PROTECT (7): latch `write_protected`, `Err(WriteRefused)`;
  - MEDIUM ERROR (3) or HARDWARE ERROR (4): `Err(Io)` with the LBA.
- **0-length CSW:** read it again once (Linux `transport.c`'s case).
- **The stick pulled:** the hot-unplug path sets `gone` (`msc.rs:572-610`); a write in flight returns
  `Err(DeviceGone)`. `RETIRED_DISKS` keeps the device memory alive, so nothing frees a buffer the controller may
  still write.
- **Waits are counted in time, not spins** (CP-80). The write path's waits use a calibrated-TSC deadline: 10 s for a
  WRITE chunk, 30 s for SYNCHRONIZE CACHE. A deadline that passes is a transport error → reset recovery, and
  `Err(Timeout)` after one retry. The read path's spin count is left as it is.
- **Which errors are fatal to the mount** is decided in NeoFS (§4.2), not here: the driver reports, NeoFS decides.

### 3.5 The write probe runs after the guard, never at bind (revision 1, Dexter's F4)

The first draft probed at bind, which would have sent SYNCHRONIZE CACHE to every USB mass-storage device, foreign
ones included, before any guard judgement. Now:
- **Bind is unchanged:** today's INQUIRY / TEST UNIT READY / READ CAPACITY only.
- The probe runs **once, from the persistence mount (§4), only on the identified boot device, and only after
  `writes_allowed()` is true for it**:
  1. MODE SENSE(6), header only, for `write_protected`;
  2. TEST UNIT READY;
  3. one SYNCHRONIZE CACHE(10): success sets `supports_flush`; ILLEGAL REQUEST latches `sync_unsupported`;
  4. one line: `usb-msc: write probe dev=<name> wp=<yes|no|unknown> sync=<ok|unsupported|failed:<k/asc/ascq>>
     max_blocks=<n>`.
- A device the guard hasn't armed never receives MODE SENSE, SYNCHRONIZE CACHE or WRITE. R2's QEMU scenario attaches a
  second, unarmed `usb-storage` disk and asserts (from QEMU's trace of SCSI commands on that drive) that it saw no
  opcode outside today's read-only six.

**A stick that lies about its cache** (Linux `ALWAYS_SYNC`, `WRITE_CACHE`) gets the strict v1 rule anyway: every NeoFS
commit point flushes. A stick with no SYNCHRONIZE CACHE gets no persistence in v1: read-only, with the line saying
why. FUA writes as a substitute are a named later option (§10).

## 4. NeoFS on the area (dispatch item 3)

### 4.1 Which volume, and when it's writable

Under a new `usb-persistence` feature (it implies `usb-storage-write` and `neofs-partition`):
1. **Identify the boot device, without guessing** (revision 1, Dexter's layout (c) and F6).
   - `boot_disk_guid()` moves from behind `installer` into a small shared module (same Limine request).
   - The boot device is identified only if the GUID is non-zero **and exactly one** registered block device's
     primary GPT carries that disk GUID. Zero, none, or two or more (two sticks written from the same image share
     the GUID) all mean **not identified**.
   - Not identified: no USB device is mounted read-write. The line is `persist: boot device not identified
     (guid=<g|none>, matches=<n>); nothing on USB is written`.
2. **The read-write mount**, all of:
   - the boot device is identified and is the device carrying the partition;
   - `persist=on` (§2.3);
   - `block::arming_report(dev)` (new, read-only, F3) says the armed claims are exactly one: `marker=neofs`, extent
     equal to the partition's span. Any extra claim on the device (a stray sealed log-sink header, a scratch sector)
     refuses the RW mount;
   - the write probe (§3.5) gave `wp=no` or `unknown`, and `sync=ok`.
   Anything else: the partition mounts read-only, with one line naming the first condition that failed.
3. **Every other volume.** Under `usb-persistence`, a LogansOS partition on a `usb-storage` device that is **not** the
   identified boot device is never mounted read-write. LogansOS volumes on internal disks (an installed system) keep
   today's selection, so an installed LogansOS still mounts its own volume read-write. On the Zenbook there is none
   (Windows only), so the statement there is unconditional: nothing but the boot stick's area is ever writable.
4. **`xtask vm-image` always builds without `--persist-mib`**, and refuses a persistent ISO as its boot disk (F5),
   so `check_vm_layout`, `check_one_disk` and the installer's invariants keep holding. A unit test pins that refusal.

`block::arming_report(dev) -> Option<ArmingReport { claims: Vec<Claim>, table: &'static str }>` forces the lazy
judgement (as `writes_allowed()` does) and returns a copy. It is what the census (§5) prints.

### 4.2 Flipping to read-only (revision 1, Dexter's F2)

- `NeoFsState.writable: bool` becomes an `AtomicBool`, plus `fault: AtomicU8` (the reason). `is_writable()` loads
  with `Acquire`; every mutation entry point already calls it (`inode.rs:1941`, `:2042`, `:2072`, `:2106`;
  `mount.rs:112`, `:142`, `:165`), so they all start refusing with `ReadOnly` the moment the flag clears.
- **Who flips it, and on what.** The drain worker classifies the error from `drain_worker_cycle` / `flush`:
  - **fatal:** `WriteRefused` (DATA PROTECT or the guard), `DeviceGone`, `Timeout` after the driver's retry, or a
    flush `Err` after its retries;
  - **not fatal:** none in v1. Transient conditions (UNIT ATTENTION, NOT READY) are retried inside the driver and
    never reach NeoFS.
  On a fatal error the worker, in order: stores `fault`, clears `writable` (`Release`), counts the dirty blocks it
  is dropping, prints once `persist: volume lost (<reason>); read-only - <n> unsaved blocks dropped`, raises the
  user notice, and exits `Failed`. It does **not** write a superblock. The stick keeps the last committed generation
  (NeoFS's copy-on-write commit order: data, inodes, directories and the free map first, superblocks only after a
  flush, `commit_logic.rs:1-22`, `:68-80`), so the next boot mounts that generation.
- **A mutation racing the flip** may land in the cache after the worker checked. It is never committed: the worker is
  gone, and the next mutation or `fsync` returns `ReadOnly`. `fsync` on a read-only-flipped mount returns the fault,
  never `Ok`.
- **What the user sees.** A desktop notice (and Arthur's status line): "Your changes since <time> could not be saved:
  <the stick was removed | the stick is write-protected | the stick stopped responding>. LogansOS is now read-only
  until you restart." The kernel side is a fault code the shell reads with the existing filesystem status call; R3
  names the call (it adds a field, not a syscall, if one fits).
- **The same switch serves "Try"** (§2.3): a user switch drains first (a clean flip), then clears `writable` with
  `fault=user`.
- **Test** (R3, Dexter's `usb_persist_unplug_midwrite`): boot read-write, start a multi-block write, QMP `device_del`
  the stick, then (a) attempt a second write and an `fsync`; (b) assert every mutation returns `ReadOnly` (or the
  device error) and `is_writable()` is false; (c) assert the `volume lost` line printed exactly once; (d) re-attach
  and reboot: the volume mounts at the last committed generation. A host KAT drives the classifier with DATA
  PROTECT, `DeviceGone`, a flush `Err` and a timeout, and asserts the same latch.

### 4.3 What lives there, and the missing area

- Whatever the product writes to its NeoFS volume today, nothing new: Arthur's memory (OT-17), settings, the user's
  files and, from R4, the boot log.
- **Missing area** (an old image, a Rufus ISO-mode write, a corrupt superblock): nothing arms, the identified boot
  device has no partition, and under rule 3 no other USB volume is written. The boot carries on read-only. R3 boots
  with the area absent and with it corrupt, plus Dexter's F6 case (a second LogansOS stick attached): nothing is
  mounted read-write.

## 5. Safety: the internal NVMe is never written (dispatch item 4)

- **The guard is unchanged.** On the Zenbook's internal NVMe:
  - a valid Windows GPT has no LogansOS-typed entry, so `partition_claims` yields nothing (`windows_gpt_disk_never_arms`);
  - every other marker's extent lies inside a Windows partition or the table's own space, so it is dropped.

  This design adds no marker, no rule and no claim.
- **The mount's choice** (§4) only picks among already-armed extents. It can't arm one.
- **Proof, by scenario.** R3's QEMU scenario attaches two disks:
  - the persistent image as a `usb-storage` device on `qemu-xhci`;
  - an NVMe disk carrying a Windows-shaped GPT: ESP, MSR, a basic-data partition, recovery, and a planted
    NEOFSv1 superblock inside the data partition.

  The kernel tries a write to every block device. The pass condition is the `WRITE REFUSED … table=gpt` line for
  the NVMe, plus a host-side hash of the NVMe image unchanged after QEMU exits.
- **Proof, by census.** A one-line-per-device census at boot prints every device's arming:
  `persist: guard census device=<name> driver=<d> armed=<extent|none> table=<t>`, from `block::arming_report`
  (§4.1). This is new capture (the rule: capture before you need it). It means the Zenbook photo shows positively
  that nothing on the NVMe is armed.
  - `arming_report` judges with reads only, in thread context at boot (CP-82, §5.1).
- **The guard-control features stay unbuildable** with `bare-metal`, `product` and `fb-progress-console`, as
  today. `usb-persistence` doesn't change xtask's guard-off refusal.

### 5.1 Concurrency pitfall-bank consultation (`docs/concurrency-pitfalls.md`, CP-1…CP-112)

| CP | Trap | This design | Discriminator |
|---|---|---|---|
| CP-1 | Lock-order cycle | **Guard.** The order is:<br>1. NeoFS `drain`;<br>2. cache `inner`, released before any block call;<br>3. `GuardedDevice` arming (`Once`);<br>4. `MscDisk.transport`;<br>5. xHCI `EVENT` / `EP_ROUTES`.<br>No path takes them in reverse. The cache never calls the device with `inner` held. | Lockdep-style debug assert in R2/R3 builds; the R3 two-boot scenario under `--smp 4`. |
| CP-2 / CP-3 | Spin-waiting IF-off, or holding an IRQ-safe lock across an unbounded wait | **Guard.** The MSC wait polls with `transport`, a `SwitchMutex`, held, never an IRQ-safe lock. `EVENT` is taken per poll and released. No flush from IF=0. | Debug assert that `flush`/`write_block` are entered with IF=1. |
| CP-12 | ABA / slot reuse on unplug and re-plug | **Leave.** A re-plugged stick registers as a new device, and the old one stays in `RETIRED_DISKS`. NeoFS holds the old `Arc` and sees `gone`. | R3 unplug scenario: re-plug, and the new device is not silently re-mounted read-write. |
| CP-18 | A single-in-flight device whose submit, wait and complete aren't serialized: a foreground reader against a writeback worker | **Guard.** This is exactly the case: the drain kthread writes while Arthur reads. The `transport` mutex covers the whole command, CBW to CSW, for reads and writes alike. | R3: concurrent read and writeback stress on `usb-storage`; CSW tag mismatches are counted, and must be 0. |
| CP-37 / CP-71 | A log path taking a sink lock at IF=0 or under another lock | **Guard.** R4's log file is written by a kthread from an in-memory ring, never from `serial_println!`'s caller. | R4: log under an interrupt storm (the I-14 scenario). |
| CP-77 / CP-78 | DMA grant order, and the descriptor OWN/doorbell order | **Introduce → guard** (revision 1: the first draft wrongly said "leave"; today there is no bulk-OUT data TRB, only the CBW). The OUT data stage is new: fill the bounce, then a release fence, then write the Normal TRB with its cycle bit last, then the doorbell; the device never sees a TRB whose buffer isn't filled. | R2's write-then-read-back across chunk boundaries, plus the host-side image hash: a TRB published before its data shows as wrong bytes on the image. |
| (new seam, no CP entry yet) | The read-only flip: `writable` cleared by the drain worker while mutations run on other CPUs | **Introduce → guard.** `AtomicBool` with `Release` on the flip and `Acquire` on every check; a racing mutation may enter the cache but is never committed (§4.2). R3 adds the entry to the bank. | `usb_persist_unplug_midwrite` under `--smp 4`; the classifier KAT. |
| CP-80 | A wait counted in spins, not time | **Introduce-then-guard.** The write and sync waits use TSC deadlines (§3.3). | R2 KAT on the deadline arithmetic; F10 on the Zenbook. |
| CP-82 | A lazy `Once` whose initializer blocks | **Guard.** Arming judges with MSC reads. The census and the mount force it in boot-thread context, before the drain kthread's first write. | Assert in the drain worker that the arming is already resolved. |
| CP-91 / CP-111 | The last `Arc` dropped under a global lock | **Leave.** Retire never frees, so NeoFS's drop of the device on `gone` happens outside the registry lock. | R3 unplug scenario under `--smp 4`. |

## 6. Encryption (dispatch item 5)

- **Out of scope for v1.** Tails encrypts its Persistent Storage (LUKS2 with Argon2id since Tails 5.13) and unlocks
  it with a passphrase on the Welcome Screen.
- **What v1 stores in the clear on the stick:** everything on the volume: settings, the user's files, the boot log,
  and Arthur's memory.
  - OT-17's per-category ChaCha20-Poly1305 keys live beside the `.mem` files. OT-17 itself says its payoff is
    "NOT stolen-disk confidentiality". So a lost stick discloses Arthur's memory too.
- **Rule for v1:** network secrets (Wi-Fi PSKs) are not written to the persistence volume until encryption lands.
  The setup and network pages keep them in RAM for the boot. R3's dispatch carries that as a checklist item.
- **The later step:** a volume key under a passphrase KDF, unlocked before the mount. It's named in §10 with what it
  needs:
  - a KDF choice (Argon2id, as Tails);
  - an on-disk header;
  - a byte oracle;
  - the unlock UI.

## 7. Prior-art survey (unfamiliar feature)

Checked 2026-11-10 from this container. GitHub was read with a shallow `git fetch`; Tails' and Redox's own hosts
answered with a proxy 403 and could not be checked.

| Project | What it does | Licence | Last commit (checked) | Fit | What we take |
|---|---|---|---|---|---|
| **Tails Persistent Storage** (`tps`, Welcome Screen; tails.net design pages) | A LUKS partition `TailsData` created **on the running stick** in its free space, after the system partition. ext4 inside, unlocked by passphrase each boot, with per-feature bind mounts. | GPL-3.0+ (Tails) | **Release-dated, not commit-dated** (revision 1). No Tails git host was reachable: `gitlab.tails.boum.org`, `git.tails.net`, `0xacab.org`, `salsa.debian.org` and `repo.or.cz` all refused the CONNECT (403), and there is no official GitHub mirror. Pinned instead to the **Tails 7.12 release, 2026-09-03** (the Tor Project blog's "New Release: Tails 7.12" notes); tails.net's home page lists 7.15.1 as current (undated in what was reachable). Alive: a release every two weeks since 7.12. Its 7.11 (2026-08-19) fixed Persistent Storage activation on some computers. | Design only, never code. | The shape: a separate partition after the system one, consent per boot, the system untouched, opt-in per feature. **Not taken for v1:** creating it at run time (§2.1 B), and the encryption (§6). |
| Debian `live-boot` persistence (`persistence` label + `persistence.conf`); Ubuntu `casper-rw` / `writable` | A labelled partition or image file overlaid on the live root | GPL-3.0 (live-boot) / GPL (casper) | Upstream on salsa.debian.org, not checked. GitHub mirror `Lernstick/live-boot` last pushed 2017-09-07. | Principle only. | Label-found persistence and opt-in by boot flag, so the same idea as our `persist=off`. The overlay model is not taken: our root isn't a union filesystem. |
| Ventoy persistence plugin | A persistence **image file** inside its data partition, mapped per distro | GPL-3.0 (`COPYING`) | `74cff34240dc`, 2026-10-22 | No fit. | Nothing: it needs a filesystem we would read and write first. |
| Linux `usb-storage` + `sd` | The reference BOT and SCSI host stack. The quirk table is `unusual_devs.h` / `usb_usual.h` | GPL-2.0 (`transport.c`: GPL-2.0+) | `b3dfbde62cf1`, 2026-11-09 | Principle only, never copied. | The trap list in §8.1, the 0-length CSW retry, the reset recovery order, and the MODE SENSE caution. |
| Haiku `usb_disk` | A BOT host driver with WRITE(10/12/16), SYNCHRONIZE CACHE with a support countdown, MODE SENSE write-protect, and `MAX_IO_BLOCKS` 256 | MIT (`usb_disk.cpp` header) | `f4e3fd5e6ab9`, 2026-11-09 | Good. Same problem, permissive licence. | **Design borrowed, no code vendored:** latching sync unsupported on ILLEGAL REQUEST, DATA PROTECT → write-protected, and checking WP before writing. |
| TinyUSB `msc_host` | An embedded MSC host with READ10/WRITE10 | MIT (`LICENSE`) | `26adc208a2bf`, 2026-11-09 | Partial: no SYNC CACHE, no WP. | The WRITE10 CBW layout, as a second reference for R2's byte oracle; we build its bytes from SBC-3, not from this code. |
| Redox `usbscsid` | A Rust userspace BOT and SCSI driver | MIT | Not checked: `gitlab.redox-os.org` refused (403) | Would fit in shape. | Not read. If the cross-family reader can reach it, it's worth a look for R2. |

**Decision:** write the MSC write path ourselves, in the existing driver's shape, using the traps above. No code is
vendored. Nothing GPL is copied.

## 8. Failure modes and discriminators (Zenbook, blind hardware)

The Zenbook has no serial port. The evidence is the fb progress console (photos), and from R4 the boot log on the
persistence volume, which the *next* boot can show. Every line below is printed in the boot console.

### 8.1 Trap-mining: USB mass-storage writes (seeded into the bank)

| Trap (source) | What it does | Our handling |
|---|---|---|
| `NO_WP_DETECT`; MODE SENSE page 0x3f with a big length hangs some sticks (Linux `sd`) | A hang or stall at the probe | A 4-byte header only. A stall reads as `wp=unknown`. |
| A stick that lies about write-protect, or reports no WP but returns DATA PROTECT | Writes fail | DATA PROTECT → read-only, with a line (F2). |
| `ALWAYS_SYNC` / `WRITE_CACHE` (lies about its cache) | Data lost on unplug despite a "clean" write | Every NeoFS commit point flushes (§3.5). |
| SYNCHRONIZE CACHE unsupported (ILLEGAL REQUEST) / `SENSE_AFTER_SYNC` | Flush fails, or reports late | Latch unsupported → no persistence in v1 (F3). REQUEST SENSE after any failed CSW. |
| `BROKEN_FUA` | FUA writes fail | Never set FUA. |
| 0-length CSW (extra ZLP after data) | A spurious transport error | Re-read the CSW once. |
| `IGNORE_RESIDUE` (bogus residue) | False short-write errors | For a write, the residue must be 0 with status OK; any other residue counts as a failure and is printed. If R5 shows a stick reporting a bogus residue on writes that read back correctly, that's a quirk to add, with its line. |
| `MAX_SECTORS_64` / `MAX_SECTORS_240` | Big transfers stall or time out | 64 KiB chunks; the line in F10 tells chunk size from timing. |
| `INITIAL_READ10`, and the first command after reset failing | Bind or first-write failure | UNIT ATTENTION retry once (§3.4). |
| `BULK_IGNORE_TAG` | CSW tag mismatch | A tag mismatch is a transport error → reset recovery. Counted (CP-18 row). |
| Phase error / stalled bulk-out mid-data | A hung transfer | BOT §5.3.4 reset recovery, then one retry. |
| The stick pulled mid-write | Partial writes | `gone` → `DeviceGone`, read-only. NeoFS commit order (§4.2). |
| Rufus in **ISO mode**, not DD | Rufus writes its own FAT layout: no GPT from our image, no LogansOS partition | Nothing arms. The boot is read-only and says so (F6). |
| Firmware, or Windows on the PL's PC, "repairs" the GPT by moving the backup to the stick's end | The GPT changes under us | The primary still validates (new alternate and CRCs). The census prints `table=gpt` vs `gpt-backup` (F12). |

### 8.2 The Zenbook run's bank

| # | Mode | What the screen shows (discriminator) | Next action |
|---|---|---|---|
| F1 | MODE SENSE stalls | `usb-msc: write probe wp=unknown …`, with persistence still on | None needed. Logged so a later hang is not blamed on it. |
| F2 | Write-protected stick | `wp=yes`, or `usb-msc: DATA PROTECT lba=… ; persistence read-only` + `persist: volume lost (write-protected); read-only - <n> unsaved blocks dropped` if it happens after mount | Check the stick's switch, or swap the stick. |
| F3 | SYNC unsupported | `sync=unsupported` + `persist: no durable flush; booting read-only` | A different stick, or the FUA option (§10). |
| F4 | WRITE fails with sense | `usb-msc: WRITE(10) lba=… blocks=… FAIL sense=<k>/<asc>/<ascq>` | Map the sense to a quirk. |
| F5 | Phase error / reset loop | `usb-msc: reset recovery #n after <cmd> lba=…`, counted | More than 3 in a boot: drop the chunk to 64 blocks (F10). |
| F6 | No persistence partition | `persist: no LogansOS partition on boot disk guid=… (table=…)` | Re-write the stick with Rufus **DD** mode. |
| F7 | Boot device not identified | `persist: boot device not identified (guid=<g|none>, matches=<n>); nothing on USB is written`, then every block device's GPT disk GUID | No USB write happens. If `guid=none`, Limine reported no GPT disk UUID; if `matches=0`, the stick's GPT isn't read as expected; if `matches=2+`, two sticks from one image are attached. |
| F8 | Mount fails | `boot-neofs: mount failed <err>; boot continues` + `persist: read-only` | The superblock/commit diagnostics already printed by NeoFS. |
| F9 | **Anything armed on the internal NVMe** | The census line `device=nvme… armed=<extent>` | **Stop the run.** It must read `armed=none table=gpt`. Any other value is a guard defect, which was proven impossible, so this is the line that would show it. |
| F10 | Timeouts on large writes | `usb-msc: WRITE(10) deadline 10000 ms passed lba=… blocks=128` | Retry with 64-block chunks: this tells `MAX_SECTORS_64` from a dead stick. |
| F11 | Stick pulled or dropped off the bus | `usb-msc: device gone during WRITE lba=…` + `persist: volume lost (device gone); read-only - <n> unsaved blocks dropped`, once | Expected if the PL pulls it. Otherwise a hub or power issue. |
| F12 | GPT rewritten by firmware or Windows | The census shows `table=gpt-backup`, or the arming extent differs from the build's | Fine if the extent equals the partition's span. |
| F13 | Writes "succeed" but don't persist | Boot 2 prints `persist: boot-count=<n>`. It must be boot 1's plus one, with boot 1's last-shutdown flag | A count that didn't move means the flush lies. That's `ALWAYS_SYNC` territory; fall back to the §10 FUA option. |

The run is two boots. Boot 1 writes a file and the boot counter, and the boot log shows the census. Boot 2 shows boot
1's counter and boot 1's log. Each outcome above maps to one line, so a failure localises from the photos.

## 9. I-14 fault containment (connectivity/device driver rule)

- **(a) Lock isolation.**
  - The write path holds `MscDisk.transport` and takes xHCI's `EVENT` and `EP_ROUTES` per poll, exactly as reads
    do.
  - It never takes `INFLIGHT`, `RECENCY`, `ARENA`, the reservation or `TABLE`. Completion is polled in the caller's
    thread. The ISR drains the same event ring through the same locks.
  - `msc.rs:11-17`'s I-14 note stays true. R2 adds a check that `msc.rs` and `msc_logic.rs` use no governance module (a path check on `crate::ai::` and the named governance statics, not a string match on `ARENA`, which `dma_arena` would trip), plus the
    existing runtime no-AI-lock-in-IRQ assertion.
- **(b) Liveness.**
  - A wedged stick blocks only the thread holding `transport`: the NeoFS drain worker, or a reader.
  - With TSC deadlines, the wait ends in at most 30 s with `Err`, and the volume goes read-only.
  - Governance never waits on NeoFS writeback for its own state, so it degrades only by losing persistence.
- **(c) Memory isolation.**
  - Every OUT transfer uses the same fenced bounce and wrapper buffers as reads (`DmaBuffer::alloc_in_arena`,
    `dma_arena::fence` → `FencedDmaAddr`, `msc.rs:111-117`, `:404-409`), under the existing xHCI IOMMU containment
    (ADR-050).
  - Caller memory is never handed to the device. No new DMA path exists.

## 10. The rungs

Each rung lands behind its feature. With the feature off, the kernel and the image are today's.

| Rung | What | Gate | Kill-switch (off ≡ today) | Oracle |
|---|---|---|---|---|
| **R1** | This THINK | The cross-family read (Dexter: DeepSeek, a different family from the author). PM approval. | — | — |
| **R2** | MSC write path: the (opcode, direction) allowlist, the data-out stage (§3.2), `next_write`, WRITE(10), SYNC CACHE(10), MODE SENSE(6) header, the `write_protected`/`supports_flush`/`flush` overrides, the post-guard probe (§3.5), TSC deadlines, sense mapping, the 32-bit read-edge refusal. Feature `usb-storage-write` (implies `usb-storage`). | Full tier (kernel driver). Host KATs in `msc_logic`, including **`usb_msc_write_cbw_direction`**. QEMU `usb_msc_write`: a `usb-storage` disk on `qemu-xhci` with a sealed scratch sector; write a pattern across 64- and 128-block chunk boundaries, flush, read back; **xtask hashes the raw image after QEMU exits**. A second, unmarked `usb-storage` disk must refuse writes (`WRITE REFUSED`) and, by QEMU's SCSI trace, receive no opcode outside today's six. | `usb-storage-write` off: today's allowlist and `command()` (KAT on the table and on a READ(10) CBW's bytes), `write_block` → `Unsupported`, `supports_flush` false. | Hand-laid CBW/CDB bytes for each new command (§3.1, §3.2), independent of the builder; the host-side image hash, independent of the kernel's read-back. |
| **R3** | The area and its mount: `xtask iso --persist-mib` and the GPT edit (§2.2), the two Limine entries and the cmdline parser (§2.3), boot-device identification, `arming_report`, the RW rule and the read-only flip (§4), the census (§5), the vm-image refusal, and the Zenbook runbook's update (the stick is now a data medium, not only a boot medium). Feature `usb-persistence` (implies R2 and `neofs-partition`). | Full tier. Scenarios: **`usb_persist_two_boot`** (boot 1 writes and bumps the counter, boot 2 reads it); **`usb_persist_never_internal`** (§5); **`usb_persist_unplug_midwrite`** (§4.2); the try entry (stick hash unchanged, mount read-only); area absent; area corrupt; a second LogansOS stick attached (nothing RW); two sticks from one image (not identified). | The feature off: `xtask iso` without `--persist-mib` gives an image **byte-identical** to the pre-R3 builder's on the same commit and inputs (R3 first pins xorriso's dates so the build is reproducible, if it isn't already); the kernel mounts as today. | The image oracle of §2.2 (its own GPT parser, both copies, the hand-laid superblock fields); the guard KAT over the built image and its "repaired" variant. |
| **R4** | The durable boot log: a kthread drains the console ring into `/logs/boot-<n>.log` on the volume. A "last boot's log" view. | **Full tier** (revision 1: a new kthread and a new file-write path are a new seam). A scenario: boot 2 shows boot 1's log tail. | The feature off: no log file, console as today. | Boot 2's log equals the bytes boot 1 printed (compared on the host). |
| **R5** | The Zenbook run, then the 0.1 showcase: one ISO, one Zenbook boot, all five rungs | PL run, two boots (§8), photos | — | The census line `nvme… armed=none`; boot 2's counter = boot 1's + 1. |

**Named later steps (not v1):**
- **Grow to fill the stick.** Option B: an INCOMPLETE-guarded GPT rewrite on the boot stick, with consent. It needs
  a narrow "boot-stick tail" claim, which is a relax-and-re-prove of I-17 with its own THINK.
- **Encryption** (§6).
- **FUA writes** for sticks without SYNCHRONIZE CACHE, guarded against `BROKEN_FUA`.
- **READ/WRITE(16)** for sticks over 2 TiB.
- **Per-feature persistence toggles**, as Tails offers.

## 11. Questions for the cross-family read

Revision 1 answers question 3 (§4.1: a unique GUID match or no write). Still open:
1. Is "unchanged guard, partition at build time" (A) right for v1, or does the PL's "Tails style" need first-boot
   creation (B) now? (PL question; the design stands on A until he says otherwise.)
2. Is refusing persistence on a stick without SYNCHRONIZE CACHE too strict for cheap sticks? The alternative is FUA,
   or "flush is a no-op when the caching mode page says WCE=0".
3. Is 512 MiB the right v1 size, given the release asset grows by that much?

## 12. Revision 1: Dexter's relay-385 findings → changes

| Finding (`Plans/RETURN-2026-11-10-pr103-ot100-think-read-dexter.md`) | Change |
|---|---|
| **F1 (high)** / item 3: no data-out stage; `command` hard-codes device-to-host (`msc_logic.rs:391`) | §3.2: `Data::{None, In, Out}`, CBW flag `0x00` for Out, the bulk-OUT data stage through the fenced bounce, the BOT §6.7.3 stall case, the residue rule for writes, the (opcode, direction) allowlist (§3.1), `next_write` with the 32-bit refusal. **`usb_msc_write_cbw_direction` is in R2.** CP-77/78 corrected to introduce → guard (§5.1). |
| **F2 (high)**: no read-only flip; `writable` immutable, drain worker just exits | §4.2: `AtomicBool` + fault code, the fatal-error classifier, the once-only line, the dropped-block count, no superblock written, `fsync` returns the fault, the user notice. **`usb_persist_unplug_midwrite` is in R3**, with a classifier KAT. New-seam row in §5.1. |
| **F3**: the mount rule and census need state the API doesn't expose | §3.3: `BlockDevice::write_protected()`, forwarded by `PartitionDevice`; §4.1: `block::arming_report`; the RW rule requires exactly one claim, equal to the partition. |
| **F4**: the probe sent SYNCHRONIZE CACHE to every MSC device at bind | §3.5: bind unchanged; the probe runs only on the identified, armed boot device, from the mount. R2 checks, by QEMU's SCSI trace, that an unarmed stick sees only today's six opcodes. |
| **F5**: vm-image layouts assume no NeoFS on the boot disk | §4.1 item 4: `xtask vm-image` never takes a persistent ISO, with a unit test. |
| **F6** / item 6: the "any device" fallback could mount another LogansOS volume RW | §4.1 items 1 and 3: no write when the boot device isn't uniquely identified; a non-boot USB LogansOS volume is never RW; installed internal LogansOS keeps today's rule (it's ours); on the Zenbook the statement is unconditional. R3 adds the two-stick cases. |
| Item 2(a): the appended entry and both GPT copies not established | §2.2: the host-side GPT edit covers both arrays and all four CRCs; the image oracle with its own GPT parser checks both copies, the entry, and the superblock; the guard KAT runs on the image and on its "repaired" variant. **There is no kernel write outside the claimed extent in option A** (the PM's item 3a): the only GPT writer is xtask, on the host. |
| Item 2(b): `persist=off` not wired | §2.3: two Limine entries with `cmdline:`, `timeout: 3` on the stick image only, `ExecutableCmdlineRequest`, a host-tested parser whose default is **off**; "Try" in the setup menu flips the current boot read-only through §4.2's switch, with no hidden next-boot state. |
| Item 2(c): `boot_disk_guid()` may be zero, then a guess | §4.1 item 1: zero, no match or several matches = not identified = no USB write. |
| Item 7: R3's kill-switch should be byte identity; R4's tier | §10: R3 off ≡ the pre-R3 image byte for byte (dates pinned first); R4 is full tier. |
| Item 8: no Tails date | §7: Tails 7.12, 2026-09-03 (release notes), with every git host tried and refused listed, and 7.15.1 noted as current. |
| Small: `mod.rs:383-395` | §1: `:383-427`. |
| Small: "read-only as today" vs the product's RW mount | §0 and §1 say which case is read-only today. |
| Small: "never calls `writes_allowed()`" under `persist=off` | §2.3: "never issues a write to the stick"; the census may judge it (reads only). |
| Small: the 32-bit read edge would panic, not refuse | §3.2: R2 adds the explicit refusal on both paths. |
| Small: the runbook says image A writes nothing on the Zenbook | R3 updates `docs/runbook-baremetal-zenbook-s14.md`: the stick is now a data medium; the NVMe rule is unchanged (§10). |
| Small: the ELF-string kill-switch proxy | Dropped from R2's kill-switch; the allowlist and trait KATs are the checks. |
| Small: the governance-lock grep would trip on `dma_arena` | §9: a module-path check, not a string match. |
