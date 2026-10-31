<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family adversarial review of CP-46 (error-code exception handlers on a misaligned stack) → Janus (Codex)

**Type:** review only (no code to merge). **Deliverable:** a pushed RETURN file. **Why a Codex seat:** CLAUDE.md
requires a different-model-family lens on any interrupt-core merge. The author and the AAG lenses on this
change are all Claude-family, so their agreement is weak evidence. Janus reviewed CP-40's exception shims and
CP-44's `page_fault` swap, which this change rebuilds, so one pass is cheapest. Any Codex seat (Jarvis/Chad) can
take it instead.

## What is under review

Branch `claude/tender-thompson-2u1err`, on `main` @ `82d0f591`:
`git diff origin/main...origin/claude/tender-thompson-2u1err`. The pitfall-bank entry is CP-46 in
`docs/concurrency-pitfalls.md`.

- **The defect.** On vectors 8, 10–14 and 17 the CPU pushes an error code: it aligns RSP to 16, then pushes six
  qwords, so the handler is entered 16-aligned instead of SysV's RSP ≡ 8 (mod 16). LLVM's `x86-interrupt`
  prologue does not correct for it (the rust-lang/rust#40180 / D30049 class), so every call an error-code
  handler makes runs 8 bytes off. On the ring-3 `#PF` kill path, `fault_kill_current_user` → `purge_pid` →
  `recompute_earliest` does `movaps [rsp+0x90], xmm2` whenever the dying process had a timed futex waiter.
  Measured at `NEO_SMP=4`: the fault-kill line, then `#GP rip=…2cfd rsp=…34a8` (RSP ≡ 8 mod 16), the `#GP`
  dump cut off after `InterruptStackFrame`, and a 180 s timeout.
- **The fix** (`kernel/src/arch/x86_64/idt.rs`). The seven error-code vectors (`#DF`, `#TS`, `#NP`, `#SS`,
  `#GP`, `#PF`, `#AC`) no longer use `x86-interrupt`. `set_error_code_entry!` installs a naked shim: CP-40's
  `pushfq; and [rsp], ~(AC|DF); popfq`, then `mov rdi, rsp; and rsp, -16; call body; ud2`. Each body is
  `extern "C" fn(&ErrorCodeFrame) -> !`, where `ErrorCodeFrame` is `#[repr(C)] { error_code: u64, frame:
  InterruptStackFrame }`, pinned by a `const` size/offset assertion. CP-44's RPL-keyed `swapgs` runs before
  anything else in `page_fault`'s body. Two trait bounds keep the two shims apart: `ErrorCodeVector` limits
  `set_error_code_entry!` to IDT entry types whose vectors push an error code, and `NoErrorCodeVector` limits
  `set_exception_entry!` to the rest. Nothing returns, so the shim saves no registers and does not keep the
  original RSP.
- **The discriminator.**
  - `threads_guard_fault_timed`: the `threads_guard_fault` battery, with the worker's futex-parked sibling
    waiting with a deadline an hour out (`userland/neo-worker/src/main.rs`, `tgf_park_forever`). A test-only
    counter in `futex::purge_pid` (`kernel/src/sched/futex.rs`) counts purges that removed a timed waiter;
    the harness requires it to be non-zero after the battery
    (`reason=no_timed_futex_waiter_purged_by_fault_kill`). Before faulting, the worker waits until the
    kernel's futex queue length for the park word is ≥ 1, so the sibling is a queued timed waiter, not just
    past its "reached" flag (CP-3x).
  - `exception_entry_flags` gains a structural leg: for each error-code vector (including the uninstalled
    `#CP`/`#VC`/`#SX`, skipped while not present), the 8 bytes after CP-40's mask in the gate's shim must be
    `mov rdi, rsp; and rsp, -16; call` (`reason=error_code_gate_not_realigned`).
- **The Claude AAG** (interrupt/ABI, concurrency, completeness lenses) found no surviving defect. Its hardening
  notes are folded in: the inverse trait bound, the extra vectors, the layout assertion, the queue-length
  barrier, and stale comments and docs.

## Your job: refute it

Default to suspicious. Specifically try to break:

1. **The frame.** Does `rdi` point at the error code on every one of the seven vectors, including `#DF` on its
   IST stack, and is `ErrorCodeFrame`'s layout exactly what the CPU pushed (check the `x86_64` crate's
   `InterruptStackFrame`/`InterruptStackFrameValue` layout for the version in `Cargo.lock`)? Can a nested
   exception or NMI inside the shim (before the `call`) see or leave a state the old `x86-interrupt` entry
   did not?
2. **Alignment.** After `and rsp, -16; call`, is the body entered at RSP ≡ 8 (mod 16) in every case? Do the
   no-error-code `x86-interrupt` handlers (`#BP` especially, the only one that returns) really call at an
   aligned RSP on this toolchain (`nightly-2026-04-16`)? Disassemble a few. Are there other exception or
   interrupt entries that call Rust without realigning (the IRQ stubs, timer stub, NMI, `#MC`, the syscall
   entry)?
3. **Never returns.** Every body is `-> !`. Walk `page_fault` through `handle_page_fault` (all neo-test
   hooks), `fault_kill_current_user`, the browser-runtime kill block and the fatal path, under every cfg. Is
   there anywhere a recoverable fault was expected to resume (a user-copy fixup, a probe that expects to
   continue)? Did dropping the `x86-interrupt` prologue (GPR and xmm0-15 save, `cld`) lose anything a
   diverging body still needed?
4. **The kill path.** `fault_kill_current_user` now runs from an `extern "C"` body called by the shim instead
   of from inside an `x86-interrupt` frame. Does anything on that path (the switch-away, `load_kernel_fp_control`,
   the observability trace, CP-40's AC counters, the CP-44 swap) depend on the old frame shape or on the
   handler's own stack depth? Is the kernel stack big enough for the same work, given the shim adds only one
   return address?
5. **The guards.** Do the two trait bounds really keep each shim to its kind of vector, for every entry type
   the `x86_64` crate exposes? The structural leg does not decode the `call` target, so a shim installed with
   the wrong body would pass; is there a cheap way to close that? Can either discriminator pass vacuously, or
   fail spuriously at `NEO_SMP=4` (is the queue-length barrier sound)?
6. **Anything a Claude reviewer would structurally miss.** In particular: is `rust-lang/rust#40180` really the
   cause, or is the misalignment from something else in our target spec (`kernel/x86_64-neoplutarch.json`) or
   build flags?

A finding bounces the change only if it survives your own re-verification; uncertain items go in the RETURN as
notes. "No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/cp46-crosscheck-janus        (git fetch origin && git checkout -B codex/cp46-crosscheck-janus origin/claude/tender-thompson-2u1err)
```

Read this dispatch and the code from the branch (it is not on `main`):

```
git fetch origin
git show origin/claude/tender-thompson-2u1err:Plans/DISPATCH-2026-10-18-cp46-error-code-stack-align-codex-crosscheck.md
git diff origin/main...origin/claude/tender-thompson-2u1err
```

Building/running is optional. With `NEO_SMP=4`, `cargo xtask neo-test threads_guard_fault_timed` and
`cargo xtask neo-test exception_entry_flags` should both PASS. For the control, restore only `idt.rs` to its
state in the discriminator commit (the branch's first commit, `neo-test: threads_guard_fault_timed (CP-46
discriminator)`), which is `main`'s `idt.rs` with only the test-counter cfgs widened:

```
git show <discriminator-commit>:kernel/src/arch/x86_64/idt.rs > kernel/src/arch/x86_64/idt.rs
```

`threads_guard_fault_timed` should then print `GENERAL_PROTECTION_FAULT (#GP)` right after `ring-3 #PF …
fault-killing process` and time out at 180 s; `exception_entry_flags` should FAIL with
`reason=error_code_gate_not_realigned vectors=0x00027d00`. Set `NEO_TEST_ATTEMPTS=1` to skip the retries.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='exception handler error code stack alignment x86-interrupt'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-18-cp46-codex-crosscheck-janus.md` to `codex/cp46-crosscheck-janus` and **push
it**. A review delivered only in chat does not exist. Contents: verdict (MERGE / MERGE-WITH-NITS / BOUNCE),
each finding with severity + file:line + repro or reasoning, and for each numbered item above what you
checked. Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** this is a review lane; if you built, run `cargo clean` in your worktree afterwards. Never
delete the worktree itself.
