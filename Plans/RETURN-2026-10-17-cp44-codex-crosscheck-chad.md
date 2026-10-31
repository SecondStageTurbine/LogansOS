<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: CP-44 syscall-return `rcx`/`r11` cross-check — Chad (Codex)

**Reviewed head:** `bf3b56f18b23aa0f862dc5d664a92cfda9c1e495` (`9e0f5d68` implementation)  
**Compared with:** `origin/main` at `95a2c6ce`  
**Verdict:** **MERGE-WITH-NITS**

No blocking correctness or security finding survived re-verification. The two reloads are correctly placed in
both syscall stubs in the reviewed tree. This branch has not yet been rebased onto the CP-41 mainline: its
merge base with `origin/main` is `57261d66`, and a trial three-way merge reports conflicts in
`syscall.rs`, the test harness, Cargo features, xtask, and the concurrency document. The post-rebase ordering
constraint below is therefore part of this verdict; this RETURN does not attest to an as-yet-unwritten
conflict resolution.

## Findings

### F1 — Low: the permanent `r11` non-vacuity control relies on compiler code generation

**Location:** `kernel/src/arch/x86_64/syscall.rs:6211-6224`, dispatch at `:932`

`sys_test_dirty_scratch_impl` uses inline `asm!` to write caller-saved `rcx` and `r11`, then returns through
ordinary Rust and through `syscall_dispatch`. Declaring `out("r11") _` tells the compiler that the asm
clobbers `r11`; it does not promise that later compiler-generated code will preserve the particular value.
Inlining, register allocation, or a dispatcher epilogue change could overwrite `r11` and silently make an
`rcx`-only half-fix pass again.

This is not a current failure. I built the scenario kernel with nightly 2026-04-16 and disassembled it. The
dirty function emits the kernel-address moves at `0xffffffff8000bd73`/`...bd76`, then returns with both
registers still holding that address. The dispatcher calls it at `0xffffffff8000d374` and, before `ret`, only
stores/reloads `rax` and adjusts `rsp`; it does not touch `rcx` or `r11`. Thus the current dirty leg really is
what makes the `r11` half non-vacuous, matching the reported `r11`-deletion control.

**Suggested hardening:** make the dirty-at-common-epilogue property structural in test-only assembly, or add
a build-time disassembly assertion that no instruction between the dirty write and the common epilogue
rewrites `r11`. This need not block CP-44 because the reviewed artifact and the recorded mutation control
both discriminate correctly.

## Required post-rebase ordering

The reload depends on reaching the exact IRET-frame stack position and on being the last code that may use
caller-saved registers. After resolving the rebase, both variants must retain this order:

1. save the user FPU image;
2. execute CP-41's `ldmxcsr [rip + KERNEL_DEFAULT_MXCSR]`;
3. dispatch the syscall;
4. restore the user FPU image;
5. restore `rsp` from the `rbx` anchor;
6. pop all saved GP registers, ending with `pop rbx`;
7. load `rcx = [rsp]` and `r11 = [rsp + 16]` from the IRET frame;
8. `swapgs; iretq`.

In `origin/main`, CP-41's `ldmxcsr` is immediately after `fxsave` in the legacy variant and immediately after
`fpu_save_area_asm` in the feature-on variant. It neither pushes nor pops and therefore does not change the
reviewed offsets. The reloads must not move above `fpu_restore_area_asm`, `mov rsp, rbx`, or the GP pops:
the helper/call path may clobber `rcx`/`r11`, and `[rsp]` denotes the desired RIP only after the last pop.
Keeping the reloads before `swapgs` also avoids extending the existing ring-0-CS/user-GS interval.

## Dispatch checklist

### 1. Frame offsets after rebase

The entry sequence pushes the five-word IRET frame in SS, RSP, RFLAGS, CS, RIP order, then twelve saved GP
words. The epilogue restores the saved-area anchor and pops the same twelve words. Immediately after the
last `pop rbx`, `[rsp]` is RIP and `[rsp + 16]` is RFLAGS in both variants
(`syscall.rs:595-602` and `:682-689`). Disassembly of the built feature-on stub confirms the final sequence
at `0xffffffff800005a1`: `mov rsp,rbx`, twelve pops, the two loads, `swapgs`, `iretq`.

PR #15 changes interrupt stubs rather than this syscall frame. CP-41's already-landed `ldmxcsr` has no stack
effect. The pending rebase has textual conflicts, so the order must be checked on the resolved head as stated
above.

### 2. Other ways back to ring 3

`SYS_EXIT` reaches `sched::exit() -> !`; successful `SYS_THREAD_EXIT` reaches
`thread_exit_current() -> !`. Neither returns through a stale syscall frame. `SYS_KILL` cannot self-kill and
ordinary success/error returns through the common epilogue. A fatal fault in a syscall body does not resume
that user context through an alternate syscall return.

A yield or blocking syscall switches away while preserving the current kernel stack. A previously blocked
user thread resumes its own syscall continuation and therefore its own common epilogue. A first-run user
thread uses the separate clean thread-entry trampoline and has no syscall frame. No current syscall path
rewrites the IRET frame. If one later does, deriving `rcx`/`r11` from the rewritten RIP/RFLAGS is the coherent
result because the registers then describe the same return selected by IRETQ.

### 3. SYSRET equivalence

The values loaded are the SYSCALL-captured user RIP and RFLAGS. SYSRET masks which bits reach architectural
RFLAGS, but it reads rather than rewrites the general-purpose `r11`; the raw saved-flags value remains in
`r11`. IRETQ likewise canonicalizes/privilege-checks RFLAGS independently of the GP load. Therefore unusual
reserved/RF/VM cases can make post-IRET architectural RFLAGS differ from the raw `r11`, but they do not
expose kernel data and do not invalidate the intended SYSRET-visible GP values. The discriminator uses only
ordinary settable status flags and correctly checks both raw `r11` and post-return RFLAGS.

### 4. Epilogue window

SFMASK clears IF, TF, AC, and DF on entry. The reload instructions do not change flags, so maskable
interrupts remain disabled until IRETQ restores the frame. An NMI that returns must preserve the interrupted
GP state; a fatal machine check does not produce a user return. Both loads precede `swapgs`, so CP-44 adds no
instructions to the pre-existing `swapgs`/`iretq` window.

### 5. Test-only syscall and discriminator

The number, dispatcher arm, and implementation are all gated by
`neo-test-syscall-return-clean-regs`; that feature is scenario-specific and absent from shipping defaults.
The ring-3 probe records `rcx` and `r11` as its first two instructions after each SYSCALL, computes the exact
return label, verifies distinctive `0xAD7` flags before/after, and verifies syscall-specific `rax` results.
The built dirty path was disassembled as described in F1 and currently preserves both dirty values to the
stub. The reported control where removing only the `r11` reload fails is therefore credible and non-vacuous.

### 6. Coverage

The normal qemu64 run exercises the `xsave-fpu` assembly variant through its runtime FXSAVE fallback; the
`-cpu max` evidence covers its XSAVE execution. The non-`xsave-fpu` source has the same final pop/load/return
sequence and was locally run by the author, but is not run by this scenario in CI. PR #19 only assembling it
still leaves a runtime gap. This is low risk for CP-44 because the affected instructions and frame layout are
identical and CPU-independent. SMP 1/4 plus the `SYS_SCHED_YIELD` leg covers migration/resumption behavior;
which CPU initially selects the probe does not affect the local stack invariant.

### 7. Remaining user-visible state

The stub restores `rbx`, `rbp`, `r12-r15`, `rdi`, `rsi`, `rdx`, `r10`, `r8`, and `r9`; `rax` is the return
value; CP-44 defines `rcx` and `r11`; IRETQ restores RIP, RSP, and RFLAGS. FS is untouched and `swapgs`
restores user GS. The saved FPU image restores x87/SSE/extended state. CP-41 owns kernel-MXCSR entry hygiene,
and the pre-Zen2 x87 FIP/FDP residual remains the separately tracked PR #17 issue. I found no additional GP
register leak in this path.

### 8. Other checks and validation

- RAG health was `healthy` with both Ollama and Qdrant true. Its full semantic query returned no hits, so I
  used the repository graph plus direct source and disassembly for the detailed audit.
- `git diff --check origin/main...HEAD` passed.
- `cargo xtask neo-test syscall_return_clean_regs` compiled all userspace payloads and the scenario kernel.
  Execution could not start because this Windows lane has no `/home/neo/limine`; xtask retried four times and
  ended with that environment error. This is not a kernel/test failure.
- The feature-on `syscall_entry`, dirty function, and dispatcher tail were independently disassembled from
  `target/x86_64-neoplutarch/debug/kernel` before cleanup.

