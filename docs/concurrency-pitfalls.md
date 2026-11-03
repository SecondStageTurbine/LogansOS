<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Concurrency Pitfall Bank — the known SMP/interrupt traps, pre-seeded with discriminators

**Status:** Normative (PM-maintained). **Created:** 2026-07-12 (OT-62, Oracle).
**Trigger:** the PL's insight (2026-07-12) — *"these SMP bugs have already been found and squashed in
other systems; do we not have the answer out there?"* We largely **do**, at the level of
**pattern/discipline**. Our standing trap-mining rule (CLAUDE.md, "Driver / blind-hardware dispatches
require a deep prior-art trap-mining pass") aims at **drivers/hardware**. This document extends the same
discipline to **core-kernel concurrency**: it harvests the documented TLB-shootdown / IPI / lock-ordering /
memory-barrier / scheduler-wake pitfalls into a standing checklist so a future SMP-touching build is
**pre-inoculated against the known classes** and spends real iteration only on the genuinely-ours seams.

## How this bank is consulted (the rule it enforces)

Every dispatch that **touches SMP / interrupt / cross-CPU / lock / atomic code** — the concurrency analogue
of the driver trap-mining rule — MUST, before it can be marked DISPATCHED, **consult this bank** and state,
for each pitfall class its change touches: does the change *introduce*, *guard*, or *leave-N/A* that trap,
and which discriminator proves it. New pitfalls learned on the gate are **added back here** (the bank grows;
it is never pruned). The one-line CLAUDE.md rule this doc proposes:

> **Concurrency / SMP-touching dispatches require a concurrency-pitfall-bank consultation.** Any dispatch that
> adds or changes cross-CPU / interrupt / lock / atomic code must consult `docs/concurrency-pitfalls.md`,
> map each relevant pitfall to *introduces / guards / N-A*, and name the discriminator — exactly as
> driver/blind-hardware dispatches require a prior-art trap-mining pass.

## What this bank IS and IS NOT (bounded honesty)

- **The answer is out there for the PRINCIPLE, not for drop-in code.** Prior-art fixes are welded to *their*
  kernel's primitives (Linux's `mm_cpumask`, FreeBSD's `smp_rendezvous`, their lock types). We made different
  design choices (capability-secured, our own `SwitchMutex`/`IrqSafeMutex`), and our bugs manifest at seams we
  built new — the OT-59 **second** race lives at *our* spawn/ring-3 seam. So each entry is
  **trap + discipline + our-exposure + discriminator + citation**, never a code port.
- **Patterns-known ≠ correctness-proven (the seL4 lesson).** This bank *shortens the search* for the known
  classes. It does **not** replace the adversarial-audit gate (`docs/adversarial-audit-gate.md`), the 4-shard
  QEMU gate, the byte-oracle, or the kill-switch. The genuinely-ours seams still iterate and still get
  verified.
- **Hard licensing rule.** Linux/GPL and other copyleft sources are **read for principle and cited**, never
  copied. Extract the *pattern/discipline*, cite the source location, and **re-derive in our own MPL
  primitives**. This is a learn-the-answer pass, never a lift. (Consistent with ADR-026 / the SPDX rule.)

---

## Quick index

| ID | Pitfall | Category | Our exposure | Armed discriminator? |
|----|---------|----------|--------------|----------------------|
| CP-1 | ABBA / inconsistent lock order (deadlock) | Lock ordering | **Guarded** (I-5 lockdep, but partial coverage) | **Yes — extend (see §Armed)** |
| CP-2 | IF-off spin awaiting a resource this CPU must interrupt-service | IRQ-safe locking | **Was EXPOSED → OT-59 #1** | **Yes** |
| CP-3 | Holding an IRQ-safe lock across a cross-CPU wait | IRQ-safe locking | Was exposed (CP-2 subclass) | Yes (same as CP-2) |
| CP-4 | Shooting down a CPU not running your address space | TLB / IPI | Guarded (T1 broadcast; loosely over-broad, not wrong) | Review-item (T2 gate) |
| CP-5 | Freed-page-table teardown must flush lazy/all CPUs | TLB / IPI | **Guarded** (documented exception, lazy-TLB not yet impl.) | **Yes (capture-before)** |
| CP-6 | INVLPG / paging-structure-cache & MOV-CR3 ordering | TLB / IPI | Guarded (correct sequencing) | Review-item |
| CP-7 | ACK / EOI ordering in the IPI handler | TLB / IPI | Guarded (ACK-before-EOI) | **Yes** |
| CP-8 | Shootdown to a published-but-not-yet-ready CPU | TLB / IPI + Publication | **Was EXPOSED → OT-59 #2** | **Yes** |
| CP-9 | Store→load reordering under x86-TSO (missing fence) | Memory ordering | Guarded (acquire/release used) | Review-item |
| CP-10 | Compiler fence vs CPU fence confusion | Memory ordering | N-A today (no hand-rolled lock-free) | Review-item |
| CP-11 | Wrong `Ordering` on an atomic (Relaxed where Acq/Rel needed) | Atomics | Guarded (audited) | **Yes (grep gate)** |
| CP-12 | ABA on a reused slot / generation reuse | Atomics | **Guarded** (I-13 generation keys) | **Yes (already armed)** |
| CP-13 | Torn / non-atomic multi-word read | Atomics | Guarded (snapshot reads) | Review-item |
| CP-14 | Lost wakeup (park/unpark ordering) | Scheduler / wake | **Guarded** (I-11 token+guard) | **Yes (already armed)** |
| CP-15 | Stranded wakeup — EOI-only reschedule IPI cannot preempt | Scheduler / wake | **Partially guarded** (OT-43/OT-49 lineage) | **Yes** |
| CP-16 | Wake path taken with a foreign lock held (reap→wake) | Scheduler / wake + Lock order | **Under diagnosis → OT-59 #2 sibling** | **Yes** |
| CP-17 | Publication-before-ready (general pattern) | Cross-cutting | Was exposed (CP-8/CP-16 are members) | **Yes (named invariant)** |
| CP-18 | Single-in-flight device: submit→wait→complete not serialized | Driver / concurrency | **Exposed → FIXED (OT-59 #2)** | **Yes** |
| CP-19 | Idle loop missing work-poll hook (OT-34 direct cause) | AP idle / scheduler | **Latent — OT-34 seam** | **Yes (capture-before)** |
| CP-20 | AP parks before cross-CPU spawn completes | AP idle / scheduler | **Exposed — OT-34 hang root** | **Yes (B4 probe)** |
| CP-21 | EOI-only reschedule IPI vs busy AP (CP-15 extension) | AP idle / scheduler | **Partial — latency, not deadlock** | Review-item (latency bound) |
| CP-22 | `sti; hlt` serialization: IPI lost between check and halt | AP idle / scheduler | Guarded (IF-off + atomic sti;hlt) | Review-item |
| CP-23 | MWAIT-based idle loses IPIs (C-state trap) | AP idle / scheduler | N-A today (hlt-only) | **Yes (capture-before)** |
| CP-24 | Cross-CPU spawn pushes to wrong runqueue | AP idle / scheduler | Guarded (lock-free slots bypass it) | Review-item |
| CP-25 | AP busy-yield starves BSP work during partition exec | AP idle / scheduler | Partial — bounded latency | Review-item (latency probe) |
| CP-26 | Partition join barrier races with AP parking | AP idle / scheduler | Guarded (Release/Acquire + parity test) | **Yes (parity oracle)** |
| CP-27 | Reschedule IPI to AP in per-CPU init | AP idle / scheduler | Guarded (readiness barrier) | Review-item |
| CP-28 | Idle re-check race: enqueue between rotate() and hlt | AP idle / scheduler | Guarded (IF-off + sti;hlt) | Review-item |
| CP-29 | Concurrent AP per-CPU heap init contends buddy lock (+ shootdown to IF=0 AP) | AP bringup / per-CPU init | **Fixed (serialize bringup, `c498f5ce`)** | **Yes (bare-metal boot)** |
| CP-30 | Force-zombify frees a blocked thread's stack raw ⇒ owned `Arc`/`Box`/guard on it leaks | Teardown / force-kill | **Was EXPOSED → fixed (OT-61 C1.1)** | **Yes (frame-leak audit)** |
| CP-31 | Off-ring token parking frees a ready slot ⇒ the wake has no re-entry capacity and panics | Scheduler / wake capacity | **Was EXPOSED → repaired (C1.1 fix-forward; re-gate pending)** | **Yes (`token_wake_ring_pressure`)** |
| CP-32 | Per-thread resource leak hidden by a whole-process teardown audit | Teardown / audit vacuity | **Was EXPOSED → repaired (C1.1 fix-forward; re-gate pending)** | **Yes (in-life stack-reclaim probe)** |
| CP-33 | Process kill drains only *some* owning wait queues ⇒ teardown never completes | Teardown / force-kill | **Was EXPOSED → repaired (C1.1 fix-forward; re-gate pending)** | **Yes (all-queue `threads_guard_fault`)** |
| CP-34 | Feature-exclusive root-cap assembly: every single-feature test green, the composed product broken | Capability assembly / cfg | **Was EXPOSED → repaired (C1.1 fix-forward; re-gate pending)** | **Yes (`threads_cap_compose`)** |
| CP-35 | A wake that RELOCATES a thread breaks per-CPU accounting unless it transfers the reservation | Scheduler / cross-CPU migration | **Was EXPOSED → repaired (re-gate pending)** | **Yes (`smp_c11_migration_reservation`)** |
| CP-36 | Ready→Running window is invisible to BOTH kill scans; generation checks ownership, not lifetime ⇒ UAF | Teardown / cross-CPU force-kill | **Was EXPOSED → repaired (re-gate pending)** | **Partial — see the 2(b) gap** |
| CP-37 | Non-IRQ-safe spin lock taken from BOTH a preemptible and an IF=0 context ⇒ unbreakable same-CPU deadlock | Lock discipline / mm page tables | **Was EXPOSED → repaired (re-gate pending); EXPOSED again by the `net-tick`/`net-rx` threads (IF-on vs syscalls, then IF-off vs the IF-on registry takers, 2026-10-29) → net steps IF-off, registries `IrqSafeMutex`; by construction in OT-97** | **Yes (bounded-spin owner-naming panic on `Pmap::inner`; `socket_teardown` with `wire-rx`)** |
| CP-38 | Hand-written IRQ entry stub saves GP regs only; the SSE-compiled Rust body clobbers the interrupted kernel context's xmm | Interrupt entry / FPU state | **Was EXPOSED → repaired (re-gate pending)** | **Yes (`irq_xmm_preserve`)** |
| CP-39 | Interrupt stub enters a SysV Rust body with RFLAGS.DF still set from ring 3 ⇒ `rep movs`/`stos` run backwards | Interrupt entry / ABI state | **Was EXPOSED → repaired (re-gate pending)** | **Yes (`irq_xmm_preserve`, DF leg + timer leg)** |
| CP-40 | Interrupt stub or exception handler entered with RFLAGS.AC still set from ring 3 ⇒ SMAP suspended for the whole handler | Interrupt + exception entry / SMAP | **Was EXPOSED → repaired (re-gate pending)** | **Yes (`irq_xmm_preserve` AC leg + timer leg; `exception_entry_flags`; `threads_guard_fault` ring-3 #PF leg)** |
| CP-41 | Entry stub saves the user's FPU image but runs kernel SSE code on the user's MXCSR ⇒ user-triggered #XM halt, user-chosen rounding/FTZ/DAZ in kernel results | Kernel entry / FP environment | **Was EXPOSED → repaired (re-gate pending)** | **Partial — syscall entry, both save paths (`syscall_mxcsr_entry`, `_fxsave`), `swapgs_irq_entry` and the timer stub (`irq_xmm_preserve` MXCSR leg + timer leg); init/page-fault resets unchecked; the new-thread image is checked by CP-45 (`thread_entry_clean_state`)** |
| CP-42 | `stac`/`clac` asm marked `nomem` ⇒ the compiler moves user accesses out of the SMAP window (and kernel accesses into it) | SMAP window / inline-asm options | **Latent (only optimized builds; none is built today) → repaired** | **No — review item (no gate builds an optimized kernel)** |
| CP-43 | Syscall return path never reloads `rcx`/`r11` ⇒ every syscall hands ring 3 kernel scratch in them | Syscall entry / privilege boundary | **Was EXPOSED → repaired (re-gate pending)** | **Yes (`syscall_return_clean_regs`)** |
| CP-44 | Entry path reads per-CPU state through GS before swapping to the kernel GS ⇒ `gs:[24]` hits user address `0x18`, nested `#PF` recursion, `#DF` | Interrupt entry / GS state | **Was EXPOSED → repaired in `page_fault` (re-gate pending); the `syscall_entry` exit window is CP-50** | **Yes (`threads_guard_fault_observability`)** |
| CP-45 | A new thread's first register state is copied from its creator's live registers ⇒ kernel values (and kernel pointers) reach ring 3 | Thread creation / privilege boundary | **Was EXPOSED → repaired (re-gate pending)** | **Yes (`thread_entry_clean_state`, `_fxsave`)** |
| CP-46 | Error-code exception handlers (`x86-interrupt`) call Rust at RSP ≡ 8 (mod 16) ⇒ a callee's `movaps` to a stack slot raises `#GP` (the ring-3 `#PF` kill with a timed futex waiter halts the CPU) | Exception entry / ABI state | **Was EXPOSED → repaired (re-gate pending)** | **Yes (`threads_guard_fault_timed`; `exception_entry_flags` realign leg)** |
| CP-47 | SFMASK leaves RFLAGS.NT set ⇒ the kernel runs syscalls with NT=1, its own `iretq` #GPs in ring 0, and NT crosses `context_switch` | Syscall entry / RFLAGS | **Was EXPOSED → repaired (re-gate pending)** | **Yes (`syscall_nt_masked`)** |
| CP-48 | DPL3 32-bit code segment + unprogrammed IA32_CSTAR ⇒ compatibility-mode SYSCALL enters ring 0 at RIP 0 on the user RSP and GS (plus the LDTR and SYSENTER_CS it relied on firmware for) | Syscall MSRs / GDT / LDT | **Was EXPOSED → repaired (re-gate pending); the error-code handler stack misalignment found alongside is CP-46, repaired** | **Yes (`compat_mode_unreachable`)** |
| CP-49 | SMAP window helper that does not nest and must not span a thread switch ⇒ a nested call's `clac` closes the outer window; a blocking closure hands AC=1 to the next thread | SMAP window / scheduler interaction | **Latent → guarded (no caller nests or blocks today)** | **Yes (`user_access_nesting`); blocking case partial** |
| CP-50 | Syscall exit reaches its `swapgs` with IF=1 ⇒ an interrupt between `swapgs` and `iretq` runs at CPL 0 on the user GS (CP-44's recursion) | Syscall exit / GS state | **Was EXPOSED → repaired (re-gate pending)** | **Yes (`syscall_exit_irq_window`, under `-icount`)** |
| CP-51 | A control event (escape chord, release) delivered on a best-effort `try_lock` path ⇒ contention silently drops it after the decoder already consumed it | Input routing / HID pump | **Was EXPOSED (S1-F1, Jarvis F1) → repaired: per-device stage + lock-free SAK latch** | **Yes (`hid_contention`)** |
| CP-52 | A lost button-up leaves pointer grab state stale ⇒ the next press routes to the old target | Input routing / pointer | **Designed out (B-ref: no kernel grab; staging; RESYNC record)** | **Yes (`hid_contention` buttons leg; ring RESYNC KATs)** |
| CP-53 | One-shot authority object with no reclaim owner ⇒ a restarted holder can never re-acquire it, or the first caller wins it when the gate is fail-open | Capabilities / input authority | **Designed out (exact non-delegable `InputSystem`; reclaim bumps the session generation)** | **Yes (`di1_forward` reclaim; Arthur EPERM probe)** |
| CP-54 | A bounded primary operation whose result needs an uncapped secondary lookup ⇒ the claimed bound is false | Bounds / input routing | **Was EXPOSED on main (#46, Jarvis F3: mint/forward scanned an uncapped, user-growable route vector) → repaired: client routes capped at `INPUT_ROUTE_CAP` (32); the pointer route is outside the cap** | **Yes (`di1_forward` route_cap leg)** |
| CP-55 | Work deferred across an authority change is bound to the authority current at *completion*, not at *capture* ⇒ data captured for the old owner reaches the new one | Input routing / HID staging | **Was EXPOSED (#44, Jarvis F1) → repaired: staged batches carry the activation epoch; mismatch is discarded** | **Yes (`hid_contention` handoff leg)** |
| CP-56 | An IF discipline applied to the inner lock but not to the lock acquired just before it ⇒ the outer spinlock is still mixed IF=1/IF=0 (CP-37 one level out) | HID drivers / `PRIMARY` | **Was EXPOSED (#44, Jarvis F2) → repaired; EXPOSED again via `InputFocus::drop` in teardown (#55) → `DELIVERY`/`PRIMARY`/device locks are `IrqSafeMutex` (IF=0 by construction). Blind spot: `Drop` impls and teardown paths.** **EXPOSED again on the VFS file syscalls (#120 audit) → `sys_fsync`/`open`/`readdir`/`ftruncate` bodies IF-on; and on `MOUNTS` via the unmount `Drop` → displaced filesystems dropped after release (`MountsGuard`); and on `ARENA` (inference IF-on vs tensor syscalls IF=0) → `IrqSafeMutex`** | **Yes (`hid_contention` mixed_if + teardown legs, witness counters + two controls; `phase3_neofs_syscall_if_on` + `neo-test-vfs-syscall-if-off-control`; `phase3_umount_drop_outside_mounts` + `neo-test-mounts-drop-under-lock-control`; `arena_if_off` + `neo-test-arena-mixed-if-control`)** |
| CP-57 | A server-initiated send on a synchronous-rendezvous channel ⇒ the server parks until the client receives, and deadlocks against a client parked sending to it | IPC / desktop configure | **Designed out (DI-1b: the configure is published in a seqlock mailbox; the client polls and acks)** | **Yes (`desktop_resize_boot`; mailbox KATs)** |
| CP-58 | A periodic rescuer (a tick, a deadline, a poll) ends a wait whose real waker is missing ⇒ the test passes, slower, while the wake path is broken | Wait multiplex / any event-plus-timeout wait | **Guarded (OT-22: the IPC-only wait leg must end within 5 ticks of the send (a missing fire costs ~300); the control compiles the send-path fire out and must time out)** | **Yes (`wait_any_core` ipc leg + `wait-multiplex-no-ipc-fire`)** |
| CP-59 | A hidden spinner on the CPU (a housekeeping yield loop) keeps it out of idle ⇒ every parked thread looks like it still spins, and an idle witness reads 0 | Scheduler / idle accounting | **Was EXPOSED (OT-22: the kernel's wait-for-init loop yield-spun on the BSP; `idle_ticks` counted `tid == u64::MAX`, which never happens) → repaired: the loop sleeps a tick with `wait-multiplex`; idle = running the idle thread** | **Yes (`desktop_idle_parks` ≥ 80% vs `desktop_idle_spins` ≤ 5%)** |
| CP-60 | A "kernel" value captured lazily from live CPU state (`Lazy::new(Cr3::read)`) ⇒ captured from whatever context first asks, here a user process's PML4; kernel threads then run on it, and its death frees a live page table | Scheduler / address spaces | **Was EXPOSED on main (found by OT-22's `wait_any_core`) → repaired: `kernel_cr3()` is the boot capture from `mm::init`; `Pmap::drop` asserts no CPU still has the freed PML4 active** | **Yes (`token_park_teardown_churn` + `neo-test-kernel-cr3-lazy-control`)** |
| CP-61 | A wait names a mutable slot (a numeric handle) but registers on the object the slot held when it was scanned ⇒ a close-and-reuse or a revoke changes what the slot names without firing the registered key, and a wait with no rescuer sleeps forever | Wait multiplex / handle tables | **Was EXPOSED (OT-22 rung 1, Jarvis F1) → repaired: the second scan compares each item's key + handle `CapId` with the registration's; a `HandleTable` removal fires the removed handle's key** | **Yes (`wait_any_core` rebind + revoke legs; `wait-multiplex-no-identity-check`, `wait-multiplex-no-handle-fire`)** |
| CP-62 | Boot-owned memory handed to an allocator while a `'static` borrow of it can still be minted, or from a memmap type the boot protocol does not align or de-overlap ⇒ a later borrow reads frames now owned by someone else, or the allocator coalesces pages that were never its own | Allocator / boot modules | **Prevented by design in B1a (Chad C2):** consumable modules are only ever `take`n once as a lease and never lent as `'static`; reclaim consumes the lease; pages are planned from the module's exact range, inward-rounded, type-6-only, and disjoint from the executable and every other module; admission makes a separate zone and never calls `free()` | **Yes (`boot_module_reclaim` + `tools/module-reclaim-logic-tests`)** |
| CP-63 | One shared deep kernel stack (the inference stack) entered by more than one thread at once, code that assumes `rsp` lies in the thread's own stack, or a spin lock held across a seconds-long syscall ⇒ silent frame corruption, a misread stack range, or every waiter spinning (at `IF=0`, with no ticks or IPIs) behind a preempted holder | Inference stack / locking | **Guarded by design in B1c:** a yielding lock with an interrupt window per contended round; `rsp`-outside-own-stack audited safe (no walker or range check; `TSS.rsp0` only on ring-3 entry); mapped once at boot; 64 KiB guard → `#DF` plus a canary; force-kill per CP-65 | **Yes (`inference_stack_overflow`, `real_lm_generate_boot`, host KATs in `tools/module-reclaim-logic-tests`)** |
| CP-64 | A kernel section that runs masked longer than the TLB-shootdown deadline while its CPU is still in the shootdown-ready mask (a long syscall body that never reopens interrupts after `SFMASK`) ⇒ the next shootdown from another CPU waits on an ACK that cannot come and panics (`tlb_shootdown_timeout`); on the BSP, timekeeping stops too | Interrupts / TLB shootdown | **Was EXPOSED in B1c (Jarvis J1, cross-family) → repaired:** the inference stack's closure runs `with_interrupts_enabled` and restores the caller's `IF`; `inference-stack` implies `xsave-fpu`; locks taken inside are IRQ-safe | **Yes (`inference_stack_shootdown`: 16/16 ACKed while the owner is parked; control `inference_stack_shootdown_control` panics)** |
| CP-65 | A thread suspended inside the kernel (Ready mid-syscall: yielding for the inference stack, or a preempted holder) is force-killed and zombified ⇒ the reaper frees its stack as raw bytes, so its `Arc<Model>`, prompt `Vec` and tensor `Arc`s are never dropped (CP-30), and a marker cleared before the boundary exit opens a window where terminate finds the thread neither marked nor queued | Teardown / inference stack | **Was EXPOSED in B1c (Jarvis J3, J4) → repaired:** a per-thread lifecycle marker (`Thread::inference_lifecycle`) set at the arm's entry and cleared after the boundary exit decision; the kill path never zombifies a marked thread; cancellation is cooperative (`ECANCELED`, checked in the `BUSY` loop, after winning it, and between tokens); CP-66 composes the same predicate with the admission marker | **Yes (`inference_stack_kill`, incl. the release race; control `neo-test-inference-kill-control` leaks the waiter's model `Arc`)** |
| CP-66 | A thread force-killed inside a cross-process admission queue (parked behind the holder, or the holder preempted mid-dispatch) is zombified raw ⇒ its stack-owned request `Arc`s leak (CP-30), and the queue later admits the dead principal, which never calls `complete` ⇒ the shared device wedges for every later caller | Teardown / accelerator admission | **Was EXPOSED on main (J3-P, Jarvis, B1c recheck 2) → repaired:** the admission section is marked on the thread and never zombified from a queue; `admission::cancel_pid` withdraws a dying process's requests and wakes them with `ECANCELED`; the caller unwinds and exits at the `SYS_INFER_RUN` boundary; a torn-down process cannot park; `complete` never admits a principal whose wake finds no thread | **Yes (`admission_park_kill`, `admission_park_kill_threaded`; control `neo-test-admission-park-kill-control`)** |
| CP-67 | A blocking IPC syscall keeps an owned `Arc<Channel>` (and, for a grant, a cloned source handle) in its frame across the endpoint park ⇒ a terminate or fault-kill extracts and zombifies the waiter, the reaper frees the stack raw, and one `Channel` (or the granted model) leaks per kill | Teardown / IPC | **Was EXPOSED on main (found in CP-66's audit) → repaired:** `with_channel_anchor` moves the channel into `Thread::ipc_anchor` for the call (dropped on return or in `Thread::free`); `sys_grant` drops its source before parking | **Yes (`ipc_park_kill`: rendezvous + recv/send/grant/recv_grant kill legs; control `neo-test-ipc-park-kill-control`)** |
| CP-68 | The kernel assumes xAPIC: an MP request without the x2APIC flag, `IA32_APIC_BASE` rewritten with x2APIC cleared, every register as MMIO, an 8-bit IPI destination, MSI hard-wired to APIC ID 0 ⇒ on firmware that keeps x2APIC on, Limine refuses to boot; driven by hand, the mode switch #GPs and MMIO accesses vanish | Interrupts / APIC | **Was EXPOSED on real silicon (PL laptop, B1 ISO) → repaired:** x2APIC is requested; the mode comes from the BSP's MSR; x2APIC registers go through MSRs; the ICR is one 64-bit write after `mfence; lfence`; `IA32_APIC_BASE` is never rewritten; an AP in the wrong mode halts; MSI targets the BSP's ID and is refused above 255; `xapic-only` is the kill-switch | **Yes (`x2apic_smp_ipi` on `-cpu max,+x2apic`; control `xapic-only`; `tools/apic-logic-tests`)** |
| CP-69 | The kernel tick is trusted to exist: the PIT through 8259 IRQ0 on the BSP is the only timer, and nothing checks the first tick arrives ⇒ on client silicon that gates the 8254 or has no 8259→LINT0 route, nothing preempts and every timed wait hangs; a LAPIC timer mode trusted without its first tick (TSC-deadline cannot run under TCG) has the same failure | Interrupts / timer | **Exposed on the Zenbook target (predicted, NOTE §4a) → guarded:** `lapic-timer` ticks from the LAPIC (TSC-deadline, else periodic, calibrated from CPUID 0x15 or against the HPET / ACPI PM timer / PIT, cross-checked, 0x16 only an estimate) in both APIC modes; the HPET is taken over before use, a hardware-reduced FADT has no PM timer, a PIT window needs both OUT2 edges; each armed mode must tick within five periods (waits sized from a calibrated rate or a 10 GHz bound) or it is disarmed and demoted (deadline → periodic → PIT); the EOI follows the LAPIC's ISR bit, not a flag; feature off is the PIT/PIC tick | **Yes (`lapic_timer_tick`, `_x2apic`, `lapic_timer_demote`; controls `neo-test-lapic-timer-control`, `neo-test-lapic-timer-no-verify`, `+ neo-test-lapic-timer-pit-gated`; `tools/lapic-timer-logic-tests`, `tools/acpi-timer-logic-tests`)** |
| CP-70 | An exception vector reachable from ring 3 whose handler halts the machine (or kills a process) without first routing on the saved CPL ⇒ any user program can halt the whole OS with `ud2`, a divide by zero, `hlt`, or a set RFLAGS.TF; and a kill path that runs before `swapgs` reads per-CPU state through the user GS | Privilege boundary / forced termination | **Was EXPOSED (every vector but `#PF`, and `#PF` itself without `browser-runtime`) → repaired (OT-83):** every routed vector calls `contain_user_fault` first; CPL 3 ⇒ `swapgs` then the process kill (exit 128 + signal); CPL 0 ⇒ the unchanged halt; `CR0.NE=1` so x87 errors arrive as `#MF` | **Yes (`ring3_fault_{ud,de,gp,db,mf}`; control `ring0_fault_halts`; host KAT `tools/xtask/src/ot83_tests.rs`)** |
| CP-71 | A log flush that waits on the log-sink lock (`SINK`, a plain spin mutex taken at IF=1 by every kernel print) from an IF=0 path or with another lock held ⇒ a printer preempted on the SAME CPU while holding `SINK` can never run to release it; the IF=0 spinner wedges the CPU and every shootdown to it times out (CP-64). Found as the admission withdrawal trace's `log::flush()` under `QUEUES` on the ring-3 exception kill | Logging / IF=0 lock wait | **Was EXPOSED (OT-83 F1, Jarvis) → guarded:** the exception kill runs in a per-CPU stage-only log section (lines staged under `try_lock` or dropped, counted; no flush, no block I/O); admission traces are stage-only on every path; the durable write is `log::flush_pending` (try-lock only) after `QUEUES` drops and in the reaper | **Yes (`admission_park_kill_sink_held`; control `neo-test-admission-park-kill-sink-held-control` must hang)** |
| CP-72 | An ISR for an EDGE-triggered line that leaves the device's interrupt condition asserted when it returns (it stops early with data still pending, or the condition was already true when the line was enabled) ⇒ no new edge ever arrives, the device goes silent, and nothing fails loudly. Found as the i8042 ISR capped at 16 bytes per pass against QEMU's 16-byte PS/2 queue | Interrupt delivery / edge-triggered lines | **Was EXPOSED (i8042 lane, own scenario) → guarded:** the ISR drains until the output buffer is empty (a runaway cap, counted); a drain right after enabling; a polled fallback in the pump that is counted (`polled=`) so a dead IRQ is visible | **Yes (`ps2_keyboard`: hung 3 of 6 before the fix, 8/8 after; it requires every byte by IRQ, `polled=0`)** |
| CP-73 | A loop that alternates IF=0 sections back to back (an `IrqSafeMutex` section, then a `without_interrupts` yield, repeated) with no instruction run at IF=1 between them ⇒ the `sti` that ends one is followed directly by the `cli` that starts the next, and the STI interrupt shadow means no interrupt is recognised in between: device IRQs and the tick starve for as long as the loop spins | Interrupt delivery / IF windows | **Was EXPOSED (the `ps2_keyboard` scenario's wait loop) → guarded there:** an explicit IF=1 spin between passes. Production pumps are called once per syscall, not looped | **Yes (`ps2_keyboard`: about half the runs starved without the window on QEMU 8.2.2, 18/18 with it)** |
| CP-74 | A long kernel computation inside a syscall at IF=0 (the boot-model load: BLAKE3 over 485 MB, then the model build) on the caller's 32 KiB kernel stack, which is a heap box with no guard page ⇒ CPU 0 takes no tick and no device interrupt for the whole call (the desktop and the pointer freeze), and BLAKE3 alone needs 39,536 B of stack at 485 MB, so the call overruns the box into the heap silently and can hang for good. Small QEMU models fit (18,560 B at 382 KB) and hide it | IF=0 sections / kernel stacks | **Was EXPOSED (Zenbook S14: `cpu0 ticks/s=0` for 29 s+, over 15 min once) → guarded under `boot-model-preload`:** the model is built once at boot, before userland, on the inference stack (256 KiB, guard page, interrupts on); `sys_model_load` then hands out a shared handle. Without the feature the old in-syscall load remains | **Yes (`boot_model_load_ticks` 20 ticks + co-runner during the load; its control shows 0 and 0; `boot_model_load_stack` measures the depth)** |
| CP-75 | An ISR for a LEVEL-triggered line that EOIs (or unmasks) while the device still asserts it ⇒ the interrupt is delivered again the moment the ISR returns, for as long as the condition holds, and nothing else runs on that CPU; the same storm follows from a wrong polarity (an idle line reads as asserted). The opposite of CP-72 | Interrupt delivery / level-triggered lines | **Guarded at introduction (I2C-HID touchpad lane):** the ISR masks the I/O APIC entry and only signals; the service thread reads the report, then unmasks (a still-asserted line re-fires at once, no lost wake); 16 assertions in a row with no data leave the line masked and switch to polling, printed `irq masked-storm` | **Partly (host KAT `line_governor_masks_a_storm_and_stops_on_errors`; the line is real only on the Zenbook: console `irq masked-storm` / `empty=`)** |
| CP-76 | The I/O APIC's `IOREGSEL`/`IOWIN` index/data pair written by two contexts that can interleave (an ISR on the same CPU, or another CPU) ⇒ one writer's data lands in the other's selected register: a redirection entry half-programmed, unmasked, or pointed at the wrong vector | Interrupts / I/O APIC MMIO | **Guarded at introduction (I2C-HID touchpad lane):** after boot only one entry is written at run time, by a single-writer-per-phase protocol: the ISR writes only while the pin is unmasked (the only time it can fire), the thread only while it is masked, with IF=0 on its CPU; every write selects the same index | **Review item (no QEMU device asserts GSI 0x69)** |
| CP-77 | A DMA master granted Bus Master Enable before its ring bases are programmed (it fetches a garbage base), or a driver that trusts the Bus Master Enable firmware left set (a UEFI PXE stack) instead of clearing it ⇒ a device the containment decision refused can still DMA | DMA ordering / I-14(c) | **Guarded (RTL8125 rung 1):** bind clears BME before touching the chip; one `grant_bus_master`, only for an allowed mode, after both rings are programmed | **Yes (`rtl8125_absent`: pre-set BME comes out clear, control with the opt-in FAILs `bus_master_set`; `rtl8125_vtd`: clear after bind, set only by the grant)** |
| CP-78 | Device-shared descriptor ring ordering: buffer/length read passing the OWN-bit read (RX consumes a frame still being written), OWN visible before the address (device fetches a stale address), doorbell MMIO passing the descriptor stores (device misses the new descriptor) | Memory ordering / device rings | **Guarded (RTL8125 rung 1):** acquire after each OWN-clear read, release before every OWN store, full fence before the `TX_POLL` doorbell, all counted | **Partial (host: `publish_writes_own_last`, ring model tests; silicon: the `link=up rx=0` / `tx=wedged` failure-bank rows)** |
| CP-79 | A per-CPU capability (CPUID + XCR0) decided once on the BSP and used on every CPU: an AP with different CPUID or XCR0 runs an instruction its own check rejects (`#UD`) | Cross-CPU publication / CPU-local state | **Guarded (Arthur latency fix 2):** every CPU reports its own check into one atomic census; "yes" only when all online CPUs reported and none vetoed; a late "no" still downgrades; undecided keeps the per-call local check | **Yes (host: `avx2_census_requires_every_cpu`, `avx2_census_veto_wins_under_concurrency`, `avx2_census_global_agrees_with_local_check`; QEMU: `threaded_matmul_parity` with `NEO_SMP=4` + `phase9-avx2-matmul`, `avx2=No (census: 4/4 cpus reported)` on qemu64, `avx2=Yes (census: 4/4 ...)` on `-cpu max`)** |
| CP-80 | A wait or grace window counted in `PAUSE`/spin iterations: its real length is CPU-dependent (`PAUSE` is ~10 cycles on some cores and ~140 on Skylake-and-later Intel cores), so a bound sized "sub-millisecond" on one CPU is ~0.1 s on another, paid on every fork-join job that has a busy AP | Timeouts / fork-join | **Guarded (Arthur latency round 2, `matmul-steal-grace-time`):** the par_matmul steal grace is 50 us of TSC time from the job's publish (spins only when the TSC rate is unknown); the fail-stop `BARRIER_LIMIT` stays a spin bound (a backstop, not a latency) | **Yes (host: `steal_grace_is_time_when_calibrated_else_spins`, `steal_grace_cycles_rounds_up_without_truncation`; QEMU gate: `threaded_matmul_parity_grace_time`, a held AP slot stolen no earlier than a test-set 500M-cycle grace, parity byte-identical)** |
| CP-81 | A kernel service loop that idles with a "nap" whose timed-sleep path is compiled out (here `wait_any::sleep_ticks`, only with `wait-multiplex`) ⇒ the fallback is `yield_now` + spin every tick: an always-runnable thread for the life of the system, competing with every other thread on its CPU | Scheduler / idle waits | **Was EXPOSED (I2C-HID touchpad, image A5) → guarded:** in IRQ mode the service thread parks on a wake token the ISR takes (`PENDING` re-checked under `WAITER` before parking, CP-8/CP-14), a stopped service parks until stopped. **Also EXPOSED in `usb::nap_ticks` (the USB port service, the boot input check) → guarded by `tick-sleep` (image A):** a timed park (deadline + wake token, expired by the timer tick), used by both naps without `wait-multiplex`; a full table falls back to the yield loop and is counted | **Yes (QEMU `usb_hotplug`: 20-tick sleep parks, wakes after exactly 20 ticks, past deadline does not park, every nap parks (`spins=0`); control `neo-test-tick-sleep-control` FAILs `tick_sleep_not_parked`; overlay `tp: loops=` and `sleep spins=`)** |
| CP-82 | A `spin::Once` (or any lazy-init cell) whose initializer blocks or can be preempted (here the write guard's first-write judgement, which does block I/O) ⇒ a second caller at IF=0 on the initializer's CPU spins in `call_once` forever (CP-37 with a `Once` for the lock) | Block write guard / lazy init | **Was EXPOSED on main (`arthur_memory_forgetting` 900 s timeouts, #119 gate) → guarded:** only the `judging` CAS winner runs the judgement; other callers wait on `get()` with interrupts on, yielding (the io_gate discipline) | **Yes (`block_write_guard` judge-contention leg + `neo-test-write-guard-judge-spin-control`)** |
| CP-83 | Per-thread FS.base restored selectively on switch-in (only for threads that set TLS, or skipped by a per-CPU "last written" cache) ⇒ a thread runs on the previous thread's FS.base, possibly another process's. Ring 3 can change the hardware FS.base without the kernel: a `mov fs, <selector>` loads the descriptor's base (0 for the flat user selector) even with CR4.FSGSBASE off, and `WRFSBASE` would once FSGSBASE is on, so any kernel copy of the hardware value goes stale | Context switch / TLS (OT-61 C1.2) | **Guarded (corrected 2026-10-28, C1 review fixes):** every switch into a user thread and every `SYS_THREAD_SET_TLS` writes `IA32_FS_BASE` from `Thread.fs_base`, with no cache; the per-CPU cache was removed after Jarvis's review showed a selector load made it lie | **Yes (`threads_tls` checks 2, 3, 7 and 8; with the cache restored, check 8 fails 8024 (setter) or 8034 (switch-in))** |
| CP-84 | A single-thread fallback for per-thread state (here the initial thread's static TLS control block, used before FS.base is installed) that a second thread can also reach ⇒ two threads silently share one key block; the switch to per-thread must happen before the second thread exists, on EVERY thread-creating path | TLS / thread creation (OT-61 C1.3) | **Guarded (new seam):** `tls::ensure_initial_installed` flips the process to `fs:[0]` once, after FS.base is set; every create wrapper (std `Thread::new`, libneo `sys_thread_create` and the test-only create-on-CPU) calls it first and creates nothing if it fails; a child installs its own block before any code | **Yes (`threads_tls` check 6; `std_threads` checks 6–8)** |
| CP-85 | A process-exit syscall that retires only the calling thread once a process has several ⇒ a worker's `exit`/abort leaves the process alive, siblings running and its joiner parked forever on a record that never completes | Thread teardown / exit (OT-61 C1.3) | **Guarded (new seam):** under `browser-runtime`, `SYS_EXIT` with live siblings runs the fault kill's sibling drain (`retire_process_from_current_thread`) with the caller's code; a single-threaded exit is unchanged | **Yes (`std_threads` third spawn: a worker panics with a futex-parked and a spinning sibling → exit 134; with the hook removed, the run times out)** |
| CP-86 | A process retire (fault kill or exit group) that drains only Ready, parked, IPC- and wait-parked siblings ⇒ a sibling *Running in ring 3 on another CPU* at that instant is never stopped (APs get no timer tick, and the reschedule IPI is EOI-only), so the process never finishes and its `sys_wait`er hangs | Thread teardown / SMP (OT-61 C1.1 fault kill, C1.3 exit group) | **Guarded (C1.3b; reopened 2026-10-27 after Jarvis's review found two survivors, re-closed 2026-10-28 by the C1 review fixes):** each CPU publishes the pid it runs; the retire IPIs every CPU running a sibling; a thread of a retiring process ends at every ring-3 boundary (syscall entry, the timer and reschedule IPI from CPL 3, and a final syscall-return check with interrupts off through IRETQ, CP-93); the futex park refuses a retiring process under the purge's lock, and a kernel join that gets that refusal returns to the syscall boundary. Liveness only: memory was already safe (each thread's own Arcs, freed after it is off-CPU) | **Yes (`threads_exit_group` roles 1–6: without the IPI role 1 hangs, without the futex refusal role 3, without the syscall-return check role 4, with the join's ESRCH discarded role 5, without the `cli` before the final check role 6)** |
| CP-87 | **Revoke walk vs concurrent install** (pre-existing, found 2026-10-27). A revoke that walks the derivation logs, then removes, with no lock spanning both ⇒ a child installed in between (a derive, a grant, a spawn inheritance or any install naming a parent) is not in the walk's set and outlives the revoke | Capability graph / SMP (ADR-010 revoke) | **Was EXPOSED → repaired (K1a-0): `CAP_GRAPH` (a FIFO ticket lock) spans every revoke's walk and removal; every install naming a parent re-checks it under the lock; spawn checks inherited parents and registers the child under it** | **Yes (`cap_graph_revoke` race-derive, race-grant and race-spawn legs, and the two-CPU grant leg; `neo-test-cap-graph-unlocked-control`)** |
| CP-88 | **Laundering by close or exit** (pre-existing, found 2026-10-27). A derivation edge stored with its holder and deleted on close or exit ⇒ the cap's children lose their path to every ancestor, and a revoke of the ancestor misses them (derive D″ from D′, close D′, and D″ outlives `revoke(D)`; the same when D′'s holder exits or is killed) | Capability graph (ADR-010 revoke, Q4 survive-exit) | **Was EXPOSED → repaired (K1a-0): a closed or torn-down cap with children re-parents them to its nearest live ancestor before its edge goes** | **Yes (`cap_graph_revoke` close, exit and kill legs; `neo-test-cap-graph-unlocked-control`)** |
| CP-89 | **A failed spawn keeps what it charged** (new, OT-61 C2.2). A budgeted spawn charges the child's PML4, image and stack before the child is registered; an error path that frees the frames but not the charges shrinks every account above it for good | Memory accounts (I-18, `mem-quota`) | **Guarded: the charges live in the child `Pmap`'s ledger; every error path drops the `Pmap`, whose `Drop` (and the ledger's own drop) releases exactly what it holds** | **Yes (`mem_quota` step 5; falsified by leaking the ledger: init 29 frames short)** |
| CP-90 | **Two CPUs charging one account chain** (new, OT-61 C2.2). A read-check-write charge lets siblings on two CPUs both pass a parent's limit check; a partial charge must undo exactly its own levels; a release must never wrap | Memory accounts (I-18, `mem-quota`) | **Guarded: one CAS (`fetch_update`) per level that refuses past `limit` or `u64::MAX`; a refused level undoes the levels below it; a release is `checked_sub`, refused and counted on underflow. No lock (CP-37/CP-56 N/A)** | **Yes (host hammer KAT; `mem_quota` role 4 at NEO_SMP=4; falsified by charging only the leaf: the parent underflows and the `neo-test` check panics)** |
| CP-91 | **A last `Arc` dropped under a global IF-off lock runs a shootdown** (Janus F1 on #166). A `CAP_GRAPH` scan's `Arc<Process>` can be the last (the reaper drops its own outside the lock); its `Pmap` drop waits on a TLB shootdown that a CPU spinning for the lock never answers | Capability graph (`CAP_GRAPH`); CP-56's hidden `Drop` | **Repaired: `CapGraph` pins scanned `Arc`s; after the unlock a last one is handed to the reaper (`Arc::into_inner`), never dropped in the caller's IF-off block** | **Yes (`cap_graph_revoke` F1 leg + the `Pmap::drop` witness; falsified)** |
| CP-92 | **A park under a ticket lock** (Janus F2 on #166). A last tensor release under `CAP_GRAPH` can park on a Reserved slot with interrupts-off waiters behind it | Capability graph; AI reservation park | **Repaired: removals return `Detached`; tensor counts are released after the unlock** | **Yes (`cap_graph_revoke` F2 leg + the `release_tensor` and park witnesses; falsified)** |
| CP-93 | **A flag check before returning to user, with interrupts still on** (found 2026-10-27, Jarvis's C1.3b review). The kernel checks "should this thread stop?" and then returns to ring 3 with IF=1 for a while ⇒ an IPI sent to make it stop lands in that window, sees CPL 0, EOIs and does nothing; the thread returns to ring 3 and, on a CPU with no tick, never comes back | Syscall return / SMP (OT-61 C1.3b) | **Guarded (C1 review fixes):** `syscall_dispatch` disables interrupts before its final `retire_current_if_dying`, and nothing re-enables them before SYSRET/IRETQ, so a retire IPI is either seen by the check or delivered in ring 3 | **Yes (`threads_exit_group` role 6; without the `cli`, it hangs)** |
| CP-94 | **A heap grown in place, assuming the next map lands at its top** (found 2026-10-29). A user heap that extends one region by every fresh `SYS_MAP_REGION`, relying on the per-process cursor to make the map abut its top ⇒ once anything else maps in between (a thread stack and its guard page, a buffer plane), the heap writes its allocator metadata into that mapping: a ring-3 #PF on a guard page, or silent corruption of a live stack or buffer | User heaps / threads (OT-61 C2.3a, `neo-std-pal`; libneo's DI-1b twin) | **Was EXPOSED → fixed (C2.3a):** `SyscallHeap` spans up to 64 regions; a map extends the last region only if it starts at its top, else it opens a new one; dealloc goes to the region that owns the pointer; growth is geometric. libneo's `LibneoHeap` already did this | **Yes (`servo_host_smoke` heap step after two thread spawns; with the old heap it dies 139)** |
| CP-95 | **A force-kill of a thread that is Ready inside a syscall** (found 2026-10-29, OT-93). The kill zombified any Ready thread, and the reaper frees its stack raw ⇒ a thread Ready mid-syscall (preempted, yielding, woken, or a rendezvous sender after `handoff_to_rendezvous_peer`) leaks whatever its frames own; the IPC sender holds both pmaps | Thread teardown / SMP (`kill_thread_by_pid`; terminate, fault kill, exit group) | **Guarded (OT-93):** `Thread::in_syscall` (entry to return decision, under the scheduler lock); the scan zombifies only a thread that resumes in ring 3; one inside a syscall, or running, exits at its syscall return on `Process::force_killed`, a user-wait park it would start is refused with `ESRCH`, and `terminate` IPIs the CPUs running it (ring-3 hooks in every build). CP-65/CP-66 markers kept for parked threads; a wait on the victim completes only at its last thread's reap (`DYING`). **Not bounded:** a survivor in a device wait that never completes (OT-99) | **Yes (`kill_ready_in_kernel`; with `neo-test-kill-ready-in-kernel-control`, `frame leak (+50)`; with `neo-test-kill-wait-early-control`, `wait_before_survivor`)** |
| CP-96 | **An authority checked only by a right the holder can mint itself** (found 2026-10-29, the L2 pre-merge AAG, F1). Moving the keyboard route needed only `ACTIVATE` on a focus handle, and any process could create a focus and mint itself an `ACTIVATE` sibling (meant to be granted to the WM) ⇒ any client could take the route and read every key typed: an intent, a sign-in or lock password | Input routing / capabilities | **Was EXPOSED on main → fixed:** `SYS_INPUT_FOCUS_ACTIVATE` also requires the caller to hold `InputSystem` (non-delegable, kernel-minted into the session root); the root holds it wherever the focus syscalls exist | **Yes (`desktop_pointer_boot`: Arthur creates, mints and self-activates, and gets `EPERM`)** |
| CP-97 | **A security mode held only in a user-space server's own state** (found 2026-10-29, the L2 pre-merge AAG). The session lock was the WM's: on Super+L the WM swapped the keyboard route to its own focus. A WM parked in a synchronous send (CP-57: a reply to a client that never receives it) runs no code ⇒ the chord sits in the latch, the route stays the client's, and keys typed after Super+L reach the client | Input routing / lock (OT-66 L2b) | **Guarded (kernel `sak-lock-route`):** the lock is a kernel route state. The WM registers its focus as the lock focus; Super+L sets the locked bit in the latch itself (lock-free, before any WM code runs); while it is set every route read is the lock focus, so a client receive that starts after it gets nothing. A receive already past its check re-checks after its pop, and nothing a client's queue held across a lock survives the unlock (the lock epoch). Only the input-system holder releases it | **Yes (`login_lock_park`: the WM parked in a reply, Super+L, then `x`; Arthur never gets it. Falsified: without the kernel half, Arthur gets `x`. `lock_route_races`: Super+L forced between each route check and its effect (Jarvis #160 items 1, 6, 7); each leg falsified by reverting its fix)** |
| CP-98 | **A kill that assumes its target is Ready or parked** (found 2026-10-29, S1 in A1a; OT-98). A force-kill claims the process and then looks for its thread in the scans it knows (ready queues, channel and token parks), asserting it found one ⇒ a target in a state no scan covers trips the assertion, which the product kernel keeps on: the kernel panics. States on main: a thread that called `sys_exit` and waits unreaped in a zombie ring (the natural exit claimed `exit_state` only at the reap), a thread parked in `sys_wait` (in the *waited-on* process's queue, which may be in `DYING`: FAIL-A), a waiter in transit from that queue to Ready (FAIL-B, Janus #174 F2), and a target running on another CPU (S1's SMP=4 panic). With the assertion off, the first loses the exit code and the second leaves a killed thread parked until its target exits. Also: a losing exit producer overwrote the winner's exit code (Janus #174 F1) | Thread lifecycle / kill (`terminate`) | **Was EXPOSED on main → fixed (OT-98):** `record_exit_code` claims `EXITING_NATURAL` at the commit point and only the winner stores the code; `terminate` extracts a `sys_wait`-parked target from `PROCESSES` and `DYING`; the completion wakes under the `waiters` lock; after its scans `terminate` recounts survivors and finds a thread that left the kernel meanwhile (zombie ring, the reaper's mark, the thread count); `close_peer` wakes under the endpoint guard; the kill completes by thread count (#204), and a thread no scan finds is a counted diagnostic, not a panic; the waiter parks owning nothing (CP-30); last `Arc`s under `SYS_KILL`'s IF-off go to the deferred drop (CP-91). A running target is #204's survivor (CP-95) | **Yes (`kill_at_exit`: legs E, C, exited, wait, D, A, B, F, G, F1, race; E, F and G kill from an AP; controls `-a`…`-d`, `-f`, `-g` fail `kae_kill_unlocated` in their legs, `-e` hangs leg E's wait, `-control` fails `kae_kill_of_exited_not_esrch`, `-f1` fails `kae_f1_loser_changed_code`)** |
| CP-99 | **A kernel copy to or from user memory with no fault recovery** (found 2026-10-31, the PM on main). `validate_user_buf` checks only that a range is in the user half; the copy then ran as a plain `copy_nonoverlapping` inside the SMAP window. A page that is unmapped, read-only, or unmapped by another thread of the process between the check and the copy faults in ring 0, and the page-fault handler halts on every kernel-mode fault ⇒ any process stops the machine with one syscall and a bad pointer | Syscall user copies / page-fault entry | **Was EXPOSED on main → fixed (`user-copy-fixup`):** every syscall user copy goes through `arch::x86_64::user_copy`, one `rep movsb` at a known RIP. The page-fault shim, for a ring-0 fault at that RIP on a user-half CR2, rewrites the saved RIP to the fixup and `iretq`s, and the copy returns EFAULT. Any other kernel-mode fault still halts. A concurrent unmap is the same case: the TLB shootdown completes before `vm_unmap` returns, so the copying CPU faults and recovers. Futex's compare is a second registered instruction (one aligned 4-byte load); the IPC copy uses the routine on the caller's side and, on the peer's, a permission-checked walk (`translate_user`) under the peer's `inner` lock held until the copy is done (`with_user_frame`), so the peer's sibling can't free the frame mid-copy; the test tail hook checks its word in the page tables when armed and writes it through the routine. **No committed state on EFAULT, for these paths:** SYS_READ moves the offset only after its copy; SYS_INFER_RUN closes the handles it installed; SYS_THREAD_JOIN writes the status while it still holds the exit record; SYS_SOCKET_RECV (TCP and UDP) and SYS_SOCKET_RECVFROM copy out before the bytes leave the socket queue; SYS_NET_TCP_ENABLE_TLS wipes the key before installing TLS. **Outside that guarantee:** SYS_INPUT_FOCUS_RECV and stdin SYS_READ take an event or byte that can't be put back; they check the buffer in the page tables first, but a sibling's unmap after the check loses it | **Yes (`user_copy_fault`: SYS_HANDLE_INFO and SYS_READDIR (unmapped, crossing and read-only outputs; a read-only cursor), SYS_WRITE (unmapped and crossing inputs), SYS_READ (offset kept), futex (unmapped, and unmapped under the queue lock), IPC (read-only, unmapped and crossing buffers, both directions; and a sibling on a third CPU unmapping the peer's page mid-copy, which must wait: with the lock released early it fails `ipc_unmap_race result=3`), join (still joinable), TLS key (EFAULT, a half read-only key kept), UDP receive (datagram kept); `threads_exit_group` role 6 arms the tail hook on an unmapped word; control `neo-test-user-copy-fault-control` halts on the first. The futex leg removes the page deterministically; the receive and join legs use static bad buffers, not a concurrent unmap)** |
| CP-100 | **An XSAVE area sized from CPUID.(0DH,0):EBX alone** (found 2026-11-02, the preview under WHPX). EBX is "the size for the features enabled in XCR0", so it is only right if whoever answers CPUID tracks the guest's XCR0. QEMU under WHPX (`kernel-irqchip=off`) answered 576 (x87\|SSE) with XCR0 = x87\|SSE\|AVX ⇒ every `xsave` wrote AVX state 256 bytes past the area: in the timer stub, over the saved registers and the iretq frame copy, so the first LAPIC tick's `iretq` took a #GP(0) and the boot froze; per-thread save buffers on the heap overran the same way | Interrupt entry / FPU state (`fpu::finish_xsave_init`) | **Was EXPOSED → fixed:** the area is the largest of EBX, 576, and each enabled component's offset + size from CPUID.(0DH,i) (`fpu_logic::xsave_area_size`); an understated EBX is logged (`cpuid_0d_ebx=576 (understated; …)`) | **Yes (`xsave_ebx_understated`: EBX forced to 576, the YMM/XMM/x87 context-switch legs must pass with area 832; control `neo-test-xsave-ebx-understated-control` (EBX trusted) takes the same #GP at `timer_isr_entry+0xe0`; host KATs `fpu-logic-tests`)** |
| CP-101 | **Withdrawn (2026-10-31).** Filed in 27ce30f0 as a loopback-pump reordering race (two CPUs pop `OUTBOUND` in order, then inject out of order) behind the `fetch_broker` lost request. A per-segment trace then showed every segment delivered in order; the real cause was the TLS fixture server dropping a request decrypted during its handshake (fixed in `libneo::tls`, dcf91989). The `LOOPBACK_PUMP` lock was removed. The shape is still possible in principle but has not been observed | Network / loopback TCP (`tcp::pump_loopback_egress`) | **Not guarded; unproven hazard**, no change | **No (the discriminator it cited was the fixture bug)** |
| CP-102 | **A check against a route epoch, then an act that reads it again** (found 2026-11-02, Jarvis's review of #61). Key auto-repeat checked the held key's activation epoch, then staged the repeat with `stage_event`, which stamps the stage with the epoch read *then* ⇒ a focus change or Super+L between the check and the stage (another CPU) gives a repeat checked for the old owner to the new owner or the lock screen | Input routing / key auto-repeat (`key-autorepeat`) | **Guarded:** the repeat carries the epoch it was checked under (`key_repeat::poll` returns it); `stage_event_at` drops it if the epoch has moved and otherwise stamps the stage with *that* epoch, so a move after staging is caught by the flush's comparison under `DELIVERY` (where every route change bumps it). The legacy routed path compares under `DELIVERY` (`route_event_at`). Pure decision: `key_repeat_logic::stage_action`, host-KAT'd | **Yes (`key_repeat_epoch`: a focus change and Super+L forced between the check and the stage, and Super+L between the stage and the flush; nothing reaches B or the lock screen. Falsified by `neo-test-key-repeat-epoch-control`, the fresh-read shape: FAIL `reason=repeat_crossed_focus_change`)** |

Legend — **exposure:** *Exposed* = a real reachable defect existed/exists here; *Guarded* = a mechanism
prevents it and there is (or should be) a discriminator; *N-A* = the construct doesn't exist in our kernel
yet (kept so a future build that introduces it re-checks). **Armed discriminator** = §"Armed-discriminator
recommendations" proposes it become a permanently-compiled gate check (Rule-5 paid-down debt), vs a
review-checklist item.

---

## Category A — Lock ordering / deadlock

### CP-1 — ABBA cycle / inconsistent global lock order
- **Failure mode.** CPU/thread A holds L1 and blocks for L2; B holds L2 and blocks for L1. Neither releases;
  permanent deadlock. Generalizes to any cycle in the lock-acquisition graph.
- **Root mechanism.** Two acquisition sites disagree on the order in which two lock classes are taken. The
  bug is *latent* — it manifests only under the exact interleaving, so it passes review and most runs.
- **Known discipline.** Impose one **total order** on lock classes and take them only in increasing order
  (hierarchical lock ranking). Linux's **lockdep** enforces this automatically: it assigns each lock a *class*,
  records the first order seen, and *panics at the offending acquire* on any later inversion — "two locks can
  not be taken in inverse order: `L1 → L2`, `L2 → L1` because this could lead to a deadlock"
  (`Documentation/locking/lockdep-design.rst`). Once a chain is validated it is cached and never re-checked.
- **Our exposure — Guarded (partial).** I-5 ("one documented lock order") + the debug **lockdep in
  `SwitchMutex::new_ordered`** (`kernel/src/sync/switch_mutex.rs:49-119, 178-202`): a per-thread held-level
  bitmask; an acquire whose level is **not strictly greater** than the highest held level **panics at that
  acquire** with the held mask. The documented order is `kernel/src/sync/mod.rs:21` (`ADMISSION_QUEUES=10 →
  … → ACCT_TABLE=25 → RECENCY=28`; USB band 29/30/31). **Gap:** only the **AI-residency `SwitchMutex`es** and
  a handful of ordered sites carry a level; the scheduler-inner lock, NeoFS locks, `PROCESSES`, and the
  process `waiters` lock are **unordered (level 0 = exempt)** — so a NeoFS↔scheduler inversion (the OT-59 #2
  hypothesis, CP-16) is **not** caught by the current lockdep. Extending ordered levels to the
  reap→wake→scheduler edge is the recommendation.
- **Discriminator.** The lockdep panic message already names *both* levels and the held mask — self-localizing
  in one run. Extend by assigning ordered levels to the scheduler/NeoFS/process locks so a real inversion on
  that edge trips the same panic rather than hanging.
- **Citation.** Linux `Documentation/locking/lockdep-design.rst` (lock classes; the "inverse order" and
  "hardirq-safe → hardirq-unsafe forbidden" rules; validate-once caching). Our I-5 (`docs/complexity-governance.md`).

---

## Category B — IRQ-safe locking (the exact class OT-59 #1 hit)

### CP-2 — Spin-waiting IF-off for a resource whose release requires *this* CPU to service an interrupt
- **Failure mode.** A CPU disables local interrupts, then spin-waits for a condition that can only be
  satisfied by an interrupt handler running **on that same CPU** (or by another CPU that is itself blocked
  IF-off on the same resource). The awaited event never arrives → hard hang / shootdown timeout.
- **Root mechanism.** With `IF=0` the LAPIC still *accepts* and latches the IPI (sets IRR), but the core
  **never dispatches** the handler until interrupts are re-enabled. If the only path to re-enable is *past*
  the wait, it is a self-deadlock. Cross-CPU variant: A holds lock, IF=0, waits for B to ACK; B wants the same
  lock, takes it IF-off *first*, and thus can never enter *its* IPI handler to ACK A. Classic ABBA where the
  "second lock" is *the ability to take an interrupt*. **Precision (cross-model verified — both the Codex-
  and Kimi-family reviewers flagged the over-broad phrasing):** it is a deadlock *not* whenever *any* CPU
  spins IF-off on the lock, but specifically when the ACK the holder awaits is owed by a CPU that is **itself
  a target** of the in-flight shootdown *and* is IF-off-blocked on that same lock. A non-target contender
  merely waits; the holder's real targets ACK and it releases. (This is also why narrower per-mm targeting —
  CP-4 — *shrinks* the deadlock surface: fewer targets ⇒ fewer cyclic pairs.)
- **Known discipline.** **Do the cross-CPU wait with interrupts ENABLED.** FreeBSD encodes this as a hard
  invariant: `smp_targeted_tlb_shootdown()` **`KASSERT`s that `read_rflags() & PSL_I != 0`** and panics
  `"smp_targeted_tlb_shootdown: interrupts disabled"` — the initiator must keep IF=1 while waiting so it can
  still service *incoming* shootdown IPIs from other CPUs mid-flight. Linux's TLB-flush IPI is delivered and
  serviced by the target's handler; the initiator does not spin IF-off holding a lock the target needs. The
  discipline: **a lock that is *ever* awaited across a cross-CPU round-trip must not be an IF-disabling lock;
  keep IF enabled during the wait and restore the caller's original IF only after acquisition.** (Lockdep's
  "hardirq-safe → hardirq-unsafe" rule is the general statement: a lock taken in IRQ context must never sit
  below a lock held with IRQs enabled.)
- **The fix is necessary-not-sufficient (cross-model verified).** You cannot naively `sti` inside an
  `IrqSafeMutex` — its whole contract is IF-off. Enabling interrupts during the wait makes the sender
  **preemptible and migratable**, so IF-on must be paired with three co-requisites or it trades a deadlock for
  a use-after-free: **(1) preemption disabled + the sender pinned to its CPU** (FreeBSD keeps the initiator in
  a critical section while IF stays on) so the per-CPU payload/`pending` ownership holds; **(2) the published
  payload lives in storage that outlives the wait and is per-(sender,target) or generation-guarded** (Linux
  uses per-CPU `csd`) so a mid-flight interrupt can't overwrite an unconsumed slot; **(3) the IPI handler stays
  strictly ACK-only** (invalidate → decrement `pending` → EOI) and **never acquires the sender lock**, or a
  nested/re-entrant shootdown self-deadlocks. Our single global `SHOOTDOWN_LOCK` already serializes senders
  (bounding the slot-reuse hazard) and `handle_ipi()` is ACK-only and lock-free (good) — so the residual work
  is the IF-on-plus-pinned restructuring, not the handler.
- **Our exposure — WAS EXPOSED (OT-59 #1, live on `main` until Jarvis's fix lands).**
  `SHOOTDOWN_LOCK: IrqSafeMutex<()>` (`kernel/src/mm/tlb_shootdown.rs:24`). `shootdown()` takes it
  (`:162`), so the sender holds it **IF-off**, publishes payloads, sends IPIs, then spins in
  `wait_for_acks_or_panic` (`:234`). A second CPU entering `shootdown()` spins IF-off on the *same*
  `SHOOTDOWN_LOCK` and can never take CPU-A's shootdown IPI to ACK → `tlb_shootdown_timeout` panic.
  **Fix (OT-59 #1):** replace the IF-disabling sender lock with an **interruptible `SwitchMutex` wait that
  restores the caller's original IF after acquisition** (keep IF=1 during publish/send/ack-wait). See the
  worked example below.
- **Discriminator.** The existing `tlb_shootdown_timeout` panic prints `pending / targets / unacked_slots /
  repoked / elapsed_cycles` (`:250`) — it already tells you *which* target never ACKed. **Add the
  deterministic two-CPU probe** (Jarvis's approach): force the exact ordering (both CPUs contend the sender
  lock, one already waiting on an ACK) and assert `shootdown_lock_probe=ok ipi_ack=ok eoi_progress=ok`. A
  deterministic falsifiable probe beats stress-repeat — it fails *pre*-fix, every time.
- **Citation.** FreeBSD `sys/x86/x86/mp_x86.c`, `smp_targeted_tlb_shootdown()` interrupts-enabled `KASSERT`
  (panic string `"smp_targeted_tlb_shootdown: interrupts disabled"`, attested on freebsd-amd64). Intel SDM
  Vol 3A §11.6 "Issuing Interprocessor Interrupts" (delivery; IRR/ISR). OSDev *TLB Shootdown* /
  *Spinlock* (disable-IRQ-before-a-lock-shared-with-an-IRQ, but never *await* across it).

### CP-3 — Holding an IRQ-safe lock across an unbounded / cross-CPU wait
- **Failure mode.** Even without a same-lock ABBA, holding *any* IF-disabling lock across a wait that depends
  on another CPU (or a timer, or an I/O completion) stalls interrupt service on this CPU for the whole wait —
  starving timers, reschedule IPIs, and device IRQs, and inflating worst-case latency without bound.
- **Root mechanism.** `IrqSafeMutex` is correct for *short* critical sections shared between process and IF=0
  contexts; it becomes a hazard the moment the critical section **waits** rather than **computes**.
- **Known discipline.** IRQ-safe locks guard *bounded, non-blocking* critical sections only. A cross-CPU or
  blocking wait releases the IF-off lock first (or is an interruptible lock). FreeBSD: "most kernel interfaces
  do not expect to be called with a spin mutex held and will misbehave" — spin mutexes (IF-off) are for the
  shortest paths only.
- **Our exposure — subclass of CP-2.** The `IrqSafeMutex` sender-lock-across-ACK-wait *is* this pattern.
  General guard: audit every `IrqSafeMutex::lock()` site for a *wait* inside the guard's scope.
- **Discriminator.** A grep-checkable audit: `IrqSafeMutex` guards must not enclose a spin/park/ACK-wait.
  Optionally a debug "IF-off dwell" counter that flags an IF-off critical section exceeding N cycles.
- **Citation.** FreeBSD spin-mutex discipline (`sys/kern/kern_mutex.c` / `locking(9)`); OSDev *Spinlock*.

---

## Category C — TLB shootdown / IPI

### CP-4 — Shooting down a CPU that is not running your address space
- **Failure mode.** Broadcasting every TLB invalidation to every CPU wastes IPIs (and, if a target isn't in a
  state to ACK, can hang — see CP-8). Over-broad shootdown is a *scalability/latency* trap, not a correctness
  one, **as long as** the freed-tables exception (CP-5) is honored.
- **Root mechanism.** A CPU that has never loaded CR3 for mm *M* has no TLB entries for *M*; invalidating it
  is pure waste. The information needed — "which CPUs have *M* live" — must be tracked.
- **Known discipline.** Maintain a **per-mm CPU mask** and send only to members. Linux: `mm_cpumask(mm)` +
  `native_flush_tlb_multi()` + `should_flush_tlb()`; a *lazy* CPU (`is_lazy`) is skipped and reconciles via
  `tlb_gen` when it next runs user code in `switch_mm_irqs_off()`. Linux keeps the mask **add-only on
  switch-in** (favoring the hot context-switch path over the cold IPI path); FreeBSD uses a self-callback
  (`curcpu_cb`) for the initiator's own invalidation.
- **Our exposure — Guarded (loosely over-broad, not wrong).** We ship **T1 broadcast**:
  `shootdown_ready_cpus_mask()` (`kernel/src/mm/tlb_shootdown.rs:211`) targets *every shootdown-ready CPU*
  minus self, not a per-mm subset. This is *correct* (a superset flush is always safe) and the code documents
  the T2 per-mm-cpumask upgrade explicitly (`:141-160`, citing Linux's add-only model and the
  strict-vs-loose mask tradeoff) as an ADR-tier decision deferred until inference workloads justify it.
- **Discriminator.** When T2 lands: a gate assertion that the derived mask ⊆ ready-set and that **no CPU with
  a live CR3 for the mm is ever omitted** (a stale-mask miss is a *correctness* bug, unlike over-broad).
- **Citation.** Linux `arch/x86/mm/tlb.c` (`native_flush_tlb_multi`, `should_flush_tlb`, `mm_cpumask`,
  `is_lazy`, `tlb_gen`, `switch_mm_irqs_off`); FreeBSD `curcpu_cb`. Our `tlb_shootdown.rs:141-160` T2 note.

### CP-5 — Freeing page tables must flush lazy/idle CPUs too (speculative-walk hazard)
- **Failure mode.** A CPU marked "lazy" (kernel thread, not actively using the user mm) is skipped by the
  mm_cpumask optimization — but if the shootdown is for **page-table teardown** (the page tables themselves
  are being freed), a skipped CPU can **speculatively walk** the just-freed page-table memory (now reused for
  something else) and populate a bogus TLB entry → use-after-free through the MMU.
- **Root mechanism (cross-model sharpened).** The hazard is **not** a leaf TLB entry — it is the
  **paging-structure caches** (SDM §4.10.3) and **speculative interior page-walks**. A CPU with the mm loaded
  in CR3 may cache intermediate PDE/PDPTE/PML4E entries and may speculatively walk *any* linear address
  reachable from that CR3, **independent of whether software ever touched the leaf** or whether the CPU is
  "lazy." The lazy/`should_flush_tlb` filter keys on "will this CPU's TLB serve this VA," which structurally
  does not model speculative *interior* walks — so freeing a page-table page while a walker could still reach
  it lets a speculative walk dereference the repurposed physical page as PTEs → a poison translation.
- **Known discipline (precise scope — cross-model corrected).** **Bypass the lazy filter when freeing page
  tables** — but "flush all" means **all CPUs in `mm_cpumask` with the lazy-skip disabled, NOT machine-wide**;
  a CPU that never loaded the mm has no structure-cache entries for it and needn't be flushed. Linux keys this
  on `freed_tables` in `flush_tlb_mm_range(... freed_tables=true)`; `should_flush_tlb()` returns *true* for
  lazy CPUs in that case. **The modern mainstream mechanism is RCU-deferred table freeing** (`tlb_remove_table`
  / `HAVE_RCU_TABLE_FREE`): defer the *free* until all potential in-flight walkers quiesce, rather than
  broadcasting an IPI. The crisp invariant is therefore *"no CPU may free-then-reuse a page-table page while
  any walker could still reach it,"* satisfiable by either synchronous all-`mm_cpumask` IPI **or** RCU-deferred
  free. (Our source records the IPI form of this exception verbatim.)
- **Our exposure — Guarded (documented; lazy-TLB not yet implemented).** We ship **no lazy-TLB** (SC-3C D6:
  out of scope), so *today* every ready CPU is flushed and the hazard cannot arise. Our `tlb_shootdown.rs:97-107`
  carries a **capture-before-you-need-it** note: *if* Phase-2+ adds lazy-TLB, the freed-tables exception
  **must** be encoded in the same build (the lazy filter bypassed for teardown).
- **Discriminator (capture-before).** The moment a lazy filter is introduced, ship in the same build a gate
  scenario: a lazy CPU + a page-table teardown, asserting the lazy CPU is still flushed (a negative control
  where the exception is *disabled* must fault/corrupt). Until then, keep the invariant note armed as a
  review gate on any `tlb_shootdown.rs` change.
- **Citation.** Linux `arch/x86/mm/tlb.c` (`should_flush_tlb` + `freed_tables`); our `tlb_shootdown.rs:97-107`.

### CP-6 — INVLPG / paging-structure-cache & MOV-CR3 ordering
- **Failure mode.** Assuming `INVLPG` flushes more than it does (it flushes TLB + paging-structure caches for
  *one* linear address, not global-page entries unless...), or that a `MOV CR3` clears everything (it does not
  invalidate **global** pages), leaves stale translations after a mapping change.
- **Root mechanism.** x86 caches not just final translations (TLB) but intermediate paging-structure entries
  (PDE/PDPTE caches). `INVLPG` invalidates entries for one page **and** the paging-structure caches for it;
  `MOV to CR3` flushes non-global TLB + paging-structure caches but **preserves global (G-bit) pages**;
  `INVPCID` and CR4.PGE toggling cover the global case. INVLPG's effect is architecturally complete before the
  next instruction, but it is **not a fence** for *other* CPUs — only the local one.
- **Known discipline.** Match the invalidation instruction to the change: `INVLPG` per-page for a range up to
  a threshold, full flush (`MOV CR3` / CR4.PGE toggle) beyond it and for global pages; on **every** CPU that
  has the mapping (CP-4/CP-5). Intel SDM §4.10.4 enumerates exactly which operations invalidate which caches.
- **Our exposure — Guarded (correct sequencing).** `invalidate_local_range()` uses `INVLPG` per page up to
  `INVLPG_THRESHOLD_PAGES = 33` (Linux-measured crossover) and falls back to `flush_local_full()` beyond
  (`tlb_shootdown.rs:19, 308-327`). Self-invalidation runs **after IPI dispatch, before the ACK wait**
  (`:136, 179`) — matching FreeBSD's `curcpu_cb` sequencing. The N=33 threshold and the INVLPGB/global-ASID
  upgrade path are documented (`:152-160`).
- **Discriminator.** Review-item: any change to the invalidation instruction or threshold re-checks the SDM
  §4.10.4 table (global pages, paging-structure caches). The byte/behavior oracle is the existing
  invlpg-page-count instrumentation (`shootdown_invlpg_pages`).
- **Citation.** Intel SDM Vol 3A §4.10.4 "Invalidation of TLBs and Paging-Structure Caches" (+ §4.10.2 Global
  Pages); INVLPG instruction reference (SDM Vol 2 / felixcloutier `x86/invlpg`).

### CP-7 — ACK / EOI ordering in the IPI handler
- **Failure mode.** Signaling EOI **before** doing the work the sender waits on, or omitting EOI, either lets
  a stale/again-pending interrupt fire (EOI too early) or wedges the LAPIC priority (no EOI) so no
  lower-or-equal-priority interrupt is ever delivered again on that CPU.
- **Root mechanism.** The LAPIC sets an ISR bit on dispatch; EOI clears the highest ISR bit and unblocks
  equal/lower-priority vectors. For **level-triggered** vectors the TMR bit is set and EOI is broadcast to the
  I/O APIC (SDM §11.8.5). The *sender's* liveness depends on the **ACK** (our `pending` decrement), which is a
  separate signal from EOI.
- **Known discipline.** In the handler: **do the work / publish the ACK first, then EOI last.** The ACK is the
  cross-CPU synchronization the sender polls; EOI is the local-APIC bookkeeping that re-arms this CPU. Order:
  `service → ACK (release-store) → EOI`.
- **Our exposure — Guarded (correct order).** `tlb_shootdown_ipi_body` (`kernel/src/arch/x86_64/idt.rs:392`):
  `handle_ipi()` (which `invalidate_local` then `pending.fetch_sub(1, Release)` — the ACK, `tlb_shootdown.rs:203-204`)
  **then** `apic::end_of_interrupt()`. Correct: ACK is published `Release` before EOI. `reschedule_ipi_body`
  (`:386`) is EOI-only by design (see CP-15).
- **Discriminator.** The `pending` ACK counter *is* the discriminator (sender times out naming the unacked
  CPU). A regression that reordered EOI-before-ACK would still pass (EOI doesn't gate the sender) — so the
  standing check is a **review invariant**: "IPI handlers publish their cross-CPU ACK before EOI, and every
  handler ends in exactly one EOI." Consider a debug assertion that each vector EOIs exactly once.
- **Citation.** Intel SDM Vol 3A §11.8.4-§11.8.5 "Signaling Interrupt Servicing Completion" (EOI, TMR,
  level-triggered broadcast); §11.6.1 ICR. OSDev *APIC* (EOI).

### CP-8 — Shootdown to a **published-but-not-yet-ready** CPU (OT-59 #2 — see worked example)
- **Failure mode.** CPU A fires shootdown IPIs at CPU B because a *proxy* said "B is live," but B has not yet
  reached the state where it services shootdown IPIs (not yet in ring 3 / IDT-LAPIC-scheduler live / not yet
  `tlb_shootdown_ready`). B never ACKs → timeout.
- **Root mechanism.** The target-set is derived from a signal that **races** the target's true readiness
  (member of CP-17, publication-before-ready). "A process exists" (registration) ≠ "the CPU running it is in
  ring 3 and can take an IPI."
- **Known discipline.** Derive the shootdown target-set from an **explicit per-CPU readiness flag** published
  store-release *only after* the CPU can service IPIs, read load-acquire before targeting — and, where the
  race is with "has this CPU actually executed in the target context," a **round-trip handshake** (prove the
  CPU took a reschedule interrupt *from ring 3*) rather than a boolean that a spawn can flip early.
- **Our exposure — WAS EXPOSED (OT-59 #2).** The code-level gate already exists: `shootdown_ready_cpus_mask()`
  intersects targets with `tlb_shootdown_ready` (`tlb_shootdown.rs:164-166, 211-221`; `per_cpu.rs:248-262`,
  stored `Release` only after "IDT/LAPIC/per-CPU/slab/scheduler state and IF=1"). The **test scenario**,
  however, keyed its ACK-wait on `any_user_process()` (registration, `process.rs:1433`) going true — which
  `spawn_user` sets at `register()` (`:1358`) *before* the target CPU enters ring 3 — so CPU1 fired at a not-
  yet-ready CPU0. **FIX-2 (OT-59):** a non-blocking reschedule-IPI handshake — CPU1 must observe CPU0 handled
  a reschedule interrupt *from ring 3* (the `from_ring3` IDT counter, `idt.rs:234-253`) before firing. **PM
  AAG focus:** confirm the fix corrects a *test-precondition artifact* and does not paper over a real reachable
  "shoot a published-but-not-ring-3 CPU" bug by merely making the test wait.
- **Discriminator.** The timeout panic already prints `sender_cpu / pending / targets` (e.g.
  `sender_cpu=1 pending=1 targets=0x1`). Arm the **`from_ring3` handshake counter** (`idt.rs`) as a standing
  gate signal: a shootdown may only target CPU *c* after `IRQ_RING3_ENTRIES[reschedule]` for *c* proves a
  ring-3 round-trip. The strengthened xtask oracle asserts this.
- **Citation.** Our OT-59 #2; the readiness-flag pattern (store-release readiness / load-acquire before act)
  is the memory-model publication idiom (Linux `memory-barriers.txt`, RELEASE/ACQUIRE). Intel SDM §11.6 (IPI
  delivery to a target that must be able to service it).

---

## Category D — Memory ordering / barriers

### CP-9 — Store→load reordering under x86-TSO (the one reordering x86 *does* allow)
- **Failure mode.** Two CPUs each write their own flag then read the other's; both read the stale (pre-write)
  value because x86 lets a **later load pass an earlier store to a different address** (store buffer). The
  classic "Dekker/Peterson needs a fence" bug — the *only* reordering x86-TSO permits.
- **Root mechanism.** x86 is **TSO**: stores are not reordered with stores, loads not with loads, and a load
  is not reordered with an *older store to the same address* — **but** store→load to *different* addresses can
  reorder via the per-CPU store buffer. Restoring order needs `MFENCE` or any `LOCK`-prefixed RMW (which is a
  full fence).
- **Known discipline.** Where a store must be globally visible before a subsequent load (mutual-exclusion
  handshakes, seqlocks' rare cases), insert `MFENCE` or use a `LOCK` RMW. For the common producer/consumer
  case, **release-store + acquire-load** already give the needed ordering on x86 *without* an extra fence
  (they compile to plain MOVs under TSO) — the fence is only for store→load.
- **Our exposure — Guarded.** We use `Ordering::Release`/`Acquire` on the load-bearing publications (shootdown
  `pending` Release/Acquire, per-CPU slot `AcqRel`, `tlb_shootdown_ready` Release/Acquire, `scheduler_ptr`
  Acquire). Rust's `Acquire`/`Release` lower to the correct x86 instructions (plain MOV; `SeqCst` would add
  `MFENCE`/`LOCK`). **One Dekker-style handshake exists (OT-87 R4.4):** `GuardedDevice::begin_plain_write`
  (count the write in `plain_writes`, then read `claimed`) against `take_claim` (set `claimed`, then read
  `plain_writes`), so a plain write and an install claim never overlap on one device. Both stores are `LOCK`
  RMWs (`fetch_add`, `swap`) and all four operations are `SeqCst`, so the store→load reordering can't occur:
  either the writer sees the claim and refuses, or the claim sees the write counted and waits for it.
  Discriminator: `install_claim_identity` requires the claim's refusal line for a plain write to a claimed disk.
- **Discriminator.** Review-item: any new *symmetric* two-flag handshake (each CPU stores its own + reads the
  other's) is a red flag — it needs `SeqCst` or an explicit `MFENCE`, not `Acquire`/`Release`. Grep for
  paired store/load on distinct atomics in a spin loop.
- **Citation.** Intel SDM Vol 3A §9.2 "Memory Ordering" (the 8 rules incl. "loads may be reordered with older
  stores to different locations"); Linux `Documentation/memory-barriers.txt` ("ABSTRACT MEMORY ACCESS MODEL",
  ACQUIRE/RELEASE, `smp_mb`); Sewell et al., *x86-TSO* (CACM 2010).

### CP-10 — Compiler fence vs CPU fence confusion
- **Failure mode.** Using `compiler_fence` (`barrier()`) where a **CPU** fence is needed (or vice-versa): the
  compiler is stopped from reordering but the *CPU* still reorders (or you pay for an `MFENCE` where a compiler
  barrier suffices).
- **Root mechanism.** `compiler_fence`/`barrier()` only constrains the compiler's instruction scheduling — it
  emits **no** instruction. A CPU fence (`MFENCE`/`LFENCE`/`SFENCE`, or `Acquire`/`Release`/`SeqCst` atomics)
  constrains the *hardware*. On UP or same-CPU-vs-interrupt ordering, a compiler fence is enough; across CPUs
  you need the CPU fence. Linux: "All memory barriers except the address-dependency barriers imply a compiler
  barrier"; SMP barriers "are reduced to compiler barriers on uniprocessor compiled systems."
- **Our exposure — N-A today.** We have **no hand-rolled lock-free primitive** (Rule-3 explicitly declined
  RCU/seqlock at our scale). Kept in the bank so the first build that reaches for `compiler_fence` re-checks
  this distinction.
- **Discriminator.** Review-item: a `compiler_fence` in cross-CPU code without an accompanying atomic ordering
  is almost always a bug. Any new lock-free primitive is its own dispatch with its own gate (Rule-3).
- **Citation.** Linux `Documentation/memory-barriers.txt` ("EXPLICIT KERNEL BARRIERS", `barrier()` vs
  `mb()`/`smp_mb()`); Rust `core::sync::atomic::{compiler_fence, fence}` docs.

---

## Category E — Per-CPU data + scheduler / wake races

### CP-11 — Wrong `Ordering` on an atomic (Relaxed where Acquire/Release is required)
- **Failure mode.** A flag published `Relaxed` is observed by another CPU *before* the data it "guards" is
  visible → the reader acts on half-initialized state. The most common real memory-ordering bug (more common
  than CP-9).
- **Root mechanism.** `Relaxed` gives atomicity but **no** ordering w.r.t. other memory ops. A "ready" flag
  must be `Release` (with the data written before it) and read `Acquire` (data read after) to form a
  release/acquire pair.
- **Known discipline.** Publication flags: `Release` store / `Acquire` load. Pure counters with no
  happens-before obligation: `Relaxed` is correct and cheaper. Decide *per atomic* what it orders.
- **Our exposure — Guarded (audited).** Publications use `Release`/`Acquire` (see CP-9 list); pure
  observability counters use `Relaxed` (OT-9 per-CPU counters, shootdown instrumentation). The distinction is
  applied deliberately.
- **Discriminator.** A **grep-able gate**: every `Ordering::Relaxed` store to an atomic that another CPU
  *branches on for correctness* (not just observes) is suspect. Keep a short audited allowlist of Relaxed
  "these are pure counters" sites; a new Relaxed on a control flag fails review.
- **Citation.** Linux `Documentation/atomic_t.txt` (RMW ordering: non-returning RMW are unordered; `_acquire`/
  `_release`/fully-ordered variants); `Documentation/memory-barriers.txt` (ACQUIRE/RELEASE).

### CP-14 — Lost wakeup (park / unpark ordering)
- **Failure mode.** Thread T checks a condition (false), decides to sleep; between the check and actually
  sleeping, the waker sets the condition + tries to wake T — but T "isn't parked yet," so the wake is dropped;
  T then sleeps forever. The canonical missed-wakeup race.
- **Root mechanism.** A window between "decided to block" and "actually blocked & visible to wakers" during
  which a wakeup can be lost.
- **Known discipline.** **Install the wake token while holding the same lock the waker takes, and release that
  lock atomically with the context switch** — so the waker either sees the installed token (and wakes T) or
  hasn't set the condition yet (and T re-checks under the lock). No lost-wakeup window exists. **The exact
  invariant (cross-model verified):** there is *never* an instant where the thread is simultaneously
  **(committed-to-sleep) ∧ (lock-free) ∧ (still-on-CPU)**. Dropping the lock *before* the switch reopens **two**
  failure modes, the second worse than a lost wakeup: **(1) lost wakeup** — a waker marks the thread runnable
  in the gap, then the parker's own switch-out stores clobber that mark; **(2) two-CPUs-on-one-stack
  corruption** — another CPU sees BLOCKED, acquires the lock, and *dispatches the thread on CPU2 while its
  register/stack state is still mid-save on CPU1*. Real kernels add the cross-CPU refinement: an **`on_cpu`
  load-acquire handshake** (`smp_cond_load_acquire(&p->on_cpu, !VAL)`) so a woken task is not resumed elsewhere
  until the switching-out CPU has fully saved state. "Release on the new stack" is one mechanism; the invariant
  — *not runnable-elsewhere until fully off-CPU* — is the thing that must hold.
- **Our exposure — Guarded (I-11).** `block_current_with_token_and_guard` (`kernel/src/sched/scheduler.rs:1435`):
  under `without_interrupts`, holding the caller's guard, it installs the token via `park_thread_token`,
  runs `install(&mut *caller_guard, token)` **while both locks are held**, then releases the caller lock and
  the scheduler lock **on the new stack** via `switch_context_with_release2` (`:1478-1487`). Our **`on_cpu`
  equivalent** is ownership-based: the parked thread's `Box<Thread>` is *moved into the scheduler-owned token
  table* before the switch, and the waker (`wake_thread_token`, `:1535`) can only `take_thread_by_token` under
  the **scheduler lock — which is not released until the new stack** — so a waker can never resume the thread
  until CPU1's switch has committed. Move-semantics + take-once + release-on-new-stack together give us the
  "not runnable-elsewhere until off-CPU" property. I-11 ("no lost wakeup") is the ledgered invariant.
- **Discriminator (already armed).** Park/retry-count balance assertions (P4a: 78/78) + the quiesce hook
  (parked-count == 0 at end) + teardown drain checks (I-11 guard column, `docs/complexity-governance.md`).
- **Citation.** Our I-11 / ADR-039; the "condition-check-and-sleep under the same lock" idiom (Linux
  wait-queue `prepare_to_wait`/`finish_wait`; `Documentation/scheduler/`). Do **not** copy — re-derived in our
  token+guard primitive.

### CP-15 — Stranded wakeup: an EOI-only reschedule IPI cannot preempt a busy target
- **Failure mode.** CPU A wakes a thread onto CPU B's runqueue and sends B a reschedule IPI. B's IPI handler
  only EOIs (no reschedule). If B is in a **kernel busy-loop with no timer and no voluntary yield**, B never
  re-enters the scheduler, so the woken thread is **stranded** on B's runqueue — a wakeup delivered but never
  *acted on*. (Distinct from CP-14: the wakeup wasn't lost, it was ignored.)
- **Root mechanism.** "Reschedule" on the EOI-only design is *implicit* — it happens on B's next scheduler
  entry (timer tick, syscall return, voluntary yield). Remove all three and the newly-Ready thread waits
  forever. This is the OT-43 lineage ("EOI-only reschedule IPI can't preempt busy no-timer AP → woken thread
  stranded → Arthur fork-join stalls").
- **Known discipline.** The load-bearing ingredient (cross-model verified) is **`need_resched` set by the
  waker *before* the IPI + a preemptible reschedule point on the target's IRQ-return path** — *not* the IPI
  handler body (Linux's `scheduler_ipi()` is famously near-empty for exactly this reason; `resched_curr()` sets
  `TIF_NEED_RESCHED` and the interrupt-return path calls `schedule()`/`preempt_schedule_irq()` when
  preemptible). So **EOI-only is by-design *adequate* whenever the interrupted context reaches a
  need_resched-honoring point** — return-to-usermode, the idle loop re-checking after wake-from-`hlt`, or a
  preemptible kernel with `preempt_count==0`. It is **inadequate only** in a **non-preemptible / no-timer
  kernel busy-loop** that hits none of those points. Fix: make that loop a reschedule point
  (`cond_resched`-style poll), enable kernel preemption, or guarantee a timer / `sti;hlt` idle. Our kernel took
  the timer/idle route: OT-49 gave every CPU an idle thread + idle-aware rotate + `sti;hlt` (no lost wakeup) so
  "ready queue empty" can't strand; the residual is a *busy* AP inside a long fork-join (CP-16 family).
- **Our exposure — Partially guarded (OT-43 diagnosed / OT-49 idle-thread fix).** `send_reschedule_ipi`
  (`apic.rs:89`) → `reschedule_ipi_body` EOI-only (`idt.rs:386-389`). OT-49 (per-CPU idle thread, MERGED
  `7f6c2fb`) removes the "no runnable thread" strand; the residual OT-43/OT-59-#2 family (a busy AP inside a
  long fork-join) is the *contention*-triggered remnant Cerebro is closing. The design point (EOI-only + rely
  on the return path/idle) is deliberate but is exactly the trap to re-examine on any scheduler change.
- **Discriminator.** OT-49's **timer-token-wake probe** (`B4`) hard-asserts idle-entry (a scenario that
  *reliably idles* a CPU and proves the wakeup lands). Arm it permanently. For the busy-AP case, a
  falsifiable regression that pins a woken thread on a no-timer AP and asserts it dispatches within N ticks.
- **Citation.** Our OT-43 diagnosis (`project_ot43_lineage_postspawn_flake_diagnosis.md`), OT-49 idle thread;
  Linux `TIF_NEED_RESCHED` / `resched_curr` / interrupt-return reschedule (`arch/x86/.../common.c`,
  `kernel/sched/core.c`) as the alternative (route-a) prior art.

### CP-16 — Wake path taken with a foreign lock held (reap → wake lock-order stall)
- **Failure mode.** A completion path frees/reaps a resource while a subsystem lock (e.g. a NeoFS I/O lock) is
  held, then calls into the scheduler's wake path which takes `sched.inner.lock()` — establishing a
  `NeoFS-lock → scheduler-lock` order. If any other path takes them in the opposite order, ABBA (CP-1); even
  without a cycle, the wake is done under a held foreign lock, widening the critical section and risking a
  stall the lockdep doesn't see (those locks are unordered, CP-1 gap).
- **Root mechanism.** `reap()` frees a zombie (which triggers TLB shootdowns, CP-2/CP-8 — cross-lane coupling)
  then `on_thread_reap → complete_process_waiters → wake_thread → sched.inner.lock()`. If `reap()` is pumped
  from a context already holding a NeoFS lock, the wake happens under it.
- **Known discipline.** **Do the wakeup *outside* the foreign lock.** Collect the to-wake set under the
  subsystem lock, **drop it**, *then* call the scheduler wake path (defer-wake / wake-after-unlock). Linux's
  wait-queue wakeups are structured so `try_to_wake_up` takes only the runqueue lock, not the caller's data
  lock — the two never nest.
- **Our exposure — Under diagnosis (OT-59 #2 sibling, Cerebro).** `complete_process_waiters` (`process.rs:1230`)
  is **already** structured to `take_all()` waiters under `proc_arc.waiters.lock()` inside `without_interrupts`,
  **drop that guard**, *then* `wake_thread` (which takes the scheduler lock) — i.e. the waiters lock is **not**
  held across the scheduler lock. The hypothesized stall is that `reap()` itself is *called from* a NeoFS-lock
  context (the `arthur_*` writes hang, log stops at `sched: reaped` with no `deregistered`). Root cause pending
  Cerebro's instrumented repro (`b058224xp`).
- **Discriminator.** Assign **ordered lockdep levels** to the NeoFS, `PROCESSES`, `waiters`, and scheduler-inner
  locks so a real inversion on the reap→wake edge trips the CP-1 panic (self-localizing) instead of a silent
  stall. Capture the stalled `req_id` + `armed_used_event`/`used_idx` at the park (Cerebro's plan). The
  falsifiable regression must **fail-pre at `NEO_SMP=4`** (the merge gate's width), not only `NEO_SMP=8`.
- **Citation.** Our OT-59 #2 / OT-43-lineage; Linux `try_to_wake_up` runqueue-lock-only discipline
  (`kernel/sched/core.c`); our `process.rs:1230-1265, 1472-1527`, `scheduler.rs:1502-1507, 1905-1958`.

---

## Category F — Atomics / RMW

### CP-12 — ABA / slot reuse (a freed-then-reallocated object read through a stale key)
- **Failure mode.** A pointer/index is read as value *A*, the slot is freed and reallocated (now a different
  object) but happens to present as *A* again; a CAS or a stale key then succeeds against the **wrong**
  occupant → use-after-free / cross-object corruption.
- **Root mechanism.** A bare pointer/index carries no identity across free+reuse. Comparing only the address
  can't tell "same object" from "reused slot."
- **Known discipline.** **Tag every key with a generation counter**; bump the generation on free; check the
  generation on every keyed access — a mismatch is a clean miss, never a deref of the new occupant.
- **Our exposure — Guarded (already armed, I-13).** The slot arena (`kernel/src/ai/arena.rs`) keys every slot
  with an allocation **generation**; `free` bumps it; keyed ops check first (I-13, `docs/complexity-governance.md`;
  ADR-042). The generation-mismatch counter is a permanent gate signal (gate demands >0 generated AND 0 stale
  resolves ever). The scheduler wake **token** (`ThreadWakeToken`, with `TokenTakeResult::Stale`) is the same
  pattern for parked threads.
- **Discriminator (already armed).** I-13 generation-mismatch counter + reuse-attack truth tables + keyed
  byte-tag oracle (the O1a stress gate). Extend the *pattern* to any new slab/slot/index table.
- **Citation.** Our I-13 / ADR-042; the tagged-pointer / generation-count ABA guard (Michael & Scott lock-free
  queue literature; Linux `Documentation/atomic_t.txt` on RMW). Re-derived in our generational arena.

### CP-13 — Torn / non-atomic multi-word read
- **Failure mode.** Reading a multi-field or >word-size structure without a consistency protocol yields a
  mix of old and new fields (a "torn" read) — e.g. reading a 128-bit descriptor while another CPU writes it.
- **Root mechanism.** Only aligned word-size (≤8B on x86-64) accesses are atomic. A larger structure read
  field-by-field can interleave with a writer.
- **Known discipline.** Snapshot under a lock, or use a seqlock (write-side bumps an even→odd→even counter;
  read-side retries if the counter changed / is odd), or pack into a single atomic word where possible.
- **Our exposure — Guarded.** OT-9 observability uses **explicit snapshot reads**; the `LockOwner` snapshot
  (`sync/lock_owner.rs`, `read_consistent()`) is a seqlock-style consistent read of a multi-field owner
  record. No unprotected multi-word shared struct is read cross-CPU.
- **Discriminator.** Review-item: any new cross-CPU read of a struct wider than one atomic word must name its
  consistency protocol (lock / seqlock / single-word pack).
- **Citation.** Linux seqlock (`Documentation/locking/seqlock.rst`); our `sync/lock_owner.rs::read_consistent`.

---

## Category G — Publication-before-ready (the cross-cutting pattern)

### CP-17 — Publication-before-ready
- **Failure mode.** A resource is made **visible** (registered in a global table, a predicate flips true, a
  pointer is published) **before** it is **safe to act on** from another CPU. A second CPU that keys an action
  off the visibility acts on a half-ready target. CP-8 (shootdown to a not-yet-ring-3 CPU) and CP-16
  (reap→wake before the waiter is fully parked/visible) are members; the OT-59 second race is the archetype.
- **Root mechanism.** Two facts — "X is visible" and "X is ready to be acted on" — are established by
  *different* stores at *different* times, and a cross-CPU actor observes the first without the second.
- **Known discipline.** **Gate the cross-CPU action on an explicit readiness signal, not a visibility proxy.**
  Two forms, strongest last: (1) a **per-CPU/per-object `ready` flag** published `Release` *only after* the
  object can be acted on, read `Acquire` before acting (the `tlb_shootdown_ready` pattern); (2) a **round-trip
  handshake** where the actor proves the target *executed* the readiness-establishing step (the ring-3
  reschedule handshake). The publication idiom: write the payload, `Release`-store the ready flag; the reader
  `Acquire`-loads ready, then (and only then) touches the payload.
- **The sufficiency boundary (cross-model verified — the sharp rule).** Form (1), a per-CPU **boolean**, is
  sufficient **iff (a) readiness is monotonic** (a CPU that becomes ready stays ready for the action's
  lifetime — true for our one-shot AP bring-up) **AND (b) "ready" implies *timely serviceability*** (a ready
  CPU is never wedged IF-off longer than the ACK timeout). It is **insufficient — form (2) round-trip
  required — when** readiness is **revocable** (hotplug/offline, deep-idle `hlt` with IF off, a long IF-off
  critical section, an SMI: "became ready once" ≠ "responsive now"), **or** when correctness needs proof the
  target **executed** something (flushed its TLB, observed the new mapping) — a flag cannot attest to an
  *effect*, which is exactly why the shootdown trusts the `pending` decrement, not a "CPU is up" flag, and why
  OT-59 #2 needed the ring-3 round-trip (proof CPU0 *ran* in ring 3), not just `tlb_shootdown_ready`.
  **Diagnostic corollary:** if cross-CPU timeouts persist *after* adding the readiness flag, the residual cause
  is a CP-2 transient IF-off window on the target, not a publication-order bug — the flag fixes ordering, not
  liveness.
- **Our exposure — was exposed via CP-8/CP-16; the primitive exists.** `tlb_shootdown_ready`
  (`per_cpu.rs:248-262`) is form (1) done right — stored `Release` only after IDT/LAPIC/scheduler/IF are up.
  The OT-59 #2 defect was keying a *test precondition* on a weaker proxy (`any_user_process()`); FIX-2 is form
  (2) (ring-3 round-trip handshake).
- **Discriminator (named invariant).** Recommend a **ledger invariant**: *"Cross-CPU actions gate on an
  explicit readiness signal published after the target is safe, never on a visibility proxy that races
  readiness."* Its guard: the shootdown-ready mask intersection (armed) + the `from_ring3` handshake counter
  (armed). Every future cross-CPU seam consults CP-17 and names which form it uses.
- **Citation.** Our OT-59 #2; the RELEASE/ACQUIRE publication pattern (Linux `Documentation/memory-barriers.txt`,
  "publish a pointer then the data behind it"); our `per_cpu.rs:248-262`, `idt.rs:234-253`.

---

## Category H — Device driver concurrency

### CP-18 — Single-in-flight device: submit→wait→complete not serialized
- **Failure mode.** A device driver with **one in-flight request slot + one waiter** assumes a single logical
  owner of the submit→wait→complete sequence, but under SMP (or once the path runs interrupts-enabled/
  preemptible) two threads — e.g. a foreground reader and a background writeback worker — interleave on that
  one slot. The completion path can retire request A (clearing the slot) while A is mid-sequence, letting B
  submit into the slot; A then attaches/parks against the wrong request. **Manifestation is
  transport-dependent:** under a *polling* completion worker it's a **lost-wakeup hang** (the parked owner is
  never woken); under an *IRQ-driven* completion it's a **hard assertion/panic** (`"in-flight request already
  has a waiter"`). Same root, two faces — so a driver that "passes with IRQs" may still hang the polling build,
  and vice-versa.
- **Root mechanism.** The per-request critical section is not atomic across its IF-enabled windows (an inline
  completion-poll spin between the submit lock-drop and the park). A per-slot `busy` check serializes the
  *submit* but not the *wait/attach/retire*, so the sequence is only partially serialized. This is a CP-8/CP-17
  (publication-before-ready) member specialized to a single-resource device.
- **Known discipline.** **Serialize the entire submit→wait→complete per request** with a device I/O gate held
  by exactly one owner from submit through completion — a *separate* lock from the queue-state lock, because the
  completion path must still take the queue lock freely to wake the parked owner (CP-16: the waker must never
  need the gate). Two hard sub-rules: (a) the gate's **acquire must run interrupts-enabled/preemptible** (a
  yielding acquire, not an IF-off spin) or it is a CP-2 recurrence — the parked owner is woken by the completion
  IRQ/timer, which an IF-off waiter would block; (b) **gate OUTER of the queue lock**, enforced by keying ONLY
  the thread-context gate into the CP-1 lockdep levels (`lock_order::BLOCK_IO_GATE`) so a future thread-context
  inversion trips the panic, not a silent stall. **Do NOT order the queue/`inner` lock itself** — it is also
  acquired in the completion ISR, and lockdep is invalid in interrupt context (it keys by thread-id), so keying
  it would fire a *false* inversion panic when a completion IRQ preempts a thread holding an unrelated ordered
  lock (e.g. an AI-governance lock) — matching AHCI/NVMe, which keep their ISR-touched inner lock unordered.
  (This exact false-panic was caught by the OT-59 #2 AAG and is why the gate is ordered but `inner` is not.)
  Linux serializes single-tag/legacy block queues with the queue lock + a request tag; the gate is our
  equivalent for a one-tag device.
- **Our exposure — Exposed → FIXED (OT-59 #2, Cerebro).** `kernel/src/drivers/block/virtio_blk.rs`
  `VirtioBlkDevice`: added `io_gate: SwitchMutex<()>` acquired via a yielding, interrupts-enabled `try_lock`
  loop at the top of `submit_pending_and_wait` and held across the whole sequence incl. the owner's park; the
  completion path (`poll_used`/`handle_interrupt`/`virtio_blk_completion_worker`) takes only `inner`. Exposed
  when OT-59 #2's IF-fix (`with_interrupts_enabled` around the syscall file I/O, itself CP-2) made NeoFS I/O
  preemptible, letting the reader and the `neofs-writeback` worker interleave on the single slot — a hang under
  the polling build (plain `neo-test-arthur-memory`), a panic once `virtio-blk-irq` was enabled.
- **Discriminator.** The IRQ-build panic (`attach_waiter` assert) is the loud signature; the polling-build hang
  needs a timeout-dump (per-CPU stack: the owner parked in `submit_pending_and_wait`, a second submitter in the
  same path). The gate's lockdep level makes a thread-context ordering inversion self-localizing. A falsifiable regression must
  drive **sustained reader-vs-writeback contention at `NEO_SMP=4`** (the merge-gate width) — one collision is
  not enough to catch a fairness/serialization regression.
- **Citation.** Our OT-59 #2 / OT-43 lineage; Linux single-queue tag serialization (`block/blk-mq.c`,
  legacy `request_queue` lock); our `virtio_blk.rs` (`submit_pending_and_wait`, `attach_waiter`, `poll_used`),
  `sync/mod.rs::lock_order`.

---

## The two OT-59 races — worked examples already in the bank

These are the reason the bank exists: both are **members of documented classes** the bank now pre-seeds. Had
this checklist existed, a spawn/ring-3 + cross-CPU-shootdown dispatch would have consulted CP-2 and CP-8/CP-17
*before* the gate reddened.

### Worked example #1 — the ABBA shootdown-lock deadlock (OT-59 #1, class CP-2/CP-1)
- **Symptom.** `swapgs_irq_ring3` scenario deterministically hangs (`BEGIN`, never `PASS`); real
  `tlb_shootdown_timeout`, target CPU0 unacknowledged. Reproduces on **plain `main`** at `NEO_SMP=4` (not
  B3-specific) — a main defect.
- **Mechanism (bank class CP-2).** `SHOOTDOWN_LOCK` is an `IrqSafeMutex` (`tlb_shootdown.rs:24`): the sender
  holds it **IF-off** and spins for ACKs (`:162, 234`). A second CPU in `shootdown()` spins **IF-off** on the
  same lock → cannot take CPU-A's shootdown IPI → cannot ACK → CPU-A times out. Textbook "the second lock is
  the ability to take an interrupt."
- **The known answer (would have been pre-seeded).** CP-2's discipline: never *await a cross-CPU round-trip
  while holding an IF-disabling lock*; FreeBSD asserts interrupts-enabled in exactly this routine. **Fix:**
  replace the IF-off sender lock with an interruptible `SwitchMutex` wait that **restores the caller's original
  IF after acquisition** (IF stays 1 during publish/send/ack-wait). **Necessary-not-sufficient (per CP-2's
  cross-model refinement):** IF-on must be paired with **preemption-disabled + CPU-pinned + stable/serialized
  payload** so the now-interruptible sender can't migrate or have its per-target slot overwritten mid-wait; our
  single global sender lock already serializes senders and `handle_ipi()` is ACK-only/lock-free, so this is
  bounded. Probe green: `shootdown_lock_probe=ok ipi_ack=ok eoi_progress=ok`.
- **Discriminator that made it conclusive.** A **deterministic two-CPU probe** forcing the exact ordering
  (stronger than stress-repeat) + the existing timeout panic naming the unacked target. → armed.

### Worked example #2 — the spawn/ring-3 publication race (OT-59 #2, class CP-8/CP-17)
- **Symptom.** After FIX-1, CPU1 timed out shooting down CPU0 with `sender_cpu=1 pending=1 targets=0x1`:
  CPU0 was **published but not yet in ring 3**.
- **Mechanism (bank class CP-8/CP-17).** `spawn_user` `register()`s the process — `PROCESSES.lock().insert()`
  (`process.rs:817, 1358`) — making `any_user_process()` true **before** the target CPU context-switches into
  ring 3 and reaches `tlb_shootdown_ready`. The scenario keyed its shootdown ACK-wait on that visibility
  proxy, so it fired at a not-yet-ready CPU. (The *production* mask already intersects `tlb_shootdown_ready`;
  the defect was the test precondition using a weaker proxy — the "fix the test to make it green" trap the AAG
  completeness lens exists to catch.)
- **The known answer (would have been pre-seeded).** CP-17's discipline: gate on an explicit readiness signal,
  not a visibility proxy. **FIX-2:** a non-blocking **reschedule-IPI handshake** — CPU1 must observe CPU0
  handled a reschedule interrupt *from ring 3* (`from_ring3` IDT counter, `idt.rs:234-253`) before firing the
  128 shootdowns. **Open AAG question:** prove this corrects a test-precondition artifact and does not mask a
  real reachable "publish-before-ready" shootdown bug by making the test wait.
- **Discriminator that made it conclusive.** The `sender_cpu/pending/targets` timeout print + the armed
  `from_ring3` ring-3 handshake counter + the strengthened xtask oracle. → armed.

**Cross-lane coupling the bank surfaces.** `reap()` frees memory → triggers TLB shootdowns (CP-2/CP-8), and
Cerebro's Arthur family hangs *in* `reap()` (CP-16) — so Jarvis's CP-2 fix may **incidentally green** some
Arthur scenarios. The bank makes this coupling explicit: after the shootdown fix lands, re-test the Arthur
family *before* concluding a second, independent scheduler-lock defect exists.

---

## Armed-discriminator recommendations (Rule-5: paid-down debt, stays armed)

Per `docs/complexity-governance.md` Rule 5 (discriminators are never removed) and the I-14 precedent, the
following bank entries warrant a **permanently-compiled gate discriminator or a ledger invariant** — as
opposed to a review-checklist item.

**Promote to permanently-armed discriminator / ledger invariant:**
1. **CP-2/CP-3 (IF-off cross-CPU wait).** The **deterministic two-CPU shootdown-lock probe** (`shootdown_lock_probe`)
   + the timeout panic. Arm permanently in the QEMU gate at `NEO_SMP≥4`.
2. **CP-7 (ACK-before-EOI).** Keep the `pending` ACK counter armed; add a debug "exactly-one-EOI-per-vector"
   assertion.
3. **CP-8/CP-17 (publication-before-ready).** Arm the **`from_ring3` reschedule handshake counter** as the
   standing gate for any cross-CPU action against a freshly-spawned target; recommend a **new ledger invariant**
   (candidate **I-16**, *pending the one-page-fit re-check per Rule 2*): *"Cross-CPU actions gate on an explicit
   readiness signal published after the target is safe, never on a visibility proxy."*
4. **CP-15 (stranded wakeup).** Arm OT-49's **timer-token-wake idle-entry probe (B4)** permanently + a
   no-timer-AP dispatch-within-N-ticks regression.
5. **CP-16 (reap→wake lock order).** **Extend I-5's ordered lockdep levels** to the scheduler-inner / NeoFS /
   `PROCESSES` / `waiters` locks so a real inversion on that edge *panics at the acquire* rather than stalling
   (closes the CP-1 coverage gap).
6. **CP-11 (wrong Ordering).** A **grep-gate**: `Ordering::Relaxed` stores to control flags fail review unless
   on the audited pure-counter allowlist.
7. **CP-5 (freed-tables lazy exception).** **Capture-before-you-need-it**: the exception + its negative-control
   gate ship in the *same* build that first introduces lazy-TLB.

**Already armed (keep):** CP-12 (I-13 generation counter), CP-14 (I-11 park/retry balance + quiesce hook),
CP-1 (the existing I-5 lockdep — extend per CP-16).

**Review-checklist items (not standing discriminators):** CP-4 (T2 mask ⊆ ready, no-omission — arms *when* T2
lands), CP-6 (SDM §4.10.4 re-check on any invalidation-instruction change), CP-9/CP-10 (symmetric-handshake /
compiler-fence red flags), CP-13 (multi-word read consistency protocol named), CP-21 (latency-bound probe for
EOI-only reschedule IPI), CP-24 (grep-gate: production cross-CPU spawn must use explicit target enqueue),
CP-25 (partition-time vs tick-period measurement), CP-27 (per-CPU bootstrap-complete flag before IPI send),
CP-28 (IF-off-through-check-halt review invariant).

**OT-34-specific armed discriminators (Category I):**
8. **CP-19 (idle loop missing poll hook).** The standing code comment at `scheduler.rs:1240-1248` is the
   capture-before guard. Strengthen to a compile-gate: if `threaded-matmul` is on, `idle_thread_entry` must
   call `ap_poll` (enforced by a `#[cfg]` assertion or shared callback registration). Gate: any commit that
   routes APs through `idle_thread_entry` must re-prove `threaded_matmul_parity`.
9. **CP-20 (AP parks before spawn completes).** Arm OT-49's **timer-token-wake probe (B4)** permanently as the
   standing gate for IPI-delivery-to-idle. Additionally, document that `send_reschedule_ipi`'s LAPIC write acts
   as an implicit fence ordering the enqueue against the receiver — any future wake mechanism replacing the IPI
   must replicate this with explicit atomics.
10. **CP-23 (MWAIT loses IPIs).** Capture-before-you-need-it: the moment MWAIT is introduced, ship a per-C-state
    IPI-delivery test in the same build. Any C-state that drops an IPI is excluded from the idle driver's table.

---

## Proposed CLAUDE.md rule extension (for PM/PL ratification at RETURN)

> ## Concurrency / SMP-touching dispatches require a concurrency-pitfall-bank consultation (PM dispatch standing rule)
>
> Any dispatch that adds or changes **cross-CPU / interrupt / lock / atomic** code — the concurrency analogue
> of the driver/blind-hardware prior-art trap-mining rule — MUST, before it can be marked DISPATCHED, include a
> **concurrency-pitfall-bank consultation**: for each pitfall class the change touches (`docs/concurrency-pitfalls.md`,
> CP-1…CP-28), state *introduces / guards / N-A* and name the discriminator that proves it. New pitfalls found
> on the gate are added back to the bank (it grows; it is never pruned). This pre-seeds the **known** classes so
> real iteration is spent on the genuinely-ours seams — it does **not** replace the adversarial-audit gate, the
> QEMU gate, the byte-oracle, or the kill-switch (patterns-known ≠ correctness-proven; the seL4 lesson).
> Copyleft/GPL sources are read for principle and cited, never copied (ADR-026).

---

## Primary sources (read directly; cite exact locations)

- **Intel® 64 and IA-32 SDM, Volume 3A (System Programming Guide):** §4.10 "Caching Translation Information"
  (esp. §4.10.4 "Invalidation of TLBs and Paging-Structure Caches", §4.10.2 "Global Pages"); §9.2 "Memory
  Ordering" (x86-TSO rules); §11.6 "Issuing Interprocessor Interrupts" (§11.6.1 ICR / delivery-status polling);
  §11.8.4–§11.8.5 "Signaling Interrupt Servicing Completion" (EOI, TMR, level-triggered). *Section numbers are
  edition-dependent — cite by title.* SDM Vol 2: `INVLPG`, `MFENCE`/`LFENCE`/`SFENCE` instruction references.
- **Linux kernel source & Documentation (read for principle, never copied):**
  - `arch/x86/mm/tlb.c` — `native_flush_tlb_multi()`, `should_flush_tlb()`, `flush_tlb_func()`,
    `mm_cpumask`, `is_lazy`, `tlb_gen`, `switch_mm_irqs_off()`, the `freed_tables` exception.
  - `Documentation/memory-barriers.txt` — abstract memory model; write/read/general/address-dependency
    barriers; ACQUIRE/RELEASE; `mb()/rmb()/wmb()` vs `smp_*` vs `barrier()`; UP-reduces-to-compiler-barrier.
  - `Documentation/locking/lockdep-design.rst` — lock classes; hardirq/softirq usage states; the "inverse
    order" and "hardirq-safe → hardirq-unsafe forbidden" rules; validate-once caching.
  - `Documentation/atomic_t.txt` — RMW ordering (non-returning RMW unordered; `_acquire`/`_release`/fully-ordered).
  - `Documentation/locking/seqlock.rst`; `kernel/sched/core.c` (`try_to_wake_up`, `resched_curr`,
    `TIF_NEED_RESCHED`) for the wake/preempt prior art.
- **FreeBSD source:** `sys/x86/x86/mp_x86.c` — `smp_targeted_tlb_shootdown()` interrupts-enabled `KASSERT`
  (`"smp_targeted_tlb_shootdown: interrupts disabled"`), the `curcpu_cb` self-invalidation callback;
  `sys/kern/subr_smp.c` (`smp_rendezvous`); spin-mutex discipline (`locking(9)`).
- **OSDev wiki:** *TLB Shootdown*, *APIC* (ICR, EOI, IPI), *Spinlock* (IF-and-spinlock discipline).
- **Academic:** Sewell, Sarkar, et al., *x86-TSO: A Rigorous and Usable Programmer's Model for x86
  Multiprocessors* (CACM 2010) — the formal x86 memory model behind CP-9.
- **Our own (the exposure map):** `kernel/src/mm/tlb_shootdown.rs`, `kernel/src/sync/{switch_mutex,irqsafe_mutex,mod}.rs`,
  `kernel/src/arch/x86_64/{idt,apic,per_cpu}.rs`, `kernel/src/sched/{scheduler,process,kthread}.rs`;
  `kernel/src/ai/par_matmul.rs`; `kernel/src/arch/x86_64/smp.rs`;
  `docs/complexity-governance.md` (I-1…I-15); `docs/OPEN-THREADS.md` (OT-59, OT-43, OT-47, OT-49, OT-34);
  ADR-013 (SMP retrofit), ADR-019 (SwitchMutex IRQ-off-by-construction), ADR-039/042.

---

## Category I — AP idle-hook / cross-CPU scheduler handoff (OT-34 trap-mining)

**Framing note — the OT-34 concrete failure.** On Cerebro's 12-core bare-metal smoke test, enabling
threaded-matmul (OT-34) **deadlocked**: the AP idle-hook parks APs before cross-CPU scheduling finishes,
so the run hangs at the spawned hog. The BSP publishes matmul partitions and spawns worker threads; APs
enter their bootstrap loop (`run_ap_bootstrap`, `scheduler.rs:328`), poll `ap_poll` for a partition, then
`yield_now()`. If an AP's idle thread is activated (e.g., after the bootstrap loop transitions to the idle
thread in a future pass), it enters `idle_thread_entry` (`:1249`) which has **no `ap_poll` hook** — so any
partition assigned to that CPU is never claimed. The spawned hog on another CPU waits for all partitions to
complete → hangs. CP-19 through CP-28 below map the documented traps in this class against our concrete seams.

### CP-19 — Idle loop missing a work-poll hook (OT-34 direct cause)
- **The trap.** An AP's idle loop does not poll for externally-published work slots. When the BSP assigns
  work to that CPU (via an atomic slot), the AP enters `hlt` and never observes the assignment → work
  starves, join barrier on the BSP hangs forever. This is the **exact** OT-34 failure mode: if a future
  pass unifies AP bootstrap into the idle thread (OT-47's 8-core lane), the `ap_poll` call must migrate
  with it or matmul silently starves.
- **Where it bites our seam.** `idle_thread_entry` (`scheduler.rs:1249-1283`) has no `ap_poll`;
  `run_ap_bootstrap` (`:328-358`) does. The code carries a capture-before-you-need-it note (`:1240-1248,
  :346-352`) requiring any future unification to carry the hook. Currently APs never reach `idle_thread_entry`
  (ISC-4 reconciliation: the bootstrap thread busy-yields and never parks). The seam is latent until an
  AP-efficiency pass activates it.
- **Discriminator.** The existing code comment at `:1240-1248` names the invariant explicitly. Gate: any
  commit that routes APs through `idle_thread_entry` must include `ap_poll` and re-prove
  `threaded_matmul_parity`. A negative control: remove `ap_poll` from `run_ap_bootstrap` and assert the
  parity test fails within N ticks.
- **Guard.** The standing note (`:1240-1248`) is the guard. Strengthen: compile-gate the invariant — if
  `threaded-matmul` feature is on, `idle_thread_entry` should `#[cfg]`-assert that `ap_poll` is called, or
  better, refactor `ap_poll` into a per-loop callback registered at scheduler init so both loops share the
  same hook point.
- **GUARD CLOSED (OT-34 Milestone-A, 2026-07-19, Oracle — PM ratification pending).** The seam is no longer
  latent: the poll is now a single shared hook `scheduler::poll_ap_matmul_work(cpu_id)` called from **both**
  `run_ap_bootstrap`'s busy-yield loop (the live path) **and** `idle_thread_entry`'s park/halt loop, so the
  hook can never be present in one and missing in the other (the "better" fix this entry recommended). In the
  idle loop it runs at the loop TOP with IF=1, *before* the IF-off ready-queue check, so the CP-22/CP-28
  `sti; hlt` sequence is byte-for-bit unchanged; feature-off compiles it to an empty no-op. A parked AP that
  wakes on a tick now drains its work slot instead of sleeping through the assignment. This is defense-in-depth
  layered over the CP-20 steal-back (which already makes `run_generic` deadlock-free on an absent AP), not the
  primary liveness path. The negative control this entry names is realized as the `par_matmul::set_perturb`
  arm of `neo-test-threaded-matmul-parity`.
- **Source.** Linux: the idle thread (`kernel/sched/idle.c`) calls `tick_nohz_idle_exit()` + RCU idle exit
  before `hlt`, and re-enters those on wake — the principle is "idle loop must drain all pending work classes
- **PM RATIFIED + PRECISION CORRECTION (AAG 2-lens, 2026-07-19).** The CP-19 guard is confirmed sound (poll byte-above the `sti;hlt` region, no lock, no `hlt`, bounded; CP-22/CP-28 untouched). **But the deadlock-freedom is NOT "steal-back by construction" as stated above** — steal-back (`barrier_action`) only reclaims *never-claimed* partitions; a **claim-then-park** partition is never re-stolen (single-winner CAS) and spins to `BARRIER_LIMIT` → fail-STOP **panic**. What actually makes enablement safe THIS milestone: **APs carry no routed work, so `rotate()` returns `None` and an AP's `run_disjoint` compute cannot be preempted between claim and `done`.** ⚠️ **When AP scheduling lands (OT-47 8-core lane / any routed AP work), claim-then-park becomes reachable** (a tick preempts a claimed-but-unfinished slot) — the idle-loop poll adds a 2nd such claim site. The next AP-scheduling dispatch MUST re-prove `threaded_matmul_parity` and either extend steal-back to claimed-but-abandoned partitions or forbid preempt-mid-claim. (This is the "MUST re-prove" debt the in-tree comments flag, stated precisely.)
- **Source.** Linux: the idle thread (`kernel/sched/idle.c`) calls `tick_nohz_idle_exit()` + RCU idle exit
  before `hlt`, and re-enters those on wake — the principle is "idle loop must drain all pending work classes
  before halting." OSDev wiki *Sleeping/Waking CPUs*: "an idle CPU that misses a work notification because it
  halted between the publish and the poll window will sleep forever." **Confident mapping** — this is our
  direct OT-34 failure.

### CP-20 — AP parks before cross-CPU spawn completes (OT-34 hang root)
- **The trap.** The BSP spawns threads on APs' runqueues and sends reschedule IPIs, but the AP has already
  entered `hlt` (idle) before the IPI arrives. If the IPI is delivered after `sti; hlt` has committed, it
  wakes the idle loop correctly — BUT if the spawn's thread-enqueue and the IPI are not ordered such that the
  IPI follows the enqueue with a release-acquire chain, the AP may wake, see an empty queue (the enqueue is
  not yet visible under TSO store-buffer reordering), and re-halt → the spawned thread is stranded on a
  visible-but-not-yet-ordered runqueue entry.
- **Where it bites our seam.** `wake_thread_token` (`scheduler.rs:1562-1592`): enqueues the thread under
  `sched.inner.lock()` (IF-off), drops the lock, then sends `send_reschedule_ipi` (`apic.rs:89`). The IPI
  is sent **after** the enqueue completes, so ordering is correct for a single sender. However, the IPI
  handler (`reschedule_ipi_body`, `idt.rs:391`) is **EOI-only** — it does not reschedule, it relies on the
  idle loop's next `rotate()` call to pick up the thread. If the AP is in the `sti; hlt` instruction when
  the IPI arrives, the `hlt` aborts, the IPI handler runs (EOI), then the loop continues to
  `disable_interrupts(); rotate(...)`. The `rotate()` acquires the scheduler lock and reads the ready queue.
  Under TSO, the enqueue's stores are visible because the sender completed them before sending the IPI (which
  is a write to the target LAPIC — a full fence on the sender). **This specific seam is guarded**, but only
  because the IPI acts as an implicit fence. If the IPI were replaced by a lock-free flag, TSO store→load
  reordering (CP-9) would reopen this window.
- **Discriminator.** The IPI-to-LAPIC-ICR write is architecturally ordered (SDM §11.6.1: ICR write completes
  before the next instruction). A regression that replaces the IPI with a flag-based wake must add an
  explicit `Release` store on the sender and `Acquire` load on the receiver. The existing
  `timer-token-wake` probe (B4, OT-49) exercises this path; arm it permanently.
- **Guard.** The IPI's implicit fence is the current guard. Make it explicit: document that
  `send_reschedule_ipi` orders the enqueue against the receiver's idle loop via the LAPIC write fence. Any
  future wake mechanism that drops the IPI must replicate this ordering with atomics.
- **Source.** Linux `try_to_wake_up` (`kernel/sched/core.c`): sets `TASK_RUNNING`, enqueues under rq lock,
  then sends IPI — the IPI's delivery to the target LAPIC acts as a full barrier ensuring the enqueue is
  visible on the receiver. Intel SDM Vol 3A §11.6.1: "writing to the ICR ... causes the processor to wait for
  completion" — implicit fence. **Confident mapping** — our seam matches, but the IPI-as-fence is implicit
  and fragile if the wake mechanism changes.

### CP-21 — EOI-only reschedule IPI vs busy AP that never re-enters scheduler (CP-15 extension)
- **The trap.** CPU A sends a reschedule IPI to CPU B, which only EOIs in the handler. If B is running a
  **kernel busy-loop** (not in idle, not yielding, no timer tick reaching it), B never calls `rotate()` or
  checks `need_resched` equivalent → the woken thread sits on B's runqueue forever. This extends CP-15: even
  with the OT-49 idle-thread fix (no strand when queue empty), a **busy** AP in a long kernel computation can
  strand cross-CPU wakes.
- **Where it bites our seam.** `reschedule_ipi_body` (`idt.rs:391-394`) is EOI-only. Our kernel has no
  `need_resched` flag checked on IRQ return (no preemptible kernel). The only reschedule points are explicit
  `yield_now()`, timer-driven `on_tick()` → `rotate(Ready, Ready)`, and the idle loop's `rotate()`. If an AP
  is in a long kernel busy-loop with no yields and the timer has not yet preempted it, a cross-CPU wake to
  that AP's runqueue will sit until the next tick. For OT-34: if the BSP spawns a hog on CPU1 while CPU1 is
  executing a matmul partition in a tight loop, the hog won't run until the timer preempts CPU1 — this is
  **latency, not deadlock**, because the timer guarantees eventual reschedule. But under deep-idle or SMI,
  the timer itself may be delayed.
- **Discriminator.** Measure worst-case wake latency: inject a cross-CPU wake while the target is in a known
  busy-loop, measure ticks until dispatch. If > TIMER_HZ, the EOI-only design is inadequate for that workload.
  The existing `smp_scheduler_worker` test spawns hogs on multiple CPUs — it exercises this path but doesn't
  measure latency bounds.
- **Guard.** The timer tick (`on_tick`, `scheduler.rs:1095`) guarantees eventual reschedule. For bounded
  latency, either (a) add a `need_resched` flag checked on IRQ return (Linux model), or (b) make the reschedule
  IPI handler actually call `rotate()` if the ready queue is non-empty (FreeBSD model). The current design
  trades implementation complexity for worst-case latency — acceptable for Phase 1.5, but a known bound to
  tighten in OT-47's 8-core lane.
- **Source.** Linux: `scheduler_ipi()` sets `TIF_NEED_RESCHED`, the interrupt-return path calls
  `preempt_schedule_irq()` when preemptible (`kernel/sched/core.c`). FreeBSD: `schedcpu()` in the reschedule
  IPI handler directly calls `sched_dispatch()`. Intel SDM Vol 3A §11.6: IPI delivery semantics (EOI-only is
  valid if the target honors a pending-reschedule flag). **Confident mapping** — our EOI-only design is
  correct under the timer guarantee, but the latency bound is unmeasured.

### CP-22 — `sti; hlt` serialization: IPI lost between check and halt
- **The trap.** The idle loop checks the ready queue (empty), then executes `sti; hlt`. Between the check and
  `sti`, another CPU enqueues a thread and sends an IPI. If the IPI arrives before `sti`, it is latched in the
  LAPIC IRR but not dispatched (IF=0). The `hlt` then executes, and when the IPI finally dispatches (after
  `sti`'s implicit re-enable), it wakes from `hlt`. This is **safe** on x86 because `sti; hlt` is architecturally
  serialized: SDM §9.7 guarantees that if an interrupt is pending when `sti` executes, the CPU will service it
  rather than executing the next instruction (`hlt`). The trap fires if the check-halt sequence is split by a
  window where IF=1 and the IPI can be both delivered AND consumed before `hlt`.
- **Where it bites our seam.** `idle_thread_entry` (`scheduler.rs:1259-1280`): `disable_interrupts()`, then
  `rotate()`, then on empty queue `enable_interrupts_and_halt()` (`:1279`). The `sti; hlt` is a single inline
  asm with `options(nomem)` (`arch/x86_64/mod.rs:330-348`). The check (`rotate`) runs IF-off, the `sti; hlt`
  is atomic. **This seam is guarded correctly.** The critical invariant: the ready-queue check must run IF-off,
  and `sti; hlt` must be a single instruction pair with no compiler-inserted memory accesses between them.
  Our `options(nomem)` on `enable_and_hlt` prevents the compiler from inserting loads/stores between `sti` and
  `hlt`.
- **Discriminator.** Review invariant: any idle loop that checks-then-halts must (a) check IF-off, (b) use a
  single `sti; hlt` asm block with `options(nomem)`. The existing code satisfies both. A negative control:
  split the asm into separate `sti` and `hlt` calls and verify that under load, IPIs are lost (they won't be
  on x86 due to SDM §9.7, but the compiler could insert memory accesses between them).
- **Guard.** The current code is correct. The `options(nomem)` on `enable_and_hlt` (`:339`) and the IF-off
  check in `idle_thread_entry` are the guards. Document this as a review invariant for any future idle-loop
  change (e.g., MWAIT-based idle).
- **Source.** Intel SDM Vol 3A §9.7 "Interaction Between HLT, MOV-TO-CR and External Interrupts": "if an
  interrupt is pending when STI executes, the processor will service it before executing the next instruction."
  Linux `cpu_idle.c`: the `sti; hlt` pair is in a single asm block with `"memory"` clobber absent. OSDev wiki
  *Sleeping/Waking CPUs*: "the check-then-halt race is solved by disabling interrupts around the check and
  using STI;HLT atomically." **Confident mapping** — our seam is guarded, but MWAIT upgrades must re-check.

### CP-23 — MWAIT-based idle loses IPIs (C-states and interrupt routing)
- **The trap.** Replacing `sti; hlt` with `MWAIT` for deeper C-states changes the interrupt wake semantics:
  `MWAIT` takes hint parameters (C-state depth, wait-type). At deep C-states (C3+), the core may not monitor
  all interrupt sources — specifically, **local APIC interrupts may be routed through the package's power
  management unit**, and IPI delivery can be delayed or lost if the APIC's TPR (task priority register) is
  below the interrupt's vector priority. Additionally, `MWAIT` does not have the SDM §9.7 serialization
  guarantee of `sti; hlt`: a pending IPI at the time of `MWAIT` may not wake the core if the C-state hint
  disables monitoring for that interrupt class.
- **Where it bites our seam.** Currently we use `hlt` only. If OT-47's efficiency pass introduces MWAIT, this
  trap becomes reachable. The `enable_and_hlt` function (`arch/x86_64/mod.rs:330`) would need to become a
  configurable idle primitive. Linux handles this with `cpuidle` drivers that verify IPI delivery at each
  C-state before allowing the deeper state.
- **Discriminator.** If MWAIT is introduced, gate it with a per-C-state IPI-delivery test: send a reschedule
  IPI while the target is in each C-state, assert wake within N cycles. Any C-state that drops an IPI must be
  excluded from the idle driver's state table.
- **Guard.** Until MWAIT lands, this is N-A. When it does: (a) start with `MONITOR/MWAIT` at C1 (shallowest),
  prove IPI delivery, then incrementally deepen; (b) keep `hlt` as a fallback for states that fail the IPI
  gate; (c) never use `MWAIT` with IF=0 — it requires IF=1 and relies on the APIC TPR for filtering.
- **Source.** Intel SDM Vol 3A §7.8 "MONITOR and MWAIT": "software must ensure that interrupts which can wake
  the processor from the wait state are enabled." Linux `drivers/acpi/cpuidle.c`: C-state validation, IPI
  delivery testing per state. FreeBSD `cpu_idle_acpi`: similar. **Verify-at-fix-time** — this is a future trap;
  our current `hlt`-only design avoids it entirely.

### CP-24 — Cross-CPU spawn pushes to wrong runqueue (affinity mismatch)
- **The trap.** The BSP spawns a thread intended for an AP, but `spawn()` pushes to the **current CPU's**
  ready queue (the BSP's). Unless the scheduler has work-stealing or cross-CPU migration, the thread sits on
  the BSP's queue and the AP never runs it. This is not a race — it's a static scheduling bug: the spawn site
  assumes the thread will run on a specific CPU but pushes to the wrong queue.
- **Where it bites our seam.** `spawn()` (`scheduler.rs:463-468`) calls `push_ready_or_panic`, which pushes
  to `current_scheduler()` — the caller's CPU. All smoke-test threads (ping, pong, hog) are spawned on BSP and
  run on BSP. For cross-CPU work, `place_thread_on_cpu` (`:1657-1691`) exists but is `#[cfg(neo-test)]`. The
  OT-34 threaded-matmul uses `ap_poll` (lock-free slot claim) rather than scheduler-driven dispatch — it
  bypasses the runqueue entirely. If a future pass migrates AP work to the scheduler's cross-CPU enqueue, it
  must use `place_thread_on_cpu` or an equivalent production path. Currently no production code spawns on a
  foreign CPU's queue.
- **Discriminator.** Grep for any production call that expects a spawned thread to run on a non-BSP CPU without
  using `place_thread_on_cpu` or explicit migration. The `ap_poll` mechanism is self-verifying: each AP polls
  its own slot, so affinity is encoded in the slot index, not the scheduler.
- **Guard.** Keep `place_thread_on_cpu` test-only until a production cross-CPU enqueue primitive exists. Any
  future cross-CPU spawn must either (a) push to the target CPU's queue + send IPI, or (b) use a lock-free
  slot mechanism like `ap_poll`. Do not rely on work-stealing — we don't have it.
- **Source.** Linux: `select_task_rq()` (`kernel/sched/core.c`) assigns affinity; `set_cpus_allowed_ptr()`
  constrains it. FreeBSD: `thread_lock`, `TD_ON_RUNQ` + target CPU's runqueue. **Confident mapping** — our
  current design avoids this by using lock-free slots, but any scheduler-driven cross-CPU spawn must address it.

### CP-25 — AP bootstrap busy-yield starves BSP work (no voluntary yield under load)
- **The trap.** APs in `run_ap_bootstrap` (`scheduler.rs:341-357`) execute `ap_poll()`, `yield_now()`,
  `spin_loop()` in a tight loop. When an AP claims a matmul partition, it runs `run_disjoint()` (a compute
  kernel) **without yielding** — the partition execution is a tight inner loop. If all APs claim partitions
  and run them to completion without yielding, they never check the scheduler's ready queue. Any thread
  spawned on an AP's runqueue during this window is stranded until the partition completes (or the timer
  preempts). For large matrices, a single partition can take many milliseconds — enough for the BSP's join
  barrier to time out if it has other work that depends on AP-resident threads.
- **Where it bites our seam.** `ap_poll` (`par_matmul.rs:460-490`) runs `run_disjoint()` (line 487) in a tight
  loop with no yields. The AP's bootstrap loop calls `yield_now()` only **between** partitions, not during
  them. If the BSP spawns a thread on an AP's queue while that AP is mid-partition, the thread waits for
  partition completion + timer tick. This is **latency, not deadlock**, because the partition is bounded and
  the timer fires. But if the BSP's join barrier has a timeout shorter than the worst-case partition time, it
  could fire falsely.
- **Discriminator.** Measure partition execution time vs timer period. If partition_time > TICK_PERIOD, a
  cross-CPU spawn during partition execution will experience >1-tick latency. The existing `threaded_matmul_parity`
  test exercises the full fork-join but doesn't inject concurrent spawns on AP queues. Add a probe: BSP spawns
  a thread on each AP's queue mid-partition, asserts dispatch within N ticks.
- **Guard.** For bounded latency, either (a) insert periodic yield points in `run_disjoint()` (e.g., every M
  rows), or (b) accept the worst-case latency as bounded by partition size + timer period. Option (a) is
  cleaner but adds overhead; option (b) requires documenting the bound. Linux: `cond_resched()` in long kernel
  loops. **Confident mapping** — this is a real seam, currently bounded by partition size and timer.

### CP-26 — Partition join barrier races with AP parking (fork-join completion ordering)
- **The trap.** The BSP's join loop waits for all AP partitions to set `done=true`. An AP sets `done` via
  `Release` store (`par_matmul.rs:489`). If the BSP observes `done` but the AP's result stores are not yet
  visible (TSO store-buffer), the BSP reads stale output. Conversely, if the AP completes and returns to the
  bootstrap loop, it may claim a new partition before the BSP has observed `done`, leading to double-execution
  of a partition.
- **Where it bites our seam.** `run_generic` (`par_matmul.rs:450-454`): the join loop reads `slot.done` with
  `Acquire`. The AP writes `slot.done.store(true, Release)` (`:489`). This forms a correct release-acquire
  pair. The double-claim is prevented by the `pending` CAS (`:470-476`): once claimed, `pending=false`, so a
  second claim fails. The join loop only reads `done`, not `pending`. **This seam appears guarded**, but the
  join loop's termination condition (all `done`) must be ordered against the BSP's subsequent use of the output.
  After the join returns, the BSP reads the output — this is safe because the join's `Acquire` on `done`
  synchronizes-with the AP's `Release`.
- **Discriminator.** The parity test (`threaded_matmul_parity`) compares threaded vs serial output byte-for-byte.
  Any store-buffer reordering that lets the BSP read stale output would fail the parity check. The test is the
  discriminator here — it already fires on this class of bug.
- **Guard.** The `Release`/`Acquire` pair on `done` + the CAS on `pending` are the guards. Keep them. Any
  future change to the join loop must preserve the acquire semantics on the `done` reads.
- **Source.** Intel SDM Vol 3A §9.2: x86-TSO allows store→load reordering, but release-acquire pairs (via
  `AcqRel` CAS + `Release` store / `Acquire` load) establish happens-before. Linux: `smp_store_release()` /
  `smp_load_acquire()` in workqueue completion barriers. **Confident mapping** — our seam is guarded by the
  atomic orderings, verified by the parity test.

### CP-27 — Reschedule IPI delivered to an AP still in per-CPU init (bootstrap-before-ready)
- **The trap.** The BSP sends a reschedule IPI to an AP before the AP has finished its bootstrap sequence
  (per-CPU slab, scheduler, IDT). The AP's LAPIC accepts the IPI, but the IDT entry for the reschedule vector
  points to uninstalled handlers → triple fault or garbage execution. This is a CP-17 member specialized to
  the bootstrap window: the AP is "online" (LAPIC initialized) but not "ready" (IDT/scheduler not installed).
- **Where it bites our seam.** `ap_entry` (`smp.rs:179-220`): initializes CR0/CR4, calls `per_cpu::init_ap`,
  then `init_traps_ap`, `slab::init_per_cpu`, then `sched::run_ap_bootstrap`. The scheduler is installed in
  `run_ap_bootstrap` → `init_cpu_scheduler`. Interrupts are enabled at `scheduler.rs:339`, **after** the
  scheduler and IDT are installed. The BSP's `bring_up_aps` waits on `APS_READY` (`:158`), which is signaled
  at `scheduler.rs:336` — **before** IF=1, but after all per-CPU init. So the BSP cannot proceed to
  `smoke_test()` (which spawns threads) until all APs have completed bootstrap. **This seam is guarded by the
  readiness barrier.** However, if a future pass lets the BSP run work before the barrier clears, this trap
  reopens.
- **Discriminator.** The `APS_READY` barrier (`smp.rs:158`) with its timeout print is the discriminator. If
  the BSP ever proceeds before all APs signal ready, add a per-CPU `bootstrap_complete` flag checked before
  sending any IPI to that CPU.
- **Guard.** The readiness barrier + IF=0 during bootstrap are the guards. Strengthen: document that the BSP
  must never send IPIs to an AP until `APS_READY` confirms it, and that this ordering is enforced by the
  barrier, not by convention. Any future pass that decouples the BSP from the barrier (e.g., asynchronous AP
  bring-up) must add per-CPU readiness flags.
- **Source.** Linux: `cpu_online_mask` + `cpu_up()` returns only after the target CPU has reached
  `cpu_start()` → `start_secondary()` → calibrated idle. FreeBSD: `smp_started` barrier, AP sets
  `CPU_STATE_INITIALIZED` before returning from SIPI. Intel SDM Vol 3A §8.4.4: INIT-SIPI-SIPI sequence; the
  BSP must not send IPIs until the AP's IDT is valid. **Confident mapping** — our barrier guards this, but the
  guard is implicit (barrier timing) rather than explicit (per-CPU flag checked at IPI send time).

### CP-28 — Idle thread re-check race: ready queue non-empty between rotate() and hlt
- **The trap.** The idle loop calls `rotate(Ready, Ready)` IF-off. `rotate()` sees an empty queue, returns
  `None`. Between the `rotate()` return and `enable_interrupts_and_halt()`, a preemption-disabled but IF-on
  interrupt (e.g., from a device that bypasses the IF check — NMI, SMI) could theoretically trigger a context
  switch. However, our `disable_interrupts()` at `:1261` keeps IF=0 through the entire check-halt sequence, so
  this cannot happen. The real race: if `rotate()` releases the scheduler lock before returning (it does — the
  lock is held only during the queue manipulation), another CPU could enqueue a thread on this CPU's queue
  between `rotate()`'s lock release and our `hlt`. But because IF=0, the IPI is latched and will wake us from
  `hlt`. **This seam is guarded by the IF-off check + sti;hlt serialization (CP-22).**
- **Where it bites our seam.** `idle_thread_entry` (`scheduler.rs:1261-1280`): `disable_interrupts()`, then
  `rotate()`, which acquires and releases `sched.inner.lock()` internally. The lock is released before the
  function returns to the idle loop. Between lock release and `enable_interrupts_and_halt()`, a cross-CPU wake
  can enqueue + send IPI. IF=0 prevents dispatch, but latches the IPI. `sti; hlt` wakes on the latched IPI.
  **Correct.** But note: if the idle loop ever runs with IF=1 between check and halt (e.g., an optimization to
  reduce latency), this race reopens. Linux avoids it by holding the rq lock across the check-halt transition
  in some configurations, or by using `TIF_NEED_RESCHED` checked after `sti`.
- **Discriminator.** The existing code is correct. A negative control: enable interrupts between `rotate()` and
  `hlt`, run under cross-CPU wake load, and verify that threads are not stranded (they won't be on x86 due to
  SDM §9.7, but the IPI could dispatch and call `rotate()` again before `hlt`, which is wasteful but correct).
- **Guard.** IF-off across the entire check-halt sequence + atomic `sti; hlt` are the guards. Document this as
  a review invariant: "the idle loop's ready-queue check through halt must be IF-off, with a single `sti; hlt`
  to re-enable." Any future optimization that enables interrupts early must add a `need_resched` re-check.
- **Source.** Linux `cpu_idle_loop()` (`kernel/sched/idle.c`): holds rq lock across check in some
  configurations; checks `TIF_NEED_RESCHED` after `sti`. FreeBSD `mi_cpu_idle`: similar IF-off check + `sti;hlt`.
  Intel SDM §9.7: STI-HLT serialization. **Confident mapping** — our seam is guarded, but any future
  optimization must re-check.

### CP-29 — Concurrent AP per-CPU heap init contends the shared buddy lock (real-silicon-only deadlock)
- **The trap.** Bringing all APs up at once (start every AP, then wait at one barrier) lets their per-CPU init
  run concurrently. Each AP's `slab::init_per_cpu` (+ its scheduler `Box`) allocates from the shared
  buddy/heap allocator. On real multi-core silicon this deadlocks two ways, both invisible under QEMU's
  serialized single-core TCG: (1) N APs contend the one buddy/heap lock with a wide hold window (768 MiB
  Stage-B heap + MAX_ORDER 16); and (2) any TLB-shootdown IPI raised during a heap op (buddy split/coalesce,
  heap growth) targets sibling APs still in per-CPU init with **IF=0** (`init_traps_ap` loads the IDT but does
  not `sti`) — those APs cannot service the IPI, so the shootdown sender spin-waits for an ACK that never
  comes. The pre-existing `APS_READY` barrier only ordered **BSP-vs-AP** (so the BSP's later IF=0 thread spawns
  don't race the APs' allocations); it did nothing for **AP-vs-AP** init contention.
- **Where it bit our seam.** `smp::bring_up_aps` (`smp.rs`): the 12-core OptiPlex bare-metal boot hung
  intermittently mid-bringup — serial froze at `gdt: cpu=N loaded` (inside `init_traps_ap`, before
  `slab::init_per_cpu` completed and the AP signaled `AP cpu_id=N ready`), the ready-barrier never clearing.
  Intermittent because it depends on the lock/shootdown interleaving; the same ISO booted clean on other tries.
- **Discriminator.** Serial freeze mid-bringup at `gdt: cpu=N loaded` with NO following `AP cpu_id=N ready` and
  NO `AP ready-barrier cleared/TIMEOUT` line ⇒ an AP wedged in per-CPU init. QEMU (serialized TCG) cannot
  reproduce it — **the bare-metal boot IS the test.** Post-fix negative control: `started N AP(s) (serialized
  bringup)` + `ready-barrier cleared (N/N up)` + boot proceeds to `model_load`.
- **Guard (FIXED, `c498f5ce`) — two guards, one per vector; do NOT conflate (AAG 2026-07-19).** The serialize
  fix closes **vector (1) ALONE**: start one AP, wait for it to finish per-CPU init and signal `APS_READY`,
  THEN start the next — only one AP allocates at a time, so no AP-vs-AP buddy-lock contention. **Vector (2)
  (shootdown to an IF=0 AP) is closed by OT-59's I-16 `shootdown_ready_cpus_mask`, NOT by the serialization** —
  `signal_ap_ready()` is called just *before* `enable_interrupts()`, so a just-signaled AP has an IF=0 tail,
  but it isn't in the shootdown-ready mask until `mark_tlb_shootdown_ready()` (after `sti`), so no shootdown
  targets it. **Both guards must stay** — removing the I-16 mask on the belief that ordering covers the IF=0
  case would reopen vector (2). The final barrier is a fail-safe backstop; note it is diagnostic-preserving but
  does **not** recover a boot if a wedged AP holds the global buddy lock (the held lock cascades). **Validated
  on the OptiPlex** (11 APs up clean, boot proceeded to Arthur's reply).
- **Source.** Standard SMP-bringup discipline: Linux historically brought CPUs online one at a time
  (serialized `cpu_up()`) precisely to avoid concurrent-init races; the shootdown-to-a-CPU-that-can't-ACK
  hazard is the classic "never IPI-and-wait on a CPU with interrupts masked." Read for principle, never copied.
  **Confident mapping** — the fix removes both mechanisms by construction; validated on silicon.

---

## Category J — Teardown / force-kill of blocked threads, and per-thread reclamation (OT-61 C1.1)

Category J is the **ownership** category. Categories A–I ask *"can two CPUs reach this state in a bad
order?"*; J asks *"when the kernel retires a thread without letting it return, what did that thread still
own — and who frees it?"* The OT-61 C1.1 slice (threads + futex) added the kernel's first user-blocking
syscalls beyond IPC, which is exactly the seam where this class becomes reachable.

**Status honesty for CP-31…CP-34.** All four are **repaired in code with a falsifiable discriminator that
passes locally**, but they are marked *re-gate pending* rather than *Fixed*: the corrected candidate has not
yet run its exact-SHA 4-shard QEMU gate (the runner token is PM/PL-controlled). Promote each to **Fixed** only
when that gate is green on the exact SHA. CP-30 is marked Fixed — it was proven by the earlier C1.1 gate — but
its guard was *re-implemented* by blocker F below, so it too is re-proven on the corrected candidate's gate.

### CP-30 — Force-zombify frees a blocked thread's kernel stack as raw memory ⇒ owned `Arc`/`Box`/guard on that stack leaks
- **The trap.** When the kernel **force-retires** a thread that is BLOCKED inside a syscall — the forced-kill
  and fault-kill paths (`kill_thread_by_pid` / `zombify_parked_thread_by_pid` → the zombie reaper →
  `Thread::free`) — it reclaims that thread's kernel stack (`Box<[u128]>`) as **raw memory**: no unwinding, no
  `Drop`. Any owned reference-counted resource living in that syscall's stack frame — an `Arc<Process>`, an
  `Arc<Pmap>`, a `Box`, a lock guard — has its owner **leaked**. A leaked `Arc<Process>`/`Arc<Pmap>` strands
  the *entire process* and its `Pmap`: the process never reaches refcount zero, so `Pmap::drop` never runs and
  every user frame it owns leaks. Normal wakeup returns *through* the frame and drops everything; only
  force-teardown abandons the stack, so the leak is invisible until a process is killed while a thread is parked.
- **Where it bit our seam.** `futex_wait` and `thread_join` both initially held an owned `Arc` across the
  block: `sys_futex_wait_impl` kept the caller's `Arc<Pmap>` live for the value re-check, and
  `thread_join_for_current` held `Arc<Process>` to reach the per-process thread registry. On a
  guard-page-overflow **fault-kill** of a multi-threaded process (one thread parked in `futex_wait`, one in
  `thread_join`), both Arcs leaked and stranded the whole `Pmap` — observed as a **+157-frame leak** that
  tripped the boot's baseline==final frame-leak assertion.
- **Root mechanism.** The slab reaper (`Thread::free`) is a *raw* free by design: it drops the tracked owned
  fields (`stack`, `pmap`, `fpu_state`) explicitly, then returns the storage to the slab **without running
  `Drop` on the stack contents**. That is correct for a thread that parked cleanly — a parked thread's syscall
  frame is dormant and its owned locals are reclaimed when it *resumes*. Force-zombify skips the resume: the
  frame is never unwound, so whatever it owns is orphaned.
- **The invariant.** **A user-blocking syscall holds NO owned reference-counted resource across
  `block_current_*`.** Translate/copy exactly what the blocked path needs into `Copy` values (a physical
  address, a tid, a deadline) and **drop every `Arc`/`Box`/guard before parking**; reach shared state that must
  survive the block through a pointer whose target is kept alive by a *registry*, not by the blocked thread's
  own stack. This is the ownership analogue of CP-14: CP-14 governs *lock state* at the park point, CP-30
  governs *ownership* — the blocked thread must own nothing whose `Drop` is load-bearing.
- **Our guard (fixed; re-proven on the corrected candidate).**
  - **`futex_wait`:** translates `uaddr → paddr` and **drops the `Arc<Pmap>` before** parking; the core
    primitive `wait_on_key` takes a `Copy` key plus a borrow-free re-check closure and holds only the static
    `FUTEX_QUEUE`. The value re-check reads `uaddr` directly — the caller's own CR3 is active, so no Arc is needed.
  - **`thread_join`:** captures a **raw pointer** to the per-process registry lock and **drops the
    `Arc<Process>` before parking**. `PROCESSES` registration keeps the process alive for as long as this
    thread — a member of that pid — is unreaped: `on_thread_reap`, which deregisters it, cannot run until this
    thread is reaped, which cannot happen while it is parked. *(Blocker F re-implemented this park on the
    futex done-word; the no-owned-Arc discipline is unchanged and still the load-bearing part.)*
  - **`sys_wait`:** the same rebind — an owned `Arc<Process>` to the *target* is dropped before the block,
    because a `sys_wait`-parked thread is itself force-zombifiable (see CP-33).
  - **`fault_kill` / `thread_exit`:** run the *minimum* in a fragile context and `drop(proc_arc)` **before**
    `scheduler::exit()`; heavy teardown runs on the LAST thread's reap via `on_thread_reap`, from the reaper's
    clean kernel context — never inline in an exception frame.
- **Discriminator (armed).** The boot's **frame-leak audit** (`mm::frames_in_use()` baseline captured
  pre-spawn, asserted `== final` post-reap) catches a stranded `Pmap` deterministically — it caught this at
  +157. The `threads_guard_fault` battery is the permanent regression.
- **Corollary.** Any new user-blocking syscall (C1.2 TLS wait, sealed-handle waits, signal wait, file-I/O
  completion) must re-assert this at review: *what owned resources are live across the block, and is each
  either `Copy`, dropped-before-park, or kept alive by a registry rather than by this frame?*
- **Source.** Ours — surfaced and fixed at our slab-reaper + force-zombify seam. The general principle
  (don't strand owned resources on a stack you reclaim without unwinding) is the ownership analogue of the
  wait-queue `finish_wait` discipline; re-derived here, not copied.

### CP-31 — Off-ring token parking frees a ready slot ⇒ the wake has no re-entry capacity and panics
- **The trap.** A scheduler that parks a blocked thread by moving its owned `Box<Thread>` **out of the ready
  ring** into a separate side table (a token/park slot) has silently **returned that ring slot to general
  admission**. New threads can then fill the ring to capacity while the parked thread still, by construction,
  *must* be able to come back. The wake path consumes the park token and then enqueues into a **full** ring. If
  that enqueue is infallible-by-panic, a perfectly valid futex wake / timer expiry / join completion **panics
  the kernel**; if it silently drops, the extracted waiter is lost — the thread is owned by nobody and never runs again.
- **Why it survives ordinary tests.** Every single-waiter test passes: park one, wake one, ring never near
  full. The defect needs *backpressure* — a full ready ring at the exact moment of a wake — which no
  functional test produces by accident. This is the capacity analogue of a lost wakeup.
- **The invariant.** **Parking must not surrender re-entry capacity.** A thread that left the ready ring to
  block is still *live*, and the scheduler owes it a slot. Either (a) cap **admission** by the live-thread
  total rather than by the physical ring occupancy — so `ready.len() ≤ live_count ≤ capacity` and a re-entry
  push provably fits — or (b) make the wake enqueue **fallible** and *retain/restore the token* on failure so
  the waiter stays owned exactly once and is retried. Never (c): consume the token, then push into a ring that
  may be full.
- **Our guard.** Admission (`try_push_ready` / kernel-thread admission) goes through a single
  `admit_ready_locked` chokepoint capped at `SCHEDULER_RING_CAPACITY` by a per-CPU `live_count`; wake/yield
  re-entry pushes never raise `live_count`, so they always fit. `live_count` is decremented at **reap** (the
  thread genuinely leaves the live population). Report `Woken` / increment the futex wake count only **after**
  the waiter is actually queued exactly once.
- **Discriminator (`token_wake_ring_pressure`, armed).** Deterministically park a token waiter, fill that
  CPU's ready ring to the admitted limit **with interrupts off** (so no filler drains a slot between fill and
  wake), then fire the wake. The kernel must stay live, report `Woken`, and the waiter must resume exactly
  once with its wake value. Non-vacuity: on the pre-fix scheduler the fill reaches physical-full and the wake
  reaches the panicking enqueue.
- **Source.** Ours (our token-park design). The general shape — *reserve capacity for threads you removed
  from a bounded queue but still owe a slot* — is the standard bounded-queue admission discipline.

### CP-32 — A per-thread resource leak hidden by a whole-process teardown audit
- **The trap.** A leak audit that measures **at process exit** cannot see a **per-thread** leak, because the
  process-wide teardown (dropping the whole `Pmap`) frees the leaked resource *as a side effect* and the audit
  then reads clean. The audit is not wrong; it is **vacuous for this defect class**. Concretely: created
  thread stacks were mapped at `thread_create` but only unmapped on *creation rollback* — normal join/detach
  removed the registry metadata and left the mapping. A long-lived process (exactly the browser case) could
  cycle through thousands of threads, leaking every completed stack, and still pass a green final frame audit.
- **The invariant.** **Every per-thread resource is freed at a per-thread boundary, proven while the process
  is still alive.** Retain created-stack ownership until the target is off *both* its user and kernel stacks,
  then unmap/free it **exactly once** at the join/detach/zombie-collection boundary — never the initial
  process stack, never a live thread's stack. Whole-process teardown remains the backstop for the last thread only.
- **Audit-design rule (the general lesson).** *An audit whose measurement point is outside the lifetime of
  the thing it audits proves nothing about it.* When adding a per-X resource, ask what the existing audit's
  measurement point is; if it is per-process and the resource is per-thread, the audit must be **re-scoped**,
  not merely re-run. This is the vacuity twin of the round-trip-test trap in the byte-oracle rule.
- **Our guard.** `note_reaped` returns the retiring thread's stack bounds when the thread was `created` **and
  the process survives**; `on_thread_reap_threaded` unmaps them after releasing the registry lock (never hold
  a spinlock across a TLB shootdown). Record lifetime is decoupled: `reaped` (stack freed) and
  `collected`/`detached` (status consumed) are separate, so the unmap happens exactly once regardless of
  whether join collects before or after the reap.
- **Discriminator (in-life stack-reclaim probe, armed).** Warm the page-table overhead, then run
  substantially more than `MAX_THREADS_PER_PROCESS` sequential cycles covering spawn→join, spawn→detach, and
  zombie→later-join/detach, and assert **while the parent is still alive** that each retired stack now
  translates **absent**. The probe must be **non-destructive** (`PROT_READ|PROT_WRITE`, the stack's own
  permissions) — probing with `PROT_READ` alone strips write access from a *live* detached thread's stack and
  fault-kills the process, which is a test bug that masquerades as a kernel bug. Non-vacuity: on the pre-fix
  kernel the base stays mapped and the probe never reports absent.
- **Source.** Ours. The general principle is standard resource-accounting hygiene (measure inside the
  lifetime); the specific vacuity was found by adversarial audit, not by the green gate.

### CP-33 — Process kill drains only *some* owning wait queues ⇒ teardown never completes
- **The trap.** In a kernel where blocking primitives **take ownership** of the blocked thread's `Box<Thread>`
  and park it in *their own* queue, a process kill must drain **every** such queue. Miss one, and that
  sibling's ownership keeps the process's live-thread count nonzero forever: teardown never completes, the
  parent's `wait` never returns the exit status, and the process becomes an unkillable zombie holding its
  address space. Our fault-kill purged futex state and handled Ready/token-parked siblings, but **IPC**
  rendezvous queues and the **process-wait** queue each park threads on their own path — and a `sys_wait`er
  parks in the *target's* queue, not its own process's, so a naive "walk my own structures" sweep misses it.
- **The invariant.** **Kill completeness is enumerable, not incidental.** For every construct that can own a
  parked `Box<Thread>`, the kill path must have a corresponding extraction — each queue losing the target
  **exactly once**, with stale generation/token wakes remaining harmless no-ops. Adding a new
  `block_current_into*` site is therefore a **two-part** change: the park, *and* its kill-path drain. A
  blocking primitive that owns threads without a drain hook is incomplete by construction.
- **Context discipline (why it is split).** A ring-3 `#PF` runs in a **fragile exception frame**: no heap
  allocation, minimal locking. So the fault path does only the minimum — set a `fault_killing` flag, purge the
  futex/join queue, yank Ready + token-parked siblings — and **defers** the heavy walk (handle table, IPC
  endpoints, every process's wait queue) to the **reaper's normal kernel context**, which is safe to walk. The
  drain is idempotent and re-runs per reap until the live count reaches zero.
- **A bounded scan is not a complete scan (the sub-trap).** Keeping the deferred walk alloc-free invites
  snapshotting each owning collection into a **fixed-size stack array** — which silently reintroduces the very
  defect, because the underlying collections are unbounded (a `Vec` handle table, a `BTreeMap` process
  registry). Stopping at the bound leaves a sibling parked, and that stalls teardown exactly as a missing
  drain would. Nor does it self-heal: the drain re-runs only when *another* sibling reaps, which is precisely
  what the missed sibling prevents, so a logged truncation is still a permanent stall. The fix is to **page**
  each scan and run it to exhaustion. Page the process registry **by key** (pids strictly greater than the
  last seen), not by index: teardown deregisters processes while the drain runs, and an index-based page would
  shift and skip an entry, whereas a key cursor is stable under concurrent insertion and removal.
- **Rule of thumb.** In a kill/teardown path, a fixed bound is only safe when the thing it bounds is *itself*
  provably bounded. Otherwise the bound is a silent correctness limit wearing the costume of a safety limit.
- **Discriminator (all-queue `threads_guard_fault`, armed).** A child leaves one sibling blocked in **IPC**
  and another in **process-wait**, plus the futex/token-parked witness; each sets a "reached the blocking
  call" flag so the orchestrator can confirm all queues are genuinely occupied **before** it triggers the
  guard-page overflow (non-vacuity — the test must prove the states were actually reached, not merely
  intended). The parent must then observe exit 139 within a bounded deadline, every parked owner must drain
  exactly once, and the pre-exit frame audit must be clean.
- **Source.** Ours. The class is the classic "kill must reach every wait channel" completeness problem
  (Linux's per-wait-queue teardown is the same shape); our version is sharper because our primitives *own*
  the thread object rather than referencing a shared task struct.

### CP-34 — Feature-exclusive root-cap assembly: every single-feature test green, the composed product broken
- **The trap.** Assembling a fixed set (here: the trusted root capabilities handed to `init`) with
  **mutually-exclusive `cfg` arms** — one arm per feature combination — scales combinatorially and silently
  encodes "these features are never on together." Each feature's own scenario enables exactly one feature, so
  **every single-feature test passes**, while the real product — which enables several at once — falls into an
  arm that installs only *some* of the authorities. Here `browser-runtime + net-dial-cap` selected the
  NetDial-only arm, so `init` never received `BrowserRuntime` and `SYS_THREAD_CREATE` failed `EPERM` in
  exactly the configuration the product ships.
- **Why it is a concurrency-bank entry.** It is not a race, but it is the same *composition* failure mode the
  bank exists for: correct in isolation, wrong when composed, and invisible to tests that never compose. It
  belongs next to CP-31/CP-33 because the C1.1 audit found all three by asking "what did the green gate NOT cover?"
- **The invariant.** **Assemble capability sets compositionally, never combinatorially.** Build the set by
  *accumulation* — each enabled authority pushes its cap in a fixed, documented order — so enabling N features
  installs N authorities by construction. Preserve: a stable handle order (children depend on it), fail-closed
  default-off (no feature ⇒ no authority), no userspace self-mint, and delegation/inheritance as the only path
  for children. Genuinely self-contained scenarios may still be an exclusive arm, but that must be a stated
  exception, not the default shape.
- **Our guard.** A single accumulating builder with a documented order — `[0] NetDial`, `[1] GpuSubmit`,
  `[2] BrowserRuntime` — replacing the exclusive `cfg` arrays.
- **Discriminator (`threads_cap_compose`, armed).** An **exact combined-feature** scenario builds with
  `browser-runtime + net-dial-cap` together and proves *both* authorities are live: `thread_create` succeeds
  **and** a NetDial derive from `Handle(0)` succeeds, while a child with no delegated cap still gets `EPERM`.
  A single-feature test cannot catch this class — the discriminator must itself be a *composed* build.
- **Source.** Ours. The general lesson is standard combinatorial-configuration hygiene: *if your build system
  can express N independent features, your tests must cover at least one composed configuration, because the
  shipped product is always a composed configuration.*

### CP-35 — A wake that RELOCATES a thread breaks per-CPU accounting unless it transfers the reservation
- **The trap.** The moment a scheduler keeps **per-CPU** accounting (a reservation, a quota, a live count),
  every path that moves a thread between CPUs becomes an accounting event — not just a queue operation. Miss
  one, and the source keeps a **phantom reservation forever** while the destination gains a thread it never
  reserved. The per-CPU invariant (`ready.len() <= live_count`) then silently goes false on the destination,
  and every guarantee derived from it evaporates. Here the guarantee was CP-31's "a wake can always re-enter
  the ready ring", so the full-ring wake **panic became reachable again** — the fix for CP-31 looked complete
  while the property it promised was false under migration.
- **Why the tests didn't catch it.** The migrating path existed and was exercised — IPC rendezvous deliberately
  hands the parked peer to the **completer's** CPU — and there were already cross-CPU batteries driving it.
  They asserted *delivery* (the message arrived, the thread ran) but never the *accounting invariant*. A green
  SMP gate therefore coexisted with a broken invariant for as long as the invariant went unasserted.
- **The invariant.** **Every thread-relocating operation is a reservation transfer, under a destination
  capacity check.** Enumerate them: admission, wake-to-token-owner, wake-to-current-CPU (handoff), forced
  zombification, and any future work-stealing or load-balancing. A relocation that cannot fit the destination
  must fall back to the CPU that still holds the reservation, never force the destination over capacity.
- **Our guard.** `wake_thread_front` (the IPC handoff) checks the destination's capacity, transfers the
  reservation, and on a saturated destination wakes on the **source** — which provably has room, because the
  thread is counted there but not in its ready ring — plus a reschedule IPI. `wake_thread_token` was already
  correct: it targets `scheduler_for_cpu(token.cpu_id)`, i.e. the reserving CPU, so it never relocates.
- **Lock-order note (why the accounting is split).** The transfer deliberately does **not** hold both per-CPU
  scheduler locks. Two CPUs migrating toward each other would be a textbook ABBA deadlock. Incrementing the
  destination first and releasing the source in a separate scope leaves, at worst, a transient **over-count on
  the source** — which is conservative: it can only make that CPU admit *less*, never violate
  `ready.len() <= live_count`. **Prefer a bounded conservative skew to a new deadlock seam.**
- **Discriminator (`smp_c11_migration_reservation`, armed).** Drives real cross-CPU rendezvous migrations and
  asserts the invariant on every CPU at baseline, after **each** iteration (so drift is attributed to the
  migration that caused it), and again after drain-to-quiescence. Fails loudly on <2 CPUs rather than passing
  vacuously — with one CPU `src == dst`, no migration occurs, and a green result would mean nothing.
- **Source.** Ours, found by a **cross-model** adversarial check: the author and the entire AAG panel were
  Claude-family and shared the blind spot; a Codex-family lens built the interleaving. This is the concrete
  case for the cross-model rule on SMP work.

### CP-36 — The Ready→Running window is invisible to both kill scans; generation checks ownership, not lifetime
- **The trap.** A force-kill that enumerates threads by **where they are parked** has a hole: a thread that was
  *validly woken* is briefly neither parked nor yet running-and-visible. Concretely, the ready-only kill scan
  (`kill_thread_by_pid` walks `ready`) and the parked-only token scan each see half the world, and a thread
  transitioning Ready→Running is in **neither**. If teardown then drops the last `Arc<Process>`, that thread
  resumes and dereferences freed memory.
- **Why generation checks don't help.** Token generations serialize *ownership* of the parked `Box<Thread>` —
  they answer "may I take this thread?". They say nothing about *lifetime*. In the failing interleaving the
  wake is entirely legitimate: the token is current-generation and correctly returns `Woken`. The failure is
  strictly **after** the token check. `Stale`/`Gone` cannot fire, because nothing is stale.
- **The interaction with CP-30 (the real subtlety).** CP-30 forces a blocking syscall to own **nothing** across
  the park, or force-zombify leaks it — so `thread_join` keeps only **raw** pointers to the registry and the
  done-word. That is leak-safe and lifetime-*unsafe*: the two rules pull in opposite directions. Resolving one
  in isolation reintroduces the other.
- **The invariant.** **Anything a suspended thread will dereference on resume must be kept alive by the
  scheduler, not by that thread's own stack.** Put the owning reference in the `Thread` (which the scheduler
  owns and whose reaper drops explicitly) — then the stack owns nothing (CP-30 holds) *and* the target
  outlives every thread that can still touch it (no UAF).
- **Our guard.** `Thread::process: Option<Arc<Process>>`, installed at user-thread creation and released in
  `Thread::free` at reap. Generation widened to `u64` so counter reuse cannot defeat the ownership check
  either (that was always a separate, theoretical concern — never the lifetime fix).
- **Cost, stated honestly.** `Process` transitively owns parked `Box<Thread>` values (its wait queue; IPC
  endpoints reachable from its handle table), so a mutual wait between two processes can form an `Arc` cycle.
  It is bounded and non-fatal — teardown drains those queues and `Thread::free` clears the field — and a
  bounded leak in a pathological mutual-wait is strictly better than a use-after-free.
- **KNOWN GAP (tracked, not closed here).** Closing the *memory-safety* hole does **not** close the
  *containment* hole in the same window: a **currently-running** sibling still escapes a whole-process
  fault-kill, because there is no running-sibling retirement. Doing that needs a process-kill flag checked at
  **syscall/interrupt return** plus an **IPI** to retire running siblings — a new arch mechanism (no such hook
  exists today), scoped as its own gated build. Until it lands, the guard-fault battery asserts memory-safe
  containment and parked/ready drain **only**, and must NOT be read as proving complete sibling retirement.
- **Source.** Ours, same cross-model check as CP-35. The general lesson generalizes past this kernel:
  *an enumeration-based kill is only as complete as its enumeration, and a liveness token is not a lifetime
  token.*

### CP-37 — A non-IRQ-safe spin lock taken from BOTH preemptible and IF=0 contexts ⇒ unbreakable SAME-CPU deadlock

- **Failure mode.** Thread A takes a **non-IRQ-safe** spin lock (a plain `spin::Mutex` — no interrupt masking)
  from a context where `IF=1`, so its critical section is fully **preemptible**. A timer tick lands inside the
  section and the scheduler moves A off-CPU — **still holding the lock**. Thread B is then dispatched on the
  *same* CPU, wants the same lock, and spins for it with `IF=0` (e.g. a syscall body, where `SYSCALL` masked IF).
  Now neither side can move: B is spinning rather than blocking, so it never yields the CPU, and with interrupts
  masked no tick can preempt B — so A can never be rescheduled to release the lock. **One CPU deadlocks against
  itself via two threads**; no peer is involved, so a 4-CPU machine with three healthy idle APs still hangs.
- **Root mechanism.** A spin lock's correctness argument is "the holder runs to completion in a bounded
  section." That silently assumes the section is **non-preemptible**. It is not, unless the lock masks
  interrupts. The asymmetry is what kills: the same lock is reached from a preemptible context (a kernel
  thread — here the reaper) *and* from a non-preemptible one (a syscall at `IF=0`). Either context alone is
  survivable; the mix is not.
- **The near-miss worth recording.** The first fix attempt assumed the preemption window was a *specific* call
  inside the section — the TLB shootdown, whose `SHOOTDOWN_LOCK` wait deliberately re-enables interrupts
  (`tlb_shootdown::acquire_shootdown_lock`) so the CPU can still ACK a peer. Moving the shootdown out of the
  critical section was **necessary but not sufficient**: the hang reproduced unchanged, with the identical
  holder/waiter pair. The window was never one call — it was the **entire** section, because the lock masked
  nothing. Do not go hunting for the one blocking call inside a preemptible critical section; the whole section
  is the window.
- **Why it is invisible.** The stuck waiter spins with `IF=0`, so **that CPU takes no timer interrupt** — and on
  this kernel the timer is PIC IRQ0, delivered to the BSP only, with no LAPIC timer. When the BSP is the stuck
  CPU, *every* tick-driven diagnostic dies with it: watchdogs, heartbeat prints, deadline scans. The guest goes
  **completely silent, with no panic**, until the harness times out. Diagnosis is only possible from **outside**
  (a QEMU monitor register/stack dump) or from a **bounded-spin check at the lock site itself**.
- **Our exposure — REPAIRED (OT-61 C1.1).** `Pmap::inner` was a plain `spin::Mutex<PmapState>`, taken by the
  thread-reap path (kernel context, `IF=1`, preemptible) via `unmap_user_range` *and* by the thread-spawn
  syscall path (`IF=0`) via `allocate_user_region_guarded`. Exactly the asymmetric mix above.
  **The fix is two parts, and both are load-bearing:**
  1. **`Pmap::inner` is now an `IrqSafeMutex`** — held with interrupts masked, so no holder can ever be
     preempted mid-section. This is the part that actually closes the deadlock. (`IrqSafeMutex` gained a
     `try_lock` for the detector below; it restores the caller's prior IF state on failure so a retry loop does
     not silently run with interrupts forced off.)
  2. **The TLB shootdown moved OUT of the critical section** — a prerequisite for (1), not an alternative to it:
     an `IF=0` section must never wait on another CPU (CP-2/CP-3), and a shootdown both IPIs peers and waits for
     their ACKs. Callers now do the PTE writes under the guard, **drop it**, and only then flush — the discipline
     `process.rs` already documented for the process-registry lock ("never hold the registry spinlock across
     it"). Two ordering obligations were preserved across the move: flush **before freeing** any frame an unmap
     returned (else a stale translation outlives the frame into a reallocation — a real UAF), and flush **before
     reporting success** for a permission downgrade. Ranged operations now do **one** shootdown for the whole
     range instead of one per page.
- **Discriminator (armed, keep it armed).** A bounded-spin detector on `Pmap::inner`
  (`kernel/src/mm/pmap.rs`, `neo-test`-gated): each of the 7 acquire sites is labelled, records the acquirer
  (cpu / tid / site / pmap), and a stuck acquire **panics** naming both sides plus a verdict — same tid ⇒
  re-entrant acquire; different tid ⇒ an off-CPU holder (parked, preempted, or force-zombified). Recording only
  the *acquire* is sufficient and not a shortcut: while a lock is held no further acquire can succeed, so the
  last successful acquire **is** the current holder exactly when a waiter is stuck. This converted a 180 s
  silent timeout into a one-line attribution and is what closed the case.
- **How it presented (worth recognising again).** `threads_futex` reddened the 4-shard gate ~2-in-3, and
  reproduced **4/4 locally at `NEO_SMP=4`**. It was framed for a long time as a *futex lost wakeup* — the
  freeze always followed a `thread_create`, and the visible symptom was "threads parked, never woken, total
  silence." A cross-family (Codex) lens proved the futex park/wake ordering correct, and the idle `sti; hlt`
  path was proven correct too; only then did a monitor capture show all four vCPUs in `_mm_pause`, with the BSP
  at `RFL=0x02` under `allocate_user_region_guarded` ← `thread_create_for_current`. Panic evidence: holder
  `cpu=0 tid=6 unmap_user_range`, waiter `cpu=0 tid=11 allocate_user_region_guarded`, same `Pmap`.
  **Lesson: "parked and never woken" is the symptom of a lost wakeup AND of a wait-free deadlock upstream of
  the wakeup. Do not let the symptom pick the subsystem.**
- **The grep-able rule this leaves behind.** For any lock that does **not** mask interrupts, no critical section
  may contain a call that can enable interrupts, block, park, or wait cross-CPU. Candidates to audit are
  `spin::Mutex` (not `IrqSafeMutex`) critical sections that call into TLB shootdown, the frame allocator's
  slow path, or anything reaching the scheduler.
- **Citation.** Ours (OT-61 C1.1). The general class is standard practice elsewhere: Linux requires
  `spin_lock_irqsave` whenever a spinlock is shared with interrupt context precisely so a holder cannot be
  preempted into this state, and its TLB-shootdown IPI wait is deliberately not performed under the page-table
  spinlock. Read for principle; no code copied.
- **Second instance, reached through an accessor (OT-66 K1a, Janus #112 F2, repaired).** The lock need not be
  visible at the call site. Spawn ran `is_spawn_inheritable_dir_cap` under the caller's IF-off handles lock, and
  it called `OpenFile::is_directory()`, which read `inode.kind()`. For NeoFS that takes the inode's
  `record: SwitchMutex`, a preemptible lock the readdir path holds with `IF=1`. Same pair: one holder preempted
  IF-on, one waiter on the same CPU spinning IF-off. The repair records the kind in the `OpenFile` at open (an
  inode's kind never changes), so the IF-off check reads a plain field. **Audit rule this adds:** an accessor
  called inside an IF-off section is part of that section. Before calling a trait method there (an `Inode`, a
  `BlockDevice`, a filesystem), read what it locks, or record the value earlier, IF-on.
- **Third instance: a new IF-off thread on locks an IF-on thread already held (correction, 2026-10-29, OT-95 #215
  and OT-96).** The entry above reads as if the IF=0 side is always a syscall. It need not be, and the hazard is
  symmetric: adding any new taker changes which contexts share the lock.
  1. **An IF-on kthread against IF=0 syscalls.** `net-tick` (and OT-96's `net-rx`) first took the net spin locks
     (`tcp`'s `CONNECTIONS`/`OUTBOUND`, `socket`'s `BOUND_SOCKETS`, ARP, DHCP, all plain `spin::Mutex`) with
     IF on. Every socket syscall takes the same locks at IF=0. A tick preempting the thread mid-section, then a
     syscall on that CPU, is this entry's pair exactly. **Discriminator:** `socket_teardown` on a kernel with
     `wire-rx` hung 3 runs in 4 (silent, then the 180 s budget); with the thread's flush removed, 8/8 passed;
     with every net step under `without_interrupts`, 10/10. The test harness, also a kernel thread, takes the
     same locks IF-on, and hung against the IF-off thread the same way (`wire_rx_demux` 2 hangs, then 10/10 with
     the harness IF-off). Harness scenarios that drive TCP or a NIC IF-on (`phase4_tcp_connect_stream_close`,
     `phase4_dhcpv4_ipv4_udp`, `phase4_virtio_net_smoltcp_wrap_smoke`) hang if built with `wire-rx`, for the
     same reason; no CI kernel combines them.
  2. **The reverse: making the new thread IF-off exposes the IF-on takers it now meets (#215 AAG).** The tick,
     now IF-off, takes the device table (`drivers/net/mod.rs` `REGISTRY`) and the interface registry
     (`net/identity.rs` `REGISTRY`), plain `SwitchMutex`es that the USB port-service kthread takes IF-on on
     hotplug (`cdc_ecm::register_device`, `identity::register`) and `/net/if` reads take IF-on
     (`vfs_body_if_on`). UDP-send syscalls already took them at IF=0, so it was live on main at a lower rate.
     **Repair:** both are `IrqSafeMutex`, and nothing prints under them.
  **Audit rule this adds:** a new IF-off taker (a kthread, a syscall, a timer path) of an existing plain lock
  means listing every existing taker and its IF state, not only the new one's. The fix that holds by
  construction is the lock masking interrupts itself (OT-97: the net locks to `IrqSafeMutex`).

---

### CP-38 — A hand-written IRQ entry stub that saves only GP registers, in a kernel compiled with SSE

- **Failure mode.** An interrupt lands while kernel code running with `IF=1` holds live values in `xmm`
  registers: a blake3 hash over a NeoFS directory block, a `memcpy`-style struct move of a `Vec`/`Box`
  header, `core::fmt`. The entry stub saves the 15 general-purpose registers and calls an `extern "C"`
  Rust body. That body is ordinary SSE2 code and treats `xmm0-15` as caller-saved scratch. On `iretq`
  the interrupted code resumes with different `xmm` contents and finishes its computation on garbage.
- **Root mechanism.** An interrupt is **not a call boundary**. The SysV split between caller-saved and
  callee-saved registers only holds across a `call`; at an arbitrary instruction *every* register is live.
  The stub owned the obligation to preserve all of them and discharged it only for GP registers. The
  assumption that made this look safe is recorded verbatim in `context.rs`: "kernel emits no float code;
  if it ever does, this routine becomes wrong". The kernel target leaves SSE on (`"features": ""`) and
  the compiler emits roughly 26,000 `xmm` instructions across the kernel, so that premise expired long ago.
- **Why it is invisible.** The corruption is silent and lands *downstream* of the interrupt, in whatever
  the interrupted code was computing, so the symptom is never at the cause:
  - a hash check that fails on data that is actually fine (NeoFS `ContentHashMismatch`, which the VFS then
    reports as `ESTALE`: a successfully written file reads back as "stale" and Arthur's undo is lost);
  - a corrupted pointer that later trips the allocator (`linked_list_allocator` "Hole address not
    aligned!").
  It needs an interrupt to land inside a short `xmm`-live window on the CPU the MSI targets, so it shows up
  at ~1% of runs and passes most gates.
- **Our exposure — REPAIRED.** `swapgs_irq_entry` (`arch/x86_64/idt.rs`) served the TLB-shootdown and
  reschedule IPIs and the AHCI, NVMe, virtio-blk, virtio-accel, xHCI and spurious vectors with no FPU save.
  The timer stub and the syscall entry already saved FPU state. Exception handlers used the `x86-interrupt`
  ABI (since CP-46, only the vectors without an error code do), whose prologue spills `xmm0-15` and emits
  `cld` (checked in the `breakpoint` and `page_fault`
  disassembly) but saves **no** YMM upper halves, x87 or MXCSR. That is sufficient only while no *returning*
  exception handler reaches AVX/x87 code: today `breakpoint` is the one that returns (`page_fault` does not —
  its test hook returns only to fatal handling), and it reaches none. The rules for returning exception
  handlers are written down at "exception entry" in `idt.rs` (see CP-40), and `exception_entry_flags` checks
  that `int3` hands back `xmm0-15` intact.
  The fix saves and restores x87/SSE around the body with the same two variants the timer stub uses:
  `fxsave`/`fxrstor` without `xsave-fpu`, and `fpu_save_area_asm`/`fpu_restore_area_asm` with it.
- **User mode was hit too.** Userland is also SSE2 (`x86_64-neoplutarch-user.json` `"features": ""`), and the
  same stub ran the same bodies when an MSI/IPI landed in ring 3 — the `virtio_blk` body alone reaches dozens
  of `xmm`-using functions — so user `xmm` was clobbered the same way. Unexplained intermittent userland
  symptoms that predate this fix are candidates for this cause and worth re-checking after it lands:
  bimodal hangs (OT-59 #2 family, OT-74 residuals), `threads_futex` attempt-1 failures, and
  `arthur_memory_*`, where a corrupted ChaCha20-Poly1305 run fails its tag check and *silently* degrades to
  ephemeral memory by design.
- **Measured before the fix.** Instrumenting the stub to snapshot `xmm` before and after the body: in one
  ordinary `arthur_multistep_agentic` run, **354 of 468 interrupts that landed in ring 0 (76%) returned
  with the interrupted context's `xmm` registers changed**. The undo loss reproduced at ~0.7% (3 of 450
  runs on `57261d66`), each time as `ContentHashMismatch` during `resolve_path` under `sys_read`. `sys_read`
  runs that lookup with interrupts on; `sys_open` runs the identical lookup at `IF=0`, which is why the
  open succeeded and the next read failed.
- **Discriminator (stays armed).** `irq_xmm_preserve` (`-cpu max`, real XSAVE/XRSTOR) and
  `irq_xmm_preserve_fxsave` (`qemu64`, the `xsave-fpu` stub's FXSAVE fallback): load a distinct pattern into
  `xmm0-15`, `int` a test-only vector routed through the same stub whose body overwrites all sixteen
  registers, and read them back in the same asm block. Each asserts which save path it took. With the save
  removed both fail 64/64; with it both pass. The compile-time non-`xsave-fpu` variant was verified the
  same way locally only (CI builds `--no-default-features` but runs no scenario on it). Not covered: the
  ring-3 source path
  and YMM upper halves.
- **Known discipline.** Every interrupt entry must preserve the complete register state the handler body can
  touch. Linux avoids the problem from the other side: it builds the kernel without SSE (`-mno-sse`) and
  brackets the few SIMD users with `kernel_fpu_begin()`/`kernel_fpu_end()`. Either is sound. Mixing an
  SSE-compiled kernel with a GP-only entry stub is not. Any new hand-written entry stub must route through
  `swapgs_irq_entry` or copy its FPU save.
- **Citation.** Intel SDM Vol. 1 §13 (FXSAVE/XSAVE state components) and Vol. 3A §6.12 (interrupt delivery
  saves only RIP/CS/RFLAGS/RSP/SS); System V AMD64 ABI §3.2.1 (`xmm0-15` caller-saved); Linux
  `arch/x86/include/asm/fpu/api.h` (`kernel_fpu_begin`), read for principle only.

### CP-39 — An interrupt stub that does not clear RFLAGS.DF before running Rust

- **Failure mode.** Ring 3 executes `std` (unprivileged) and is interrupted by an MSI, IPI or timer tick. The
  hand-written stub calls a SysV Rust body with DF still set. The SysV ABI requires DF=0 at every function
  entry, and the kernel's `compiler_builtins` `memcpy`/`memset` are built on `rep movsb`/`movsq`/`stos`. With
  DF=1 they walk **backwards**, writing below the destination: user-triggerable kernel memory corruption.
- **Root mechanism.** Interrupt delivery saves RFLAGS and clears IF/TF, but not DF. `SYSCALL` masks DF through
  `SFMASK`, so the syscall path was safe; `swapgs_irq_entry` and the timer stub never ran `cld`. Exception
  handlers used the `x86-interrupt` ABI, whose LLVM prologue already emits `cld` (seen in the `page_fault`
  disassembly); their `exception_entry` shims (CP-40) now clear DF as well, before that prologue. Since CP-46
  the error-code vectors have no such prologue, and the shim's mask is what clears DF for them.
- **Why it was partly masked.** After CP-38's fix, the XSAVE branch of `fpu_save_area_asm` happens to execute
  `cld`, so XSAVE-active CPUs were covered by accident; the FXSAVE fallback and the non-`xsave-fpu` build were
  not. Relying on that side effect would have been fragile.
- **Our exposure — REPAIRED.** Right after the GP pushes in `swapgs_irq_entry` and in both timer-stub
  variants, a flags mask clears DF (and AC, CP-40): `pushfq; and qword ptr [rsp], ~(AC|DF); popfq`. The saved
  interrupt frame is untouched, so `iretq` restores the interrupted code's own DF.
- **Discriminator (stays armed).** `irq_xmm_preserve` enters the test vector with DF=1 and the body records
  RFLAGS.DF; after `iretq` the test also asserts the caller got DF (and AC) back. `irq_xmm_preserve_fxsave`
  (qemu64) is the leg that can fail: with a mask that clears AC but not DF it fails 64/64 with
  `reason=df_set_in_irq_body`. The timer stub's mask is covered by the timer leg (see CP-40): with a timer mask
  that clears AC but not DF, `irq_xmm_preserve_fxsave` fails 64/64 with
  `reason=entry_state_bad_in_timer_body leg_df_set=64`. Under `-cpu max` the XSAVE helper's `cld` masks it, as
  above.
- **Citation.** System V AMD64 ABI §3.2.1 ("The direction flag DF in the %rFLAGS register must be clear (set to
  'forward' direction) on function entry and return"); Intel SDM Vol. 3A §6.12.1 (flags handling on interrupt
  delivery); Linux `arch/x86/entry/entry_64.S` (`cld` in the entry macros), read for principle only.

### CP-40 — An interrupt stub or exception handler that leaves RFLAGS.AC set, suspending SMAP for the handler

- **Failure mode.** Ring 3 sets RFLAGS.AC with `popfq` (unprivileged; with CR0.AM clear it has no user-mode
  effect) and spins until a hardware interrupt arrives. Interrupt delivery clears IF/TF/RF/NT but **not** AC.
  The stub runs the Rust body with AC=1, and with CR4.SMAP enabled AC=1 is exactly the "explicit user access
  allowed" override: every supervisor read or write of a user page inside the handler succeeds instead of
  faulting. SMAP is silently off for the whole handler, and the timer path can even context-switch into
  another kernel continuation with AC still set (the context shims do not switch RFLAGS).
- **Root mechanism.** The kernel opens user-access windows deliberately (`with_user_access`) and relies on SMAP
  to catch every access outside one. That guarantee depends on AC=0 at every kernel entry. `SYSCALL` masks AC
  through `SFMASK`; neither the hand-written interrupt stubs nor the `x86-interrupt` exception handlers
  established it (exception delivery does not clear AC either).
- **Our exposure — REPAIRED.** Found by the Codex cross-family review (Janus) of PR #15, deterministic
  64/64 on four boots. The same flags mask as CP-39 now clears AC at entry to `swapgs_irq_entry` and both
  timer-stub variants. `clac` is not used because it raises #UD on CPUs without SMAP (`qemu64`); the
  `pushfq`/`and`/`popfq` sequence works everywhere, and `iretq` restores the caller's AC from the untouched
  frame. This is a missing protection, not a demonstrated exploit: no specific unchecked user pointer in an
  interrupt body is known.
- **Discriminator (stays armed).** `irq_xmm_preserve` and `irq_xmm_preserve_fxsave` enter the test vector with
  AC=1 and the body records RFLAGS.AC; with a mask that clears DF but not AC both fail 64/64 with
  `reason=ac_set_in_irq_body`. After `iretq` the test asserts the caller's AC and DF were restored
  (`reason=caller_flags_not_restored`).
- **Timer leg (stays armed).** The same rounds go through the real timer stub via `int 32`. This stub is the one
  ring 3 enters most, and the one whose body can switch threads. The leg runs on the BSP with IF=0 and
  preemption disabled:
  - vector 32 is BSP-only and the stub keeps r10/r11 in one global scratch, so no real tick may interleave;
  - `on_tick` returns without switching.

  `timer_rust_handler` records its entry state on every tick (`TIMER_BODY_FLAGS`: DF, AC, and a non-default
  MXCSR for CP-41). So the check covers every tick since boot as well as the leg
  (`reason=entry_state_bad_in_timer_body`). In this scenario's build the timer body also overwrites xmm0-15,
  as the test-vector body does. That makes the timer stub's x87/SSE save load-bearing: without its restore the
  leg fails 64/64 with `reason=xmm_clobbered_across_irq stub=timer` on all three builds. The leg also checks
  that the caller gets its own AC, DF and MXCSR back. Each round asserts that AC and DF were really set at the
  `int`, so a dropped `popfq` is reported as `ac_df_not_set_at_int`. It would fail anyway, as
  `caller_flags_not_restored`, since `iretq` restores whatever was set.

  | Build / CPU | Fix | Timer mask without AC | Timer mask without DF | Timer stub without `ldmxcsr` | Timer stub without its FPU restore |
  |---|---|---|---|---|---|
  | `irq_xmm_preserve` (`-cpu max`, XSAVE) | PASS | FAIL 64/64 `leg_ac_set=64` | PASS (XSAVE helper's `cld`) | FAIL 64/64 `leg_mxcsr_bad=64` | FAIL 64/64 `xmm_clobbered_across_irq stub=timer` |
  | `irq_xmm_preserve_fxsave` (qemu64, FXSAVE fallback) | PASS | FAIL 64/64 | FAIL 64/64 `leg_df_set=64` | FAIL 64/64 | FAIL 64/64 |
  | non-`xsave-fpu` build, qemu64 (local only) | PASS | FAIL 64/64 | FAIL 64/64 | FAIL 64/64 | FAIL 64/64 |

  In some controls a total exceeded the leg by one, e.g. `total_df_set=65`: a real tick landed inside a
  vector-leg window, where the caller has AC=1, DF=1 and MXCSR `0xFFC0` set. Not covered:
  - the ring-3 source path (both legs enter from ring 0; the mask sits after the ring split and runs on both);
  - the switch-away inheritance itself (the timer leg disables preemption);
  - deterministic hardware-delivered ticks (`int 32` runs the same stub, but the PIC has nothing in service, so
    the body's EOI does nothing).
- **Exception entry — REPAIRED.** The `x86-interrupt` prologue emits `cld` but leaves AC alone, so an exception
  taken with AC=1 ran its handler with SMAP suspended. Audit of the 18 installed vectors (every gate is DPL0,
  so `int3`/`int n` from ring 3 raises #GP instead; vectors 9, 15 and 20-31 are not present, so delivering one
  raises #NP):

  | Vectors | Reachable from ring 3 with AC=1? | After a ring-3 entry the handler… |
  |---|---|---|
  | #DE, #DB (TF, `int1`), #UD, #NP, #SS, #GP, #MF, #XM | yes | dumps and halts the CPU |
  | #BR | no since CP-48 (`bound` is #UD in 64-bit mode, MPX is off so `bndcl`/`bndcu` cannot raise it, and ring 3 no longer reaches compatibility mode) | — |
  | NMI, #MC, #DF | yes (asynchronous / nested delivery) | dumps and halts |
  | #PF | yes | under `browser-runtime`: fault-kills the process (futex purge, sibling kill/zombify, then `scheduler::exit()` switches threads); otherwise dumps and halts |
  | #BP | no (DPL0 ⇒ #GP) | ring 0 only; the one handler that **returns** |
  | #OF | no (`into` is #UD in 64-bit mode and DPL-checked ⇒ #GP in compatibility mode, which ring 3 no longer reaches since CP-48) | — |
  | #NM | no (`enable_sse` clears CR0.EM; CR0.TS is never set, and must be clear for the kernel's own SSE to run) | — |
  | #AC | no (`enable_sse` clears CR0.AM on every CPU; `exception_entry_flags` asserts it on the BSP) | — |
  | #TS | no (only from a malformed TSS during a stack switch; ring 3 cannot cause it) | — |

  So the exposure was concrete on one path. A ring-3 #PF taken with AC=1 ran the whole fault-kill path with SMAP
  off. Because `context_switch` does not save RFLAGS, the thread `scheduler::exit()` switched to also resumed
  with AC=1, and could pass it on through further voluntary switches, until a `with_user_access` `clac` or an
  `iretq` to a clean frame cleared it. That switch-away leak is inferred from `context_switch`, not measured.
  Ring 3 needs only `popfq` and a bad pointer. As with the interrupt stubs, no unchecked user access is known on
  that path; this is a missing protection, not a demonstrated exploit. Every other ring-3-reachable vector halts.
  - **Fix.** Every installed exception gate points at an `exception_entry` shim (`arch/x86_64/idt.rs`):
    `pushfq; and qword ptr [rsp], ~(AC|DF); popfq; jmp <handler>`. It runs before the compiler-generated
    prologue and leaves RSP, the GPRs and the CPU-pushed frame untouched, so the handler sees exactly what the
    CPU delivered and `iretq` (in `breakpoint`) restores the caller's AC and DF. A first-statement mask in each
    handler would also have worked today, since the prologue touches no user memory, but it leaves
    compiler-generated code running at AC=1 and is easy to forget in a new handler. The shim mirrors
    `swapgs_irq_entry` and Linux's `ASM_CLAC` at every IDT entry. `set_exception_entry!` generates each shim as a
    naked function from the handler's own name, so a shim cannot jump to the wrong handler, and keeps the
    compiler checking each handler's signature against its vector. (CP-46 later replaced the `jmp` on the seven
    error-code vectors with a realigning `call` into an `extern "C"` body; the mask is unchanged, and the
    no-error-code vectors keep this shim.) `enable_sse` now also clears CR0.AM on every
    CPU, so a ring-3 AC=1 can never raise #AC (and halt the CPU) whatever the bootloader left.
  - **What a returning exception handler must not touch.** It gets the GPRs and `xmm0-15` saved by the prologue
    and AC=0/DF=0 from the shim. Nothing else of the SIMD/x87 state is saved, MXCSR included, and the exception
    path does no `swapgs`. So `breakpoint`, and anything a future returning handler calls:
    - must not execute any instruction that writes SIMD/x87 state beyond `xmm0-15`: VEX (AVX/AVX2, `vzeroupper`
      included: YMM upper halves), EVEX (AVX-512: upper ZMM lanes, `zmm16-31`, opmask `k0-7`) or x87;
    - must not do SSE floating-point arithmetic (it runs under the interrupted MXCSR and can set its sticky
      status bits);
    - must not touch user memory outside `with_user_access`;
    - if it can be entered from ring 3, must not use GS-relative per-CPU state without a saved-CS-qualified
      `swapgs` first and the matching one before `iretq` (as `swapgs_irq_entry` does), or a common wrapper that
      does both. Gate DPL only stops software `int n`; a hardware exception raised at CPL3 still arrives on the
      user's GS.

    The ring-3 #PF path does one `swapgs`, first thing in `page_fault`, before any per-CPU access (CP-44), and
    the kill path adds CP-41's `load_kernel_fp_control` before the kill, and diverges. (Cross-family review:
    Janus, P2, folded in.)
  - **Discriminators (stay armed).**
    - `exception_entry_flags` (ring 0, `qemu64`): 64 rounds of `int3` entered with AC=1, DF=1 and distinct
      `xmm0-15` patterns; `breakpoint` records the live RFLAGS.AC. It asserts AC=0 in the handler, the caller's
      AC/DF restored, `xmm0-15` preserved, and, reading the live IDT through SIDT, that every present gate in
      vectors 0-31 (at least the 18 installed) starts with the 10-byte mask, is DPL0, and has the expected IST
      slot (NMI and #DF only; installing a shim resets the gate options). Controls (local QEMU 8.2, 4/4 boots
      each, `NEO_SMP=1` and `NEO_SMP=4`): handlers installed without the shim ⇒
      `reason=ac_set_in_exception_handler bp_ac_set=64 rounds=64`; only #GP left unshimmed ⇒
      `reason=exception_gate_without_flags_mask vectors=0x00002000`; #DF's IST slot dropped ⇒
      `reason=exception_gate_wrong_ist vectors=0x00000100`.
    - `threads_guard_fault` ring-3 leg: the guard-overflow thread sets AC=1 before it recurses, and `page_fault`
      counts user faults whose saved frame had AC=1 (must be ≥1, so the probe really armed) and those it ran with
      AC=1 (must be 0). Control without the shim ⇒ `reason=ac_set_in_page_fault_handler pf_live_ac=1
      pf_frame_ac=1` (4/4).
  - **Not covered.** The switch-away leak is not measured directly (the ring-3 leg stops it at the handler).
    The NMI, #MC and #DF shims are verified structurally only. `context_switch` still does not normalize
    RFLAGS, so a `with_user_access` closure that blocked or yielded would hand its AC=1 to the next thread; none
    of the current call sites does (checked by the AAG concurrency lens), but nothing enforces it.
- **Citation.** Intel SDM Vol. 3A §4.6 (SMAP and the RFLAGS.AC override), §6.12.1.3 (flags cleared on
  interrupt/exception delivery); Linux `arch/x86/entry/` (`ASM_CLAC` at every kernel entry), read for
  principle only.

### CP-41 — Kernel SSE code running on the caller's MXCSR

- **Failure mode.** The kernel is compiled with SSE (`x86_64-neoplutarch.json` `"features": ""`) and syscall
  bodies run hardware float arithmetic. The Arthur inference path is the main user: dequantizers, Q8
  quantizers, dot kernels, the sampler, `qwen2_output_logits`. MXCSR holds the six SIMD exception masks, the
  rounding mode, FTZ and DAZ, and ring 3 can write it with `ldmxcsr`. Neither `SYSCALL` nor interrupt delivery
  touches it. The syscall stub saved the user's FPU image and then ran the Rust body on the **user's** MXCSR. Two
  consequences:
  1. **User-triggerable kernel halt.** Ring 3 clears the mask bits and makes any syscall that reaches kernel
     float code. The first NaN, Inf, overflow, denormal or inexact result raises #XM (vector 19) in ring 0, and
     `simd_floating_point` halts the machine. Almost every float op is inexact, so unmasking PM alone is enough.
  2. **User-chosen kernel results.** With the masks left set, ring 3 picks round-toward-zero, FTZ or DAZ. The
     kernel's inference output then depends on an untrusted register, and every byte-parity oracle (the
     llama.cpp goldens) breaks for that caller.
- **Root mechanism.** Saving the user's FPU state is not the same as entering the kernel's FPU *environment*.
  The SysV ABI makes the MXCSR control bits callee-saved with a defined value: all exceptions masked, round to
  nearest. LLVM compiles every float op on that assumption. `fxsave`/`xsave` preserve the user's state; they do
  not establish the kernel's.
  - This is the same kind of bug as CP-39 (DF) and CP-40 (RFLAGS.AC): state that ring 3 controls and that the
    entry stub must set to the kernel's value before Rust runs. Here the state is the float
    environment.
  - `context.rs` recorded the assumption that made it look safe: "MXCSR / x87 control word: callee-saved, but
    identical across both sides of the switch in a single-binary kernel". That holds for kernel-to-kernel
    switches and fails at the ring boundary.
- **Why it is invisible.** Ordinary user code never writes MXCSR, so every gate ran on 0x1F80. Much of the
  kernel's float code is immune: the `ai::ops` reference ops (matmul, softmax, RMSNorm) use `soft_f32` integer
  arithmetic. The default kernel has three float compares and no float arithmetic.
  - `phase9-real-quant` (the Arthur path) adds hardware SSE across the inference path: the Q4_K/GGUF
    dequantizers, the `vec_dot_*` kernels, RoPE, the sampler and `qwen2_output_logits`.
  - It also adds the Q8 activation quantizers (`quantize_block_q8_k` / `_q8_0`, `round_away_from_zero`,
    `nearest_int`), which a rounding-mode change alters by construction.
  - Even integer code can depend on MXCSR: LLVM lowers chacha20's SSE2 variable shifts through `cvttps2dq`.

  QEMU TCG also **never delivers #XM**: its SSE helpers update the MXCSR flags but never raise the exception
  (QEMU 10.2.3 `target/i386/ops_sse.h`). The halt cannot reproduce in the gate at all; only the result changes
  can.
- **Our exposure — REPAIRED.** Every entry that runs kernel Rust on a saved user image now loads MXCSR 0x1F80
  right after the save:
  - `syscall_entry`, both variants. On exit, FXRSTOR or XRSTOR reloads the user's MXCSR from the saved image.
    Standard-form XRSTOR loads MXCSR whenever RFBM[1] or RFBM[2] is set, whatever XSTATE_BV says. The `-cpu max`
    leg covers it.
  - `timer_isr_entry`, both variants. A tick from ring 3 runs `on_tick`, which may switch to a kernel thread.
    Without `xsave-fpu` the switch does not swap FPU state, so that kernel thread would run on the user's MXCSR.
  - The **cold paths** reset x87 as well, through `load_kernel_fp_control()` (`fninit` + `ldmxcsr`):
    - `enable_sse` on the BSP and every AP, so boot and idle threads, and every image cloned from them, start
      from the defaults instead of whatever the bootloader left;
    - a new thread's image (`xsave-fpu`), which `FpuState::new_initial()` (CP-45) builds from constants with
      the default FCW/MXCSR, empty x87 tags and no creator payload, whichever context creates it;
    - the ring-3 page-fault kill path (`browser-runtime`), where without `xsave-fpu` the next thread would
      inherit the faulting thread's state.
  - **x87 on the hot path: deliberately not reset.** The kernel contains no compiler-generated x87 instruction
    (`llvm-objdump` of the `phase9-real-quant` scenario kernel finds only this change's own `fninit` and the
    probe's `fnstcw`). FNINIT measured ~70 TSC ticks per call on a Xeon, against ~1 for LDMXCSR, so it would tax
    every syscall and tick for nothing. Linux x86-64 makes the same call, below. Rule: kernel code that ever uses
    x87 must `fninit` first.
  - `swapgs_irq_entry`, both variants of `irq_fpu_save!()`. Its bodies do no float arithmetic today and do not
    switch threads, so this is defense in depth; it became possible once CP-38 gave the stub an FPU save to
    restore from (loading MXCSR without one would destroy the interrupted context's value). Exception handlers
    other than the page-fault kill path halt, so they run no kernel float code on user state.
- **Measured before the fix** (QEMU 8.2 TCG, `-cpu max` XSAVE path; neo-init leg → kernel probe):
  - Default leg: clean.
  - `0xFFC0` (masked, RZ, FTZ, DAZ): the kernel ran on MXCSR=0xffc0. **10 of 14** result words differed from the
    round-to-nearest oracle, including 3 of the 4 real `qwen2_output_logits` logits.
  - `0x6000` (all unmasked, RZ): the kernel ran on 0x6000. **8 of 14** words differed; TCG raised no #XM.
  - The qemu64 (FXSAVE) leg failed identically.
  - On real silicon the same `ldmxcsr 0x6000` + `1.0f32 / 3.0f32` raises #XM (Linux delivers SIGFPE, exit 136).
    In ring 0 that is the halt above.
- **Discriminator (stays armed) — partial.** It covers the `syscall_entry` reset on both save paths, through
  `syscall_mxcsr_entry` (`-cpu max`, XSAVE/XRSTOR) and `syscall_mxcsr_entry_fxsave` (qemu64, the `xsave-fpu`
  stub's FXSAVE fallback). Each scenario asserts which save path it took. The `swapgs_irq_entry` reset is
  covered by an MXCSR leg in CP-38's `irq_xmm_preserve(_fxsave)`: the caller enters the test vector on MXCSR
  `0xFFC0`, the body must see `0x1F80`, and the caller must get `0xFFC0` back.
  - neo-init runs three legs: default; `0xFFC0`/FCW `0x0F7F`; `0x6000`/FCW `0x0C40`. Each loads the control
    state and issues `SYS_TEST_FLOAT_PROBE` (0xF00, test builds only) in one asm block, so no user float op runs
    on the hostile state.
  - The probe records the MXCSR and FCW it inherited. It then runs nine hardware SSE ops that depend on the
    rounding mode, FTZ or DAZ and between them raise all six SIMD exceptions, plus the real
    `qwen2_output_logits`.
  - A leg passes only if the kernel saw MXCSR 0x1F80, every result matches a hand-derived round-to-nearest
    oracle, and the user's MXCSR and FCW came back bit-exact. The kernel's FCW is logged, not asserted (see the
    x87 note above).
  - A #XM in any `neo-test` build now fails as `unexpected_trap trap=simd_floating_point` instead of halting
    into the QEMU timeout. A real #XM on a bare-metal run would therefore name itself, but no bare-metal run of
    this scenario exists yet.
  - The timer reset is covered by CP-40's timer leg: the timer body records its MXCSR on every tick. Without the
    timer stub's `ldmxcsr`, `irq_xmm_preserve(_fxsave)` fail 64/64 with
    `reason=entry_state_bad_in_timer_body leg_mxcsr_bad=64` on `-cpu max` and qemu64, and on the
    non-`xsave-fpu` build (local only). This proves the body runs on the default MXCSR. It does not exercise the
    consequence that makes the reset matter without `xsave-fpu`: a switch from the timer body to a thread that
    then inherits the MXCSR. The timer leg disables preemption.
  - Not covered: the `enable_sse` and page-fault resets. In the default `xsave-fpu` build,
    `context_switch` swaps per-thread images, so those resets change nothing a scenario can observe. The
    new-thread image is observable and is checked by CP-45's `thread_entry_clean_state`.
    They matter for the non-`xsave-fpu` build, which CI builds but runs no scenario on. Real #XM delivery is not
    covered either,
    because TCG has none. The non-`xsave-fpu` `syscall_entry` variant was checked locally only:
    `syscall_mxcsr_entry_fxsave` with `inference-preemptible` dropped from the default features passes with its
    reset and fails with code 22 without it.
- **Known discipline.** On x86-64, Linux's `kernel_fpu_begin()` saves the user state and loads
  `MXCSR_DEFAULT`. It runs `fninit` only for callers that ask for `KFPU_387` (32-bit kernels, or code that uses
  x87), because FNINIT is slow. The split here is the same. The stub that saves the user image is where kernel FP
  use begins, so that is where the default MXCSR loads. Any new entry stub that saves FPU state and calls Rust
  must load it too.
- **Adjacent, not fixed here.**
  - A **ring-3** #XM (user float code with exceptions unmasked) also reaches `simd_floating_point` and halts.
    It belongs to the general gap that user exceptions other than a `browser-runtime` #PF are not contained: in
    the default build a user `ud2` halts too.
  - Threads from `SYS_THREAD_CREATE`, and processes from `SYS_SPAWN`, now start with the default MXCSR instead of
    the creator's. POSIX `pthread_create` inherits the float environment. Before this change, inheritance
    happened only by accident, together with live kernel `xmm` (the creator-copy leak filed alongside CP-38,
    fixed by CP-45).
- **Citation.**
  - Intel SDM Vol. 1 §10.2.3 (MXCSR), §11.5 (SIMD floating-point exceptions) and §13.8 (XRSTOR).
  - Intel SDM Vol. 3A §2.5 (CR4.OSXMMEXCPT: #XM rather than #UD).
  - System V AMD64 ABI §3.2.1 (MXCSR control bits are callee-saved) and §3.4.1 (initial MXCSR 0x1F80).
  - Linux `arch/x86/include/asm/fpu/api.h` (`kernel_fpu_begin`, `KFPU_387` / `KFPU_MXCSR`), read for principle
    only.

### CP-42 — `stac`/`clac` marked `nomem`: the compiler moves memory accesses across the SMAP window

*(CP-38…CP-41 landed with #15 and #18; this is the next number. It belongs with CP-40, which clears
RFLAGS.AC on interrupt entry.)*

- **Failure mode.** `with_user_access(f)` runs `stac`, then `f`, then `clac`. Both were
  `asm!(…, options(nomem, nostack))`. `nomem` promises the compiler that the asm reads and writes no memory,
  so LLVM may move loads and stores across it. A user access moved out of the window takes a SMAP `#PF`. A
  kernel access moved in runs with SMAP lifted, so a stray user-pointer dereference there goes uncaught.
- **Root mechanism.** Asm options are promises to the optimizer, not a description of the instruction.
  `stac`/`clac` have no memory operand, but their only job is to change which accesses are allowed, so they
  must order memory accesses. Without `nomem`, the compiler assumes the asm may read or write any memory the
  program can reach, which makes it a compiler memory barrier. `read_volatile` does not help: volatile accesses
  are ordered against other memory side effects, and a `nomem` asm has none.
- **How it showed.** `with_user_access` has two paths: SMAP on (`stac; f; clac`) and SMAP off (`f`). LLVM
  tail-merges the stores both paths share into the join block, after `clac`. Measured on `57261d66` by
  disassembling every `stac … clac` window before and after the fix:

  | Build | `stac … clac` windows | User access outside its window before the fix |
  |---|---|---|
  | dev, opt-level 0 (every `neo-test` scenario, `--features product`) | `stac`/`clac` are out-of-line calls | none possible: an opaque call is already a barrier |
  | dev, opt-level 2, default features | 16 | none (codegen identical before and after) |
  | dev, opt-level 2, wide features¹ | 19 | 1: `futex::wait` reads the user futex word (`read_volatile`) after `clac` |
  | release (LTO), wide features¹ | 35 | 7: four empty windows (`sys_futex_wait`, `sys_net_stats`, two in `sys_readdir`) and three that write part of their out-struct after `clac` (`sys_tensor_query`, `sys_model_query`, `sys_inspect`) |

  ¹ `product,browser-runtime,kernel-rng,kernel-sysstat,kernel-observability,net-dial-cap,virtio-accel,framebuffer-console`.
  After the fix, all of those accesses sit between `stac` and `clac` on the SMAP-on path, and every other
  window is unchanged.
- **Our exposure — LATENT → REPAIRED.** No build path compiled an affected kernel. `neo-test` and CI build the
  kernel at opt-level 0, and the one opt-level-2 scenario (`arthur_real_brain_boot_qwen35`) is skipped without
  its model blob; built at opt-level 2, its 18 windows are identical before and after. A `--release` kernel, or
  any future optimized profile, would have faulted. `stac()` and `clac()` in `arch/x86_64/mod.rs` no longer
  pass `nomem`; they keep `nostack`. PR #16 dropped `nomem` from its `clear_rflags_ac()` for the same reason.
- **Discriminator — not armed.** Neither SMAP scenario goes through `with_user_access`: `smap_catch` reads a
  user page with no window and expects the fault, and `smap_policy` checks the CR4 bit. With no optimized
  kernel in the gate either, no gate run can show this fix working or failing. An armed check would
  disassemble an optimized kernel and assert that every `stac … clac` window contains its user access. Until
  one exists, this is a review item.
- **Known discipline.** Any asm that opens or closes a SMAP window (`stac`, `clac`, or a `popf` that writes
  RFLAGS.AC) must not be `nomem`, `readonly` or `pure`. `write_cr4_raw` keeps `nomem` because it sets CR4.SMAP
  only during CPU bring-up and in the SMEP/SMAP scenarios, never while a user access is in flight. A CR4
  write that toggles SMAP around live user accesses would need the same treatment.
- **Citation.** Rust Reference, inline assembly options (`nomem`: the compiler may assume the block reads and
  writes no memory outside it); Linux `arch/x86/include/asm/smap.h`, where `stac()`/`clac()` go through
  `alternative()`, whose asm carries a `"memory"` clobber. Read for principle only.

### CP-43 — The syscall return path never reloads `rcx`/`r11`, so ring 3 gets kernel scratch in them

> This entry closes the "Syscall return leaks `rcx`/`r11`" residual listed under PR #17's thread-creation
> entry (`thread_entry_clean_state`).

- **Failure mode.** After every syscall, ring 3 finds kernel values in `rcx` and `r11`. These are whatever
  the dispatcher and the code it called last left there: zero, kernel text, kernel heap pointers. The heap
  sits at a fixed offset above the kernel image (`heap::try_above_kernel`), so one heap pointer is enough to
  recover the KASLR slide. Unprivileged code gets one on demand, from a plain syscall.
- **Root mechanism.** SYSCALL overwrites `rcx` (the return RIP) and `r11` (RFLAGS). SYSRET reverses this by
  construction, because it *reads* those two registers to return, so they hold user values on the way out.
  This kernel returns with IRETQ instead (CVE-2012-0217: SYSRET with a non-canonical `rcx` faults in ring 0
  on Intel). The stub copied `rcx`/`r11` into the IRETQ frame and restored every other GP register from its
  save area. Nothing gave the two registers a value again, so they were neither saved user state (SYSCALL
  had already destroyed that) nor operands of the return instruction. Rust code is free to use both as
  scratch, and does.
- **Why it is invisible.** The syscall ABI says `rcx`/`r11` are clobbered, so no well-behaved program reads
  them, and every functional test passes. Only a program that looks sees the leak. `r11` hides even from a
  program that looks: LLVM allocates it last among the caller-saved registers, so on most ordinary routes
  measured below it came back still holding the user's RFLAGS. That is codegen and scheduling luck, not a
  guarantee: after a rebase the context-switch route leaked `r11` too, on 1 attempt in 4.
- **Measured before the fix** (`main` @ `57261d66`, QEMU qemu64, the default `xsave-fpu` build on its
  FXSAVE fallback; the non-`xsave-fpu` build at `NEO_SMP=4` showed the same pattern). Every attempt failed
  with `leaked=enosys:rcx,write:rcx,chan_create:rcx,sched_yield:rcx,dirty_scratch:rcx+r11`.
  `SYS_CHAN_CREATE` returned a kernel heap pointer in `rcx`, e.g. `0xffffffff8d609698`. Across boots the
  low 21 bits stayed `0x09698` while the high bits moved in 2 MiB steps with the KASLR slide. In the
  non-`xsave-fpu` build the same leg returned `0xfffffffff4422828`; resolved against that build's symbol
  table (slide taken from the dirty-scratch leg's known function address) it is
  `__kernel_image_end + 0x254828`, inside the heap. The other legs returned `0xfffffffffffc7900`, `0x0`,
  or kernel text in `rcx`. On `57261d66` every ordinary leg returned the user's RFLAGS untouched in `r11`.
  On `d9e179ab` (after #15, #18 and #20) the no-fix control again fails 4/4, and on 1 of the 4 the
  `SYS_SCHED_YIELD` leg leaked `r11` as well. Only the deliberate leg (below) exposes the `r11` path every
  time.
- **Our exposure — REPAIRED.** Both `syscall_entry` variants now end with `mov rcx, [rsp]` /
  `mov r11, [rsp + 16]` after the GP pops, right before `swapgs; iretq`. These are the user RIP and RFLAGS
  from the IRETQ frame. They are exactly what SYSRET would leave, so the user-visible ABI does not change
  if a SYSRET fast path is ever added, and it matches what Linux userland sees. The return is still IRETQ:
  `rcx` is only loaded, never used as a return target, so the CVE-2012-0217 non-canonical-RIP trap does not
  come back. `rax` (the return value) is untouched. With this, every GP register ring 3 sees after a syscall
  is its own saved value, the return value, or a SYSRET-defined value.
- **Discriminator (stays armed).** `syscall_return_clean_regs`. A hand-written ring-3 probe makes five
  syscalls, each leaving the kernel by a different route:
  - an unassigned number (straight out of the dispatcher);
  - `SYS_WRITE` (user copy + serial driver);
  - `SYS_CHAN_CREATE` (heap allocation + capability install);
  - `SYS_SCHED_YIELD` (a context switch);
  - the test-only `SYS_TEST_DIRTY_SCRATCH` (`0xF01`, built only for this scenario), whose return leaves
    a kernel address in both registers. The registers are dirtied by `syscall_return_test_hook`, a few
    instructions spliced into `syscall_entry` itself after the GP pops, right before the reload, so no
    compiler-generated code runs in between. The hook exists only in this scenario's build. In the
    default build the macro expands to nothing, and `syscall_entry` disassembles identically to a build
    without it. (Originally the syscall body dirtied the registers in inline asm and returned through
    ordinary Rust, so an inlining or epilogue change could have overwritten `r11` and let an `rcx`-only
    half-fix pass. Codex review F1, made structural in follow-up E.)

  Each syscall is made with RFLAGS = `0xAD7` (all six status flags). The probe stores `rcx`/`r11`/`rax`
  as the first instructions after the `syscall`, plus its own return RIP (`lea rax, [rip + label]`). The
  kernel requires `rcx` = that RIP and `r11` = that RFLAGS on every leg. Non-vacuity checks: each leg's
  `rax` must be its syscall's real result, the RFLAGS must carry the distinctive flags, the return RIP
  must lie in the probe's code page, and the hook must have fired on exactly the dirty-scratch leg's
  return (it records that return's user RIP).
  - Before the fix every attempt fails (above). A half-fix that reloads only `rcx` fails every attempt
    with `leaked=dirty_scratch:r11`; re-run with the stub hook in place (`NEO_SMP=4`), it fails 4/4 the
    same way, and the no-fix control fails 4/4 on all five legs.
  - After the fix it passes on qemu64 at `NEO_SMP=1` and `=4`, and under `-cpu max` (real XSAVE).
  - The non-`xsave-fpu` build was run locally only: it fails before the fix and passes after, at
    `NEO_SMP=4`. With the stub hook it passes, and its `rcx`-only half-fix fails with
    `leaked=dirty_scratch:r11`.
- **Two lessons for writing this kind of discriminator.**
  1. **A register that is correct by accident makes its half of the check vacuous.** Only the forced
     dirty-scratch leg proves the `r11` reload. Without it, the probe passes against an `rcx`-only fix.
     And the forcing itself must not depend on codegen: dirty the registers in asm placed after the last
     compiler-generated instruction, and check that it actually fired.
  2. **Make the expected value distinctive.** The first probe used the trampoline's RFLAGS (`0x202`). That
     is also a typical *kernel* RFLAGS, so "r11 restored" and "r11 left over from the kernel" could not be
     told apart. Setting all six status flags (ZF and SF together never come out of an arithmetic result)
     removed the ambiguity, and is what showed that `r11` had never been touched.
- **Not covered / residuals.**
  - CI assembles the non-`xsave-fpu` `syscall_entry` (the no-default-features build, #19) but never runs
    this scenario in it, and never assembles the non-`xsave-fpu` copy of the test hook; that variant is
    checked locally only (above).
  - Only the syscall return. The IRQ and timer stubs save and restore all 15 GP registers. The ring-3
    thread-entry trampolines are a separate seam with the same class of leak (CP-45).
  - Spectre-class hygiene (zeroing user-controlled registers on *entry* so kernel code cannot consume
    them speculatively, as Linux's `PUSH_AND_CLEAR_REGS` does) is a different property and is not done here.
- **Known discipline.** On a privilege-lowering return, every register the less-privileged side can read
  gets a defined value: its own saved value, a return value, or an architectural constant. Linux's
  `entry_SYSCALL_64` saves the SYSCALL-time `rcx`/`r11` in `pt_regs` and restores them from there on both
  exits. It takes the SYSRET exit only when `regs->cx == regs->ip` and `regs->r11 == regs->flags`, so user
  code sees `rcx` = RIP and `r11` = RFLAGS whichever way it returns. Read for principle only.
- **Citation.** Intel SDM Vol. 2B `SYSCALL` / `SYSRET` (RCX/R11 semantics); AMD APM Vol. 3 `SYSCALL` /
  `SYSRET`; CVE-2012-0217 (SYSRET with non-canonical RCX on Intel); Linux `arch/x86/entry/entry_64.S`
  (`entry_SYSCALL_64`) and `do_syscall_64` (the SYSRET-eligibility checks), read for principle only.

### CP-44 — An entry path that reads per-CPU state through GS before swapping to the kernel GS

- **Failure mode.** A ring-3 thread faults; a guard-page stack overflow is the archetype. `page_fault` (an
  `x86-interrupt` handler behind CP-40's `exception_entry` shim) made `record_page_fault` its first real call.
  With `kernel-observability` on, that runs `record_trace` → `per_cpu::current_cpu_id()`, which reads `gs:[24]`.
  Exception delivery does not swap GS, so GS_BASE still held the user value, which is 0 (`KERNEL_GS_BASE`
  starts at 0 and userland never sets it). The read hits linear `0x18`, which is unmapped, and raises a nested
  `#PF` from ring 0. The nested handler takes the same read on the same user GS and faults again. Each level
  pushes another frame and `x86-interrupt` spill area until a push lands in the kernel-stack guard and the
  CPU raises `#DF`. The process is never fault-killed; the kernel dies instead.
- **Root mechanism.** `GS_BASE = &PER_CPU` is a ring-0 invariant (ADR-005 Part B) that only an entry path can
  establish. `syscall_entry` always swaps (it is only ever entered from ring 3), and `swapgs_irq_entry` and the
  timer stub swap when the saved CS.RPL is 3. The exception shims (CP-40) clear AC/DF but do not swap, so a
  handler reachable from ring 3 must swap before *any* per-CPU access, including one hidden behind a
  feature-gated helper. `page_fault` did swap, but only inside the `browser-runtime` fault-kill block, after
  the observability call.
- **Second occurrence of the class.** The Phase-5 SMP=4 bare-metal page fault (`f5aacff`, 2026-06-04;
  `Plans/CLOSE-REPORT-2026-06-04-phase-5.md`) was the same read with the same `cr2=0x18`, reached from MSI/IPI
  vectors that were plain `x86-interrupt` handlers. That fix created `swapgs_irq_entry` but did not change the
  exception handlers, and the class was never entered in this bank.
- **Why CI could not see it.** It needs `kernel-observability` and a ring-3 `#PF` in the same build.
  `kernel-observability` is on only in `kernel_observability` and the three `arthur_*` scenarios
  (`agentic_actions`, `reversibility`, `multistep_agentic`), none of which takes a ring-3 fault. The ring-3
  `#PF` scenarios (`threads_guard_fault`, the atrium RO probes) build without it. No product build enables
  `kernel-observability`, so no shipped build was exposed. The rest of the ring-3 fault path (`serial_println`,
  `log::flush`, the scenario phase hooks) reads no per-CPU state, with one exception. When a bare-metal block
  log sink is installed, a flush can allocate a DMA buffer. That goes through `with_frame_alloc`, whose
  debug-build reentry check reads `gs:[24]` (`mm/mod.rs`). Bare-metal test runs could therefore hit the same
  recursion from a print alone.
- **Measured before the fix.** `-d int` on the pre-CP-40 tree, where `page_fault` had the same order.
  Interrupt 33 is the ring-3 guard fault (`cpl=3 e=0006 CR2=0x400000073fd8`). It is followed by 226 nested
  `#PF`s, all at `cpl=0 e=0000` at one IP, all with `CR2=0x18` and `GS base = 0`, each `0x380` bytes further
  down the stack. A final write fault hits the stack guard (`e=0002`), and then comes `#DF`. The serial log
  shows `DOUBLE_FAULT (#DF)` where `threads_guard_fault` prints `ring-3 #PF pid=… — fault-killing process
  (I-14b)`, and the scenario times out.
- **Our exposure — REPAIRED in `page_fault`.** `page_fault` now swaps first thing, in every build, ahead of
  CP-40's test-only AC counters, which read only the frame and RFLAGS. The swap is keyed on the saved CS.RPL,
  the same `test [rsp+8], 3` the IRQ stubs use, and the `browser-runtime` kill block keeps only CP-41's
  `load_kernel_fp_control`. Nothing swaps back, because `page_fault` never returns:
  - the neo-test `handle_page_fault` hook either diverges (`pass`/`fail`/`fail_with_hex`/`debug_exit` and the
    atrium and q7c asserts are all `-> !`) or returns only when no fault is expected;
  - `fault_kill_current_user` is `-> !` and switches away for good;
  - the fatal path ends in `unexpected_trap` or `halt_forever`.

  The SMEP/SMAP probes fault from ring 0 and are unaffected. The atrium RO probes fault from ring 3 and now run
  their asserts on the kernel GS. Their asserts read no per-CPU state, so they passed either way. A future
  recoverable path, such as demand paging or a user-copy fixup, must `swapgs` back before it `iretq`s to
  ring 3.
- **Why CS.RPL and not the error code's U/S bit.** The swap has to follow which GS is live, and the interrupted
  CPL decides that. U/S=1 implies CPL 3, but not the other way round. When a CPL-3 instruction makes an
  *implicit supervisor* access (a GDT/LDT descriptor load, an IDT read during delivery) and that access
  faults, U/S is 0 while the user GS is live, and a U/S-keyed swap would miss it. The fault-kill gate stays on
  `USER_MODE`, unchanged. `USER_MODE` ⇒ CPL 3 ⇒ the swap has already run.
- **Residual windows.** CPL 0 runs on the user GS in these places:
  - the `swapgs` that opens `syscall_entry`, and the instructions before the swap on every gate entered from
    ring 3: the IRQ stub's `test`/`jz`, the timer stub's scratch stores, the exception shims' flag mask, and
    `page_fault`'s own prologue;
  - between an exit `swapgs` and its `iretq` (`syscall_entry`, the IRQ and timer stubs, the
    `enter_userspace*` trampolines);
  - the whole body of every fatal exception handler entered from ring 3 (`#DE`, `#UD`, `#GP`, …), which never
    swaps (see *Other handlers*).

  None of the instructions in the first two groups can raise `#PF`: they touch only registers, mapped kernel
  stacks and `.bss`, and a bad `iretq` frame raises `#GP`. The fatal handlers can, through the bare-metal
  block-sink path above, and that `#PF` arrives with RPL 0 on the user GS. So the RPL test is exact for
  `page_fault` everywhere except in bare-metal test builds after another fatal exception. Other events can also
  land in the first two groups:
  - NMI, `#MC` and `#DB`. Linux gives these a `paranoid_entry` that reads `MSR_GS_BASE` instead of trusting
    CS. Ours print and halt without per-CPU reads today.
  - **A maskable interrupt, at `syscall_entry`'s exit (was EXPOSED → repaired, CP-50).** `SFMASK` clears IF
    on entry, but `spawn_user` returns with IF enabled (`shootdown_quiesce.resume()`), and the exit sequence
    had no `cli` before its `swapgs; iretq`. A timer tick or IPI landing between those two instructions
    arrived with saved CS.RPL=0 and the user GS live; the stub did not swap, and its body's per-CPU read
    faulted at `0x18`, the same recursion as above. The exit now masks first, and CP-50's audit found no
    other exit `swapgs` reachable with IF=1. CP-50 has the fix, the audit and its discriminator.
- **Other handlers (sibling, not fixed here).** The remaining exception handlers that ring 3 can reach (`#DE`,
  `#UD`, `#GP`, …) print the frame and halt, on the user GS. They read per-CPU state only in bare-metal test
  builds, where `serial_println` → block-sink flush → `DmaBuffer::alloc_zeroed` → `with_frame_alloc` reaches its
  debug-build `current_cpu_id()`. The AHCI/NVMe `SwitchMutex` owner tracking (`kernel-diagnostic`) reads it
  too. The result is a `#PF` at `0x18` from RPL 0, so `page_fault` does not swap. Without
  `kernel-observability` that misreports the original trap as `PAGE_FAULT`; with it, it recurses into `#DF`.
  Either way the kernel was already halting, and only the diagnostic is lost. The fix is the same RPL-keyed
  swap at the top of each ring-3-reachable handler, or in the CP-40 shim, which knows each vector's frame
  layout. Every handler ring 3 can reach diverges, so the shim would not need a matching swap back.
- **Discriminator (stays armed).** `threads_guard_fault_observability` is the `threads_guard_fault` battery
  built with `kernel-observability`. The with-cap worker overflows its guarded stack in ring 3 (with AC=1, so
  CP-40's ring-3 AC leg runs here too), the kernel fault-kills it with exit 139, and neo-init proves the
  scheduler is still live. A test hook in the `#PF` handler, right after `record_page_fault`, checks two
  things for a ring-3 fault:
  - the handler is on the kernel GS: the `IA32_GS_BASE` MSR points at a `PER_CPU` slot (else
    `reason=ring3_pf_body_not_on_kernel_gs`). The slot's index is taken from the MSR, not from a `gs:` read;
  - the trace ring's newest page-fault event for this `cr2` carries that index as its cpu id
    (`reason=ring3_pf_traced_wrong_cpu` / `ring3_pf_not_traced`). The id `record_trace` stored came from
    `gs:[24]`, so this compares two independent sources.

  The scenario then requires at least one such fault (`reason=no_ring3_pf_reached_the_handler`), so it cannot
  pass vacuously. The hook does only scalar work, because it was written on CP-46's misaligned stack.
  Results, with only the swap differing between each pair of runs:
  - on `main` after CP-40's shims, `NEO_SMP=4`: swap reverted → `DOUBLE_FAULT (#DF)` where the fault-kill line
    should be, then the 180 s timeout; with the swap → PASS (7.8 s);
  - on the pre-CP-40 tree: the same `#DF` at `NEO_SMP=1`; PASS at `NEO_SMP=1` (6.9 s) and `NEO_SMP=4` (7.3 s).

  The real signal is the first: without the swap the handler never reaches the hook. Not covered:
  - a swap keyed on the error code's U/S bit would also pass, because no scenario takes a CPL-3 fault with
    U/S=0 (none can be built here: GDT, IDT and TSS are always mapped and there is no LDT);
  - the fail-reason prints themselves are untested (they ran on CP-46's misaligned stack until that fix).

  A false FAIL would need the 16-slot trace ring to wrap between `record_page_fault` and the hook, a
  few-hundred-instruction window with IF=0 while the other CPUs are idle. That has not been observed.
- **Found while building this discriminator: the error-code handlers ran their callees on a stack misaligned
  by 8.** The hook's first version raised `#GP` on a `movaps`. That is now CP-46 (fixed); the hook's
  scalar-only shape and the untested fail prints above predate the fix.
- **Known discipline.** Linux's `error_entry`, the path `#PF` takes, tests `CS(%rsp)` for RPL 3 and `swapgs`
  before calling any C. Do the same here: swap at the top, keyed on where the CPU came from, before any Rust
  that could reach per-CPU data. Any new ring-3-reachable entry, stub or `x86-interrupt` handler, needs it.
- **Citation.** Intel SDM Vol. 2B `SWAPGS`; Vol. 3A §4.6 (implicit supervisor-mode accesses from CPL 3: GDT/LDT,
  IDT, TSS), §4.7 (the page-fault error code's U/S bit reports the access mode, not the CPL) and §6.14.2
  (64-bit mode aligns RSP to 16 before pushing the interrupt frame); System V AMD64 ABI §3.2.2; Linux
  `arch/x86/entry/entry_64.S` (`error_entry`, `paranoid_entry`), read for principle only.

### CP-46 — Exception handlers for error-code vectors call Rust on a stack misaligned by 8

- **Failure mode.** A ring-3 thread faults while a sibling in the same process is parked in a futex wait
  that has a deadline. `page_fault` fault-kills the process: `fault_kill_current_user` → `futex::purge_pid`,
  which removes the timed waiter and calls `recompute_earliest`. That function stores a 16-byte local with
  `movaps [rsp+0x90], xmm2`. The stack is 8 bytes off 16, so the store raises `#GP` from ring 0 inside the
  `#PF` handler. The `#GP` handler has the same misalignment: its `dump()` stops after `InterruptStackFrame`
  and the CPU halts. The dying process never finishes exiting, and whatever waits on it waits forever.
- **Root mechanism.** On vectors 8, 10–14 and 17 the CPU pushes an error code. In 64-bit mode it aligns RSP to
  16 and then pushes six qwords (SS, RSP, RFLAGS, CS, RIP, error code), so the handler is entered with RSP
  16-byte aligned. A SysV function expects RSP ≡ 8 (mod 16) at entry, which is what the five-qword frame of a
  vector without an error code produces. LLVM's `x86-interrupt` prologue for the error-code form does not
  correct for the extra qword. In `page_fault` it pushes ten qwords and subtracts `0x278`, so every call the
  body makes happens at RSP ≡ 8 (mod 16) instead of 0, and every callee runs 8 bytes off. A callee that keeps
  a 16-byte stack slot with `movaps`/`movdqa` faults; one that uses `movups` or scalar moves does not, which is
  why most of the kernel ran correctly on this stack. This is the LLVM bug class of rust-lang/rust#40180 and
  D30049, observed on `nightly-2026-04-16` (the kernel's dev profile is `opt-level = 0`, so even trivial
  helpers are real calls with their own frames).
- **Why CI could not see it.** The fault needs a `movaps` callee on the path an error-code handler actually
  takes. The fatal handlers print and halt, so there the only cost was a cut-off diagnostic. The one
  non-fatal path is the ring-3 `#PF` kill (`browser-runtime`). The `movaps` callee found on it,
  `recompute_earliest`, runs only when the dying process had a *timed* futex waiter, and
  `threads_guard_fault`, the scenario built to exercise that kill with a parked sibling, parks it with
  `FUTEX_DEADLINE_INFINITE`. Found under gdb while building CP-44's discriminator, whose first version made
  the same `#GP` with a struct copy. Other `movaps` callees on the kill path were not searched for
  exhaustively; after the fix it no longer matters which ones exist.
- **Measured before the fix.** `main` @ `82d0f591` plus the discriminator below, `NEO_SMP=4`. The serial log
  shows the fault-kill line, then `GENERAL_PROTECTION_FAULT (#GP) err=0x0`, then
  `#GP rip=0xffffffffa2c92cfd rsp=0xffffffffa36634a8` (RSP ≡ 8 mod 16), and the dump ends after
  `InterruptStackFrame`. The scenario times out at 180 s. The RIP symbolizes to
  `kernel::sched::futex::recompute_earliest`, at `movaps XMMWORD PTR [rsp+0x90], xmm2`. After the fix: PASS
  (3.9 s), with `cp46: … x1`.
- **Our exposure — REPAIRED.** Every error-code vector (`#DF`, `#TS`, `#NP`, `#SS`, `#GP`, `#PF`, `#AC`) now
  enters through `set_error_code_entry!` instead of an `x86-interrupt` handler. The shim runs CP-40's flags
  mask, then `mov rdi, rsp` (the error code and CPU frame, as `&ErrorCodeFrame`), `and rsp, -16`, and calls
  an `extern "C" fn(&ErrorCodeFrame) -> !` body; a `ud2` follows the call. The `call` pushes a return address,
  so the body is entered at RSP ≡ 8 (mod 16), as SysV requires, and the compiler's own alignment holds from
  there.
  - All seven bodies diverge. The fatal ones halt. `page_fault` ends in the neo-test hook's `-> !` asserts,
    in `fault_kill_current_user` (`-> !`), or in the fatal path. Nothing returns through the realigned
    stack, so the shim keeps no registers and does not record the original RSP.
  - The bodies lose the `x86-interrupt` prologue: no GPR or xmm0-15 save, and no `cld`. None of them returns
    to the interrupted context, and the shim's mask already clears DF.
  - CP-44's swap runs before anything else in `page_fault`'s body except taking `&f.frame` for its test,
    still keyed on the saved CS.RPL (`f.frame.code_segment`). `#DF` keeps its IST slot.
  - Guards, all compile-time:
    - `set_error_code_entry!` takes only IDT entry types whose vectors push an error code (the
      `ErrorCodeVector` trait), so it cannot be put on a vector where `rdi` would point at RIP. It also
      type-checks the body against `extern "C" fn(&ErrorCodeFrame) -> !`.
    - `set_exception_entry!` takes only entry types whose vectors push none (`NoErrorCodeVector`). Without
      that, a future `#CP`, `#VC` or `#SX` gate written the old way would compile and bring CP-46 back.
    - A `const` assertion pins `ErrorCodeFrame` at 48 bytes with the CPU frame at offset 8.
  - **A recoverable fault cannot use this entry.** The shim keeps neither the GPRs nor the original RSP, and
    the body is `-> !`, so nothing can resume the interrupted context. Demand paging, a user-copy fixup or an
    `rdmsr_safe`-style probe needs a returning stub that saves the registers, realigns, calls, restores and
    `iretq`s (and swaps GS back for ring 3), like `swapgs_irq_entry` or Linux's `error_entry`.
  - Vectors without an error code keep `x86-interrupt`. Their five-qword frame gives the entry alignment SysV
    expects, and the prologue keeps the calls aligned. `#BP` was checked under gdb, and all eleven prologues
    in the disassembly are nine pushes then `sub rsp, 0x100` (`0x110` for `#MC`), which puts every call at
    RSP ≡ 0. The hand-written IRQ and timer stubs and `syscall_entry` already align the stack themselves
    before calling Rust.
- **Discriminator (stays armed).** Two legs.
  1. **Behavioural — `threads_guard_fault_timed`.** The `threads_guard_fault` battery, but the worker's
     futex-parked sibling waits with a deadline an hour out (`deadline_after_ns`) instead of
     `FUTEX_DEADLINE_INFINITE`. The guard fault's kill must purge a timed waiter. The kernel counts `purge_pid`
     calls that removed one and ran `recompute_earliest`; with a count of 0 the scenario fails with
     `reason=no_timed_futex_waiter_purged_by_fault_kill`, so a failed clock read that left the wait infinite
     cannot pass it. The worker triggers the fault only once the kernel's queue length for the park word is
     ≥ 1 (`sys_futex_queue_len`), not merely once the sibling has set its "reached" flag: a sibling preempted
     between the flag and the syscall would be Ready, not a timed waiter, and fail the scenario falsely
     (CP-3x). Before the fix: the `#GP` above and the 180 s timeout. After: PASS.
  2. **Structural — `exception_entry_flags`.** For each error-code vector, the scenario reads the 8 bytes
     that follow CP-40's mask in the gate's shim and requires `mov rdi, rsp; and rsp, -16; call`
     (`reason=error_code_gate_not_realigned vectors=…`). That covers the fatal vectors, which no scenario
     raises. The set also includes `#CP`, `#VC` and `#SX`, which are not installed today; not-present gates
     are skipped. With the pre-fix `idt.rs`: FAIL, `vectors=0x00027d00` (exactly vectors 8, 10–14, 17). With
     the fix: PASS (`error_code_realigned=ok`).

  Results on the final tree (local QEMU 8.2, one attempt each). With the fix, at `NEO_SMP=4`: both legs,
  `threads_guard_fault`, `threads_guard_fault_observability`, the SMAP/SMEP catch and policy scenarios,
  `irq_xmm_preserve` (both variants), `kernel_observability`, `atrium_slice1`/`3` and
  `q7c_concurrent_munmap_smp` pass; at `NEO_SMP=1`, both legs and the two other `threads_guard_fault`
  scenarios pass. With the pre-fix `idt.rs`, `threads_guard_fault_timed` prints the fault-kill line, then
  `GENERAL_PROTECTION_FAULT (#GP)`, and times out at 180 s at both `NEO_SMP=1` and `NEO_SMP=4`.
- **Not covered.**
  - The behavioural leg exercises one `movaps` callee on one path, so its power to catch a regression rests on
    how `recompute_earliest` happens to compile; the structural leg does not depend on code generation. Only
    `page_fault` runs live in any scenario; the other six bodies are checked structurally only.
  - The structural leg does not decode the `call` target, so a shim installed with the wrong body (say
    `page_fault` on `#GP`) would pass it. The old per-vector handler types caught some of those swaps (`#PF`,
    `#DF`); the uniform `extern "C"` body type catches none. The mapping in `idt.rs` was checked by hand and
    in the disassembly.
  - The witness counter does not check its caller. `terminate()` (`sys_kill`) also calls `purge_pid`; nothing
    in this scenario sends one.
  - The no-error-code handlers' alignment is checked by reading the disassembly, not by a gate. If a
    toolchain change broke it, `breakpoint` (the only returning handler) would show it first.
  - The fatal dumps are not gated. A throwaway build that forces a ring-0 `#GP` (a non-canonical read) prints
    the full `InterruptStackFrame { … }` with the fix and stops after `InterruptStackFrame` without it; no
    scenario raises a fatal error-code exception on purpose.
  - CP-44's *Other handlers* residual is unchanged: the fatal error-code handlers still run on the user GS
    when entered from ring 3.
  - Pre-existing, recorded in CP-40's table, not changed here: a ring-3 `#GP`, `#SS`, `#UD`, `#DE`, … still
    dumps and halts the CPU instead of fault-killing the process. I-14b's kill covers `#PF` only. The
    `extern "C"` bodies make extending it to the error-code vectors a matter of calling the same kill.
- **Known discipline.** Linux enters every exception through asm (`idtentry`, `error_entry`) that builds
  `pt_regs` and calls an ordinary C function on a stack it has set up itself, so the C side's ABI
  assumptions hold by construction. A compiler-specific interrupt calling convention is fine only where its
  stack layout has been checked; where it has not, an asm entry that realigns and calls a normal function is
  the known-good shape.
- **Citation.** Intel SDM Vol. 3A §6.14.2 (64-bit mode aligns RSP to 16 before pushing the interrupt frame);
  System V AMD64 ABI §3.2.2 (the stack is 16-byte aligned at a `call`, RSP+8 aligned at entry);
  rust-lang/rust#40180 and LLVM D30049 (the `x86-interrupt` alignment bug class); Linux
  `arch/x86/entry/entry_64.S` (`idtentry`, `error_entry`), read for principle only.

### CP-50 — A syscall exit that reaches its `swapgs` with IF=1

*(This entry closes CP-44's "maskable interrupt at `syscall_entry`'s exit" residual. CP-46 landed with #26
and CP-47…CP-49 are claimed by open PRs #27 and #28, so this is the next free number. It sits with CP-44,
the class it belongs to.)*

- **Failure mode.** A syscall that returned with interrupts enabled ran
  `… mov r11, [rsp+16]; swapgs; iretq`. A timer tick or IPI arriving after the `swapgs` and before the
  `iretq` was taken at CPL 0 with the user GS base live. Its saved CS.RPL is 0, so neither
  `swapgs_irq_entry` nor the timer stub swaps, and the body's first per-CPU read (`gs:[24]`,
  `current_cpu_id`) hits linear `0x18`. From there it is CP-44's recursion: `page_fault` sees RPL 0, does not
  swap, and faults again until `#DF` (with `kernel-observability`) or a halt. The kernel dies on an ordinary
  syscall return.
- **Root mechanism.** Two assumptions, each true almost everywhere, met at one instruction:
  1. `SFMASK` clears IF on entry, so the exit was written as if IF were still 0. `spawn_user` breaks that: it
     returns with IF=1 so the CPU can acknowledge remote shootdowns as soon as
     `LocalShootdownQuiesce::resume()` republishes it, and `SYS_SPAWN` hands that state straight to the exit.
  2. The IRQ stubs decide whether to swap from the saved CS.RPL. That is exact everywhere except between an
     exit `swapgs` and the privilege change that follows it.
- **Audit — which exit `swapgs` can be reached with IF=1?**

  | Exit | IF at the exit `swapgs` | Why |
  |---|---|---|
  | `syscall_entry` (both variants) | **1 after `SYS_SPAWN`** (before this fix) | `spawn_user` → `resume()` → `enable_interrupts()`, and nothing masks again before the exit |
  | timer stub, ring-3 path | 0 | Interrupt gate. If `on_tick` switches away, the thread is switched back in by a `context_switch` that runs with IF=0 (every caller is inside `without_interrupts` or, in the idle loop, after `disable_interrupts`; `rotate` asserts it), and then the body's own `without_interrupts` restores the IF=0 it saved on entry |
  | `swapgs_irq_entry`, ring-3 path | 0 | Interrupt gate. No body context-switches (the reschedule IPI is EOI-only), and every IF-setter a body can reach restores the state it found |
  | `enter_userspace_trampoline`, `enter_userspace_arg_trampoline` | 0 | Reached only through `context_switch`'s `ret` into a new thread, so IF is the switching thread's, which is 0 by `context_switch`'s contract |

  The IF-setters, from a grep of the kernel: `without_interrupts`, `with_interrupts_enabled`, `IrqSafeMutex`
  and `acquire_shootdown_lock` all restore the state they found. `LocalShootdownQuiesce::resume()` is the
  only unconditional enable on a syscall path; the others (`main.rs` boot, `run_ap_bootstrap`,
  `thread_start_trampoline`'s `sti`, the idle loop's `sti; hlt`) are not on a syscall or interrupt return.
  So `SYS_SPAWN` is the only production route found to the exposed exit. Test-only syscalls that enable
  interrupts are others, in test builds.
- **Why CI could not see it.** QEMU TCG checks for interrupts only between translation blocks. In its
  translator `swapgs` does not end a block and `iretq` does, so `… mov r11, [rsp+16]; swapgs; iretq` runs as
  one block, and no gate run could ever take an interrupt between the two.
- **Our exposure — REPAIRED.** Both `syscall_entry` variants now run `cli` immediately before the exit
  `swapgs`. `iretq` restores the user's IF from the frame (a ring-3 RFLAGS always has IF=1, since CPL 3 cannot
  clear it at IOPL 0), so ring 3 sees no change. The `cli` comes after the `rcx`/`r11` reloads (CP-43); an
  interrupt before it is taken on the kernel GS and returns through the stub's full GP save and restore, so
  the reloaded values survive. Two global labels, `syscall_exit_swapgs` and `syscall_exit_iretq`, mark the
  window for the discriminator and emit no code.
- **Discriminator (stays armed).** `syscall_exit_irq_window` is the first gate scenario run under `-icount`
  (`-icount shift=0,sleep=off`, from `scenario_qemu_icount` in xtask). In icount mode QEMU executes an exact
  instruction budget up to each timer deadline and takes a pending interrupt at that boundary, even inside a
  block. At `shift=0` one instruction is 1 ns of virtual time, which is one undivided LAPIC timer count.
  - A ring-3 probe calls the test-only `SYS_TEST_IRQ_WINDOW` (`0xF02`) with counts 1..=4096, twice. Each call
    starts a one-shot LAPIC timer that many bus cycles out, on a test vector (`0xE1`) routed through the real
    `swapgs_irq_entry`, then returns with IF=1, as `SYS_SPAWN` does.
  - The vector's body receives the interrupted frame from a naked shim (`rbp` + 136, the stub's anchor
    layout; a CS.RPL that disagrees with the stub's `from_ring3` flag fails the run). It records the frame's
    RIP and CS, and whether it ran on the kernel GS, using CP-44's test (the `IA32_GS_BASE` MSR, no `gs:`
    read). It reads no per-CPU state, so a run that hits the defect survives to report it.
  - **Fails** if any CPL-0 interrupt lands at `syscall_exit_swapgs` or `syscall_exit_iretq`, or runs on the
    user GS (`reason=irq_inside_syscall_exit_window`), or if the byte before `syscall_exit_swapgs` is not
    `0xFA` (`reason=exit_cli_missing`; checked after the window, so a no-`cli` build reports the landing).
  - **Non-vacuity:** every count must produce an interrupt. At least one must land at CPL 0 exactly on the
    `cli`, and CPL-0 landings must cover at least three instruction boundaries in the 16 bytes before the
    window, which is exact delivery inside the block that ends `swapgs; iretq`
    (`reason=sweep_missed_the_exit_window`). The scenario also reports how often the count after a `cli`
    landing is delivered at the probe's return address, but does not require it: a late delivery produces
    the same landing, so it is not evidence of the window.

  Measured on `82d0f591` plus this change, default features (`xsave-fpu`, FXSAVE fallback on qemu64), on
  QEMU 10.2.3 built from the tag CI's image uses, and on QEMU 8.2.2. Each pair of runs differs only in the
  `cli`:

  | QEMU | Build | `NEO_SMP` | Result | Landings over 2 × 4096 counts |
  |---|---|---|---|---|
  | 10.2.3 | `cli` removed | 4 | FAIL | 30 at `swapgs` (IF=1, kernel GS), 13 at `iretq`, all 13 on the user GS; the first user-GS RIP is `syscall_exit_iretq` |
  | 10.2.3 | `cli` removed | 1 | FAIL | one at `swapgs` and one at `iretq` per pass, both `iretq` landings on the user GS |
  | 10.2.3 | fixed | 4 | PASS (12.9 s) | none in the window or on the user GS; 30 on the `cli` |
  | 10.2.3 | fixed | 1 | PASS (11.7 s) | none in the window; one on the `cli` per pass (count 44); the first user-return landing is count 45 |
  | 8.2.2 | `cli` removed | 4 | FAIL, 3/3 attempts | 28 at `swapgs`, 12 at `iretq`, all 12 on the user GS |
  | 8.2.2 | `cli` removed | 1 | FAIL, 3/3 attempts | one at `swapgs` and one at `iretq` per pass |
  | 8.2.2 | fixed | 4 | PASS (7.1 s) | none in the window; 27 on the `cli` |
  | 8.2.2 | fixed | 1 | PASS (6.2 s) | none in the window; one on the `cli` per pass (count 44) |

  After rebasing onto `2779c20b` (#26), the fixed build gave the same 10.2.3 results at both settings.

  Everywhere, the CPL-0 landings in the 16 bytes before the window cover seven boundaries (−16, −14, −12,
  −11, −10, −6, −1 from `syscall_exit_swapgs`: the last pops, the `mov rcx`, the `mov r11` and the `cli`).
  That is every instruction boundary of the exit sequence's last 16 bytes.
- **Not covered / honesty.**
  - One count per instruction is QEMU's icount model, not an architectural property. With several vCPUs,
    icount's round-robin scheduler shares the clock among them, so a count is less than one instruction on
    the probe's CPU and the mapping is not monotone (at `NEO_SMP=4` the `cli` landings scatter over counts
    314–425). On 8.2.2 the smallest counts are also delivered late, at the user return; 10.2.3 does not do
    that. What shows the sweep reaches the window is the no-fix control above, run on both QEMU versions,
    not the model. The `NEO_SMP=4` landing counts move a little with code layout (27–30 on the `cli` across
    builds and versions), so the margin over the coverage minimum is what matters, and it is wide.
  - Only the `SYS_SPAWN`-shaped exit is swept. The other exits rely on the audit, not a scenario: on
    `context_switch` running with IF=0 (`rotate`'s `debug_assert!`) and on every IF-setter restoring state. A
    future body that enables interrupts unconditionally, or an interrupt body that context-switches, would
    reopen their windows, and nothing armed would notice.
  - NMI, `#MC` and `#DB` can still land between `swapgs` and `iretq` (CP-44, *Residual windows*); `cli` does
    not mask them.
  - The `cli` closes only the window. Everything after `resume()` on the `SYS_SPAWN` path (the rest of
    `spawn_user`, `sys_spawn_impl`, the end of `syscall_dispatch`, the FPU restore, the GP pops) still runs
    with IF=1. Nothing there depends on IF=0 today; code added there must not assume it.
  - An `iretq` that faults (a non-canonical return RIP, e.g. a `syscall` in the last bytes of the user half)
    raises `#GP` at CPL 0 on the user GS. That is independent of IF, predates this change, and the `#GP`
    handler halts.
  - The scenario runs only in the `xsave-fpu` build. CI's `--no-default-features` build (PR #19) assembles the
    other variant's `cli`, but nothing runs it.
- **Known discipline.** On every path from an exit `swapgs` to `iretq`, IF=0 is established in the exit
  sequence itself, not inherited from an argument about the callers. Linux's `syscall_exit_to_user_mode()`
  disables interrupts (`local_irq_disable_exit_to_user()`) before the final exit work, and its asm return
  paths assume they stay off. Read for principle only.
- **Citation.** Intel SDM Vol. 2A `CLI`, Vol. 2A `IRET` (RFLAGS, IF included, comes from the popped frame;
  IRET has no interrupt shadow), Vol. 2B `SWAPGS`, Vol. 3A §6.8.1 (masking maskable hardware interrupts);
  QEMU `docs/devel/tcg-icount.rst` and `accel/tcg/cpu-exec.c` (interrupts checked between blocks; icount
  budgets); Linux `kernel/entry/common.c` (`syscall_exit_to_user_mode`), read for principle only.

## OT-34 trap-mining summary & honesty flags

| CP | Trap | Directly covers OT-34 hang? | Confidence |
|----|------|----------------------------|------------|
| CP-19 | Idle loop missing work-poll hook | **Yes** (latent until AP→idle unification) | Confident |
| CP-20 | AP parks before cross-CPU spawn completes | **Yes** (root race window) | Confident |
| CP-21 | EOI-only IPI vs busy AP that never re-enters scheduler | Partially (latency, not deadlock, under current timer) | Confident |
| CP-22 | `sti; hlt` serialization | No (currently guarded) | Confident |
| CP-23 | MWAIT loses IPIs | No (future trap, N-A today) | Verify-at-fix-time |
| CP-24 | Cross-CPU spawn pushes to wrong runqueue | No (lock-free slots avoid it) | Confident |
| CP-25 | AP busy-yield starves BSP work | Partially (latency under large partitions) | Confident |
| CP-26 | Partition join barrier races with AP parking | No (Release/Acquire guarded, parity-tested) | Confident |
| CP-27 | Reschedule IPI to AP in per-CPU init | No (barrier-guarded) | Confident |
| CP-28 | Idle re-check race: enqueue between rotate() and hlt | No (IF-off + sti;hlt guarded) | Confident |

**Bounded honesty.** CP-19 and CP-20 are **confident mappings** to the OT-34 failure seam. CP-21, CP-25 are
**latency traps** — they don't cause deadlock under the current timer guarantee but bound worst-case wake
latency for cross-CPU work during AP busy periods. CP-22, CP-26, CP-27, CP-28 are **currently guarded** —
included because the future fix-dispatch will change these seams (unifying idle, adding cross-CPU enqueue) and
must not regress them. CP-23 is **verify-at-fix-time** — it's a future trap for any MWAIT-based idle upgrade.

**Patterns-known ≠ correctness-proven.** These entries harvest documented classes from Linux, FreeBSD, Intel
SDM, and OSDev. They pre-inoculate the fix-dispatch against known failure modes. They do not replace the
future fix's QEMU gate, byte-oracle, or adversarial audit. The genuinely-ours seam — our specific
spawn/idle/handoff ordering under MPL primitives — still requires verification at fix time.

---

## CP-3x: wake-count test must barrier on enqueued, not pre-syscall "about to wait"

**Trap.** A wake-count test (e.g., "wake(2) returns exactly N, summing to exactly 3") must not use a
pre-syscall "about to wait" counter as its barrier. The value-protocol handles real lost-wakeups
(CP-8/CP-14), but it does not guarantee the waiter is actually parked in the futex queue. A counter
incremented before `sys_futex_wait` signals *intent* to park, not actual parking. The waker may call
`futex_wake` against an empty queue — the waiters haven't entered the queue yet — and get 0 picks.
All waiters then timeout. The test fails with a wake-count sum that never reaches N.

**Discriminator.** The kernel's real futex-queue length (ground truth) for the waker's key. A
`queue_len_for_key(key)` probe under the queue lock reveals whether the queue is empty at wake time.
An empty queue with3 parked-but-not-enqueued waiters is the signature.

**Fix shape.** Either: (a) poll the kernel queue length until it matches the expected count (ground
truth), or (b) bump the readiness counter from *inside* `wait_on_key` after the waiter is enqueued
(under the queue guard), so the counter genuinely proves parking.

**NOT valid:** sleeps, yields, or barriers on the pre-syscall counter — these are non-deterministic
and will be bounced.

**Citation.** Ours (OT-61 C1.1 C1.1). The general class is standard: POSIX `pthread_barrier_wait`
and Linux's `wait_for_completion` both barrier on the *actual* state change, not on intent. The
bug manifested as an intermittent ~2-in-3 failure on shard 2 because pure scheduling order (all
threads on cpu0) determined whether the waker ran before the waiters parked.

---

## Category K — Thread-creation register hygiene (privilege boundary)

### CP-45 — A new thread's first register state is copied from its creator's live registers

- **Failure mode.** A ring-3 thread is created from kernel context: `spawn_user` (every process's first
  thread), `SYS_THREAD_CREATE`, a test harness. Whatever the creating CPU's registers hold at that moment is
  what the new thread starts with, and the creating context is kernel code: a blake3 hash state, a
  struct-move temporary, an in-kernel crypto intermediate, a heap or stack pointer. The thread's first
  instruction in ring 3 can read all of it. That is a kernel information leak, and with kernel pointers in
  the mix it is a KASLR bypass. The reverse direction also exists: a kernel thread created during a syscall
  inherits the *user's* MXCSR (e.g. exceptions unmasked), so ring 3 chooses the floating-point environment
  of a new kernel thread, and in-kernel inference on that thread can take a `#XM`.
- **Root mechanism — two seams, one rule.** A new thread has no previous state of its own, so "save the
  outgoing state, restore the incoming state" does not apply on its first switch-in. Whatever is restored
  must be *constructed*, and both seams constructed it from the wrong source:
  1. **FPU (`xsave-fpu`).** `FpuState::new_current()` filled a new thread's save image by running
     `fpu_save_area_asm` on the *creating* CPU. The first switch-in then XRSTOR/FXRSTOR'd the creator's
     x87/SSE/AVX state into the new thread. Without `xsave-fpu` there was no per-thread image at all, so a
     new user thread was entered on whatever was live at the switch.
  2. **GP.** `enter_userspace_trampoline` was `swapgs; iretq`. The callee-saved registers come from the
     zeroed initial frame, but the caller-saved ones still held `context_switch`'s arguments and scratch:
     the outgoing thread's `rsp` slot address, the incoming kernel stack pointer, the FPU-image pointers,
     the XSAVE mask. (`enter_userspace_arg_trampoline` already zeroed them.)
- **Why it is invisible.** Nothing fails. Userland starts with garbage registers it never reads, every
  functional test passes, and the leak only matters to a program that looks. The kernel being compiled with
  SSE (see CP-38) is what turns "garbage" into "kernel data": the xmm registers are full of live kernel
  values at every thread creation.
- **Measured before the fix** (`main` @ `57261d66`, QEMU, `NEO_SMP=4`). The new ring-3 thread saw the
  spawner's armed MXCSR (`0x9f80`), x87 control word and stack marker, 12 of 16 xmm registers carrying the
  spawner's pattern and all 16 YMM upper halves carrying it (`-cpu max`). Its scratch GP registers held
  kernel addresses (e.g. `0xffff80001fed77d8`, a direct-map address). In the non-`xsave-fpu` build, `xmm0`
  held a kernel-image address (`0xffffffffafc2a140`). The kernel-thread leg showed the same FPU inheritance.
  Re-measured on `main` @ `ea2c1b77`, after CP-41's control-word reset: both scenarios still fail 4/4. MXCSR,
  FCW, FSW and FTW now read as the defaults, but 12/16 xmm registers, 16/16 YMM upper halves (`-cpu max`), the
  x87 register contents (tagged empty, still readable through FXSAVE) and kernel addresses in the GP registers
  (e.g. `rcx=0xffffffffd02243c0`) still reach the new thread.
- **Our exposure — REPAIRED.**
  - `FpuState::new_current()` is gone. CP-41 had already reset its control words (FCW/FSW/FTW/MXCSR),
    but it still copied the creator's FOP/FIP/FDP and x87/SSE/AVX payload. Every `Thread` constructor
    (`new`, `adopt_current`, `new_user`, `new_user_with_arg`) now uses `FpuState::new_initial()`, built
    entirely from constants: the legacy region is `FXSAVE_INIT_IMAGE` (FCW = `KERNEL_X87_FCW` 0x037F,
    MXCSR = `KERNEL_MXCSR` 0x1F80, everything else zero; a `const` assertion pins those bytes) and the
    XSAVE header is zero. XSTATE_BV=0 makes
    XRSTOR initialize every component (x87 FNINIT state, xmm and YMM uppers zero). MXCSR has to be written
    explicitly, because standard-form XRSTOR loads it from the legacy region whenever SSE or AVX is in
    RFBM, regardless of XSTATE_BV; a zeroed image would unmask every SIMD exception. `adopt_current`'s image
    is overwritten by its first switch-out before it is ever restored; it uses the same constructor so that
    no way of copying live state remains.
  - Both ring-3 entry trampolines zero `rax, rcx, rdx, rsi, rdi, r8-r11` (`rdi` = the arg in the arg
    variant). Without `xsave-fpu` they also `fninit; fxrstor` the init image. FNINIT also clears FIP/FDP/FOP,
    which FXRSTOR does not restore on AMD parts when no x87 exception is pending (the CVE-2006-1056 class).
- **Discriminator (stays armed).** `thread_entry_clean_state` (`-cpu max`, real XSAVE) and
  `thread_entry_clean_state_fxsave` (qemu64, the `xsave-fpu` FXSAVE fallback). The spawner arms a pattern
  in xmm0-15 (all of ymm0-15 under AVX), MXCSR=0x9F80, FCW=0x027F and an x87 marker, then creates a ring-3
  thread through each entry trampoline and a kernel thread. Each new thread stores its GP registers and
  FXSAVEs as its very first instructions (the ring-3 probes also store ymm0-15). The kernel probe's entry is
  a naked function whose first instruction is the `fxsave` (then a `jmp` to the Rust remainder), so no
  compiler prologue can touch the FPU state before the capture. (Originally the entry was an ordinary Rust
  function with an inline `fxsave`, and a prologue could in principle have emitted vector instructions
  first. Codex review F1, made structural in follow-up E.) All must see the init state,
  including the default control words (MXCSR 0x1F80, FCW 0x037F, every x87 tag empty), and the ring-3 probes
  must also see zero GP registers. Before the fix both scenarios fail on every attempt with
  `dirty=entry:fpu+gp,arg:fpu,kthread:fpu`; after it both pass. The compile-time non-`xsave-fpu` variant
  was checked locally only (CI builds `--no-default-features` but runs no scenario on it): it fails before the
  fix, passes after, and fails again (`entry:fpu,arg:fpu`) with the trampoline reset removed. With the naked
  kernel entry, re-introducing the creator snapshot for kernel threads only (the pre-fix `Thread::new`
  behaviour) fails both scenarios 4/4 with `dirty=kthread:fpu` (`NEO_SMP=4`).
- **Not covered / residuals.**
  - Without `xsave-fpu`, kernel threads still start on the live registers. That build keeps no
    per-thread FPU state by design, and kernel-to-kernel is not a privilege boundary.
  - On pre-Zen2 AMD (no CPUID `XSAVEERPTR`), FXRSTOR does not load FIP/FDP/FOP when no x87 exception is
    pending. So with the FXSAVE fallback (`xsave-fpu` on a CPU without XSAVE/AVX) a new thread keeps the x87
    instruction/data pointers of the last x87 instruction run on that CPU. That can be another process's
    ring-3 x87 code, which makes it a cross-process user-address leak, and it applies to every context
    switch, not only thread creation. Linux's `X86_BUG_FXSAVE_LEAK` workaround (clobber the pointers with a
    known `fild` before restoring) is the known fix; it is not done here. The non-`xsave-fpu` trampolines
    are covered by their FNINIT.
  - No CI job builds without `xsave-fpu`, so the non-`xsave-fpu` trampoline reset (like every other
    non-`xsave-fpu` asm variant) is never assembled in CI; `cargo check` does not assemble `global_asm!`.
- **Neighbours of the same class, fixed separately.** The syscall return path's `rcx`/`r11` (CP-43); kernel
  code running on the interrupted user's MXCSR (CP-41); an interrupt body's xmm values landing in the
  interrupted ring-3 context (CP-38). CP-45 covers only thread creation.
- **Known discipline.** A new execution context gets *constructed* state, never a copy of the creator's live
  registers. Linux starts kernel threads and exec'd programs from `init_fpstate`, and `fpu_clone()` copies
  only the parent's *saved user* image, which is safe there because Linux keeps kernel code out of the FPU
  (`-mno-sse`, `kernel_fpu_begin()`). `start_thread_common()` clears the user register frame for exec. Read
  for principle only.
- **Citation.** Intel SDM Vol. 2A `FXSAVE`/`FXRSTOR` (legacy-area layout) and `FINIT/FNINIT` (x87 init
  state); Vol. 1 §13.4.2 (XSAVE header) and §13.8.1 (standard-form XRSTOR: XSTATE_BV-driven initialization,
  MXCSR loaded when RFBM[1] or RFBM[2] is set); AMD APM `FXSAVE`/`FXRSTOR` (FIP/FDP/FOP behaviour); Linux
  `arch/x86/kernel/fpu/core.c` (`fpu_clone`, `init_fpstate`) and `arch/x86/kernel/process_64.c`
  (`start_thread_common`), read for principle only.

## Category L — SYSCALL MSR configuration (privilege boundary)

### CP-47 — SFMASK leaves RFLAGS.NT set, so the kernel runs with NT=1 and its own `iretq` faults

- **Failure mode.** Ring 3 sets RFLAGS.NT with `popfq` (NT is not privileged; POPF at CPL 3 may change it)
  and makes any syscall. SYSCALL clears only the RFLAGS bits named in IA32_FMASK, and NT was not one of them,
  so the kernel ran the whole syscall with NT=1. In 64-bit mode IRETQ with NT=1 is #GP(0) (the task-return
  path does not exist in IA-32e mode), so the syscall's own return `iretq` faulted in ring 0, on the user GS
  (it sits right after the exit `swapgs`), and the #GP handler halted the CPU. A syscall that blocked or
  yielded first would hand NT=1 on: `context_switch` does not save RFLAGS, so the next thread resumed in
  ring 0 with NT=1, and the next `iretq` it ran while NT was still live faulted: a syscall exit, a new
  thread's first entry to ring 3, or an interrupt handler's return. Interrupt delivery clears NT, but a thread
  that had been preempted inside a handler resumes there with the switching thread's flags and runs the
  handler's `iretq` with NT=1 (measured; see the controls below).
- **Why it matters beyond the halt.** Today any ring-3 #UD or #GP also halts its CPU (user faults other than
  #PF are not contained yet; see the CP-40 audit table), so the halt adds no new denial of service on its
  own. What CP-47 adds is a fault *in ring 0*, at the kernel's own return instruction, on the user GS, which
  user-fault containment would not cover, and a flag that crosses from one process's thread to another's.
- **Measured before the fix** (`main` @ `82d0f591` plus the discriminator, NT removed from SFMASK, local
  QEMU 8.2, `NEO_SMP=4`). FAIL 4/4: `reason=unexpected_trap trap=general_protection_fault rip=<kernel>
  cs=0x8 rflags=0x4046 err=0x0 user_gs_at_trap=true`. The trapped RFLAGS has NT (`0x4000`) set, CS is the
  kernel's and GS is the user's: the `iretq` after the exit `swapgs` of the first leg.
- **Our exposure — REPAIRED.** `RFlags::NESTED_TASK` is now in SFMASK (`arch/x86_64/syscall.rs`,
  `write_syscall_msrs`; the boot line reads `SFMASK=IF|TF|AC|DF|NT`). The user's NT is not lost: R11 keeps the
  user RFLAGS and the `iretq` frame restores it, and IRETQ checks the *current* NT, not the popped image.
  Nothing else needed changing:
  - Interrupt- and trap-gate delivery clears NT architecturally (with TF, VM and RF), so the IRQ stubs, the
    timer stubs and every exception handler enter with NT=0. (Every IDT entry this kernel installs is an
    interrupt gate; nothing calls `disable_interrupts(false)` on an entry.)
  - The kernel's only `popfq`s are the CP-39/CP-40 entry masks, which reload the live (already NT=0) kernel
    flags with AC and DF cleared. No kernel path loads a user-supplied RFLAGS value outside an `iretq` frame,
    and the ring-3 entry trampolines' frames use the constant `USER_RFLAGS_RAW`.
- **Discriminator (stays armed).** `syscall_nt_masked`. A ring-3 probe sets RFLAGS to `0x4202` (IF|NT|bit 1)
  and makes three syscalls: an unassigned number (straight back out of the dispatcher with `-ENOSYS`) and two
  `SYS_SCHED_YIELD`s (a context switch while NT would be live). It records RFLAGS just before and just after
  each `syscall`, with only `mov`s in between. The kernel counts dispatches that ran with NT=1, and #GP and
  #DF route to a hook that fails the scenario; those two are the deterministic guards. The harness thread,
  which runs between the probe's yields, also checks its own NT, but that is a weak check: it sees whichever
  thread last switched to it, and a timer interrupt in between clears it. PASS requires every leg's return
  value, NT set before each syscall (so the probe really armed it), RFLAGS unchanged across it, 0 NT=1
  dispatches, and NT=0 in the harness thread. After the fix: PASS (`legs=enosys,sched_yield,sched_yield_2
  user_nt=armed+restored kernel_nt=0 harness_nt=0`) at `NEO_SMP=1` and `4`. Controls (4/4 each,
  `NEO_SMP=4`): NT left out of SFMASK ⇒ the #GP above; NT left out of SFMASK but cleared just before both
  exit `iretq`s (a fix in the wrong place) ⇒ `reason=kernel_ran_with_nt syscalls=4` on three attempts, so the
  dispatch counter catches what the syscall's own `iretq` no longer does, and on the fourth a #GP at an
  `iretq` in ring 0 on the kernel GS (`user_gs_at_trap=false`, RSP on a kernel thread stack): NT carried by
  `context_switch` into a thread that had been preempted inside an interrupt handler.
- **Not covered.** The context-switch leak shows up only in the wrong-place control, on one attempt in four:
  with the fix simply removed, the first leg's `iretq` halts the CPU before any yield. The probe runs on the harness thread's CPU,
  so at `NEO_SMP=4` only that CPU's SFMASK is exercised; the APs write theirs in the same
  `write_syscall_msrs`. The kernel never sets its own RFLAGS at the boot handoff (`_kernel_start`, the AP
  entry); NT=0 there relies on the bootloader. SFMASK still leaves the arithmetic flags, RF and ID alone
  (Linux masks those too); none of them changes what ring-0 code does here.
- **Known discipline.** Mask every RFLAGS bit at SYSCALL that ring 3 can set and that changes ring-0 behaviour.
  Linux's `syscall_init()` writes `MSR_SYSCALL_MASK` with NT included (alongside TF, IF, DF, AC, IOPL, RF, ID
  and the arithmetic flags); read for principle only.
- **Citation.** Intel SDM Vol. 2B `IRET/IRETD/IRETQ` (in IA-32e mode, NT=1 ⇒ #GP(0)), `SYSCALL` (RFLAGS ←
  RFLAGS AND NOT IA32_FMASK), `POPF` (NT writable at CPL 3); Vol. 3A §6.12.1.3 (interrupt and trap gates
  clear TF, VM, RF and NT); Linux `arch/x86/kernel/cpu/common.c` (`syscall_init`), read for principle only.

### CP-48 — A DPL3 32-bit code segment plus an unprogrammed IA32_CSTAR: ring 3 reaches ring 0 at RIP 0

- **Failure mode.** GDT slot 3 (selector `0x18`, `0x1B` with RPL 3) held a present DPL3 32-bit code segment
  (`0x00CF_FA00_0000_FFFF`), and IA32_CSTAR, the SYSCALL target for compatibility mode, was never written (0
  from reset). Ring 3 can far-jump to `0x1B` and run in compatibility mode. A `syscall` there is, on AMD and
  under QEMU TCG, a ring-0 entry at RIP = CSTAR = 0, in 64-bit mode at CPL 0, with the user's RSP and the
  user's GS: no stack switch and no `swapgs`, because the entry code that would do them does not exist.
  - With SMEP and SMAP (the gate's `qemu64,+smep,+smap`) and RSP on the user stack: the fetch at 0 takes a
    #PF, pushing the #PF frame onto the user stack is an SMAP violation, and the CPU double-faults and halts.
  - RSP is the user's choice, and nothing in the far jump and `syscall` changes it. Set to a kernel address
    first, the #PF frame is a supervisor write to a supervisor page, which SMAP allows: a kernel-memory write
    at an address ring 3 picks, then the #PF handler running on that stack with the user's GS. (This needs
    the upper half of RSP to survive the switch to compatibility mode, which the architecture leaves
    undefined and which QEMU TCG and real CPUs do in practice. Inferred, not measured.)
  - Without SMEP (SMEP and SMAP are enabled only when the CPU advertises them): ring 3 can map page 0 (the
    ELF loader puts no lower bound on a PT_LOAD address), so the fetch at RIP 0 runs user code at CPL 0. That
    is a full privilege escalation. Inferred from the architecture, not measured.
  - On Intel, SYSCALL in compatibility mode is #UD, but compatibility mode itself stayed reachable, with
    `bound` (#BR) and the rest of the 32-bit instruction set.
- **Root mechanism.** The 32-bit segment was there because x86-64 GDT layouts traditionally reserve
  STAR's user base for one (SYSRETL returns to it). This kernel never runs 32-bit code and never uses SYSRET,
  so the segment had no user, and the MSR that would have made it safe was left at its reset value.
- **Measured before the fix** (local QEMU 8.2, `NEO_SMP=4`, `qemu64,+rdrand,+smep,+smap`; 4/4 each):
  - Old slot 3 and CSTAR unwritten: `reason=unexpected_trap trap=double_fault rip=0x0 rsp=0xe20ff8 cs=0x8
    rflags=0x2 err=0x0 user_gs_at_trap=true`, with the structural leg reporting slot 3
    (`gdt_ring3_bad_slots=0x8`). Ring 0 at RIP 0 on the probe's own user stack (`0xe20ff8`) and GS, IF cleared
    by SFMASK.
  - Old slot 3, CSTAR at the new sink: the probe reaches the sink and the CPU halts (the scenario times out).
- **Our exposure — REPAIRED, in layers.**
  1. **No compatibility-mode code segment.** Slot 3 is now a null descriptor (`gdt.rs`,
     `COMPAT_CODE_HOLE_RAW`). The slot is kept, not removed, so the user selectors (`0x23` data, `0x2B` code,
     hard-coded in the syscall asm and asserted) and STAR's SYSRET arithmetic (user base `0x1B`, data =
     base + 8, code = base + 16) are unchanged. Loading `0x1B` into CS now raises #GP(`0x18`). The kernel
     installs no call gates and no conforming code segment.
  2. **No LDT.** `gdt::init_for_cpu` now loads a null LDTR (`lldt 0`) on every CPU. After reset LDTR is a
     usable table at linear 0 (limit 0xFFFF), and nothing guaranteed that the firmware or bootloader had
     reloaded it; if not, a TI=1 selector takes its descriptor from page 0 of the current address space,
     which ring 3 can map without SMAP, and a user-written 32-bit code descriptor or call gate there would
     bypass layer 1. Measured under the gate's QEMU and Limine boot: LDTR was already invalid (a ring-3
     `lar` on an LDT selector finds nothing and fetches nothing), so this layer removes a dependency on
     firmware rather than fixing a reproduced bug. With layers 1 and 2 the 64-bit user code segment is the
     only code segment a far transfer or an `iretq` frame from ring 3 can name.
  3. **CSTAR is programmed.** It points at `compat_syscall_sink` (`cli; 1: hlt; jmp 1b`), which never touches
     memory or the stack. If a compatibility-mode segment ever came back, a compat SYSCALL would park that CPU
     instead of running ring 0 on a user-chosen RSP and GS. That fails closed but is not contained to the CPU:
     the parked CPU stays in the TLB-shootdown target mask, so the next shootdown times out and panics the
     kernel, and the threads on its ready queue are stranded. Today's halt on a ring-3 #GP or #UD has the same
     effect. NMI enters on its own IST stack, so an NMI in the sink does not touch the user stack either. Linux
     points CSTAR at `ignore_sysret`, which returns `-ENOSYS` with SYSRETL. That is friendlier, but it is code
     that runs only when the layers above have already failed; halting is the smaller fail-closed choice.
  4. **SYSENTER_CS is written 0.** On Intel, SYSENTER is legal from 64-bit ring 3 and enters ring 0 at
     SYSENTER_EIP/ESP unless SYSENTER_CS[15:2] is 0, which makes it #GP(0). The kernel never wrote the SYSENTER
     MSRs, so that relied on firmware leaving the reset value. (AMD #UDs SYSENTER in long mode.)
- **Discriminator (stays armed).** `compat_mode_unreachable`.
  - Structural legs, read before the probe runs: no descriptor in this CPU's live GDT that ring 3 could enter
    may be anything but 64-bit code (SGDT walk: no present code segment with L=0 that is DPL3 or conforming,
    no present DPL3 system descriptor; the 16-byte TSS descriptor is skipped); IA32_CSTAR equals the sink;
    IA32_SYSENTER_CS[15:2] = 0.
  - Live legs, in a ring-3 probe: `lar` on an LDT selector (`0x0F`) and on `0x1B` must both report nothing
    (`lar` clears ZF on a selector ring 3 cannot use, without faulting, but a live LDT makes it read the
    descriptor, which faults on the probe's empty page 0). Then a far jump to `0x1B` (a `jmp m16:32` through a
    memory operand), aiming at a compatibility-mode `syscall`, must take #GP(`0x18`) at exactly that
    instruction, in ring 3 with CS `0x2B`.
  - #GP, #DF, any ring-3 #PF and a ring-0 #PF at RIP 0 route to the scenario's hook.
  - After the fix: PASS (`lar_ldt=none lar_compat=none compat_cs_load=gp(0x18) gdt=64bit_only cstar=sink
    sysenter=gp`) at `NEO_SMP=1` and `4`. Controls (4/4 each, `NEO_SMP=4`): old slot 3 and CSTAR unwritten ⇒
    the `double_fault rip=0x0` failure above; slot 3 null but CSTAR left at 0 ⇒ `reason=cstar_not_sink
    cstar=0x0`; a live LDT at base 0 (an LDT descriptor added to the GDT and loaded instead of the null LDTR)
    ⇒ `reason=ldt_descriptor_fetch_faulted rip=0xe00005 cr2=0x8` (the `lar`, reading LDT entry 1 at linear
    `0x8`); SYSENTER_CS written `0x08` ⇒ `reason=sysenter_cs_usable sysenter_cs=0x8`.
  - The far pointer is a memory operand, not a `retfq` off the stack: QEMU TCG pops a far return's stack as a
    supervisor access, so under SMAP a ring-3 `retfq` takes a #PF on its own user stack before it ever checks
    CS. Real hardware does not; the memory-operand form avoids the emulator quirk.
- **Found while building the discriminator, not fixed here (EXPOSED).** Every `x86-interrupt` handler whose
  vector pushes an error code (#DF, #TS, #NP, #SS, #GP, #PF, #AC) makes its calls with RSP ≡ 8 (mod 16), 8
  bytes off the SysV alignment its callees assume. The handlers without an error code are aligned. Measured on
  this toolchain by disassembly and live (an `rsp` read in a callee). It matches LLVM bug 45215 (the
  x86-interrupt prologue's extra 8-byte adjustment for the error code). A callee that uses an aligned SSE
  stack slot raises #GP(0). Here a string `==` in the hook did it: an unoptimized `memcmp` keeps `u128`s in
  `movaps` slots. The #GP re-entered the same handler, and it recursed (33 levels) until the kernel stack
  overflowed: a #DF at `NEO_SMP=1`, and at `NEO_SMP=4` a panic in the scheduler's ready ring, consistent with
  the overflow running into neighbouring heap objects (kernel stacks are heap allocations without guard
  pages). The hook now takes an enum, and a trap inside it ends the run at once. The production #PF
  fault-kill path runs on the same misaligned stack, and static analysis of the gate's (debug) kernel finds an
  aligned SSE store on it: `page_fault` → `fault_kill_current_user` → `futex::purge_pid` →
  `futex::recompute_earliest` (`movaps` to stack slots that are 8 mod 16), reached when the killed process has
  a thread in a timed `SYS_FUTEX_WAIT`. If that holds, two ring-3 threads (a timed futex wait and a bad
  pointer) make ring 0 take #GP(0) and halt with the futex queue lock held, which can deadlock other CPUs.
  Not measured yet. Tracked as its own follow-up.
- **Not covered.**
  - At `NEO_SMP=4` the probe, the SGDT walk and the MSR reads all run on the harness thread's CPU. The APs
    get their GDT, LDTR and MSRs from the same `gdt::init_for_cpu` and `write_syscall_msrs` (both run in
    `init_traps_ap` before an AP's scheduler is published), but nothing reads them back.
  - The LDT and SYSENTER layers cannot fail before the fix here: the gate's boot already leaves LDTR invalid
    and SYSENTER_CS at 0. Their controls show only that the legs would see a regression.
  - Intel's #UD for a compatibility-mode SYSCALL and SYSENTER's #GP(0) are not exercised: the probe faults at
    its far jump and never executes SYSENTER.
  - #MC has no IST stack, so a machine check in the sink (or in `syscall_entry` before its stack switch) is
    delivered on the user-chosen RSP. That is true of the 64-bit entry already and is not changed here.
  - A regression that turns the far jump into #UD, #NP, #TS or #SS ends in the scenario's timeout, not a named
    FAIL: those handlers halt and are not hooked.
- **Known discipline.** Do not give ring 3 a code segment or descriptor table for a mode the kernel does not
  support, and program every entry MSR and descriptor-table register the CPU can use, even ones for
  unsupported modes. Linux loads a null LDT in `cpu_init()` and, without IA32 emulation, points CSTAR at
  `ignore_sysret` and loads SYSENTER_CS with an invalid selector so SYSENTER faults; read for principle only.
- **Citation.** Intel SDM Vol. 2B `SYSCALL` (#UD outside 64-bit mode on Intel) and `SYSENTER`
  (#GP(0) when SYSENTER_CS[15:2] = 0), Vol. 2A `JMP` (far jump: #GP(selector) for a descriptor that is not a
  code segment), `LAR` and `LLDT` (a null selector marks LDTR invalid); Vol. 3A, "Processor State After Reset"
  (LDTR base 0, limit 0xFFFF); AMD APM Vol. 3 `SYSCALL` (compatibility-mode SYSCALL through CSTAR);
  LLVM bug 45215 (x86-interrupt stack alignment with an error code); Linux `arch/x86/kernel/cpu/common.c`
  (`syscall_init`, `cpu_init`) and `arch/x86/entry/entry_64.S` (`ignore_sysret`), read for principle only.

### CP-49 — A SMAP window helper that does not nest and must not span a thread switch

- **Failure mode.** `with_user_access(f)` runs `stac`, then `f`, then `clac`.
  - If `f` calls `with_user_access` itself, the inner `clac` closes the window while the outer closure still
    has user accesses to make. The next one takes a SMAP `#PF` and halts the kernel.
  - If `f` blocks or yields, `context_switch`, which does not save RFLAGS, hands AC=1 to the next thread, so
    that thread runs with SMAP off. This thread later resumes with the other thread's AC, and its next user
    access faults.
- **Root mechanism.** AC is a single CPU flag. It is not a nesting count and not per-thread state, so `clac`
  cannot tell that an outer window is still open, and the scheduler neither saves nor restores it.
- **Our exposure — LATENT → GUARDED.** Jarvis's Codex review of CP-42 (`Plans/RETURN-2026-10-17-cp42-codex-crosscheck-jarvis.md`
  on `codex/cp42-crosscheck-jarvis`, `ca4f027c`) audited every caller. None nests, and every closure is a
  plain copy. Janus's review of #22 raised the blocking case. No current caller blocks either.
- **Guard.** When SMAP is active, `with_user_access` (`arch/x86_64/mod.rs`) checks AC on entry. If AC is
  set, it closes the window it is abandoning with `clac`, so the panic path runs under SMAP, and panics. The
  check holds because kernel code outside a window runs with AC=0:
  - SYSCALL masks AC through SFMASK;
  - the IRQ and timer stubs and every exception vector clear AC before any Rust runs (CP-40).

  So an interrupt taken inside a window enters its handler with AC=0: a `with_user_access` in IRQ context
  passes the check, and `iretq` restores the interrupted window. A window preempted with IF=1 is also safe,
  because the timer stub clears AC for its body and `iretq` restores it. AC=1 at entry therefore means a
  lexical nest or an AC leaked from a blocked window, and both are bugs.
- **Why not make nesting work instead.**
  - *"Skip `stac`/`clac` when AC is already set"* makes a leaked AC sticky. Today the next window's `clac`
    clears a leak; under that rule the leak would persist.
  - *A nesting depth* needs per-thread state. Syscall bodies can run with IF=1 (CP-38 records `sys_read`'s
    path lookup doing so). A window opened there can be preempted, and another thread then runs on that
    CPU, so a per-CPU depth would be wrong.

  Neither is worth it while no caller nests.
- **Discriminator (stays armed).** `user_access_nesting`. Leg 1 reads the user page in a single window and
  checks that AC is clear afterwards. Leg 2 nests a call inside a window and then reads once more in the
  outer closure.
  - **With the guard:** the inner call panics on entry, and the scenario passes with
    `expected_panic=with_user_access_nested`. The harness accepts only this panic: the flag is armed inside the
    outer window just before the inner call, and the panic must be raised in `arch/x86_64/mod.rs` with the
    guard's message.
  - **Without it** (the assertion disabled locally): FAIL 4/4 with
    `reason=outer_window_closed_by_nested_with_user_access`, which is the SMAP `#PF` at the outer read.
- **Not covered.**
  - **The blocking case, only in part.** The guard fires in a thread that inherits AC=1 only if that thread
    was itself blocked in a kernel body and opens a window afterwards. Otherwise the leak goes unseen:
    - a thread resuming from a timer preemption returns through its own `iretq`, which reloads its flags;
    - a new thread's first `iretq` to ring 3 loads its initial user flags;
    - a user thread returning to ring 3 gets the user's flags back;
    - a kernel thread never returns to ring 3, keeps AC=1 across its own interrupts (each `iretq` restores
      it), and passes it on at its next `context_switch`, so the leak can last indefinitely.
  - **The blocked thread's side.** It resumes with the other thread's AC. If that is 0, its next user access
    faults: loud, but not attributed to the block.
  - **A scheduler-side check.** Asserting AC=0 when switching away would catch a block at its source. It is
    not added here because it touches the scheduler.
  - **Untested paths.** No test blocks inside a window, calls `with_user_access` from IRQ context, or
    preempts a thread mid-window. The preemption argument rests on the timer stubs' AC clear, which no
    discriminator exercises: CP-40's AC leg goes through the `swapgs_irq_entry` test vector.
  - **Profile.** The gate builds the dev profile only. In an optimized build, the order "RFLAGS read before
    `stac`" is held by asm semantics (neither block is `pure`), not by a test.
- **Known discipline.**
  - A closure passed to `with_user_access` copies and returns. It must not make a nested call, take a lock
    that can block, allocate memory in a way that can sleep, or yield.
  - The guard makes AC=0 on kernel entry an availability invariant as well as a security one. A new entry
    path that skips the AC mask (an IRQ vector wired without the `swapgs_irq_entry` mask, a new gate, a
    `popfq` of a saved RFLAGS) now panics on its first user copy instead of silently running with SMAP off.
  - Linux enforces the same rules between `user_access_begin()` and `user_access_end()`. objtool warns on a
    recursive UACCESS enable, the nesting case, and on a call to any function not marked safe for an open
    UACCESS region.
- **Citation.** Intel SDM Vol. 3A §4.6.1 (SMAP and EFLAGS.AC); Linux `arch/x86/include/asm/uaccess.h`
  (x86 `user_access_begin`/`user_access_end`) and `tools/objtool/check.c` ("recursive UACCESS enable", and the
  call-with-UACCESS-enabled warning), read for principle only.

### CP-51 — A control event on a best-effort delivery path

- **Shape.** A producer that must not block uses `try_lock` and drops on contention. That is fine for motion,
  which the next report supersedes. It is wrong for a control event (an escape chord, a button release, a
  key release): the decoder has already consumed the transition, so dropping it loses state for good.
- **Where it bit.** S1-F1 (Chad): `route_event` dropped a key press or release when another CPU held
  `DELIVERY`. Jarvis F1: the proposed SAK used the same path, so a chord could be swallowed by the filter
  and then dropped.
- **Guard.** Decoded events go into a bounded per-device stage under the device lock and are flushed in order
  under one `try_lock`; contention moves nothing and loses nothing. Chords go to a lock-free coalescing
  latch (atomic OR) and never touch a queue. Overflow and contention are counted separately.
- **Discriminator.** `hid_contention`: a thread on another CPU holds `DELIVERY` while real reports drain;
  all press/release pairs arrive in order and the chord is latched exactly once. The legacy direct path
  fails it (0 of 20 events).

### CP-52 — A lost button-up leaves grab state stale

- **Shape.** Grab state ends when the observed buttons become zero. If the release is lost before it is
  observed, the grab outlives the press and the next press is routed to the old target.
- **Guard.** No kernel grab (DI-1 is B-ref: the WM owns grab policy). Releases are no longer lost to
  contention (CP-51). Every pointer record carries the full button bitmap, and a record minted after an
  expired or dropped transition carries `RESYNC`, so the WM re-learns the true state before acting on the
  next press. The raw-button latch holds the last decoded bitmap.
- **Discriminator.** `hid_contention` (a release drained while `DELIVERY` is held reaches the client; the
  latch reads 0); host KATs for RESYNC after an expired transition.
- **Bit again (#46, Jarvis F2).** The WM refused to treat a RESYNC record as a fresh press but kept
  the old `Grab::Client`, so a held-button RESYNC (a lost up, then a new press) was forwarded to the
  old target. Repaired in `neo_atrium::pointer_grab::decide` (the WM's per-record decision): a RESYNC
  record with a button held cancels the grab (consume until all buttons are up); with every button up
  it still reaches the grabbed client as its missed release. Host KATs drive that function.

### CP-53 — One-shot authority with no recovery owner

- **Shape.** "The first caller claims the system route, once." If the gate is fail-open (a feature toggle,
  an enforcement switch), any process can win the claim. If the holder dies or restarts, nobody can claim it
  again.
- **Where it bit.** Jarvis F3 on DI-1 Option A: the gate reused `scanout_authorized`, which is true whenever
  GPU-submit enforcement is off.
- **Guard.** An exact capability kind (`InputSystem`) that only the kernel mints, into the session root,
  with no DERIVE/GRANT. The check never consults another subsystem's enforcement state. A claim or reclaim
  bumps the session generation; a reclaim kills the old route and every record it minted.
- **Discriminator.** `di1_forward` (reclaim = new generation; the old route can no longer forward);
  `desktop_pointer_boot` (a client's claim, forward and SAK take all return EPERM).

### CP-54 — A bounded step whose result needs an uncapped lookup

- **Shape.** "At most 16 compares per press" was true for the rectangle walk, but delivering to the hit
  entry's focus id needed a linear search of an uncapped route list, on every grabbed motion.
- **Where it bit.** Jarvis F4 on DI-1 Option A.
- **Guard.** Count all the work a bound claims to cover, including resolution of the result. B-ref has no
  per-event lookup by id: a forward resolves one handle the caller names.
- **Discriminator.** Review only (no test can prove a bound on work it does not instrument).
- **Bit again (#46, Jarvis F3).** B-ref resolved the target *handle* first, but `mint_records` and
  `forward_record` still looked the route entry up by id in `Delivery.routes`, which any process could
  grow (create, activate, retain focus objects). Repaired by capping live client routes at
  `INPUT_ROUTE_CAP` (activation past it returns `EBUSY`); the pointer route is outside the cap, so a
  flood cannot block the WM's claim. Discriminator: `di1_forward` route_cap leg (cap+4 activations:
  exactly the cap succeeds, the pointer route still reclaims under the flood, freed routes are reused).

### CP-55 — Deferred work bound to the owner at completion, not at capture

- **Shape.** A HID report is decoded (captured) while focus A holds the keyboard, waits in a per-device
  stage because `DELIVERY` is busy, and is flushed (completed) to whoever is active *then*. After a
  handoff to B, A's keystrokes go to B: a disclosure, not just a delay.
- **Where it bit.** #44 (S1-F1 staging), Jarvis F1. The pre-staging code lost the event instead, which
  hid the class.
- **Guard.** Bind the deferred work to an epoch of the authority it was captured under. Every route
  change bumps `ACTIVATION_EPOCH` under `DELIVERY`; a stage carries the epoch of its batch; staging
  into a batch from an older epoch, or flushing one, discards it (counted `HANDOFF_DROPPED`). Fail
  closed: a lost key is an availability cost, a crossed key is a security one. A client that loses the
  keyboard treats every key as released (ADR-074 amendment), so a discarded release leaves no stale
  state behind.
- **Discriminator.** `hid_contention` handoff leg: DELIVERY held on another CPU, a real key staged for
  A, B activated before any flush; B and A must receive nothing and two events must be discarded.

### CP-56 — IF discipline fixed at one lock, missed at the lock before it

- **Shape.** CP-37 was repaired by taking the device lock with interrupts off, but the caller first
  cloned the device out of `PRIMARY` (another spinning `SwitchMutex`) with interrupts *on*. The RECV pump
  takes `PRIMARY` with them off, so a preempted fd-0 reader holding it deadlocks a same-CPU pump.
- **Where it bit.** #44, Jarvis F2.
- **Guard.** Audit every acquisition of every lock on the path, not just the one that was flagged: in
  `hid_keyboard`/`hid_mouse`, `PRIMARY` (bind store, lookups, `is_bound`) and every device-lock site run
  inside `without_interrupts`, whoever the caller is.
- **Discriminator.** `hid_contention` mixed_if leg: a test-only witness counts any `PRIMARY` or
  device-lock acquisition made with IF=1; the fd-0 entry points are driven with interrupts ON and the
  count must be 0. The `neo-test-hid-mixed-if-control` feature restores the pre-fix acquisition and
  must FAIL. (The hang itself needs a timer preemption inside the window, which only the BSP takes; the
  witness checks the invariant that rules it out.) Since Jarvis's N1 (the #55 cross-check), every
  acquisition is sampled, because each driver takes `PRIMARY` only through `primary_slot()` and its
  device lock only through `device()`. Before that, `debug_inject_report` and the bind-time `PRIMARY`
  stores went unsampled. The wrappers sample *before* acquiring, because `IrqSafeMutex::lock` masks IF
  itself: a sample taken after it sees IF=0 always, so it could only catch a `lock_as_caller` site,
  never a call site that stopped masking (Claude AAG on N1). A second control, `neo-test-hid-device-if-control`, takes the *mouse device*
  lock with the caller's IF, so the witness is proven for that lock class too, not only for the keyboard
  `PRIMARY`.
- **Known blind spot: `Drop` impls and teardown paths (#55).** The #44 audit walked every *call site* that
  takes a lock, and every one was IF=0 — yet `InputFocus::drop` still took `DELIVERY` with IF as its
  caller had it. A `Drop` has no call site to audit: it runs wherever the last reference dies, and process
  teardown (`terminate`, natural reap) drains the handle table inside `without_interrupts` but drops the
  drained vector *after* interrupts are back on. So the final `Arc<InputFocus>` died at IF=1 and took a
  lock the RECV syscall takes at IF=0: the same deadlock, reached from a path no grep for `lock()` call
  sites finds. Treat every `Drop` that takes a lock, and every teardown that drops handles, as an
  acquisition site whose caller is unknown.
- **Guard, made structural (#55).** A per-call-site `without_interrupts` is a convention each new caller
  (or `Drop`) must remember. `DELIVERY`, both HID `PRIMARY`s and both HID device locks are now
  `IrqSafeMutex`es: the guard masks interrupts itself and restores the caller's IF on release, so an IF=1
  acquisition is impossible by construction, whoever the caller is. `DELIVERY` is reached only through
  `focus::delivery()`/`try_delivery()`, which feed a neo-test witness
  (`debug_delivery_if_on_acquisitions`).
- **Discriminator (#55).** `hid_contention` teardown leg (NEO_SMP=4): a process whose handle is the last
  reference to the *active* focus, holding queued input, is killed by a caller with interrupts ON while
  a thread on another CPU keeps taking `DELIVERY` and pumping the real HID devices. The focus must be
  dropped by the kill, its route cleared, and the `DELIVERY` IF=1 witness must read 0. Control:
  `neo-test-delivery-drop-control` restores the pre-fix `Drop` (lock as the caller has it) and the
  scenario must FAIL `reason=delivery_taken_with_interrupts_enabled_in_teardown`.
- **EXPOSED again: the VFS file syscalls (found in the #120 lock audit).** OT-74 made `sys_read` and
  `sys_write` run their NeoFS body IF-on, so the NeoFS `SwitchMutex`es (`entries`, `root`, `record`,
  `inode_block`, `metadata`) gained preemptible IF=1 holders. `sys_fsync`, `sys_open`, `sys_readdir` and
  `sys_ftruncate` still took the same locks at the syscall's IF=0. One of them landing on the CPU of a
  `sys_write` preempted in `commit_child_update` would spin on `entries` forever. `sys_fsync` was also the
  IF=0 waiter in the CP-82 hang. **Guard:** the four bodies run in `vfs_body_if_on` (`with_interrupts_enabled`).
  User copies and handle-table work stay IF-off. Open's two short `MOUNTS` sections run under
  `without_interrupts`, because `MOUNTS` is still taken at IF=0 (mount, umount, model provenance).
  **Discriminator:** `phase3_neofs_syscall_if_on`. For each call, a kthread on the same CPU holds `entries`
  while yielding IF-on, and the call arrives at IF=0. It must complete no earlier than the holder's release.
  The control `neo-test-vfs-syscall-if-off-control` (old IF) hangs at the first leg.
  **`sys_model_load` too (lane A):** its NeoFS open and `read_model_file` run in `vfs_body_if_on`, and the
  build is unchanged. A preloaded boot model never reaches them. The scenario gained a `model_load` leg.
  **A model's provenance check too (lane D, review #58 item 4):** `validate_current` re-resolves a
  NeoFS-backed model's name (`entries`) from `sys_model_query` and the infer/generate entries. It runs in
  `vfs_body_if_on` at all six sites, with its `MOUNTS` check IF-off. A boot-module model's provenance has
  no mount and no generation and never reaches NeoFS. The scenario gained a `model_query` leg, with a
  second control that skips `model_load` so the hang lands in `model_query`.
  See `Plans/NOTE-2026-10-25-model-load-if-on-design.md`.
  See `Plans/NOTE-2026-10-25-vfs-syscalls-if-on.md`.
- **Was EXPOSED → guarded (lane C): `ARENA` (found in the model-load audit).** `ARENA` was a plain
  `SwitchMutex::new_ordered`, taken IF=1 and preemptibly by the forward on the inference stack (`exec.rs`
  weight `mint`, the KV cache), and at IF=0 by the tensor syscalls and handle-table `retain`/`release`. A tick
  inside an inference thread's `ARENA` section, then a same-CPU tensor syscall, was CP-37.
  **Guard (structural):** `ARENA` is an `IrqSafeMutex::new_ordered` at the same level (12), reached only
  through `arena_lock()`. Every section is short and park-free (`free`/`release` drain after the lock; no
  section takes another lock), and no ISR takes it, so I-14(a) is unchanged.
  **Discriminator:** `arena_if_off`. An alloc at IF=1, a `mint` from the inference stack's IF-on closure, a
  `mint` at IF=0 and a release must leave the witness of acquisitions that saw IF=1 inside at 0. The control
  `neo-test-arena-mixed-if-control` (the plain `SwitchMutex`) FAILs `arena_held_with_interrupts_on`. See
  `Plans/NOTE-2026-10-25-model-load-if-on-design.md`.
- **The same `Drop` blind spot, on `MOUNTS` (fixed with the follow-up).** Unmount dropped the last
  `NeoFsState` with `MOUNTS` held, and so did `install_root_child`'s replacement and the neo-test
  `neofs_typed` shortcut. `NeoFsState::drop` stops and joins the writeback kthread (block I/O, a park), so
  the holder went IF-on and parked while IF=0 takers spun. **Guard (structural):** `MOUNTS` is reached only
  through `lock_mounts()`. `MountTable` parks displaced filesystems in a graveyard, and `MountsGuard`
  releases the lock before dropping it. **Discriminator:** `phase3_umount_drop_outside_mounts`. A same-CPU
  kthread unmounts with the state drop held IF-on, and the scenario must take `MOUNTS` at IF=0 while the
  drop is held. The control `neo-test-mounts-drop-under-lock-control` hangs.

### CP-57 — A server-initiated send on a synchronous-rendezvous channel

- **Shape.** IPC here is a synchronous rendezvous: `sys_send` parks until the peer receives, and there is
  no non-blocking send. A server (the WM) that sends a client an unsolicited message parks until the
  client next receives on that channel, which a polling client may not do for a long time. If the client
  is itself parked sending the server a request (a commit), each waits on the other: a deadlock with no
  lock involved.
- **Where it would have bit.** DI-1b resize. `OP_CONFIGURE` (DW-2) flows WM → client, and the first design
  sent it on a per-surface channel. Caught in design, before it ran.
- **Guard.** A server never sends a client anything the client did not ask for. State the server must push
  is *published*: the WM writes the configure into a four-word seqlock mailbox (`libneo::atrium_configure`)
  that the client maps read-only, and the client polls it while idle (one atomic load). The reply leg
  stays client-initiated: the client acks with an ordinary `OP_CONFIGURE` request on its surface channel,
  and the WM honours it only if it equals the configure it published.
- **Discriminator.** Host KATs for the mailbox (no torn read in any interleaving of the publish's stores;
  the byte layout). `desktop_resize_boot`: a real usb-mouse corner drag is published, acked and shown, with
  the client's commits flowing on the same surface channel throughout.

### CP-58 — A periodic rescuer hides a missing wake

- **Shape.** A wait that also ends on a tick or a deadline still "works" when its event wake is lost: the
  rescuer ends it later, the level-triggered re-check finds the event, and the caller proceeds. Every
  functional test passes; only latency (or power) betrays the bug.
- **Where it would have bit.** OT-22's `SYS_WAIT_ANY`. Input waits re-check every tick by design, and every
  wait has a deadline, so a channel fire that never ran would be invisible to a desktop scenario.
- **Guard.** Test the wake path with nothing else able to end the wait: one channel item, a far deadline,
  no input item, and require the wait to end within a few ticks of the send (5; the deadline is 300) and
  the IPC-wake counter to move.
- **Discriminator.** `wait_any_core` ipc leg. Control: `wait-multiplex-no-ipc-fire` compiles the send-path
  fire out; the leg must FAIL (`reason=wait_any_ipc_wake_missing`; it shows the wait ending ~300 ticks
  late with `ipc_wakes=0`, its result still "ready": the rescuer at work).

### CP-59 — A hidden spinner keeps the CPU out of idle

- **Shape.** Parking every user loop does not idle a CPU if anything else on it yield-spins. Here the
  kernel's own wait-for-init loop (`yield_now(); reap()`) ran on the BSP for the whole session. Separately,
  the old `idle_ticks` counter tested `current_thread_id == u64::MAX`, which never holds once a scheduler is
  installed (the idle thread has a real id), so it read ~0 whatever happened.
- **Where it bit.** OT-22 rung 1's first run: the parked desktop measured 0% idle.
- **Guard.** The loop sleeps a tick per check (`wait_any::sleep_ticks`) when `wait-multiplex` is built; a
  tick counts as idle when the CPU is running its idle thread (`scheduler::current_is_idle`).
- **Discriminator.** `desktop_idle_parks` (≥ 80% of a ≥ 200-tick quiet gap idle) against
  `desktop_idle_spins` (the same kernel and script with the userland loops spinning: ≤ 5%). The pair is the
  evidence: either alone could be accounting noise.

### CP-60 — A kernel value captured lazily from live CPU state

- **Shape.** `KERNEL_CR3` was `Lazy::new(Cr3::read)`: "the kernel PML4 is what CR3 held before any user
  process loaded, captured on first call". Its first call is the first switch from a user-process thread to
  a kernel thread, and that switch reads it while the user process's PML4 is still loaded. From then on
  every kernel thread ran on that process's page table. When that process died, its PML4 was freed while
  still being the live CR3. The next `Pmap::new` reused the frame and zeroed it under the running CPU
  (`#PF` → `#DF` → triple fault). Writes through the freed tables also corrupted the buddy allocator, which
  once handed out the kernel's own PML4.
- **Where it bit.** OT-22's `wait_any_core` churns short-lived test processes. It reproduces on `main` with
  no OT-22 code (`token_park_teardown_churn`: 4/6 boots triple-faulted). On a normal boot the captured PML4
  is neo-init's, which lives until shutdown, so the bug stayed latent.
- **Why it hid.** Any earlier `kernel_cr3()` call taken while CR3 really was the kernel's forced the right
  value. Serial prints and debug checks that called it moved the first call earlier and made the crash
  vanish. "Instrumentation makes it disappear" pointed at an initialization order, not a race.
- **Guard.** `kernel_cr3()` returns the value captured at `mm::init`, before any user `Pmap` exists, never
  a lazy read of live state. `Pmap::drop` asserts that the freed PML4 is not the kernel's and is not any
  CPU's cached active CR3, so a stale CR3 fails loudly instead of corrupting memory.
- **Where the assertion runs (Jarvis F1, the bounce of the first fix).** It must run **before anything is
  freed and outside `with_frame_alloc`**. The first fix asserted inside the allocator closure, with
  `FRAME_ALLOC` held and IF=0, after the descendant frames were already freed. The bare-metal neo-test
  panic path (`panic_failed` → `debug_exit` → `log::flush` → block sink → `DmaBuffer::alloc_zeroed`)
  allocates on the same CPU, so the panic would recurse in debug builds or deadlock in release. **A panic
  raised under a lock that the panic path itself takes cannot report.**
- **Witness for that placement.** A per-CPU "holds `FRAME_ALLOC`" flag (neo-test).
  - `token_park_teardown_churn` requires at least 60 checks, with 0 of them under the lock.
  - The panic path prints `frame_alloc_held=`, so the lazy-CR3 control must fail with it `false`.
  - With the check moved back inside the lock, the scenario fails `pmap_live_check_under_frame_alloc` and the
    control shows `true`.
- **Discriminator.** `token_park_teardown_churn`: 60 rounds of a process whose thread token-parks and is
  `terminate`d, so each dead PML4 frame is handed to the next `Pmap::new`. Control:
  `neo-test-kernel-cr3-lazy-control` restores the lazy capture. The scenario must FAIL (the `Pmap::drop`
  invariant fires on the first round).

### CP-61 — A wait registers on what a mutable slot held, not on the slot

- **Shape.** `SYS_WAIT_ANY`'s items are numeric handles. The registration watched the channel each handle
  resolved to at the first scan. Two paths change what a handle names without touching that channel:
  - a close followed by an install reuses the lowest free slot, so the same number now names another
    channel (`HandleTable::install`);
  - a revoke clears the slot in every affected table (`remove_cap_ids`) and closes nothing.

  A waiter parked on the old channel's key is then woken by nothing. With no deadline and no input item,
  nothing rescans it, ever.
- **Where it bit.** OT-22 rung 1, found by the Codex cross-check (Jarvis rung-1 F1) before merge.
  - A close-and-reuse that lands between the first scan and the registration fires the old key to nobody.
  - A revoke after the park fires nothing at all.
- **Why the rung-1 legs missed it.** They checked wake *timing* for a stable handle (send before or after
  registration, the race window, the kill). None of them changed what a handle names. The tick rescues
  input waits, so only channel-only waits strand, and those are exactly the ones the IPC leg isolates.
- **Guard.** Two parts, one for each side of the registration:
  1. The scan records each channel item's wait key and handle `CapId`. `CapId`s are kernel-unique and
     never reused. The second scan runs after the registration and must see the same identity, or the
     wait drops the registration and starts over (re-watching, or returning EBADF). A change before the
     second scan's reads is seen here.
  2. A change after those reads goes through a `HandleTable` removal chokepoint (`remove`,
     `remove_cap_ids`, `drain`), which fires the removed channel handle's key (`WHY_HANDLE`). The
     happens-before chain runs from the second scan's handle-table lock to the remover's lock, then to the
     remover's `WAIT_TABLE` fire.
- **Discriminator.** `wait_any_core`:
  - **rebind leg:** a sibling thread of the waiter's own process closes the handle and reuses its slot
    inside the scan-to-register window, and a send on the new channel must wake the waiter;
  - **revoke leg:** an ancestor revokes the waiter's derived handle under an infinite, IPC-only wait, and
    the wait must return EBADF with no channel traffic.

  Controls: `wait-multiplex-no-identity-check` must FAIL the rebind leg (`wait_any_rebind_stranded`);
  `wait-multiplex-no-handle-fire` must FAIL the revoke leg (`wait_any_revoke_stranded`).

### CP-62 — Boot-owned memory handed to an allocator while it can still be borrowed

- **Shape.** Limine module bytes were documented as living for the kernel's lifetime, and
  `find_module_by_path_suffix` could mint a fresh `'static` slice of any module at any time. Handing a
  module's frames to the frame allocator under that contract leaves a live re-borrow capability: a later
  lookup reads frames that now belong to a tensor, a page table or the heap. Separately, the boot protocol
  guarantees alignment and non-overlap only for `USABLE` and `BOOTLOADER_RECLAIMABLE` entries. An
  `EXECUTABLE_AND_MODULES` entry may be unaligned, may overlap, and may describe the kernel and other
  modules too. And the buddy's coalescer computes buddies relative to its one `base_phys`, so a frame from
  outside its zone passed to `free()` underflows that arithmetic and poisons the free lists.
- **Where it would have bit.** B1's first design ("copy the model, then return the module's frames"). Chad's
  cross-family review (C2) refuted it before any code was written.
- **Guard (B1a).**
  - `boot_modules`: a consumable `/models/` module is never lent as `'static`. `take` hands out one
    `ModuleLease` and tombstones the path; its bytes borrow from the lease; `reclaim(self)` consumes it.
    `find_module_by_path_suffix` and `spawn_neo_init` route through the registry.
  - `module_reclaim_logic::plan_module_reclaim` plans from the module's exact file range, rounded **inward**
    to whole pages. The pages must be covered by type-6 entries only, and must be disjoint from the
    executable and every other module (both rounded **outward**). Anything else admits nothing.
  - `BuddyAllocator::admit_external_region` creates a separate zone with its own lists and `ADMITTED_PAGES`
    accounting. It never calls `free()`. A free that no zone owns is refused and counted, never coalesced.
  - CP-60's placement lesson applies: planning and logging run outside `with_frame_alloc`; under the lock,
    admission only pushes free-list nodes and cannot panic.
- **Discriminator.** `boot_module_reclaim`:
  - a consumable probe is not borrowable before the take, and is not reacquirable after it;
  - the forged-plan control (memmap re-typed) is refused with zero pages admitted;
  - admitted pages are allocated, written, freed and reallocated;
  - `frames_in_use` returns to baseline with `refused_frees=0`.

  Host KATs in `tools/module-reclaim-logic-tests` cover unaligned, overlapping, merged and fragmented memmaps,
  shared edge pages and address-space overflow.

### CP-63 — A shared deep stack under preemption, and a lock held across a long syscall

- **Shape.** B1c runs every real-LM forward on one dedicated 256 KiB kernel stack instead of deepening
  every thread's stack. Three hazards come with a stack that is not the running thread's own:
  1. **Two users at once.** If two threads (on two CPUs, or one preempted mid-forward and another
     scheduled on the same CPU) enter the forward together, both write the same stack and corrupt each
     other's frames with no fault.
  2. **Code that assumes `rsp` lies in the thread's stack.** A stack walker, a stack-range check, or a
     ring-3 entry that reloads `TSS.rsp0` from "the current stack" would misbehave while `rsp` points at
     the shared stack.
  3. **A lock held across a long syscall.** A spin lock held for the whole forward (seconds on the 0.5B)
     makes a second caller burn its CPU for that long, and a holder that is preempted (the forward is
     preemptible under `inference-preemptible`) strands every spinner behind it.
- **Guard (B1c).**
  - One user at a time: `inference_stack::run` takes a **yielding** lock (`compare_exchange` plus
    `yield_now()` on contention, then a short interrupt window before restoring the caller's `IF`).
    This is *not* `SwitchMutex`, which spins.
  - **`IF=0` waiters (found by the B1c AAG).** A syscall body runs at `IF=0`, and the inventory forward
    never reopens interrupts. A waiter that only yielded would therefore spin for a whole generation
    with no timer ticks and no shootdown or reschedule IPIs. The interrupt window in each contended
    round is the guard.
  - `rsp` outside the thread's stack is safe here, by audit: a context switch saves `rsp` per thread;
    `TSS.rsp0` and the per-CPU `kernel_rsp` come from the thread's own stack top and are used only on
    ring-3 entry, which cannot happen inside the syscall; no stack walker or stack-range check exists.
    **A future one must treat the inference stack as a valid kernel stack.**
  - The stack is mapped once at boot, before any user pmap or AP, so no mapping changes at run time
    (no shootdown).
  - Overflow is fail-closed: 64 KiB of unmapped guard below the stack turns an overflow into `#DF` on
    its IST stack, which names it and halts. It is 64 KiB, not one page, because the kernel target emits
    no stack probes, so one large frame could step over a single page. A canary at the lowest 64 bytes
    catches a near miss.
  - **Force-kill (B1c AAG J1, then J3/J4; see CP-65).** The first note here said force-termination acts
    only at the return to ring 3. That was wrong: `kill_thread_by_pid` zombifies any *Ready* thread,
    and an inference participant is Ready mid-syscall. The first fix skipped only the holder; Jarvis's
    re-check (J3) showed a *waiter* yielding for the stack is just as exposed. The guard is now CP-65's
    per-thread lifecycle marker plus cooperative cancellation.
- **Discriminator.** `inference_stack_overflow`: a deep-but-legal control returns with the canary intact
  and the lock observed held inside the call and free after; a canary write is detected; a 400 KiB
  recursion reaches `#DF` on the low guard (`[ trap  ] inference stack overflow guard=Low`).
  `real_lm_generate_boot` asserts every generate ran on the inference stack (calls, high-water). Host
  KATs in `tools/module-reclaim-logic-tests` cover the layout, guard classification, canary and depth scan.

### CP-64 — A long `IF=0` kernel section while the CPU is still shootdown-ready

- **Shape.** A CPU in `shootdown_ready_cpus_mask` is a promise: it will ACK a TLB-shootdown IPI within
  the sender's deadline (1 ms re-poke, 50 ms panic). A kernel section that runs with interrupts masked
  for longer than that breaks the promise without anyone noticing. Nothing goes wrong until another CPU
  happens to shoot down: a process reap, an unmap, a `protect`. Then that CPU waits on an ACK that cannot
  come, and **panics the kernel** (`tlb_shootdown_timeout`). On the BSP the same section also stops the
  only timer (PIC IRQ0 is delivered to the BSP alone), so timekeeping and preemption stop for its length.
  The trap is easy to miss because syscalls *enter* at `IF=0` (SFMASK): a long syscall body never
  "disabled" anything, it just never turned interrupts back on.
- **Found by.** Jarvis's B1c code AAG (J1, cross-family): the real-LM forward ran its whole generation
  (seconds) at syscall-entry `IF=0` on a shootdown-ready CPU. The first fix had only given the lock's
  *waiters* an interrupt window; the owner was the problem.
- **Guard.**
  - A kernel section expected to run longer than the shootdown deadline must run with interrupts on
    (the inference stack's closure does: `with_interrupts_enabled`, the caller's `IF` restored before it
    returns), or leave the ready mask first and flush on rejoin (`LocalShootdownQuiesce`, the option the
    PM ruled for only if interrupts-on proved unsound).
  - Running interrupts-on makes the section preemptible on the BSP. That has its own obligations:
    the FPU state must survive a context switch (`inference-stack` implies `xsave-fpu`, so the AVX2
    matmul's YMM uppers are saved), every lock taken inside must be IRQ-safe (the heap is), and a
    preempted holder of anything must not be killable (CP-63's force-kill guard).
  - A test that deliberately overflows a stack inside such a section masks interrupts for that part
    alone, since a timer frame copy near the guard faults as a plain `#PF` rather than reaching `#DF`.
- **Discriminator.** `inference_stack_shootdown` parks the inference-stack owner on the BSP, entered at
  `IF=0`, while CPU 1 sends 16 real shootdowns at it. It asserts:
  - every shootdown is ACKed, with `timeouts=0`, and the BSP is still shootdown-ready afterwards;
  - the BSP timer ticked while the owner was parked;
  - a co-runner on the BSP ran, so the owner was preempted;
  - the caller's `IF=0` is restored and the stack is released clean.

  `inference_stack_shootdown_control` builds the pre-fix owner, which keeps the caller's `IF`, and must
  end in the expected `tlb_shootdown_timeout` panic from CPU 1. Measured: 16/16 ACKed, 4 BSP ticks and
  43,808 co-runner iterations while parked; the control panicked after 500,002,889 cycles.

### CP-65 — Force-killing a suspended thread whose kernel stack owns `Arc`s (an inference waiter)

- **Shape.** Force-kill (`kill_thread_by_pid`) zombifies a Ready thread, and the reaper frees its kernel
  stack as raw bytes. Any Rust value living in that thread's frames is never dropped: an `Arc<Model>`
  (whose last clone keeps a whole model alive), a prompt `Vec`, tensor `Arc`s, a process `Arc` (CP-30).
  A thread is Ready mid-syscall whenever it yields or is preempted. B1c made this reachable:
  - the inference-stack *holder* is preempted on the BSP;
  - a *waiter* for the stack yields on every contended round, while its stack owns the request's
    resources.

  Making only the holder non-killable (B1c's first J1 fix) left the waiter exposed, and that was
  Jarvis's J3. A second shape, J4: a marker cleared before the syscall-boundary exit decision lets a
  remote terminate find the thread neither marked nor queued.
- **Guard (B1c).**
  - **One per-thread lifecycle marker** (`Thread::inference_lifecycle`) plus a per-process participant
    count. `syscall::inference_syscall` sets both at the arm's entry, before any allocation, model clone
    or `Vec`, and clears them only after the boundary exit decision.
  - **The marker covers the inference-stack arms only:** `SYS_GENERATE`, `_CONSTRAINED`, `_MASKED`, and
    `SYS_INFER_RUN`'s inventory route, marked once the route is known. `SYS_INFER_RUN`'s graph route
    is unmarked, as on `main` (re-check #2, J3-P).
  - **The kill path never zombifies a marked thread.** Terminate's accounting counts participants as
    handled.
  - **Cancellation is cooperative.**
    - A waiter checks the teardown in the `BUSY` retry loop and gives up (`try_run` gives `None`,
      `ECANCELED`) without running the forward.
    - A waiter that wins `BUSY` checks again before entering the closure. Otherwise a holder releasing
      between the teardown and the waiter's next CAS hands a dead process a whole forward (re-check
      #2, C1). Every forward closure also checks before its first forward.
    - The holder checks it before prefill and between tokens, and stops early.
    - Every local is dropped as the arm returns, and the thread exits at the boundary.
  - **Stack ownership is separate from the marker.** `BUSY`, the owner tid and the owner pid are
    separate atomics, published and cleared with interrupts masked. Only tests read the tid and pid; no
    safety decision depends on them.
  - **The general rule:** a thread that can be suspended inside the kernel while owning heap resources
    must either be unkillable there or be cancelled cooperatively. It must never be zombified from a
    queue.
- **Residual (pre-existing, not inference-specific).** After the marker is cleared, a thread on its
  syscall return path holds nothing, but a remote terminate can still find it Running and neither
  queued nor parked. Every syscall's return path has that window today.
- **Discriminator.** `inference_stack_kill`:
  - Holder A parks on the stack, on the BSP, where it can be preempted. Waiter B yields for the stack.
  - CPU 1 terminates B, then A, each while it sits Ready: the BSP holds its CPU with preemption
    disabled and interrupts on.
  - Both cancel and exit at the boundary, B never runs the forward, and the stack is released and then
    serves another call.
  - The model `Arc` count and frames return to baseline, and the heap does not grow.

  - **The release race:** waiter F is gated just before its CAS and terminated while waiting. Holder E
    then releases, and F wins `BUSY` and cancels without running a forward (`cancelled_after_win=1
    forwards=1`).

  `inference_stack_kill_control` builds the pre-J3 rule (skip only the holder). It leaks the waiter's
  model `Arc` (2 against a baseline of 1) and 5 frames.

### CP-66 — Force-killing a thread inside a cross-process admission queue: a stack leak, then a wedge

- **The trap.** A shared device is handed out by an admission queue: one holder runs, the others park with a
  wake token, and the holder's `complete` admits the next request and wakes it. A caller's kernel stack owns
  its request (here a graph `SYS_INFER_RUN`: an `Arc<Model>`, a `Vec<Arc<Tensor>>`, the handle vectors)
  while it is in that section. It can be found by a force-kill in two states:
  - **parked** in the token table (contended admission), where `zombify_parked_thread_by_pid` takes it;
  - **Ready**, as the holder, preempted inside a dispatch that runs with interrupts on (`inference-preemptible`,
    the default and product builds), where `kill_thread_by_pid` takes it.

  Either way the thread is zombified raw, so its frames never drop and the `Arc`s leak (CP-30). That much is
  the known class. The new part is what the **queue** does next:
  - a killed waiter's `PendingRequest` still carries its now-dead token, and the holder's `complete` admits it
    (`running = dead principal`), ignores the failed wake, and returns;
  - a killed holder never reaches `complete`, so `running` is never cleared.

  In both cases no thread will ever call `complete` for that principal, so **every later request defers behind
  a dead principal for the lifetime of the system**. The leak is bounded; the wedge is not.
- **Why it hid.** Every admission test parks and wakes cooperating threads; none kills one mid-section. The
  token generation check does its job (the wake returns `Gone`/`Stale`), but `complete` discarded the result —
  the ownership check fired and nobody listened (compare CP-36: a liveness token is not a lifetime token; here
  a failed wake is not a completion).
- **The rule.** **A queue that hands a shared resource from thread to thread must not let a force-kill remove
  a thread it still counts on.** Either the kill path withdraws the thread's place in the queue *and* lets the
  thread unwind its own frames, or the thread is unkillable there. Never zombify it from a queue.
- **Our guard (CP-66).**
  - `Thread::admission_section`, set by `admission::run_scheduled` around the whole section; `kill_thread_by_pid`
    and the token table's by-pid take skip such a thread.
  - `admission::cancel_pid(pid)`, called by `terminate` and `fault_kill_current_user` after `exiting` is
    published: under `QUEUES`, removes every pending request of `pid` and wakes it with `ECANCELED`. The
    woken caller unwinds (no work, no `complete` — it was never admitted), and `SYS_INFER_RUN`'s boundary
    exits it instead of returning to ring 3. A killed **holder** runs to completion (the admission policy) and
    calls `complete`, then exits the same way.
  - `submit_and_park` reads the caller's `exiting` **under `QUEUES`** (the `Arc<Process>` for it is dropped
    before parking — CP-30), so a request is either pending when `cancel_pid` scans or refuses to park.
  - `complete` treats a wake that finds no thread as "not admitted" and chooses again — defence in depth.
  - Lock order unchanged: `QUEUES → scheduler`, as `complete` already used.
  - **Composition with CP-65.** B1c's `Thread::inference_lifecycle` protects the generate / inventory
    arms (the shared inference stack); this marker protects the graph route's admission section. The scans
    use one predicate, `scheduler::kill_protected` — `(CANCEL_ON && admission_section) ||
    inference_lifecycle` — in **both** the ready-queue scan and the token-table take (the latter had
    honoured neither before). Each control restores only its own pre-fix rule. The graph route is not
    marked by B1c, so no request is handled by both boundaries.
- **Discriminator.** `admission_park_kill` (product configuration): a stand-in process's real graph
  `SYS_INFER_RUN` (the dispatcher's arm and boundary) parks behind a hold; `terminate` cancels it, it exits at
  the boundary, the pending queue is empty, `complete` leaves the device idle, a fresh process's inference is
  served, and the model and input `Arc` counts return to baseline. `admission_park_kill_threaded` adds the
  fault-kill sibling path and a holder that is Ready inside its dispatch. The control
  (`NEO_TEST_EXTRA_FEATURES=neo-test-admission-park-kill-control`) builds the pre-fix rule and must FAIL: the
  model `Arc` stays above baseline and the next inference is never admitted.
- **Related, not fixed here (same class, found in the audit).**
  - **Channel park:** `sys_send`/`sys_recv` keep an owned `Arc<Channel>` in the frame across the park; a
    terminate that extracts and zombifies the waiter leaks one `Channel` per kill (no Pmap stranded, no wedge).
    **Fixed since as CP-67** (the CP-36 scheduler-owned anchor; the grant arm's source handle too).
  - **`ai::reservation::guarded_clone`:** token-parks a user thread while a mover holds a tensor reservation;
    the caller may own `Arc`s. A leak, no wedge (a stale waiter token is ignored by design). **Assessed since:
    unreachable outside test builds** (no production code starts the mover);
    the guard for the build that ships one is in `Plans/NOTE-2026-10-20-guarded-clone-kill-s2.md`.
  - **Product-build `sys_wait`:** `terminate` does not extract a wait-parked thread at all (a containment gap,
    CP-36's known one, not a leak).
- **Source.** Ours: found by a cross-family (Codex) adversarial read of B1c (Jarvis, J3-P). The general shape
  — a scheduler that hands a resource from waiter to waiter must survive a waiter disappearing — is the
  admission analogue of a mutex whose owner dies (robust futexes' `OWNER_DIED`); re-derived here, not copied.

### CP-67 — An `Arc` owned by a syscall frame across an IPC park (a force-killed waiter leaks it)

- **Shape.** A blocking channel syscall (`sys_send`, `sys_recv`, `sys_grant`, `sys_recv_grant`) looks up its
  channel as an owned `Arc<Channel>` and keeps it in the frame while it parks in the endpoint. Terminate and
  fault-kill extract a parked waiter (`extract_parked_if_owned`) and zombify it, and the reaper frees the
  stack as raw bytes, so that count is never dropped: one `Channel` leaks per kill. `sys_grant` also kept an
  owned clone of its **source handle** across the park, and a model handle carries an `Arc<Model>`. There is
  no wedge: the peers are closed with `EPIPE`, and each waiter's `Arc<Pmap>` lives in the endpoint state,
  which is dropped on extract. The leak is product-reachable: any terminate of a process blocked in IPC.
- **Why the obvious fix does not work.** Dropping the `Arc` before parking is not possible: the channel
  guard borrows it until the switch shim releases the lock after the old thread is parked.
- **Guard (CP-36's pattern).**
  - `sched::with_channel_anchor(channel, op)` moves the channel into `Thread::ipc_anchor` for the call and
    lends `op` a reference into it. A thread that returns (rendezvous, `EPIPE`, or one that never parked) takes it back
    and drops it outside the scheduler lock. A reaped one drops it in `Thread::free`.
  - The slot is written only by its own thread and by `free`. It stays put while the thread parks and
    migrates, because the `Thread` is boxed.
  - While a thread is parked, `Channel → state → Box<Thread> → Arc<Channel>` is a cycle. It lasts exactly
    as long as the old frame's count did, and extract or wake breaks it.
  - `sys_grant` drops its source handle as soon as it has the `cap_id` and rights, before it can park.
  - `SYS_WAIT_ANY` was already clean: its frame holds only keys and fixed arrays across the park (CP-30
    by design). `try_recv` never parks.
- **Discriminator.** `ipc_park_kill`: stand-in processes enter each syscall through the real
  `syscall_dispatch`.
  - A rendezvous leg (a recv parked, then a send) returns 1/1 with the channel back at the harness's count,
    so the take-back path drops the anchor exactly once.
  - Four kill legs park in `SYS_RECV`, `SYS_SEND`, `SYS_GRANT` (with a model source handle) and
    `SYS_RECV_GRANT`, each on a channel with no peer, and terminate the process. The channel goes from 3
    while parked to 1 after the reap, and the model count returns to baseline.
  - The control (`neo-test-ipc-park-kill-control`) restores both frame-owned shapes. Each leg ends at 2 and
    the grant leg leaks a model count, so it must FAIL with `ipc_park_kill_arc_leaked`.
- **The general rule (CP-30 restated for borrowed-across-park values).** If a park cannot run with nothing
  owned on the stack (a lock guard borrows the value), move ownership into the `Thread` for the call,
  rather than leaving it on a stack the reaper frees raw.

### CP-68 — Assuming xAPIC on silicon whose firmware keeps x2APIC on

- **Shape.** Every piece of the local-APIC path assumed xAPIC:
  - the Limine MP request carried no x2APIC flag;
  - `init_local_apic` cleared `X2APIC_ENABLE`;
  - every register was an MMIO access through the `IA32_APIC_BASE` window;
  - the IPI destination was 8 bits in ICR-high;
  - every MSI address was the constant `0xFEE0_0000`, which targets APIC ID 0.

  On firmware that enables or locks x2APIC (common on modern laptops), Limine must disable x2APIC for such
  a kernel, and when it cannot it refuses to boot: `smp: Kernel does not support x2APIC, but machine
  requires it`. Even past the bootloader, each assumption fails on its own:
  - leaving x2APIC directly for xAPIC is #GP;
  - the MMIO window is not decoded in x2APIC mode, so an EOI or IPI written there vanishes silently;
  - an x2APIC ID above 255 cannot be named in 8 bits.
- **Found by.** The PL booting the B1 product ISO on a laptop; Limine panicked before the kernel ran.
  QEMU could not have shown it: its TCG cannot model firmware-locked x2APIC, and QEMU 8.2's TCG has no
  x2APIC at all.
- **Guard.**
  - The kernel requests x2APIC (`MP_FLAG_X2APIC`). The mode is resolved from the BSP's own
    `IA32_APIC_BASE`, with Limine's report cross-checked and a mismatch logged, and announced as
    `apic: mode=`.
  - In x2APIC mode:
    - registers are MSR `0x800 + (offset >> 4)`;
    - the ICR is one 64-bit write with the 32-bit destination in bits 63:32, preceded by `mfence;
      lfence`, because WRMSR to the x2APIC ICR is not serializing (the CP-9 class, newly reachable);
    - `IA32_APIC_BASE` is never rewritten, and no MMIO page is mapped;
    - a CPU not in the BSP's mode halts with a named line, in BOTH directions, checked before any
      `IA32_APIC_BASE` write (`apic_logic::ap_mode_check`): under an xAPIC BSP a CPU found in x2APIC
      mode is not moved, because x2APIC to xAPIC directly is #GP (Jarvis's cross-check, P2).
  - The xAPIC path is unchanged, except that an APIC ID above 255 is refused rather than masked to 8
    bits: such a CPU is never started (`smp: skipping ... not addressable`), and the ICR helper
    returns nothing for it.
  - The MSI and MSI-X addresses target the BSP's registered APIC ID. They are refused, with the device
    left without MSI, when that ID exceeds 255 with interrupt remapping off; a truncated destination
    would alias another CPU.
  - The kill-switch is `xapic-only`: the pre-CP-68 request, with today's path wherever firmware allows
    xAPIC.
- **Discriminator.**
  - `x2apic_smp_ipi` boots `-cpu max,+x2apic` (CI's QEMU 10) and asserts:
    - `mode=x2APIC`;
    - 16 rounds of reschedule IPIs to every AP, each confirmed by that CPU's receive counter;
    - 16 broadcast TLB shootdowns with every target ACKed and no timeout;
    - the BSP-0 MSI address `0xfee00000`.
  - Control: `NEO_TEST_EXTRA_FEATURES=xapic-only` comes up in xAPIC on the same CPU model and fails
    `apic_mode_not_x2apic`.
  - `tools/apic-logic-tests` pins the MSR map, both ICR encodings, the MSI address and the >255 refusal to
    hand-written SDM values.
- **The general rule.** An interface the platform can switch is the platform's to choose. Drive the mode
  the hardware reports, never the one the code was written for, and make each mode-specific access route
  on that resolved state rather than on an assumption made at build time.
- **Source.** Ours (real silicon). The register map, ICR format, transition rules and the WRMSR
  serialization note are from the Intel SDM Vol. 3A ch. 11; the request and response flags are from Limine's
  `PROTOCOL.md`. Read for principle; no code copied.

### CP-69 — Trusting a kernel tick that never arrives

- **Shape.** The kernel's only tick was the 8254 PIT through 8259 IRQ0, delivered to the BSP, and nothing
  checked that it arrived. Newer Intel client platforms can clock-gate the 8254 (a firmware option, on for
  S0ix) or leave no working 8259 → LINT0 virtual-wire route. Then `TICKS` never moves: nothing preempts,
  every sleep or timed wait hangs, and the boot looks frozen right after `sched: preemption ready`. The same
  trap returns one level up: a LAPIC timer mode can be armed correctly on paper and still never fire, and
  the mode most silicon will use, TSC-deadline, **cannot run under QEMU's TCG** (8.2 and 10.2.3 both refuse
  `+tsc-deadline`). So the gate can never prove it.
- **Found by.** Trap-mining for the Zenbook S14 (Panther Lake) target. It is predicted, not yet seen.
- **Guard.**
  - `lapic-timer` drives the BSP's tick from the local APIC timer on the same vector, 32, in xAPIC and
    x2APIC mode.
  - The TSC and LAPIC rates come from CPUID 0x15 (exact) or are measured against an **independent
    clock**: the HPET (ACPI `HPET` table) or the ACPI PM timer (FADT). PIT channel 2 (times out if gated)
    **only cross-checks** another source and is never the sole source of a rate (round 4: a poller held
    off across terminal count reads a 20 ms window as 10 ms, a 2x rate that no local check can see). With
    only the PIT, nothing is calibrated and the verified PIT/PIC tick runs instead (`lapic=pit-only-clock`).
    Each is cross-checked against a second independent source, and a disagreement beyond 5%
    disqualifies the mode that depends on that rate. CPUID 0x16, the base frequency, is only a
    last-resort estimate, logged as `rate=estimated(cpuid16)` (Jarvis's review: it is not a TSC rate).
    **An estimate is never used** (round 3): it sets no TSC-deadline period, does not calibrate the
    LAPIC, and sizes no wait. With no CPUID 0x15 and no independent clock, the LAPIC tick is refused and
    the PIT fallback is tried.
  - The mode is TSC-deadline when CPUID.01H:ECX[24] is set, else periodic.
  - Using the reference clocks safely (Jarvis's re-check, R2–R4):
    - **The HPET is taken over before it is enabled.** The counter is stopped, legacy replacement is cleared
      (giving IRQ0 back to the 8254, which the fallback needs), every comparator's interrupt is disabled, and
      latched status is cleared. Only then does the counter start. Enabling it as firmware left it lets any
      comparator firmware armed interrupt.
    - **A hardware-reduced FADT** (flags bit 20) has no PM timer, whatever its PM timer fields hold.
    - **A PIT channel-2 window needs both edges of OUT2**: LOW after programming, then HIGH at terminal count.
      Each edge is timed on the read that observes it, so a level first seen after its deadline is late.
      A window under half its programmed length, bounded with a 500 MHz TSC floor, is refused, and so
      is one where two polls were further apart than a tenth of the window (the poller was held off; defence
      in depth, the PIT-never-alone rule above is the fix). A port reading stuck HIGH (0xff from an absent
      or gated PIT) is not a finished window.
    - The HPET's status is written back only for **level-triggered** comparators; an edge-triggered
      comparator's bit must be written 0 (HPET 1.0a §2.3.6).
  - **Every armed mode must deliver its first tick within five periods**, or it is disarmed (LVT masked,
    count and deadline zeroed) and demoted: TSC-deadline, then periodic, then the PIT. **These waits, and
    the PIT fallback's, are sized from a calibrated TSC rate, else from a 10 GHz bound, never from a
    refused or estimated rate** (R1). The calibration measurements are timed by the reference clock
    itself; the TSC there only bounds a timeout. A wait that is too long only delays a demotion, but one that is too
    short disarms a working tick: CPUID 0x15 claiming 24 MHz for a 2.4 GHz TSC gave a 0.5 ms wait for a
    10 ms tick.
  - The timer handler picks its EOI from the **LAPIC's in-service bit for vector 32**, not from the
    `active` flag. A LAPIC tick that lands just after `disarm` still EOIs the LAPIC; otherwise its ISR bit
    would stay set and block every vector of its priority class. A PIT tick arrives as ExtINT, sets no ISR
    bit, and EOIs the PIC.
  - **The PIT/PIC fallback is itself verified** within about half a second. With no tick source at all,
    the BSP prints `timer: NO TICK SOURCE (lapic=<reason>, pit=gated)` on screen and serial and halts:
    a photo names the failure instead of showing a frozen boot.
  - The kill-switch is the feature off: the PIT/PIC tick, unchanged.
- **Discriminator.**
  - `lapic_timer_tick` (xAPIC, periodic on TCG) and `lapic_timer_tick_x2apic` (QEMU ≥ 9) assert:
    - the LAPIC is the tick, and PIC IRQ0 is still masked;
    - the tick rate, counted against the HPET over 0.5 s, is 100 Hz within 10%;
    - the x2APIC variant really runs in x2APIC mode.
  - `lapic_timer_demote` arms its first mode masked, a silent timer. It asserts:
    - the mode was demoted to the PIT;
    - IRQ0 is unmasked;
    - ticks advance.
  - Controls:
    - `neo-test-lapic-timer-control` forces "not calibrated", and the run fails `timer_not_lapic`;
    - `neo-test-lapic-timer-no-verify` trusts the silent mode and fails `no_tick_after_unverified_mode`:
      the hang the first-tick check exists to prevent, named;
    - `+ neo-test-lapic-timer-pit-gated` (with the control) leaves the PIT's IRQ0 masked and fails
      `no_tick_source` after printing the NO TICK SOURCE line.
  - Host KATs (`tools/acpi-timer-logic-tests`, `tools/lapic-timer-logic-tests`) for R1–R4, each from Jarvis's
    reproduction, and each failing with its fix removed:
    - a refused 24 MHz TSC rate sizes no wait;
    - HPET register images: config 0x2 with comparator 0 armed, and already running with legacy routing;
      no write turns the counter on while legacy routing or a comparator interrupt is still set;
    - a 276-byte, rev-6, hardware-reduced FADT, in both PM timer forms;
    - port 0x61 traces: constant 0xff and constant 0x00 are both refused, as are a late LOW, a late HIGH
      and a too-fast HIGH (round 3), and a poll held off across the edge (round 4);
    - the PIT alone is `PitOnly`, never `Calibrated`, and plans the PIT tick; as a checker, a 2x PIT sample
      disputes a good HPET or CPUID 0x15 rate (round 4);
    - an edge-mode comparator's status bit is never written 1.
  - On silicon, the `timer: LAPIC tick mode=... first_tick=Ncyc` line (or its demotion line) is on screen
    through `fb-progress-console`.
- **The general rule.** A device the code cannot observe working has not been shown to work. Prove an
  interrupt source by its first delivery, within a bounded wait, and keep a known-good fallback behind it;
  that matters most for a path the test bench cannot emulate.
- **Source.** Ours (trap-mining). The timer modes, LVT encoding and ISR/EOI rules are from the Intel SDM
  Vol. 3A ch. 11; the TSC leaves are from Vol. 3B ch. 18 and the CPUID reference; the HPET takeover from the
  IA-PC HPET spec 1.0a §2.3.5 and §2.4.2; the hardware-reduced rule from ACPI 6.5 §5.2.9. Linux's PIT-optional
  timer setup was read for its decision order only. No code copied.

### CP-70 — A ring-3-reachable exception vector that halts the machine instead of killing the process

- **Shape.** Every CPU exception handler did `dump(); halt_forever()` regardless of where the exception was
  raised. Only the ring-3 `#PF` was contained (OT-61 C1.1), and only under `browser-runtime`. So any user
  program could halt the whole machine:
  - `ud2` (this is also stock Rust std's `abort`, which is how OT-61 C0.5 found it);
  - an integer divide by zero;
  - a privileged instruction (`hlt` at CPL 3 is `#GP(0)`);
  - setting RFLAGS.TF (`#DB`);
  - an unmasked x87 or SSE floating-point error.
- **Why it is a kill-path pitfall, not only a containment gap.** The fix is a forced termination entered from
  exception context, on any CPU, at IF=0, with the user GS live. Each of those has bitten a kill path here
  before:
  - **CP-44:** a per-CPU read before `swapgs`;
  - **CP-46:** Rust called on a misaligned stack from an error-code vector;
  - **CP-41:** the faulting thread's MXCSR or x87 control word following the kernel;
  - **CP-30, CP-66:** owned frames and queue places of killed threads.
- **Guard (OT-83).**
  - **One routing decision,** `user_fault_logic::route(vector, saved CS)`, pure and host-tested. CPL 3 kills,
    CPL 0 halts exactly as before.
  - **`contain_user_fault` is the first call in every routed handler.** Only test hooks that diverge run
    before it (CP-47/48's `syscall_msr_trap`). It does `swapgs`, then `load_kernel_fp_control`, then
    `fault_kill_current_user_exception`.
  - **The kill is the C1.1 path, generalized and unconditional.** It records 128 + signal, publishes
    `exiting`, cancels admission waiters (CP-66), yanks Ready and token-parked siblings, then
    `scheduler::exit()`. The heavy teardown runs on the last reap in the reaper's context.
  - **Without `browser-runtime` a process has one thread,** so the sibling drain is moot. The faulting
    thread was in ring 3, so it holds no kernel lock and is in no kill-protected section.
  - **`CR0.NE = 1` on every CPU.** With NE left to the bootloader, a ring-3 x87 error took the legacy
    FERR#/IRQ13 path, which nothing services, and was silently lost (observed under QEMU).
- **Discriminator.**
  - `ring3_fault_{ud,de,gp,db,mf}`: the probe dies with its exit code, and a second process then runs and
    exits 0.
  - `ring0_fault_halts`: a ring-0 `#UD` reaches `halt_kernel_fault` and never the kill path.
  - `tools/xtask/src/ot83_tests.rs`: the vector→code table against hand-written Linux values, and the CPL
    decision for every selector value.
- **The general rule.** Every exception handler reachable from ring 3 is a privilege-boundary entry point.
  Its first act is to decide, from the saved CS, whose problem the fault is. A user's problem never costs
  the machine, and a kernel problem is never quietly contained.
- **Source.** Ours (OT-83). Linux's `arch/x86/kernel/traps.c` sends the analogous signals (`do_trap` →
  `force_sig`). Read for principle; no code copied.

### CP-71 — A log flush that waits on the sink lock at IF=0, or under another lock

- **Shape.** `log::block_sink` guards its staging buffer with `SINK`, a plain `spin::Mutex`. Every
  `serial_println!` takes it at IF=1 (after the UART's own IRQ-off section), so a printer can be preempted
  while holding it. CP-37's fix made `write_fmt` drop a contended line instead of waiting. `log::flush()` was
  left with a blocking `SINK.lock()`. Any flush reached at IF=0 can therefore spin on a holder that is
  suspended on the same CPU and can never run again there.
- **Where it bit.** OT-83 routed every ring-3 exception into `fault_kill_current_user_exception` (IF=0).
  Its CP-66 `admission::cancel_pid` withdraws a parked sibling's request with `QUEUES` held and printed a
  `WITHDRAWN` trace that called `log::flush()`. With a preempted printer holding `SINK`, the faulting CPU
  wedged, and any peer shootdown targeting it would time out (CP-64). Found by the cross-family review
  (Jarvis), from source; reproduced by the discriminator's control.
- **Guard (OT-83).**
  - **A per-CPU stage-only section** (`log::enter_stage_only` / `leave_stage_only`, nestable). A line logged
    on that CPU inside it is staged under `try_lock` of both sink locks, or dropped and counted. A full
    staging buffer drops the line instead of flushing. No wait, no block I/O. The exception kill runs inside
    one from its first print through the sibling scans. The section ends just before `scheduler::exit()`;
    that suffix was inspected (Jarvis re-check) and logs nothing that waits.
  - **Admission traces are stage-only on every path,** so nothing flushes while `QUEUES` is held.
  - **The durable write moves to normal context:** `log::flush_pending()` (try-lock only, on both sink
    locks; it leaves the flag set if either is held), called after `QUEUES` drops and from the reaper.
- **Discriminator.** `admission_park_kill_sink_held`: a kernel thread holds `SINK` across yields on CPU 0.
  A sibling of a process whose caller is parked in admission then fault-kills it on CPU 0 at IF=0. The kill
  must progress, and a full shootdown must be ACKed while `SINK` is still held. The control restores the
  trace's blocking flush and hangs (QEMU timeout; the serial log stops at the `WITHDRAWN` line).
- **What stays exposed.** Outside a stage-only section, `write_fmt` still flushes inline when its 4 KiB
  staging fills, and `log::flush()` still waits. Both can run at IF=0 from other paths (`sys_write`, drivers
  that flush after a trace). Neither is on the exception kill. A new IF=0 path that logs must either enter a
  stage-only section or prove no `SINK` holder can be suspended on its CPU.
- **The general rule.** A lock taken at IF=1 is never waited for at IF=0 on the same CPU. Logging from a
  context that cannot be preempted stages or drops; the write happens later, in a context that can wait.
- **Source.** Ours (OT-83 F1). Same shape as CP-37 (the sink's `write_fmt`, `threads_futex`).

### CP-72 — An edge-triggered line left asserted when the ISR returns

- **Shape.** An edge-triggered interrupt fires once per low→high transition. If the ISR returns while the
  device still asserts its condition, the line never goes low, so no further edge arrives. Two ways to get
  there:
  - the ISR stops early, with data still pending;
  - the condition was already true when the interrupt was unmasked.

  The device then goes silent. Nothing faults, nothing logs: the symptom is a peripheral that "just stops".
- **Where it bit.** The i8042 ISR read at most 16 bytes per pass. QEMU's PS/2 model queues up to 16 bytes
  and keeps IRQ1 asserted while the output buffer is full. A burst of keys filled the queue, the ISR left a
  byte behind, and the keyboard went silent for good. The scenario hung 3 runs in 6 at `keys=10 irqs=2
  bytes=16`. On hardware the 8042 holds one byte, but the same trap opens at enable time, with a byte already
  waiting.
- **Guard (i8042 lane).**
  - The ISR drains until the condition is clear. Its cap (256) only bounds a stuck device, and hitting it is
    counted (`capped=`).
  - One drain runs right after the interrupt is enabled.
  - The pump has a polled fallback, counted separately (`polled=`), so a lost or dead interrupt shows up as a
    number in the status line, not as silence.
- **Discriminator.** `ps2_keyboard`: 14 exact events, and every byte must arrive by IRQ (`polled=0`,
  `irq_bytes == bytes`). It passes 8/8 after the fix, and the capped version hangs.
- **The general rule.** For an edge-triggered source, an ISR's exit condition is "the device no longer
  asserts", never "I handled N items". A fixed per-pass budget needs a second path (a level re-check, a poll)
  that cannot also miss the condition.
- **Source.** Ours (i8042 lane). The principle is in every PC interrupt-controller text: an edge latch is
  re-armed only by a transition.

### CP-73 — Back-to-back IF=0 sections leave no window for interrupts

- **Shape.** `without_interrupts(f)` restores IF with `sti` at its end. After `sti`, the CPU recognises
  external interrupts only after the next instruction (the STI interrupt shadow, Intel SDM Vol. 2B `STI`;
  CP-22 relies on the same property for `sti; hlt`). A loop whose body is two IF=0 sections back to back,
  such as a lock-protected drain followed by `yield_now`, can end one section's `sti` directly before the next
  section's `cli`. The IF=1 window is then empty, and pending interrupts wait for as long as the loop spins.
- **Where it bit.** The `ps2_keyboard` scenario polled with `drain_events(); yield_now();`. On QEMU 8.2.2
  about half the runs starved:
  - IRQ1 was taken only a few times, so QEMU's 16-byte PS/2 queue overflowed and dropped keys;
  - the tick froze, so the scenario's tick-based deadline never fired, and the run hit QEMU's 180 s
    budget.
  - The same loop passed before an unrelated rebase moved the code around. Timing hid it.
- **Guard.** That loop now spins with interrupts on between passes, and its deadline is also bounded by the
  TSC.
- **What stays exposed.** Any other loop of back-to-back IF=0 sections. The production keyboard pumps (USB
  and PS/2) run once per syscall or wait-any pass, not in a tight loop, so they are not.
- **Discriminator.** `ps2_keyboard` requires all 14 key events by IRQ (`polled=0`). With the window it
  passes 18/18; the earlier loop failed about half the runs on QEMU 8.2.2.
- **The general rule.** A spin-wait that expects interrupts to make progress needs at least one instruction
  executed at IF=1 per iteration. Better still, it waits with `sti; hlt` (CP-22) when it can.
- **Source.** Ours (i8042 lane). The STI shadow is architectural (Intel SDM).


### CP-74 — A long kernel computation in a syscall at IF=0, on an unguarded 32 KiB stack

- **Shape.** Syscalls enter at IF=0 (SFMASK), and nothing re-enables interrupts unless the path does so
  itself. `sys_model_load` of a boot model did all of this inside the syscall:
  - BLAKE3 over the whole 485 MB module;
  - the model build, which hashes every weight again for deduplication and copies it into tensor storage.

  It ran on the caller's kernel stack: a 32 KiB `Box<[u128]>` in the heap, with no guard page.
- **Two failures from one call.**
  - **The freeze.** At IF=0, CPU 0 takes no LAPIC tick and no xHCI MSI for the whole load. Every user
    process runs on CPU 0, so the WM stops and the pointer stops.
  - **The silent overrun.** `blake3::hash` recurses about log₂(input ÷ 1 KiB) levels deep, and at the kernel's
    opt-level 0 each level is large. Measured on the guarded inference stack (`boot_model_load_stack`,
    one `blake3::hash` over an owned heap buffer of each length):

    | Input | Depth, opt-level 0 | Depth, `blake3` at opt-level 3 |
    |---|---|---|
    | 4 KB (the hermetic fixture) | 8,368 B | 4,800 B |
    | 382 KB (the KAT model) | 18,560 B | 13,440 B |
    | 8 MiB | 28,144 B | 21,168 B |
    | 64 MiB | 33,840 B | 25,520 B |
    | 485 MB (the real 0.5B) | 39,536 B | 29,808 B |

    The depth is a watermark (the deepest byte written), not the whole syscall's stack use: over 32 KiB
    proves the box too small, under 32 KiB does not prove the syscall fits.

    At the real size the hash alone overruns the 32 KiB box, before the syscall's own frames. The overrun
    writes into whatever heap memory lies below the stack. Every QEMU scenario uses a model under 400 KB,
    so none could see it.
- **Where it bit.** On the Zenbook S14 (Panther Lake, 16 CPUs), the freeze-watchdog overlay read
  `cpu0 ticks/s=0 | model load-build weight 0/0` with Arthur 29 s inside syscall `0x110`: the hash had
  returned, and the build never reached its first weight. Once, the freeze lasted more than 15 minutes.
  Heap corruption that hangs the next allocation at IF=0 fits that; the OptiPlex's different heap layout
  would explain why it survived.
- **Guard (`boot-model-preload`, in the product image).**
  - The boot model is verified and built once at boot, before any user process exists
    (`boot_modules::preload`).
  - The build runs on the inference stack, which is 256 KiB with a guard page and runs its closure at IF=1
    (CP-64).
  - `sys_model_load` hands every Arthur the same `Arc<Model>`. On `main`, a second Arthur got `ENOENT`,
    because the first had consumed the module.
  - A model whose pinned vocab fails is left untaken, as Arthur's `compose` leaves it (B1c F8).
- **Lock safety at IF=1.** The build takes `ARENA`, `DEDUP` and `REGISTRY`, which are plain spin locks
  (`SwitchMutex`). A holder preempted at IF=1 deadlocks only a waiter that spins at IF=0 on the same CPU.
  No thread takes those locks before userland exists, and the frame allocator and the heap mask
  interrupts themselves.
- **What stays exposed.** Without the feature, the in-syscall load remains (every QEMU scenario's small
  model fits). Any other long syscall at IF=0 on a thread stack has the same shape.
- **Discriminators.**
  - `boot_model_load_ticks`: the load is held between its hash and its build. CPU 0 takes 20 ticks and a
    CPU-0 co-runner runs meanwhile; a second lookup returns the same model.
  - Its control, `boot_model_load_ticks_control`: the old load inside an IF=0 section. It shows 0 ticks and
    0 co-runner runs.
  - `boot_model_load_stack` measures BLAKE3's depth up to the real size.
- **The general rules.**
  - A kernel computation proportional to user-supplied or module-sized input does not run at IF=0.
  - It does not run on a 32 KiB thread stack without measuring its depth at the real size.
- **Source.** Ours (Zenbook ask-freeze; S2's diagnosis `Plans/NOTE-2026-10-22-zenbook-ask-freeze.md`).

### CP-75 — A level-triggered line EOI'd or unmasked while the device still asserts it

- **Shape.** A level-triggered I/O APIC entry delivers while its input is asserted and its remote IRR is
  clear. The local APIC's EOI clears remote IRR, so if the ISR EOIs with the entry unmasked and the device
  still asserting, the interrupt is taken again as soon as the ISR returns. A device whose condition is
  cleared only by a slow bus transaction (an I2C read of a HID report) cannot be serviced inside the ISR. The
  CPU then spends all its time entering and leaving the handler. A line with the wrong polarity looks like
  this from the first unmask: idle reads as asserted.
- **Where it applies.** The Zenbook touchpad: GSI 0x69, level, active-low (`_CRS` `Interrupt`). The device
  deasserts only after the host reads its input register over I2C. That is about 250 µs at 400 kHz, far too
  slow for an ISR, and the read needs the bus, which an ISR must not own.
- **Guard (I2C-HID touchpad lane).**
  - The ISR masks the entry, sets `PENDING` and EOIs. That is all it does.
  - The service thread reads one report, then unmasks. A line still asserted fires again at once; it is
    level-triggered, so there is no lost wake.
  - `LineGovernor`: 16 assertions in a row with no data leave the line masked and switch to polling
    (`irq masked-storm`). 8 failed transfers stop the reader.
  - Before the reset, while masked, stale input is drained, so the reset's interrupt is not confused with a
    left-over report.
- **Discriminator.** Host KAT `line_governor_masks_a_storm_and_stops_on_errors`. On the Zenbook, the console's
  `count=`/`reads=`/`empty=` and `mode=` say which path is live.
- **What stays exposed.** EOI broadcast suppression (LAPIC SVR bit 12, preserved from firmware): remote IRR is
  never cleared by the local EOI. The line then delivers once and never again, the opposite symptom. It is
  printed (`eoi-suppress=`) but not mitigated; that would need the I/O APIC's directed EOI.
- **The general rule.** A level-triggered ISR's contract is "the line is masked or the device has
  deasserted before I EOI". Deferred service masks first and unmasks only after the device has been read.
- **Source.** Ours (I2C-HID lane). The principle is the "oneshot" threaded interrupt of every OS that drives
  slow-bus devices (Linux `IRQF_ONESHOT`, read for principle).

### CP-76 — Two writers interleaving on the I/O APIC's index/data pair

- **Shape.** The I/O APIC is programmed by writing a register index to `IOREGSEL` (base + 0x00), then the
  value to `IOWIN` (base + 0x10). Two contexts that each do that pair can interleave: A selects, B selects
  and writes, then A writes into B's register. The redirection entry ends up half-programmed, unmasked
  when it should be masked, or with the wrong vector. Nothing faults.
- **Where it applies.** Before the touchpad lane the I/O APIC was written only at boot (the i8042's route).
  The touchpad masks its entry in the ISR and unmasks it from the service thread, at run time.
- **Guard (I2C-HID touchpad lane).** A single writer per phase:
  - the ISR writes only while the pin is unmasked, which is the only time it can fire;
  - the thread writes only while the pin is masked, with interrupts off on its own CPU, so the ISR (BSP-only)
    cannot run between its two writes there;
  - every runtime write selects the same index, so an interleave would at worst repeat a value.
  - The diagnostic read-back runs only while the pin is masked.
- **What stays exposed.** A future second runtime user of the I/O APIC (another level device) needs a lock
  that is safe from interrupt context, or per-entry discipline of the same kind.
- **Discriminator.** Review item. No QEMU device asserts the Zenbook's GSI.
- **Source.** Ours (I2C-HID lane). The register pair is architectural (Intel 82093AA §3.1).


### CP-77 — Granting a DMA master before its rings, or trusting firmware's Bus Master Enable

- **Shape.** Two orderings around one bit, PCI COMMAND bit 2 (Bus Master Enable):
  - A NIC whose ring-base registers still hold reset or firmware values starts fetching descriptors the
    moment it is enabled. If bus mastering is already on, it DMAs from a garbage base before the driver
    has programmed one.
  - A UEFI network stack (PXE, HTTP boot) can leave Bus Master Enable set when it exits. A driver that
    only *sets* the bit when containment allows, and never *clears* it, inherits a DMA-capable device it
    decided not to trust.
- **Where it applies.** The RTL8125 (rung 1), on the PL's AMD main PC, where there is no IOMMU driver and
  the default is "no bus mastering at all".
- **Guard (RTL8125).** The bind writes COMMAND with Bus Master Enable **cleared** (and INTx disabled, I/O
  decode off) before the chip is touched (`rtl8125_logic::bind_command`). Bus mastering is granted in one
  function, `grant_bus_master`, only for a mode the containment table allows, and only after both ring bases
  are programmed with fenced arena addresses.
- **Discriminator.** `rtl8125_absent`: a stand-in NIC with Bus Master Enable pre-set comes out of the bind
  with it clear, on hardware state read back from config space. The control (the opt-in forced on) FAILs
  `bus_master_set`. `rtl8125_vtd`: the bit is clear after bind and set only by the grant.
- **The general rule.** Read Bus Master Enable as firmware's leftover, never as zero. Clear it at bind and
  set it last, after every address the device can fetch is valid.
- **Source.** Dispatch trap 4 (`Plans/DISPATCH-2026-10-23-rtl8125-ethernet.md`). The firmware-leftover half
  is ours. The ordering principle is the same one every r8169-family driver follows.

### CP-78 — Descriptor OWN bit: status before contents, contents before OWN, OWN before doorbell

- **Shape.** A device-shared descriptor ring has three orderings, and each breaks in its own way:
  1. **RX consume.** The driver reads the OWN bit, then the length and the buffer. If the buffer or length
     read can pass the OWN read, it reads a frame the device has not finished writing.
  2. **Publish.** The driver writes the buffer address and length, then the OWN bit. If OWN becomes visible
     first, the device fetches a stale address.
  3. **Doorbell.** The TX doorbell is an MMIO write. If it can pass the descriptor stores, the device polls
     the ring before the new descriptor is there and misses it until the next kick.
- **Guard (RTL8125).**
  - `RxRing::reap` calls an `acquire()` hook (`fence(Acquire)`) after each OWN-clear opts1 read and before
    the event that hands the buffer out.
  - `RingMem::publish` writes the address and opts2, then `fence(Release)`, then opts1 (which carries OWN).
  - `tx_submit` issues `fence(SeqCst)` between the post and the `TX_POLL` doorbell.
  - x86 TSO already orders the second and third. The fences are for the compiler and for a future
    weakly ordered target, and they are counted (`barriers=pub:/acq:/bell:` on the console line).
- **Discriminator.** Host test `publish_writes_own_last` (the opts1 store follows the address store for
  every slot), and `rx_ring_*` / `tx_completion_out_of_order_is_held` against a register model. On silicon,
  the `link=up rx=0` and `tx=wedged` rows of the failure bank.
- **A fourth trap in the same ring: the length is device-written metadata.** The OWN/acquire discipline makes
  the length field *current*, not *sane*. A completion claiming more bytes than the armed buffer holds (the
  14-bit field allows 16383; Janus's review inputs were `0x30003fff` and wire length 2053 against a 2048-byte
  buffer) became an out-of-bounds slice and a kernel panic in the first cut. The DMA fence bounds where the
  chip writes, not what it reports. Guard: `rx_classify_bounded` rejects a wire length over the armed
  capacity or a frame shorter than an Ethernet header, before any `RxFrame` exists (`badlen=` on the line),
  and `rx_take` re-checks `len <= buffer.len()`. Discriminator: `rx_ring_never_hands_out_an_oversized_frame`
  (fails on the unbounded classifier).
- **The general rule.** One acquire per consumed descriptor, one release per published one, and a full
  barrier before any MMIO doorbell that tells the device to look. Validate every device-written field
  before using it as a size or an index.
- **Source.** Dispatch traps 5 and 6. Linux `r8169` (`dma_rmb()` after the DescOwn read; the doorbell
  after the descriptor writes) and OpenBSD `rge(4)`, read for principle only.

### CP-79 — A per-CPU capability decided on one CPU and used on all

- **Shape.** Whether AVX2 may run depends on CPUID (the feature bits) and on XCR0 (whether the OS enabled YMM
  state). XCR0 is per-CPU: the BSP and each AP write it in their own `enable_sse` → `finish_xsave_init`. The
  first cut of the Arthur latency fix 2 cached the BSP's answer in one global and let every CPU's matmul
  partition read it. An AP whose CPUID lacks XSAVE (it never writes XCR0), or an FXSAVE build where the BSP's
  XCR0 is 7 and an AP's is 3, would then run AVX2 instructions its own check rejects: `#UD`, or YMM state
  that the context switch does not save.
- **Where it bit.** Nowhere on hardware: homogeneous x86 parts report the same bits on every core. Found by
  the cross-family review (Janus, relay #44, items 8 and 10), from source.
- **Guard.** `ops::qk_matmul::AVX2_CENSUS`, one `AtomicU64` changed only by `fetch_update` (AcqRel): a
  decision (undecided / no / yes), a veto bit and a reported-CPU count.
  - Each CPU reports its own `local_avx2_check()` at the end of its `enable_sse`, after its XCR0 write and
    before it can poll for matmul work (the CP-19 ordering).
  - A "no" sets the veto and the decision "no" at once. "No" is always safe: the scalar kernels are the
    reference.
  - `publish(online)`, called in `_kernel_start` after `bring_up_aps`, sets "yes" only when the census is
    undecided, unvetoed, and `reported == online > 0`. A short count (a degraded AP) leaves it undecided.
  - Undecided means every call runs its own local check, the old behaviour.
  - A report that lands after a "yes" and says no still downgrades it to "no", before that CPU runs a
    matmul.
- **Discriminator.** Host KATs: `avx2_census_requires_every_cpu` (asymmetric reports; short count;
  late veto), `avx2_census_veto_wins_under_concurrency` (200 rounds, 8 threads, one vetoes: never "yes"),
  `avx2_census_global_agrees_with_local_check` (on this host, the census decision equals the local check,
  and AutoAvx2 equals Scalar bit for bit).
- **QEMU discriminator (whole boot, every CPU reporting).** `threaded_matmul_parity` with the AVX2 kernels
  compiled in through the extra-features mechanism (the scenario's own feature does not enable them), on
  4 CPUs:
  ```
  NEO_SMP=4 NEO_TEST_EXTRA_FEATURES=phase9-avx2-matmul cargo neo-test --no-meta threaded_matmul_parity
  ```
  - On the default `-cpu qemu64,+rdrand,+smep,+smap` (no AVX2): `[ kernel ] ai: matmul avx2=No (census:
    4/4 cpus reported)`, then `NEO_TEST/1 event=PASS name=threaded_matmul_parity q4_K=byte_identical
    q8_0=byte_identical ...`.
  - On `-cpu max`: `[ kernel ] ai: matmul avx2=Yes (census: 4/4 cpus reported)`, then the same PASS line.
    The gate has no switch for this: the run used a local, uncommitted edit adding
    `threaded_matmul_parity` to `scenario_needs_qemu_max` in `tools/xtask/src/main.rs`.
  - Read it as: every online CPU reported (`4/4`), and the decision equals what every CPU's own check
    says. A short count (`3/4`) must print `avx2=Undecided`, never `Yes`.
- **The general rule.** A capability that depends on CPU-local state is decided per CPU, or as the AND of
  every CPU's own answer, published only once every CPU has answered. Until then, each CPU checks for itself.
- **Source.** Ours (Janus F1 on PR #105).

### CP-80 — A wait counted in spins, not time

- **Shape.** `par_matmul`'s steal grace was `STEAL_GRACE_SPINS = 2_000_000` iterations of
  `spin_loop()` (`PAUSE`), documented as "~sub-millisecond". `PAUSE` takes ~10 cycles on some cores and
  ~140 on Intel cores since Skylake, so on those the grace is ~280M cycles, about 0.1 s. The BSP pays it on
  every fork-join job whose assigned AP is not polling (running a user thread, the WM, in an interrupt).
  The forward has ~120 jobs per token, so one busy AP can cost seconds per token without any error.
- **Where it bit.** Suspected, not yet measured: Arthur's answer on the Zenbook (16 CPUs) took 57 s for a
  0.5B model (round 2 of `Plans/NOTE-2026-10-23-arthur-prefill-latency.md`). Found from code; the
  `prefill-perf` overlay's `st=` (steals) and `bar=` (barrier share) on the next photo say how much of it
  this was.
- **Guard.** `matmul-steal-grace-time` (in image A): the grace is `STEAL_GRACE_US = 50` microseconds of TSC
  time since the job was published (`par_matmul_logic::past_steal_grace`), with the spin count only when the
  TSC rate is unknown. It still decides only when an UNCLAIMED slot may be stolen; a claimed slot never
  reaches it (`barrier_action`, the F1 fail-stop), and `BARRIER_LIMIT` stays a spin backstop.
- **Two traps in the time version (Chad, relay #52, both fixed forward).**
  - *The threshold truncated to nothing.* The first cut computed `tsc_hz / 1_000_000 * grace_us`: 0 cycles for
    any rate below 1 MHz (CPUID leaf 15 can report 500 kHz with no reference), so the BSP stole on its first
    look, and a non-integral MHz rate lost up to 50 cycles. `steal_grace_cycles` now multiplies first, in
    u128, rounds up, and saturates at `u64::MAX`.
  - *The publish stamp could precede the publish.* A Release store orders later memory accesses, not a
    later RDTSC, so a bare `rdtsc` after the `pending` stores could read the clock before any AP could see the
    job, starting the grace early. The stamp is `MFENCE; LFENCE; RDTSC` (SDM vol. 2B, RDTSC), once per job.
    The per-poll reads stay plain: a late read only lengthens the grace.
- **Discriminator.** Host: `steal_grace_is_time_when_calibrated_else_spins` (time decides when calibrated,
  whatever the spin count; spins exactly as before when not) and `steal_grace_cycles_rounds_up_without_truncation`
  (500 kHz, 2,399,999,999 Hz, 3 GHz, `u64::MAX`). QEMU gate: `threaded_matmul_parity_grace_time` runs the parity
  scenario with the feature on (byte-identical at 1/2/4 threads, negative control red), then holds AP slot 1
  so the BSP must steal it, and asserts the calibrated path ran, the steal happened, it came no earlier than
  the grace after the publish stamp, and the output is byte-identical. The scenario sets a test-only rate and
  grace (1 GHz, 500 ms = 500M cycles) through the production threshold: hermetic (a runner whose LAPIC
  calibration fails still takes the calibrated path; the first cut of this check required calibration and
  passed only on xtask's retry), and far longer than the BSP's own partition under TCG (~50M cycles), which
  at the production 50 µs would hide a 0-cycle threshold. A 0-cycle grace fails it (`stole_before_grace`). The plain `threaded_matmul_parity` keeps covering the feature-off spin grace. The
  first version of this entry claimed a feature-on QEMU parity run that the gate never ran (item 6).
- **The general rule.** A latency bound is time (TSC, a tick, a deadline), never an iteration count of a
  pause or a spin. A spin count is only a backstop whose length nobody depends on.
- **Source.** Ours. Intel's optimization manual notes the Skylake `PAUSE` latency change (~140 cycles);
  Linux sizes its spin waits in time (`ktime`), not iterations.

### CP-81 — A nap whose sleep path is compiled out becomes a yield loop

- **Shape.** `touchpad::nap_ticks` (like `usb::nap_ticks`) sleeps with `wait_any::sleep_ticks` under
  `wait-multiplex` and otherwise loops `yield_now()` + `spin_loop()` until the deadline tick. The product
  image does not build `wait-multiplex`, so the touchpad's service thread, which napped one tick per loop,
  never blocked: it was runnable on every tick for the life of the system.
- **Where it bit.** Image A5 on the Zenbook, the first image where the touchpad binds. Found reading the
  code after a report of slow input at desktop start; the start-up delay itself is a separate lane.
- **Guard, part 1 (#115).** IRQ mode (the touchpad's mode on the Zenbook) parks on `WAITER`, a wake token
  the ISR takes after setting `PENDING`. The thread re-checks `PENDING` (and the mode) under `WAITER` before
  it installs the token, and the ISR drops `WAITER` before calling `wake_thread_token`, so a wake is never
  lost and the lock order stays `WAITER` → scheduler. A failed service parks until stopped.
- **Guard, part 2 (`tick-sleep`, in image A).** The periodic naps need a timed wake, not an event: the USB
  port service re-reads ports every 250 ms (QEMU's xHCI delivers no MSI, and hub ports report through the
  hub's interrupt endpoint, not a port-status-change event), and so do the boot input check and the
  touchpad's polled modes. `sched::tick_sleep` is the smallest timed park: 8 slots of (deadline tick, wake
  token) under one `IrqSafeMutex`, expired by the timer tick after `wait_any`'s scan. The sleeper checks its
  deadline and takes a slot under the lock with IF off, and the context switch releases the guard, so a tick
  on another CPU always finds the token (CP-8/CP-14). The tick takes due tokens under the lock and wakes
  them after dropping it: order `TABLE` → scheduler, as part 1. A full table falls back to the yield loop and
  is counted. `usb::nap_ticks` and the touchpad's `nap_ticks` use it when `wait-multiplex` is off. A kthread
  stop request does not cut a sleep short; it still ends at its deadline.
- **Discriminator.**
  - The watchdog's `tp: mode=irq loops=N` line: `loops` (service-loop passes) stays still while the finger
    is off the pad. Before part 1 it would climb by about 100 a second.
  - The watchdog's first line, `sleep parked=N full=N spins=N usb loops=N`: `spins` stays 0 while every nap
    parks, and `usb loops` climbs by about 4 a second. A loop count alone cannot tell the two naps apart:
    the yield loop also passes once per 250 ms. Only `spins` does.
  - QEMU `usb_hotplug` (with `tick-sleep`): a test thread's 20-tick sleep parks (its slot appears), does not
    run before its deadline, is woken by the tick after exactly 20 ticks, and a past deadline does not park.
    Every nap of the scenario parks (`full=0 spins=0`), and the plug/unplug/re-plug still enumerates. The
    control `neo-test-tick-sleep-control` (never parks) FAILs `tick_sleep_not_parked`.


### CP-82 — A lazy-init `Once` whose initializer blocks, waited on at IF=0

- **Shape.** `GuardedDevice::arming` was `self.arming.call_once(|| self.judge())`. The judgement reads the
  disk (`GuardReader`), so the judging thread parks on I/O and can be timer-preempted (NeoFS writeback runs
  on a preemptible kthread). A `spin::Once` makes every other caller spin, without yielding, until the
  initializer returns. An fsync syscall reaches `arming()` at IF=0 before the I/O gate's acquire turns
  interrupts on (`sys_fsync_impl` was not wrapped IF-on when OT-74 wrapped `sys_read`/`sys_write`). If it lands on the judge's CPU while the judge is parked or preempted, the judge is
  queued behind a spinner that is never preempted: CP-37, with the `Once` as the lock.
- **Where it bit.** Gate run 40174433840 (#119): `arthur_memory_forgetting` timed out at 900 s twice, then
  passed in 18.6 s. Reproduced locally (`NEO_SMP=4`, 4 host CPU hogs): 3 hung runs in 12. Every hung attempt
  stops mid-judgement with no `writes armed` line, and the QEMU monitor dumps of three of them show CPU 0 at
  IF=0 in `Once::poll` ← `GuardedDevice::arming` ← `write_block` ← `neofs::cache::drain_pass`, with the other
  CPUs idle. Full evidence: `Plans/NOTE-2026-10-25-write-guard-judge-hang.md`.
- **Why the earlier audit missed it.** OT-74 fixed the NeoFS cache lock for this exact shape (CP-56), but
  the write guard (#85/#88) was added later, one call further down the same path, and a `Once` does not
  look like a lock.
- **Guard.** Only the caller that wins the `judging` CAS enters `call_once`. Every other caller waits on
  `arming.get()` inside `with_interrupts_enabled`, calling `yield_now()` each round (CP-2). The judgement is
  unchanged: one judge per device, its first writer.
- **Discriminator.** QEMU `block_write_guard` judge-contention leg: a kthread is held inside the judgement's
  first read for 20 ticks while the scenario thread asks for the same judgement with interrupts off. It must
  report `waited_yielding=1 spun_if_off=0` and the right result. The control
  `neo-test-write-guard-judge-spin-control` (the old `call_once` wait) FAILs
  `judge_waiter_spun_with_interrupts_off`.
- **The general rule.** A lazy-init cell is a lock for as long as its initializer runs. If the initializer
  can block or be preempted, waiters must wait the way a sleeping lock's waiters do: interrupts on, yielding.
- **Source.** Ours; the CP-37 / CP-56 family.

### CP-83 — Per-thread FS.base restored selectively, or never saved while ring 3 can write it

- **Shape.** C1.2 gives each user thread an FS.base (its TLS key block). Two easy designs are wrong:
  1. **Restore only when it matters**: load FS.base only for threads that called `SYS_THREAD_SET_TLS`, or
     skip the load when the incoming thread's base is 0, or keep a per-CPU "last loaded" cache that starts
     at 0. Each leaves the previous thread's value in `IA32_FS_BASE`: a thread that never set one (or a
     fresh `thread_create` child before its bootstrap runs) reads another thread's key block through
     `fs:`, and across a process switch, another process's address. The cache variant is the subtle one:
     the MSR's value at boot is whatever firmware left, so a cache that starts at 0 claims a write it never
     made.
  2. **Assume ring 3 cannot change FS.base.** The C1.2 build did, and it was wrong (Jarvis's review,
     2026-10-27). Even with CR4.FSGSBASE off, ring 3 can run `mov ax, 0x23; mov fs, ax`: loading a
     selector into FS loads the descriptor's base into the hidden FS.base, and the flat user data selector
     (`gdt.rs`) has base 0 (SDM Vol. 3A §3.4.4). So the hardware value can differ from anything the kernel
     last wrote. C1.2 kept a per-CPU "last written" cache to skip the WRMSR, and after such a load it lied
     twice: a later switch-in of any thread whose `fs_base` equalled the cached value skipped the write
     (that thread could be in another process, since layouts repeat), and `SYS_THREAD_SET_TLS(B)` returned
     0 while the hardware still held 0. Enabling CR4.FSGSBASE adds a second way (`WRFSBASE`).
  3. **Never save on switch-out.** Still correct, but for a narrower reason than C1.2 stated: the
     authoritative value is `Thread.fs_base`, and the hardware is rewritten from it on every switch-in. A
     user who changes FS.base behind the kernel's back loses that change at its next switch-in. It cannot
     reach another thread, because the next thread's switch-in writes its own value. If user-written bases
     ever need to persist (FSGSBASE for a `WRFSBASE` hot path), the switch-out must save FS.base (or kernel
     entry must) in the same change.
- **Where it applies.** `scheduler::cross_process_prep` / `same_process_user_prep` (`load_user_fs_base`),
  `set_current_fs_base`, `sys_thread_set_tls_impl`, `Thread.fs_base`.
- **Guard (corrected 2026-10-28).**
  - Every switch into a user thread writes `IA32_FS_BASE` from its `fs_base`, unconditionally; a thread
    that never set one gets 0. There is no per-CPU cache: the kernel never reads the hardware value, so it
    has no way to know when a cache would be true.
  - `SYS_THREAD_SET_TLS` stores the field and writes the MSR with interrupts off under the CPU's scheduler
    lock, so a preemption sees both or neither, and the new base is live on return whatever ring 3 loaded.
  - A switch into a kernel thread skips the write: kernel code does not use FS.
  - **Cost.** One WRMSR per switch into a user thread. I did not measure it; the figure I cite is the
    commonly quoted order of 100 cycles for `IA32_FS_BASE` on recent Intel cores, small beside the rest
    of a context switch (the CR3 load and the register restore), and a cross-process switch already
    writes CR3.
  - **FS.base follows the thread, not the CPU.** A migrated thread gets its own value written on the CPU
    it lands on. Nothing per-CPU holds FS state.
  - **Interrupt, exception and NMI paths never use FS** (the kernel's per-CPU data is GS-relative), so an
    interrupt that lands between the field update and the MSR write, or on a CPU still holding a previous
    thread's base, reads nothing through it. Kernel code must keep not using FS.
  - **Anyone who reintroduces a cache, or enables CR4.FSGSBASE, must handle the user-written value in the
    same change:** a cache must be invalidated whenever ring 3 may have changed FS.base (any non-zero FS
    selector, any `WRFSBASE`), and persisting a user-written base needs a save on switch-out. That is the
    rule this entry exists to keep.
- **Discriminator.** `threads_tls` (QEMU, `NEO_SMP=4`):
  - check 2: four threads read their own key value back across repeated switches; with the switch-in load
    removed (both prep paths), it fails with code 2003, a child reading a sibling's value;
  - check 3: a thread parked on CPU 0 and woken onto CPU 1 by the IPC handoff's migrating wake still has
    its recorded FS.base, the live MSR and `fs:[0]` equal to its own control block;
  - check 7: a thread that skips the bootstrap reads FS.base 0 and MSR 0, never a sibling's;
  - check 1: `SYS_THREAD_SET_TLS` takes effect within the caller's quantum (`fs:[0]` read right after the
    call sees the new block). With the MSR write dropped from `set_current_fs_base`, it fails (code 114);
  - check 6: a key access on a thread whose FS.base was never set fault-kills its process (139) instead of
    reading address 0;
  - check 8 (added 2026-10-28): a thread on CPU 0 sets TLS B, loads selector `0x23` into FS (code 6 if
    the load does not zero FS.base), calls `SYS_THREAD_SET_TLS(B)` again and must read B back (codes 20+),
    then loads `0x23` again, sleeps two ticks so CPU 0 switches to its idle kernel thread, and must read B
    back after the switch-in (codes 30+). With the cache restored in both paths it fails 8024 (the setter
    skipped its write); with the cache only on switch-in, 8034.
  - **A test race in check 8 (fixed 2026-10-29).** Step 6 loads `0x23` and then probes the MSR in a
    syscall. A timer preemption between the two switches the thread back in, and the switch-in writes
    `Thread.fs_base`, which is the guard this entry asks for. The probe then reads B, not 0, so step 6
    failed (8006) on a correct kernel, once in the PM's batch. Step 6 now loads and probes again, up to 16
    times, but only while the probe reads exactly B, the value a switch-in writes. Any other value fails at
    once, and a load that never zeroes the base still fails 8006. The same window before step 8's sleep is
    harmless: the sleep switches the thread in again either way.
- **Source.** Ours (OT-61 C1.2; premise corrected after Jarvis's review,
  `Plans/RETURN-2026-10-27-ot61-c1.2-tls-review-jarvis.md` items 2, 3, 7). Principle: Linux x86-64 `__switch_to` / `x86_fsgsbase_load` and the
  FSGSBASE enabling series, which had to add a save path once user mode could write the base (read for
  principle only).

### CP-84 — A single-thread fallback for per-thread state that a second thread can reach

- **Shape.** C1.3 lets a process use TLS keys before FS.base is installed: std's `no_main` binaries have no
  runtime start to install the initial thread's control block, and a process without the C1 capability can
  never install one. Until then the key layer uses a static block for the initial thread. That is correct
  only while the process has one thread. If a second thread can start while the layer still takes the
  fallback, both threads read and write the same key block: one thread's `thread_local!` values, and its
  destructors, become another's. Nothing faults; the corruption is silent. The easy mistakes:
  1. putting the switch in the friendly spawn API only, so a raw `sys_thread_create` (or a test-only
     create-on-CPU) still starts a thread on the fallback;
  2. flipping the flag before `SYS_THREAD_SET_TLS` succeeded, so the initial thread reads `fs:[0]` with
     FS.base 0 (a fault) or, after a failed install, nothing points at its keys;
  3. dropping the initial thread's existing key block at the switch, so values set before the first spawn
     vanish (std's own thread-locals are set long before).
- **Where it applies.** `userland/neoplutarch-std-overlay/std/neoplutarch_rt/tls.rs` (`current_tcb`,
  `ensure_initial_installed`, `install_at`), std's `Thread::new`, libneo's `sys_thread_create` and
  `sys_test_thread_create_on`.
- **Guard.**
  - `ensure_initial_installed` points the initial thread's FS.base at the same static block it already uses
    (so its keys carry over), and only after `SYS_THREAD_SET_TLS` succeeds does it set `FS_INSTALLED`
    (Release). From then on `current_tcb` reads `fs:[0]` on every thread.
  - Every thread-creating wrapper calls it first and returns its errno, creating no thread, if it fails.
    While the flag is clear the caller is the only thread, so the flip cannot race.
  - A child installs its own block (`install_at`) before any application or std code; one started without
    the bootstrap has FS.base 0 and faults on its first key access (process-contained), never shares.
  - `install_at` hands the keys back to the static block if `SYS_THREAD_SET_TLS` refuses.
- **Discriminator.**
  - `threads_tls` check 6: a raw `sys_thread_create` child whose FS.base was never set is fault-killed
    (139). With `ensure_initial_installed` removed from `sys_thread_create`, the child takes the fallback and
    the worker returns 46 (see the C1.3 PR for the run).
  - `std_threads` checks 6–8: the initial thread's `thread_local!` value set before the first spawn survives
    the switch, and four threads hold distinct values across switches.
  - `ntls-logic-tests`: a refused `SYS_THREAD_SET_TLS` leaves the process on the static block with its
    values, and a refused `install_at` gives them back.
- **Source.** Ours (OT-61 C1.3). Principle: glibc and musl install the initial thread's TCB in the startup
  code before `main` and before any `pthread_create`, so the fallback never exists there; a runtime without
  that hook has to retire the fallback at the first create (read for principle only).

### CP-85 — A process exit that retires only the calling thread

- **Shape.** Before threads, `SYS_EXIT` and "end the calling thread" were the same act. Once a process has
  several threads they are not, and a process-exit call that keeps the old body retires only its caller.
  Every runtime that ends a process from an arbitrary thread then breaks quietly. std's `process::exit` and
  every panic under `panic=abort` (`abort_internal` → `SYS_EXIT(134)`) can run on a worker. The process
  stays alive with the remaining threads running, and a joiner parks forever: the exiting thread never
  completed its join record, because it left through the process path, not `SYS_THREAD_EXIT`. Nothing
  faults, and the parent's `sys_wait` never returns.
- **Where it applies.** `sys_exit_impl` (`kernel/src/arch/x86_64/syscall.rs`),
  `process::exit_group_current` and `retire_process_from_current_thread`, the sibling drain shared with the
  ring-3 fault kill.
- **Guard.** With `browser-runtime`, `SYS_EXIT` from a process whose registry counts more than one live
  thread records the code, marks the process exiting (and fault-killing, so its reap drains IPC- and
  wait-parked siblings), purges its futex and join queues, yanks its Ready and token-parked siblings, then
  retires the caller; the last reap runs the normal teardown. With one live thread, nothing changes (nothing
  can add a thread while the caller is the only one). Without `browser-runtime` a process has one thread
  and the hook is compiled out.
- **Known limit, shared with the fault kill: CP-86.** The drain reaches siblings that are Ready,
  token-parked, IPC-parked or wait-parked, not one *running on another CPU* at that instant. C1.3b added
  the cross-CPU retire; see CP-86 for its guard and the two survivors the C1 review fixes closed.
- **Discriminator.** `std_threads`, third spawn (`std-real-thread-abort`): a worker panics while one
  sibling is parked in a `Condvar` wait and another spins on `yield_now`. The process must exit 134. With
  the `exit_group_current` call removed from `sys_exit_impl`, the panicking thread alone exits and the run
  times out (see the C1.3 PR).
- **Source.** Ours (OT-61 C1.3), found by a falsification run. Principle: Linux `exit_group(2)`, which
  glibc's `exit` uses so that a process exit ends every thread (read for principle only).

### CP-86 — A process retire that never reaches a sibling running on another CPU

- **Shape.** The fault kill (C1.1) and the exit group (C1.3) end a process by draining its siblings:
  Ready ones are yanked from the ready queues, futex/join/token-parked ones are purged and zombified, and
  IPC- and wait-parked ones are zombified by the reaper. A sibling that is *Running in ring 3 on another
  CPU* at that instant is in none of those places. Nothing stops it afterwards: APs get no timer tick, so
  it is never preempted into a ready queue, and the reschedule IPI was EOI-only (CP-15/CP-21). The process
  never reaches its last reap, and its `sys_wait`er hangs. A thread that was inside a syscall when the
  retire began has the same problem if it then parks after the futex purge.
- **Not a memory-safety defect.** Each running thread holds its own `Arc<Pmap>` and `Arc<Process>`
  (`kernel/src/sched/thread.rs`, fields `pmap` and `process`), released only by `Thread::free` at reap,
  after it is off-CPU (`scheduler::exit` holds the scheduler lock across the switch, OT-31). The process
  teardown (`on_thread_reap`) runs only when `ThreadRegistry.live` reaches 0 (`note_reaped`), and a running
  sibling still counts. So the address space outlives it; the defect is liveness.
- **Where it applies.** `process::retire_process_from_current_thread` (the retire tail),
  `kick_cpus_running`, `retire_current_if_dying`, `RETIRING` and `Process::drop`; `per_cpu::current_pid`
  (published at every switch in `scheduler.rs`); `syscall_dispatch`; `idt::reschedule_ipi_body`;
  `timer::timer_rust_handler` and its stubs' `timer_ring3_arg!`; `futex::wait_on_key`.
- **Guard (C1.3b).**
  - **Mark first.** The retire stores `fault_killing` and `exiting` before it drains.
  - **Retire at every ring-3 boundary.** A thread whose process is retiring calls `scheduler::exit()` at
    syscall entry, at syscall return, and in the timer and reschedule-IPI handlers when they interrupted
    CPL 3. Nothing is held at those points (CP-63/CP-65/CP-67); the IRQ-context checks take the PROCESSES
    and scheduler locks only because the interrupted CPU was in ring 3 and so holds neither (CP-37).
  - **Interrupt the CPUs running a sibling.** After the drain, the retire reads each CPU's published pid
    and sends the reschedule IPI to every other CPU running the process. A CPU publishes the pid it
    switches in while holding its scheduler lock, and the drain took every CPU's scheduler lock before the
    scan. So a sibling the drain did not yank as Ready was already published as running, and the scan sees
    it. The IPI handler retires it if it interrupted ring 3; from ring 0 the thread is in a syscall, whose
    return path checks.
  - **No late parker.** `futex::wait_on_key` refuses (`ESRCH`) to park a thread of a retiring process,
    reading the flag under `FUTEX_QUEUE`. The retire stores the flag before `purge_pid` takes that lock,
    so a waiter either parked before the purge (and is purged) or sees the flag; its syscall return then
    retires it. IPC and process-wait parks are re-drained by the reaper on every reap.
  - **The kernel join honours the refusal (fixed 2026-10-28, Jarvis finding 1).** `thread_join_for_current`
    loops on `wait_on_key` until the joined thread's done word is set. C1.3b discarded the call's result, so
    a join refused with `ESRCH` looped forever in CPL 0 with IF=0 (SFMASK) and never reached the
    syscall-return check. It now returns `ESRCH` to the syscall boundary, which retires the thread. Every
    other kernel caller of `wait_on_key` already returns its result.
  - **The last check runs with interrupts off (fixed 2026-10-28, Jarvis finding 2; CP-93).** Some
    syscalls return with IF=1 (`spawn_user` through `LocalShootdownQuiesce::resume`). The retire IPI is the
    only thing that reaches a sibling spinning in ring 3 on an AP, and if it lands between the return check
    and the `cli` before SYSRET/IRETQ, its handler sees CPL 0, EOIs and does nothing, and the thread then
    runs ring 3 forever. `syscall_dispatch` now disables interrupts before the final
    `retire_current_if_dying`, so an IPI either lands before the check (which then sees the flag) or stays
    pending until ring 3, where its handler retires the thread.
  - **The counter follows the flag.** `fault_killing.swap(true)` publishes the flag first; only the
    first swap then increments `RETIRING`. A boundary that runs between the two can read `RETIRING == 0`
    and skip its check. That is safe: the initiator increments before it drains and before it sends the
    IPIs, so a thread that skipped is still yanked, purged or interrupted afterwards. (C1.3b's dispatch
    said the increment comes first; it does not.)
  - **Cost when nothing is retiring:** one load of `RETIRING` per boundary.
  - With `browser-runtime` off, every hook is compiled out, and the timer stub is unchanged (its extra
    `mov rdi` is a cfg'd fragment).
- **Discriminator.** `threads_exit_group` (QEMU, `NEO_SMP=4`), three `neo-worker` roles:
  1. a thread spinning in ring 3 on CPU 2 (placed with `SYS_TEST_THREAD_CREATE_ON`) plus a futex-parked
     one, then `SYS_EXIT(7)`; without the IPI (`kick_cpus_running` removed), the process never finishes;
  2. the same spinner, then a ring-3 page fault (139), the fault kill's twin;
  3. a waiter on CPU 1 armed with the test-only `SYS_TEST_FUTEX_LATE_PARK`, whose futex wait reaches its
     park only after the retire's purge; without the futex refusal, it parks forever;
  4. a sibling on CPU 2 inside the test-only `SYS_TEST_KERNEL_SPIN` (ring 0, interrupts on) when the main
     thread calls `SYS_EXIT(7)`. The IPI lands in ring 0, and after the syscall the sibling spins in ring 3
     with no further syscall. Without the syscall-return check, it never ends;
  5. (added 2026-10-28) a sibling armed with the late park that calls `SYS_THREAD_JOIN` on a parked
     sibling, so its join's futex wait reaches the park only after the purge. With the join discarding
     `ESRCH` (the C1.3b code), the run hangs;
  6. (added 2026-10-28) a sibling on CPU 2 in the test-only `SYS_TEST_TAIL_HOLD`, which re-enables
     interrupts, and after the final check holds in the syscall tail, with IF left as the tail left it,
     until the process is retiring and three ticks have passed. It then returns and spins in ring 3. With
     the `cli` before the check, the IPI stays pending through the hold and retires the thread in ring 3;
     without it, the IPI is used up in ring 0 and the run hangs.
- **Not discriminated directly: the timer path.** The scenario does not cover a BSP sibling that the timer
  preempts between the retire's scan and the IPI. Such a sibling sits Ready, not yanked. When it next runs,
  it resumes through its interrupt frame, and on the BSP the next timer tick from CPL 3 retires it. A
  deterministic scenario would need to place a timer tick inside a window a few instructions wide. The
  check is there; the scenario is not.
- **`RETIRING` and a leak.** If a retiring process's `Arc<Process>` were never dropped, `RETIRING` would
  stay above 0, and every syscall and every ring-3 interrupt would take the slow path. That path looks up
  only the caller's own process and checks its own flag, so no thread of another process is retired. The
  futex refusal does not read the counter, and pids are not reused (`NEXT_PID` only grows). A leak
  therefore costs speed, never correctness.
- **Open item: APs get no timer tick.** A ring-3 thread on an AP is never preempted, by anything. That is
  a fairness and latency question beyond process exit (OT-31's ledger row notes it), and no OT or CP
  records it as a decision yet.
- **Source.** Ours (OT-61 C1.1/C1.3). Principle: Linux's `zap_other_threads` + `signal_wake_up`, which
  marks every thread of an exiting group and kicks the CPUs running one, each thread leaving at its next
  return to user (read for principle only).

### CP-87 — Revoke walk vs concurrent install (pre-existing, found 2026-10-27)

- **Shape.** `revoke_handle` computed the descendant set by walking every process's `derivations` map,
  one process at a time, and then removed the set from every table. Nothing spanned the two steps. A
  derive on another CPU that read its parent after the walk passed its process, but before the removal,
  installed a child the set did not contain. The parent then died and the child lived. `complete_grant`
  (source read, lock dropped, recipient installed), spawn inheritance (handles cloned, child registered
  later, invisible to the walk until then) and every install naming a parent (tensor share, TCP accept,
  GPU fence) had the same window.
- **Guard (K1a-0).** One global `CAP_GRAPH` lock, a FIFO ticket spin lock (`sync::ticket_mutex`) taken with
  interrupts off and never from an interrupt path. Order:
  `Channel.state → CAP_GRAPH → PROCESSES → handles → derivations → wait-any table → scheduler`. FIFO bounds a
  revoke's wait behind a stream of derives and closes to the lockers queued ahead of it. Revoke holds it across the walk and the
  removal. Every install naming a parent holds it and re-checks that the parent is still in the table
  (`install_minted_locked`, `install_cap_excluding`, `derive_handle`, `derive_net_dial_cap`).
  `complete_grant` does not re-check at its install (it passes `parent_in_proc = false`): it reads its
  source in the same `CAP_GRAPH` section as the install, so the read is the check, and splitting that
  section in two would reopen the window (corrected 2026-10-29). Spawn checks under it that each inherited parent is live, then installs and
  registers the child, and refuses with `EBADF` otherwise. Nothing under it parks, takes a
  `Channel.state` or waits on another CPU, so its waiters spin with interrupts off. **Corrected
  2026-10-27 (Janus, #166 review):** as merged in #166 that last sentence was false in two hidden
  ways, a scan's last `Arc<Process>` dropping under the lock (a `Pmap` shootdown wait, CP-91) and a
  last tensor release under it (a reservation park, CP-92). Both are fixed by construction in the
  K1a-0 review-fix PR; the sentence holds from that PR on.
- **Discriminator.** `cap_graph_revoke`, three race legs, one per install path: `SYS_TEST_CAP_GRAPH_RACE`
  arms the next revoke to derive from, grant from, or spawn-inherit a doomed descendant between its walk
  and its removal, where a racing CPU's install landed (the spawn twin clones the inherited handle inside
  the window, as `sys_spawn` clones before `spawn_user`). With the lock, the hook first proves the revoke
  holds `CAP_GRAPH` (`try_lock` fails), then runs the install after the revoke returns: derive and grant
  get `EBADF`, and the spawn is refused. Under `neo-test-cap-graph-unlocked-control` each runs inside the
  window and its cap survives. The hooks are deterministic; they place the interleaving rather than hope
  for it.
- **Discriminator, two CPUs (added 2026-10-29, Janus's #183 item 6).** The race hooks above defer the
  install rather than contend for the lock, so none of them can catch a grant that splits its section.
  `cap_graph_revoke`'s two-CPU grant leg does: a grant on CPU 1 stops at a test barrier inside
  `complete_grant` between its source lookup and its install, holding `CAP_GRAPH`; a revoke of the source
  starts on CPU 2; the BSP checks two ticks later that the revoke is still waiting, then releases the
  grant. The recipient must end with nothing. Falsified by dropping and re-taking the lock around the
  barrier: the leg fails `granted_child_survived_the_revoke`.

### CP-88 — Laundering by close or exit (pre-existing, found 2026-10-27)

- **Shape.** A cap's edge (`child → parent`) lived only in its holder's `derivations` map. `sys_close`
  (and the tensor, model and socket closes) deleted it; process exit and kill dropped the whole map. A
  child of that cap, in the same process or granted or passed to another, then pointed at a parent no
  map knew, and the revoke walk, which follows parents upward to the revoked cap, never reached it. Any
  holder could launder a cap out of its ancestor's reach with derive + close, or by granting it on and
  exiting. Logout (OT-66 §3.5) kills the session's clients before it revokes their caps, so the exit
  shape was on that path.
- **Guard (K1a-0).** Under `CAP_GRAPH`, a cap whose edge is going (a close, a teardown) first hands its
  children to its nearest ancestor that is not also going (`CapGraph::retire`). A set of caps that have
  had children keeps the common close (a cap with none) to one lookup. ADR-010 Q4 still holds: the
  children survive their donor's exit; they just stay revocable by the donor's ancestors. All closes go
  through `process::close_handle`; teardown goes through `CapGraph::teardown`, and the kill path
  unregisters and tears down in one `CAP_GRAPH` section.
- **Discriminator.** `cap_graph_revoke`, close leg (D′ from the root, D″ from D′, close D′, revoke the
  root, D″ must be `EBADF`), exit leg (a worker derives from its inherited cap, grants the child back,
  exits) and kill leg (the same, killed while it waits). In the exit and kill legs the revoke of the root
  is placed by a hook inside the teardown, between the edges leaving and the re-parent: with the lock it
  waits, and sees the new tree; it never sees neither. All three leak under the control. Host KATs
  (`tools/cap-graph-logic-tests`): re-parenting preserves every surviving ancestor and keeps rights
  monotone, over every close order of a five-chain's middle three, two adjacent caps torn down at once,
  and a dying root.


### CP-89 — A failed spawn keeps what it charged (new, OT-61 C2.2, 2026-10-27)

- **Shape.** A budgeted spawn charges the child's account for its PML4, image and stack *before* the
  child is registered, so no process owns those charges yet. If the spawn fails part-way (the budget
  can't hold the image, the scheduler is full, an inherited parent is gone), an error path that frees
  the frames but forgets the charges leaves the spawner's account (and every account above it)
  permanently short. A loop of failing spawns then shrinks a parent's budget to nothing.
- **Guard.** The charges live in the child `Pmap`'s `ChargeLedger`, and every spawn error path drops
  that `Pmap`. `Pmap::Drop` releases exactly what the ledger holds, after the frames are freed, and a
  dropped `ChargeLedger` releases whatever is left (RAII), so no error path needs its own release.
- **Discriminator.** `mem_quota` step 5: `SYS_SPAWN_BUDGETED` with a 4 KiB budget is `ENOMEM`, and
  init's `used` is unchanged afterwards. Falsified by leaking the ledger in `Pmap::Drop`
  (`core::mem::forget`): the step fails with init's account 29 frames short. Host KATs
  `path_failed_spawn_cp89`, `path_dropped_ledger_releases_what_it_holds`.

### CP-90 — Two CPUs charging one account chain (new, OT-61 C2.2, 2026-10-27)

- **Shape.** Accounts nest, and sibling processes on different CPUs charge the same parent. A
  read-check-write charge (`if used + n <= limit { used += n }`) lets both pass the check and push the
  parent over its limit. A charge that fails at an upper level must undo the lower levels it already
  took, and an undo that races a release must not wrap the counter.
- **Guard.** Every level is one `fetch_update` (a CAS loop) that refuses to pass `limit` or wrap past
  `u64::MAX`, so `used <= limit` holds at every instant. A refused level undoes exactly the levels
  below it that this charge took. A release is `fetch_update(checked_sub)`, so an underflow is refused
  and counted, never wrapped, and under `neo-test` it panics. No lock is involved, so it is safe at IF=0,
  under `Pmap::inner` and under `FRAME_ALLOC` (CP-37 and CP-56 are N/A: there is no lock whose IF
  discipline could be wrong).
- **Accuracy.** A rollback can make a concurrent sibling's charge fail for an instant (the parent was
  briefly charged for frames that were then undone). That is conservative: never over a limit, never a
  leak.
- **Discriminator.** Host KAT `hammer_never_exceeds_a_limit_and_returns_to_zero`: four threads charge
  and release one 50-frame parent through 20-frame children, asserting `used <= limit` at every step
  and zero at the end. `mem_quota` role 4 at `NEO_SMP=4` (two grandchildren under a 12 MiB parent).
  Falsified by charging only the leaf level: the first `Pmap` drop's release underflows the parent and
  the `neo-test` check panics.

### CP-91 — A last `Arc` dropped under a global IF-off lock runs a shootdown (found 2026-10-27, Janus F1 on #166)

- **Shape.** CP-56's hidden-`Drop` lesson at a new site. `CAP_GRAPH` holders scan every process
  (`processes_snapshot()`: `retire`, `cap_is_live`, `revoke_handle`). The reaper unregisters a process
  and drops its own `Arc` *outside* the lock, so a scan running concurrently can be left holding the
  last `Arc`. Its drop runs `Pmap::drop`, a TLB shootdown that waits for every ready CPU, and a CPU
  spinning for `CAP_GRAPH` with interrupts off never answers: the 50 ms shootdown timeout panics. (A
  contended `SHOOTDOWN_LOCK` would also briefly enable interrupts under the lock.)
- **Guard.** `CapGraph` pins every `Arc<Process>` its scans take (`CapGraph::snapshot`). Its `Drop`
  releases the lock first, then lets go of each pin with `Arc::into_inner`: a decrement that, only when
  the pin was the last reference, hands the `Process` back instead of dropping it. Those go on
  `DEFERRED_PROCESS_DROPS`, which the reaper (and `terminate`) drains at its own process-drop point.
  Direct lookups under the lock are pinned the same way (`CapGraph::get_pinned`: a grant's grantor and
  recipient, both derives'; Janus #127): a plain `get` held across the section is just as able to be
  last. And the reaper's top level drains the list on every pass (`sched::reap_and_drain` →
  `drop_deferred_processes_if_any`, one atomic swap when it is empty), because a pin released after the
  last teardown's drain would otherwise wait for a teardown that may never come. **Only the top level**
  (Janus #182 item 2): plain `sched::reap` is also called inside the NeoFS retry paths, under `entries`
  and the cache's `drain` guard, so it doesn't drain; the drain asserts interrupts are on. **One enqueue**
  (item 4): every producer queues through `defer_process_drop`, which always publishes the pending flag,
  so no producer can queue a `Process` the recurring drain never sees. Releasing after the unlock alone is not enough: every `CAP_GRAPH` section runs inside its caller's
  interrupts-off block, which can still hold spawn's shootdown quiesce or a grant's `Channel.state`,
  and a contended `SHOOTDOWN_LOCK` turns interrupts on briefly (`acquire_shootdown_lock`).
  `revoke_handle` takes its caller's `Arc` before the lock. Removed handles (channel, buffer `Arc`s)
  drop after the unlock too (`Detached`, CP-92).
- **Discriminator.** `cap_graph_revoke` F1 leg: under the lock a scan pins a fixture process, which is
  then unregistered and its other `Arc` dropped, so the scan's is the last; the drop must come after
  the unlock, and not inside the caller's interrupts-off block: the leg checks it was handed to the
  deferred list and is dropped by the drain. The `neo-test` witness in `Pmap::drop` panics if this CPU
  holds `CAP_GRAPH` (`CAP_GRAPH_HOLDER`) or is inside `CapGraph::drop`'s pin release
  (`RELEASING_PINS`). Falsified twice: dropping the pins before the unlock, and dropping them plainly
  after it, both panic with `K1a-0 F1: a Pmap dropped (TLB shootdown) under CAP_GRAPH or its caller's
  IF-off section`. F1b (Janus #127): a grant's recipient exits and is unregistered between the
  grant's lookup and its install (a test hook), so the install refuses it and the lookup is the last
  reference; falsified by an unpinned lookup (the `Pmap::drop` witness panics). F1c: with no teardown
  after it, a plain `reap()` must leave that process queued (`f1c_nested_reap_drained` otherwise) and
  the top-level `reap_and_drain()` must drain it; falsified by removing that drain
  (`f1c_not_drained_by_reap`). F1d: a `Process` queued through `defer_process_drop` with no teardown
  after is freed by the next top-level pass; `neo-test-deferred-pending-control` (the enqueue without its
  flag publish) fails the scenario at F1c. F1f (the PM's #226 bounce): the NeoFS retry paths and a
  kthread join reap with `sched::reap_nested()`, under their locks; a zombie it pops still has its
  teardown run, but `finish_kill`, `on_thread_reap` and `terminate` drop no `Process` there (neither
  the queue nor the zombie's own last `Arc`): both go to the next `reap_and_drain()`. The leg spawns a
  hand-built exit-at-once ELF and reaps it nested; `neo-test-nested-reap-control` (no nesting mark)
  fails it at `f1f_nested_reap_drained`. F1f also checks the zombie's own `Process` is queued, by name.
  Janus #188 recheck 2: two more releases bypassed the deferral. `on_thread_reap`'s early return (a kill
  claimed the process, then completed on another CPU and released its references) held the last clone, and a
  thread's `ProcessAnchor` published the count before its `Arc` field dropped, so a kill seeing 0 could
  release first and leave the anchor last, inside `Thread::free`'s IF-off block. Both now go through
  `release_last_process`; `Thread::free` drops the anchor after its IF-off block. F1g (a hook unregisters
  the process and drops the kill's `Arc` after a nested reap's lookup) and F1h (an anchor holding the last
  `Arc`, dropped with IF off) each fail on their revert. `neo-test-deferred-pending-control` is now armed
  for F1d's enqueue alone and fails at `f1d_queued_drop_never_drained`.
- **Rule.** Under any lock whose waiters spin with interrupts off, no value whose `Drop` can wait on
  another CPU (a `Pmap`, a `Process`) may be the last reference released. Pin and release after.

### CP-92 — A park under a ticket lock (found 2026-10-27, Janus F2 on #166)

- **Shape.** A last tensor-handle release (`HandleTable::remove`/`remove_cap_ids`/`drain` →
  `release_tensor` → `arena::release` → `drain` → `reservation::guarded_clone`) parks when the slot is
  Reserved by a migration. Those removals ran under `CAP_GRAPH` (close, revoke, teardown), so a holder
  could park with interrupts-off waiters spinning behind it, and the ready waiters could keep it from
  ever running again to release the lock.
- **Guard.** The removals return a `Detached` set and release nothing themselves; its tensor counts are
  released when it drops or is unwrapped, after the caller's `CAP_GRAPH` section (`close_handle`
  unwraps after its section; `revoke_handle` declares its `Detached` list before the guard; teardown's
  callers drop it after theirs).
- **Discriminator.** `cap_graph_revoke` F2 leg: the last handle to a tensor leaves by close, revoke and
  teardown, and its slot must be freed. The `neo-test` witnesses in `release_tensor` and in the
  reservation park panic if this CPU holds `CAP_GRAPH`. Falsified by releasing inside `remove`: `K1a-0
  F2: a tensor count released while this CPU holds CAP_GRAPH`. The witness covers every last release,
  not only a Reserved one, so it does not depend on winning a migration race.

### CP-93 — A flag check before returning to user, with interrupts still on

- **Shape.** A kernel that stops threads asynchronously (a process retire, later a signal) needs two
  things: a flag the thread reads at its return to user, and an IPI that brings a thread running on
  another CPU to that return. The IPI handler acts only when it interrupted ring 3; from ring 0 it relies on
  the return path's check. If the check runs and then the kernel goes on with interrupts on before it
  returns, an IPI that lands in between is used up: its handler sees CPL 0 and EOIs, and the check has
  already passed. The thread returns to ring 3 and spins there. On an AP with no timer tick nothing else
  ever interrupts it.
- **Where it applies.** `syscall::syscall_dispatch`'s tail (`retire_current_if_dying` and the return
  path), any syscall that returns with IF=1 (`spawn_user` through `LocalShootdownQuiesce::resume`), and
  any future return-to-user check (signals, a pending-work word).
- **Guard.** Disable interrupts, then do the last check, then return with no `sti` in between. An IPI sent
  after the flag is set either arrives before the `cli` (and the check, which reads the flag after it, sees
  it), or is held pending until the IRETQ/SYSRET restores IF in ring 3, where the handler acts. A pending
  latch that the IPI sets and the IF=0 tail re-reads would also work; the `cli` needs no new state.
- **Discriminator.** `threads_exit_group` role 6 (QEMU, `NEO_SMP=4`): a sibling on CPU 2 calls the
  test-only `SYS_TEST_TAIL_HOLD`, which re-enables interrupts like an IF=1 return, then holds in the tail
  after the final check until its process is retiring and three ticks have passed, then spins in ring 3.
  Without the `cli` the run hangs.
- **Source.** Found by Jarvis's review of C1.3b (`Plans/RETURN-2026-10-27-ot61-c1.3b-review-jarvis.md`,
  finding 2). Principle: Linux's `exit_to_user_mode_loop`, which re-checks the work flags with interrupts
  disabled before returning, and loops if one was set (read for principle only).

### CP-94 — A heap grown in place, assuming the next map lands at its top

- **Shape.** `SYS_MAP_REGION` hands out addresses from one per-process cursor, and so does every other
  user mapping the kernel makes for the process: a thread's stack and guard page
  (`allocate_user_region_guarded`), buffer planes, `sys_vm_map`. A heap that grows by mapping a fresh
  region and calling `linked_list_allocator::Heap::extend` assumes the region starts at its top. While
  the process is single-threaded and maps nothing else, it does. After the first thread spawn it need
  not: the next heap map lands past the new stack, and `extend` writes a free-list hole at the old top,
  which is now the stack's guard page (a ring-3 #PF, exit 139) or, with another layout, a live stack or
  buffer (silent corruption).
- **Where it applies.** `userland/neo-std-pal` (`SyscallHeap`, every real-std binary's global
  allocator) and `userland/libneo` (`LibneoHeap`). libneo hit the buffer-plane form in DI-1b and fixed it
  with several arenas; the PAL's copy, "re-derived" from libneo's earlier discipline, kept the single
  heap until C2.3a.
- **Guard.** A map extends the last region only when it starts exactly at that region's top; any other
  map opens a new region (up to 64; past that an allocation fails, null, never aliases). `dealloc` finds
  the region whose `[bottom, top)` holds the pointer. Growth is geometric (at least a quarter of what
  the heap owns), so the region count grows with the log of the heap size. A request larger than one
  16 MiB map needs consecutive maps to abut; if another thread maps in between, that request fails
  cleanly.
- **Discriminator.** `servo_host_smoke` (QEMU, `NEO_SMP=4`): the smoke spawns and joins threads, then
  grows its heap to 2 MiB and 4 MiB. With the old single-heap `SyscallHeap` it is fault-killed at that
  step (`ring-3 #PF ... rip` in `linked_list_allocator::hole`, exit 139).
- **Source.** Ours (OT-61 C2.3a, found by the first `servo_host_smoke` run). The rule generalizes: any
  allocator that owns address ranges it did not reserve must not assume adjacency.

### CP-95 — A force-kill of a thread that is Ready inside a syscall

- **Shape.** Force-kill took any Ready thread of its target off the ready queue and zombified it; the reaper
  frees a zombie's kernel stack raw (CP-30). CP-30's rule covers a thread *parked* in a syscall: a park site
  owns nothing. A thread is also Ready *inside* a syscall, and then its frames are live: it was preempted
  with interrupts on, it yielded, it was woken from a park, or a rendezvous handed its CPU to the peer.
  The concrete case is `ipc::Channel::send`'s `RecvBlocked` arm: the sender holds its own `Arc<Pmap>` and
  the receiver's as locals, wakes the receiver and yields, so it sits Ready with both on its stack. A
  receiver that kills it right then leaks both address spaces. CP-65 and CP-66 were two marked sites of
  this shape (the inference and admission sections); the handoff was an unmarked one.
- **Where it applies.** `scheduler::kill_thread_by_pid` (via `position_by_pid_killable`), every caller of
  it (`process::terminate`, the C1.1 fault kill, the C1.3 exit group), and any future path that removes a
  thread from a ready queue to end it.
- **Guard (OT-93).**
  - `Thread::in_syscall`, set at `syscall_dispatch` entry (`scheduler::enter_syscall`) and cleared at the
    return decision (`scheduler::leave_syscall`), both under the CPU's scheduler lock. The ready-queue scan
    zombifies a thread only when it is clear (`ready_kill_safe`): that thread resumes in ring 3 and its
    kernel stack holds only an interrupt frame.
  - `Process::force_killed`, published by `terminate` before its scans. A thread the scan leaves reads it
    through its own process anchor and exits at its syscall return, after its frames have unwound. The
    clear of `in_syscall` and the check are one section, so a later kill finds the thread running and out
    of the syscall, and its IPI is taken in ring 3 (CP-93).
  - A thread that would start a user wait after the kill (an IPC endpoint or process wait,
    `block_current_into_with_guard`; a futex or wait-any, `try_block_current_with_token_and_guard`) is
    refused with `ESRCH` instead; nothing would wake it. So is one whose process is retiring (CP-86), since
    the retire's drains also leave such a thread now. Infallible kernel waits (`block_current_with_token_and_guard`:
    device I/O, the NeoFS cache, a tick sleep) still park; refusing them could free a buffer the device
    still writes. **They are not bounded:** a survivor parked on a device that never completes stays parked
    with its process anchor and stack (Janus #149, F1). Main has the same gap for any killed thread in device
    I/O; the fix is a device-wait cancellation that keeps the device owning its DMA until it quiesces (OT-99).
    So a survivor reaches its boundary *if* every wait it is in completes, not unconditionally.
  - `terminate` then sends the reschedule IPI to every other CPU running a thread of the target
    (`scheduler::kick_kill_survivors`); the timer and IPI handlers end such a thread when they interrupted
    ring 3, in every build.
  - CP-65 and CP-66's markers stay: they protect a thread *parked* with owned frames (an inference waiter, an
    admission waiter), which `in_syscall` cannot single out, since every parked thread is inside a syscall.
  - **A wait on the victim returns only after it can no longer act** (Janus #149, F2). While a survivor, an
    admission caller or an inference participant is still in the kernel, its syscall can still take effect
    (a NeoFS write, say). And until `terminate`'s teardown is done, its sockets are still bound (#202 F2,
    Chad). So `terminate` moves every killed process from `PROCESSES` to `DYING` (one section; the
    registration still goes first, for `close_peer`) and leaves its waiters. They complete with ESRCH, and
    EXITED is published, when the teardown is done and its last thread is reaped; whichever comes last
    completes them (`finish_kill`). A `sys_wait` that arrives in between finds the process in `DYING` and
    parks. "Its last thread" is a count, not a scan (the PM's #204 bounce, item 1): every user thread's
    process anchor counts in `Process::live_threads` until `Thread::free`, and every reap re-checks. The
    first version decided at the kill, from the survivor count, whether to wait for a reap, and recorded
    the reap only for a process already in `DYING`: a survivor reaped between the count and the insert
    left the kill waiting for a reap that had already happened, and every `sys_wait` on it parked forever.
- **Discriminator.** `kill_ready_in_kernel` (QEMU, `NEO_SMP=4`): a test hold keeps the worker Ready inside
  its send, after the handoff to init, until init's `SYS_KILL` of it returns. The kernel then checks the
  victim's pmap is gone (a `Weak` taken at the kill) and every frame came back. With the pre-OT-93 rule
  restored (`neo-test-kill-ready-in-kernel-control`): `frame leak: baseline=1028 final=1078 (+50)`.
  F2: init's `sys_wait` on the worker after the kill must return only after the worker's last reap
  (`wait_after_survivor_reaped=1`); with the waiters completed at the kill
  (`neo-test-kill-wait-early-control`) it returns first and the scenario fails `wait_before_survivor`.
- **Known limits.**
  - A Ready thread that resumes in ring 3 and migrates between the scan's per-CPU passes is missed by both
    the scan and the kick, as before; it ends at its next syscall or ring-3 timer tick. Being uncounted, it
    does not defer the kill's waiters (F2), but it resumes in ring 3, not in a syscall, so it has nothing in
    flight.
  - A survivor in an infallible device wait that never completes (F1, above; OT-99).
  - The production syscall-return tail (`leave_syscall`'s check, then IRETQ with interrupts off) has no
    runtime discriminator of its own: `syscall_exit_irq_window`'s test syscall skips it (Janus #149, item
    6). Its ordering is argued from the source (CP-93).
- **Source.** Ours (OT-93), found by the socket-teardown kill leg (#202). Principle: Linux delivers a
  fatal signal as a pending flag acted on at the return to user (`exit_to_user_mode_loop`), never by
  freeing a task mid-kernel (read for principle only).

### CP-96 — An authority checked only by a right the holder can mint itself

- **Shape.** An operation is gated by a right on the handle it acts on, and the caller can make itself a
  handle with that right. `SYS_INPUT_FOCUS_CREATE` needs no authority and returns a `RECEIVE` root;
  `SYS_INPUT_FOCUS_MINT_ACTIVATOR` needs only `RECEIVE` and installs an `ACTIVATE` sibling in the
  caller's own table (so a client can grant it to the WM); `SYS_INPUT_FOCUS_ACTIVATE` checked only
  `ACTIVATE`. So create, mint, activate let any process take the keyboard route.
- **Where it applies.** `kernel/src/arch/x86_64/syscall.rs` (`sys_input_focus_activate_impl`). The
  general form: a capability meant to be *handed to* an authority is also *usable by* its minter.
- **Guard.** Activation also requires the caller to hold `InputSystem` (`caller_holds_input_system`),
  which has no create syscall, no `GRANT` and no `DERIVE`, and is installed by the kernel into the session
  root only. The root holds it wherever the focus syscalls exist (`atrium-compositor-g1`), not only with
  the pointer route. Minting and granting are unchanged.
- **Discriminator.** `desktop_pointer_boot`: Arthur's input-system probe creates a focus, mints an
  activator and activates it, and must get `EPERM` (`activate=EPERM`); had it worked, the route would
  have left Arthur and the scenario's later keys would not reach it. With the old check restored the probe
  logs `NOT_REFUSED` and the scenario fails.
- **Source.** Ours (the OT-66 L2 pre-merge AAG, F1). Principle: an authority-holder check must be on
  something the caller cannot forge or mint (here the non-delegable `InputSystem` handle), never only on
  a right of the object being acted on.

### CP-97 — A security mode held only in a user-space server's own state

- **Shape.** A mode that must hold the instant a key is pressed (here, locked: no client gets input) is
  kept by a user-space server, which makes it true by acting (swapping the route). The server acts only
  when it runs. A server parked in the kernel (a synchronous send to a client that never receives, CP-57;
  a long wait it can't leave) leaves the mode false for as long as it is parked, and the press that asked
  for it sits unread in the latch.
- **Where it bit.** OT-66 L2, found by the pre-merge AAG on #208. The WM locked by swapping the keyboard
  route to its own focus from its loops. Its replies are synchronous sends, so a client that sends a
  commit and doesn't receive the reply parks the WM, and every key typed after Super+L went to that
  client. Serving the lock from the WM's waits (#208's F2/F3) covers the waits the WM runs; it can't cover
  a wait inside the kernel.
- **Guard.** Make the mode kernel state, set on the event path itself. L2b's `focus::LOCK_ROUTE` is one
  word (the registered lock focus, and a locked bit; transitions in `sak_logic::lock_word`). The SAK
  filter sets the bit when it latches Super+L, with one compare-exchange loop and no lock, so it holds
  before any user code runs. Every route read (`route_event`, `flush`, a client's RECEIVE and POLL) goes
  through `input_route`, which returns the lock focus while locked, and a pointer record can be forwarded
  only to the lock focus (checked under `DELIVERY`). The WM registers, locks on idle, and unlocks through
  `SYS_INPUT_SAK_TAKE`'s lock ops (input-system authority only). Unlock makes the restored owner the route
  and clears the bit under `DELIVERY`, with the activation epoch and the lock epoch bumped. A dying lock
  focus while locked leaves no route at all (fail closed): it clears the active route before it publishes
  the unlocked word, so no lock-free route read sees the client's route in between.
- **What the guard is, exactly (Jarvis #160 items 1, 6, 7, 10).** The bit is set outside `DELIVERY`, so
  it can land between any route check and its effect. What holds:
  - A RECEIVE or POLL whose route read comes after the bit's store gets nothing. A RECEIVE already past its
    check re-checks after its pop and returns nothing if the bit is set by then; one whose re-check came
    first linearizes before the lock (it returns a key queued before the chord, and can't return anything
    else).
  - A key or record can still be ENQUEUED to a client after the bit is set: a flush, route or forward that
    read the route or passed its lock check before the store. It can't be read while locked (the receive
    check), and it can't be read after: every client queue carries the lock epoch it was filled under,
    every end of a lock (unlock, fail-closed drop) bumps it under `DELIVERY`, and a queue from an earlier
    epoch is emptied before it is next read or written. So nothing a client's queue held across a lock
    reaches the client, whether it was queued before the chord or raced it.
  - A POLL's "ready" can be stale (it delivers nothing; the RECEIVE that follows checks again).
  - This is destination isolation, not delivery: contention, an epoch mismatch, overflow and the unlock's
    emptying all drop input.
- **Ordering (same device).** A flush already holding `DELIVERY` may have passed its epoch check when the
  bit is set. Its events were decoded before the chord on their own device (a device's decode and flush
  share its own state), and a flush reads the route after its epoch check, so a key decoded after the
  chord on the same device always goes to the lock focus.
- **Discriminator.** `login_lock_park`: Arthur sends a commit and polls its keys for 12 s before it
  receives the reply, so the WM is parked in that send; Super+L, then `x`, during the park. Arthur's only
  keystroke is the one that began the park, and the lock screen comes after the park. Built without the
  kernel half (the WM falls back to its own swap), Arthur gets `x`. `lock_route_races` forces Super+L, on
  one CPU, at an armed point (`focus::race`) between each route check and its effect: between a receive's
  check and its pop (item 1), between a forward's lock check and its enqueue (item 6), a key queued before
  the chord, and a route read between a dying lock focus's two stores (item 7). Reverting each fix fails its
  leg (`recv_returned_key_after_latch`, `forward_record_crossed_lock`, `drop_reopened_client_route`). Host
  KATs for the word's transitions against hand-laid words.
- **Source.** Ours (the L2 pre-merge AAG, PM's Claude lens; the PM chose the kernel route as the fix). The
  principle is the usual one for a secure attention key: the system's response to it can't depend on the
  cooperation of anything the user may be fighting.

### CP-98 — A kill that assumes its target is Ready or parked

- **Shape.** A force-kill first claims the process (`exit_state` RUNNING → EXITING_FORCED), then looks for
  its thread in the places it knows and asserts it found it. Any state outside those places breaks the
  assertion. The product kernel keeps debug assertions on, so the kernel panics. The window is real and
  user-reachable: any holder of a `ProcessHandle` (a WM, any spawner) can kill a process that is exiting.
- **The states a kill's target can be in** (default build: user threads are spawned on, and woken onto, the
  CPU that spawns or wakes them; nothing pins a kernel-side killer, or the target, to the BSP):
  1. **Ready** (in a ready queue): `kill_thread_by_pid` zombifies it. Inside a syscall that is CP-95 (#204).
  2. **Parked in a channel, or in a token slot:** extracted from the drained handles, or by
     `zombify_parked_thread_by_pid`.
  3. **Exited, not yet reaped:** after `sys_exit` (or a fault kill) the thread is on its way to the zombie
     queue, or in it until the BSP's main loop reaps. The natural exit claimed `exit_state` only at that
     reap, so the kill's claim won and no scan found the thread. **This is S1's A1a case:** the WM closes
     a client's channel, the client sees EPIPE and returns 0, and the WM's `sys_kill` lands before the reap.
  4. **Parked in `sys_wait`:** the waiter sits in the *target of its wait*'s `waiters` queue, which none of
     the kill's scans read. Before OT-98, only the browser-runtime fault kill extracted it. **FAIL-A** (the
     PM's AAG): that target may itself be mid-`terminate`, moved from `PROCESSES` to `DYING`, so a scan of
     the registry alone misses the waiter.
  5. **Running on another CPU, or Ready inside a syscall:** no scan can take it. #204 counts it as a
     survivor (`kick_kill_survivors`), kicks it and defers the kill's completion to its reap (CP-95). This is
     not browser-runtime only: S1's SMP=4 run of `kill_at_exit` panicked in a race round where the kill ran on
     one CPU while the child was Running on another, on its way into `SYS_EXIT`.
  6. **In transit from a `sys_wait` queue to Ready.** **FAIL-B** (the PM's AAG, and Janus #174 F2): the
     completion took the waiters out of the queue and woke them after releasing its lock, so for a while a
     waiter was in no queue; a preempted reaper on one CPU makes the window as long as it likes. And even
     with the wake under the lock, the waiter can be woken after the kill's first survivor count and before
     its `sys_wait` scan, run to its syscall return, read the kill and exit before the recount (Janus #174
     F2's cross-CPU schedule, made deterministic by leg D).
- **The exit code (Janus #174 F1).** Two exit producers of one process can both be past their entry checks
  (two threads in `SYS_EXIT`, or a fault exit racing one). The first version stored the code before the
  claim, whatever its outcome, so the loser overwrote the winner's code.
- **With the assertion compiled out:** state 3 completes the teardown in `terminate` and the reap then
  finds no process, so nothing leaks and nothing is freed twice, but the waiter gets ESRCH instead of the
  exit code. State 4 leaves the killed thread parked; when its target exits it is woken into a process that
  no longer exists, and if the target never exits it stays parked with its address space forever.
- **Guard.**
  - Claim the natural exit where it is committed, not where it finishes: `record_exit_code` (the `sys_exit`
    and fault-kill path) does the RUNNING → EXITING_NATURAL claim, so a kill that loses returns ESRCH, and
    the reap (which accepts the earlier claim) delivers the code. Under `browser-runtime` the last thread out
    through `SYS_THREAD_EXIT` claims too (every thread record done or reaped).
  - Only the winning claim stores the code (F1). The CAS and the store are one section under `PROCESSES`;
    the store precedes, in the winner's program order, its thread's move to Zombie (published under its
    CPU's scheduler lock), and the reaper reads the code only after taking that lock to pop the winner's
    zombie, so the store happens-before the read.
  - `terminate` extracts a `sys_wait`-parked target in every build, scanning `PROCESSES` and `DYING` (FAIL-A;
    a process moves between them in one section, so it is always in one).
  - The waiter drops its owned `Arc<Process>` under the target's `waiters` lock before it parks (never the
    last one: every teardown records the completion under that lock before it drops its own), so the raw
    free leaks nothing (CP-30).
  - The completion wakes its waiters under the `waiters` lock (FAIL-B), so a waiter is in the queue or in a
    ready queue, never in neither.
  - **The kill's completion counts threads, not scan results** (the PM's bounce on `66904ae1`, item 1; #204).
    Every user thread's process anchor counts in `Process::live_threads` until `Thread::free`. A killed
    process in `DYING` completes (waiters woken, EXITED published) once its teardown is done and the count
    is 0; the reaper re-checks after every free. The first version decided at the kill, from its survivor
    count, whether to wait for a reap: a survivor reaped between the count and the `DYING` insert hung every
    `sys_wait` on the process.
  - After its scans, `terminate` counts survivors again (a thread woken since the first count), then looks
    for a thread that left the kernel meanwhile: in a zombie ring, popped and being freed by the reaper
    (`scheduler::reaping`, item 2: the window between the pop and the free), or freed (the count is 0). The
    thread passes through those in the order they are read, each move published before the next read.
  - `close_peer` wakes its extracted waiters under the endpoint guard (item 3), as the `sys_wait` completion
    does.
  - **The accepted residual** (the PM's re-review, note 1): since the completion counts threads, a thread
    that never reaches its reap (parked where no kill scan reads, or in a device wait that never completes,
    OT-99) leaves the kill's `sys_wait`ers parked, silently, where the old assertion would have panicked the
    kernel. No unscanned user park is known; the device wait is OT-99's.
  - **What the scans can't find is a diagnostic, not a panic.** The assertion ("neither Ready nor parked")
    is gone: the product kernel keeps debug assertions on, and a thread can be between two places no
    snapshot reads (the send handoff, `handoff_to_rendezvous_peer`, still is: it can't hold the endpoint
    guard across its switch). Such a kill is still correct, by the count; it increments
    `note_kill_unlocated`, which `kill_at_exit` requires to stay 0 after every leg.
  - CP-91 (Janus #174 F3): `SYS_KILL` runs `terminate` inside `without_interrupts`. The `sys_wait` scan's
    clones, `terminate`'s early-return clone and `finish_kill`'s last reference go through
    `release_scan_pin`/`release_last_process`, which hand a last `Arc` to the reaper's deferred drop when
    interrupts are off; the deferred list is drained only with interrupts on.
- **Discriminator.** `kill_at_exit` (`NEO_SMP=4`), with stand-ins that carry process anchors. The harness
  kills on CPU 0 (checked); legs E, F and G kill from CPU 1 through the real `SYS_KILL` dispatcher (its
  interrupts-off `terminate`). After every leg, no kill may have gone unlocated.
  - leg E (item 1): the survivor, held Running in `SYS_EXIT` on CPU 2, is released in the kill's pause after
    its survivor count, exits and is reaped before the `DYING` insert; the wait on it must complete
    (`-control-e`, the pre-count rule: the wait hangs, `kae_e_wait_hung`);
  - leg C: a child on CPU 1 held Running inside `SYS_EXIT` before its claim; the kill counts it as a
    survivor, and the waiter gets ESRCH only after the child's reap (`-control-c`, no survivor count);
  - the exited leg: a child's thread waits unreaped in a zombie ring, the kill returns ESRCH, and a later
    waiter gets exit code 7 (`-control`: claims at the reap and skips the extraction, as before OT-98; the
    kill wins and returns 0, `kae_kill_of_exited_not_esrch`);
  - the wait leg: `terminate` of a process parked in `sys_wait`, then of its target, frees both processes
    and their address spaces;
  - leg D (the completion-to-Ready barrier): P's wait completes inside `terminate(P)` between the first
    count and the `sys_wait` scan, and P exits at its return (half the rounds also reaped) before the scans
    end; a wait on P completes only after P's reap (`-control-d`, without the left-the-kernel check);
  - leg A (FAIL-A, `-control-a`) and leg B (FAIL-B, `-control-b`), each a pause inside the kill or the reap;
  - leg F (item 2): the reaper pauses between popping P's zombie and freeing it while the kill scans
    (`-control-f`, without the reaping mark);
  - leg G (item 3): `close_peer` pauses with P extracted, before its wake, while the kill runs
    (`-control-g`, the old wake after the guard);
  - the F1 leg: two exit producers (two `SYS_EXIT`s, or a `SYS_EXIT` and a fault exit) on CPUs 1 and 2,
    held before the claim and released in turn or together; one wins, and the waiter gets its code
    (`-control-f1`, the old store-then-claim: `kae_f1_loser_changed_code`);
  - 200 kill-at-exit races, each 0 or ESRCH, with no leak.

  `-control-a` to `-control-d`, `-f` and `-g` fail `kae_kill_unlocated`, each in its own leg.
- **Source.** Ours. Principle: a kill must agree with every exit path on one claim, made at the point the
  exit becomes irrevocable, and must know every place a live thread can be, including the moves between
  places; an assertion over a list of places is only as good as the list. The payload of a claim is written
  by its winner only.

### CP-99 — A kernel copy to or from user memory with no fault recovery

- **Shape.**
  - A syscall checks that a user range is in the user half (`validate_user_buf`), then copies with an
    ordinary load or store inside `with_user_access`.
  - The check says nothing about whether the pages are mapped or writable. The kernel can't make it say
    that: another thread of the same process can `vm_unmap` or `vm_protect` the page between the check and
    the copy.
  - The resulting ring-0 page fault reaches the handler's kernel-mode branch, which halts. So one syscall
    from any process stops the machine.
- **Why translating first is not a guard.** A walk of the page tables before the copy only moves the
  window (check-then-use). The fault itself has to be survivable.
- **Guard (`user-copy-fixup`).**
  - The kernel touches user memory through one routine, `arch::x86_64::user_copy::copy`: a `rep movsb`
    at the symbol `neo_user_copy_insn`, followed by a fixup that returns RCX (the bytes left).
  - The page fault's entry shim checks before it calls into Rust. For a CPL-0 fault whose saved RIP is
    that instruction and whose CR2 is below 2^47, it rewrites the saved RIP to `neo_user_copy_fixup`, drops
    the error code and `iretq`s.
  - The shim saves and restores the one register it uses. The CPU frame restores the copy's RFLAGS (AC=1
    in its SMAP window, its IF). At CPL 0 there is no GS to swap (CP-44).
  - Every other fault takes the old path, so a kernel bug still halts. That includes a fault in the
    routine on a kernel-half address.
  - Prior art, read for principle only: Linux's exception table (`__ex_table`, GPL), FreeBSD's
    `pcb_onfault`, and Redox's `arch_copy_to_user` RIP range (MIT).
- **SMP.**
  - Thread A copies while thread B unmaps the page. B's `vm_unmap` clears the PTE and shoots down the TLB
    on every CPU running the address space before it returns.
  - So A either completes the copy before the shootdown, with the old translation, or faults after it and
    gets EFAULT.
  - The copy runs with the SMAP window open, never across a switch (CP-49), and holds no lock the fault
    path takes. The fixup path takes none.
- **Discriminator.**
  - `user_copy_fault` drives SYS_HANDLE_INFO and SYS_READDIR (its entry and its cursor) with an
    unmapped, a page-crossing and a read-only output buffer, and SYS_WRITE with an unmapped and a
    page-crossing input buffer (a read-only input is readable, so valid). Each must return EFAULT, and
    then a well-formed call must work.
  - Its control, `neo-test-user-copy-fault-control`, turns off the shim's recovery. Its first leg then
    halts (`unexpected_trap trap=page_fault`, CR2 the unmapped page).
- **The other sites (cross-family review of PR #13).**
  - Futex's compare-before-park reads its 32-bit word with `user_copy::try_load_u32`: one aligned
    `mov eax, [rdi]` at `neo_user_load_u32_insn`, the shim's second registered RIP, sharing the fixup (RCX
    is 4 before the load). The compare stays a single access. A fault returns EFAULT from `wait_on_key`,
    unparked, and the queue guard drops normally.
  - The IPC rendezvous copies between two address spaces. The side live on this CPU goes through the
    routine at its user VA. The other side goes through the HHDM inside `Pmap::with_user_frame`, which
    takes that Pmap's `inner` lock, resolves the page with `Pmap::translate_user` (present and user at
    every level, a 4 KiB leaf, and writable for a destination) and holds the lock until the chunk is
    copied. A receiver's read-only page is EFAULT, where the HHDM copy used to write it, and a sibling of
    the peer that unmaps the buffer waits for the copy: every path that frees a mapped frame clears its
    PTE under `inner` first (CP-37). Lock order: channel guard → a peer Pmap's `inner`; with neither side
    live (harness callers), a bounce buffer takes one Pmap's lock at a time.
  - `tail_hold_for_test` checks its flag mapped user-writable in the page tables when armed (no write)
    and writes it through the routine.
- **No partial state on EFAULT (the paths it covers).** A fault recovered mid-syscall leaves no kernel
  state committed on these paths, because the user copy happens before the commit, under the same lock:
  - SYS_READ advances the offset only after its copy (`OpenFile::read_as_then`).
  - SYS_INFER_RUN closes the output handles it installed.
  - SYS_THREAD_JOIN writes the status under the registry lock before it collects the exit record, and
    un-claims it if the write faults.
  - SYS_SOCKET_RECV and SYS_SOCKET_RECVFROM copy out (address, its length, data) under the socket lock
    before the bytes or datagram leave the queue (`TcpSocket::recv_stream_with`,
    `Socket::recv_from_with`); an address buffer too small for the source is EINVAL with the datagram
    still queued.
  - SYS_NET_TCP_ENABLE_TLS wipes the caller's key before installing TLS. Its pages are checked writable
    in the page tables first, so a key that is partly read-only stays whole; a sibling's unmap after that
    check can still leave it half wiped, with TLS not installed.
- **Outside the guarantee.** SYS_INPUT_FOCUS_RECV pops an event that can't be put back (Super+L's
  re-check must follow the pop, so the copy can't go first), and stdin SYS_READ takes bytes from device
  FIFOs. Both check their buffer in the page tables first (`check_user_writable`, no write), so a static
  bad buffer is EFAULT with nothing taken; a sibling that unmaps the buffer after the check makes the
  event or bytes lost. Neither has a scenario leg.
- **Discriminator.**
  - `user_copy_fault` also drives SYS_READ (an unmapped, a crossing and a read-only buffer, then a
    well-formed read that must return the file's first bytes), SYS_READDIR with a read-only cursor,
    futex waits on an unmapped word and on a word whose page the scenario kernel unmaps between the
    translation and the re-check, IPC rendezvous with read-only, unmapped and crossing buffers in both
    directions, a join with a faulting status pointer (the thread must still be joinable), TLS with a
    read-only and a half read-only key (the writable half must be unchanged), and UDP receives into an
    unmapped and a read-only buffer and with a read-only address buffer (the datagram must still arrive
    whole). These use static bad buffers; the TCP receive has no leg (it needs an established
    connection).
  - Its parallel-unmap IPC leg pins a sender, a receiver and an unmapping sibling to CPUs 3, 1 and 2. The
    scenario kernel holds a 72-byte copy open until the sibling's unmap is waiting on the peer's lock, then
    a while longer (both waits bounded in TSC cycles, inside the TLB shootdown timeout), and records
    whether the peer's page stayed mapped; the harness requires it (7).
    Revert-checked: releasing the lock before the copy fails it (`ipc_unmap_race result=3`).
  - The control's halt is recorded in `Plans/EVIDENCE-2026-11-01-pr13-user-copy-fault-control.serial.log`.
  - `threads_exit_group` role 6 arms the tail hook with an unmapped word and expects EFAULT.
- **Open.** Input-focus receive and stdin (above): a sibling's unmap after the check loses the event or
  bytes; the loss stays inside that process.

### CP-100 — An XSAVE area sized from CPUID.(0DH,0):EBX alone

- **Shape.**
  - `finish_xsave_init` set XCR0 to x87|SSE|AVX, then took the XSAVE area size from CPUID.(0DH,0):EBX.
  - EBX is defined as the size for the features *currently enabled in XCR0*. A hypervisor that answers CPUID
    from its own idea of XCR0 can get it wrong. QEMU under Windows Hypervisor Platform (WHPX,
    `kernel-irqchip=off`) answered 576, the x87|SSE size, after the guest had enabled AVX.
  - `xsave` with RFBM = x87|SSE|AVX then writes the AVX component at offsets 576..832, past the area's end.
    In the timer stub the area sits just below the anchor slot, the 15 saved registers and the iretq frame
    copy, so they were overwritten; the stub's `iretq` popped a null CS and took a #GP(0) on the first LAPIC
    tick (`timer_isr_entry+0xe0`). Each thread's heap-allocated save buffer, sized the same way, overran
    its allocation on every switch.
  - TCG (and real CPUs) answer EBX = 832, so CI never saw it.
- **Guard.** `fpu_logic::xsave_area_size`: the largest of EBX, the legacy region plus header (576), and, for
  each component i >= 2 enabled in XCR0, CPUID.(0DH,i):EBX (its offset in the standard format) plus
  CPUID.(0DH,i):EAX (its size), rounded up to 64. The per-component values are static properties of the
  CPU model, not of XCR0. The boot log names EBX and says when it was understated.
- **How it was found.** The preview's photo gave `rip=…9d79b01c`. The exact preview kernel (`bef433fa`) was
  rebuilt with `cargo xtask image-kernel-build`; Limine's KASLR slide is page-aligned, and the only
  instruction at page offset 0x01c that can #GP(0) was the timer stub's `iretq`. Its boot line
  `FPU save=xsave area=576 mask=0x7` named the cause. The `#GP` line now also prints `link_rip=` (the
  KASLR slide removed), so the next photo maps straight to the ELF.
- **Discriminator.** `xsave_ebx_understated` forces EBX to 576 and runs `xsave_ymm_context_switch`'s legs
  (YMM/XMM/x87 preserved across a context switch, timer ticks live); it must pass with area 832. Its
  control, `neo-test-xsave-ebx-understated-control`, trusts EBX and takes the same #GP at
  `timer_isr_entry+0xe0` under TCG. Host KATs (`tools/fpu-logic-tests`) check the size against the SDM's
  component offsets.
- **Related, not fixed here.** `timer_isr_entry` stashes r10/r11 in one global `TIMER_SCRATCH` for all
  CPUs (its comment says "Phase 2+ SMP will need per-CPU scratch"): two CPUs entering at once can hand each
  other's interrupted threads the wrong r10/r11.
### CP-101 — Withdrawn: loopback pump reordering (not the cause)

- **Status.** Withdrawn 2026-10-31. Filed in 27ce30f0 to explain `fetch_broker`'s lost first request (OT-61
  rung 5c) as two CPUs draining `tcp::pump_loopback_egress` and injecting one connection's segments out of
  order. That was an inference, not an observation.
- **What the trace showed.** A per-segment trace in `inject_segment` (each segment's seq, length, flags and
  the receiver's `rcv_nxt` before and after) caught failing runs with every segment, the request included,
  delivered in order and accepted. The request reached the server's socket; the server lost it in user space.
- **The real cause.** When the client's Finished and its request arrive in one read, rustls (unbuffered API)
  completes the handshake in `ReadTraffic` and counts the yielded record as consumed. The fixture server took
  that state as "done" without reading it, so the request went out with the handshake bytes. Fixed in
  `libneo::tls` (dcf91989); `LOOPBACK_PUMP` removed; 8 of 8 first attempts pass.
- **Left open.** Pop-then-release-then-inject from many CPUs could still reorder in principle. Nothing has shown
  it, and the number stays reserved so it isn't reused. Anyone who sees an out-of-order loopback segment drop
  (`tcp: loopback segment dropped` stays in the log) should reopen it with a trace.
- **Lesson.** Instrument before guarding: a lock added on an inferred race "fixed" a flake by timing alone.

### CP-105 — Stopping the other CPUs: an in-flight shootdown, a CPU that never stops, a lock a parked CPU holds

- **Shape.** A shutdown (or panic, or kexec) path stops every other CPU with an IPI, an NMI as the
  fallback, then carries on as if it were alone. Three traps, each independent:
  1. **The stopper stops answering too early.** It masks interrupts before sending STOP. A sender already
     in its IF=0 ack wait has targeted the stopper; the stopper can't ACK, the sender can't take STOP, and
     the sender's shootdown deadline (50 ms) fires before the stopper's NMI escalation (CP-64's shape, on
     the shutdown path).
  2. **A CPU that never stops is assumed stopped.** Some CPU takes neither the STOP IPI nor the NMI (an
     earlier NMI still running blocks the second; a wedged CPU). Code after the stop then forces a lock
     open, programs a shared register pair (I/O APIC index/data, PCI CF8/CFC), or waits on a lock that CPU
     holds. A wait bounded only by a timer that has stopped (a PM timer reading a constant) never reaches
     the fallback at all.
  3. **A parked CPU holds a lock.** STOP is taken only with interrupts on, so its parks hold no IRQ-safe
     lock, but they can hold a plain spin lock taken with interrupts on; an NMI park can hold anything. Any
     lock the stopper takes afterwards (the UART's, a cached plan's) may never come free. **That includes
     the heap allocator's lock, taken by any drop of a heap value** (a `Vec` going out of scope), so a
     plan built before the stop and dropped after it hangs the stopper before the power-off.
- **Found by.** Janus's cross-family review of OT-90 P3b-1 (#6), items 1, 2 and 5; the allocator-lock case
  and the per-step waits by his review of P3b-2 (#7), items 2a and 3.
- **Guard (OT-90 P3b-1).**
  - The stopper takes the TLB-shootdown sender lock with interrupts on (waiting out and acknowledging any
    sender in flight), then masks interrupts and leaves the ready set, and never releases the lock
    (`tlb_shootdown::close_for_stop`). A CPU that later tries to send waits with interrupts on and takes
    STOP.
  - Each wait is bounded twice: the PM timer, and a poll count with a POST-port write per poll.
  - After the stop the stopper takes no shared lock: the UART is written lock-free (bytes may interleave
    with a holdout's), and the power plan is read from a copy taken before the stop.
  - With any holdout, the single-CPU steps (I/O APIC and PCI masking) are skipped and the path goes straight
    to the power-off or reset, which ends every CPU. The holdouts are reported.
  - **(P3b-2, K6.)** K6's plan (heap vectors, built before the stop) is held in `ManuallyDrop`, so no path
    after the stop frees it, the holdout return included. Every K6 wait is bounded by a poll count as well
    as the timer (`power_logic::wait_bounded`), so a frozen timer can't hold it either.
- **Discriminators.** `ot90_stop_shootdown`: an AP sends IF=0 shootdowns at every other CPU while K4
  starts; every AP must park by STOP (with the old mask-first order it fails 4/4, the sender parked only
  by the NMI). `ot90_stop_wedged`: the highest AP ignores STOP and, in its NMI handler, takes the UART lock
  and never acknowledges; the PM timer reads as frozen. K4 must end on its poll count, report that AP as
  the holdout, skip K5 and reach S5. `ot90_quiesce_heap_held` (P3b-2): the highest AP ignores STOP, takes
  the allocator's lock in its NMI handler and parks holding it; K6 must run and S5 must still come (with the
  plan dropped at the end of `stop_world` it hangs, 4 of 4). Host KAT `a_k6_wait_ends_on_a_frozen_pm_timer`:
  the real `pm_wait` on a timer reading 0, with a controller that never finishes.
- **Source.** Ours, with the principle from Linux's `smp_send_stop` / `crash_smp_send_stop` (read for
  principle only): the stop is best-effort, the caller must not assume it worked, and panic-time printing
  uses a lock-free console path.

### CP-106 — A C library's "unlocked until threads exist" streams, in a process whose threads aren't its own

- **Shape.** musl starts the standard streams unlocked (`stdout`/`stderr` have `lock = -1`) and turns locking
  on in `pthread_create` (`libc.threaded`, `__stdio_*` locks). That is sound in a process where only
  `pthread_create` makes threads. In a bundle process the threads are std's (`std::thread::spawn` over
  `SYS_THREAD_CREATE`), and any of them can call into C. neo-crt compiles none of musl's pthread code, so
  locking never turns on. Two threads in `printf` on the same stream then share `__fwritex`'s static buffer
  with no lock: a run with no newline overruns the buffer (two threads, a 600-byte string each). The
  outcome is memory corruption next to the buffer, not just interleaved text.
- **The general form.** A runtime that keys a safety mode on "has this program made a thread yet?" breaks
  when the program's threads come from somewhere the runtime can't see: a host language's thread API, a
  foreign pool, a callback from another runtime's thread.
- **Guard (#261, C3a).**
  - neo-crt's `c/stdout.c` and `c/stderr.c` are musl's with `.lock = 0`, so every `FLOCK` locks, with or
    without pthreads.
  - `__lockfile`/`__unlockfile` are Rust (`userland/neo-crt/src/lib.rs`): an owner table under a std
    `Mutex` and `Condvar`, keyed by the stream's address, with musl's contract (1 = this call took the lock,
    0 = this thread already holds it). musl's own versions need `__pthread_self()->tid` and its futex,
    which neo-crt doesn't have.
  - The other half of the same gap: libc++ is built `_LIBCPP_HAS_NO_THREADS` in C3a (`shared_ptr` counts
    not atomic, `call_once` unsynchronised), so no multi-threaded C++ until C3b's pthread rebuild (C3a THINK
    §6.1, a hard rule).
- **Discriminators (`cxx_runtime_smoke`).**
  - `ok(16)`: a thread holds stdout's lock through `__lockfile`, and a second thread's `fputs` to stdout
    must not return for 200 ms, then return once the lock is let go; the holder taking it again gets 0.
    Control: musl's `stdout.c` (`lock = -1`) makes the `fputs` return at once, `FAIL(16)`.
  - `ok(17)`: six threads print 600-byte runs with no newline at once (the bounce's repro).
- **Source.** The PM's AAG on #261 (2026-10-31). Principle: musl's `__stdio_*` locking and `libc.threaded`
  (`src/thread/pthread_create.c`), read for the design only.
