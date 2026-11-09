<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 302 — Janus — PR #7 re-check

Reviewed head: `fd51d950fbdb58f52498e5e24c1e78768beb594d`; fix:
`80e0960f0d2c25f3d8e6ffb7080d2c24b7528ee1`. Fetched `pull/7/head:pr7`
and read `git diff origin/main...pr7`, the earlier relay-294 RETURN on
`origin/codex/pr7-review-janus`, the current PR body/CP table, and the newest
`PM:` comment (issuecomment-6605801874). All source references below are at
that reviewed head. Result: all earlier FAILs now PASS.

- PASS — **1. DMA cutoff before IOMMU disable.**
  `kernel/src/drivers/pci.rs:952` reads Command.BME back after clearing it;
  a still-set bit or all-ones response is not success.
  `kernel/src/drivers/quiesce.rs:106` counts confirmed clears and failures;
  `kernel/src/acpi/power_logic.rs:319` permits IOMMU disable only with zero
  failed bus masters. `quiesce.rs:119` otherwise keeps translation on through
  K7. Replayed the earlier stuck-BME input against the actual PCI clear and
  Plan code: BME remained set, translation remained enabled, and the summary
  reported `bus_masters_failed=1 iommu=kept`.

- PASS — **2a. NVMe wait with a frozen PM timer.**
  `kernel/src/drivers/quiesce.rs:80` wraps the wait supplied to NVMe with
  `kernel/src/acpi/power_logic.rs:282`: the independent poll cap ends the
  wait and returns the actual completion predicate's result. Both waits in
  `kernel/src/drivers/block/nvme.rs:1017` and `:1024` use it, with explicit
  timeout results at `:1027`. Replayed CSTS.RDY=1, SHST=0 with the real
  `pm_wait` reading constant zero: `shutdown_notify` returned
  `Err("shst-timeout")`. The host KAT at
  `tools/acpi-power-logic-tests/src/tests.rs:1125` also passes, checking the
  exact frozen-timer poll count, advancing timer, and successful completion.

- PASS — **2b. AHCI/virtio timeout reporting.**
  `kernel/src/drivers/block/virtio_blk.rs:1658` returns `reset-timeout` on
  exhaustion. `kernel/src/drivers/block/ahci.rs:628` returns `cr-stuck`
  before clearing FRE; `:636` returns `fr-stuck` if the FIS engine remains
  running. `kernel/src/drivers/quiesce.rs:95` increments `disks_stopped`
  only on success and logs/counts failures separately. Replayed status
  permanently 0x0f and AHCI CR/FR stuck: explicit reset/CR errors, FRE
  preserved, and each Plan summary reported `disks_stopped=0 disks_failed=1`.
  With BME also stuck, both kept the IOMMU on as in item 1.

- PASS — **3. Plan destruction after K4, including holdouts.**
  `kernel/src/acpi/shutdown.rs:249` wraps the prepared plan in `ManuallyDrop`
  before `close_for_stop` at `:271`. Neither the holdout return at `:301`
  nor the normal return after `:321` destroys its vectors. The allocator-
  counting probe of the actual `stop_world` and Plan now observes zero
  post-park deallocations on both paths, closing the earlier three-free
  result. The heap-lock/NMI discriminator is wired at
  `kernel/src/mm/heap.rs:152`, `kernel/src/arch/x86_64/cpu_stop.rs:161`, and
  `tools/xtask/src/main.rs:11245` (requires K6, IOMMU kept, S5 and no holdout).

- PASS — **4b. Prior-art / CP table present and holds for these fixes.**
  The PR body's CP-105 rows explicitly cover the allocator lock, frozen
  timer and holdout; their enforcing sites are `shutdown.rs:249`,
  `power_logic.rs:282`, and `shutdown.rs:295`. The stuck-judgement row still
  holds: `kernel/src/drivers/block/mod.rs:430` uses nonblocking `Once::get`,
  and `kernel/src/drivers/quiesce.rs:49` skips an unarmed judgement. The DMA
  row matches item 1. The bank records the new allocator trap and K6 guards
  at `docs/concurrency-pitfalls.md:4666` and `:4681`, with the new
  discriminators at `:4688`. The PR now names the power-quiesce-enabled
  heap-lock scenario separately from `ot90_stop_wedged`; the latter alone
  still does not cover K6 ownership.

Anything new: no new findings in this narrow re-check.

Validation: `cargo test -p acpi-power-logic-tests --offline` at the reviewed
head: 37 passed. Temporary host probes included the actual Plan/power-logic
modules and verbatim extracted PCI, NVMe, AHCI, virtio and `stop_world`
functions, with register and allocator instrumentation as described above.
These are host-model checks. QEMU is unavailable here; no QEMU or bare-metal
rerun, kernel build, source fix, or independent reproduction of the PM's
reported QEMU revert experiment is claimed.
