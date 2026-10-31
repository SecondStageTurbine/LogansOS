<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family adversarial review of CP-42 (`stac`/`clac` without `nomem`) → Jarvis (Codex)

**Type:** review only (no code to merge). **Deliverable:** a pushed RETURN file. **Why a Codex seat:** the
author and both AAG lenses on this branch are Claude-family. CLAUDE.md's *mandatory* cross-family lens covers
SMP / concurrency-core merges, and this change (the SMAP window primitives) is not on that list. CLAUDE.md
still says to prefer a different-family refute lens wherever one can be routed, so this review is preferred
rather than mandatory. The PM coordinator asked for it.

## What is under review

PR #20, branch `claude/adoring-mendel-kgbzsi`. Review the whole diff
(`git diff origin/main...origin/claude/adoring-mendel-kgbzsi`). Besides this dispatch, it is two files:

- `kernel/src/arch/x86_64/mod.rs`: `stac()` and `clac()` went from `asm!(…, options(nomem, nostack))` to
  `options(nostack)`. `with_user_access(f)` does `if SMAP_ACTIVE { stac() }; let r = f(); if SMAP_ACTIVE {
  clac() }; r`. Its callers (`arch/x86_64/syscall.rs`, `sched/futex.rs`) do `read_unaligned` /
  `write_unaligned` / `copy_nonoverlapping` / `read_volatile` / `write_volatile` on user pointers inside `f`.
- `docs/concurrency-pitfalls.md`: the CP-42 table row and entry.

**The claim.** With `nomem`, LLVM may move loads and stores across `stac`/`clac`. A user access moved out of
the window takes a SMAP `#PF`; a kernel access moved in runs with SMAP lifted. Measured on `57261d66`, in a
release (LTO) build with `product,browser-runtime,kernel-rng,kernel-sysstat,kernel-observability,net-dial-cap,virtio-accel,framebuffer-console`,
7 of 35 windows had user accesses after `clac`:
- `sys_futex_wait`, `sys_net_stats` and two windows in `sys_readdir` were empty;
- `sys_tensor_query`, `sys_model_query` and `sys_inspect` wrote part of their out-struct after `clac`.

The mechanism is LLVM tail-merging the stores both paths share (SMAP on: `stac; f; clac`; SMAP off: `f`) into
the join block. At dev opt-level 2 with the same features, `futex::wait`'s `read_volatile` sat after `clac`. At
opt-level 0, which every `neo-test` scenario and CI uses, `stac`/`clac` are out-of-line calls and nothing
moves. That makes the defect **latent**, and **no gate scenario can show the fix working or failing**. The
evidence is the disassembly.

## Your job: refute it

Default to suspicious. Specifically try to break:

1. **Is the barrier real?** With `nomem` removed, can any compiler pass still move an access to user memory
   before `stac` or after `clac` on the SMAP-on path? Consider:
   - the Rust Reference rules for asm without `nomem`/`readonly`;
   - LLVM `asm sideeffect` with no `memory(...)` attribute, and whether it gets a `~{memory}` clobber;
   - pointers made by int-to-ptr casts from user addresses (provenance / exposure);
   - the passes that could move accesses: GVN, LICM, MemCpyOpt, SimplifyCFG sink/hoist, InstCombine store
     merging, MachineSink, machine scheduling.

   Check with `--emit=llvm-ir` / `--emit=asm` on a small crate if useful.
2. **The options.** Is `nostack` correct? Is leaving out `preserves_flags` right? What flags does it cover, and
   is AC among them? Should anything else differ?
3. **Completeness.**
   - Find any other asm that opens, closes or toggles SMAP and still carries `nomem`, `readonly` or `pure`. The
     entry says `write_cr4_raw` keeps `nomem` because it sets CR4.SMAP only at CPU bring-up: verify that,
     including AP bring-up in `smp.rs` and the SMEP/SMAP scenarios in `test_harness.rs`.
   - Find any user-pointer access that bypasses `with_user_access`.
4. **The evidence.** Rebuild before (`57261d66`) and after (the branch tip) with the feature set above, using
   `--release`, and disassemble with the toolchain's `llvm-objdump -d -M intel`.
   - Confirm the 7 changed windows and that no other window's contents change.
   - Then look for any window in the *after* binary where a user access is still outside `stac … clac` **on
     the path that executed `stac`**. The duplicate copy on the SMAP-off path, reached via the `je` before
     `stac`, is expected.
5. **Interactions.** Since CP-40 (#15, now on `main`), the IRQ and timer stubs clear AC on entry, but an
   `x86-interrupt` exception taken inside a window still runs its handler with AC=1 (CP-40's "Not yet
   covered"). A UB-check panic branch inside a window also leaves it with AC=1. Confirm or refute that the
   first is out of scope for this PR and that the second is harmless (`panic = "abort"`, the handler halts).
6. **The CP-42 text.** Is each factual claim right?
   - "Latent" exposure.
   - `smap_catch` / `smap_policy` never go through `with_user_access`.
   - The qwen35 opt-level-2 scenario's 18 windows are identical.
   - Linux `arch/x86/include/asm/smap.h` `stac()`/`clac()` go through `alternative()` with a `"memory"`
     clobber. Read the raw file, never a summarizing fetch.
7. **Anything else** a Claude reviewer would structurally miss.

A finding bounces the branch only if it survives your own re-verification; uncertain items go in the RETURN as
notes. "No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/cp42-crosscheck-jarvis        (git fetch origin && git checkout -B codex/cp42-crosscheck-jarvis origin/claude/adoring-mendel-kgbzsi)
```

Read this dispatch and the code from the branch (it is not on `main`):

```
git fetch origin
git show origin/claude/adoring-mendel-kgbzsi:Plans/DISPATCH-2026-10-17-cp42-smap-asm-codex-crosscheck.md
git diff origin/main...origin/claude/adoring-mendel-kgbzsi
```

Building is optional, but item 4 needs it. From `kernel/`:
`cargo build -Zjson-target-spec --release --bin kernel --features <set above>` into a separate
`CARGO_TARGET_DIR` per tree. `llvm-objdump` is under `$(rustc --print sysroot)/lib/rustlib/<host>/bin/`.
`cargo xtask neo-test smap_catch smap_policy` should PASS, but those scenarios cannot discriminate this fix.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='with_user_access stac clac SMAP window'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-17-cp42-codex-crosscheck-jarvis.md` to `codex/cp42-crosscheck-jarvis` and
**push it**. A review delivered only in chat does not exist. The RETURN contains:
- a verdict: MERGE, MERGE-WITH-NITS or BOUNCE;
- each finding with its severity, file:line, and a repro or reasoning;
- for each numbered item above, what you checked.

Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree afterwards. Never
delete the worktree itself.
