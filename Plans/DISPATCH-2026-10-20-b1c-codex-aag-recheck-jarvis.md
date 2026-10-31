<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Codex re-check of B1c's J1/J2 fixes → Jarvis

**Type:** review only. **Deliverable:** one pushed RETURN.

**Why:** your code AAG (`codex/b1c-code-aag-jarvis` @ `b08066ae`) bounced B1c on two findings:
- **J1 (HIGH):** the inference-stack owner stayed shootdown-ready at `IF=0` for the whole forward, so any
  remote shootdown could panic the kernel.
- **J2 (MEDIUM):** "bounds first" was not true at every syscall boundary, and no test went through the
  real arms.

The PM ruled option (a) for J1. The fix is on `claude/b1c-product-generation`, and the PM names the head
SHA when relaying. You closed F2, F3, F4, F6, F7 and F8; this re-check covers J1 and J2, plus anything
the fix itself broke.

## What changed

Read the NOTE's last section first: "Jarvis's code AAG (cross-family) — BOUNCE on J1 and J2, and the
fix". Then CP-64, and CP-63's corrected force-kill paragraph, in `docs/concurrency-pitfalls.md`.

**J1:**
- **The interruptible owner.** `kernel/src/mm/inference_stack.rs`: `run_checked` runs its closure under
  `with_interrupts_enabled` (`interruptible`), and the caller's `IF` is restored on the inference stack
  before the call returns. The lock and the owner record (tid, pid) are taken and released together by
  `Held`. `kernel/Cargo.toml`: `inference-stack` now implies `xsave-fpu`.
- **The non-killable owner.**
  - `sched/scheduler.rs`: `kill_thread_by_pid` uses `position_by_pid_except(pid, owner_thread())`.
  - `sched/process.rs`: `terminate`'s accounting assertion accepts `owned_by_pid`; `torn_down(pid)` is new.
  - `arch/x86_64/syscall.rs`: `after_inference` wraps the four inference syscalls in `syscall_dispatch`
    and calls `sched::exit()` if the caller's process is gone or exiting.
- **The discriminator.** `test_harness.rs`, `run_inference_stack_shootdown`:
  - scenarios `inference_stack_shootdown` and `inference_stack_shootdown_control` (`NEO_SMP>=2`);
  - the control's expected panic is handled in `panic_failed`.

**J2:**
- **The contract.** `generate_logic::check_prompt_len`, and the ordering contract in
  `check_generate_request`'s doc. The constrained and masked arms call `check_prompt_len` before their
  descriptor copy.
- **The inventory route.** In `sys_infer_run_impl`, the inventory route reads its one handle into a stack
  array, looks up the tensor, and runs `qwen_inventory::admit_input` before building `inputs`.
  `graph_infer_inputs` keeps the graph route unchanged.
- **The probe.** `syscall.rs`, `generate_abi_probe_for_test` / `abi_probe_process_for_test`, driven by
  `test_harness.rs` (`real_lm_abi_probe`): 16 cases through the real arms. The probe thread is
  attributed to a registered user process (`Thread::set_pid_for_test`); that process's CR3 is active,
  and the thread runs on CPU 1.

**F5 note:** the mapping carries `GLOBAL`.

## Your job: refute the fix

1. **Owner `IF`.**
   - Is the caller's `IF` restored on every path out of `run_checked`?
   - Is anything between `acquire` and the closure, or after it, run with the wrong `IF`?
   - Can an interrupt taken while `rsp` is on the inference stack (inside the closure, or in
     `call_on_stack`'s tail for an `IF=1` caller) observe or corrupt anything?
2. **Preemption on the BSP.**
   - Is every lock the forward takes IRQ-safe? Look at:
     - the heap;
     - the frame allocator, for tensor storage;
     - `par_matmul`'s `PAR_BUSY`, and its AP slots when the job's owner is preempted mid-barrier;
     - the admission scheduler;
     - anything in `qwen_inventory`, `forward_logic` or `ops`.
   - Is the AVX2 state saved across a kernel-mode preemption with `xsave-fpu`?
   - Is anything per-CPU cached across the forward? A thread preempted on the BSP could resume on
     another CPU if anything migrates it.
3. **The non-killable owner.**
   - Can the owner still be zombified on any path: `zombify_parked_thread_by_pid`, a channel-parked
     extract, a fault kill?
   - Is the `owner_thread()` read in `kill_thread_by_pid` race-free against the owner releasing, since a
     Ready owner cannot release concurrently?
   - Does `after_inference` reliably catch a torn-down process? Is `sched::exit()` there safe: locals
     dropped, nothing held?
   - Does terminating a process whose owner is Running on another CPU (not Ready) behave?
   - Does a multi-threaded (`browser-runtime`) terminate loop terminate?
4. **The discriminator.**
   - Does `inference_stack_shootdown` really prove ACK/EOI progress, that the CPU is ready again
     afterwards, BSP timer progress and preemption?
   - Could it pass with the bug present? The control shows the pre-fix owner panics.
   - Is the control's expected-panic check specific enough (CPU 1, a recorded timeout on CPU 0)?
5. **J2.**
   - Is the stated contract now literally true in all three generate arms and in `SYS_INFER_RUN`'s
     inventory route?
   - Does any refusal case in the probe pass for the wrong reason?
   - Is the probe a fair proxy for a ring-3 caller: thread pid attribution, CR3, an AP with no timer?
   - Is anything in `model.validate_current()` or the lookups an allocation the counter would catch?
6. **Regressions.**
   - Does the `kill_thread_by_pid` / `terminate` change alter behaviour with `inference-stack` off? It
     must not: the cfg is `inference-stack`.
   - Does `inference_stack_overflow`'s change (interrupts masked for the deliberate overflow only)
     weaken it?

Measured locally, at `NEO_SMP=4`:

- **`inference_stack_shootdown`:** `sent=16 ipis_received=16 timeouts=0 ticks_while_parked=4
  corunner_runs=43808 if_restored_off=true ready_after=true`.
- **`inference_stack_shootdown_control`:** panics `tlb_shootdown_timeout sender_cpu=1`.
- **`real_lm_generate_boot`:** all 16 ABI cases ok. The 14 refusals made 0 allocations; `plain_ok`
  made 610 and `infer_ok` 494.

A finding bounces the branch only if it survives your own re-verification. "No findings" must say what
you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\jarvis    — yours alone, never another coder's folder, never the repo root
BRANCH:   codex/b1c-code-aag-recheck-jarvis
          (git fetch origin && git checkout -B codex/b1c-code-aag-recheck-jarvis origin/claude/b1c-product-generation)
```

```
git fetch origin
git show origin/claude/b1c-product-generation:Plans/DISPATCH-2026-10-20-b1c-codex-aag-recheck-jarvis.md
git show origin/claude/b1c-product-generation:Plans/NOTE-2026-10-20-b1c-product-generation-s1.md
git diff 048182dd..origin/claude/b1c-product-generation
```

Optional runs:
- `cargo test -p ai-ops-tests -p module-reclaim-logic-tests`
- `NEO_SMP=4 cargo xtask neo-test inference_stack_shootdown inference_stack_shootdown_control real_lm_generate_boot inference_stack_overflow`

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?" questions
(faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='kill_thread_by_pid zombify ready thread terminate'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-20-b1c-codex-aag-recheck-jarvis.md` to your branch and **push it**. It must
contain:
- a verdict on J1 and J2 (closed or still open);
- any new finding, with its severity, file:line, and a repro or the reasoning;
- for each item above, what you checked.

Paste the pushed commit SHA in your reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree afterwards; never
delete the worktree.
