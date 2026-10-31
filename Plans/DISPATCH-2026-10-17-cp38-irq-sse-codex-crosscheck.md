<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family adversarial review of CP-38 (`swapgs_irq_entry` x87/SSE save) → Janus (Codex)

**Type:** review only (no code to merge). **Deliverable:** a pushed RETURN file. **Why a Codex seat:** CLAUDE.md
requires a different-model-family lens on any SMP / interrupt-core merge; everything on this PR so far
(author, both diagnosis traces, the AAG panel) is Claude-family, so agreement among them is weak evidence.

## What is under review

PR #15, branch `claude/phase-4-continuation-TlW3j`: review the whole branch diff
(`git diff origin/main...origin/claude/phase-4-continuation-TlW3j`). It holds CP-38 (x87/SSE save in
`swapgs_irq_entry`) and CP-39 (`cld` on entry to `swapgs_irq_entry` and both timer stubs — ring 3 can set
DF and the kernel's `memcpy`/`memset` are `rep movs`/`stos`).

- `kernel/src/arch/x86_64/idt.rs`: `swapgs_irq_entry` (the hand-written stub for the TLB-shootdown and
  reschedule IPIs and the AHCI, NVMe, virtio-blk, virtio-accel, xHCI and spurious vectors) previously saved
  only GP registers before calling an SSE2-compiled Rust body. It now saves and restores x87/SSE around the
  body: `fxsave`/`fxrstor` without `xsave-fpu`, `fpu_save_area_asm`/`fpu_restore_area_asm` with it (same
  variants as the timer stub in `timer.rs`). The variant is picked by `macro_rules!` + `concat!` inside
  `global_asm!`.
- New scenario `irq_xmm_preserve` (`kernel/src/test_harness.rs`, test-only vector `0xe0`).
- `docs/concurrency-pitfalls.md` CP-38 has the full evidence: 76% of ring-0 interrupts changed the
  interrupted context's `xmm` on `main`; Arthur's undo silently lost at ~0.7% via NeoFS
  `ContentHashMismatch` → `ESTALE` under the IF-on `sys_read`.

## Your job: refute it

Default to suspicious. Specifically try to break:

1. **Restore-clobbers-newer-state.** With `xsave-fpu`, the context-switch shims also save/restore per-thread
   FPU state (`context.rs`, `fpu.rs`). If a body reached through this stub context-switches (reschedule IPI,
   anything calling into the scheduler), does the stub's restore on the way out ever overwrite a
   *legitimately newer* FPU image — for the resumed thread or for the thread that was switched in? Walk the
   exact control flow, ring 0 and ring 3.
2. **Alignment and sizing.** RSP at each `call` (SysV), FXSAVE 16-byte / XSAVE 64-byte alignment, and whether
   `FPU_SAVE_AREA_SIZE` can differ between save and restore, or before FPU init / on an AP before its XCR0 is
   programmed.
3. **Register clobbers.** What `fpu_save_area_asm`/`fpu_restore_area_asm` clobber; that `rdi` still holds the
   `from_ring3` flag at `call \body`; that `rbp` (the unwind anchor) survives.
4. **AVX holes.** Can any body execute VEX/AVX instructions while the non-xsave (`fxsave`) variant is built?
   Do `extern "x86-interrupt"` exception handlers that *return* (page fault) preserve YMM upper halves and
   MXCSR if they reach AVX code?
5. **Coverage.** What `irq_xmm_preserve` (ring-0 `int 0xe0`) does not exercise: the ring-3 path, real MSI
   delivery, APs, a nested interrupt during the save itself. Which of those matter?
6. **DF (CP-39).** Is `cld` placed before every path that can reach Rust in both stubs, and is anything else in
   RFLAGS (AC, which disables SMAP when set from ring 3) also left in a state the Rust body must not inherit?
7. **Anything else** a Claude reviewer would structurally miss.

A finding bounces the PR only if it survives your own re-verification; uncertain items go in the RETURN as
notes. "No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/cp38-crosscheck-janus        (git fetch origin && git checkout -B codex/cp38-crosscheck-janus origin/claude/phase-4-continuation-TlW3j)
```

Read this dispatch and the code from the PR branch (it is not on `main`):

```
git fetch origin
git show origin/claude/phase-4-continuation-TlW3j:Plans/DISPATCH-2026-10-17-cp38-irq-sse-codex-crosscheck.md
git diff origin/main...origin/claude/phase-4-continuation-TlW3j
```

Building/running is optional: `cargo xtask neo-test irq_xmm_preserve` should PASS; temporarily replacing the
`irq_fpu_save!()`/`irq_fpu_restore!()` invocations in `idt.rs` with an empty save should make it FAIL 64/64.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='interrupt entry FPU save xsave context switch'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-17-cp38-codex-crosscheck-janus.md` to `codex/cp38-crosscheck-janus` and
**push it**. A review delivered only in chat does not exist. Contents: verdict (MERGE / MERGE-WITH-NITS /
BOUNCE), each finding with severity + file:line + repro or reasoning, and for each numbered item above what
you checked. Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** this is a review lane; if you built, run `cargo clean` in your worktree afterwards. Never
delete the worktree itself.

## Addendum — re-check of the F1 fix (2026-10-17)

Your RETURN (`bef52698`) bounced on F1 (AC not cleared at interrupt entry). Fixed in `bc67f0e0` on the PR
branch. Please re-check **that commit only** (`git show bc67f0e0`):

1. The flags mask `pushfq; and qword ptr [rsp], -263169; popfq` (= `~(AC|DF)`) replaces CP-39's `cld` in
   `swapgs_irq_entry` and both timer stubs. Is it placed before every helper/Rust call, does it leave the
   saved interrupt frame untouched, and is `popfq` safe here (IF=0 inside the gate, so it cannot re-enable
   interrupts; no other flag changes)?
2. Your diagnostic, now permanent in `irq_xmm_preserve(_fxsave)`: enter with AC=1 and DF=1, body records both,
   and after `iretq` the test asserts the caller's AC/DF came back. Does it discriminate as claimed?
3. The CP-38 text narrowing (your P2) and CP-40 in `docs/concurrency-pitfalls.md`.

Close: append a short "Re-check of bc67f0e0" section to your RETURN file on `codex/cp38-crosscheck-janus`,
push it, and give the PL the commit SHA. Same worktree and branch as before.
