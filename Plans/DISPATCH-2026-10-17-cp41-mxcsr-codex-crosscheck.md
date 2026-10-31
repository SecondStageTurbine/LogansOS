<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family adversarial review of CP-41 (kernel MXCSR at entry) → Chad (Codex)

**Type:** review only (no code to merge). **Deliverable:** a pushed RETURN file. **Why a Codex seat:** CLAUDE.md
requires a different-model-family lens on interrupt / syscall-core merges. The author and the AAG lenses on
this branch are all Claude-family, so their agreement is weak evidence.

## What is under review

Branch `claude/eloquent-bell-8eu39r`; review the whole diff
(`git diff origin/main...origin/claude/eloquent-bell-8eu39r`). CP-41: the kernel is compiled with SSE and
syscall bodies run hardware float arithmetic (the Arthur inference path: Q8 quantizers, `vec_dot_*`,
`qwen2_output_logits`). Before this change they ran on **ring 3's** MXCSR and x87 control word. Ring 3 could
unmask SIMD exceptions, which turns the first inexact result into a ring-0 #XM and a kernel halt, or set
RZ/FTZ/DAZ and change kernel inference results.

- `syscall.rs` `syscall_entry` and `timer.rs` `timer_isr_entry`, both `xsave-fpu` variants:
  `ldmxcsr [rip + KERNEL_DEFAULT_MXCSR]` (0x1F80) right after the FPU save. The user's value comes back with
  the image on FXRSTOR/XRSTOR.
- **No `fninit` on that hot path, deliberately.** The kernel has no compiler-generated x87 instructions, and
  FNINIT measured ~70 TSC ticks per call against ~1 for LDMXCSR. Linux x86-64 `kernel_fpu_begin()` makes the
  same call (MXCSR only; FNINIT only for `KFPU_387`).
- `mod.rs`: `KERNEL_MXCSR`, `KERNEL_X87_FCW`, `load_kernel_fp_control()` (`fninit` + `ldmxcsr`, cold paths
  only). It is called from `enable_sse` (BSP and every AP) and before `fault_kill_current_user` in the ring-3
  page-fault path (`idt.rs`).
- `fpu.rs` `FpuState::new_current()`: writes the default FCW/FSW/FTW/MXCSR into the new thread's image.
- `idt.rs`: kernel #XM in `neo-test` builds reports `unexpected_trap trap=simd_floating_point`.
- `idt.rs` `swapgs_irq_entry`: the same `ldmxcsr` right after CP-38's `irq_fpu_save!()` (both variants). Added
  when the branch was rebased onto PR #15 (`8a4f2990`), which gave that stub an FPU save to restore from. Its
  bodies do no float arithmetic today, so this is defense in depth.
- Discriminator: scenarios `syscall_mxcsr_entry` (`-cpu max`, XSAVE) and `syscall_mxcsr_entry_fxsave`
  (qemu64). The ring-3 side is `userland/neo-init/src/main.rs` `syscall_mxcsr_entry_tests`. The kernel side is
  the test-only `SYS_TEST_FLOAT_PROBE` (0xF00) and `test_harness::syscall_float_probe`. Both scenarios fail
  with code 22 without the stub change and pass with it. CP-38's `irq_xmm_preserve(_fxsave)` gains an MXCSR leg
  for `swapgs_irq_entry`: the caller enters the test vector on `0xFFC0`, the body must see `0x1F80`, and the
  caller must get `0xFFC0` back. Without the swapgs reset both fail 64/64 (`mxcsr_not_default_in_irq_body`).
- `docs/concurrency-pitfalls.md` CP-41 has the evidence and the stated coverage gaps.

## Your job: refute it

Default to suspicious. Specifically try to break:

1. **Entry coverage.** Is there any path where ring-0 Rust float code still runs on a user-controlled MXCSR?
   Check every `global_asm!` entry, the `x86-interrupt` exception handlers, kernel-thread first run, and thread
   FPU images. Is the `swapgs_irq_entry` reset placed correctly relative to CP-38's save, the CP-40 flags
   mask, and the `from_ring3` argument load?
2. **The x87 decision.** Is skipping `fninit` on the hot path safe? Can any kernel code, including
   `compiler_builtins`, intrinsics or inline asm, execute x87 on ring 3's control word, tag word, or pending
   exceptions? Check: `llvm-objdump -d` the kernel for `f*` mnemonics.
3. **Save/reset/restore ordering.** Can a user value survive into the Rust body, or can the reset leak back to
   ring 3? Consider XRSTOR's MXCSR rule (standard form loads MXCSR when RFBM[1] or RFBM[2] is set, whatever
   XSTATE_BV says), the x87 init optimisation, and FXRSTOR.
4. **Faults in the stub.** Can `ldmxcsr`, `fninit` (cold paths), `fxsave`/`xsave` or the restore instructions fault on
   hostile user state: pending unmasked x87 exceptions, sticky MXCSR flags with unmasked bits, invalid MXCSR?
5. **Context switches.** The timer body can switch threads. Walk both builds: `xsave-fpu`, where
   `context_switch` swaps per-thread images, and the non-`xsave-fpu` build, where it swaps nothing. Can any
   thread end up on another thread's MXCSR?
6. **`FpuState::new_current()` image edits.** Byte offsets, the XSAVE header, and whether any caller needed a
   faithful copy. Also the behaviour change for `SYS_THREAD_CREATE`: POSIX `pthread_create` inherits the float
   environment, and new threads now get the default. Is that acceptable, or does the browser runtime need it?
7. **Discriminator validity.** Can either scenario pass vacuously: constant folding, user float code on the
   hostile state, a wrong oracle, the wrong save path? QEMU TCG never delivers #XM. Is the
   control-state/result check enough evidence for the trap half?
8. **Anything else** a Claude reviewer would structurally miss.

A finding bounces the branch only if it survives your own re-verification; uncertain items go in the RETURN as
notes. "No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\chad     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/cp41-crosscheck-chad        (git fetch origin && git checkout -B codex/cp41-crosscheck-chad origin/claude/eloquent-bell-8eu39r)
```

Read this dispatch and the code from the branch (it is not on `main`):

```
git fetch origin
git show origin/claude/eloquent-bell-8eu39r:Plans/DISPATCH-2026-10-17-cp41-mxcsr-codex-crosscheck.md
git diff origin/main...origin/claude/eloquent-bell-8eu39r
```

Building and running are optional. `cargo xtask neo-test syscall_mxcsr_entry syscall_mxcsr_entry_fxsave` should
PASS, as should `irq_xmm_preserve irq_xmm_preserve_fxsave`. Deleting the `ldmxcsr` line from both
`syscall_entry` variants should make the first two FAIL with `reason=syscall_mxcsr_entry_exit_nonzero code=22`;
deleting it from both `irq_fpu_save!()` variants should make the last two FAIL with
`reason=mxcsr_not_default_in_irq_body`.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='syscall entry FPU save MXCSR'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-17-cp41-codex-crosscheck-chad.md` to `codex/cp41-crosscheck-chad` and **push it**.
A review delivered only in chat does not exist. Contents: verdict (MERGE / MERGE-WITH-NITS / BOUNCE), each
finding with severity, file:line, and a repro or reasoning, and for each numbered item above what you checked.
Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree afterwards. Never
delete the worktree itself.
