# DISPATCH: Kernel-rename bundle + ADR-020 LockOwner foundation + ADR-019 sub-phase A (IrqSafeMutex introduction)

**Date:** 2026-05-10 (later session, post-ADR-ratification)
**Routed to:** Jarvis (Codex)
**Type:** Three-sub-commit kernel-tier dispatch. First Jarvis dispatch on this project (Jarvis = Codex per `Plans/PM-NOTE-2026-05-10-agent-roster-update.md`). Calibration data point — gates tight at each sub-commit boundary; PM reviews each gate's verification before next sub-commit begins where practical.
**Sequencing:** Single-recipient Jarvis dispatch. Sub-commits execute sequentially: 1 → gate → 2 → gate → 3 → gate → push. Each sub-commit has its own gate; do not advance to next sub-commit if current gate fails.
**Coordination constraint:** Negative-list — `docs/`, `Plans/` (except for adding new artifact files like verification logs), `README.md` all OFF-LIMITS. Kernel-tier scope only: `kernel/`, `boot/`, `tools/`, `userland/`, `tests/`, root build files (`Cargo.toml`, `Cargo.lock`, `Dockerfile`, `docker-compose.yml`), target JSON files. Verification logs may land under `Plans/`.
**Consumes:** `Plans/KERNEL-BIBLE-ALIGNMENT-AUDIT-2026-05-10.md` (sub-commit 1 spec), `docs/ADR-019-switchmutex-irq-off-by-construction.md` (sub-commit 3 spec), `docs/ADR-020-owner-metadata-production-diagnostic.md` (sub-commit 2 spec).

---

## 0. Background and context

SC-3B closed 2026-05-10 via the per-call-site `without_interrupts(...)` discipline. Three ADR proposals (018/019/020) ratified by PL 2026-05-10 (evening session). Prometheus landed the ratification cycle in six commits today (`5558a4d` through `fa8ec7c`). This Jarvis dispatch executes the downstream kernel-side work in three sub-commits:

1. **Sub-commit 1 — Kernel-side rename** completing the 2026-05-10 project rename's substrate surface (kernel/, boot/, tools/, userland/, tests/, root). Includes the A3 split-identity boot decision (NEO PLUTARCH wordmark + "Intelligence as a kernel primitive." tagline byline). Spec: `Plans/KERNEL-BIBLE-ALIGNMENT-AUDIT-2026-05-10.md` §3 and §4.

2. **Sub-commit 2 — ADR-020 LockOwner foundation.** Adds `LockOwner` field to `SwitchMutex` with seqlock-protected NMI-safe reads. Spec: `docs/ADR-020-owner-metadata-production-diagnostic.md` §"Concrete proposal".

3. **Sub-commit 3 — ADR-019 sub-phase A: introduce `IrqSafeMutex`.** Creates `IrqSafeMutex<T>` as a sibling type to `SwitchMutex`, with `LockOwner` integrated from start (since sub-commit 2 landed the type). Includes the micro-bench per ADR-019 Q4. Spec: `docs/ADR-019-switchmutex-irq-off-by-construction.md` §"Concrete proposal API sketch" and §"Implementation phases" sub-phase A.

The three sub-commits are conceptually independent but mechanically related — they all modify the kernel sync/lock substrate. Bundling reduces gate-cycle overhead vs. three separate dispatches.

**Ordering rationale:** sub-commit 1 (rename) is purely textual/build-config; lowest substantive-code risk; first calibration data point for new-Jarvis. Sub-commit 2 lands the LockOwner foundation before sub-commit 3 introduces `IrqSafeMutex`, so the new type is fully-formed with `LockOwner` from its first commit (avoids backfilling `LockOwner` onto a brand-new type in adjacent commits — cleaner code-review surface).

---

## 1. Sub-commit 1 — Kernel-side rename + A3 boot identity

### 1.1 Scope

Implement the kernel-side rename per `Plans/KERNEL-BIBLE-ALIGNMENT-AUDIT-2026-05-10.md` §3 and §4. The audit is the authoritative target list — every entry in the tables in audit §3.1 through §3.7 is in scope; audit §4 specifies the boot identity (A3 decision).

### 1.2 Authoritative spec

Read `Plans/KERNEL-BIBLE-ALIGNMENT-AUDIT-2026-05-10.md` end-to-end before starting. The audit's tables enumerate every file/line/current/target. Execute each rename per the tables. Specifically:

- **§3.1 Cargo packages + target files** — rename package `altimit-neo-kernel` → `neoplutarch-kernel`; rename target JSON files via `git mv`; update all `.cargo/config.toml` references.
- **§3.2 xtask build orchestration** — update 8 lines in `tools/xtask/src/main.rs`.
- **§3.3 Userland crates** — update Cargo.toml descriptions + lib.rs / main.rs doc comments.
- **§3.4 Boot config** — update `boot/limine.conf` 3 lines.
- **§3.5 Kernel linker + Dockerfile + Compose** — update header comments + docker image/container names.
- **§3.6 Tests README** — single-line update.
- **§3.7 Kernel source comments** — mechanical text-replace of comments.
- **§4 Boot identity (A3)** — update framebuffer wordmark (`kernel/src/init/boot_frame.rs:40`) to "NEO  PLUTARCH" (preserve double-space cinematic split) and add tagline rendering for "Intelligence as a kernel primitive." below; update serial banner (`kernel/src/main.rs:254`) to two-line "NEOPLUTARCH v{}" + "Intelligence as a kernel primitive."

### 1.3 Boot rendering — A3 implementation detail

If the existing framebuffer rendering infrastructure in `kernel/src/init/boot_frame.rs` does not natively support a subtitle line below the centered wordmark, add minimal support: one additional rendering call below the wordmark, same horizontal centering, smaller text size or lower-intensity color to match an "H1 over blockquote" visual hierarchy (the README's structural analog). This is acceptable scope within sub-commit 1.

If rendering smaller text or color variation is not currently supported by the framebuffer driver and would require non-trivial driver work to add, fall back to rendering "Intelligence as a kernel primitive." at the same size as the wordmark but on a separate centered line below it. Surface this choice in the sub-commit's verification log.

### 1.4 Cargo.lock

After all `Cargo.toml` package-name changes land, run `cargo k` (or equivalent build invocation) to regenerate `Cargo.lock`. Do not hand-edit `Cargo.lock`. The expected diff to `Cargo.lock` is purely name-substitution (`altimit-neo-kernel` → `neoplutarch-kernel`, etc.) with no version or dependency changes. If `Cargo.lock` diff shows unexpected additions/removals, STOP and surface to PM.

### 1.5 Out-of-scope for sub-commit 1

- `CLAUDE.md` — explicitly out of scope per audit §5 (RAG indexer config; PL controls separately).
- `Plans/` — preserved per Bible 18.1.
- `docs/` — Prometheus already handled docs-side rename in prior commits.
- `boot-log-*.txt` at project root — historical logs; never edit.
- `target/` — build artifacts.

### 1.6 Sub-commit 1 gate

Run after all rename edits complete:

```bash
# Build verification:
cargo k                                            # expect: PASS, baseline delta 0 from prior PASS state, 18 warnings (or current baseline)

# Run full SMP integration test gate:
cargo neo-test all                                 # default config (NEO_SMP=4 per CI)
# Expect: all serial logs PASS; no NO_SERIAL or MISSING_TERMINAL_FRAME results

# Visual boot verification:
cargo neo-tty                                      # boot in QEMU with TTY visible
# Expect: serial output shows two-line banner
#   "  NEOPLUTARCH  v0.x.y"
#   "  Intelligence as a kernel primitive."
# Expect: framebuffer (if visible) shows the wordmark + tagline byline per A3
# Capture: serial-output snapshot for verification log
```

If any check fails, STOP. Do not advance to sub-commit 2. File the failure artifact (§7) and surface to PM.

### 1.7 Sub-commit 1 commit message

```
kernel: rename AltimitNeo → NeoPlutarch; A3 boot identity

Completes the 2026-05-10 project rename's kernel-side substrate.
Docs-side rename landed via commit cca6889 + Sub-step 6; this commit
finishes the rename across kernel/, boot/, tools/, userland/, tests/,
and root build files per Plans/KERNEL-BIBLE-ALIGNMENT-AUDIT-2026-05-10.md.

Cargo package altimit-neo-kernel → neoplutarch-kernel; target JSON
files renamed (x86_64-altimit{,-user}.json → x86_64-neoplutarch{,-user}.json
via git mv); .cargo/config.toml files updated; tools/xtask constants
updated; userland crate descriptions and doc comments updated; boot
config (limine.conf) updated; Dockerfile + docker-compose container
names updated; kernel source doc comments and visible-string banners
updated.

A3 boot identity (PL ratification 2026-05-10):
- Framebuffer wordmark: "ALTIMIT  NEO" → "NEO  PLUTARCH" (preserves
  cinematic double-space split-word feel)
- Tagline byline: "Intelligence as a kernel primitive." rendered
  below the wordmark
- Serial banner: two-line "NEOPLUTARCH v{}" + tagline

Bible 25.2 neo- prefix preserved: neo-init, neo-worker, libneo, .neom,
NeoFS unchanged.

Out-of-scope (preserved unchanged):
- CLAUDE.md (RAG indexer config; PL controls separately)
- Plans/ (historical record per Bible 18.1)
- docs/ (Prometheus prior commits)
- Referential mentions of "original ALTIMIT OS" and ".hack ALTIMIT
  visual language" in docs/design.md (TM-defensible homage)

Verification: cargo k PASS delta 0; cargo neo-test all NEO_SMP=4 PASS;
cargo neo-tty visual boot verified; serial-output snapshot in
Plans/KERNEL-RENAME-SUB-COMMIT-1-VERIFICATION-2026-05-10.md.

Cross-references:
- Plans/KERNEL-BIBLE-ALIGNMENT-AUDIT-2026-05-10.md
- docs/bible.md Article 25 (project name authority)
- docs/design.md (vision narrative; aesthetic referent)
```

### 1.8 Sub-commit 1 verification log

File `Plans/KERNEL-RENAME-SUB-COMMIT-1-VERIFICATION-2026-05-10.md` recording:

- The git diff stat for sub-commit 1.
- `cargo k` output (PASS confirmation + warning count delta).
- `cargo neo-test all` summary (test count, pass count, any deliberate-FAIL meta-test results).
- Serial-output snapshot from `cargo neo-tty` showing the new banner.
- Notes on any sub-decision taken (e.g., subtitle rendering fallback if `kernel/src/init/boot_frame.rs` didn't support smaller text natively).

---

## 2. Sub-commit 2 — ADR-020 LockOwner foundation

### 2.1 Scope

Implement `LockOwner` per `docs/ADR-020-owner-metadata-production-diagnostic.md` §"Concrete proposal". Integrate with `SwitchMutex`. Cfg-gate behind `kernel-diagnostic` feature; default on for development and release builds (per ADR-020 Q1 ratification).

### 2.2 Authoritative spec

Read `docs/ADR-020-owner-metadata-production-diagnostic.md` end-to-end. The §"API sketch" provides the structural template; the §"Ratification decisions" answers cover the five Q-decisions:

- **Q1:** Always-on for dev+release; cfg-gated only for embedded (which is not a current target — feature gate is plumbing for future).
- **Q2:** Apply to kernel-owned lock types only. `SwitchMutex` gets `LockOwner` in this sub-commit. `IrqSafeMutex` gets `LockOwner` from start in sub-commit 3 (since it's a new type). Plain `spin::Mutex` is NOT modified (external crate; Bible 4.3 discipline).
- **Q3:** Hold-time stats deferred to future ADR. This sub-commit lands cpu/thread_id/acquisition_site/seq only.
- **Q4:** Seqlock pattern (Phase D-proven).
- **Q5:** Independent of 018/019 ratification, bundled here for operational efficiency.

### 2.3 Implementation guidance

Per ADR-020 §"API sketch":

- New file: `kernel/src/sync/lock_owner.rs` containing `LockOwner` struct and `LockOwnerSnapshot` return type.
- `cfg(feature = "kernel-diagnostic")` controls presence:
  - Enabled (default for dev+release): struct contains `cpu: AtomicU32`, `thread_id: AtomicU32`, `acquisition_site: AtomicU32`, `seq: AtomicU32`.
  - Disabled (embedded future): struct is ZST (`pub struct LockOwner;`).
- Methods: `acquire(cpu, thread_id, site)`, `release()`, `read_consistent() -> Option<LockOwnerSnapshot>` (NMI-safe).
- Seqlock discipline: writer increments `seq` to odd before field updates, increments to even after; reader reads `seq` twice and checks for equality + even parity.
- Acquisition-site hash: macro-generated at compile time from `file!()` + `line!()` per ADR-020 §"Acquisition site encoding" PM lean. Suggested macro name: `lock_site!()` returning a `u32` const. Hash function: any cheap compile-time-evaluable hash (FNV-1a or similar; details flexible).

### 2.4 SwitchMutex integration

In `kernel/src/sync/switch_mutex.rs` (or wherever `SwitchMutex` is defined):

- Add `owner: LockOwner` field to `SwitchMutex<T>`.
- In `lock()` (after the underlying spinlock is acquired): call `self.owner.acquire(current_cpu_id(), current_thread_id(), lock_site!())`. The `current_cpu_id()` and `current_thread_id()` helpers should already exist (used by scheduler); if not, add minimal stubs returning u32 from the relevant per-CPU/thread state.
- In `SwitchMutexGuard::drop()` (before the underlying spinlock is released): call `self.owner.release()`.
- Construction: `LockOwner` is `Default`-constructible to an unowned state (cpu = u32::MAX, etc.). Add `Default` impl for `LockOwner`. `SwitchMutex::new()` constructs `owner: LockOwner::default()`.

Per Bible 15.2: any `unsafe` blocks introduced (e.g., if the seqlock atomic discipline requires `unsafe` for reordering control) get `// SAFETY: ...` comments.

### 2.5 Feature gate plumbing

In `kernel/Cargo.toml` (which becomes `neoplutarch-kernel` after sub-commit 1):

```toml
[features]
default = ["kernel-diagnostic"]
kernel-diagnostic = []
```

Verify `cargo k` works both with and without the feature (it's the default-on case that's load-bearing; the off case is checked for buildability only at this stage).

### 2.6 Sub-commit 2 gate

```bash
cargo k                                            # expect: PASS, delta 0
cargo k --no-default-features                      # expect: PASS — verify cfg-gated-off case builds
cargo neo-test all                                 # default config; expect: PASS

# Specific test: lock-owner reads sensible data
# (Add a small test in tests/ or kernel test harness if not present)
# Expected: from a test thread that acquires a SwitchMutex, NMI-handler-style
# read_consistent() returns Some(snapshot) with snapshot.cpu == current_cpu
# and snapshot.thread_id == current_thread.
```

If `cargo k --no-default-features` fails, the cfg-gating is incorrect — fix before proceeding.

If any other check fails, STOP. File failure artifact, surface to PM.

### 2.7 Sub-commit 2 commit message

```
kernel/sync: add LockOwner field to SwitchMutex (ADR-020 foundation)

Implements ADR-020 LockOwner foundation per the ratified spec.
Adds owner metadata (cpu, thread_id, acquisition_site, seqlock counter)
to SwitchMutex with NMI-safe consistent reads via seqlock discipline.

Cfg-gated behind `kernel-diagnostic` feature; on by default for dev
and release builds. Cfg-off path produces ZST LockOwner; no runtime cost.

Acquisition-site hash via compile-time macro lock_site!() from
file!() + line!(). No runtime table lookup.

Bible 15.2 SAFETY discipline followed for any unsafe blocks introduced
by the seqlock atomic ordering.

ADR-020 §"Ratification decisions" answers reflected in implementation:
- Always-on for dev+release; cfg-gated for embedded (Q1)
- Kernel-owned lock types only; plain spin::Mutex untouched (Q2)
- Hold-time stats deferred (Q3)
- Seqlock pattern (Q4)
- Independent of 018/019 (Q5; bundled for operational efficiency)

IrqSafeMutex (forthcoming sub-commit 3) will integrate LockOwner from
its first commit, since the foundation lands here.

Verification: cargo k PASS delta 0; cargo k --no-default-features
PASS (cfg-off case builds); cargo neo-test all NEO_SMP=4 PASS;
lock-owner consistent-read test passes.

Cross-references:
- docs/ADR-020-owner-metadata-production-diagnostic.md (ratified spec)
- docs/bible.md Article 8.5 (critical-section discipline)
- Plans/SC-3B-PHASE-D-FINDINGS-2026-05-10.md (diagnostic-value proof)
```

### 2.8 Sub-commit 2 verification log

File `Plans/ADR-020-SUB-COMMIT-2-VERIFICATION-2026-05-10.md` recording:

- Git diff stat.
- Both `cargo k` invocations (default + `--no-default-features`).
- `cargo neo-test all` summary.
- Lock-owner consistent-read test result.

---

## 3. Sub-commit 3 — ADR-019 sub-phase A: introduce IrqSafeMutex

### 3.1 Scope

Implement ADR-019 sub-phase A per `docs/ADR-019-switchmutex-irq-off-by-construction.md` §"Implementation phases". Introduce `IrqSafeMutex<T>` as a sibling type to `SwitchMutex`. Integrate `LockOwner` from start (since sub-commit 2 landed the foundation). Add the micro-bench per ADR-019 Q4.

No migration of existing callers in this sub-commit. Sub-phase B (migrate `SwitchMutex` mixed-context callers) and sub-phase C (migrate Sub-step-5c-wrapped `Channel.state` sites) are separate future Jarvis dispatches with their own gates.

### 3.2 Authoritative spec

Read `docs/ADR-019-switchmutex-irq-off-by-construction.md` end-to-end. The §"Concrete proposal API sketch" provides the structural template. §"Ratification decisions" covers the five Q-decisions:

- **Q1:** Keep both `SwitchMutex` and `IrqSafeMutex` as sibling types; no rename.
- **Q2:** No ban on plain `spin::Mutex`; SAFETY-comment-style justification required at declaration site for exclusively-process-context uses.
- **Q3:** No re-entrancy. `IrqSafeMutex::lock()` from same thread that holds the lock is undefined behavior.
- **Q4:** Micro-bench in sub-phase A; escalation threshold 50ns/cycle.
- **Q5:** Ratified linked with ADR-018.

### 3.3 Implementation guidance

Per ADR-019 §"API sketch":

- New file (or rename of existing): `kernel/src/sync/irqsafe_mutex.rs`.
- `IrqSafeMutex<T>` struct containing:
  - Inner `spin::Mutex<T>` (existing external crate).
  - `LockOwner` field (from sub-commit 2).
- `IrqSafeGuard<'a, T>` containing:
  - `spin::MutexGuard<'a, T>`.
  - `prior_if_state: bool` (captured pre-disable).

- `lock(&self) -> IrqSafeGuard<'_, T>`:
  1. Read current interrupt-flag state.
  2. Disable local interrupts.
  3. Acquire inner spin::Mutex.
  4. Call `owner.acquire(cpu, thread_id, site)`.
  5. Return guard with `prior_if_state` captured.

- `IrqSafeGuard::drop(&mut self)`:
  1. Call `owner.release()`.
  2. Release inner spin::MutexGuard (`Drop` runs in field-declaration order; release inner guard before re-enabling interrupts so the lock is gone before another acquirer can race on the same CPU).
  3. If `prior_if_state` was true, re-enable local interrupts.

- The asm-sequence ordering matters (this is the territory SC-3A and SC-3B's release-on-switch concerns lived in). If there's any uncertainty about field-drop order, use `ManuallyDrop` to make the sequence explicit.

- Re-entrancy semantics: do not check; if called from same thread that already holds the lock, the inner spinlock spin forever (per ADR-019 Q3). No assertion or panic; the bug surfaces as a hang (which the kernel's NMI watchdog or stress-test gates will catch).

- Panic safety: if a panic occurs with the guard held, the guard's `Drop` runs during unwinding, releasing the lock and restoring IF state. The owner metadata gets `release()` called. This matches normal Rust drop semantics.

Per Bible 15.2: SAFETY comments on all `unsafe` blocks. Particular care around the `disable_local_interrupts` / `enable_local_interrupts` asm helpers — these are existing kernel primitives, but the new caller adds them to a new call site; SAFETY comments explain the invariant.

### 3.4 Micro-bench (ADR-019 Q4)

Implement a small micro-bench measuring:

- 1000 iterations of `IrqSafeMutex::lock()` + immediate `drop` cycle (uncontended).
- 1000 iterations of `spin::Mutex::lock()` + immediate `drop` cycle (uncontended), as baseline.
- Optionally: 1000 iterations of `SwitchMutex::lock()` + drop cycle, as second baseline.

Report mean and stddev (or min/max if simpler) for each. The micro-bench lives in `kernel/src/sync/` test code or in `tests/` — wherever fits the existing kernel-test pattern.

**Escalation threshold:** If `IrqSafeMutex` overhead vs. `spin::Mutex` baseline exceeds 50 nanoseconds per acquire/release cycle, STOP after sub-commit 3 (do not push), file the verification log with the bench results, and surface to PM. PM evaluates whether the overhead is acceptable or whether sub-phase A needs revision.

If overhead is at or below 50ns/cycle, proceed to gate verification.

### 3.5 Sub-commit 3 gate

```bash
cargo k                                            # expect: PASS, delta 0
cargo k --no-default-features                      # expect: PASS — cfg-off LockOwner case still builds
cargo neo-test all                                 # default config; expect: PASS

# Micro-bench:
# Run the bench (exact invocation depends on how it's integrated — could be
# `cargo neo-test --bench irqsafe-mutex` or a specific kernel test target)
# Capture: mean/stddev for IrqSafeMutex vs spin::Mutex vs SwitchMutex
# Expected: IrqSafeMutex overhead vs spin::Mutex baseline ≤ 50ns/cycle

# Optional: spot-check that IrqSafeMutex actually disables interrupts during
# the critical section by acquiring it from a thread with IF=1, observing that
# IF reads as 0 inside the critical section, and that IF restores to 1 after drop.
# This can be a small kernel test if not already covered.
```

If any check fails, STOP. File failure artifact, surface to PM.

If micro-bench overhead exceeds 50ns/cycle threshold, STOP. Do not push. File the verification log with bench results, surface to PM for evaluation.

### 3.6 Sub-commit 3 commit message

```
kernel/sync: introduce IrqSafeMutex (ADR-019 sub-phase A)

Implements ADR-019 sub-phase A per the ratified spec. Introduces
IrqSafeMutex<T> as a sibling type to SwitchMutex. Disables local
interrupts on lock(), restores prior IF state on guard drop, captures
prior IF state in the guard. No re-entrancy (per ADR-019 Q3).

IrqSafeMutex integrates LockOwner (sub-commit 2 foundation) from
its first commit — no backfilling.

No migration of existing callers in this sub-commit. Sub-phases B
(migrate SwitchMutex mixed-context callers) and C (migrate Sub-step-5c-
wrapped Channel.state sites) are separate future Jarvis dispatches.

Plain spin::Mutex remains available for exclusively-process-context
locks per ADR-019 Q2; declaration-site SAFETY-comment-style justification
required (enforcement is code review at this stage).

Panic safety: guard Drop releases lock and restores IF state during
unwinding. No re-entrancy detection (per Q3); same-thread re-acquire
is undefined behavior (manifests as spin-forever hang, caught by NMI
watchdog or stress-test gates).

Bible 15.2 SAFETY comments on all unsafe blocks.

Micro-bench (per ADR-019 Q4):
- IrqSafeMutex lock+drop cycle: <mean> ± <stddev> ns
- spin::Mutex lock+drop cycle:  <mean> ± <stddev> ns (baseline)
- SwitchMutex lock+drop cycle:  <mean> ± <stddev> ns (second baseline)
- Overhead vs spin::Mutex: <delta> ns ≤ 50ns threshold (PASS)

Verification: cargo k PASS delta 0; cargo k --no-default-features PASS;
cargo neo-test all NEO_SMP=4 PASS; micro-bench captured in
Plans/ADR-019-SUB-PHASE-A-VERIFICATION-2026-05-10.md.

Cross-references:
- docs/ADR-019-switchmutex-irq-off-by-construction.md (ratified spec)
- docs/ADR-018-bible-8-5-4-preempt-while-holding-clarification.md (linked declarative discipline)
- docs/ADR-020-owner-metadata-production-diagnostic.md (sub-commit 2 foundation)
- docs/bible.md Article 8.5 (critical-section discipline)
- Plans/CLOSE-REPORT-2026-05-10-sc-3b.md
- Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md
```

### 3.7 Sub-commit 3 verification log

File `Plans/ADR-019-SUB-PHASE-A-VERIFICATION-2026-05-10.md` recording:

- Git diff stat.
- Both `cargo k` invocations.
- `cargo neo-test all` summary.
- Micro-bench results (mean ± stddev for all three lock types, computed overhead vs. baseline).
- Notes on any sub-decision taken (e.g., field-drop ordering with `ManuallyDrop` if used; specific cpu-id/thread-id helper integration; bench invocation specifics).

---

## 4. Pre-flight checks (before sub-commit 1)

Run these BEFORE starting sub-commit 1. Any failure → STOP and surface to PM.

```bash
cd /e/AI\ Automation/AltimitNeo
git status                                                # expect: clean working tree on main (or session branch)
git rev-parse --abbrev-ref HEAD                           # expect: main or claude/* session branch
git rev-parse HEAD                                        # capture: should be fa8ec7c (Prometheus's last commit)
git log --oneline -8                                      # expect: fa8ec7c + 6 commits from Prometheus's ratification dispatch + 3a2cb70 slogan + 27f2f8f README rewrite

# Verify the audit and ratified ADRs exist:
test -f Plans/KERNEL-BIBLE-ALIGNMENT-AUDIT-2026-05-10.md && echo "OK: audit present"
test -f docs/ADR-018-bible-8-5-4-preempt-while-holding-clarification.md && echo "OK: ADR-018 ratified"
test -f docs/ADR-019-switchmutex-irq-off-by-construction.md && echo "OK: ADR-019 ratified"
test -f docs/ADR-020-owner-metadata-production-diagnostic.md && echo "OK: ADR-020 ratified"

# Verify bible at v1.5.0 (post-ratification):
tail -10 docs/bible.md | grep -q "Version: 1.5.0" && echo "OK: bible v1.5.0"

# Verify kernel-side rename has NOT yet happened (sanity check we're at the right starting point):
grep -q "altimit-neo-kernel" kernel/Cargo.toml && echo "OK: kernel/Cargo.toml at pre-rename state"
test -f kernel/x86_64-altimit.json && echo "OK: target JSON at legacy name (pre-rename)"
grep -q 'let title = "ALTIMIT  NEO";' kernel/src/init/boot_frame.rs && echo "OK: boot wordmark at pre-rename state"

# Verify baseline build state before changes:
cargo k 2>&1 | tail -3                            # capture: baseline pass + warning count
```

If any check fails → STOP. Do not attempt repair. File `Plans/JARVIS-FAILURE-2026-05-10-kernel-bundle-preflight.md` with the failing check's output and notify PM.

---

## 5. Session branch and worktree

Use whatever session-branch convention your tooling produces (likely `claude/<session>` or `jarvis/<session>` form). PM will fast-forward `main` after each sub-commit's gate passes. Do not commit directly to `main`; commit on the session branch.

If your tooling expects a specific branch-naming pattern that differs from the project's `claude/*` convention, surface to PM before starting. The convention is descriptive, not load-bearing — what matters is that PL can fast-forward main cleanly after each sub-commit.

---

## 6. Push handling

Per `Plans/PM-NOTE-2026-05-10-hybrid-git-posture.md` push-event policy: this dispatch completes the project rename surface and lands the SC-3B ADR-020 + ADR-019 sub-phase A implementation. Both qualify as project-governance landing class, triggering a push.

After all three sub-commits land on main (via PM fast-forwards) and all gates pass:

```bash
git push origin main
git rev-parse HEAD
git rev-parse origin/main                                 # expect: equal to HEAD
```

If sub-commit 1 succeeds but sub-commit 2 or 3 fails, push the successful sub-commit(s) only — partial progress is acceptable per the gate-tight discipline. Failure artifact surfaces what didn't land and why.

---

## 7. Failure handling

If any step fails or surfaces unexpected state at any sub-commit:

1. Do NOT amend, stash, `git reset`, or otherwise attempt recovery.
2. Leave the working tree as-is on the session branch.
3. File `Plans/JARVIS-FAILURE-2026-05-10-kernel-bundle-<sub-commit-N>.md` with:
   - Which sub-commit was in progress.
   - What was attempted (which step of §1.1-§3.4).
   - What the actual state showed.
   - What the expected state was.
   - Verbatim output of the failing command.
   - Any partial commits already landed on the session branch.
4. Notify PM.
5. PM triages and dispatches either a corrective sub-step or accepts partial progress.

### 7.1 Explicit stop conditions

- Pre-flight check failure (§4) → STOP before sub-commit 1.
- `cargo k` regression (delta > 0 or new errors) at any sub-commit gate → STOP.
- `cargo neo-test all` regression at any sub-commit gate → STOP.
- `cargo neo-tty` boot doesn't show the new wordmark/tagline after sub-commit 1 → STOP, do not advance to sub-commit 2.
- `cargo k --no-default-features` fails after sub-commit 2 → STOP (cfg-gating is incorrect).
- Micro-bench overhead >50ns/cycle threshold after sub-commit 3 → STOP, do not push. File verification log with results, surface to PM.
- `Cargo.lock` diff after sub-commit 1 shows unexpected additions/removals (not pure name-substitution) → STOP.
- Any boot regression (kernel fails to boot in QEMU at any sub-commit) → STOP.

### 7.2 Acceptable partial-success cases

If sub-commit 1 lands cleanly but sub-commits 2 or 3 fail, the rename is still complete — push sub-commit 1, leave 2 and 3 for a follow-up Jarvis dispatch.

If sub-commit 2 lands but sub-commit 3 fails, push sub-commits 1 + 2, leave 3 for follow-up.

This is acceptable because:
- ADR-020 (sub-commit 2) is independent of ADR-019 (sub-commit 3).
- Sub-phases B and C of ADR-019 don't depend on sub-phase A being landed in the same dispatch.
- Partial progress is documented in the failure artifact + the successful sub-commits' verification logs.

---

## 8. Calibration notes (first Jarvis dispatch on this project)

This is the first Jarvis (Codex) dispatch on NeoPlutarch. Per `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §3.1, PM treats the first 2-3 Jarvis kernel dispatches as calibration data. Specifically watching:

- **`unsafe` block justification discipline.** Every `unsafe` block gets a `// SAFETY:` comment naming the invariant. CI fails if missing per existing project discipline.
- **`// SAFETY:` comment fidelity.** Comments should name the specific invariant being upheld, not just say "this is safe."
- **`no_std` constraint adherence.** Kernel code is `#![no_std]`; no `std::` imports, no allocator-dependent constructs outside `alloc` where the kernel has set it up.
- **Error-propagation hygiene per Bible 15.3.** No `unwrap()` or `expect()` in kernel paths reachable after `kmain` init returns. Init code may panic on failure.

PM will review each sub-commit's verification log and code diff before fast-forwarding main. Substantial-quality-issue findings get surfaced to PM as inline comments on the dispatch report, not as failure artifacts — calibration is descriptive, not pejorative.

Sub-commit 1 (rename) is the lowest-risk calibration entry point — primarily textual changes. If sub-commit 1 lands cleanly, PM's confidence rises for sub-commits 2 and 3 (which introduce new code).

---

## 9. Out of scope

- ADR-019 sub-phases B (migrate SwitchMutex mixed-context callers) and C (migrate Sub-step-5c-wrapped Channel.state sites). Separate future Jarvis dispatches.
- ADR-019 sub-phase A naming consolidation (whether to eventually merge SwitchMutex and IrqSafeMutex). Per Q1 ratification, deferred — addressed in a follow-up ADR if sub-phase C migration audit shows merge is feasible.
- ADR-020 hold-time stats. Per Q3 ratification, deferred.
- Plain `spin::Mutex` ban or fork. Per Q2 ratifications, plain `spin::Mutex` remains available with SAFETY-comment-style justification at declaration site. No code changes to plain `spin::Mutex` sites in this dispatch (sub-phase B/C handle the migration of category-(c) sites).
- CLAUDE.md / RAG indexer collection name. PL controls separately.
- Plans/ historical-record edits.
- docs/ edits (Prometheus handled all docs-side work).
- Boot performance regression analysis. If sub-commit 1's boot adds noticeable time (e.g., subtitle rendering pass), note in verification log; if egregious, surface to PM.

---

## 10. Cross-references

**Authoritative specs:**
- `Plans/KERNEL-BIBLE-ALIGNMENT-AUDIT-2026-05-10.md` — sub-commit 1 target list
- `docs/ADR-019-switchmutex-irq-off-by-construction.md` — sub-commit 3 spec (ratified)
- `docs/ADR-020-owner-metadata-production-diagnostic.md` — sub-commit 2 spec (ratified)

**Substrate referents:**
- `docs/bible.md` v1.5.0 — Articles 4.1 (monolithic kernel), 8.5 (critical-section discipline including amended 8.5.4), 15.2 (SAFETY comments), 15.3 (no unwrap/expect in steady-state), 25.1+25.2 (project name + neo- prefix convention)
- `docs/ADR-018-bible-8-5-4-preempt-while-holding-clarification.md` — declarative companion to ADR-019
- `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` — SC-3B closure record establishing Hpw mechanism
- `Plans/SC-3B-PHASE-D-FINDINGS-2026-05-10.md` — diagnostic-value proof for ADR-020
- `Plans/SC-3B-SUB-STEP-5C-VERIFICATION-2026-05-10.md` — the per-call-site fix that ADR-019/020 make structural

**Operational posture references:**
- `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.1 (Jarvis = sole kernel implementer), §3.1 (first-Jarvis calibration discipline)
- `Plans/PM-NOTE-2026-05-10-hybrid-git-posture.md` — push policy

**Forward state:**
- ADR-019 sub-phase B (migrate SwitchMutex mixed-context callers) — separate future Jarvis dispatch.
- ADR-019 sub-phase C (migrate Sub-step-5c-wrapped Channel.state sites; remove `without_interrupts(...)` wrappers) — separate future Jarvis dispatch.
- BUILD-3 sub-commit 4 (Q7.b/c — CR3 cache + IPI TLB shootdowns) — next Phase 2 kernel work after the ADR-019 sub-phases close.

---

**End of dispatch.**

*Three sub-commits, single-recipient Jarvis (Codex, first dispatch on NeoPlutarch), kernel-tier only. Sub-commit 1 (kernel rename + A3 boot identity per audit) → gate → sub-commit 2 (LockOwner foundation per ADR-020) → gate → sub-commit 3 (IrqSafeMutex per ADR-019 sub-phase A, with LockOwner from start, micro-bench with 50ns/cycle escalation) → gate → push. Three verification logs filed in Plans/. Partial-progress acceptable per §7.2 if a sub-commit fails; gate-tight discipline holds. Hand off to Jarvis when ready.*
