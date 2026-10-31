<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: CI Quality Gates — Host Tests + Lint Enforcement → Prometheus

**Date:** 2026-06-01
**From:** PM/Cerebro
**To:** Prometheus (worker tree, feature branch)
**Status:** DISPATCHED — ready for BUILD
**Runs in parallel with:** BUILD-3 (Jarvis, CPU inference) — **no overlap**: this touches CI config + workspace membership + (optionally) lint debt, not `kernel/src/ai/`.
**Motivation:** PM audit 2026-06-01 found our quality gates are weaker than the Bible claims. This closes the gap.

---

## 1. The problem (verified by PM audit)

Three findings, in priority order:

1. **Host unit tests run in no automated gate.** `tools/model-logic-tests` and
   `tools/tensor-logic-tests` are **not** in the workspace `members` list (and not
   in `[workspace] exclude`) — they are orphans. `cargo test -p model-logic-tests`
   *errors* ("did not match any packages"); `cargo test --workspace` skips them.
   They only pass because workers ran `cargo test --manifest-path …` by hand. CI
   (`.github/workflows/ci.yml`) runs **only** `cargo neo-test` (QEMU scenarios) —
   no `cargo test`, no `fmt`, no `clippy`. So these tests guard nothing.
2. **Clippy and rustfmt run in no gate at all.** Bible Article 15.5 says "clippy
   with warnings-as-errors … both run in CI." That is **not true today** — CI has
   no clippy or fmt step. (The kernel currently carries ~282 clippy warnings, so a
   hard `-D warnings` gate on the kernel would red-flag every PR immediately —
   handle that with staging, see §3.)
3. *(context, not your task)* `tools/neomc/src/importer_stub.rs` returning
   "not implemented" is **by design** (ADR-027 §D6) — leave it.

## 2. Objective

Make the gates match the doctrine: host tests, fmt, and clippy actually run in CI,
and the orphan test crates become first-class workspace members. Without
destabilizing existing green CI.

## 3. Scope & decisions (locked)

- **Workspace membership.** Add `tools/model-logic-tests` and
  `tools/tensor-logic-tests` to `[workspace] members` in the root `Cargo.toml`.
  Regenerate and **commit `Cargo.lock`** (must stay `--locked`-clean — the
  BUILD-2 lockfile breakage came from skipping this). Confirm `cargo test -p
  model-logic-tests` and `-p tensor-logic-tests` now resolve and pass.
  - *Gotcha:* these crates `#[path=…]`-import kernel source for host testing. They
    must remain **host-buildable** (no `no_std`/`mm`/kernel-only deps leaking in).
    Verify `cargo test --workspace` compiles on the host target. If membership
    pulls the kernel into a host `cargo test --workspace` in a way that breaks
    (kernel is `no_std`/custom-target), keep the kernel building under its own
    target as today and ensure the **host crates** are what `--workspace` test
    picks up — document whatever invocation actually works.
- **CI: a new `host-checks` job** in `.github/workflows/ci.yml` (sibling to
  `neo-test` and `spdx-lint`), running in the same `docker compose run --rm neo …`
  pattern so it uses the pinned `nightly-2026-04-16` toolchain:
  - `cargo test --workspace` (or the manifest-path set that covers all logic-test
    crates, if `--workspace` can't host-compile the kernel) — **blocking**.
  - `cargo fmt --all --check` — **blocking** (the tree is already fmt-clean per the
    BUILD close reports, so this should pass green from day one; if it doesn't, fix
    the formatting in this build).
  - `cargo clippy` — **two-tier**:
    - **host tool crates** (`neomc`, the logic-tests) with `-D warnings` —
      **blocking** (neomc is already clippy-clean; keep it that way).
    - **kernel** clippy — **non-blocking** (`continue-on-error: true`) for now,
      because of the ~282-warning backlog. It must *run and report* (visibility),
      but not fail the build yet. Add a comment in the workflow that flipping it to
      blocking is gated on burning down the backlog (a future cleanup build).
- **CI trigger gotcha:** `ci.yml`'s `paths-ignore` currently lists
  `.github/workflows/**`, so **edits to the workflow don't trigger CI**. Your new
  job won't run on the PR that adds it. Either (a) remove `.github/workflows/**`
  from `paths-ignore` so workflow changes self-test (preferred — recommend this),
  or (b) trigger a manual `workflow_dispatch` run to prove it green. Document which.
- **Do NOT** attempt to fix the 282 kernel clippy warnings in this build — that is
  a separate cleanup, explicitly out of scope here. This build makes the gate
  *exist and report*; the burndown is later.

## 4. Acceptance gate (ISC)

- **ISC-1 — Membership.** `model-logic-tests` + `tensor-logic-tests` are workspace
  members; `cargo test -p <each>` resolves and passes; `Cargo.lock` committed and
  `cargo metadata --locked` clean.
- **ISC-2 — host-checks CI job.** New job added; runs `cargo test` (covering all
  logic-test crates + neomc), `cargo fmt --all --check`, and clippy per the
  two-tier rule. Host-tool clippy + fmt + tests are blocking; kernel clippy
  runs non-blocking.
- **ISC-3 — Proven green.** The new job passes on this branch (via removing the
  workflow paths-ignore, or a `workflow_dispatch` run) — attach the run result /
  log excerpt in the close report. If fmt or a host test is *actually* red,
  fix it here so the gate lands green.
- **ISC-4 — No regression.** `cargo neo-test` (existing QEMU job) still passes;
  full `neo-test all --no-meta` at `NEO_SMP=4` green. (The membership change must
  not perturb the kernel build.)
- **ISC-5 — Standards + lean.** SPDX headers on any new files; workflow YAML is
  minimal and commented; `/simplify` before BUILD. No kernel clippy burndown
  (out of scope). No touching `kernel/src/ai/` (Jarvis's BUILD-3 territory).

## 5. Follow-on recorded (not this build)

The kernel clippy burndown (~282 warnings → 0, then flip the gate to blocking) is
a **separate future cleanup build**. Note it in your close report so PM can track
it; do not start it here.

## 6. Workflow

Branch off current `main` (e.g. `prometheus/ci-host-test-lint-gates`). `/simplify`
before BUILD. Single squashed commit; close report (`Plans/CLOSE-REPORT-…`) with
ISC pass/fail, the proven-green CI run evidence, and the kernel-clippy-burndown
follow-on flag. Cerebro reviews + merges.
