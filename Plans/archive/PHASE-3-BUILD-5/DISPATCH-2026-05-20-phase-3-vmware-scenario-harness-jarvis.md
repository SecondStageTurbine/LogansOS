# Dispatch - Phase 3 VMware Scenario-Harness Follow-Up

Date: 2026-05-20
Agent: Jarvis
Status: AUTHORIZED OBSERVE-FIRST IMPLEMENTATION

Authority:

- `Plans/PHASE-2-AND-3-VMWARE-HYPERVISOR-VM-VERIFICATION-2026-05-20.md`
  (VMware Workstation production-ISO smoke PASS-WITH-SCOPE across 1, 4, and
  8 vCPU configurations; AHCI and NVMe both detected and registered)
- `Plans/PROCEDURE-2026-05-20-phase-2-and-3-vmware-hypervisor-vm-verification-ADDENDUM.md`
  (full scenario/stress sweep deferred because the current harness is
  QEMU-layout-shaped)
- `Plans/HANDOFF-2026-05-20-end-of-day-phase-3-substantially-complete.md`
  (Phase 3 substantively complete in QEMU, BUILD-5b bare-metal still
  hardware-gated)
- `docs/ADR-021-phase-3-design-substrate.md` (Phase 3 storage substrate)
- `docs/bible.md` v1.6.0 Articles 14 (OBSERVE -> THINK -> PLAN ->
  BUILD/VERIFY discipline), 16.3 (bare-metal verification gate), and 17
  (source layout)

Substrate:

```text
eaee02d  plans: add vmware cpu triangulation evidence
aa52579  phase3: integrate build 4B through build 5a storage line
```

## 1. Mission

Turn the VMware production-smoke result into a repeatable path toward the
deferred VMware scenario/stress sweep.

Today's manual VMware run proved the production ISO boots under VMware
Workstation with:

- AHCI detected and registered;
- NVMe detected and registered;
- clean 1/4/8-vCPU boot triangulation;
- clean `neo-init` userspace smoke;
- clean frame leak audit.

The remaining gap is the test harness: `cargo neo-test` currently builds and
runs QEMU-specific scenario invocations, with QEMU fixture disks and device
index assumptions that do not directly transfer to VMware Workstation.

Your job is to inspect that gap and either:

1. implement a minimal, repeatable VMware-compatible scenario-harness path; or
2. file a close artifact explaining the smallest necessary follow-up if the
   implementation is larger than one bounded Jarvis lane.

This is still Phase 3 validation/tooling work. It is not Phase 4 opening.

## 2. Required OBSERVE Pass

Before editing code, inspect and summarize:

- `tools/xtask/src/main.rs`, especially `cargo neo-test`, scenario ISO
  assembly, storage fixture generation, and QEMU launch construction;
- `kernel/src/test_harness.rs`, especially hardcoded device-index assumptions
  in AHCI/NVMe scenarios;
- `kernel/src/fs/mod.rs`, especially mount-source parsing for `deviceN`;
- `kernel/src/drivers/block/mod.rs`, especially block-device registry identity
  and lookup APIs;
- current VMware evidence in
  `Plans/PHASE-2-AND-3-VMWARE-HYPERVISOR-VM-VERIFICATION-2026-05-20.md`.

In the close artifact, include a short "Observed Harness Shape" section before
implementation results. The PM is specifically looking for whether the right
fix is:

- a host-side VMware runner;
- a build-only scenario ISO mode;
- dynamic block-device lookup by driver identity;
- generic `deviceN` mount parsing;
- fixture disk export/conversion;
- or some smaller combination.

## 3. In Scope

### 3.1 Minimal Tooling Path

Add a way to build a scenario-specific ISO without launching QEMU.

Acceptable forms:

- `cargo neo-test --build-only <scenario>`;
- `cargo xtask neo-test-build <scenario>`;
- or another local pattern that fits existing `xtask` style.

Requirements:

- builds the same scenario-specific kernel and userspace artifacts that
  `cargo neo-test <scenario>` would build;
- assembles the scenario ISO at a deterministic path under `target/`;
- emits the ISO path in a machine-readable or copyable way;
- does not require VMware to be installed in the container.

The existing `cargo neo-test` QEMU path must continue to work.

### 3.2 Fixture Disk Export

For storage scenarios, provide a deterministic way to produce the fixture disk
images without launching QEMU.

Acceptable first cut:

- raw fixture disks under `target/neo-test/`;
- a manifest documenting which raw image maps to which intended controller;
- optional VMDK conversion if it can be done with already-available tooling
  (`qemu-img` is available inside the dev container).

Do not commit generated disk images.

### 3.3 Device-Index Robustness

Remove or reduce QEMU-only hardcoded device-index assumptions where doing so is
small and clearly safer.

Current known issue:

- QEMU scenario layout can make AHCI appear as `device3` and NVMe as `device4`
  because virtio fixture disks are attached first.
- VMware production layout observed AHCI at `device0` and NVMe at `device1`.

Preferred direction:

- test harnesses should locate devices by `BlockDeviceIdentity.driver`
  (`"ahci"`, `"nvme"`, etc.) when the scenario is driver-specific;
- mount-source handling should not require enumerating only `device0`,
  `device1`, `device2`, `device3`, `device4` if a bounded generic `deviceN`
  parser is straightforward and does not weaken capability discipline.

If generic `deviceN` parsing touches production semantics in a way that needs
PM/PL architectural review, surface as a stop condition instead of making a
large policy change.

### 3.4 Documentation

If implementation lands, add a concise operator note or procedure fragment
under `Plans/` explaining how PL can run one VMware scenario manually:

- build scenario ISO;
- prepare/export fixture disks;
- attach the right ISO and disks in VMware;
- configure serial log;
- expected `NEO_TEST/1 event=PASS` line.

This does not need to be a polished full procedure for every scenario. One
validated exemplar is enough for this lane if the tooling is new.

### 3.5 Verification

Required local verification:

```text
docker compose run --rm neo cargo neo-test --no-meta
```

If the full no-meta sweep is too slow for turnaround, run the smallest
targeted QEMU subset that covers touched code and explain why the full sweep
was not run.

At minimum, if storage scenario device lookup changes, verify:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_ahci_controller_detected --no-meta
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_ahci_neofs_mount_and_readwrite --no-meta
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_nvme_controller_detected --no-meta
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_nvme_neofs_mount_and_readwrite --no-meta
```

Also verify the new build-only/export command itself.

## 4. Anti-Scope

Do not:

- open Phase 4 networking work;
- change the kernel's user-facing storage ABI beyond a small generic
  `deviceN` parsing cleanup if it is clearly local and safe;
- require VMware Workstation inside Docker;
- commit generated ISOs, raw disks, VMDKs, or serial logs;
- replace the QEMU test path;
- add broad CI requirements around VMware;
- implement a full automated VMware runner unless it is unexpectedly tiny and
  uses already-installed host tooling safely;
- rework filesystem drivers, AHCI, or NVMe behavior except where test harness
  lookup assumptions require a small correction.

## 5. Stop Conditions

Stop and surface to PM if:

- VMware scenario execution requires a large host-side automation framework;
- generated fixture disks cannot be made usable in VMware without manual,
  fragile conversion;
- dynamic device lookup exposes a real production mount-policy decision;
- QEMU tests regress;
- full support requires changing the `MountedFs` trait or `BlockDevice` trait;
- the lane starts expanding into Phase 4 or bare-metal BUILD-5b scope.

## 6. Close Artifact Requirements

File:

```text
Plans/PHASE-3-VMWARE-SCENARIO-HARNESS-CLOSE-EVIDENCE-2026-05-20.md
```

Required sections:

1. Observed Harness Shape
2. Changes Made
3. How To Use The New VMware Scenario Path
4. Verification Commands And Results
5. Residual Gaps
6. Recommendation: proceed to PL manual VMware scenario run, or dispatch a
   follow-up lane

If no code lands, file the same close artifact as a BLOCKED/OBSERVE result with
the concrete smallest next step.

## 7. Expected Outcome

Best outcome: Jarvis lands a small `xtask`/test-harness improvement that lets
PL manually run at least one scenario-specific ISO under VMware with the same
AHCI/NVMe controller setup proven today.

Acceptable outcome: Jarvis proves the smallest correct fix is larger than this
lane and files a precise follow-up plan.

Either outcome converts today's "scenario sweep deferred" caveat into an
actionable engineering path.

