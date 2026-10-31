# Dispatch - Phase 3 BUILD-5a-AHCI Driver Development In QEMU

Date: 2026-05-20
Agent: Jarvis
Status: AUTHORIZED IMPLEMENTATION

Authority:

- `Plans/PM-DISPOSITION-2026-05-20-phase-3-build-4-formal-closure-at-build-4D.md`
  §5.1 (BUILD-5 option II — split BUILD-5a/5b — selected by PL 2026-05-20)
- `docs/ADR-021-phase-3-design-substrate.md` BUILD-5 boundaries ("AHCI/NVMe +
  bare-metal validation")
- `Plans/PHASE-3-BUILD-4D-CLOSE-EVIDENCE-2026-05-20.md` (substrate state;
  block-device infrastructure)
- `Plans/PM-DISPOSITION-2026-05-20-phase-3-build-4C-close-consumption.md` §3.2
  (`VIRTIO_DEVICES` multi-device polling pattern; lock-hygiene rule)
- `docs/bible.md` v1.6.0 Articles 9.9 (storage drivers from Phase 3),
  9.4 (PCI/PCIe enumeration, ACPI), 16.3 (bare-metal verification gate
  for phase closure)

Substrate:

```text
aca3fb19  build-4D: add ext4 read-only foreign filesystem driver
```

## 1. BUILD-5 Sub-Structure (PM Operational Refinement)

PL authorized option II on 2026-05-20: BUILD-5 splits into BUILD-5a
(driver development in QEMU) and BUILD-5b (bare-metal validation when
hardware arrives).

PM refines option II further into sequential sub-lanes:

| Sub-build | Scope | Gate |
|---|---|---|
| **BUILD-5a-AHCI** | AHCI driver against QEMU ICH9 emulation | this dispatch |
| **BUILD-5a-NVMe** | NVMe driver against QEMU NVMe emulation | dispatched after 5a-AHCI closes |
| **BUILD-5b** | Bare-metal validation of both drivers on real silicon | dispatched when PL bare-metal hardware arrives |

Rationale for sequential sub-lanes: AHCI and NVMe are different storage
protocols but share the block-device backend infrastructure
(`VIRTIO_DEVICES`-style registry, completion polling, integration with
the `MountedFs` layer above). Doing them sequentially lets BUILD-5a-NVMe
reuse the block-device generalization work that BUILD-5a-AHCI surfaces,
rather than racing the same refactor twice. PM expects 5a-AHCI to land
some shared block-device infrastructure that 5a-NVMe will simply plug
into.

AHCI is dispatched first because:

- The block-device interface generalization is best surfaced by a
  protocol meaningfully different from virtio-blk (AHCI is much closer
  to "raw hardware" than virtio-blk is to a virtual device);
- QEMU's ICH9 AHCI emulation is well-understood and stable;
- AHCI exercises the kernel's MSI/MSI-X interrupt path on a real-ish
  controller, which BUILD-5b bare-metal will also need.

## 2. Mission

Land a working AHCI block-device driver against QEMU's ICH9 AHCI
controller emulation. The driver is a block-device backend usable by
the existing VFS/MountedFs layer; filesystems mounted on AHCI-attached
disks must work end-to-end without changes to the filesystem code
above.

The mission is the substrate proof that the BUILD-4B/4C/4D MountedFs
abstraction composes with a non-virtio block-device backend, and that
the kernel's PCI/MSI infrastructure supports a more realistic storage
controller than virtio-blk.

## 3. Pre-Implementation Verification (Required First Step)

Per the canonical container-tooling idiom:

```text
docker compose run --rm neo bash -c 'qemu-system-x86_64 -version'
docker compose run --rm neo bash -c 'qemu-system-x86_64 -device help 2>&1 | grep -E "^name \"(ahci|ich9-ahci)\""'
```

Expected outputs:

- QEMU version (any reasonably recent; project has been using QEMU
  throughout Phase 3, so this is verification not investigation);
- One or both of `ich9-ahci` (preferred — ICH9 emulation) or `ahci`
  (generic) appear in device help.

If neither AHCI device is available in the container's QEMU, surface as
a genuine stop condition. PM expects ICH9 AHCI to be present since
QEMU has shipped it since approximately QEMU 1.7 and the container
runs a modern QEMU.

Additionally verify the kernel's existing PCI infrastructure supports
finding a non-virtio PCI device:

```text
git grep -n "vendor.*device" kernel/src/drivers/pci/  # or equivalent
```

Confirm PCI enumeration is generic (not hard-coded to virtio vendor
IDs). If it is hard-coded, surface — that's a substrate gap that needs
addressing before AHCI can land cleanly.

## 4. Scope

### 4.1 In Scope

**AHCI driver against QEMU ICH9 AHCI emulation.**

- PCI enumeration: find the AHCI controller (PCI class 0x01, subclass
  0x06, programming interface 0x01 = AHCI 1.0; or vendor/device ID
  match against ICH9 AHCI: `0x8086:0x2922` or `0x8086:0x2929`);
- AHCI Host Bus Adapter (HBA) memory-mapped register access via the
  ABAR (AHCI Base Address Register, BAR5 of the PCI config space);
- HBA initialization sequence (BIOS/OS handoff if applicable, GHC.AE
  set, GHC.IE set after port init);
- Port enumeration via PI (Ports Implemented) register;
- Per-port initialization:
  - Stop CMD/FRE if running;
  - Clear PxIS and PxSERR;
  - Set up Command List Base Address (PxCLB/PxCLBU) — 1 KiB aligned,
    32 command headers per port;
  - Set up FIS Base Address (PxFB/PxFBU) — 256 byte aligned;
  - Start FRE, then CMD;
- ATA IDENTIFY DEVICE command to confirm port has a device attached
  and characterize it (sector size, total sectors);
- ATA READ DMA EXT (command 0x25) for 28-bit LBA reads;
- ATA WRITE DMA EXT (command 0x35) for 28-bit LBA writes;
- 48-bit LBA support (for disks > 137 GiB; QEMU disks may or may not
  exceed this depending on fixture size; implement 48-bit unconditionally
  since the protocol is the same complexity once you have it);
- Interrupt-driven completion via the existing kernel interrupt path;
  MSI preferred, legacy IRQ fallback if MSI setup is problematic;
- Completion notification to the block-device subsystem via the same
  pattern `VIRTIO_DEVICES` uses for virtio-blk.

**Block-device registration.**

- AHCI ports register as block devices using the same registry pattern
  established by virtio-blk in BUILD-4C
  (`kernel/src/drivers/block/virtio_blk.rs` + `VIRTIO_DEVICES`);
- If the registry needs generalization (renamed to `BLOCK_DEVICES` or
  similar; abstracted to accept either virtio-blk or AHCI ports), do
  the generalization as part of this BUILD. Surface in close evidence
  what the registry now looks like and what changed. The
  generalization sets up BUILD-5a-NVMe to plug in cleanly.

**QEMU test harness extension.**

- Add an AHCI controller to the QEMU run command (`-device ich9-ahci,id=ahci0`);
- Attach one or more SATA drives (`-drive file=...,if=none,id=disk3,format=raw`
  + `-device ide-hd,drive=disk3,bus=ahci0.0`);
- The harness should be able to attach an AHCI disk distinctly from the
  virtio-blk disks (device0–device2 are virtio-blk; AHCI disk is on a
  separate bus and gets its own enumerated device ID).

**xtask AHCI disk image generation.**

- Generate a deterministic raw disk image for the AHCI test (any
  filesystem — FAT32 or NeoFS — populated with a fixture pattern that
  the scenarios can verify);
- Same no-binary-fixtures-in-repo discipline as BUILD-4C/4D.

**Scenarios.**

Required new scenarios:

```text
phase3_ahci_controller_detected
phase3_ahci_port_identify
phase3_ahci_read_sector_pattern
phase3_ahci_write_sector_pattern
phase3_ahci_multi_sector_io
phase3_ahci_block_device_registered
phase3_ahci_fat32_mount_and_read
phase3_ahci_neofs_mount_and_readwrite
```

Scenario expectations:

- `phase3_ahci_controller_detected` — PCI enumeration finds the ICH9
  AHCI controller; HBA registers map; GHC accessible.
- `phase3_ahci_port_identify` — IDENTIFY DEVICE on at least one
  populated port returns valid response; sector count and sector size
  reported correctly.
- `phase3_ahci_read_sector_pattern` — read a known LBA range from the
  test disk; verify content matches xtask-generated pattern.
- `phase3_ahci_write_sector_pattern` — write a pattern to an LBA
  range; flush; read back; verify content matches what was written.
- `phase3_ahci_multi_sector_io` — read a multi-sector range (e.g.
  16 sectors = 8 KiB) in a single command; verify content.
- `phase3_ahci_block_device_registered` — AHCI port appears in the
  kernel's block-device registry; the registry can dispatch reads
  and writes to it through the same interface as virtio-blk.
- `phase3_ahci_fat32_mount_and_read` — mount FAT32 (using the
  existing BUILD-4C FAT32 driver) on the AHCI-attached disk; verify
  readdir + open + read works.
- `phase3_ahci_neofs_mount_and_readwrite` — mount NeoFS (the existing
  native filesystem) on an AHCI-attached disk; verify read+write
  operations work end-to-end through the new backend.

### 4.2 Anti-Scope

NOT in this BUILD:

- NVMe driver (that is BUILD-5a-NVMe, separate dispatch);
- Bare-metal validation (that is BUILD-5b, dispatched when PL hardware
  arrives);
- Legacy PATA / IDE controller support (only AHCI mode);
- ATAPI / optical drive support;
- SATA hot-plug;
- Native Command Queuing (NCQ) optimization — queue depth 1 is acceptable
  for BUILD-5a-AHCI (use first command slot only);
- Port Multiplier support;
- FUA (Forced Unit Access) optimization;
- TRIM / SCSI UNMAP / discard;
- SMART monitoring;
- Power management (HIPM / DIPM);
- Encryption (Opal / TCG self-encrypting drives);
- AHCI-specific syscalls — the driver is purely a backend, no userspace
  surface;
- Multi-controller AHCI (multiple HBAs) — one controller per system in
  BUILD-5a-AHCI scope;
- Concurrent multi-port I/O optimization — serializing under a per-
  controller or per-port lock is acceptable for initial implementation;
- Any change to the MountedFs trait or to existing filesystem drivers;
- BUILD-3F CAS/dedup;
- Bare-metal-specific code paths (e.g. real-hardware quirks).

## 5. Architecture Notes

### 5.1 AHCI specification reference

The authoritative spec is Intel "Serial ATA Advanced Host Controller
Interface (AHCI) Specification" revision 1.3.1 (June 2008; though
newer rev 1.3.1 Errata may apply). Section references in close
evidence should cite this spec when explaining design decisions.

QEMU's ICH9 AHCI emulation implements AHCI 1.3 substantially; some
edge cases differ from real hardware but the core protocol is faithful.

### 5.2 Memory-mapped DMA buffers

AHCI requires the Command List and FIS Receive Area to be in
physically-contiguous, DMA-coherent memory. The kernel's existing
DMA allocation infrastructure (the `DmaBuffer` pattern from
virtio-blk) should be reusable. If it isn't, surface the gap.

Command Lists are 1 KiB aligned. FIS Receive Areas are 256-byte
aligned. Command Tables (referenced from Command Headers) are 128-byte
aligned. The PRDT (Physical Region Descriptor Table) entries within
command tables are 4-byte aligned.

### 5.3 MSI vs legacy IRQ

QEMU's ICH9 AHCI supports MSI. The kernel's existing interrupt path
should be checked to confirm MSI is supported as an interrupt source.
If only legacy PCI IRQ is currently wired, AHCI can be made to work
via legacy IRQ — but a future bare-metal target may require MSI, so
PM mildly prefers MSI from the start. Surface the choice and
rationale in close evidence.

### 5.4 Lock discipline

Per the BUILD-4C close-hygiene rule reinforced by PM:

> Any new lock site must explicitly name lock type AND IF discipline
> location in the close artifact.

AHCI driver almost certainly introduces at least one new lock (for
the per-controller HBA state, possibly per-port command-slot
allocation). For each new lock, the close evidence must name:

- The lock type (`SwitchMutex<...>`, `spin::Mutex<...>`, etc.);
- Where the IF discipline is documented (declaration site comment);
- The interrupt-state invariant the lock holders are required to
  maintain (IF=0 hold, IF=1 hold, either, etc.).

### 5.5 Block-device registry generalization

If `VIRTIO_DEVICES` (introduced in BUILD-4C) is extended to a generic
`BLOCK_DEVICES` registry to accept both virtio-blk and AHCI ports,
that's a substantive change. Close evidence must:

- Describe the registry's new shape;
- Confirm existing virtio-blk devices still register correctly
  (anti-regression);
- Confirm the registry's lock and IF discipline are documented at
  the declaration site.

If the registry is NOT generalized (i.e. AHCI has its own
`AHCI_DEVICES` registry), close evidence explains why and notes that
BUILD-5a-NVMe will need a third registry or eventual unification.

### 5.6 Filesystem composition

AHCI block devices appear in the same logical namespace as
virtio-blk devices for filesystem mounting purposes. The
`source = "device3"` (or whatever name is chosen) is the AHCI-
attached disk. FAT32 and NeoFS mount on it without changes to their
own code; that is the substrate-validation test of BUILD-5a-AHCI.

## 6. Verification Gates

Container-canonical idiom:

```text
docker compose run --rm neo bash -c 'qemu-system-x86_64 -version'
docker compose run --rm neo bash -c 'qemu-system-x86_64 -device help 2>&1 | grep ahci'
docker compose run --rm neo cargo k
docker compose run --rm neo cargo k --no-default-features
docker compose run --rm neo cargo fmt -- --check
docker compose run --rm neo cargo neo-test phase3_ahci_controller_detected
docker compose run --rm neo cargo neo-test phase3_ahci_port_identify
docker compose run --rm neo cargo neo-test phase3_ahci_read_sector_pattern
docker compose run --rm neo cargo neo-test phase3_ahci_write_sector_pattern
docker compose run --rm neo cargo neo-test phase3_ahci_multi_sector_io
docker compose run --rm neo cargo neo-test phase3_ahci_block_device_registered
docker compose run --rm neo cargo neo-test phase3_ahci_fat32_mount_and_read
docker compose run --rm neo cargo neo-test phase3_ahci_neofs_mount_and_readwrite
```

Anti-regressions (BUILD-3 and BUILD-4 era scenarios must remain green):

```text
phase3_mount_neofs_device0_readonly
phase3_neofs_readdir_root
phase3_neofs_filecap_generation_stale
phase3_neofs_fsync_filecap_stale_regression
phase3_fat32_mount_readonly
phase3_fat32_readdir_root
phase3_fat32_open_read_regular_file
phase3_fat32_lfn_lookup
phase3_fat32_type_import_is_regular
phase3_ext4_mount_readonly
phase3_ext4_readdir_root
phase3_ext4_extent_chain_read
phase3_ext4_type_import_is_regular
phase3_sched_wake_token_abandon
phase3_vfs_directory_read_is_eisdir
ipc_heap_kill_smoke
cap_revoke_local
```

Final close gate:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Literal `NEO_TEST/1 event=PASS` lines required for every new scenario.

## 7. Stop Conditions

Stop and surface to PM before committing if:

- QEMU ICH9 AHCI emulation is unavailable inside the container.
- Kernel PCI enumeration is hard-coded to virtio vendor IDs and won't
  find an AHCI controller without modification beyond the BUILD-5a
  scope (i.e. requires substantial PCI infrastructure rework).
- Kernel interrupt infrastructure does not support PCI MSI in any
  form (legacy IRQ might be acceptable as a fallback; absence of both
  is a stop condition).
- AHCI requires a `MountedFs` trait change. (It shouldn't — AHCI is a
  block-device backend, not a filesystem — but if you find yourself
  there, surface immediately.)
- AHCI requires modifications to existing filesystem drivers (FAT32,
  ext4, NeoFS). Again, shouldn't; the filesystems should compose
  unchanged. If they don't, that's a substrate gap to surface.
- A new lock site is introduced without explicit IF discipline AND
  explicit lock-type naming at the declaration site.
- The block-device registry generalization (if you do it) requires
  changes to virtio-blk's existing behavior beyond the surface of
  trait registration. Anti-regression must hold.
- New unjustified warnings appear (52 baseline from BUILD-4D).
- Any anti-scope item starts appearing in the diff.

## 8. Required Close Artifact

Create:

```text
Plans/PHASE-3-BUILD-5a-AHCI-CLOSE-EVIDENCE-2026-05-XX.md
```

(Date matches final commit; BUILD-5a-AHCI may span multiple days given
the protocol complexity.)

Required content:

- container-canonical QEMU AHCI verification output (§3 first step);
- commit hash(es) — implementation and close-evidence as in BUILD-4D;
- diff stat;
- build outputs with warning count;
- literal `NEO_TEST/1 event=PASS` lines for all eight new scenarios;
- targeted anti-regression results;
- final `NEO_SMP=4 --no-meta` result;
- AHCI implementation summary (PCI enumeration, HBA init, port init,
  command flow, ATA commands used);
- MSI vs legacy IRQ choice and rationale;
- block-device registry disposition (generalized vs separate);
- explicit naming of any new lock sites WITH lock type AND interrupt-
  state discipline location;
- explicit confirmation: "AHCI implementation did not require
  modifications to the `MountedFs` trait" — or surface immediately;
- explicit confirmation: "AHCI implementation did not require
  modifications to existing FAT32/ext4/NeoFS drivers" — or surface;
- explicit confirmation: "no NVMe code landed; no bare-metal-specific
  code landed; no NCQ/Port Multiplier/hot-plug code landed";
- explicit confirmation: "no AHCI-specific userspace syscall ABI
  landed";
- deviations / course corrections.

Suggested implementation commit subject:

```text
build-5a-AHCI: add AHCI block-device driver for QEMU ICH9 emulation
```

## 9. PM Posture

PM monitoring. Lanes after this dispatch:

- **Jarvis:** BUILD-5a-AHCI (this dispatch).
- **Cerebro:** idle. Available for any unexpected stop-condition triage
  or parallel doc work if PL signals.
- **Prometheus:** idle.

BUILD-5a-AHCI is the longest-lead Phase 3 lane remaining. AHCI is
substantially more complex than ext4 (which itself was meaningfully
more complex than FAT32). PM does not impose a hard timeline; Bible
Article 1.2 (correctness > coherence > performance) governs. Surface
stop conditions immediately.

When BUILD-5a-AHCI closes, PM consumes and dispatches BUILD-5a-NVMe.
When BUILD-5a-NVMe closes, PM holds for PL hardware acquisition before
dispatching BUILD-5b (bare-metal validation).

PL's parallel hardware acquisition (T480s 20L8 path) runs independently;
no current Phase 3 lane is blocked by it until BUILD-5b opens.
