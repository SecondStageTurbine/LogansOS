<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family adversarial review of CP-40 at the exception boundary → Janus (Codex)

**Type:** review only (no code to merge). **Deliverable:** a pushed RETURN file. **Why a Codex seat:** CLAUDE.md
requires a different-model-family lens on interrupt-core changes. The author and the AAG panel on this branch
are Claude-family. **Fallback seat:** Chad (Codex) if Janus is busy.

## What is under review

Branch `claude/magical-sagan-pi6azh`, stacked on PR #15 (`claude/phase-4-continuation-TlW3j` @ `0135fb55`).
Review only the commits above PR #15: `git diff origin/claude/phase-4-continuation-TlW3j...origin/claude/magical-sagan-pi6azh`.
It closes the gap your F1 RETURN (`bef52698`) left open ("the returning exception boundary should receive the
same follow-up audit").

- `kernel/src/arch/x86_64/idt.rs`: all 18 exception gates now point at an `exception_entry` shim,
  `pushfq; and qword ptr [rsp], -263169; popfq; jmp <handler>` (the same `~(AC|DF)` mask as
  `swapgs_irq_entry`). `set_exception_entry!(entry, handler)` generates each shim as a naked function from the
  handler's own name (`sym`), so a shim cannot jump to the wrong handler, and passes the handler to
  `install_exception_shim` so the compiler still checks its signature against the vector. #DF and NMI keep
  their IST indices. A comment block ("exception entry") lists what a returning exception handler must not
  touch. `arch/x86_64/mod.rs` `enable_sse` now clears CR0.AM on every CPU.
- Why it matters: exception delivery does not clear AC. A ring-3 #PF taken with AC=1 ran the whole
  `browser-runtime` fault-kill path with SMAP off, and `scheduler::exit()` then switched to another thread
  with AC still live (`context_switch` does not save RFLAGS).
- Discriminators: new scenario `exception_entry_flags` (`int3` with AC=1/DF=1 + `xmm0-15` patterns, 64
  rounds; plus a SIDT walk asserting every present gate in vectors 0-31 starts with the 10-byte mask, is DPL0
  and has the expected IST slot; plus CR0.AM=0). `threads_guard_fault` gains a ring-3 leg: the guard-overflow thread sets AC=1 before it recurses,
  and `page_fault` counts frame-AC vs live-AC.
- `docs/concurrency-pitfalls.md` CP-40: the per-vector audit table, the fix, the rules, the controls.

## Your job: refute it

Default to suspicious. Specifically try to break:

1. **The shim against the `x86-interrupt` ABI.** Does each handler see exactly the CPU-delivered stack (frame,
   and the error code on #DF/#TS/#NP/#SS/#GP/#PF/#AC)? Is `popfq` here safe (IF=0 in the gate; TF, RF, NT)?
   Does `iretq` in `breakpoint` still restore the caller's AC/DF? Anything about IST stacks or an NMI/#MC
   landing between `pushfq` and `popfq`?
2. **The audit table.** Are the reachability claims right: every gate DPL0 (so ring-3 `int3` is #GP), INTO and
   BOUND are #UD in 64-bit mode, CR0.TS/EM/AM never set, not-present vectors (9, 15, 20-31)? Which handlers keep
   running kernel code after a ring-3 entry? Is the "next thread inherits AC" inference right?
3. **Leaks the shim does not stop.** Can AC=1 still reach another thread some other way, e.g. a block, yield or
   switch while inside a `with_user_access` window?
4. **The rules for returning handlers.** Is "no AVX, no x87, no SSE float/MXCSR, no user memory outside
   `with_user_access`" the complete list given what the prologue saves (GPRs, `xmm0-15`, `cld`)?
5. **Discriminator vacuity.** Can `exception_entry_flags` or the `threads_guard_fault` leg pass while the
   property is false? The ordering of the `Relaxed` counters relative to the harness reading them is one place
   to look.
6. **Anything else** a Claude reviewer would structurally miss.

A finding bounces the branch only if it survives your own re-verification; uncertain items go in the RETURN as
notes. "No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/cp40-exception-crosscheck-janus   (git fetch origin && git checkout -B codex/cp40-exception-crosscheck-janus origin/claude/magical-sagan-pi6azh)
```

Read this dispatch and the code from the branch (it is not on `main`):

```
git fetch origin
git show origin/claude/magical-sagan-pi6azh:Plans/DISPATCH-2026-10-17-cp40-exception-entry-codex-crosscheck.md
git diff origin/claude/phase-4-continuation-TlW3j...origin/claude/magical-sagan-pi6azh
```

Building/running is optional: `cargo xtask neo-test exception_entry_flags` and
`cargo xtask neo-test threads_guard_fault` should PASS (set `NEO_SMP=4`, as CI does). Making
`install_exception_shim` install `_handler` directly (`entry.set_handler_fn(_handler)`, add the
`HandlerFuncType` bound) reproduces the pre-fix state: both should then FAIL
(`ac_set_in_exception_handler bp_ac_set=64`, `ac_set_in_page_fault_handler pf_live_ac=1`).

Already raised by the Claude AAG panel and **out of this branch's scope** (pre-existing; confirm or refute if
you have time, but they do not bounce this branch): SYSCALL's SFMASK does not clear RFLAGS.NT, and IRETQ with
NT=1 is #GP(0) in 64-bit mode; CSTAR is never written while the DPL3 32-bit user code segment is present.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='exception handler RFLAGS AC SMAP entry'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-17-cp40-exception-codex-crosscheck-janus.md` to
`codex/cp40-exception-crosscheck-janus` and **push it**. A review delivered only in chat does not exist.
Contents: verdict (MERGE / MERGE-WITH-NITS / BOUNCE), each finding with severity + file:line + repro or
reasoning, and for each numbered item above what you checked. Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** this is a review lane; if you built, run `cargo clean` in your worktree afterwards. Never
delete the worktree itself.
