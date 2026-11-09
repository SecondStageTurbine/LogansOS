<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 294 — Janus — PR #7 DMA quiesce review

Reviewed head: `acb98b7f4382b89ffad7175ac2fc121984a89681` (fetched
`refs/pull/7/head`, matching the dispatch). References below are at that head.
Read the PR body and its latest `PM:` comment, including the CP table.

- FAIL — **1. DMA cutoff before IOMMU disable [P1].**
  `kernel/src/drivers/pci.rs:935` returns `()` after writing Command.BME;
  it never reads back whether the bit cleared. `kernel/src/drivers/quiesce.rs:94`
  unconditionally follows these writes with `shutdown_disable` at line 98,
  and line 105 reports the number of planned functions as successfully cleared.
  Only armed disks receive controller stop commands; a network or USB controller
  depends entirely on this unverified BME write. **Concrete input:** fault-inject
  a network function whose Command register continues to read `0x0004` after
  the clear, with its DMA engine still active and VT-d enabled. The host probe
  using the actual PCI clear function and `Plan::run` observed BME still set,
  the IOMMU-disable callback invoked, and
  `bus_masters_cleared=1 iommu_units_off=1`. This permits subsequent DMA after
  translation is removed. Read back and propagate the cutoff result; retain
  translation/protection when cutoff cannot be established, with a defined
  power-off/reset fallback. This is a fault-injection result, not a claim of
  reproduced physical memory corruption.

- FAIL — **2a. NVMe wait is not bounded when the PM timer freezes [P1].**
  `kernel/src/drivers/block/nvme.rs:1017` and line 1024 rely on the supplied
  wait function. `kernel/src/acpi/shutdown.rs:317` supplies `spin_us`, which
  reaches `power_logic::pm_wait` through `kernel/src/acpi/power.rs:473`.
  `kernel/src/acpi/power_logic.rs:261` has no independent poll limit. K4's
  fallback counter belongs to K4's predicate
  (`kernel/src/arch/x86_64/cpu_stop.rs:93`); K6's register predicates do not
  inherit it. **Concrete input:** an armed NVMe controller stays at
  `CSTS.RDY=1, CSTS.SHST=0`, and the present PM timer always reads zero.
  The extracted, unchanged `shutdown_notify` using the actual `pm_wait`
  did not return before an external two-second timeout (exit 124); the same
  controller with an advancing timer returned `Err("shst-timeout")`.
  With the frozen timer, even clearing BME and reaching K7 never happens.
  Add a timer-independent budget to both NVMe waits and preserve the timeout
  result separately from successful completion.

- FAIL — **2b. AHCI/virtio timeout outcomes are reported as success [P2].**
  `kernel/src/drivers/block/virtio_blk.rs:1646` exhausts 100,000 polls and
  returns `()` even if reset never completes. `kernel/src/drivers/block/ahci.rs:611`
  similarly ignores exhaustion of both engine-stop loops; it also clears FRE
  without establishing that CR cleared. `kernel/src/drivers/quiesce.rs:76`
  and line 80 convert these unconditional returns into `Ok(...)`.
  **Concrete inputs:** virtio device status remains `0x0f` for every read after
  status-zero is written; AHCI PxCMD.CR and PxCMD.FR remain set for both
  1,000,000-poll loops. Register-model probes of the actual stop functions
  and `Plan::run` produced `virtio-reset ok` and `ahci-port-stopped ok`,
  respectively. These loops are finite, but the failed devices are not
  identified as failed; the summary also counts every attempted disk as
  stopped (`quiesce.rs:103`). Return explicit stop/reset failures, log them,
  and apply the verified DMA-cutoff fallback from item 1.

- FAIL — **3. K4/K5 integration can deadlock before either K7 action [P1].**
  `kernel/src/acpi/shutdown.rs:245` owns a normal `Plan`, whose heap-backed
  vectors are declared at `kernel/src/drivers/quiesce.rs:33`, line 36 and
  line 38. `stop_world` returns at `shutdown.rs:318`, or at line 297 for
  holdouts, so Rust drops those vectors after K4. K7 is called by the caller
  only afterward (`shutdown.rs:177`). The comment that the plan is never
  dropped because K7 does not return is incorrect. Deallocation acquires
  `IrqSafeHeap.inner` at `kernel/src/mm/heap.rs:140`.
  **Concrete input:** after preparation, an AP acquires the heap lock with
  interrupts off and stalls; K4 parks it by NMI. With a populated plan, the
  shutdown CPU spins on that AP's allocator lock while dropping the plan,
  before either power-off or restart. An AP remaining a holdout with that
  lock has the same effect on the early return. An allocator-counting host
  probe of the actual `stop_world` and `Plan` observed three post-park
  deallocations on both normal and holdout returns. Keep ownership alive
  through the diverging K7 action, or deliberately suppress destruction
  before K4, including on the holdout branch.

- PASS — **3. Explicit register-operation ordering and common restart entry.**
  Preparation precedes `close_for_stop` (`kernel/src/acpi/shutdown.rs:245`,
  line 267); K6 runs after K5 (`shutdown.rs:302`, line 317).
  Holdouts return before either K5 or `Plan::run` (`shutdown.rs:291`).
  Both `Kind::PowerOff` and `Kind::Restart` pass through this same sequence
  (`shutdown.rs:176`). `kernel/src/acpi/power.rs:341` also preserves #6's
  holdout restriction on reset steps using shared PCI ports. These explicit
  branches are correct; the destructor failure above prevents the overall
  shutdown/restart safety check from passing.

- PASS — **4a. Production kill-switch source path.**
  `kernel/Cargo.toml:418` makes `power-quiesce` imply `power-stop-cpus`;
  it is absent from defaults (`kernel/Cargo.toml:81`). The module, new driver
  methods and plan preparation/execution are feature-gated
  (`kernel/src/drivers/mod.rs:45`, `kernel/src/drivers/block/mod.rs:127`,
  `kernel/src/acpi/shutdown.rs:244`, line 316). With `power-stop-cpus` on
  and `power-quiesce` off, the production shutdown remains the previous
  K4/K5 path. Both feature configurations compiled. This is source and
  compile verification, not a binary-identity or QEMU claim.

- FAIL — **4b. CP table exists, but its lock-free and bounded-wait
  claims do not hold for the complete shutdown path.** The PR body contains
  the CP-105, holdout and stuck-judgement rows. The concrete frozen-timer
  and heap-lock inputs in 2a and 3 contradict its first row
  (`kernel/src/drivers/block/nvme.rs:1017`,
  `kernel/src/acpi/shutdown.rs:318`, `kernel/src/mm/heap.rs:140`). The
  holdout row correctly describes skipping K6 register writes, but that
  branch still drops the plan at `shutdown.rs:297`. The stuck-judgement
  guard itself is sound: `kernel/src/drivers/block/mod.rs:430` uses
  nonblocking `Once::get`, and `quiesce.rs:49` skips unarmed disks.
  The named `ot90_stop_wedged` discriminator alone does not enable
  `power-quiesce` (`kernel/Cargo.toml:2115`), so its ordinary passing run
  does not exercise this newly introduced plan destruction.

- PASS — **Validation completed on the reviewed head.**
  `cargo test -p acpi-power-logic-tests --offline`: 35 passed.
  From `kernel/`, `cargo check -Zjson-target-spec --features power-quiesce,vtd --offline`
  and `cargo check -Zjson-target-spec --features power-stop-cpus --offline`
  both passed. Temporary host probes included the actual `Plan` and
  `power_logic` modules and verbatim extracted `stop_world`, NVMe, AHCI,
  virtio and PCI stop functions, with hardware/scheduler/allocator
  instrumentation; their negative results are recorded above. No source
  fixes were made. QEMU scenarios were not rerun: the QEMU executable is
  unavailable in this environment.
