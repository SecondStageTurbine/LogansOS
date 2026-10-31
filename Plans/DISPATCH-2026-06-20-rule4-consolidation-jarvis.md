<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Rule-4 consolidation — no-feature paydown → Jarvis

**Date:** 2026-06-20
**From:** PM (Claude)
**To:** Jarvis (main-tier; just freed off OT-23). New branch off `origin/main` HEAD: `codex/rule4-consolidation-jarvis`.
**Status:** DISPATCHED. **The overdue Rule-4 consolidation — ~10 feature/fix builds since the last one (ADR-059, 2026-06-18): CI-hygiene, OT-24, q8_K, E1a, E2, 10.3, I-15, E3-audit, q4_K-dequant, OT-23, M1.** That's ~3× the ~3–4-build cadence. **No feature work** — collapse duplication, delete dead scaffolding, reconcile docs/ADR pointers, re-measure the invariant ledger, pass the fresh-reader legibility test. Behavior unchanged (the gate proves it).

## 0. The decision this build makes
Pay down the maintainability debt accrued across the last ~10 builds so the ledger stays one page and a fresh reader can still navigate. **Behavior-preserving by construction** — if any change alters behavior, it's out of scope.

## 1. References
```
git fetch origin
git show origin/main:docs/complexity-governance.md     # the invariant ledger I-1..I-15 + the metrics to re-measure; ADR-059 is the prior consolidation (the template)
git show origin/main:docs/bible.md                      # Article-13 ledger — reconcile the recent entries' cross-refs
git show origin/main:docs/OPEN-THREADS.md              # the OT register — close resolved, prune the dispositioned section, verify open threads accurate
git show origin/main:INDEX.md                           # reconcile artifact pointers
git show origin/main:userland/libneo/src/grant_store.rs # the OT-18 E1a/E2/E3 slices (~1450 lines) — the prime duplication-review target
```

## 2. ISCs (consolidation — no feature work)
- **ISC-1 — Collapse duplication / delete dead scaffolding.** Review the recently-grown surfaces — esp. `grant_store.rs` (E1a+E2+E3 layered, ~1450 lines), the `ai/ops` AVX2 trio (matmul/q8_K/q4_K — already inspected clean under BUILD-K at consolidation §233, re-confirm), `fpu.rs` (OT-23). Collapse genuine duplication; **remove avoidable `#[allow(...)]` and dead code added since `ADR-059`.** **Gate discriminators STAY** (paid-down debt, armed permanently — never removed as "test-only weight").
- **ISC-2 — Reconcile the ADR/doc pointers.** ADRs 057 (CSPRNG), 058 (getrandom), 060 (grant store), 061 (file-cap) + the M2 design THINK + the W2 discriminator THINK — statuses correct, cross-refs consistent, no "Proposed" that's actually shipped. Reconcile `INDEX.md` + README/CLAUDE.md pointers.
- **ISC-3 — Re-measure the invariant ledger (`complexity-governance.md`).** Re-audit I-1…**I-15** (I-15 = file caps, new this cycle); re-count: production lock statics, `new_ordered` call sites, max lockdep level, AI pure-tier LOC share. Confirm **no new cross-cutting invariant crept in unnamed**; if the ledger now exceeds one page, flag it (the alarm condition). Record the fresh measurements.
- **ISC-4 — Prune the OT register.** Move verified-resolved threads to the dispositioned section (and prune that section's stale rows per its own note); confirm the open threads are accurately stated (OT-2 W2-swap still open, OT-7 hardware-gated, OT-16/20/21/22, the E1b/E3-revoke/M3/M4 seams). Close OT rows whose work landed (e.g. confirm OT-23/OT-24/OT-25 closures are reflected).
- **ISC-5 — The fresh-reader legibility test.** Read the repo cold via INDEX/README/CLAUDE.md → can a new reader find the current state? Fix the broken/stale pointers.
- **ISC-6 — Standards.** **No feature work, no behavior change, no new invariant/`unsafe`.** Kill-switch is N/A (the "equality" here is *behavior unchanged* — the gate proves it: same tests green, no scenario altered). `fmt`/`clippy --workspace --all-targets -D warnings` via Docker. SPDX intact. The consolidation itself gets an ADR (the ADR-059 successor) recording what was paid down + the fresh ledger metrics.

## 3. Gate
- **Host + the full suite green, unchanged.** No test deleted or weakened; the existing scenarios pass identically (behavior-preserving).
- **If the cleanup touches kernel code** (removing kernel-side dead allows/scaffolding) ⇒ the pre-merge `workflow_dispatch` QEMU gate (all 4 shards) — **serialize behind the in-flight kernel gates** (E1b, M2). If it's docs/userland-only, the push CI suffices (state which).
- `cargo clippy --workspace --all-targets -D warnings` green via Docker.

## 4. Scope guard
Consolidation **only**. **Do NOT** add features, change behavior, alter any scenario's semantics, remove gate discriminators, or refactor working cohesive code for its own sake (the BUILD-K rule: scalar/AVX2 kernels are *genuinely different* cohesive code — leave them). If a "cleanup" changes a single test's behavior, it's out of scope. No new invariant/`unsafe`.

## 5. Merge (PM)
**Close-gate: run `scripts/worker-close-gate.sh`** (rebase-freshness — rebase LAST; the exact CI host-checks; SPDX; self-stat — **inspect the diff for any behavior change; there should be none**). Two hard rules: green-CI-URL, and the PM re-inspects your `git show --stat` (a consolidation's diff is mostly deletions + doc edits — a *feature* sneaking in is the thing to catch). PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (the ADR-059-successor consolidation note + the fresh ledger metrics).

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main` and confirmed. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Jarvis
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-20-rule4-consolidation-jarvis.md` (+ `complexity-governance.md` and ADR-059 as the template). **Branch:** `codex/rule4-consolidation-jarvis` off `origin/main`.
- **One-line:** Run the **overdue Rule-4 consolidation** (~10 builds since ADR-059) — **no feature work, behavior unchanged.** Collapse duplication + delete dead scaffolding / avoidable `#[allow]`s added since ADR-059 (esp. `grant_store.rs`'s E1a/E2/E3 layering; **keep gate discriminators + genuinely-cohesive scalar/AVX2 kernels**); reconcile ADR 057/058/060/061 + the M2/W2 THINKs + INDEX/README/CLAUDE pointers; **re-measure the invariant ledger** (I-1…**I-15**, lock counts, lockdep level, AI-tier LOC share — confirm no unnamed invariant crept in, flag if it exceeds one page); prune the OT register (resolved→dispositioned, confirm OT-23/24/25 closures); pass the fresh-reader legibility test. **No behavior change, no new invariant/`unsafe`** — the gate proves equality (same tests green, no scenario altered). Write the ADR-059-successor consolidation ADR with the fresh metrics. Gate: full suite green unchanged + clippy; **if it touches kernel code, the 4-shard QEMU gate — serialize behind E1b/M2, rebase LAST.** **Close-gate: `scripts/worker-close-gate.sh`; RETURN pastes the green CI URL(s) + the fresh ledger metrics.**
