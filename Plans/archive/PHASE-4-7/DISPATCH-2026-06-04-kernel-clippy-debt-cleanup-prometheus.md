<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Kernel clippy/warning debt cleanup → Prometheus

**Date:** 2026-06-04
**From:** PM/Cerebro
**To:** Prometheus (worker tree, feature branch — e.g. `prometheus/kernel-clippy-debt-cleanup`)
**Status:** DISPATCHED — parallel hygiene, runs alongside Jarvis's Phase 6 BUILD-1
**Requires:** Docker toolchain (kernel clippy). Pure cleanup — no behavior change.
**Current `main` tip:** `7ce56e8`.

---

## 1. Objective

The kernel has accumulated **~282 clippy warnings** (CI runs `cd kernel && cargo clippy -- -W warnings` **non-blocking** — `.github/workflows/ci.yml:281-285` — so the debt has grown unchecked since Phase 2 while host-checks only gate the non-kernel crates). Pay it down: **meaningfully reduce the kernel clippy warning count** with safe, mechanical fixes and justified `#[allow]`s, **with zero behavior change**.

This is a parallel hygiene task while Jarvis builds Phase 6 BUILD-1. **The file-exclusion list in §3 is the contract that keeps the two from colliding — honor it strictly.**

## 2. Discipline (this is cleanup, not a refactor)

- **No behavior change. None.** Every fix preserves semantics. The kernel must still build for its bare-metal target, and the full `cargo neo-test` suite must still pass.
- **Fix the safe/mechanical class:** needless borrows, redundant clones, `map(..).flatten()` → `flat_map`, `unwrap_or_else(|| x)` → `unwrap_or(x)` where eager is fine, needless `return`, redundant casts, `len() == 0` → `is_empty()`, collapsible `if`/`match`, unnecessary lifetimes, etc.
- **Some warnings are load-bearing — do NOT blindly auto-fix.** A warning that's wrong for a real reason (a `dead_code` item used only under a `neo-test` feature; an intentional pattern at a hardware boundary; a deliberately-eager evaluation) gets a **scoped `#[allow(clippy::lint_name)]` with a one-line comment explaining why**, not a behavior-changing "fix." Never silence a warning by changing what the code does.
- **Do not touch `unsafe` semantics, MMIO/asm, ordering on atomics, or the soft-float/no-XMM patterns** to satisfy clippy. If clippy suggests something at a hardware/ordering boundary, `#[allow]` + comment; flag anything that looks like a *real* latent bug to PM separately (don't fix it silently in a cleanup pass).
- **`cargo clippy --fix` is allowed as a starting point** but every changed hunk must be eyeballed — autofix is not trusted blind in kernel code.

## 3. Scope — files you MUST avoid (BUILD-1 owns these)

To run conflict-free alongside Phase 6 BUILD-1, **do not modify** any of:
- `kernel/src/ai/**` (the whole AI subtree)
- `kernel/src/drivers/accel/**` and `kernel/src/drivers/mod.rs`
- `kernel/src/arch/x86_64/syscall.rs`
- `kernel/src/test_harness.rs`

**Everything else in `kernel/src/` is fair game** — `mm/`, `sched/`, `fs/`, `net/`, `sync/`, `arch/x86_64/` (except `syscall.rs`), `drivers/{block,net,console,fb,pci}/`, etc. That's where the bulk of the 282 warnings live anyway. (`tools/` and `userland/` are already clippy-clean under `-D warnings`; leave them. **Do not touch `Plans/` or `docs/`** — that's a separate task.)

## 4. Method

1. Baseline: `docker compose run --rm neo bash -c "cd kernel && cargo clippy -- -W warnings 2>&1 | tee /tmp/clippy-before.txt"`; count warnings.
2. Work subsystem by subsystem (mm, then sched, then fs, …), skipping the §3 exclusions. Small, reviewable commits per subsystem are fine (squash at the end).
3. Re-measure after each subsystem; keep the kernel building.
4. Final: full `cargo neo-test` suite at `NEO_SMP=4` green (no behavior change), and report the before/after count.

## 5. Close gate

- `git fetch origin && git rebase origin/main` before close.
- **Warning count meaningfully reduced** — report **before → after** counts and a short breakdown (fixed vs. justified-`#[allow]`). Zero is not required (some are load-bearing); the bulk of the mechanical class should be gone.
- **Kernel builds** for its target; **full `cargo neo-test` suite green at `NEO_SMP=4`** (proves no behavior change).
- Host-checks green (`cargo fmt --all --check`, `cargo test --workspace --exclude neoplutarch-kernel`, `cargo clippy --workspace --exclude neoplutarch-kernel -- -D warnings`).
- **None of the §3-excluded files touched** (so BUILD-1 rebases clean).
- Single squashed commit; push to your **feature branch, not `main`** — PM merges via `pm-merge-check` + `--ff-only`. Close report with the before/after count, the per-subsystem breakdown, and any warnings you `#[allow]`-ed with the reason.

## 6. Scope guard

Kernel clippy hygiene **only**, within §3's boundary. No feature work, no behavior change, no `Plans/`/`docs/` edits, no touching the BUILD-1 files. If a clippy finding looks like a **real bug** (not a style nit), **stop and report it to PM** — don't fix it inside a cleanup pass where it'd hide in the noise.

*(Follow-up candidate, not this dispatch: archiving the ~68 closed Phase-4/Phase-5 docs under `Plans/archive/` — noted so it isn't lost.)*
