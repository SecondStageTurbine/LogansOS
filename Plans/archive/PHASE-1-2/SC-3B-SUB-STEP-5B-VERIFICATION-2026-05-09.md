# SC-3B Sub-step 5b verification

Date: 2026-05-09

Substrate:
- Commit: `2d8f65da77b4ea10f3d53226724e9381961c48c0`
- Parent: `a61e318d93321c9e84732869f5de9da96abe3d5f`
- Subject: `build-3 sub-step 5b: add reschedule IPI infrastructure`
- Backup: `Z:\AltimitNeo\2026-05-09-224332`

## Implementation summary

Sub-step 5b added minimal xAPIC infrastructure:
- `kernel/src/arch/x86_64/apic.rs`: xAPIC enablement, CPU-to-LAPIC-ID registration, EOI, and `send_reschedule_ipi(target_cpu)`.
- `kernel/src/arch/x86_64/idt.rs`: shared IDT vectors `0xfd` (reschedule IPI) and `0xff` (spurious).
- `kernel/src/arch/x86_64/smp.rs`: register Limine LAPIC IDs and enable BSP/AP local APICs.
- `kernel/src/sched/scheduler.rs`: `place_thread_on_cpu` sends a reschedule IPI after remote enqueue succeeds.
- `kernel/src/mm/mod.rs`: `map_kernel_mmio_page` maps the APIC MMIO page, because Limine's HHDM did not cover device MMIO.

No changes were made to `wake_thread_on_current_cpu`, `wake_thread`, or `wake_thread_front`; they enqueue only onto the current CPU by design.

## Course correction

Initial `NEO_SMP=4 cargo neo-test smp_cross_cpu_rendezvous_stress` failed before commit with a page fault:

```text
[ trap  ] PAGE_FAULT (#PF) cr2=0xffff8000fee000f0
NEO_TEST/1 event=FAIL name=smp_cross_cpu_rendezvous_stress reason=unexpected_trap trap=page_fault
```

Cause: xAPIC is MMIO at physical `0xfee00000`; Limine's HHDM maps RAM, not necessarily MMIO holes. APIC SVR access through `phys+hhdm` faulted.

Fix: add `mm::map_kernel_mmio_page(phys)` and map the local APIC page once from BSP APIC init before starting APs. The mapping is global, writable, no-cache, write-through, NX.

## Pre-commit verification

`docker compose run --rm neo cargo k`
- Result: PASS
- Warning count: 18
- Baseline: 18
- Delta: 0

`docker compose run --rm -e NEO_SMP=4 neo cargo neo-test smp_cross_cpu_rendezvous_stress`
- Result: PASS
- PASS frame:

```text
NEO_TEST/1 event=PASS name=smp_cross_cpu_rendezvous_stress iterations=1000 send_first=500 recv_first=500 cpus=4
```

`docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all`
- Result: PASS
- Serial logs: 34
- PASS terminal frames: 33
- Meta-test terminal frame:

```text
NEO_TEST/1 event=FAIL name=expect_pass_but_fail reason=deliberate_harness_failure
```

## Gate: Step 6, NEO_SMP=4

Command shape:

```powershell
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test smp_cross_cpu_rendezvous_stress
```

Artifact directory:

```text
target/verify/sc3b-sub-step-5b-gate/neo-smp4/
```

Result: FAIL, stop condition fired.

Observed:
- Runs attempted: 52
- PASS: 51
- First failure: run 052
- Failure class: `MISSING_TERMINAL_FRAME`
- xtask result: `QEMU timed out after 180s`
- Serial terminal frames: none after BEGIN

Run 052 serial signature:

```text
NEO_TEST/1 event=BEGIN name=smp_cross_cpu_rendezvous_stress phase=dispatch
```

Run 052 xtask signature:

```text
xtask: QEMU timed out after 180s for `smp_cross_cpu_rendezvous_stress`
```

Step 7 (`NEO_SMP=2` 25-run loop) was not run because Step 6 hit the dispatch stop condition. Per dispatch §6.2, this is Outcome B: IPI infrastructure remains landed because it is a standalone-correct architectural fix, but it did not close the SC-3B gate.

## State after verification

- Tracked worktree: clean
- Stashes preserved:
- `stash@{0}: On main: sc-3b-fix-6-1-attempt`
- `stash@{1}: On main: wedge-investigation-8-nmi-watchdog`
- `stash@{2}: On main: wedge-investigation-2-instrumentation`
- `stash@{3}: On main: h1-experiment-instrumentation`

Outcome: B. Open Phase C instrumentation path per dispatch.
