# Phase 1.5c - Pre-Phase-2 Consolidation

| | |
|---|---|
| **Status** | Complete |
| **Started** | 2026-05-03 |
| **Closed** | 2026-05-04 |
| **ADRs landed** | 008 (Phase 1 security close-out) · 009 (governance cleanup + bare-metal deferral) |
| **Commits** | `8847fd4` (BUILD-1) · `6591ab9` (BUILD-2) · `3aa07d1` (BUILD-3) · BUILD-4 close-out in this commit |
| **Next phase** | Phase 2 THINK stage: capability system, SMP, buddy/slab allocators |

---

## What Phase 1.5c Built

Phase 1.5c closed the pre-Phase-2 security and verification debt left after
Phase 1.5b. The phase added a real QEMU integration-test harness, enabled
SMEP/SMAP with bounded STAC/CLAC discipline, enabled executable-image KASLR
with a PIE kernel, closed the Rust 2024 and phase-numbering drift, and recorded
the remaining bare-metal verification gap as an explicit ADR-009 deferral.

Phase 2 now starts from a mechanically gated baseline: `cargo neo-test` boots
one kernel per scenario, verifies structured serial frames plus QEMU
`isa-debug-exit`, and catches both policy regressions and a deliberately red
test.

---

## Original Criteria

The Phase 1.5c close-out criteria from `docs/HANDOFF.md`:

- **ISC-1.5c.1** - QEMU integration-test runner with stable serial frame ABI
  and deliberate-failure meta-test.
- **ISC-1.5c.2** - SMEP, SMAP, and STAC/CLAC production policy enabled and
  tested.
- **ISC-1.5c.3** - KASLR enabled for the kernel executable image with PIE,
  Limine executable-address verification, hardcoded-address audit, and CI gate.
- **ISC-1.5c.4** - Article 17/docs cleanup and phase/security close-out docs.
- **ISC-1.5c.5** - Opportunistic one-line deviations closed if they are truly
  one-line: Rust 2024 edition and stale boot-frame phase label.
- **Anti-criterion ISC-1.5c.A1** - Do not start Phase 2 implementation.
- **Anti-criterion ISC-1.5c.A2** - Do not weaken the six load-bearing
  invariants from the handoff.

---

## BUILD-1 - Test Infrastructure

BUILD-1 established the QEMU integration-test bedrock required by Bible Article
16.2 before the security debt close-out work began.

Landed shape:

- `cargo neo-test` xtask command builds one test-mode kernel per scenario,
  boots QEMU headless, captures serial logs, and requires the expected
  `isa-debug-exit` status.
- Stable serial frame ABI: `NEO_TEST/1 event=<KIND> name=<scenario> [k=v ...]`.
- Default passing scenarios at landing: `ipc_heap_kill_smoke`, `smep_catch`,
  `smap_catch`.
- Ignored meta scenario: `expect_pass_but_fail`, required by the default
  meta-test to prove the runner catches red tests.
- GitHub Actions PR/push gate runs `docker compose run --rm neo cargo neo-test`.

---

## BUILD-2 - SMEP/SMAP Policy Enablement

BUILD-2 closed the SMEP/SMAP/STAC portion of ADR-003.

Landed shape:

- Early x86_64 CPU bring-up enables CR4.SMEP and CR4.SMAP when CPUID leaf 7
  advertises support.
- `with_user_access` brackets deliberate direct user-VA reads with STAC/CLAC
  only after SMAP is active.
- `sys_write_impl` copies user buffers in bounded chunks before serial output,
  so AC is not held across slow I/O.
- `sys_spawn_impl` copies process-name and inherited-handle arrays into
  kernel-owned memory before parsing or handle-table lookup.
- IPC `sys_send` / `sys_recv` remain HHDM-mediated via `Pmap::translate` and
  `phys_to_virt`; no STAC/CLAC is required for that copy path.
- `sys_map_region` reloads CR3 after installing mappings in the active user
  address space, making new heap pages visible before returning to ring 3.
- Default `cargo neo-test` now includes `smep_policy` and `smap_policy`.

---

## BUILD-3 - KASLR Enablement

BUILD-3 closed the KASLR portion of ADR-003. The Limine `kaslr: yes` flag is
now backed by a relocatable kernel image and a CI policy smoke, not just a boot
configuration line.

Landed shape:

- Kernel target builds as PIE (`ET_DYN`) with dynamic relative relocations for
  Limine to apply at load time.
- `main.rs` adds Limine's `ExecutableAddressRequest` and logs the physical and
  virtual executable base during boot.
- Kernel heap placement derives from relocated `__kernel_image_end` instead of
  the old fixed `0xffff_ffff_8800_0000` virtual address.
- Paging sanity checks relocated `_kernel_start` and walks page tables using
  Limine's actual HHDM offset.
- `kaslr_smoke` is part of the default `cargo neo-test` suite and fails if
  Limine reports the old fixed `0xffff_ffff_8000_0000` executable base.
- Production `boot/limine.conf` now sets `kaslr: yes`.

Scope note: current BIOS/QEMU Limine randomizes the executable image but leaves
HHDM fixed at `0xffff_8000_0000_0000`. ADR-008 records this as partial KASLR
scope, not full address-space randomization.

---

## BUILD-4 - Close-Out

BUILD-4 closed the remaining Phase 1.5c documentation and trivial-deviation
items without starting Phase 2.

Landed shape:

- ADR-008 records the Phase 1 security close-out and supersedes ADR-003's
  implementation gate.
- ADR-009 records the bare-metal verification deferral and amends the Bible
  phase ledger, ADR path convention, and license article.
- Workspace edition is now Rust 2024. The 2024 fallout was mechanical:
  `#[unsafe(no_mangle)]`, `#[unsafe(link_section = ...)]`, `unsafe extern`
  blocks, and explicit unsafe blocks inside unsafe syscall/CPU helpers.
- Boot-frame version text now reports `PHASE 1.5c` instead of stale
  `PHASE 1.3`.
- `docs/HANDOFF.md` now points the next coder at Phase 2 THINK/design work
  rather than the already-landed Phase 1.5c builds.

---

## Course Corrections

- Serial output had an interrupt/preemption deadlock risk: a timer interrupt
  could preempt while the serial spinlock was held, then another path could
  spin forever trying to log. The serial writer now disables interrupts while
  holding the UART lock.
- The first IPC test runner shape could falsely pass on `leak_audit=clean`
  even if `neo-init` reported a semantic failure and exited nonzero. The IPC
  scenario now requires `neo-init` exit code 0.
- The Phase 1.5b single-CPU rendezvous invariant was documented but not
  enforced by the scheduler. Rendezvous completion now front-queues the peer
  and yields so the blocked peer resumes before the completing syscall returns.
  Phase 2 SMP must replace this with an explicit barrier or real direct-handoff
  primitive.
- Simplify gate hardening: the runner now rejects duplicate terminal frames
  instead of accepting the last PASS/FAIL it sees.
- A loaded TCG run hit the initial 60s per-scenario timeout before the scenario
  sentinel. The timeout is now 180s so CI fails on missing sentinels rather
  than host slowness.
- BUILD-2 exposed stale local TLB/paging-structure state after
  `sys_map_region` mutates the active Pmap. The production path now flushes the
  local TLB after successful user-region allocation.
- BUILD-2 also exposed serial output in the IPC smoke rendezvous-to-block
  handoff window. The smoke now blocks immediately after `go-block`; diagnostics
  stay outside that load-bearing window.
- BUILD-4 production boot showed that removing serial from the handoff window
  was still not enough under host/QEMU timer timing: a stale PIT tick could
  preempt the receiver before its follow-on blocking syscall. IPC front-wakes
  now grant a bounded receiver handoff quantum, cleared on the receiver's next
  syscall entry or when it blocks.
- BUILD-3 found a hidden fixed-HHDM assumption in the paging sanity check. The
  check now uses the actual Limine HHDM offset passed into `mm::init`.
- BUILD-3 verification shows executable-image KASLR, not HHDM randomization, on
  the current BIOS/QEMU setup. The docs now say that explicitly.
- BUILD-4's Rust 2024 edition bump surfaced unsafe-attribute and
  unsafe-op-in-unsafe-fn requirements. These were fixed directly; the edition
  deviation is closed rather than deferred again.

---

## Verification Log

### BUILD-1

- `docker compose run --rm neo cargo k` - PASS.
- `docker compose run --rm neo cargo neo-test ipc_heap_kill_smoke --no-meta` -
  PASS, with `neo-init exit code 0` and terminal frame
  `event=PASS ... leak_audit=clean exit_code=0`.
- `docker compose run --rm neo cargo neo-test` - PASS. Covers IPC, SMEP, SMAP,
  and deliberate-failure meta-test.
- `expect_pass_but_fail` asserts the runner correctly reports FAIL; this gates
  against silent-CI-pass regressions.
- `docker compose run --rm neo bash -lc 'cargo iso && timeout 60 cargo neo-tty; code=$?; if [ $code -eq 124 ]; then exit 0; else exit $code; fi'` -
  PASS.

### BUILD-2

- `docker compose run --rm neo cargo k` - PASS.
- `docker compose run --rm neo cargo neo-test smep_policy --no-meta` - PASS,
  terminal frame `event=PASS ... cr4_smep=on`.
- `docker compose run --rm neo cargo neo-test smap_policy --no-meta` - PASS,
  terminal frame `event=PASS ... cr4_smap=on`.
- `docker compose run --rm neo cargo neo-test ipc_heap_kill_smoke --no-meta` -
  PASS, verifying SMAP-safe `sys_write` / `sys_spawn` under userland smoke.
- `docker compose run --rm neo cargo neo-test` - PASS. Covers IPC, SMEP/SMAP
  catch tests, SMEP/SMAP policy tests, and deliberate-failure meta-test.
- `docker compose run --rm neo bash -lc 'cargo iso && timeout 60 cargo neo-tty; code=$?; if [ $code -eq 124 ]; then exit 0; else exit $code; fi'` -
  PASS.

### BUILD-3

- `docker compose run --rm neo cargo k` - PASS.
- `docker compose run --rm neo cargo neo-test kaslr_smoke --no-meta` - PASS,
  terminal frame includes `kaslr=on`, a non-fixed executable `virt_base`, and
  Limine-reported `phys_base`.
- `docker compose run --rm neo bash -lc 'readelf -h target/x86_64-altimit/debug/kernel | grep Type && readelf -d target/x86_64-altimit/debug/kernel | head -n 20 && readelf -r target/x86_64-altimit/debug/kernel | head -n 5'` -
  PASS. Kernel reports `DYN (Position-Independent Executable file)` with PIE
  flags and `R_X86_64_RELATIVE` relocations.
- `docker compose run --rm neo cargo neo-test` - PASS. Covers IPC, SMEP/SMAP
  catch tests, SMEP/SMAP policy tests, `kaslr_smoke`, and deliberate-failure
  meta-test.
- `target/neo-test/kaslr_smoke.serial.log` after the default suite recorded
  executable `virt_base=0xffffffff8d028000`, `phys_base=0x1f769000`, dynamic
  heap base `0xffffffff8d400000`, and fixed HHDM `0xffff800000000000`.
- `docker compose run --rm neo bash -lc 'cargo iso && timeout 60 cargo neo-tty; code=$?; if [ $code -eq 124 ]; then exit 0; else exit $code; fi'` -
  PASS under committed `kaslr: yes`.

### BUILD-4

- `docker compose run --rm neo cargo k` - PASS after Rust 2024 and bounded
  handoff-quantum fixes.
- `docker compose run --rm neo cargo neo-test ipc_heap_kill_smoke --no-meta` -
  PASS after the BUILD-4 scheduler handoff correction.
- `docker compose run --rm neo cargo neo-test` - PASS. Covers IPC, SMEP/SMAP
  catch tests, SMEP/SMAP policy tests, `kaslr_smoke`, and deliberate-failure
  meta-test.
- `docker compose run --rm neo bash -lc 'cd /workspace && readelf -h target/x86_64-altimit/debug/kernel | grep Type && readelf -d target/x86_64-altimit/debug/kernel | head -n 20 && readelf -r target/x86_64-altimit/debug/kernel | head -n 8'` -
  PASS. Kernel remains `DYN`/PIE with `R_X86_64_RELATIVE` relocations.
- Production `cargo iso && cargo neo-tty` under committed `kaslr: yes` - PASS
  by host-side log scan requiring no `KERNEL PANIC` / `BUG:`, `neo-init exit
  code 0`, and `frame: leak audit clean`.
- `git diff --check` - PASS.

Bare-metal verification remains deferred by ADR-009. This is the only
documented Article 16.3 gap at Phase 1.5c close.

---

## Anti-Criteria

- **Phase 2 was not started.** Capability-system, SMP, and allocator design are
  the next THINK-stage conversations, not hidden BUILD-4 work.
- **Scheduler classes were not scaffolded.** Article 8.1 remains Phase 2/5
  planning territory; adding placeholder queues without policy would be toy
  work.
- **Full KASLR was not overclaimed.** HHDM fixedness is documented in ADR-008
  and remains a Phase 2 hardware/bootloader revisit point.
- **Bare-metal verification was not faked.** QEMU verification is complete;
  hardware verification is deferred by ADR-009.

---

## Phase 2 Transition Notes

Take a design beat before code. Phase 2 has three interacting subsystems:
capability system, SMP, and buddy/slab allocators. Each needs its own
Q1/Q2/Q3-shaped design conversation and ADR before implementation lands.

The load-bearing invariants from the Phase 1.5b/1.5c handoffs still apply:
do not flatten `Handle`, do not add locks to `Pmap::translate`, do not weaken
the process cleanup ordering, and do not assume single-CPU rendezvous tests
remain valid under SMP without explicit barriers.

Warnings remain pre-existing unless tracked separately; Phase 1.5c did not make
general warning cleanup part of scope.
