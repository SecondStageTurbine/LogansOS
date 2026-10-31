<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-61 C0.5 std overlay — cross-family adversarial review → Chad

**Date:** 2026-10-21
**From:** S3 (Claude family; author of the branch), for the PM to relay
**To:** **Chad** (main-tier; **Codex family**). Janus is an acceptable substitute (same family).
**Why Codex:** the writer is Claude. A same-family review of a "no silent-wrong stubs" claim is weak
evidence (CLAUDE.md §Cross-model diversity), so the refute-lens must come from a different family.
**Status:** DISPATCHED. A **review, NO-GATE** lane: you push a RETURN doc; you do not change the branch
under review and you run no QEMU gate.
**Under review:** `claude/ot61-c05-std-overlay`. Head SHA: see the PR titled
`OT-61 C0.5 … [DO NOT MERGE: PL decision]`, and review the head named there.
**Merge:** PL-reserved. Your verdict feeds the PL's decision; nobody merges on it alone.

## What was built (read these first)

```
git fetch origin
git show origin/claude/ot61-c05-std-overlay:docs/RESEARCH-2026-10-21-ot61-c05-FINDINGS.md   # the claims + evidence
git show origin/claude/ot61-c05-std-overlay:Plans/NOTE-2026-10-21-ot61-c05-plan-s3.md        # the scaffold audit
git diff origin/main...origin/claude/ot61-c05-std-overlay --stat                             # the surface
git show origin/main:docs/ADR-078-ot61-browser-c-ladder.md                                   # the authority (§3 C0.5)
```

The main files are:

- `userland/neoplutarch-std-overlay/` (`apply-overlay.py`, `std/*.rs`)
- `userland/neo-std-pal/a4_gate.rs` + `build.rs`
- `tools/xtask/src/ot61_c05_tests.rs`
- `userland/std-real-smoke/src/{main,panic}.rs`
- the `std_real_smoke` arm in `kernel/src/test_harness.rs`
- the xtask staging, overlay-stamp and serial-needle code (grep `std_real` in `tools/xtask/src/main.rs`)

The overlay patches the toolchain's rust-src. To see the patched std, run
`python3 userland/neoplutarch-std-overlay/apply-overlay.py` in your tree (it is reversible: it keeps
`*.neo-pristine` backups and restores from them on every apply). Then read
`$(rustc --print sysroot)/lib/rustlib/src/rust/library/std/src/sys/`.

## What to REFUTE (default to suspicious; each finding needs a concrete repro or code path)

1. **Overlay completeness: no silent-wrong stubs.** The claim is that every `sys/*` surface a std
   program can reach is either implemented correctly or fails **loudly** (`Err(Unsupported)`, or a panic
   that aborts with exit 134). Hunt for any std API on this target that returns a plausible-but-wrong
   `Ok`. Candidates to check:
   - `env::var`/`args` empty;
   - `thread::yield_now`/`set_name` no-ops;
   - `is_terminal`;
   - `io::Error::last_os_error()` with `errno() == 0`;
   - `System` dealloc leaking;
   - `Stdout::flush`;
   - the `no_threads` sync primitives (are they sound given `thread::spawn` is refused?);
   - `hashmap_random_keys`;
   - `std::time`;
   - the 14 audited fallbacks in `apply-overlay.py` (is the list complete for this toolchain's
     `sys/mod.rs`?).

   Is "accurately empty" (no argv, no env) honestly distinguishable from "silently wrong"?
2. **The errno mapping.** `io_error_neoplutarch.rs` `ERRNO_TABLE` versus `kernel/src/errno.rs`. Is any
   kind wrong in a way that changes std control flow? Specifically:
   - `WouldBlock`/`Interrupted`/`TimedOut` drive retry and wait logic. Check `EAGAIN → WouldBlock` and
     `EBADF`/`EFAULT → InvalidInput`.
   - Check the `EALREADY → AlreadyExists` and `ECANCELED → Uncategorized` choices.
   - Check that `is_interrupted ≡ false` is safe.
   - Does the KAT (`tools/xtask/src/ot61_c05_tests.rs`) actually prove exhaustiveness? Could its line
     parser silently skip a row?
3. **The panic and abort path.** `panic_abort → __rust_abort → abort_internal → neoplutarch_rt::abort →
   SYS_EXIT(134)`. Check:
   - Is there any std path to `core::intrinsics::abort()`/`ud2` that does NOT go through
     `abort_internal`, and would therefore still halt the machine via the kernel `#UD` handler (OT-83)?
     Look at double panic, panic-in-hook, `rtabort!`, `handle_alloc_error`, and stack overflow.
   - Can the panic message be lost before the exit?
   - Is the patch to the shared `pal/unsupported/common.rs` correctly `cfg`'d so other targets are
     unaffected?
4. **The A4 manifest gate's teeth.** `a4_gate.rs` rules G1–G5 are lexical. Construct a source edit to the
   PAL or the overlay that issues an off-manifest syscall **and passes the gate**. Avenues include
   macros, a renamed trampoline, a `syscall3` defined with a different parameter name, a second `fn` on
   the same line, a `#[path]`/`include!` of an unscanned file, block comments, raw strings, an `asm!`
   template spread across lines, and code in a non-`.rs`/`.sh`/`.py` file. Each bypass you find is a
   finding: state whether the fix is in proportion, or whether it is a stated limit.
5. **The no-new-syscall / NX claims.** Verify:
   - the empty diff on `kernel/src/arch/x86_64/syscall.rs`, `errno.rs` and libneo;
   - that `SYS_MAP_REGION` maps `USER_DATA_RW` with `NO_EXECUTE`;
   - that the ELFs have no W+X or `PT_TLS` segment;
   - that the only `syscall` instructions are in the two trampolines (`objdump -d`).
6. **The default-build identity.** FINDINGS §4.4 claims all 12 default ISO-tree files are byte-identical
   to `main` with `NEO_BUILD_ID` pinned. Reproduce it, or refute it. Is there any path by which a
   *default* build touches the overlay, the std smoke, or the shared rust-src?
   (`build_user()` sets `stage_std_real_smoke(false)`.) Also: rust-src is patched in place, so does a
   later *non-scenario* `-Z build-std` build (the kernel, neo-init) compile differently because the
   patched files are present? The patches are `cfg(target_os = "neoplutarch")`; check that this holds
   for the `sys/mod.rs` and `common.rs` edits.

## Rules of the review

- A finding counts only if it comes with a concrete repro or code path. Uncertain items are RETURN
  notes, not blockers (a gate that cries wolf is a fired KC-5).
- "No findings" on a lens requires stating what you checked.
- Don't rubber-stamp, and don't refactor: you are refuting, not rewriting.
- The verdict is one of **SOUND / SOUND-WITH-NITS / BOUNCE**, with the surviving findings ranked.

## Deliverable + close

- **Artifact:** `Plans/RETURN-2026-10-21-ot61-c05-codex-review-chad.md` (CC-BY-4.0). It contains:
  - §0, the verdict;
  - one section per lens 1–6 (what you checked, what you found, the repro);
  - the commands you ran.
- **Push it.** A review delivered only in chat does not exist (CLAUDE.md §Work is only real when it is on
  `origin`). Branch: `codex/ot61-c05-codex-review-chad`, cut from `origin/main`, containing **only**
  your RETURN doc.
- Close condition: the RETURN is on `origin` and its path is posted back to the PM.
- **Disk reclaim:** a review lane builds little. If you built the branch in your tree, `cargo clean` it
  once the RETURN is pushed. Never delete the tree, and never delete it while the RETURN is unpushed.

## Relay — what to tell Chad

```
WORKTREE: E:\AI Automation\workers\chad     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot61-c05-codex-review-chad  (git fetch origin && git checkout -B codex/ot61-c05-codex-review-chad origin/main)

Read the dispatch (it lives on the branch under review, not on main):
  git fetch origin
  git show origin/claude/ot61-c05-std-overlay:Plans/DISPATCH-2026-10-21-ot61-c05-codex-review.md

One line: cross-family (you=Codex, author=Claude) adversarial review of OT-61 C0.5 — refute the std
overlay's no-silent-wrong-stubs claim, the errno table, the panic->exit-134 path, the A4 gate's teeth,
the no-new-syscall/NX claims, and the default-build byte identity. Push
Plans/RETURN-2026-10-21-ot61-c05-codex-review-chad.md on your branch. Do not edit the reviewed branch.

RAG (semantic code search) is available for this project — use it for conceptual "where is X
done?" questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: Invoke-RestMethod http://127.0.0.1:8404/health  → expect {"status":"healthy",...}.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct
file reads (never a blocker). Optional self-serve start (coders run PowerShell):
  powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
  cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
Query e.g.: $body=@{query='sys_write fd console file handle table'; top_k=5}|ConvertTo-Json;
  Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"
(The RAG indexes main; the overlay/gate files are branch-only — read those with git show.)
```
