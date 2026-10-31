<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family adversarial review of CP-45 (new threads start from clean register state) → Chad (Codex)

**Type:** review only (no code to merge). **Deliverable:** a pushed RETURN file. **Why a Codex seat:** CLAUDE.md
requires a different-model-family lens on thread-lifecycle and ring-3-entry merges. The author and the AAG lens
on this branch are Claude-family, so their agreement is weak evidence. **Why Chad:** your CP-41 review (N1 in
`codex/cp41-crosscheck-chad` @ `d7a0b1a3`) found the `new_current()` payload copy this branch removes; the PL
may reassign to any Codex seat.

## What is under review

PR #17, branch `claude/sharp-edison-zt98qa`, rebased onto `main` after #15/#18/#20/#21/#22. The PM names the
exact head SHA in the PR comment that relays this dispatch; review the whole diff at that head
(`git diff origin/main...origin/claude/sharp-edison-zt98qa`).

CP-45: a freshly created thread inherited its creator's live registers, and the creator is kernel code, so ring
3 could read kernel data (hash state, heap/stack/image pointers: a KASLR bypass).

- `kernel/src/arch/x86_64/fpu.rs`: `FpuState::new_current()` (which ran `fpu_save_area_asm` on the creating CPU,
  and after CP-41 only rewrote FCW/FSW/FTW/MXCSR) is replaced by `FpuState::new_initial()`, built entirely from
  constants: legacy region = `FXSAVE_INIT_IMAGE`, XSAVE header all zero (XSTATE_BV=0 ⇒ XRSTOR inits every
  component; MXCSR is written explicitly because standard-form XRSTOR loads it whatever XSTATE_BV says).
- `kernel/src/arch/x86_64/context.rs`: `FXSAVE_INIT_IMAGE` (FCW = `KERNEL_X87_FCW`, MXCSR = `KERNEL_MXCSR`, rest
  zero, pinned by a `const` byte assertion). Both ring-3 entry trampolines (`enter_userspace_trampoline`,
  `enter_userspace_arg_trampoline`) now zero `rax rcx rdx rsi rdi r8-r11` (`rdi` = arg in the arg variant)
  before `swapgs; iretq`; without `xsave-fpu` they also `fninit; fxrstor [rip + USER_ENTRY_FXSAVE_INIT]`.
- `kernel/src/sched/thread.rs`: all four constructors (`new`, `adopt_current`, `new_user`, `new_user_with_arg`)
  use `new_initial()`. `adopt_current`'s image is claimed never to be restored before its first save.
- Discriminator: `thread_entry_clean_state` (`-cpu max`, XSAVE) and `thread_entry_clean_state_fxsave` (qemu64,
  FXSAVE fallback), module `thread_entry_clean_state` in `kernel/src/test_harness.rs`. The spawner arms an
  xmm/ymm pattern, MXCSR 0x9F80, FCW 0x027F and an x87 marker, then creates a ring-3 thread through each
  trampoline plus a kernel thread; a hand-written ring-3 probe blob FXSAVEs, stores its GP registers (absolute
  addressing) and ymm0-15 as its first instructions. Every leg must see the init state with the default control
  words, and the ring-3 legs zero GP registers.
- `docs/concurrency-pitfalls.md` CP-45 has the pre-fix evidence, residuals and citations.

## Your job: refute it

Default to suspicious. Specifically try to break:

1. **Image correctness.** Byte offsets of FCW/MXCSR in the legacy region; abridged FTW=0 meaning "all empty";
   the XSAVE header (XSTATE_BV, XCOMP_BV, reserved bytes); image size vs `FPU_SAVE_AREA_SIZE` (CPUID 0xD after
   XCR0 is written) — can any thread be built before `finish_xsave_init` and then be restored with a larger
   size? Can XRSTOR or FXRSTOR #GP on this image?
2. **`adopt_current`.** Is its image ever *restored* before it is *saved*, on any path (BSP boot thread, AP
   adoption, idle threads, `exit` → idle, IPC handoff switches, kill paths)? If so the init image would clobber
   live state.
3. **Coverage of creation paths.** Any other way a thread first reaches ring 3, or a kernel thread first runs,
   that still carries creator state: GP, x87/SSE/AVX, RFLAGS bits (AC/DF/NT/TF), FS/GS base, debug registers,
   segment selectors?
4. **Trampoline asm.** Register clobbers and ordering with `pop rdi`, `swapgs`, `iretq`; the RIP-relative
   `fxrstor` to a `no_mangle` static under the kernel code model / KASLR; whether the non-`xsave-fpu` reset can
   fault. Is anything in userland relying on a non-zero entry register?
5. **Interaction with CP-41.** #18 put `load_kernel_fp_control()` in `enable_sse` and the page-fault kill path
   and `ldmxcsr` on the syscall/timer/IRQ hot paths. With `new_current()` gone, is any reset now missing, or any
   doc/comment now false?
6. **Discriminator validity.** Can either scenario pass vacuously (wrong save path, probe not first, kernel
   codegen touching xmm in the kernel-thread probe, data page freed before the read)? Does the pre-fix FAIL hold
   on current `main` (post-CP-41, the control words are already reset, so only payload + GP should be dirty)?
7. **Residuals.** Pre-Zen2 AMD FXRSTOR not loading FIP/FDP/FOP when FSW.ES=0 (cross-process x87 pointer leak on
   every switch with the FXSAVE fallback) is recorded, not fixed. Agree with the scoping, or is it a blocker?
8. **Anything else** a Claude reviewer would structurally miss.

A finding bounces the branch only if it survives your own re-verification; uncertain items go in the RETURN as
notes. "No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\chad     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/cp45-crosscheck-chad        (git fetch origin && git checkout -B codex/cp45-crosscheck-chad origin/claude/sharp-edison-zt98qa)
```

Read this dispatch and the code from the branch (it is not on `main`):

```
git fetch origin
git show origin/claude/sharp-edison-zt98qa:Plans/DISPATCH-2026-10-17-cp45-thread-entry-clean-state-codex-crosscheck.md
git diff origin/main...origin/claude/sharp-edison-zt98qa
```

Building and running are optional. `cargo xtask neo-test thread_entry_clean_state thread_entry_clean_state_fxsave`
should PASS. Restoring the old `new_current()` body (snapshot via `fpu_save_area_asm`) should make both FAIL with
`dirty=...:fpu`; removing the GP `xor`s from `enter_userspace_trampoline` should make both FAIL with
`dirty=entry:gp` and kernel addresses in `gp_dirty=`.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='new thread FPU image initial state trampoline'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-17-cp45-codex-crosscheck-chad.md` to `codex/cp45-crosscheck-chad` and **push it**.
A review delivered only in chat does not exist. Contents: verdict (MERGE / MERGE-WITH-NITS / BOUNCE), each
finding with severity, file:line, and a repro or reasoning, and for each numbered item above what you checked.
Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree afterwards. Never
delete the worktree itself.
