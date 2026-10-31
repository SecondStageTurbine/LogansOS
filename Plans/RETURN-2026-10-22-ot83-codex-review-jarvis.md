<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: OT-83 / PR #92 — Jarvis review

**Verdict: BOUNCE.** Item 4 has a concrete CP-37 deadlock through admission-cancellation logging. The CPL/GS/FP routing itself is sound. Item 8 also has a nonblocking documentation mismatch for the unreachable `#NM` fallback.

## Review identity and evidence

- Requested and reviewed head: `220f2a6bd2f2c5887f49154869c6028f3f170750`; merge base `9c8c3dea`.
- Review branch was cut from fetched `origin/main`, `4e9ea28ca3b9ef91e4e66904a0e59eacc6b9b8e0`.
- At fetch, the PR branch had advanced to `a6386ea6689787319e8a90f8f62eef2c071c5d23`. Its changes since the requested head are the main merge/product-build updates and FINDINGS evidence. The four core files, admission, logging, scheduler, and syscall code cited below are unchanged. All local file:line references below name **the requested head**, not this documentation branch.
- Read the complete dispatch, FINDINGS, implementation diff, entry stubs, teardown/reaper, admission cancellation, lock implementations, and probe/harness wiring. Examined the current `origin/main...origin/claude/ot83-ring3-exception-kill` diff as requested, and pinned conclusions against `9c8c3dea...220f2a6b`.
- Ran the five existing OT-83 host tests against source extracted from `220f2a6b`, preserving the original relative module/include paths: `rustc --edition 2024 --test .../tools/xtask/src/ot83_tests.rs`, then the resulting executable. **5 passed, 0 failed.** No test was added. `git diff 9c8c3dea...220f2a6b --check` passed.
- This is a source review plus host KAT run; I did not rerun the QEMU gate or hardware probes. The newer FINDINGS' green gate is author-supplied evidence, not a run performed by this review.
- RAG health was healthy (`ollama=true`, `qdrant=true`); used semantic queries for discovery, then direct pinned source reads. Graft's committed graph reported drift; its callers query refreshed against the local main checkout and was used only as a discovery hint.

## Blocking finding F1 — cancellation flush can deadlock the faulting CPU (P1)

**Review call site:** `kernel/src/sched/process.rs:2866`.

The exception kill calls `admission::cancel_pid` at IF=0. Removing a matching pending request calls `trace_withdrawn` **while `QUEUES` is still held** (`kernel/src/ai/admission.rs:353`, `:362`, `:369`, `:375`). That trace calls `crate::log::flush()` (`kernel/src/ai/admission.rs:412`). `flush` performs a blocking `SINK.lock()` (`kernel/src/log/block_sink.rs:617`), although `SINK` is a plain spin mutex (`:43`). It is not part of the stated IRQ-safe, bounded lock set.

**Concrete input and interleaving:**

1. Build with `browser-runtime`. Process P has sibling A parked in admission behind another principal's accelerator request, and sibling B ready to execute `ud2` in ring 3. This is a newly contained vector in OT-83.
2. On CPU 0, an unrelated kernel logger runs `serial_println!` with IF=1. `serial::_print` releases its UART IRQ-off section before calling the log sink (`kernel/src/drivers/console/serial.rs:115`, `:127`). Pause immediately after `block_sink::write_fmt` successfully obtains `SINK` at `kernel/src/log/block_sink.rs:514`, before it releases the guard. Keep `IN_FLUSH=false`.
3. A timer preempts the logger and selects B on that same CPU (`kernel/src/sched/scheduler.rs:1338`). B executes `ud2`. Its handler correctly swaps GS and calls the exception kill with IF=0.
4. The kill's initial print tries `SINK` and drops the contended log line, as designed. It then publishes `exiting`, withdraws A's admission request, and enters `trace_withdrawn -> log::flush -> SINK.lock`.
5. CPU 0 spins forever. The logger that must release `SINK` is suspended on CPU 0, and IF=0 prevents it being scheduled. `IN_FLUSH` never becomes true in this schedule. A peer shootdown targeting CPU 0 cannot receive its ACK and can escalate the local wedge to `tlb_shootdown_timeout`.

**No installed block sink is required.** `write_fmt` acquires `SINK` before checking whether its `Option` is `Some` (`kernel/src/log/block_sink.rs:514`, `:518`), and `flush` also acquires it before that check. With a sink installed, flushing additionally permits block I/O from this exception/`QUEUES` context; the lock counterexample already suffices without I/O.

This is a static execution-path witness, not a claimed runtime reproduction. The repository already documents this exact mixed-IF failure shape at `kernel/src/log/block_sink.rs:490`; the nonblocking repair in `write_fmt` was not applied to `flush`. The unsafe downstream path predates OT-83's browser `#PF` handling, but this PR adopts it for every newly contained exception. Calling it unchanged does not establish checklist item 4.

**Required follow-up:** keep CP-66 withdrawal and wake ordering, but remove blocking log flushing/I/O from exception cancellation. For example, defer the durable flush to normal context and ensure the cancellation trace only stages/drops output without waiting. Add a discriminator that suspends a `SINK` holder on the faulting CPU and then fault-kills a process with an admission waiter; require progress and a successful subsequent shootdown. Merely making the top-level fault print nonblocking does not close this path.

## Checklist answers (1–10)

### 1. PASS — CPL routing and ordering

`kernel/src/user_fault_logic.rs:110` and `:115` use the hardware-saved CS RPL. The new handlers call containment before their diagnostics: `kernel/src/arch/x86_64/idt.rs:590` (#DE), `:597` (#DB), `:630` (#OF), `:636` (#BR), `:642` (#UD), `:648` (#NM), `:660` (#NP), `:670` (#SS), `:692` (#GP), `:830` (#MF), `:836` (#AC), and `:844` (#XM). The #GP test hook at `:687` is the explicit exception and diverges. #PF retains its separate saved-CS check at `:750`, before `record_page_fault` at `:786` or any diagnostic.

Saved CS describes the CPL at which the fault/trap was delivered; the target selector being validated by a failed transfer is not substituted for it. Thus a fault in ring-0 `iretq` is saved as ring 0 even if its proposed return CS was 0x2b. SYSCALL enters ring 0 before the first instruction at `kernel/src/arch/x86_64/syscall.rs:667`; that first instruction is `swapgs` at `:669`. SFMASK clears TF at `syscall.rs:520`. Actual return uses IRETQ (`syscall.rs:756`, `:859`), not SYSRET, so there is no active noncanonical-SYSRET path to audit. Ordinary CPL-3 exceptions save RPL 3; they do not masquerade as CPL 0.

Boundary: this does not contain a kernel-mode fault caused by a malformed return frame. For example, an unassigned syscall with user RSP `0x8000000000000000` can fault at the kernel IRETQ; the saved CS correctly remains ring 0. Such return-path hardening is preexisting and is not claimed as repaired by this change.

### 2. PASS — exactly one GS swap on the contained path (CP-44)

The exception shims only mask AC/DF and transfer control (`kernel/src/arch/x86_64/idt.rs:83`, `:138`). The common path does pure routing at `:562`, then its sole `swapgs` at `:566`, before FP reset and the first scheduler/per-CPU read. It never returns. #PF's sole swap is `:754`; its kill does not call the common containment helper a second time. `fault_kill_current_user_exception` starts its scheduler access at `kernel/src/sched/process.rs:2822` and contains no swap.

The #GP CP-47/48 hook runs before common containment (`idt.rs:687`). `kernel/src/test_harness.rs:8639` reads the GS-base MSR directly, establishes kernel GS if needed, and ends through its scenario handler/debug exit; it cannot fall through into a second swap. A ring-0 fault in a syscall entry/exit GS window follows the existing fatal path, not this kill path.

### 3. PASS — ring-0 behavior is retained

For the twelve newly routed vectors, saved CPL 0 returns from containment, then the same diagnostics run and `kernel/src/arch/x86_64/idt.rs:582` calls `halt_forever` at `:586`. #PF retains `unexpected_trap`/halt at `:823`; #XM retains that split at `:849`. #TS, #DF, #MC, NMI, and returning #BP are unchanged. The ring0 control changes behavior only under its dedicated test feature. It proves #UD, not a runtime execution of every ring-0 vector; the broader conclusion comes from inspecting each branch.

For #PF, the U/S error bit still describes access privilege, not the page's U/S mapping. A supervisor SMEP/SMAP violation does not become a ring-3 kill merely because the target page is user accessible. The unchanged error-bit branch at `idt.rs:799` preserves the existing CPL-0 behavior for these accesses.

### 4. FAIL — hidden logging lock violates exception-context safety

**Concrete failure:** F1 above; `kernel/src/sched/process.rs:2866 -> kernel/src/ai/admission.rs:369 -> :412 -> kernel/src/log/block_sink.rs:617`. This defeats the bounded-lock/shootdown-ACK obligation.

The rest of the audited path has the following enforcing points:

| Operation / lock | Enforcing location and conclusion |
|---|---|
| Current pid / scheduler | `kernel/src/sched/scheduler.rs:1678`: local scheduler lock inside `without_interrupts`; user execution cannot already hold it. |
| Fault diagnostics | `kernel/src/drivers/console/serial.rs:115`: UART lock is IRQ-off; sink staging tries its outer lock at `kernel/src/log/block_sink.rs:514`. With a sink installed, staging may itself flush when full (`block_sink.rs:212`), so the documentation's unconditional allocation-free/no-I/O wording also needs qualification. |
| Process registry | `kernel/src/sched/process.rs:1921`, `:1589`: `PROCESSES` accesses are IRQ-off, and are released before admission/scheduler scans. The user thread holds no registry lock. |
| Futex / wait-any | `kernel/src/sched/futex.rs:104`, `:403`; `kernel/src/sched/wait_any.rs:78`, `:337`: `IrqSafeMutex` queues; purges remove registrations without holding an owned process/thread across exit. |
| Admission | `kernel/src/ai/admission.rs:353`: plain `SwitchMutex`, not intrinsically IRQ-safe. Live graph syscall entry/completion retain IF=0 around this lock; the interruptible compute restores its caller's IF before completion. `cancel_pid` holds QUEUES while waking (`:365`), preserving QUEUES -> scheduler ordering, but its trace adds F1. |
| Ready/token scans | `kernel/src/sched/scheduler.rs:2516`, `:2185`: take one CPU scheduler lock at a time and move owned thread boxes into zombie queues. They do not free Pmaps or wait for shootdowns while holding those locks. |
| Final exit / reaper | `kernel/src/sched/process.rs:2879`; `kernel/src/sched/scheduler.rs:1506`, `:2611`: switch away under the scheduler release-on-new-stack discipline, then free off-stack. Full process teardown is `kernel/src/sched/process.rs:2379`. |

CP-30 itself passes: the temporary process Arc ends at `kernel/src/sched/process.rs:2854`; the caller holds only scalars at `:2879`. Scheduler boxes transfer ownership to queues instead of staying on the abandoned exception frame. CP-66 cancellation precedes the scans (`:2866`). Ready and token scans skip admission/lifecycle participants via `kernel/src/sched/scheduler.rs:321`, `:1275`, `:1393`; those participants unwind and exit at the inference syscall boundary (`kernel/src/arch/x86_64/syscall.rs:2855`). CP-67's channel owner lives in the thread anchor (`scheduler.rs:1426`), so IPC extraction does not strand a stack-owned channel Arc.

**Inherited limitation, not a newly found blocker:** running siblings are not retired by these Ready/parked scans. `docs/concurrency-pitfalls.md:1343` explicitly tracks CP-36's containment gap. Process anchors prevent UAF; they do not prove prompt whole-process termination. For example, a sibling running a register-only loop on another CPU can survive the fault-kill scan. The new single-threaded probes do not test that gap.

### 5. PASS — feature-off processes have one thread and teardown runs

`kernel/src/sched/process.rs:2182` allocates a new process/pid and creates its single initial user thread at `:2229`. The second-thread constructor body is behind `browser-runtime` (`:2503`), as are its dispatcher arm (`kernel/src/arch/x86_64/syscall.rs:934`) and `Thread::new_user_with_arg` (`kernel/src/sched/thread.rs:499`). Examined all `Thread::new_user` callers; the additional direct constructors are scenario code, not a second production thread-creation ABI. No default-build fork/clone path was found.

Without the feature, reaping calls classic `on_thread_reap` (`kernel/src/sched/scheduler.rs:2617`); that path reads the recorded exit code, completes waiters, unregisters, and drops process resources (`kernel/src/sched/process.rs:2404`, `:2409`, `:2418`, `:2425`). The kill sets `exiting`, not `exit_state`, so it does not accidentally prevent the natural-reap state transition at `:2388`. No threaded refcount drain is required in this configuration.

### 6. PASS — faulting FP control does not flow into teardown/next thread

`kernel/src/arch/x86_64/idt.rs:570` resets control before the generalized kill; #PF does so at `:803`. `kernel/src/arch/x86_64/mod.rs:76` uses **FNINIT**, not waiting FINIT/FWAIT, then loads MXCSR 0x1f80 (`:83`). This clears pending x87 state, masks exceptions, and resets rounding/status; the pre-reset route/shims perform integer/control work, not FP arithmetic.

Without `xsave-fpu`, the reset remains live through the legacy context switch (`kernel/src/arch/x86_64/context.rs:37`); first user entry also loads the clean image (`:250`). With `xsave-fpu`, switch restore loads the next thread's image (`context.rs:124`, `:186`); initial images are built from constants (`kernel/src/arch/x86_64/fpu.rs:129`). A resumed thread may intentionally restore its own user state, but cannot inherit the killed thread's pending exception/control word. The killed exception frame is never resumed. This conclusion is a source proof; QEMU's missing #XM delivery prevents claiming an end-to-end #XM hardware test here.

### 7. PASS — NE is CPU-local and set on BSP and APs

`kernel/src/arch/x86_64/mod.rs:135` sets CR0.NE in `enable_sse`. BSP calls it at `kernel/src/main.rs:543`, APs at `kernel/src/arch/x86_64/smp.rs:242`. Both paths subsequently initialize FP control before normal scheduling. Searched kernel architecture/AI code and inspected FP assembly: production uses FNINIT, FXSAVE/FXRSTOR or XSAVE/XRSTOR and SSE; there is no production waiting x87 arithmetic/FLDCW/FWAIT sequence to newly deliver a pending user #MF in ring 0. The intentional FLDCW/divide/FWAIT sequence is in `userland/fault-probe/src/bin/mf.rs:17`, in ring 3.

### 8. FAIL (nonblocking claim mismatch) — reachable codes match; #NM is not Linux's policy

The implemented values at `kernel/src/user_fault_logic.rs:85` are 132 (#UD), 133 (#DB), 135 (#NP/#SS/#AC), 136 (#DE/#MF/#XM/#NM), and 139 (#OF/#BR/#GP/#PF). The reachable fatal vectors match Linux's signal choices. Checked the independent [Linux v6.18 trap handlers](https://github.com/torvalds/linux/blob/v6.18/arch/x86/kernel/traps.c), including the SIGBUS cases and FP handlers.

**Concrete counterexample to the literal Linux claim:** `route(Vector::DeviceNotAvailable, 0x2b)` returns kill 136 (`user_fault_logic.rs:91`, `:115`). On Linux, #NM is generally repaired/restored rather than translated to SIGFPE; for example its TS path clears CR0.TS and resumes. Its unexpected #NM path calls `die`, whose fallback signal is SIGSEGV, not SIGFPE ([Linux handler](https://github.com/torvalds/linux/blob/v6.18/arch/x86/kernel/traps.c#L1425), [die](https://github.com/torvalds/linux/blob/v6.18/arch/x86/kernel/dumpstack.c#L434)). Thus 136 is a Neo defensive fallback, not a faithful Linux #NM mapping. Since #NM is unreachable in the supported running configuration, this is a documentation/policy nit, not the reason for BOUNCE. Label the exception explicitly instead of claiming the whole table is Linux-derived.

Compatibility passes: `kernel/src/sched/process.rs:2770` retains page-fault code 139 and the existing wrapper at `:2775`. `record_exit_code` and waiter completion accept arbitrary i32 statuses (`:1921`, `:2409`). The old 139 assertions are specifically guard-page tests (`userland/neo-init/src/main.rs:3524`), whose #PF behavior remains 139; no production consumer was found assuming every exception must be 139.

### 9. PASS — scoped probe coverage/reachability; hardware gaps remain explicit

`tools/xtask/src/main.rs:1531`, `:1540`, `:1549`, `:1570`, `:1591`, `:1599` register the five ring-3 probes plus ring-0 #UD without ignoring them. #SS and #XM are explicitly ignored at `:1561` and `:1582`; their probe bodies are `userland/fault-probe/src/bin/ss.rs:17` and `xm.rs:18`. The harness requires the expected exit and then a second process exit 0 (`kernel/src/test_harness.rs:2076`). The pure KAT tests routing, not CPU exception delivery.

The relevant QEMU claim is about **these probes**, not an assertion that TCG has no #SS implementation anywhere. Independently inspected [QEMU v10.2.3 canonical-address checking](https://github.com/qemu/qemu/blob/v10.2.3/target/i386/tcg/system/excp_helper.c#L590): it selects #GP without an SS-access distinction. Its [SSE scalar helpers](https://github.com/qemu/qemu/blob/v10.2.3/target/i386/ops_sse.h#L487) run softfloat division without delivering #XM for the unmasked arithmetic result. These support keeping the two probes out of the TCG gate. I did not independently reproduce the stated local 8.2.2 #PF observation; upstream 8.2.2 also has a generic noncanonical #GP check, so keep that observation tied to the author's actual local binary/log rather than generalizing it to every 8.2.2 build.

INTO/BOUND are invalid in 64-bit mode; software INT attempts hit the DPL-0 gates and become contained #GP instead. There is no present user compatibility code descriptor (`kernel/src/arch/x86_64/gdt.rs:133`). MPX bounds state is not enabled (`kernel/src/arch/x86_64/fpu.rs:22`). CR0.AM is cleared (`kernel/src/arch/x86_64/mod.rs:130`); EM is cleared at `:126` and TS is not set by any runtime path, so ordinary user #AC/#NM are unavailable. This does not claim coverage of unsupported facilities such as firmware-enabled split-lock detection or XFD/AMX.

#NP is routed (`kernel/src/arch/x86_64/idt.rs:660`), even though the installed usable user descriptors are present. #TS would require broken kernel TSS/transition setup; long mode does not expose user hardware-task switching. #DF, NMI, and #MC are system/hardware failures, not ordinary process faults. #BP remains returning and its DPL-0 software gate sends a user INT3 attempt through #GP. No reachable ordinary CPL-3 exception in this configured feature set is left on an unconditionally fatal, unrouted handler. The known running-sibling gap is separate from vector coverage.

### 10. PASS — default-build diff is confined to containment and its FP prerequisite

The production delta is unconditional exception routing/kill, unconditional #PF containment, the pure routing module (`kernel/src/main.rs:82`), and CR0.NE (`kernel/src/arch/x86_64/mod.rs:135`). NE deliberately changes unmasked user x87 errors from a potentially lost legacy FERR#/IRQ13 event into #MF; it is a prerequisite of the stated containment behavior, not an unrelated change. FP-control reset is on the newly one-way kill path. Scheduler/reaper algorithms, syscall numbers, and default feature selection are not changed by this lane.

The new scenario bodies are feature-gated. Probes are built/staged only when `fault_probe_stage` selects a scenario, and ordinary builds clear that selection (`tools/xtask/src/main.rs:4975`, `:5349`, `:6034`). The workspace/Cargo.lock additions register the test package but do not add probes to product modules. The later main merge's product LAPIC settings are not an OT-83-only delta.

## Disposition

Fix F1 and rerun a contention discriminator plus the existing host/QEMU gates before merging. Correct or qualify the #NM Linux attribution and retain CP-36's explicit running-sibling limitation. The existing green single-threaded exception probes do not exercise admission cancellation with a suspended logger, so they do not refute F1.

This review changes only this RETURN file. Existing `.gitignore` and `.ignore` edits in the worktree were preserved.
