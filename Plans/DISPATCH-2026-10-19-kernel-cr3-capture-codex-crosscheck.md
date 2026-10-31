<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family check of the CP-60 kernel-CR3 capture fix → Jarvis (Codex)

**Type:** review only. **Deliverable:** a pushed RETURN file. **Why you:** this changes which page table every
kernel thread runs on, and it adds a hard invariant to `Pmap::drop`. That is address-space / scheduler core, so
a different-family lens is required before merge. The fix blocks OT-22 (#58) from merging.

## What is under review

Branch `claude/token-park-teardown-fix` (from `origin/main` 35930c52); the PM names the head SHA in the relay.
Diff: `git diff origin/main...origin/claude/token-park-teardown-fix`.

1. **The bug (CP-60).** `process::KERNEL_CR3` was `Lazy::new(Cr3::read)`.
   - Its first call happened at the first switch from a user-process thread to a kernel thread, while that
     process's PML4 was still loaded, so that PML4 became "the kernel's".
   - Every kernel thread then ran on it.
   - When that process died, the freed PML4 was reused by the next `Pmap::new` and zeroed under the running
     CPU. That shows up as a triple fault, or as buddy corruption that once handed out the kernel's own PML4.
   - Instrumentation hid it, because any early `kernel_cr3()` call forced the correct value.
2. **The fix.**
   - `mm::init` captures `(frame, flags)` into `mm::KERNEL_CR3` at the same point as `KERNEL_PML4_PHYS`,
     before any user Pmap exists.
   - `process::kernel_cr3()` returns that value.
   - The control feature `neo-test-kernel-cr3-lazy-control` restores the lazy read.
3. **The invariant.** `Pmap::drop` asserts that the PML4 it frees is neither the kernel's nor any CPU's
   cached active CR3 (`per_cpu::active_pmap_cr3`).
4. **The discriminator.** `token_park_teardown_churn` runs 60 rounds of: register a process, run a kernel
   thread on its Pmap that parks on a scheduler wake token, then `terminate` + reap.

## Evidence (claimed)

- On `main` (scenario only, no fix): 4 of 6 boots triple-faulted.
- Fixed: 3/3 PASS.
- Control: FAILs on every boot with `Pmap::drop: PML4 … is still the active CR3 on cpu 0` (round 1).

## Refute it

1. **Boot value.** Is `mm::init`'s `Cr3::read` guaranteed to be the kernel's final page table? Does anything
   switch to or rebuild the kernel PML4 after `mm::init` (Limine handoff, a later remap, AP bring-up), and do
   APs share the BSP's PML4?
2. **The invariant.** Can it fire on a legitimate path?
   - a Pmap dropped on a spawn failure;
   - a Pmap dropped while an AP's cached `active_pmap_cr3` still names it after that AP switched to a
     same-pid thread, or to the idle thread or its bootstrap thread;
   - the per-CPU cache going stale anywhere a `Cr3::write` bypasses it (test harness writers included).
   - Is a panic the right severity versus a debug assertion?
   - **The assertions panic while the frame-allocator lock is held** (`Pmap::drop` checks inside
     `with_frame_alloc(|fa| …)`, just before `fa.free(pml4)`). Can the panic path (the panic handler,
     serial output, the test harness's `panic_failed`, anything that allocates or frees a frame) take
     that lock and deadlock, or spin with interrupts off, instead of reporting? Should the check run
     before the lock is taken, with the free afterwards? If so, say whether anything can change
     between the check and the free (another CPU loading this PML4 in that window).
3. **Other lazily captured "kernel" state.** Any other `Lazy`/`Once` initialized from live CPU or
   per-process state on first use that could capture a user context (CR3, GS, TSS, FPU)? Name them.
4. **Exposure on `main` before the fix.** Which production paths ran kernel threads on neo-init's PML4, and
   could anything have read user-half memory through it? Consider `current_process_for_syscall`'s
   `get_user_by_active_cr3` fallback, which attributes by the cached CR3.
5. **Discriminator.** Is the churn scenario non-vacuous? Does the control exercise the exact pre-fix code?
6. Anything else a Claude reviewer would structurally miss.

A finding counts only if it survives your own re-verification; uncertain items are notes. "No findings" must
say what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/kernel-cr3-capture-crosscheck-jarvis   (git fetch origin && git checkout -B codex/kernel-cr3-capture-crosscheck-jarvis origin/claude/token-park-teardown-fix)
```

```
git fetch origin
git show origin/claude/token-park-teardown-fix:Plans/DISPATCH-2026-10-19-kernel-cr3-capture-codex-crosscheck.md
git diff origin/main...origin/claude/token-park-teardown-fix
```

Optional runs, at `NEO_SMP=4`:
- `cargo xtask neo-test token_park_teardown_churn` should PASS.
- The same with `NEO_TEST_EXTRA_FEATURES=neo-test-kernel-cr3-lazy-control` should FAIL.

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?" questions;
grep still wins for exact symbols. Health-check first with `Invoke-RestMethod http://127.0.0.1:8404/health`,
expecting `{"status":"healthy",...}`. If it is down or degraded, the PL manages it: note that in your RETURN
and continue with direct reads.
Optional self-serve start:
`powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"`, then
`cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
Example query: `$body=@{query='kernel CR3 address space switch to kernel thread'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-19-kernel-cr3-capture-codex-crosscheck-jarvis.md` to
`codex/kernel-cr3-capture-crosscheck-jarvis` and push it. Contents:
- a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
- each finding with severity, file:line and a repro;
- what you checked for each numbered item.

Paste the pushed SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree; never delete the
worktree.
