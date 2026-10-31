<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family adversarial review of CP-47 (SFMASK NT) + CP-48 (CSTAR / compat mode) → Janus (Codex)

**Type:** review only (no code to merge). **Deliverable:** a pushed RETURN file. **Why a Codex seat:** CLAUDE.md
requires a different-model-family lens on kernel-entry changes; the author and the AAG panel are Claude-family.
**Fallback seat:** Chad (Codex).

## What is under review

Branch `claude/magical-sagan-pi6azh`, cut from `main` @ `82d0f591`. Review the whole branch diff
(`git diff origin/main...origin/claude/magical-sagan-pi6azh`). Follow-up B of the entry-path series: the two
pre-existing SYSCALL-MSR gaps reported on PR #22.

- **CP-47 (`kernel/src/arch/x86_64/syscall.rs`).** SFMASK masked IF|TF|AC|DF but not NT. POPFQ can set NT at
  CPL 3, SYSCALL clears only SFMASK bits, and IRETQ with NT=1 is #GP(0) in 64-bit mode, so the syscall's own
  `iretq` faulted in ring 0 (CPU halt), and a syscall that blocked handed NT=1 to the next thread through
  `context_switch` (which does not save RFLAGS). Fix: `RFlags::NESTED_TASK` added to SFMASK. The interrupt and
  exception entries are left alone: delivery through an interrupt/trap gate clears NT architecturally.
- **CP-48 (`gdt.rs`, `syscall.rs`).** GDT slot 3 (`0x18`/`0x1B`) held a present DPL3 32-bit code segment, and
  IA32_CSTAR was never written (0). Ring 3 could `retfq` to `0x1B` and, on AMD (and QEMU TCG), run `syscall` in
  compatibility mode, entering ring 0 at RIP 0 on the user's RSP and GS. Fix, in layers:
  1. Slot 3 is now a null descriptor. Nothing runs 32-bit code; the slot is kept so the user selectors (`0x23`,
     `0x2B`, hard-coded in the syscall asm and asserted) and STAR's SYSRET arithmetic are unchanged.
  2. CSTAR points at `compat_syscall_sink` (`cli; 1: hlt; jmp 1b`), which never touches memory, so even if a
     compatibility-mode code segment ever came back, a compat SYSCALL would park its CPU (and the next TLB
     shootdown would then panic the kernel) instead of having the kernel push frames onto a user-chosen RSP.
  3. `gdt::init_for_cpu` loads a null LDTR (`lldt 0`) and `write_syscall_msrs` writes SYSENTER_CS = 0, so
     neither the reset-state LDT (base 0, limit 0xFFFF) nor SYSENTER depends on what firmware left. Both were
     already safe under the gate's QEMU/Limine boot (measured), so these are hardening, not reproduced bugs.
- **Discriminators (new scenarios):**
  - `syscall_nt_masked`: a ring-3 probe sets NT=1 and makes three syscalls (an unassigned number, two
    `SYS_SCHED_YIELD`s). The kernel counts dispatches that ran with NT=1; the harness thread checks its own NT.
  - `compat_mode_unreachable`: SGDT walk (no present code segment with L=0 that is DPL3 or conforming, no
    DPL3 system descriptor), CSTAR == the sink, SYSENTER_CS[15:2] == 0, then a ring-3 probe: `lar` on an LDT
    selector and on `0x1B` must find nothing, and a far jump (`jmp m16:32`, memory operand) to `0x1B` must take
    #GP(`0x18`) at that instruction. Not `retfq`: QEMU TCG pops a far return's stack as a supervisor access,
    so under SMAP it #PFs before checking CS.
  - Both route #GP/#DF (and, for the compat scenario, any ring-3 #PF and a ring-0 #PF at RIP 0) to
    `test_harness::syscall_msr_trap`, which swaps to the kernel GS when the live GS base is a user one and ends
    the run at once if it is re-entered.
- **Claude AAG (4 lenses: entry ABI, interrupt/exception paths, concurrency, discriminator vacuity), folded
  in:** the probe-data publish race; the hook re-entry guard; the LDT and SYSENTER layers; the wider GDT walk;
  the exact-RIP origin check; a wrong-place NT control (dispatch counter fires; it also measured the
  `context_switch` NT leak); doc corrections (sink containment, pre-fix severity with a kernel RSP, the
  futex fault-kill path below).
- **Side finding, not fixed on this branch:** every error-code `x86-interrupt` handler calls out with RSP ≡ 8
  (mod 16) (LLVM bug 45215); a string `==` in the hook recursed on #GP(0) until the stack overflowed, so the
  hook takes an enum. Static analysis also finds an aligned SSE store on the production #PF fault-kill path
  (`futex::recompute_earliest`, via `purge_pid`, when the process has a timed futex waiter), not yet run.
  Written up under CP-48. Say whether you agree with the measurement and with leaving the fix to its own branch.
- `docs/concurrency-pitfalls.md`: CP-47, CP-48, index rows, and the CP-40 audit table's #BR and #OF rows.

## Your job: refute it

Default to suspicious. Specifically try to break:

1. **NT elsewhere.** Is there any other way the kernel can run with NT=1: an entry path that does not clear it,
   a kernel `popfq` of a user-supplied value, a path that builds an RFLAGS image and loads it in ring 0? Is "IRQ
   and exception delivery clears NT" true for every gate type this kernel installs?
2. **The compat-mode closure.** Is ring 3 left with *any* way into compatibility mode (another GDT slot, an LDT,
   a call gate, IRETQ with a crafted CS)? Does a null descriptor in slot 3 break anything (STAR, SYSRET
   arithmetic, any code that loads `0x18`/`0x1B`, debuggers, bare-metal firmware expectations)?
3. **The CSTAR sink.** Is `cli; hlt; jmp` really stack-free on entry with a user RSP, user GS and IF=0? What do
   NMI, #MC and SMI do there? Is halting better than an alternative (e.g. a proper compat entry that kills the
   process), given the slot is gone?
4. **Discriminator vacuity.** Can `syscall_nt_masked` pass while NT reaches the kernel, or
   `compat_mode_unreachable` pass while compatibility mode is reachable? Check the probe's RFLAGS capture (no
   flag-changing instruction between `pushfq` and `syscall`), the GDT walk (TSS 16-byte entries, limits), the
   `#GP(0x18)` expectation, and the GS handling in `syscall_msr_trap`.
5. **Anything else** a Claude reviewer would structurally miss (Intel vs AMD vs QEMU TCG behaviour of SYSCALL in
   compatibility mode is one candidate).

A finding bounces the branch only if it survives your own re-verification; uncertain items go in the RETURN as
notes. "No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/cp47-cp48-crosscheck-janus   (git fetch origin && git checkout -B codex/cp47-cp48-crosscheck-janus origin/claude/magical-sagan-pi6azh)
```

Read this dispatch and the code from the branch (it is not on `main`):

```
git fetch origin
git show origin/claude/magical-sagan-pi6azh:Plans/DISPATCH-2026-10-18-cp47-cp48-syscall-msr-codex-crosscheck.md
git diff origin/main...origin/claude/magical-sagan-pi6azh
```

Building/running is optional: `NEO_SMP=4 cargo xtask neo-test syscall_nt_masked` and
`NEO_SMP=4 cargo xtask neo-test compat_mode_unreachable` should PASS. Removing `RFlags::NESTED_TASK` from SFMASK
should make the first FAIL with `unexpected_trap trap=general_protection_fault`; restoring the old slot-3
descriptor (`0x00CF_FA00_0000_FFFF`) and skipping the CSTAR write should make the second FAIL.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='SYSCALL SFMASK CSTAR compatibility mode GDT user code segment'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-18-cp47-cp48-codex-crosscheck-janus.md` to `codex/cp47-cp48-crosscheck-janus` and
**push it**. A review delivered only in chat does not exist. Contents: verdict (MERGE / MERGE-WITH-NITS /
BOUNCE), each finding with severity + file:line + repro or reasoning, and for each numbered item above what you
checked. Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** this is a review lane; if you built, run `cargo clean` in your worktree afterwards. Never
delete the worktree itself.
