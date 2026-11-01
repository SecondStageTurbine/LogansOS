<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: L3u, USB mass storage (Bulk-Only Transport + SCSI), read-only first (2026-10-31)

The PM's call (2026-10-31, on #9): USB mass storage is its own lane, this seat's after L3a, and it starts with a
prior-art survey and trap mining. This is that THINK. With L3a (MBR + FAT, #17 and #18) it is what makes "read an
ordinary USB stick" true.

## 1. What is already there

- **xHCI host** (`kernel/src/drivers/usb/xhci.rs`): enumeration, `configure_endpoints` for bulk and interrupt
  endpoints (SuperSpeed `max_burst` included), `EndpointPipe::submit`/`completions`, `control_read`/`control_write`/
  `control_no_data` with a distinct "stalled" result, `clear_endpoint_halt` (ClearFeature(ENDPOINT_HALT) on EP0) and
  `recover_endpoint` (Reset Endpoint + Set TR Dequeue). Bulk endpoints already run in production for CDC-ECM
  (`cdc_ecm.rs`) and the MT7601U.
- **Descriptor parsing** (`usb_parse.rs`): interfaces, alternate settings, endpoints (`is_bulk`).
- **Block layer** (`drivers/block`): `BlockDevice`, the registry, and the write guard every registered device sits
  behind (`GuardedDevice`). A USB disk gets no sealed marker, so the guard refuses every write to it. L3u is
  read-only anyway, and this keeps the Zenbook rule (never write the internal NVMe) out of reach.
- **Above it:** L3a's `deviceN:pM` MBR view and the rust-fatfs mount.

## 2. Prior-art survey (checked 2026-10-31)

| Candidate | What it is | Licence | Last activity (checked) | Fit | What we take |
|---|---|---|---|---|---|
| `embassy-usb-host` (`embassy-rs/embassy`, `class/msc.rs`) | Async embedded USB host; an MSC class driver doing BOT + SCSI transparent, CBW → data → CSW, stall and phase-error handling | MIT OR Apache-2.0 | crate 0.1.0 published 2026-05-03 (crates.io); repo `HEAD` is `14cf6a68` (`git ls-remote`, 2026-10-31; its commit date not checked here) | Close in shape (no_std), but bound to embassy's async host-driver traits, not our xHCI | **Design only**: the state machine and its error cases, read as a cross-check of ours. No code. |
| `scsi` (`ischeinkman/scsi-rs`) | no_std SCSI command structs for BOT devices | Apache-2.0 | crate 0.2.1, 2019-06-23; self-described work in progress | Command encoding only | **Nothing**: abandoned (no release in seven years); the CDBs we need are five, and their bytes come from the spec with our own byte oracle. |
| `usbh-scsi` | BOT/SCSI over `rusb` for desktop tools | MIT OR Apache-2.0 | crate 0.1.0, 2025-10-01 | `std` + libusb; not embeddable | Nothing. |
| `usbd-storage` | Device-side (gadget) BOT/SCSI | MIT | crate 3.0.0, 2026-07-05 | Wrong side (device, not host) | Its CBW/CSW structs are a second reading of the layout for the byte oracle's review, nothing more. |
| Redox `drivers` (`usbscsid` on `xhcid`) | Userspace BOT + SCSI driver over Redox's xHCI daemon | MIT (Redox's stated licence; **not re-checked**) | **unverified**: `gitlab.redox-os.org` is not reachable from this container; the MR that added it is public | Same architecture idea (MSC over an xHCI host), different process model | Design reading only, if the next reader can reach it. |
| Linux `drivers/usb/storage` (`transport.c`, `unusual_devs.h`, `scsiglue.c`) | The reference BOT implementation and its quirk table | GPL-2.0 | active | **Read for principle only, never copied** | The trap list in §3 (reset recovery, the last-sector and capacity quirks, the 36-byte INQUIRY, the transfer-size cap). |
| USB-IF *Mass Storage Class Bulk-Only Transport* rev 1.0 | The spec: CBW/CSW layout, the thirteen host/device cases, reset recovery | spec | 1999, stable | Normative | The byte layouts and the error-handling cases, fetched raw (PDF) when the oracle is written. |

**Decision:** write the BOT + SCSI layer ourselves on the existing xHCI pipes. It is small: five SCSI commands, two
fixed-layout wrappers and one state machine. Every candidate with code is either bound to another host stack or
abandoned. We borrow the design and the trap list, not code. No vendoring, so no licence obligations; Linux is GPL
and is read for principle only. The Linux constants quoted in §3 (the 36-byte INQUIRY, the 240-sector default, the
quirk flag names) are recalled, not re-read; L3u-2 re-reads them from the raw source (`git`, never a summarizing
fetch) before they are relied on, and the THINK is corrected if any differs.

## 3. Trap mining: documented field gotchas, each with a discriminator

| # | Trap | Source | Discriminator / guard |
|---|---|---|---|
| U1 | **UAS vs BOT alternate settings.** Many USB 3 drives offer UAS (protocol `0x62`) on alt setting 1 and BOT (`0x50`) on alt 0. Binding to the first interface descriptor seen, or to the UAS alt, gives a device that never answers. Same class of trap as CDC-ECM's alt-setting-1. | Linux `uas.c`/`usb-storage` binding; our own CDC-ECM history | Match class `08` / subclass `06` / protocol `50` exactly, and `SET_INTERFACE` to that alt. Log `msc bind alt=N proto=0x50`. |
| U2 | **CBW is exactly 31 bytes, CSW exactly 13.** Signatures `USBC` (`55 53 42 43`) / `USBS` (`55 53 42 53`), little-endian fields. A 32-byte CBW (padding) is a common bug. | BOT §5.1, §5.2 | Host KAT: a hand-laid CBW for READ(10) of LBA 2048 × 8 blocks, and a CSW, asserted as literal bytes (the byte oracle). |
| U3 | **CSW tag must equal the CBW tag**; a wrong signature, wrong tag or status `02` is a phase error, which needs **reset recovery**, not a retry. | BOT §5.3, §6.7 | `CswCheck` returns `Ok`/`Failed`/`PhaseError`; a phase error triggers reset recovery and is logged with both tags. KATs for each case. |
| U4 | **Reset recovery** is three steps in order: class request Bulk-Only Mass Storage Reset (`0xFF`, interface), then ClearFeature(HALT) on bulk-in, then on bulk-out. Skipping a halt clear leaves the next transfer stalled forever. | BOT §5.3.4 | One function, unit-checked for order (a recorded request list in a host test of the state machine). |
| U5 | **A stall on the data stage is not the end of the command:** clear halt on that pipe, then still read the CSW. A stall reading the CSW: clear halt and read it once more; a second failure means reset recovery. | BOT §6.7.2, §6.7.3 (the 13 cases) | State-machine host tests driving each case with a scripted fake pipe. |
| U6 | **Residue.** `dCSWDataResidue` > the requested length is invalid (phase error); a short read with status 0 is valid, and the block layer must see a short read as an error for a full-block request. | BOT §6.7 | KAT on the residue check; the block read fails `Io` on any residue. |
| U7 | **Get Max LUN (`0xFE`) stalls on many single-LUN devices.** Treat a stall as LUN 0 and clear EP0 (the stall is protocol, not error). | Linux `usb_stor_Bulk_max_lun` | Log `msc maxlun=0 (stalled)`; never fail binding on it. |
| U8 | **INQUIRY: ask for exactly 36 bytes.** Some devices hang or stall on other allocation lengths. | Linux `scsiglue.c` (36-byte INQUIRY) | Fixed 36; log vendor/product strings (ASCII-sanitised). |
| U9 | **TEST UNIT READY fails at first with UNIT ATTENTION** (sense key 6, ASC `28`/`29`: medium changed / power-on reset); a spinning-up device says NOT READY, becoming ready (key 2, ASC/ASCQ `04/01`); one that wants START UNIT says `04/02`; card readers with no card say NOT READY `3A`. | SPC; Linux `sd_spinup_disk` | `ready_action` maps each (KAT `sense_formats_and_actions`): UNIT ATTENTION → retry at once, at most 5; `04/01` → **wait** ~250 ms and ask again, at most 40 times (~10 s; Chad's review: slow spin-up), logged `msc: spin-up waits=N ready=R`; `04/02` → decline, logged `msc: needs START UNIT` (START STOP UNIT is not on the allowlist, U17); `3A` → final `no medium`, not retried (as Linux). |
| U10 | **READ CAPACITY(10) returns the *last* LBA, not the count.** Count = last + 1. | SBC §5.16 | KAT on a hand-laid 8-byte reply. |
| U11 | **READ CAPACITY(10) returns `FFFFFFFF`** for > 2 TiB: must use READ CAPACITY(16) (service action `0x10`). | SBC | Issue (16) when (10) says `FFFFFFFF`; KAT on both replies. |
| U12 | **Capacity off by one** (some devices report count + 1; Linux's `US_FL_FIX_CAPACITY` / last-sector hacks): the last sector read fails. | Linux `unusual_devs.h`, "add last-sector hacks" | Read-only means only a filesystem's own request reaches the end. L3a's MBR check already refuses a partition past the reported end. A failed read of the reported last LBA is logged `(last-lba read failed)`, never fatal to the mount. **Boundary tests** (Chad's review), in `usb_storage_read`: the reported last LBA reads; one past it, and a two-block read ending one past it, are refused `InvalidRequest` before any command is sent. |
| U13 | **Block size ≠ 512** (4K-sector drives). | SBC | Geometry carries the real size; L3a's FAT path refuses non-512 with a clear log line, so a 4K stick fails visibly, not silently. |
| U14 | **Transfer size.** Some devices fail large transfers; Linux caps usb-storage at 240 sectors (120 KiB) by default and lower for known devices. | Linux `scsiglue.c` `max_sectors` | Cap READ(10) at 128 blocks (64 KiB) per command, and at the bounce buffer for larger blocks; larger reads are split. **Boundary KAT** `reads_split_at_64_kib`: 300 blocks from LBA 2048 split into exactly (2048, 128), (2176, 128), (2304, 44), then none; and the last READ(10)-addressable block `0xFFFF_FFFF` reads alone while two blocks from it are refused. |
| U15 | **Short packets.** A data stage that ends on a short packet completes with xHCI code 13 (Short Packet); the ring must treat it as success with a length. | xHCI §4.10.1.1 | Our ring already reports transfer length; the MSC layer cross-checks it against the residue (U6). |
| U16 | **SuperSpeed bulk:** max packet 1024 and MaxBurst from the SS endpoint companion descriptor. | xHCI / USB 3 | `configure_endpoints` already takes `max_burst`; the QEMU gate runs the stick on `qemu-xhci` at SuperSpeed and at High Speed. |
| U17 | **Commands to avoid on the read-only path:** MODE SENSE, SYNCHRONIZE CACHE, PREVENT/ALLOW MEDIUM REMOVAL (and START STOP UNIT) hang or stall some devices. | Linux quirk flags (`US_FL_NO_*`) | An **allowlist**, enforced in `msc_logic::command`: only INQUIRY, TEST UNIT READY, REQUEST SENSE, READ CAPACITY(10/16) and READ(10) are sent; any other opcode returns `Reset{NotAllowed}` before the pipe is touched. KAT `only_allowlisted_commands_reach_the_device` (Chad's review): a scripted device that **stalls every unsupported opcode** passes every command the driver builds, and MODE SENSE, SYNCHRONIZE CACHE, PREVENT ALLOW, START STOP UNIT and WRITE(10) never reach it (zero pipe calls). |
| U18 | **Hot unplug mid-command.** A transfer never completes, or completes with an error; the stick's block device must not outlive it. | general; Jarvis's #30 review | Every BOT step has a timeout. On disconnect the port service tells the bound driver (`UsbClassDriver::detach`); the disk is marked gone (the transfer in flight stops waiting at its next poll, halt clears and resets are skipped), its registry slot becomes a `detached` placeholder answering `NoDevice`, and its transport is retired, not freed. Scenario `usb_storage_unplug`: a reader running while the stick is pulled ends with `NoDevice`, a held handle's next read is `NoDevice`, and no `usb-storage` device is found any more. |
| U19 | **Multiple LUNs** (card readers). | BOT §3.2 | LUN 0 only in L3u; Max LUN is logged so a multi-LUN reader is visible. |

## 4. Design

- **`drivers/usb/msc_logic.rs`** (pure, `#[path]`-shared with a host KAT crate): CBW encode, CSW decode and check
  (U2, U3, U6), the five CDBs (INQUIRY 36, TEST UNIT READY, REQUEST SENSE 18, READ CAPACITY 10/16, READ 10), the
  sense decode (U9), capacity decode (U10, U11), and the BOT state machine over a `Pipe` trait (U4, U5), so every
  error case runs on the host against a scripted fake.
- **`drivers/usb/msc.rs`**: binding (U1, U7, U8, U9), the `Pipe` impl over `EndpointPipe`, and a `BlockDevice`
  (`read_block` splits at 64 KiB, U14; `write_block` returns `ReadOnly`), registered through the block registry, so
  it sits behind `GuardedDevice` like every other disk.
- **Kill-switch:** kernel feature `usb-storage`. Off: a mass-storage interface is not bound, exactly as today (the
  device enumerates and is ignored). A `-off` scenario proves no block device appears.
- **I-14:** (a) the MSC path runs in the thread that issued the block read; the xHCI ISR it waits on takes no
  governance lock (as today). (b) every BOT step is bounded, so a wedged stick slows only its own reader. (c) data
  buffers are the block layer's fenced `DmaBuffer`s under the existing IOMMU containment, as CDC-ECM's are; no new
  DMA path.
- **Not SMP-core:** one device lock per stick, held for one command; no new cross-CPU path.

## 5. Gate and acceptance

- **Host KATs (push-CI):** the byte oracle (hand-laid CBW/CSW, each CDB, READ CAPACITY 10/16 replies, sense data),
  and the state machine through each BOT error case (U3–U6, U9).
- **QEMU scenarios:** `-device qemu-xhci -device usb-storage,drive=…` (QEMU's BOT device) holding L3a's MBR+FAT32
  fixture: the stick binds, its capacity is right, `deviceN:p1` mounts and reads back byte-exact; the same at High
  Speed; a no-medium case if QEMU can present one; `usb_storage_off` (kill-switch). The hot-unplug case uses HMP
  `device_del` mid-read (the Zenbook input lane already drives HMP hot-add).
- **Acceptance (bare metal, a milestone, not the merge bar):** an ordinary FAT32 stick on the Zenbook lists and
  reads; the runbook gets the procedure. The write guard keeps the internal NVMe untouched regardless.

## 6. Order

1. L3u-1: `msc_logic.rs` and its host KATs (no kernel binding yet; push-CI only).
2. L3u-2: binding + `BlockDevice` + the QEMU scenarios.
3. Acceptance run on the Zenbook.
