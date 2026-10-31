# Dispatch - Phase 3 BUILD-5a-NVMe Driver Development In QEMU

Date: 2026-05-20
Agent: Jarvis
Status: AUTHORIZED IMPLEMENTATION

Authority:

- `Plans/PM-DISPOSITION-2026-05-20-phase-3-build-5a-AHCI-close-consumption.md`
  §5.1 (BUILD-5a-AHCI closed; PM dispatching BUILD-5a-NVMe per standing
  authorization and PL signal 2026-05-20 "go for A")
- `Plans/DISPATCH-2026-05-20-phase-3-build-5a-AHCI-driver-jarvis.md` §1
  (BUILD-5 sub-structure; AHCI then NVMe then bare-metal)
- `Plans/PHASE-3-BUILD-5a-AHCI-CLOSE-EVIDENCE-2026-05-20.md` (substrate
  state: block-device registry is generic; VFS CREATE_REGULAR routing
  generalized; MSI infrastructure proven)
- `docs/ADR-021-phase-3-design-substrate.md` BUILD-5 boundaries
- `docs/bible.md` v1.6.0 Articles 9.9 (storage drivers from Phase 3),
  9.4 (PCI/PCIe enumeration), 16.3 (bare-metal verification gate for
  phase closure)

Substrate:

```text
3da6563  build-5a-AHCI: add AHCI block-device driver for QEMU ICH9 emulation
```

(Plus close-evidence commit `396585a`; both authorized for push by
`PM-DISPOSITION-2026-05-20-phase-3-build-5a-AHCI-close-consumption.md` §1.)

## 1. Mission

Land a working NVMe block-device driver against QEMU's NVMe controller
emulation. The driver is a block-device backend usable by the existing
VFS/MountedFs layer; filesystems mounted on NVMe-attached disks must
work end-to-end without changes to the filesystem code above.

This is the third block-device backend under the substrate (virtio-blk,
AHCI, NVMe). BUILD-5a-AHCI confirmed the BlockDevice registry and
MountedFs trait compose with a non-virtio backend. BUILD-5a-NVMe extends
that result to a meaningfully different storage protocol — NVMe uses
submission/completion queue pairs and a native command set rather than
AHCI's port-based command lists with ATA commands.

After BUILD-5a-NVMe closes, the QEMU-based driver development lane of
Phase 3 is complete and Phase 3 awaits only BUILD-5b (bare-metal
validation) on PL hardware acquisition.

## 2. Pre-Implementation Verification (Required First Step)

Per the canonical container-tooling idiom:

```text
docker compose run --rm neo bash -c 'qemu-system-x86_64 -device help 2>&1 | grep -E "^name \"nvme"'
```

Expected output: `nvme` device available (QEMU has shipped NVMe device
emulation since approximately QEMU 1.6; QEMU 7.2 in the container has
mature NVMe support).

If `nvme` is unavailable, surface as a stop condition (would require
Dockerfile QEMU upgrade lane). PM expects it to be present.

Also verify the existing block-device registry from BUILD-5a-AHCI:

```text
git grep -n "register_device\|get_device" kernel/src/drivers/block/
```

Confirm the registry is the generic `Arc<dyn BlockDevice>` registry
established by BUILD-3 and exercised by BUILD-5a-AHCI. NVMe ports
register through this same registry without modification.

## 3. Scope

### 3.1 In Scope

**NVMe driver against QEMU NVMe controller emulation.**

PCI enumeration:

- Mass storage class 0x01, subclass 0x08, prog-if 0x02 (NVMe);
- Vendor/device ID match against QEMU NVMe (typically `0x1b36:0x0010`)
  is acceptable as a secondary check, but PCI class-triplet match is
  the canonical mechanism;
- BAR0 MMIO mapping (NVMe controller registers; differs from AHCI's
  BAR5).

Controller initialization (NVMe Base Spec §3.5):

- Read controller capabilities (CAP register);
- Disable the controller (CC.EN = 0); wait for CSTS.RDY = 0;
- Configure admin queue: program AQA (queue sizes), ASQ (admin
  submission queue base), ACQ (admin completion queue base);
- Enable the controller (CC.EN = 1); wait for CSTS.RDY = 1;
- Issue admin commands (see below).

Admin queue pair:

- One admin submission queue (ASQ) + one admin completion queue (ACQ);
- Queue depth: ≥ 8 entries for ASQ/ACQ (NVMe minimum is 2; 8 gives
  headroom for initialization commands without contention);
- Physically-contiguous, DMA-coherent allocations via the existing
  `DmaBuffer` allocator.

Admin commands required:

- Identify Controller (opcode 0x06, CNS=0x01) — returns controller
  capabilities, model, firmware version, max queue entries;
- Identify Namespace (opcode 0x06, CNS=0x00, NSID=1) — returns LBA
  format, namespace size in LBAs, sector size;
- Create I/O Completion Queue (opcode 0x05) — creates the I/O CQ;
- Create I/O Submission Queue (opcode 0x01) — creates the I/O SQ
  linked to the CQ;
- Optional: Set Features (opcode 0x09) — number of queues, interrupt
  coalescing. Use if needed to negotiate queue count = 1.

I/O queue pair (one pair sufficient for BUILD-5a-NVMe scope):

- I/O submission queue (NSQ-1) + I/O completion queue (NCQ-1);
- Queue depth: ≥ 16 entries (more than AHCI's queue-depth-1 because
  NVMe is designed for parallelism; but BUILD-5a may still issue
  commands serially through the synchronous BlockDevice API);
- Same physically-contiguous DMA allocation pattern.

I/O commands required:

- NVM Read (opcode 0x02) — for block reads;
- NVM Write (opcode 0x01) — for block writes;
- NVM Flush (opcode 0x00) — for durability barriers (NeoFS fsync path).

Doorbell register access for queue tail/head updates per NVMe spec §3.1.

PRP (Physical Region Page) list construction for command data buffers.
For BUILD-5a-NVMe, PRP1 + PRP2 chained PRPs suffice (no SGL — Scatter-
Gather Lists — needed at this scope).

MSI or MSI-X interrupts:

- NVMe is designed for MSI-X with per-queue vector affinity;
- For BUILD-5a-NVMe, MSI (not MSI-X) is acceptable since only one I/O
  queue pair exists. The kernel's existing MSI infrastructure
  (proven by BUILD-5a-AHCI) is reused;
- If MSI-X is straightforward to wire, that's fine — but not required;
- Same simplification as BUILD-5a-AHCI: completion path may remain
  synchronous polling on the completion queue head pointer. The MSI
  handler acknowledges, but doesn't drive a wake-token rendezvous.
  The forward marker established in BUILD-5a-AHCI §3.4 applies
  identically here.

**Block-device registration.**

NVMe namespaces register through the same `register_device` /
`get_device` registry as virtio-blk and AHCI ports. NVMe namespace 1
becomes device4.

Per BUILD-5a-AHCI close evidence §3.2, the registry is already generic;
no abstraction changes required. If the NVMe driver discovers it needs
registry changes, surface as a stop condition.

**QEMU test harness extension.**

Add an NVMe controller to the QEMU run command. Suggested form:

```text
-drive file=<image>,if=none,id=disk4,format=raw
-device nvme,drive=disk4,serial=neoplutarch-nvme
```

The serial string is required by QEMU's NVMe device. Any value works.

**xtask NVMe disk image generation.**

Generate a deterministic raw disk image for the NVMe test (any
filesystem — FAT32 or NeoFS — populated with a fixture pattern that
scenarios can verify). Same no-binary-fixtures-in-repo discipline as
BUILD-4C/4D/5a-AHCI.

**Scenarios.**

Required new scenarios:

```text
phase3_nvme_controller_detected
phase3_nvme_namespace_identify
phase3_nvme_read_sector_pattern
phase3_nvme_write_sector_pattern
phase3_nvme_multi_sector_io
phase3_nvme_block_device_registered
phase3_nvme_fat32_mount_and_read
phase3_nvme_neofs_mount_and_readwrite
```

Scenario expectations:

- `phase3_nvme_controller_detected` — PCI enumeration finds the NVMe
  controller; BAR0 mapped; admin queue pair created; CC.EN=1 with
  CSTS.RDY=1.
- `phase3_nvme_namespace_identify` — Identify Controller succeeds;
  Identify Namespace for NSID=1 succeeds; LBA format returned;
  namespace size reported.
- `phase3_nvme_read_sector_pattern` — read a known LBA range from
  namespace 1; verify content matches xtask-generated pattern.
- `phase3_nvme_write_sector_pattern` — write a pattern to an LBA
  range; issue NVM Flush; read back; verify content matches.
- `phase3_nvme_multi_sector_io` — read a multi-sector range
  (e.g. 16 sectors = 8 KiB) in a single command; verify content. This
  exercises PRP list construction for multi-page data buffers.
- `phase3_nvme_block_device_registered` — NVMe namespace 1 appears in
  the kernel's block-device registry as device4; the registry can
  dispatch reads and writes through the same `BlockDevice` interface
  used by virtio-blk and AHCI.
- `phase3_nvme_fat32_mount_and_read` — mount FAT32 on device4
  (NVMe-attached disk); verify readdir + open + read.
- `phase3_nvme_neofs_mount_and_readwrite` — mount NeoFS on device4;
  verify initial read, then create + write + fsync + readback through
  NVMe end-to-end. Same shape as the BUILD-5a-AHCI NeoFS composition
  test.

### 3.2 Anti-Scope

NOT in this BUILD:

- Bare-metal validation (that is BUILD-5b);
- NVMe-oF (Fabrics);
- Multiple I/O queue pairs — single SQ + single CQ in I/O scope;
- SR-IOV / NVMe-MI;
- Namespace management commands (create/delete/attach namespaces) —
  use existing namespace 1 only;
- Multiple namespaces — namespace 1 only;
- TCG Opal / SED / encrypted namespaces — refuse to mount encrypted
  namespaces with a clear error;
- Persistent Memory Regions (PMR);
- Controller Memory Buffer (CMB);
- SGL (Scatter-Gather Lists) — PRP only;
- Dataset Management (TRIM / deallocate);
- SMART / Asynchronous Event Notification (AEN);
- Power management state transitions;
- Reset handling (Controller-Level Reset, Subsystem Reset);
- Firmware download/activation;
- Telemetry log pages;
- ZNS (Zoned Namespaces);
- Predictable Latency Mode;
- Endurance Group / NVM Set management;
- Boot Partition;
- Multi-path I/O (ANA);
- Hot-plug;
- NCQ / parallel command issue — synchronous BlockDevice API still
  governs; commands issue and complete serially even with queue depth
  > 1;
- Any change to the MountedFs trait or to existing filesystem drivers
  (FAT32, ext4, NeoFS, AHCI);
- Any change to the AHCI driver from BUILD-5a-AHCI;
- AHCI work of any kind;
- NVMe-specific userspace syscall ABI — the driver is purely a
  backend, no userspace surface;
- BUILD-3F CAS/dedup;
- Bare-metal-specific code paths.

## 4. Architecture Notes

### 4.1 NVMe specification reference

The authoritative spec is the NVMe Base Specification, latest revision
~2.0d at time of writing. NVMe 1.4 features cover everything BUILD-5a-
NVMe needs; newer revisions add features that are anti-scope here
(ZNS, multipath, etc.).

QEMU's NVMe emulation implements NVMe 1.4 features substantially.
Section references in close evidence should cite the NVMe Base Spec
when explaining design decisions.

### 4.2 Memory-mapped queues

NVMe queue pairs require physically-contiguous, DMA-coherent memory.
The kernel's existing `DmaBuffer` infrastructure (used by virtio-blk
and AHCI) handles this. PRP lists for data buffers also require
contiguous DMA-coherent memory.

Queue entry sizes are fixed by NVMe spec: 64 bytes for SQ entries,
16 bytes for CQ entries. Queue depth × entry size gives allocation
size per queue.

### 4.3 Doorbell offsets

NVMe doorbells are at controller MMIO offset `0x1000` plus
`(2 * queue_id + (is_cq ? 1 : 0)) * stride`, where stride is
`4 << CAP.DSTRD`. For QEMU NVMe, stride is typically 4 bytes (DSTRD=0).
Compute doorbell offsets from CAP, not hard-code.

### 4.4 MSI vs MSI-X

NVMe is canonically MSI-X with per-queue vector affinity. For BUILD-5a-
NVMe (single I/O queue pair), MSI suffices and the kernel's existing
MSI infrastructure (proven by BUILD-5a-AHCI) is the natural reuse path.

If MSI-X is straightforward to wire (QEMU NVMe supports it cleanly),
that's acceptable. But MSI is the dispatched minimum. Surface the
choice in close evidence.

Same forward marker as BUILD-5a-AHCI applies: completion may remain
synchronous polling on CQ head with MSI/MSI-X acknowledged but unused
for waiter wakeup. Future async BlockDevice API or NCQ trigger
revisits this.

### 4.5 Lock discipline

The BUILD-4C-established lock-hygiene rule applies:

> Any new lock site must explicitly name lock type AND IF discipline
> location in the close artifact.

NVMe driver almost certainly introduces at least one new lock (for
the per-controller state, possibly per-queue head/tail tracking). For
each new lock, the close evidence must name:

- The lock type (`SwitchMutex<...>`, `spin::Mutex<...>`, etc.);
- The declaration-site IF-discipline comment location;
- The interrupt-state invariant the lock holders must maintain.

### 4.6 Block-device registry reuse

BUILD-5a-AHCI confirmed the existing `register_device`/`get_device`
registry accepts any `Arc<dyn BlockDevice>` implementor without
modification. NVMe namespace 1 plugs in as device4 using the same
pattern.

If the registry needs ANY modification, surface as a stop condition.
PM does not expect modifications based on the BUILD-5a-AHCI result.

### 4.7 Filesystem composition

NVMe block devices appear in the same logical namespace as virtio-blk
and AHCI devices. `source = "device4"` is the NVMe-attached disk.
FAT32 and NeoFS mount on it without changes; the substrate-validation
test extends across three backends.

The VFS CREATE_REGULAR routing was generalized in BUILD-5a-AHCI to
"any mounted root child," so NeoFS-on-NVMe at any mount path works
without additional VFS changes.

## 5. Verification Gates

Container-canonical idiom:

```text
docker compose run --rm neo bash -c 'qemu-system-x86_64 -device help 2>&1 | grep -E "^name \"nvme"'
docker compose run --rm neo cargo fmt -- --check
docker compose run --rm neo cargo k
docker compose run --rm neo cargo k --no-default-features
docker compose run --rm neo cargo neo-test phase3_nvme_controller_detected
docker compose run --rm neo cargo neo-test phase3_nvme_namespace_identify
docker compose run --rm neo cargo neo-test phase3_nvme_read_sector_pattern
docker compose run --rm neo cargo neo-test phase3_nvme_write_sector_pattern
docker compose run --rm neo cargo neo-test phase3_nvme_multi_sector_io
docker compose run --rm neo cargo neo-test phase3_nvme_block_device_registered
docker compose run --rm neo cargo neo-test phase3_nvme_fat32_mount_and_read
docker compose run --rm neo cargo neo-test phase3_nvme_neofs_mount_and_readwrite
```

Anti-regressions (BUILD-3 through BUILD-5a-AHCI scenarios must remain
green):

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
phase3_ahci_controller_detected
phase3_ahci_port_identify
phase3_ahci_read_sector_pattern
phase3_ahci_write_sector_pattern
phase3_ahci_multi_sector_io
phase3_ahci_block_device_registered
phase3_ahci_fat32_mount_and_read
phase3_ahci_neofs_mount_and_readwrite
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

## 6. Stop Conditions

Stop and surface to PM before committing if:

- QEMU NVMe emulation is unavailable inside the container.
- The block-device registry requires modification to accept NVMe
  namespaces — should not happen per BUILD-5a-AHCI result; surface
  if it does.
- NVMe requires a `MountedFs` trait change — should not happen; surface
  immediately if it does.
- NVMe requires modifications to existing filesystem drivers or the
  AHCI driver. Anti-regressions must hold.
- A new lock site is introduced without explicit IF discipline AND
  explicit lock-type naming at the declaration site.
- The kernel's MSI infrastructure proven for AHCI cannot handle NVMe
  for some reason (different vector layout, etc.). Surface; do not
  silently rewire MSI to work around.
- An encrypted-namespace controller is encountered. Surface (refuse
  to mount, document with clear error).
- New unjustified warnings appear (52 baseline from BUILD-5a-AHCI).
- Any anti-scope item starts appearing in the diff.
- QEMU NVMe emulation behaves materially differently from real-NVMe
  spec in a way that would make BUILD-5b bare-metal validation
  diverge significantly. Surface so BUILD-5b can plan for it.

## 7. Required Close Artifact

Create:

```text
Plans/PHASE-3-BUILD-5a-NVMe-CLOSE-EVIDENCE-2026-05-XX.md
```

(Date matches final commit; BUILD-5a-NVMe may span multiple days.)

Required content:

- container-canonical QEMU NVMe verification output (the §2 first step);
- commit hash(es) — implementation and close-evidence as in BUILD-5a-
  AHCI;
- diff stat;
- build outputs with warning count;
- literal `NEO_TEST/1 event=PASS` lines for all 8 new scenarios;
- targeted anti-regression results (including all BUILD-5a-AHCI
  scenarios);
- final `NEO_SMP=4 --no-meta` result;
- NVMe implementation summary (PCI enum, controller init, admin queue
  pair, I/O queue pair, command flow, NVMe commands used);
- MSI vs MSI-X choice and rationale;
- doorbell stride / CAP-derived offset handling explanation;
- PRP list construction summary;
- block-device registry disposition (should be: no change required;
  plugged in unchanged);
- explicit naming of any new lock sites WITH lock type AND interrupt-
  state discipline location;
- explicit confirmation: "NVMe implementation did not require
  modifications to the `MountedFs` trait" — or surface immediately;
- explicit confirmation: "NVMe implementation did not require
  modifications to existing FAT32/ext4/NeoFS/AHCI drivers" — or
  surface immediately;
- explicit confirmation: "no bare-metal-specific code landed; no
  NVMe-oF / namespace management / SED / SGL / PMR / CMB / Dataset
  Management / SMART / power-management / ZNS / multipath / hot-plug
  code landed";
- explicit confirmation: "no NVMe-specific userspace syscall ABI
  landed";
- explicit confirmation: "no AHCI changes landed";
- any divergences between QEMU NVMe behavior and NVMe spec that BUILD-
  5b bare-metal should plan for;
- deviations / course corrections.

Suggested implementation commit subject:

```text
build-5a-NVMe: add NVMe block-device driver for QEMU NVMe emulation
```

## 8. PM Posture

PM monitoring. Lanes after this dispatch:

- **Jarvis:** BUILD-5a-NVMe (this dispatch).
- **Cerebro:** idle.
- **Prometheus:** idle.

This is the last QEMU-based driver lane in Phase 3. After BUILD-5a-NVMe
closes, Phase 3 awaits BUILD-5b (bare-metal validation) on PL hardware
acquisition (T480s 20L8 path).

NVMe is conceptually simpler than AHCI in several ways (cleaner queue
abstraction, less legacy baggage) but introduces queue pair management
and PRP list construction. PM does not impose a hard timeline; Bible
Article 1.2 governs. Surface stop conditions immediately.

The substrate-agnosticism result extends across:

- Phase 3 (this BUILD): three filesystems × three block-device
  backends (virtio-blk + AHCI + NVMe), no MountedFs trait changes,
  no filesystem driver changes.
- Phase 5+ (future): the AI subsystem will mount AI-object storage on
  whatever block-device backend the deployment provides. The
  substrate validation closing through BUILD-5a-NVMe establishes that
  filesystem-and-backend independence is real, not coincidental.

When BUILD-5a-NVMe closes, PM consumes and Phase 3 enters its final
hold-state pending PL hardware acquisition for BUILD-5b.
