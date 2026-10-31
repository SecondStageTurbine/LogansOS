# PM Disposition - Phase 3 BUILD-5a-AHCI Close Consumption And Ratification

Date: 2026-05-20
From: PM
Status: ACCEPTED / BUILD-5a-AHCI CLOSED / PUSH-AND-MERGE AUTHORIZED
Consumed artifact:

```text
Plans/PHASE-3-BUILD-5a-AHCI-CLOSE-EVIDENCE-2026-05-20.md
```

Commits:

```text
3da6563  build-5a-AHCI: add AHCI block-device driver for QEMU ICH9 emulation
396585a  build-5a-AHCI: add close evidence
```

Diff: 8 files, 1336 insertions, 22 deletions.

## 1. Disposition

BUILD-5a-AHCI is closed. All dispatch acceptance gates from
`Plans/DISPATCH-2026-05-20-phase-3-build-5a-AHCI-driver-jarvis.md` are
met. PM consumes the close artifact and authorizes push and merge of
both commits to main.

## 2. Gate Verification

PM walked the close artifact against the dispatch contract; every gate
passes:

**Pre-implementation verification (dispatch §3):** Container-canonical
QEMU 7.2.22 verified; `ich9-ahci` device available; PCI enumeration
confirmed generic (not hard-coded to virtio IDs).

**AHCI driver (dispatch §4.1):**

- PCI enumeration by class 0x01/subclass 0x06/prog-if 0x01 with q35
  dual-controller handling — ✓
- BAR5 ABAR MMIO mapping; GHC.AE before port init, GHC.IE after MSI
  enable — ✓
- Port enumeration via PI; per-port init sequence (stop CMD/FRE, clear
  SERR/IS, program CLB/FB, FRE then CMD) — ✓
- ATA IDENTIFY DEVICE, READ DMA EXT (0x25), WRITE DMA EXT (0x35),
  FLUSH CACHE EXT (0xea) — ✓ (FLUSH CACHE EXT not strictly required by
  dispatch; appropriate engineering addition)
- 48-bit LBA unconditional — ✓
- MSI configured (vector 240 at LAPIC 0xfee00000); no legacy IRQ path
  landed — ✓ matches dispatch §5.3 mild preference for MSI
- DMA buffers via existing `DmaBuffer` allocator — ✓ matches §5.2

**Block-device registration:** AHCI ports register through the existing
generic `BlockDevice` registry (`Arc<dyn BlockDevice>` behind
`register_device`/`get_device`). Registry was already abstract — no
generalization rename needed. AHCI appears as device3 in the registry,
sibling to virtio-blk devices 0/1/2.

**QEMU harness:** `-device ich9-ahci,id=ahci0 -device ide-hd,drive=hd3,bus=ahci0.0`
with deterministic xtask-generated disk image. No committed binary
fixtures.

**Scenarios:** All 8 new scenarios PASS with literal `NEO_TEST/1 event=PASS`
lines. The two composition scenarios deliver the substrate-agnosticism
test:

- `phase3_ahci_fat32_mount_and_read` — FAT32 mounts on device3 (AHCI),
  readdir + open + read works without FAT32 driver changes.
- `phase3_ahci_neofs_mount_and_readwrite` — NeoFS mounts on device3,
  `initial_read=ok create_write_fsync_readback=ok bytes=38`. End-to-end
  read+write+durability through AHCI.

**Anti-regressions:** Full BUILD-3/4A/4B/4C/4D scenario suite PASS,
including all NeoFS generation-stale lanes, all FAT32 and ext4 scenarios,
the wake-token saga, VFS directory-read EISDIR, IPC heap kill smoke,
local cap revocation. Seventeen scenarios validated.

**Final close gate:** `NEO_SMP=4 cargo neo-test --no-meta` exits 0.

**Build state:** `cargo fmt --check`, `cargo k`, `cargo k --no-default-features`
all PASS at 52 warnings (unchanged baseline since BUILD-4D; no new
unjustified warnings).

## 3. Substantive Disclosures

### 3.1 Substrate-agnosticism extended to a non-virtio block backend

The MountedFs + BlockDevice abstractions now compose across:

- **Three filesystems:** NeoFS (read-write), FAT32 (read-only), ext4
  (read-only).
- **Two block-device backends:** virtio-blk (virtual storage), AHCI
  (PCIe SATA controller via ICH9 emulation).

No MountedFs trait change required for AHCI. No existing filesystem
driver change required. The abstraction line held for a meaningfully
different block-device protocol than virtio-blk — AHCI uses MMIO BAR
control registers, port-based command lists, ATA command sets, and PCI
MSI interrupts. The composition test confirms the substrate is
genuinely backend-agnostic, not just filesystem-agnostic.

This generalizes the BUILD-4D result. Phase 3's storage substrate is
validated against the diversity it was designed to handle.

### 3.2 Block registry was already abstract — no rename needed

Jarvis discovered during implementation that the existing block-device
registry (`drivers::block::register_device`/`get_device`) was already
`Arc<dyn BlockDevice>`-based and accepted AHCI ports without
modification. The BUILD-5a-AHCI dispatch §4.1 had anticipated a possible
generalization from `VIRTIO_DEVICES` to a general `BLOCK_DEVICES` registry;
it turned out unnecessary because `VIRTIO_DEVICES` (introduced in BUILD-4C)
was always a virtio-blk-internal device list, not the central block
registry. The central block registry has been generic since BUILD-3.

PM records this for future agent reference: the central block registry
is already filesystem-and-driver-agnostic. BUILD-5a-NVMe should plug into
the same registry pattern AHCI used, without needing to revisit
abstractions.

### 3.3 VFS CREATE_REGULAR routing fix

The NeoFS-on-AHCI composition scenario exposed a small substrate
coupling: `CREATE_REGULAR` was hard-coded to accept `/neofs/...` prefixes
only. Jarvis generalized to "any mounted root child," enabling
`/ahci-neofs/...` to create files.

This is a substrate fix, not a NeoFS driver change. The MountedFs trait
is unchanged. The fix removes a coupling that would have surfaced in
any future build that mounted NeoFS at a non-`/neofs` path. PM accepts
as in-scope; it's the kind of "discovered while implementing" coupling
that the scenario-driven discipline is designed to surface.

### 3.4 MSI installed; completion path remains synchronous polling

The most interesting disclosure. MSI is fully wired: vector 240 at LAPIC
0xfee00000, handler installed at IDT entry, acknowledges HBA and port
interrupt status, ends LAPIC interrupt. **But command completion uses
synchronous polling** on PxCI/PxIS/PxTFD rather than waiting on a
scheduler wake-token rendezvous.

Jarvis justifies this as a deliberate BUILD-5a simplification:

> "the public `BlockDevice` API remains synchronous and all required
> composition tests pass through that API. A future NCQ or asynchronous
> block API would be the trigger for a fuller AHCI completion
> queue/waiter design."

PM accepts this with the forward marker noted explicitly. The reasoning
is sound: the existing `BlockDevice` API is synchronous (single
command in flight per device); the MSI handler cannot benefit downstream
callers if those callers block on synchronous return anyway. Wiring MSI
correctly now serves BUILD-5b bare-metal needs (where polling
correctness is more vulnerable to hardware behavior than under
QEMU emulation) without committing to an async refactor that isn't
needed yet.

**Forward marker for future revisit (PM records explicitly):**

When ANY of the following materializes, the AHCI completion path should
become asynchronous (wake-token rendezvous) to extract the value MSI is
already wired to deliver:

a. The `BlockDevice` trait gains an async or non-blocking variant.
b. NCQ (Native Command Queuing) is added to support concurrent commands
   in flight per device.
c. Phase 5+ AI workloads benefit from concurrent storage I/O while
   the kernel does other work (e.g. model loads overlapping with
   inference).

Until then, polling with MSI-acked-but-unused interrupts is correct and
intentional.

### 3.5 q35 dual-AHCI controller quirk

QEMU's q35 machine exposes both the explicitly-attached `00:06.0` AHCI
controller (with the BUILD-5a test disk) and the built-in `00:1f.2`
AHCI controller (without a disk). The initial probe overwrote the
working-controller snapshot with the no-disk controller. Jarvis fixed
this by preserving the first successful populated controller and
ignoring later no-disk controllers, matching BUILD-5a's single-
controller scope discipline.

PM notes this as a forward marker for BUILD-5b: real laptops typically
have multiple PCIe storage controllers visible (M.2 NVMe slot, SATA
ports, optional eMMC, etc.), and BUILD-5b bare-metal will need a more
deliberate controller-selection policy than "first populated." The
current heuristic is correct for BUILD-5a-AHCI/NVMe in QEMU; it's
explicitly insufficient for bare-metal and BUILD-5b will need to
revisit.

### 3.6 Lock hygiene observed at full discipline

Two new lock sites, both meeting the BUILD-4C-established hygiene rule
verbatim:

- `AHCI_DEVICES: SwitchMutex<Vec<Arc<AhciDevice>>>` — declaration-site
  IF discipline comment; invariant documented (probe-time mutation,
  MSI-time read).
- `AhciDevice.inner: SwitchMutex<AhciPortInner>` — declaration-site IF
  discipline comment; invariant documented (IF=1 hold by submitter,
  MSI handler does not take).

PM finds the lock-hygiene rule has now been exercised twice (BUILD-4C
`VIRTIO_DEVICES` and BUILD-5a-AHCI's two locks). The discipline is
working; no further reinforcement needed.

## 4. Phase 3 Status After BUILD-5a-AHCI

- **BUILD-1, 2, 3, 4 (A through D), 5a-AHCI:** closed.
- **BUILD-5a-NVMe:** ready to dispatch. PM has standing authorization
  per `Plans/DISPATCH-2026-05-20-phase-3-build-5a-AHCI-driver-jarvis.md`
  §9 ("When BUILD-5a-AHCI closes, PM consumes and dispatches
  BUILD-5a-NVMe."). See §5.1 for dispatch posture.
- **BUILD-5b (bare-metal validation):** waiting on PL hardware
  acquisition.

Phase 3 substrate is now ~95% complete by code-substrate count. What
remains is one more QEMU-based driver lane (NVMe) and the bare-metal
validation gate.

## 5. Forward Decisions

### 5.1 BUILD-5a-NVMe dispatch posture

PM has standing authorization to dispatch BUILD-5a-NVMe immediately
upon BUILD-5a-AHCI closure. The chain was authorized by PL signing off
on option II for BUILD-5 sub-structure.

PM elects to **surface to PL rather than auto-dispatch** because:

a. The day has carried substantial output (BUILD-4B, 4C, 4D, BUILD-5a-
   AHCI, plus the doc/spec lane closures). A natural check-in moment is
   appropriate before opening the next major implementation lane.
b. BUILD-5a-NVMe is the last QEMU-based driver lane in Phase 3. After
   it closes, the project pauses for hardware acquisition. PL may want
   to time the dispatch with hardware acquisition or other
   considerations.
c. PL's standing authorization remains valid; PM is choosing to honor
   the spirit (sequential lanes, no auto-chaining mid-day) rather than
   exercise the standing letter unilaterally.

PM stands ready to dispatch BUILD-5a-NVMe on PL signal. If PL prefers
to pause and dispatch tomorrow, that's defensible. If PL says "go," PM
dispatches.

### 5.2 BUILD-5b hardware acquisition

T480s 20L8 acquisition path remains the front-runner. No PM-side action
until hardware is committed.

## 6. Lane Status

After this disposition:

- **Cerebro:** idle.
- **Jarvis:** BUILD-5a-AHCI closed, idle. Available for BUILD-5a-NVMe
  when authorized.
- **Prometheus:** idle.

PM stands ready for:

- PL signal to dispatch BUILD-5a-NVMe (or to pause).
- PL signal on BUILD-5b hardware acquisition.
- Any other direction.
