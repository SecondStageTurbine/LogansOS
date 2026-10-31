<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: CP-46 error-code exception stack-alignment cross-check (Janus/Codex)

**Verdict:** MERGE

**Reviewed tree:** `origin/main` at `1a840e54b1fb80aa626462867223aa69b6ece83a` (the CP-46 merge is `2779c20bd035c22ad60b6433418e52ece7755e2d`).

No surviving correctness finding was found. The merged implementation matches the CPU frame, the locked crate ABI, SysV alignment, and the repository's one-way page-fault policy. The two test limitations already recorded in CP-46 (the structural leg does not decode the call target, and only `#PF` has a behavioral leg) remain accurate residual-coverage notes, not merge blockers.

## Checks against the dispatch

### 1. Frame and nesting

- `Cargo.lock` pins `x86_64` 0.15.4. Its `InterruptStackFrame` is `repr(transparent)` over a `repr(C)` `InterruptStackFrameValue` containing five eight-byte slots: RIP, padded CS, RFLAGS, RSP, padded SS. Thus `ErrorCodeFrame { error_code, frame }` is exactly six qwords; the assertions at `kernel/src/arch/x86_64/idt.rs:120-123` pin size 48 and frame offset 8.
- All seven installed vectors are architecturally error-code vectors. `#DF` still installs the generated shim and then selects `DOUBLE_FAULT_IST_INDEX` (`idt.rs:219-220`); IST changes the stack source, not the error-code-first frame layout, so `rdi = rsp` remains correct.
- The CP-40 `pushfq`/mask/`popfq` sequence has zero net stack displacement. Interrupt gates keep maskable IRQs out. An NMI uses its own IST and its own entry mask; it cannot corrupt the outer scratch flags word, and it returns to the interrupted shim. This is not a new nesting exposure relative to the pre-CP-46 CP-40 shim.

### 2. Alignment and other entry families

- Each error-code shim at `idt.rs:137-156` takes the delivered pointer before changing RSP, rounds RSP down to 16, then `call`s. The body therefore enters at RSP congruent to 8 modulo 16 as SysV requires.
- A fresh build with nightly-2026-04-16 succeeded. `llvm-objdump` showed all seven shims as the expected mask, `mov rdi,rsp`, `and rsp,-16`, direct `call`, `ud2`; every direct call target matched its IDT vector.
- The same disassembly showed `breakpoint` (the only returning exception body) entering through the no-error shim and using nine register pushes plus a 0x100-byte XMM spill area before its calls, leaving call-site RSP 16-aligned. The remaining no-error handlers have the same compiler-generated convention (machine check differs only in its local-frame size).
- The handwritten `swapgs_irq_entry` and timer paths explicitly align before calling Rust (`idt.rs:301-303,327-328,389-482`; `timer.rs:136-146,268-277`). Both syscall variants likewise align before their Rust/FPU calls (`syscall.rs:690-706,790-813`). No other Rust-calling entry family was found with CP-46's error-code mismatch.

### 3. Never returns

- All seven bodies have the compiler-checked type `extern "C" fn(&ErrorCodeFrame) -> !` (`idt.rs:152`). Six print/dump and halt. `page_fault` either terminates in a neo-test `pass`/`fail`/`unexpected_trap`, fault-kills a browser-runtime user process via `fault_kill_current_user -> scheduler::exit`, or halts on the fatal path (`idt.rs:692-781`; `test_harness.rs:3879-3939,11523-11533`; `process.rs:2681-2726`). There is no user-copy fixup, demand-paging return, or probe-resume path.
- Losing the `x86-interrupt` GPR/XMM save is safe only because none returns to the interrupted context. The shim itself clears DF before compiler code, and CP-40's AC mask remains intact. Any future recoverable fault must use a different save/realign/restore/swapgs-back stub; the source states that constraint.

### 4. Kill path and stack use

- `page_fault` tests saved CS.RPL and performs `swapgs` before error decoding, CR2 reads, tracing, test hooks, or scheduler work (`idt.rs:692-713`). The fault-kill path still reloads kernel FP control before teardown (`idt.rs:753-762`) and reaches the same `fault_kill_current_user` implementation.
- Nothing on the kill path consumes the old compiler-generated interrupt frame shape: consumers receive only CR2/RIP values or `&InterruptStackFrame`. Observability and the CP-40 counters read the pinned frame fields, while scheduling switches away permanently.
- The shim adds at most 15 bytes of downward alignment plus one return address. The fresh default build's `page_fault` C body used a 0x188-byte frame; this is materially smaller than the former interrupt prologue plus XMM saves. The normal kernel-thread stack is 32 KiB (`sched/thread.rs:54-63`), and `#DF` remains on its dedicated IST stack.

### 5. Compile-time guards and discriminators

- `ErrorCodeVector` is implemented only for the crate's three error-code entry types; `NoErrorCodeVector` only for its two no-error types (`idt.rs:128-131,175-177`). This prevents applying either installer to the opposite vector class, including future crate fields typed as one of those entry families.
- The macro also type-checks every body as the uniform divergent C signature. It cannot prove semantic vector-to-body identity; however, the source mapping and the fresh disassembly's seven direct targets were checked manually. Strengthening the structural test to decode relative-call targets would improve coverage but would duplicate a source/disassembly invariant rather than repair a defect.
- `threads_guard_fault_timed` waits for the per-key kernel queue length before faulting, and the feature-only counter increments only when `purge_pid` actually removes a finite-deadline waiter and executes `recompute_earliest` (`neo-worker/src/main.rs:755-775`; `sched/futex.rs:403-438`). The static futex word is scenario-private, and no scenario kill syscall is issued, so the leg is non-vacuous in this run shape.
- The structural leg enumerates all present IDT gates, skips absent future error-code vectors, requires at least 18 installed exception gates, and checks the exact eight post-mask bytes. It cannot pass by observing zero gates.

### 6. Cause and target configuration

- The custom target retains the x86-64 SysV stack alignment (`data-layout ... S128`), disables the red zone, and changes only kernel/PIC code-model properties relevant here. No target flag explains or repairs the extra error-code qword.
- The fresh pinned-toolchain disassembly reproduces the distinction: a no-error `x86-interrupt` prologue preserves aligned `movaps` spills, while the replacement error-code shims establish alignment explicitly. That supports the stated LLVM `x86-interrupt` error-code ABI bug class rather than a repository target-spec anomaly.

## Verification performed

- RAG health: healthy (`ollama=true`, `qdrant=true`); used for conceptual discovery, followed by direct reads of the complete relevant code.
- `cargo k`: PASS on the reviewed tree.
- Fresh `llvm-nm`/`llvm-objdump`: PASS for vector-to-shim mapping, seven error-code realignment sequences and targets, and the returning `#BP` prologue.
- `NEO_SMP=4 NEO_TEST_ATTEMPTS=1 cargo xtask neo-test exception_entry_flags`: build PASS; execution unavailable on this host because xtask requires a Limine checkout at `/home/neo/limine`. This is an environment limitation, not a code/test failure. I did not rerun the 180-second negative control; the merged final-tree gate evidence and static witness path were reviewed.

## Findings

None.
