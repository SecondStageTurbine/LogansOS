<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Codex code-AAG of B1c (real generation in the product build) → Jarvis

**Type:** review only. **Deliverable:** one pushed RETURN.

**Why Jarvis:** you refuted the B1c plan (`Plans/RETURN-2026-10-20-b1c-design-crosscheck-jarvis.md`,
F1–F8). The PM accepted all eight and cleared revision 2 against your "minimum revised-plan gate". This
review checks the **built** branch against that gate. The author and the first AAG lens are Claude-family.
B1c touches the scheduler-adjacent seam: a shared kernel stack used under preemption, plus a lock held
across a long syscall. That makes a cross-family lens mandatory (CLAUDE.md, SMP/concurrency-core AAG).
If you are busy, Chad takes it unchanged.

## What is under review

- **Branch:** `claude/b1c-product-generation`. The PM names the head SHA when relaying.
- **Diff:** `git diff origin/main...origin/claude/b1c-product-generation`.
- **Start from the NOTE:** `Plans/NOTE-2026-10-20-b1c-product-generation-s1.md`. Read revision 2
  (sections 1–8) and the "As built" section at the end. The as-built section lists every place the build
  differs from the plan:
  - a **yielding** lock instead of `SwitchMutex` (which spins);
  - the stack mapped at boot;
  - the scope of the zero-allocation proof;
  - the clippy baseline;
  - the ELF scan standing in for the section 7 runtime guard.

What B1c adds:
- **F1** `generate_logic::check_generate_request`: one policy for `SYS_GENERATE`, `_CONSTRAINED` and
  `_MASKED`.
  - The descriptors are read into stack arrays (`copy_user_array`).
  - `model_lookup` is factored into `model_from_handle`.
- **F2** `tools/real-lm-oracle/inventory_ref.py` and `expected_tokens.txt`.
  - The kernel's `real_lm_generate_boot` parses that file.
  - Two negative controls: a control prompt, and a graph model through the same route.
- **F3** The cfg matrix (`syscall.rs`), plus `cargo xtask product-elf-check`.
- **F4** `MAX_ORDER` 16 under `real-lm`:
  - `neomc inspect`;
  - `module_reclaim_logic::guaranteed_order` (brute-force host KAT);
  - the order-16 primary allocation/free/coalesce check in `real_lm_generate_boot`;
  - the guaranteed-order allocation in `boot_module_reclaim`;
  - the primary trim logged.
- **F5** `kernel/src/mm/inference_stack.rs` and `inference_stack_logic.rs`:
  - one 256 KiB stack with guard pages, mapped at boot;
  - `call_on_stack` (inline asm), the yielding lock, the canary and high-water;
  - the `#DF` handler naming an overflow (`idt.rs`);
  - the `inference_stack_overflow` scenario;
  - CP-63.
- **F6** The attention heap-shape KAT (`ai-ops-tests`, counting allocator).
- **F7** The kill-switch wording.
- **F8** xtask `blake3` pins, and Arthur's typed `VocabLoad` plus `compose`.

## Your job: refute it

1. **The inference stack (F5): the highest-risk item.**
   - **`call_on_stack`:** register constraints, alignment, `clobber_abi`, and `pop rsp`. Can the
     compiler place anything it needs after the asm on the stack it just left?
   - **Preemption on the foreign stack:** the timer IST path, `context_switch`, and `TSS.rsp0` /
     `kernel_rsp`. Is any path that assumes `rsp` lies in the thread's own stack reachable while on it?
   - **The yielding lock:**
     - lost wakeups;
     - a holder that never releases (force-kill, a fault inside the forward);
     - an `IF=0` caller spinning on `yield_now` with an empty ready queue;
     - lock order against the admission scheduler and `PAR_BUSY`.
   - **Overflow:** does it really escalate to `#DF`? Could a `#PF` handler with its own stack (none
     today) or the timer IST mask it? Is the guard classification right for both guards?
   - **Placement:** is the region provably unmapped, outside the image, heap and HHDM, in the KASLR
     fallback too? Is it shared by every later pmap?
2. **The bounds (F1).** Find a path that allocates, copies from the user, or looks up the model before
   `check_generate_request` in any of the three arms. Does the errno-precedence change in the masked arm
   (mask limits now refused before the `EFAULT` checks) break an ABI promise?
3. **The oracle (F2).**
   - Is `inventory_ref.py` independent of the kernel?
   - Does it read the `.neom` by ADR-028 alone?
   - Is the rotate-half RoPE convention the kernel's?
   - Could both be wrong the same way?
   - Is the 0.0106 margin safe?
4. **The cfg matrix (F3).**
   - Is anything test-only reachable in the product build?
   - Does `product-elf-check` scan the right ELF, with controls that can fail?
5. **`MAX_ORDER` (F4).**
   - Is order 16 the minimum?
   - Is `guaranteed_order` right and tight?
   - Is the primary-trim cost stated correctly?
   - CP-29: does anything grow per-CPU initialization?
6. **The heap budget (F6).** Is the 9,220 × S term the true peak at S = 2,304, or does another
   allocation overlap it? Is the threaded-matmul scratch really 0?
7. **The kill-switch (F7).** With `real-lm` off, is the kernel policy- and behaviour-equivalent to
   `main`?
8. **Pins and composition (F8).**
   - Can xtask stage a file that fails its pin?
   - Is every `compose` row right?
   - Is the model ever consumed when the vocab is bad?

A finding bounces the branch only if it survives your own re-verification. "No findings" must state what
you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\jarvis    — yours alone, never another coder's folder, never the repo root
BRANCH:   codex/b1c-code-aag-jarvis
          (git fetch origin && git checkout -B codex/b1c-code-aag-jarvis origin/claude/b1c-product-generation)
```

```
git fetch origin
git show origin/claude/b1c-product-generation:Plans/DISPATCH-2026-10-20-b1c-product-generation-codex-aag.md
git show origin/claude/b1c-product-generation:Plans/NOTE-2026-10-20-b1c-product-generation-s1.md
git diff origin/main...origin/claude/b1c-product-generation
```

Optional runs:
- `cargo test -p ai-ops-tests -p module-reclaim-logic-tests -p arthur-logic-tests -p neomc -p xtask`
- `python3 -I tools/real-lm-oracle/inventory_ref.py --check`
- `cargo xtask product-elf-check`
- `cargo xtask neo-test real_lm_generate_boot inference_stack_overflow boot_module_reclaim`

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?" questions
(faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='context switch rsp kernel stack TSS rsp0 preemption'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-20-b1c-product-generation-codex-aag-jarvis.md` to your branch and **push
it**. It must contain:
- a verdict per item;
- each finding with its severity, file:line, and a repro or the reasoning;
- for each item, what you checked.

Paste the pushed commit SHA in your reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree afterwards; never
delete the worktree.
