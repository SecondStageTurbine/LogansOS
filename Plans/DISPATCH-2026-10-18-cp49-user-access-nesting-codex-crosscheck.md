<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family adversarial review of CP-49 (`with_user_access` nesting guard) → Chad (Codex)

**Type:** review only (no code to merge). **Deliverable:** a pushed RETURN file. **Why a Codex seat:** the
author and the AAG lenses on this branch are Claude-family. The change itself is small, but its soundness
argument rests on interrupt/exception entry and `context_switch` behaviour, so a different-family lens is
the useful check. It follows up Jarvis's CP-42 review note (`ca4f027c` on `codex/cp42-crosscheck-jarvis`)
and Janus's #22 note on blocking closures (`codex/cp40x-crosscheck-janus`).

## What is under review

Branch `claude/adoring-mendel-kgbzsi`. Review the whole diff
(`git diff origin/main...origin/claude/adoring-mendel-kgbzsi`). Besides this dispatch, it is four files:

- `kernel/src/arch/x86_64/mod.rs`: when SMAP is active, `with_user_access` now checks RFLAGS.AC on entry. If
  AC is set, it runs `clac` (so the panic path runs under SMAP) and panics ("nested call or leaked SMAP
  window (CP-49)"). Its doc comment states the rule:
  closures must not nest a call and must not block, yield or otherwise reach `context_switch`.
- `kernel/src/test_harness.rs`, `kernel/Cargo.toml`, `tools/xtask/src/main.rs`: new scenario
  `user_access_nesting` (feature `neo-test-user-access-nesting`).
  - Leg 1 reads a mapped user page in one window and checks AC is clear afterwards.
  - Leg 2 nests a call and then reads again in the outer closure.
  - With the guard, `panic_failed` turns only the guard's own panic into PASS. The flag is armed inside the
    outer window just before the inner call, and the panic must be raised in `arch/x86_64/mod.rs` with the
    guard's message.
  - Without the guard, the outer read takes a SMAP `#PF`, reported as
    `reason=outer_window_closed_by_nested_with_user_access` (FAIL 4/4 locally).
- `docs/concurrency-pitfalls.md`: CP-49 row and entry.

**The soundness claim.** Kernel code outside a `with_user_access` window always runs with AC=0:
- SYSCALL masks AC through SFMASK (`syscall.rs`);
- `swapgs_irq_entry` and both timer stubs clear AC before Rust (CP-40);
- every exception vector enters through the `set_exception_entry!` shim (`idt.rs`, #22), which clears AC.

So AC=1 at entry means a lexical nest or an AC leaked from a blocked window, and the assertion cannot fire on
a correct path. In particular, a `with_user_access` in IRQ or exception context, interrupting an open window,
sees AC=0.

## Your job: refute it

Default to suspicious. Specifically try to break:

1. **False positives.** Find any correct kernel path that reaches `with_user_access` with AC=1 and SMAP active.
   Candidates:
   - the boot/AP order around `enable_sse` (`clac` then `SMAP_ACTIVE.store`);
   - a new thread's first run (it inherits the switcher's RFLAGS through `context_switch`'s `ret`);
   - kernel threads;
   - the ring-3 `#PF` kill path ending in `scheduler::exit()`;
   - any `popfq` that restores a saved RFLAGS;
   - NMI and `#DF` (IST) entry.

   A false positive halts the kernel, so this item matters most.
2. **False negatives.** Can a nested call slip past? For example, SMAP active but `SMAP_ACTIVE` read as false
   on one CPU, or an inner call that the compiler hoists or merges. Can a leaked AC=1 reach a window without
   tripping the check?
3. **Ordering.** The check is `x86_64::registers::rflags::read_raw()`, which is `pushfq; pop` with
   `options(nomem, preserves_flags)`. It runs before `stac`. Can the compiler move the RFLAGS read after
   `stac`, or merge two calls' reads? Consider that neither asm block is `pure`.
4. **The preemption argument.** The CP-49 entry says a window preempted with IF=1 is safe: the timer stub
   clears AC for its body, and `iretq` restores the interrupted AC. Check this for both timer-stub variants
   and for a switch to another thread and back.
5. **The discriminator.** Can `user_access_nesting` pass vacuously? Consider SMAP off, the panic from
   somewhere else, the flag armed too early, CR2/PF expectation mismatches, or the `user_data_page` mapping
   not being user-accessible. Does the control really fail for the stated reason?
6. **The CP-49 text.** Is each factual claim right? In particular the "Not covered" list for blocking
   closures, and the Linux comparison (`arch/x86/include/asm/uaccess.h`, objtool's "recursive UACCESS
   enable" and call-with-UACCESS warnings).
   Read raw source, never a summarizing fetch.
7. **Anything else** a Claude reviewer would structurally miss.

A finding bounces the branch only if it survives your own re-verification; uncertain items go in the RETURN as
notes. "No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\chad     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/cp49-crosscheck-chad        (git fetch origin && git checkout -B codex/cp49-crosscheck-chad origin/claude/adoring-mendel-kgbzsi)
```

Read this dispatch and the code from the branch (it is not on `main`):

```
git fetch origin
git show origin/claude/adoring-mendel-kgbzsi:Plans/DISPATCH-2026-10-18-cp49-user-access-nesting-codex-crosscheck.md
git diff origin/main...origin/claude/adoring-mendel-kgbzsi
```

Building is optional. `cargo xtask neo-test user_access_nesting` should PASS with
`expected_panic=with_user_access_nested`. With the assertion's condition forced true, it should FAIL with
`reason=outer_window_closed_by_nested_with_user_access`.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='RFLAGS AC cleared on kernel entry'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-18-cp49-codex-crosscheck-chad.md` to `codex/cp49-crosscheck-chad` and **push
it**. A review delivered only in chat does not exist. The RETURN contains:
- a verdict: MERGE, MERGE-WITH-NITS or BOUNCE;
- each finding with its severity, file:line, and a repro or reasoning;
- for each numbered item above, what you checked.

Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree afterwards. Never
delete the worktree itself.
