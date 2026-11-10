<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: OT-100, Tails-style persistence on the boot stick

**Date:** 2026-11-10. **Seat:** F. **Status:** step 1 of `Plans/DISPATCH-2026-11-10-seat-f-ot100-usb-persistence.md`.
This is the design only. No kernel code changes in this step. The cross-family read (Dexter) comes next, then the PM
dispatches the rungs.
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
feature. At boot, when the boot device is that stick and the user didn't pick the "try" entry, NeoFS mounts the
partition read-write as the product volume. Otherwise the boot is read-only, as today. Growing the area to fill the
stick, and encrypting it, are named later steps. Both need decisions this v1 doesn't have to make.

## 1. What exists today

- **The guard** wraps every block device, with no production off switch (`block/mod.rs:705-731`).
  - It arms lazily, on the first write or on `writes_allowed()` (`mod.rs:383-395`).
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
  - With no volume, the boot carries on and logs why (`:1166-1183`).
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
- **How:** `xorriso … -append_partition 3 <type> persist.neofs -appended_part_as_gpt`.
  - `persist.neofs` comes from `tools/neofs-mkfs`, the same tool as `vm_image`'s data disk.
  - **To verify in R3:** that the dev container's xorriso accepts a GPT type GUID for an appended partition, and
    where it puts the partition relative to the ESP. Some xorriso versions take only an MBR type byte. If ours
    does, xtask rewrites the one entry's type GUID and recomputes both CRCs. That is a host-side edit, oracled
    below.
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

### 2.3 Consent and "Try" (the setup menu)

Tails asks before creating the area and asks again, for a passphrase, each boot. v1 creates the area at build time,
so the consent left is per boot: **do we write this time?**
- The Limine menu on the image gets two entries:
  - `LogansOS` (the default): uses the persistence area;
  - `LogansOS — try (nothing is saved)`: passes `persist=off` on the kernel command line.
- With `persist=off`, the kernel never calls `writes_allowed()` on the stick, and it mounts the area read-only, or
  not at all.
- The setup menu's "Try" button (THINK-2026-11-02-setup-menu) maps to the same flag for the next boot. It needs
  nothing new in the guard.

## 3. The USB mass-storage write path (dispatch item 2)

The design is ours. The traps are from Linux `usb-storage`/`sd` (read for the principle only, GPL-2.0). The
fallback shapes are from Haiku's `usb_disk` (MIT; design borrowed, no code). See §7.

### 3.1 Commands added to the allowlist, under a new feature `usb-storage-write`

| Op | Command | CDB (byte oracle in R2) | Why |
|---|---|---|---|
| `0x2a` | WRITE(10) | `2a 00 LBA[4, BE] 00 LEN[2, BE] 00` | The write. FUA bit 3 of byte 1 stays **0** (Linux `BROKEN_FUA`). |
| `0x35` | SYNCHRONIZE CACHE(10) | `35 00 00000000 00 0000 00` (the whole medium) | `flush()`. |
| `0x1a` | MODE SENSE(6) | `1a 08 3f 00 04 00` (DBD, all pages, 4-byte header only) | Write-protect: byte 2 bit 7 of the header. |

- **Not added:** WRITE(16) and READ(16). The current reads already stop at a 32-bit LBA (`msc_logic.rs:302-311`),
  and the stick is 30 GB. A larger stick is refused writes with a line, not mis-addressed.
- Also not added: PREVENT/ALLOW MEDIUM REMOVAL, START STOP UNIT and WRITE SAME. The allowlist doc's warning
  still holds for them.
- **MODE SENSE asks for the 4-byte header only.** Linux's `sd` does the same for USB: devices that crash on page
  `0x3f` with a large allocation length are why it has `NO_WP_DETECT` and the "use 192 bytes" heuristics. A stall
  or a failure reads as "write-protect unknown". Then the first WRITE's sense is the fallback: DATA PROTECT,
  sense key 7, sets `write_protected` (Haiku's shape).
- **The feature off** keeps today's allowlist and today's `Unsupported`, bit for bit (§10, R2's kill-switch).

### 3.2 `write_block`, `supports_flush`, `flush`

- **`write_block(lba, buffers)`:**
  1. Copy the caller's data into the fenced 64 KiB bounce buffer. Caller buffers never reach the device, the same
     rule as reads (`msc.rs:346-356`).
  2. Issue WRITE(10) in chunks of at most `BOUNCE_LEN / block_size` blocks, under the same `transport` mutex as
     reads (CP-18, §5).
  3. Check every CSW: signature, tag, residue (must be 0 for a write) and status.
- **The maximum transfer stays 64 KiB (128 × 512 B).** Linux needs `MAX_SECTORS_64` (32 KiB) for a few devices.
  If R5 shows a stick timing out or failing big writes but not small ones, the chunk drops to 64 blocks.
  - That is a constant, not a design change. The discriminator in §8 says which case it is.
- **`supports_flush`:** true once a SYNCHRONIZE CACHE has succeeded at bind time (the "sync probe", §3.4). Until
  then it's false, so the NeoFS writable mount refuses, as today.
- **`flush`:** SYNCHRONIZE CACHE(10).
  - ILLEGAL REQUEST (sense key 5, ASC 0x20) means unsupported. Haiku sets `sync_support = 0` on it; we latch
    `sync_unsupported`.
  - Any other failure retries twice, then gives `Err`, and NeoFS keeps the volume dirty.
  - Linux's `SENSE_AFTER_SYNC` quirk (some sticks only report a sync failure on the next REQUEST SENSE) is
    covered: a failed CSW already triggers REQUEST SENSE here.

### 3.3 Errors, sense, the stick pulled mid-write

- **A CSW with status 1 (FAIL):** REQUEST SENSE, then map it:
  - UNIT ATTENTION (6): retry once;
  - NOT READY (2): retry after 250 ms, at most 4 times;
  - DATA PROTECT (7): `write_protected`, `Err(WriteRefused)`, and the volume drops to read-only;
  - MEDIUM ERROR (3) or HARDWARE ERROR (4): `Err(Io)` with the LBA.
- **Status 2 (PHASE) or an invalid CSW:** the existing reset recovery, in BOT §5.3.4 order (`msc.rs:255-265`).
  Then retry the command once.
- **A 0-length CSW:** read it again once (the Linux transport's case, "unnecessary zero-length packets").
- **The stick pulled:** the hot-unplug path sets `gone` (`msc.rs:572-610`). Any write in flight returns
  `Err(DeviceGone)`.
  - NeoFS treats that as a dead device: no retry loop, the mount goes read-only, and a line is printed (§8 F11).
  - The `RETIRED_DISKS` holder keeps the device memory alive, so nothing frees a buffer the controller may still
    write.
  - What survives is NeoFS's own commit order (`fs/neofs/commit_logic.rs`). R3's QEMU unplug-mid-write scenario
    tests that order on this device.
- **Waits are counted in time, not spins.** `WAIT_POLLS` today is 400 000 polls of 20 000 spins, "a few seconds"
  (`msc.rs:39-41`): CP-80's shape. A Zenbook core spins several times faster than a CI runner. SYNCHRONIZE CACHE
  on flash can legitimately take seconds.
  - The write path's waits use a deadline from the calibrated TSC: 10 s for WRITE and 30 s for SYNC. The read
    path's spin count is left alone; R2 doesn't touch it.

### 3.4 Bind-time probe (write feature on)

After READ CAPACITY:
1. MODE SENSE(6), header only, for `write_protected`.
2. TEST UNIT READY.
3. One SYNCHRONIZE CACHE(10): success sets `supports_flush`; ILLEGAL REQUEST latches `sync_unsupported`.
4. One line: `usb-msc: write probe wp=<yes|no|unknown> sync=<ok|unsupported|failed:<sense>> max_blocks=<n>`.

**A stick that lies about its cache** (Linux `ALWAYS_SYNC`, `WRITE_CACHE`) gets the strict v1 rule anyway: every
NeoFS commit point flushes. A stick that has no SYNCHRONIZE CACHE gets no persistence in v1. It boots read-only,
with the line saying why. FUA writes as a substitute are a named later option (§10), because of `BROKEN_FUA`.

## 4. NeoFS on the area (dispatch item 3)

- **Which volume.** Under a new `usb-persistence` feature:
  1. The boot mount first looks for a LogansOS partition on the **boot device**: the block device whose GPT disk
     GUID equals `boot_disk_guid()`.
  2. Only then does it fall back to today's "first LogansOS partition on any device".
  - That stops another LogansOS stick, or an installed disk, from being mounted as this boot's persistence volume.
  - `boot_disk_guid` moves out from behind `installer` into a small shared module. It's the same Limine request
    either way.
- **Read-write only when all hold:**
  1. the feature is on;
  2. no `persist=off`;
  3. the stick's probe reported `wp=no` (or `unknown`) and `sync=ok`;
  4. the guard armed exactly that partition: `writes_allowed()` is true, and the armed extent equals the
     partition's span.

  Otherwise it mounts read-only, or not at all, and the boot carries on (today's `boot continues` lines plus the
  reason).
- **What lives there:** whatever the product writes to its NeoFS volume today, nothing new:
  - Arthur's memory (OT-17: typed records, the per-category keys, the `.mem` files);
  - settings;
  - the user's files;
  - from R4, the boot log (§10, R4).
- **Missing area** (an old image, a non-DD write, a corrupt superblock): nothing is armed, the mount finds nothing,
  and the boot is read-only. Nothing hangs. The R3 scenario boots with the area absent and with it corrupt.

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
  `persist: guard census device=<name> driver=<d> armed=<extent|none> table=<t>`. This is new capture (the rule:
  capture before you need it). It means the Zenbook photo shows positively that nothing on the NVMe is armed.
  - The census calls `writes_allowed()`, which judges with reads only, in thread context at boot (CP-82, §5.1).
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
| CP-77 / CP-78 | DMA grant order, and the descriptor OWN/doorbell order | **Leave.** The OUT TRBs use the same ring code as today's bulk-in. | The existing xHCI ring KATs, plus R2's write-then-read-back. |
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
| **Tails Persistent Storage** (`tps`, Welcome Screen; tails.net design pages) | A LUKS partition `TailsData` created **on the running stick** in its free space, after the system partition. ext4 inside, unlocked by passphrase each boot, with per-feature bind mounts. | GPL-3.0+ (Tails) | **Not checked:** `gitlab.tails.boum.org` refused (403). Read via its public docs only. | Design only, never code. | The shape: a separate partition after the system one, consent per boot, the system untouched, opt-in per feature. **Not taken for v1:** creating it at run time (§2.1 B), and the encryption (§6). |
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
| `NO_WP_DETECT`; MODE SENSE page 0x3f with a big length hangs some sticks (Linux `sd`) | A hang or stall at bind | A 4-byte header only. A stall reads as `wp=unknown`. |
| A stick that lies about write-protect, or reports no WP but returns DATA PROTECT | Writes fail | DATA PROTECT → read-only, with a line (F2). |
| `ALWAYS_SYNC` / `WRITE_CACHE` (lies about its cache) | Data lost on unplug despite a "clean" write | Every NeoFS commit point flushes (§3.4). |
| SYNCHRONIZE CACHE unsupported (ILLEGAL REQUEST) / `SENSE_AFTER_SYNC` | Flush fails, or reports late | Latch unsupported → no persistence in v1 (F3). REQUEST SENSE after any failed CSW. |
| `BROKEN_FUA` | FUA writes fail | Never set FUA. |
| 0-length CSW (extra ZLP after data) | A spurious transport error | Re-read the CSW once. |
| `IGNORE_RESIDUE` (bogus residue) | False short-write errors | For a write, the residue must be 0 with status OK; any other residue counts as a failure and is printed. If R5 shows a stick reporting a bogus residue on writes that read back correctly, that's a quirk to add, with its line. |
| `MAX_SECTORS_64` / `MAX_SECTORS_240` | Big transfers stall or time out | 64 KiB chunks; the line in F10 tells chunk size from timing. |
| `INITIAL_READ10`, and the first command after reset failing | Bind or first-write failure | UNIT ATTENTION retry once (§3.3). |
| `BULK_IGNORE_TAG` | CSW tag mismatch | A tag mismatch is a transport error → reset recovery. Counted (CP-18 row). |
| Phase error / stalled bulk-out mid-data | A hung transfer | BOT §5.3.4 reset recovery, then one retry. |
| The stick pulled mid-write | Partial writes | `gone` → `DeviceGone`, read-only. NeoFS commit order (§3.3). |
| Rufus in **ISO mode**, not DD | Rufus writes its own FAT layout: no GPT from our image, no LogansOS partition | Nothing arms. The boot is read-only and says so (F6). |
| Firmware, or Windows on the PL's PC, "repairs" the GPT by moving the backup to the stick's end | The GPT changes under us | The primary still validates (new alternate and CRCs). The census prints `table=gpt` vs `gpt-backup` (F12). |

### 8.2 The Zenbook run's bank

| # | Mode | What the screen shows (discriminator) | Next action |
|---|---|---|---|
| F1 | MODE SENSE stalls | `usb-msc: write probe wp=unknown …`, with persistence still on | None needed. Logged so a later hang is not blamed on it. |
| F2 | Write-protected stick | `wp=yes`, or `usb-msc: DATA PROTECT lba=… ; persistence read-only` | Check the stick's switch, or swap the stick. |
| F3 | SYNC unsupported | `sync=unsupported` + `persist: no durable flush; booting read-only` | A different stick, or the FUA option (§10). |
| F4 | WRITE fails with sense | `usb-msc: WRITE(10) lba=… blocks=… FAIL sense=<k>/<asc>/<ascq>` | Map the sense to a quirk. |
| F5 | Phase error / reset loop | `usb-msc: reset recovery #n after <cmd> lba=…`, counted | More than 3 in a boot: drop the chunk to 64 blocks (F10). |
| F6 | No persistence partition | `persist: no LogansOS partition on boot disk guid=… (table=…)` | Re-write the stick with Rufus **DD** mode. |
| F7 | Boot device not identified | `persist: boot disk guid=… matches no block device`, and every device's GPT disk GUID listed | Limine reported a different GUID (for example the ESP's): fix the matching rule. |
| F8 | Mount fails | `boot-neofs: mount failed <err>; boot continues` + `persist: read-only` | The superblock/commit diagnostics already printed by NeoFS. |
| F9 | **Anything armed on the internal NVMe** | The census line `device=nvme… armed=<extent>` | **Stop the run.** It must read `armed=none table=gpt`. Any other value is a guard defect, which was proven impossible, so this is the line that would show it. |
| F10 | Timeouts on large writes | `usb-msc: WRITE(10) deadline 10000 ms passed lba=… blocks=128` | Retry with 64-block chunks: this tells `MAX_SECTORS_64` from a dead stick. |
| F11 | Stick pulled or dropped off the bus | `usb-msc: device gone during WRITE lba=…` + `persist: volume lost; read-only` | Expected if the PL pulls it. Otherwise a hub or power issue. |
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
  - `msc.rs:11-17`'s I-14 note stays true. R2 adds a grep check that `msc.rs` names no governance lock, plus the
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
| **R2** | MSC write path: WRITE(10), SYNC CACHE(10), MODE SENSE(6) header, the bind probe, TSC deadlines, sense mapping. Feature `usb-storage-write` (implies `usb-storage`). | Full tier (kernel driver). Host KATs in `msc_logic`. QEMU scenario `usb_msc_write`: a `usb-storage` disk on `qemu-xhci` carrying a sealed scratch sector. The scenario writes a pattern across several chunk boundaries, flushes and reads it back. Then **xtask hashes the raw image file after QEMU exits**. A second scenario writes an unmarked stick and expects `WRITE REFUSED`. | `usb-storage-write` off: `ALLOWED_OPCODES` unchanged (a KAT on the array), `write_block` → `Unsupported`, `supports_flush` false; the product ELF must not contain the new probe's line. | Hand-laid CBW bytes for each new CDB (§3.1), independent of the builder. The host-side image hash is independent of the kernel's own read-back. |
| **R3** | The persistence area and its mount: `xtask iso --persist-mib`, `-append_partition`, `neofs-mkfs`, the two Limine entries, boot-disk matching, the RW mount rule, and the census. Feature `usb-persistence` (implies R2 and `neofs-partition`). | Full tier. Scenarios:<br>1. **`usb_persist_two_boot`**: the image as a `usb-storage` boot stick; boot 1 writes and bumps the counter, boot 2 reads it.<br>2. **`usb_persist_never_internal`**: §5's Windows-shaped NVMe stays byte-identical and its line is printed.<br>3. Area absent and area corrupt: read-only, no hang.<br>4. `persist=off`: no write reaches the stick (image hash unchanged).<br>5. Unplug mid-write (QMP `device_del`): read-only, no panic, next boot mounts. | The feature off: `xtask iso` without `--persist-mib` yields today's image with no LogansOS entry. The kernel mounts as today. | The GPT bytes of the built image are checked by xtask against a hand-laid expectation: type GUID, start/end LBA, attributes, and the CRCs recomputed independently. A guard KAT over the built image's first and last MiB asserts it arms exactly the appended partition, never the ESP or ISO ranges, and never the metadata. |
| **R4** | The durable boot log: a kthread drains the console ring into `/logs/boot-<n>.log` on the volume. A "last boot's log" view. | Quick tier unless it touches the SMP core. A scenario: boot 2 shows boot 1's log tail. | The feature off: no log file, console as today. | Boot 2's log equals the bytes boot 1 printed (compared on the host). |
| **R5** | The Zenbook run, then the 0.1 showcase: one ISO, one Zenbook boot, all five rungs | PL run, two boots (§8), photos | — | The census line `nvme… armed=none`; boot 2's counter = boot 1's + 1. |

**Named later steps (not v1):**
- **Grow to fill the stick.** Option B: an INCOMPLETE-guarded GPT rewrite on the boot stick, with consent. It needs
  a narrow "boot-stick tail" claim, which is a relax-and-re-prove of I-17 with its own THINK.
- **Encryption** (§6).
- **FUA writes** for sticks without SYNCHRONIZE CACHE, guarded against `BROKEN_FUA`.
- **READ/WRITE(16)** for sticks over 2 TiB.
- **Per-feature persistence toggles**, as Tails offers.

## 11. Questions for the cross-family read

1. Is "unchanged guard, partition at build time" (A) right for v1, or does the PL's "Tails style" need first-boot
   creation (B) now?
2. Is refusing persistence on a stick without SYNCHRONIZE CACHE too strict for cheap sticks? The alternative is
   FUA, or "flush is a no-op when the caching mode page says WCE=0".
3. Is matching the boot device by `boot_disk_guid()` enough, or can Limine's response name a GUID that the stick's
   GPT doesn't carry (F7)?
4. Is 512 MiB the right v1 size, given the release asset grows by that much?
