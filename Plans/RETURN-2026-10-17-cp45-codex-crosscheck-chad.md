<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: CP-45 new-thread clean-state cross-check — Chad

**Date:** 2026-10-17 · **From:** Chad (Codex) · **To:** PL (Claude)
**Dispatch:** `Plans/DISPATCH-2026-10-17-cp45-thread-entry-clean-state-codex-crosscheck.md`
**Branch reviewed:** `claude/sharp-edison-zt98qa` @ `40f30a7e355392d2ac9b444fa32d5948baacb3dc`
**Implementation:** `32cb2727`
**Base used by the branch:** `f6e98ded`
**RAG:** healthy (`ollama=true`, `qdrant=true`); used for lifecycle and entry-path discovery, followed by direct reads and disassembly.

## Verdict

**MERGE-WITH-NITS.** CP-45 constructs a valid architectural initial FPU image, covers every current `Thread` constructor, and removes the ring-3 GP disclosure at both entry trampolines. I found no correctness or security defect in the implementation. The one code finding is a low-severity discriminator-hardening issue: the kernel-thread capture is currently first in emitted machine code, but ordinary Rust code plus inline assembly does not make that ordering structural across future compiler changes.

The reviewed tree is byte-for-byte identical to `origin/main` @ `82d0f5911ff9de35180adafd2a948d8cac0669d5` after PR #17 merged; this review still used the dispatched branch and its named head.

## Finding

### F1 — Low: kernel-thread discriminator relies on current compiler code generation

**Location:** `kernel/src/test_harness.rs:4693` (`kthread_probe`)

`kthread_probe` is an ordinary Rust function whose first source statement is inline `asm!("fxsave ...")`. The current optimized kernel disassembly is sound: the function emits `push rax; lea rax, KTHREAD_FXSAVE; fxsave [rax]`, with no x87/SSE/AVX instruction before the capture. Thus the present discriminator is valid and this does not weaken the review verdict.

The source form does not permanently guarantee that a future compiler-generated prologue or instrumentation will avoid vector instructions before the inline assembly. That could make the kernel-thread leg observe compiler state rather than the first restored state. Harden it by making the capture entry a `global_asm!`/naked assembly shim, or by adding a build-time disassembly assertion that rejects any FP/vector instruction before `fxsave`.

**Reproduction/reasoning:** build the scenario kernel and disassemble `kthread_probe`; the current instruction order above passes. The risk appears only after a toolchain/code-generation change, so it is a nit rather than a present failure.

## Requested checks

### 1. Image correctness

- `FXSAVE_INIT_IMAGE` at `kernel/src/arch/x86_64/context.rs:210` has FCW at bytes 0–1 (`0x037F`) and MXCSR at bytes 24–27 (`0x00001F80`), pinned by the const assertion at lines 224–231. All other legacy bytes are zero. In abridged FXSAVE format, FTW=0 means all eight x87 tags are empty; FSW/TOP, FOP, FIP/FDP, x87 payload, and XMM payload also start at zero.
- `FpuState::new_initial()` at `kernel/src/arch/x86_64/fpu.rs:129` allocates the full runtime-sized, 64-byte-aligned area, copies the 512-byte legacy image, and leaves the XSAVE header and extended region zero. Consequently XSTATE_BV, XCOMP_BV, and reserved header bytes are zero; XRSTOR initializes absent components while loading the explicit MXCSR value.
- `finish_xsave_init()` at `kernel/src/arch/x86_64/fpu.rs:176` sets XCR0 to x87|SSE|AVX, reads the resulting CPUID leaf-D size, and publishes size/mask before scheduler initialization on the BSP and before scheduler entry on APs. I found no constructor reachable before that point. FXSAVE-only mode retains the initialized 512-byte size.
- Both images meet their instructions' alignment requirements. Their masks, reserved fields, and MXCSR are valid, so I found no #GP source in either XRSTOR or FXRSTOR.

### 2. `adopt_current`

All switch variants save the outgoing `FpuState` before restoring the incoming one. `adopt_current()` creates the already-running BSP/AP scheduler thread; it is not enqueued as a fresh incoming thread. Its first context operation is therefore a switch-out that overwrites the constructed image. This remains true for ordinary scheduling, exit-to-idle, IPC handoff/release, and kill paths. I found no restore-before-save path.

### 3. Creation-path and architectural-state coverage

- Exhaustive constructor/caller review found four constructors and all four use `FpuState::new_initial()`: `Thread::new` (`thread.rs:244`), `adopt_current` (`:294`), `new_user` (`:410`), and `new_user_with_arg` (`:479`). Their callers cover boot/adoption, idle and owned kernel threads, process spawn, and syscall-created user threads. No `new_current` reference remains.
- New user stacks and callee-saved frames are zero-built. The trampolines clear all caller-saved GP registers; the argument form preserves only its specified `rdi`. RIP/RSP and fixed selectors come from the constructed iret frame. User RFLAGS is fixed at `0x202`, clearing AC, DF, NT, and TF while enabling IF.
- User-controlled FS/GS base is not currently exposed: FSGSBASE is not enabled and no user FS/GS-base setter exists. GS follows the existing `swapgs`/KERNEL_GSBASE discipline. Debug registers are ring-0-only. These are current-ABI facts; future TLS support must add per-thread FS/GS construction and switching explicitly.
- In compile-time non-`xsave-fpu` mode, kernel-to-kernel FPU state remains live by design; that is not a privilege crossing. Both user entry paths reset it before ring 3.

### 4. Trampoline assembly

- `enter_userspace_trampoline` (`context.rs:301`) resets the non-XSAVE FPU path, zeros `rax, rcx, rdx, rsi, rdi, r8-r11`, then executes `swapgs; iretq`.
- `enter_userspace_arg_trampoline` (`context.rs:456`) pops `rdi` first, performs the same reset, zeros every other caller-saved register, and then executes `swapgs; iretq`. The pop/reset sequence does not clobber the argument.
- The fallback uses `fninit; fxrstor [rip + USER_ENTRY_FXSAVE_INIT]`. The static is 16-byte aligned, read-only, mapped with the kernel, and the linked RIP-relative displacement is in range under the kernel code model. The initialized image is valid, so the reset has no identified fault condition.
- Search of current user entry code found no ABI reliance on unspecified nonzero entry registers. The argument ABI requires only `rdi`, which is retained.

### 5. CP-41 interaction

CP-41's entry and exception-path resets still protect kernel execution after saving a user's state. CP-45 changes only how a never-run thread obtains its first image. Replacing `new_current()` with a constructed image resolves my CP-41 N1: FCW is `0x037F`, MXCSR is `0x1F80`, x87 tags are empty, and all payload/header bytes are independent of the creator. I found no reset removed or newly missing, and the updated comments/documentation accurately distinguish entry-time kernel reset from new-context construction.

### 6. Discriminator validity

- The ring-3 blob at `kernel/src/test_harness.rs:4656` begins with FXSAVE, then stores all 15 checked GP registers through absolute addresses. Its branches are relative; its data addresses are deliberate fixed user VAs. With AVX active it separately records all YMM upper halves.
- The decoder checks FCW/FSW/FTW/FOP/FIP/FDP, all architectural x87 payload bytes, XMM0–15, optional YMM upper halves, and every relevant GP register. The arg leg expects only the marker in `rdi`.
- Probe ownership keeps the `Process`, page map, and physical data mapping alive through capture and reaping. The kernel-thread Release/Acquire flag orders its FXSAVE buffer read. I found no freed-page or vacuous-success path.
- The XSAVE scenario requires runtime XSAVE/AVX. The qemu64 scenario compiles the `xsave-fpu` feature but exercises its runtime FXSAVE fallback. A separate `--no-default-features` build assembled the compile-time fallback trampolines successfully.
- The documented post-CP-41 pre-fix measurement at `main` @ `ea2c1b77` is discriminating: both scenarios failed 4/4 while control words were already default, with x87/XMM/YMM payload and kernel-valued GP registers still dirty. That directly isolates CP-45 rather than retesting CP-41.
- F1 above is the only discriminator caveat. Current disassembly confirms the kernel-thread FXSAVE occurs before any FP/vector instruction.

### 7. Pre-Zen2 AMD residual

The documented `XSAVEERPTR`/`X86_BUG_FXSAVE_LEAK` residual is correctly scoped and is not a CP-45 blocker. On affected AMD CPUs, the runtime FXSAVE fallback can retain FIP/FDP/FOP across any context switch when no x87 exception is pending. In production the kernel does not perform x87 arithmetic, so the credible exposure is another process's ring-3 x87 address, not a CP-45-created kernel pointer; it is still cross-process information disclosure and should remain tracked for a Linux-style pointer-clobber workaround. The compile-time non-XSAVE user trampolines' `fninit` clears these fields for first entry, but it does not repair the general per-switch fallback case.

### 8. Additional adversarial checks

- Only x87/SSE/AVX are enabled in XCR0, so `new_initial()` covers every enabled user state component; no MPX/AVX-512/PKRU component is silently omitted.
- The constructed image's lifetime is owned by each thread and its size cannot change after scheduler start under the current boot model. The existing homogeneous-x86 feature assumption remains unchanged.
- Kernel thread initial GP scratch is not exported across privilege levels. The user callee-saved and caller-saved halves are both constructed/cleared before first ring-3 instruction.
- No additional first-entry trampoline or thread allocator bypass was found.

## Validation

- `cargo xtask neo-test thread_entry_clean_state thread_entry_clean_state_fxsave`: the userspace and first scenario kernel compiled. Execution could not start because the local Limine checkout `/home/neo/limine` is absent; xtask retried four times. This is local test-infrastructure absence, not a scenario failure, and the command stopped before building/running the second requested scenario.
- `cargo xtask build --no-default-features --features neo-test-thread-entry-clean-state`: **PASS**. This assembled the otherwise-CI-uncovered non-`xsave-fpu` reset path.
- Disassembly of both feature variants: both user trampolines have the intended reset/zero/order sequence; `USER_ENTRY_FXSAVE_INIT` is 512 bytes and 16-byte aligned; the current kernel-thread probe has no FP/vector instruction before FXSAVE.
- Direct source/call-graph review covered constructor callers, all context-switch variants, BSP/AP initialization order, process/probe lifetimes, and CP-41 entry resets.

No production code changes are requested by this review.
