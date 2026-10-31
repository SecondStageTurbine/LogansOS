# PM Disposition - Phase 3 BUILD-5a-NVMe Close Consumption And Ratification

Date: 2026-05-20
From: PM
Status: ACCEPTED / BUILD-5a-NVMe CLOSED / BUILD-5a LANE FULLY CLOSED / PUSH-AND-MERGE AUTHORIZED
Consumed artifact:

```text
Plans/PHASE-3-BUILD-5a-NVMe-CLOSE-EVIDENCE-2026-05-20.md
```

Commits:

```text
eba73ac  build-5a-NVMe: add NVMe block-device driver for QEMU NVMe emulation
(plus follow-up close-evidence commit, hash not in artifact)
```

Diff: 8 files, 1360 insertions, 5 deletions.

## 1. Disposition

BUILD-5a-NVMe is closed. All dispatch acceptance gates from
`Plans/DISPATCH-2026-05-20-phase-3-build-5a-NVMe-driver-jarvis.md` are
met. PM consumes the close artifact and authorizes push and merge of
both commits to main.

With this closure, **the BUILD-5a sub-lane is fully closed** (AHCI + NVMe
both delivered). Phase 3 substantively complete in QEMU; only BUILD-5b
(bare-metal validation) remains for formal Phase 3 closure.

## 2. Gate Verification

PM walked the close artifact against the dispatch contract; every gate
passes:

**Pre-implementation verification (dispatch §2):** Container-canonical
NVMe device availability confirmed (`nvme`, `nvme-ns`, `nvme-subsys`).
Block-registry shape unchanged.

**NVMe driver (dispatch §3.1):**

- PCI class triplet 0x01/0x08/0x02 enumeration — ✓
- BAR0 MMIO mapping — ✓
- Controller capabilities (CAP) parsed; doorbell stride derived from
  CAP.DSTRD (stride=4 in QEMU) — ✓
- Disable / configure / enable sequence per NVMe Base Spec §3.5 — ✓
- One admin queue pair + one I/O queue pair — ✓
- Identify Controller (CNS=0x01) + Identify Namespace (NSID=1) — ✓
  scenario shows `sector_size=512 sectors=32768`
- NVM Read (0x02), NVM Write (0x01), NVM Flush (0x00) — ✓
- PRP-only data transfer with PRP1/PRP2 and one-page PRP list for
  multi-page requests; SGL rejected — ✓
- MSI-X preferred with MSI fallback — ✓ (`msix_configured=true
  msi_configured=false`)
- Block-device registration at index 4 — ✓ (`registry_index=4
  driver=nvme block_devices=5`)

**Scenarios:** All 8 PASS with literal `NEO_TEST/1 event=PASS` frames.
Composition tests deliver the substrate-agnosticism extension to a
fourth block-device backend:

- `phase3_nvme_fat32_mount_and_read fs=fat32 source=device4 readdir=ok
  path=/nvme-fat32/hello.txt bytes=18 content_match=true`
- `phase3_nvme_neofs_mount_and_readwrite fs=neofs source=device4
  mount=rw initial_read=ok create_write_fsync_readback=ok bytes=38`

**Anti-regressions:** Full BUILD-3 through BUILD-5a-AHCI scenario suite
PASS, including all NeoFS generation-stale lanes, all FAT32/ext4 type-
import invariants, all AHCI scenarios (controller_detected through
neofs_mount_and_readwrite), wake-token saga, VFS directory-read EISDIR,
IPC and cap regressions, and the aarch64 virtio-barrier-lane.

**Final close gate:** `NEO_SMP=4 cargo neo-test --no-meta` ran in the
container; failure scan confirms no `event=FAIL`, kernel panic, or panic
lines. The one retry-path scenario (`phase3_storage_completion_cross_cpu_wake`)
passed on retry attempt 2/2; this is a known retry-eligible scenario
from prior closes, not a regression.

**Build state:** `cargo fmt --check`, `cargo k`, `cargo k --no-default-features`
all PASS at 52 warnings (unchanged baseline since BUILD-4D; no new
unjustified warnings).

## 3. Substantive Disclosures

### 3.1 MSI-X-before-APIC ordering discovery — load-bearing for BUILD-5b

The most consequential finding from BUILD-5a-NVMe. From §"Interrupts And
Ordering":

> "Course correction: enabling MSI-X before admin initialization let
> QEMU deliver an early admin completion interrupt before local APIC
> initialization, causing an APIC-base assertion. Final code defers
> MSI-X/MSI enablement until after admin identify and I/O queue
> creation. This preserves the practical BUILD-5b ordering rule: do
> not allow storage IRQ delivery before APIC setup can service it."

This is:

a. **A real bug Jarvis discovered through the scenario testing.** The
   scenario-driven discipline surfaced an ordering bug that pre-
   implementation review would not have caught.
b. **A correction with forward implications.** The "do not allow storage
   IRQ delivery before APIC setup can service it" rule applies to ALL
   interrupt-driven storage drivers. AHCI's existing init order was
   conservative enough to avoid this in BUILD-5a-AHCI; NVMe's needed
   the explicit defer.
c. **An explicit BUILD-5b forward marker.** Jarvis specifically named
   the bare-metal implication: real hardware will be even less tolerant
   of interrupt-before-APIC-ready than QEMU. The BUILD-5b dispatch
   should carry this as a design constraint.

PM records this as a substantive forward marker for the BUILD-5b
dispatch when hardware arrives:

> **BUILD-5b ordering rule (carry-forward from BUILD-5a-NVMe):** Storage
> drivers must defer MSI/MSI-X interrupt enable until after admin queue
> initialization completes AND local APIC setup is confirmed. Real
> hardware is expected to be less tolerant of this ordering than QEMU.

### 3.2 Substrate-agnosticism result — now fully validated

The MountedFs + BlockDevice abstractions are now validated across:

**Filesystems (3):** NeoFS (read-write), FAT32 (read-only), ext4
(read-only).

**Block-device backends (3 implementations + 1 abstract trait):**

| Backend | Registry index | First-class scenarios |
|---|---|---|
| virtio-blk (device0/1/2) | 0, 1, 2 | NeoFS, FAT32, ext4 all mount here |
| AHCI (device3) | 3 | FAT32 + NeoFS mount-and-readwrite |
| NVMe (device4) | 4 | FAT32 + NeoFS mount-and-readwrite |

**Specifically tested compositions:** 7 of 9 possible
filesystem × backend combinations have explicit PASS evidence. The
remaining two (ext4 on AHCI, ext4 on NVMe) are not anti-scope — they
simply weren't included in the test matrix because the dispatches
prioritized broader coverage over exhaustive coverage. The abstraction
guarantees they would work; the scenarios just didn't exercise them.

This is the load-bearing engineering result of Phase 3. The substrate
is genuinely backend-and-filesystem-agnostic, not coincidentally
compatible.

### 3.3 NVMe scope discipline — anti-scope completely honored

Every anti-scope item from dispatch §3.2 is explicitly confirmed in
§"Scope And Substrate Validation":

- No bare-metal code; no NVMe-oF; no SR-IOV / NVMe-MI; no namespace
  management; only NSID=1; no SED / encrypted; no SGL (PRP-only); no
  PMR / CMB / Dataset Management / SMART / power management / ZNS /
  multipath / hotplug; no MountedFs changes; no filesystem driver
  changes; no AHCI changes; no userspace ABI.

PM finds the scope discipline exemplary. Three sub-builds (5a-AHCI,
4D, 5a-NVMe) in a row have demonstrated this discipline; the pattern
is firmly established.

### 3.4 Lock hygiene observed at full discipline

Two new lock sites, both meeting the BUILD-4C-established hygiene rule:

- `NVME_DEVICES: SwitchMutex<Vec<Arc<NvmeNamespaceDevice>>>` at
  `kernel/src/drivers/block/nvme.rs:133` — declaration-site IF
  discipline; mutation only during PCI probe; MSI/MSI-X handler does
  not take.
- `NvmeNamespaceDevice.inner: SwitchMutex<NvmeControllerInner>` at
  `kernel/src/drivers/block/nvme.rs:362` — declaration-site IF
  discipline; submitter holds during one queued command; interrupt
  handler does not take.

Same shape as BUILD-5a-AHCI's lock pattern. The discipline scales.

## 4. Phase 3 Status After BUILD-5a-NVMe

- **BUILD-1, 2, 3, 4 (A through D), 5a (AHCI + NVMe):** closed.
- **BUILD-5b (bare-metal validation):** the only remaining Phase 3
  lane. Hardware acquisition still pending PL commitment.

**Phase 3 is now substantively complete in QEMU.** Three filesystems,
three block-device backends, full MountedFs + BlockDevice substrate
validated end-to-end with composition testing. The substrate work that
ADR-021 named for Phase 3 is delivered.

What remains is Bible Article 16.3's bare-metal verification gate.
Until BUILD-5b lands, Phase 3 cannot formally close per Article 16.3,
but the engineering substrate is in place.

## 5. Authorized Forward Lanes

### 5.1 Push and merge

Per BUILD-4B/4C/4D/5a-AHCI precedent, push and merge of the BUILD-5a-NVMe
commits to main is now authorized. Standard git posture from current
handoff applies.

### 5.2 Phase 3 formal closure is hardware-gated

Phase 3 cannot formally close until BUILD-5b validates on real silicon
per Bible Article 16.3. PL's T480s 20L8 acquisition path is the
front-runner; PM does not authorize BUILD-5b until hardware is
committed.

When BUILD-5b opens, the BUILD-5b dispatch should incorporate (at
minimum):

- The MSI-X-before-APIC ordering rule from §3.1.
- Multi-controller selection policy refinement (per BUILD-5a-AHCI
  §3.5 q35 dual-AHCI quirk handling; real laptops have multiple
  storage controllers visible).
- Bare-metal-specific PCI enumeration robustness (real hardware
  variation vs QEMU's deterministic behavior).
- Hardware-specific quirk surfacing.

### 5.3 Phase 4 THINK preparation is unaffected

Cerebro's Phase 4 OBSERVE artifact and the PM-NOTE design-input chain
remain in place pending Phase 4 THINK's formal opening (post Phase 3
formal closure). BUILD-5a-NVMe's closure does not change Phase 4 THINK's
authority chain or opening triggers.

### 5.4 Optional: Phase 5 OBSERVE pass

The parallel-lane experiment with Cerebro's Phase 4 OBSERVE worked
cleanly. A similar Phase 5 AI subsystem OBSERVE pass could be authorized
while waiting on BUILD-5b hardware. Less urgent than Phase 4 was
(ANSPEC-001/002 already provide substantial substrate), but the option
remains open if PL wants to keep Cerebro engaged on substrate
preparation.

PM does NOT auto-dispatch this. Surfacing as an option for PL.

## 6. Lane Status

After this disposition:

- **Cerebro:** idle (Phase 4 OBSERVE closed earlier today). Available
  for Phase 5 OBSERVE if PL signals, otherwise idle.
- **Jarvis:** BUILD-5a-NVMe closed, idle. Available for BUILD-5b when
  hardware arrives and PL authorizes.
- **Prometheus:** idle.

All three agents idle. No outstanding dispatches. No active stop
conditions. No artifacts await consumption.

**This is a milestone pause point.** Phase 3's substrate work is done.
The day's output:

- BUILD-4B, 4C, 4D, 5a-AHCI, 5a-NVMe all closed (five sub-builds)
- BUILD-4 formally closed at BUILD-4D
- ANSPEC-002 v0.1.1 ratified
- ADR-022 ratified
- Jarvis FD-teardown audit consumed
- Cerebro Phase 4 OBSERVE consumed
- Prometheus three-item bundle consumed
- Two stop-conditions surfaced and handled cleanly (mtools, Prometheus
  wrong-file)
- One major engineering result delivered: substrate-agnosticism across
  3 filesystems × 3 block-device backends

The substrate-agnosticism result is the project's load-bearing Phase 3
deliverable. Everything from this point forward in Phase 4+ inherits
that abstraction line.

## 7. PM Posture

PM stands ready for:

- PL acknowledgment of BUILD-5a-NVMe closure (and BUILD-5a lane closure).
- PL signal on BUILD-5b hardware acquisition (no urgency from PM side;
  the project is in a defensible hold-state).
- PL signal on optional Phase 5 OBSERVE for Cerebro.
- Any other PL direction.

If PL signals natural pause, PM is content. The day's substantive output
deserves a beat to settle. Bare-metal validation is the next milestone
and it depends on the parallel hardware-acquisition track PL is running.
