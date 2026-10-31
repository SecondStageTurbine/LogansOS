<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-83 ring-3 exception containment — cross-family review → Jarvis

- **To:** Jarvis (Codex family). The author, S3, is Claude.
- **MODEL:** GPT-6.1-Sol
- **EFFORT:** high (this is a kill path reachable from every exception vector)
- **Branch under review:** `claude/ot83-ring3-exception-kill`, at the head named in its PR.
- **Your RETURN:** `Plans/RETURN-2026-10-22-ot83-codex-review-jarvis.md` on branch
  `codex/ot83-codex-review-jarvis`, cut from `origin/main`, containing only that file. Push it; a review that
  exists only in chat does not count.
- **Verdict:** SOUND, SOUND-WITH-NITS, or BOUNCE. A finding needs a concrete repro or code path. An uncertain
  item is a note, not a blocker.

## Read first

```
git fetch origin
git show origin/claude/ot83-ring3-exception-kill:Plans/DISPATCH-2026-10-22-ot83-codex-review.md
git show origin/claude/ot83-ring3-exception-kill:docs/RESEARCH-2026-10-22-ot83-FINDINGS.md
git diff origin/main...origin/claude/ot83-ring3-exception-kill --stat
```

The core files are:
- `kernel/src/user_fault_logic.rs` (the decision);
- `kernel/src/arch/x86_64/idt.rs` (`contain_user_fault`, `halt_kernel_fault`, every handler);
- `kernel/src/sched/process.rs` (`fault_kill_current_user_exception`);
- `kernel/src/arch/x86_64/mod.rs` (the `CR0.NE` line).

## Checklist: refute each one, and say what you checked

1. [ ] **CPL routing.** Every routed handler calls `contain_user_fault` before any print or per-CPU access.
   Is the saved CS RPL the right CPL test for each vector? Can a ring-0 fault arrive with an RPL-3 CS, or a
   ring-3 fault with an RPL-0 CS? Look at the SYSRET path, `iretq` to ring 3 faulting, and the syscall entry
   before its `swapgs`.
2. [ ] **GS (CP-44).** On a ring-3 fault the handler swaps GS exactly once, before anything that reads
   per-CPU state. Is there any per-CPU read before the swap on any routed vector? Is there any way to reach
   the kill path with the kernel GS already live, which would double-swap? (The CP-47/48 `syscall_msr_trap`
   hook in `#GP` swaps on its own and must diverge first.)
3. [ ] **Ring 0 still halts.** A CPL-0 fault on every routed vector reaches `halt_kernel_fault` (or the
   existing neo-test `unexpected_trap` for `#PF`/`#XM`) unchanged. The `ring0_fault_halts` scenario proves
   it for `#UD` only. Is any vector's CPL-0 behaviour changed?
4. [ ] **The kill from exception context.** `fault_kill_current_user_exception` runs IF=0 on the faulting
   thread's kernel stack. Check:
   - every lock it takes, and whether any can already be held by this CPU or is spin-waited for while a
     peer waits on this CPU's shootdown ACK (CP-2, CP-3, CP-37, CP-64);
   - that no `Arc`/`Box` is held across `scheduler::exit()` (CP-30);
   - the CP-66 admission cancel;
   - whether a sibling inside a CP-63/65/66/67 kill-protected section is correctly skipped.
5. [ ] **Unconditional.** The kill path used to be `#[cfg(feature = "browser-runtime")]` (only `#PF`). It
   now compiles in every build, with only `fault_killing`, the futex purge and `wait_any` behind the
   feature. Without `browser-runtime`, is every process really single-threaded, so that the sibling drain
   is moot and the normal `on_thread_reap` teardown completes? Does anything else in the default build put
   a second thread in a user pid?
6. [ ] **FP state (CP-41).** After a ring-3 `#MF`/`#XM`, `load_kernel_fp_control` (FNINIT, which does not
   wait, then the kernel MXCSR) runs before the kill. Can the faulting thread's unmasked FCW or MXCSR, or a
   pending x87 exception, reach kernel code or the next thread on that CPU, with or without `xsave-fpu`?
7. [ ] **`CR0.NE = 1`.** This is now set at boot so x87 errors arrive as `#MF` instead of the legacy
   FERR#/IRQ13 path. Does any kernel code execute a waiting x87 instruction that could now raise `#MF` in
   ring 0? Is it set on every CPU, APs included?
8. [ ] **Exit codes.** 128 + signal: `#UD` 132, `#DB` 133, `#DE`/`#MF`/`#XM`/`#NM` 136, `#SS`/`#NP`/`#AC`
   135, `#GP`/`#PF`/`#BR`/`#OF` 139. Do they match what Linux delivers? Does any existing caller depend on
   the old `#PF`-only 139 contract?
9. [ ] **Coverage honesty.** Scenarios in the gate: `ring3_fault_{ud,de,gp,db,mf}` plus `ring0_fault_halts`.
   `ring3_fault_{ss,xm}` are `ignored`, because QEMU TCG mis-vectors `#SS` (as `#PF`/`#GP`) and never
   delivers `#XM` (FINDINGS §3). Is that QEMU claim right? `#OF`/`#BR` (invalid opcodes in 64-bit mode),
   `#AC` (`CR0.AM=0`) and `#NM` (EM=0, TS never set) are not reachable from ring 3 here and are covered by
   the host KAT only. Is any of those reachability claims wrong? Is a reachable vector left unrouted
   (`#TS`, `#NP`, `#DF`, NMI, `#MC`)?
10. [ ] **Default-build delta.** This lane deliberately changes the default kernel (a containment fix, so
    there is no kill-switch). Is any behaviour change beyond "ring-3 exception kills the process instead of
    halting" hiding in the diff?

## Relay

```
WORKTREE: E:\AI Automation\workers\jarvis    (yours alone; never another coder's folder, never the repo root)
BRANCH:   codex/ot83-codex-review-jarvis     (git fetch origin && git checkout -B codex/ot83-codex-review-jarvis origin/main)
MODEL:    GPT-6.1-Sol      EFFORT: high

Read: git fetch origin && git show origin/claude/ot83-ring3-exception-kill:Plans/DISPATCH-2026-10-22-ot83-codex-review.md
Push Plans/RETURN-2026-10-22-ot83-codex-review-jarvis.md on your branch. Do not edit the reviewed branch.

RAG (semantic code search) is available for this project — use it for conceptual "where is X
done?" questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: Invoke-RestMethod http://127.0.0.1:8404/health  → expect {"status":"healthy",...}.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct
file reads (never a blocker). Optional self-serve start (coders run PowerShell):
  powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
  cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
Query e.g.: $body=@{query='exception handler swapgs fault kill process'; top_k=5}|ConvertTo-Json;
  Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"
```

**Disk reclaim:** a review lane builds little. Run `cargo clean` in your tree once the RETURN is pushed. Never
delete the tree.
