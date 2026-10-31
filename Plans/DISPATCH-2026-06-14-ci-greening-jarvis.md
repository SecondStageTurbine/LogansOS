<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: CI-greening — get the family green, re-establish CI as a live gate → Jarvis

**Date:** 2026-06-14
**From:** PM/Cerebro
**To:** Jarvis (new branch off `origin/main` `4b6a20c`: `codex/ci-greening-jarvis`) — **your env has the QEMU/Limine harness (it ran BUILD-LO's Tier-B), which this lane needs.**
**Status:** DISPATCHED. **The task is to get the full CI family GREEN on `main` and keep it that way** — a terminal-state "kick it until it passes" lane, not a one-shot. Re-diagnose and re-fix on each failure until the run is green.

## 0. Why this lane (the root cause)
CI on `main` has been **masked-red**: BUILD-B, W0, W0b, V, K all touched kernel/tool code **and merged with `[skip ci]`**, so fmt/clippy/host-tests/QEMU were never run on any of them and debt accumulated invisibly. BUILD-LO was the first non-`[skip ci]` run since — it surfaced everything at once. The fmt half is **already fixed** (`4b6a20c` on `main` — `cargo fmt` on `cdc_ecm.rs`, pre-existing BUILD-B debt). What remains is a **QEMU shard 3/4 scenario failure** — plus anything else the host job never reached.

## 1. The known state (from the BUILD-LO CI run, 27485339371)
- **Host job "Host Tests, Format & Lint":** failed at **step 6 `cargo fmt --check`** → **FIXED in `4b6a20c`**. **But steps 7–10 were SKIPPED** (they run only after fmt passes): `cargo test --workspace`, the neo-test shard-coverage check, `clippy --workspace`, and `clippy kernel`. **Their state is UNKNOWN — the same `[skip ci]` masking may hide clippy/test debt too. Verify all of them.**
- **QEMU shards 1, 2, 4:** PASS. **QEMU shard 3/4:** FAILURE (a scenario failed ~34 min in; the FAIL is mid-log, not in the tail). SPDX job: PASS.

## 2. Tasks
1. **Reproduce shard 3/4 locally** (`cargo neo-test` for the shard-3 partition, **NEO_SMP=4** as CI runs it) and **name the failing scenario(s)** — there may be more than one. Use the discriminator discipline: capture the serial log, localize the failure (panic? assertion? timeout? a `NEO_TEST event=FAIL name=…`?).
2. **Pin ownership — pre-existing vs BUILD-LO.** Run the failing scenario(s) against **`0c7f175`** (main *before* BUILD-LO) AND current main (`4b6a20c`). Report which:
   - **Fails on both** ⇒ pre-existing (the likely case — BUILD-LO's new scenario is feature-gated default-off, not in the shard run; its only default-build change is the behavior-preserving `tensor_handle_lookup` extraction, host-validated by `tensor_create_query_drop`).
   - **Fails only on current main** ⇒ a BUILD-LO regression (then the extraction is the suspect — fix or revert that part).
3. **Verify the steps the host job never reached:** `cargo fmt --all --check` (should now pass), `cargo clippy --workspace -D warnings`, `cargo clippy` on the kernel, `cargo test --workspace`, and the neo-test shard-coverage check. **Fix any clippy/test debt you find** (it's the same masked-debt class).
4. **Fix the QEMU failure.** If pre-existing: diagnose the real cause (a genuine bug masked since it landed, a flaky/timing scenario, or a CI-environment issue) and fix it — a real fix with a discriminator, not a skip. If a scenario is genuinely environment-only-flaky, make it deterministic (warm-up/retry-bounded) rather than disabling it; if you must quarantine one, say so explicitly with the reason. **Do not delete a gate scenario to go green.**
5. **Confirm the WHOLE family green locally** before push: all 4 QEMU shards (NEO_SMP=4) + host fmt/clippy/test + shard-coverage + SPDX. That is the close-gate.
6. **(Cosmetic hygiene — NOT the fix for the red.)** Silence the misleading `git config: Permission denied` cleanup warning. Root cause (PL-diagnosed, confirmed): the QEMU job runs the container as UID 1000 but the runner is UID 1001, so `actions/checkout`'s post-step can't rewrite `.git/config` during cleanup. It is a **`##[warning]` in post-job cleanup that does NOT fail the job** — verified: it appears identically in the *passing* shard 1/4 (`error: could not lock config file .git/config: Permission denied` at 02:18:05) and in the failing shard 3/4, and the "Post Checkout" step reports success in both. So it is a red herring for the red — but it nearly mis-led the diagnosis, so kill the noise: add to each QEMU-shard job in `.github/workflows/ci.yml`, after the final disk-usage probe, a `Restore workspace ownership for cleanup` step with `if: always()` running `sudo chown -R 1001:1001 "${{ github.workspace }}"`. This touches `.github/` (lets CI run — not a `[skip ci]`). **Do this alongside the real shard-3 fix, not instead of it.**

## 3. Gate (the terminal state)
**A fully green CI run on `main`.** Local-green is the close-gate for the push; the **real** terminal state is the CI run after I merge going green. This is a kick-until-it-passes lane: if the merged run is still red, re-diagnose and re-fix — one round is not the task. When CI is green, that *is* the deliverable.

## 4. Scope guard
Get the family green + re-establish the gate. **Do NOT** add features; **do NOT** disable/delete gate scenarios to fake green (fix the cause, or quarantine-with-explicit-reason as a last resort); **do NOT** touch the AI hot-path or Janus's 8.1 surface (`generate.rs`/`generate_logic.rs`) — stay in the CI/test-harness + whatever specific file owns the failing scenario. The `cdc_ecm.rs` fmt fix is already on `main`; don't re-touch it. If the QEMU failure is a deep pre-existing bug that needs its own build, **BLOCKER-to-PM with the diagnosis** rather than forcing it.

## 5. Policy this lane establishes (record in the RETURN)
**No `[skip ci]` on code changes.** Pure-doc/`Plans/` pushes already bypass CI (the workflow paths-ignores `docs/**`, `Plans/**`, `**.md`), so `[skip ci]` is reserved for those. Anything touching `kernel/`, `tools/`, `userland/`, or `Cargo.*` **lets CI run** — that is what keeps debt visible. (PM will codify this in CLAUDE.md.)

## 6. Merge (PM)
Close-gate: rebase onto current `main`, re-run the full family locally green, push. PM `pm-merge-check` → `--ff-only` → `main`, then **watch the `main` CI run to green** (I'll subscribe to it). On green: CI is a live gate again, and the masked-debt era is closed.

---

## Relay — what to tell Jarvis
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-14-ci-greening-jarvis.md`
- **Read it from `main`** (your tree is on a feature branch — a local file-open won't find it):
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-14-ci-greening-jarvis.md
  ```
- **Branch:** `codex/ci-greening-jarvis` off `origin/main` `4b6a20c`.
- **The failing CI run to study:** GitHub Actions run **27485339371** (BUILD-LO's run) — host job failed at `cargo fmt` (FIXED in `4b6a20c`); **QEMU shard 3/4** failed (a scenario ~34 min in); shards 1/2/4 passed.
- **One-line:** Get the **whole CI family green** and keep it green (kick-until-it-passes). fmt is already fixed on `main`. **Reproduce QEMU shard 3/4 locally** (NEO_SMP=4), name the failing scenario(s), and **pin pre-existing-vs-BUILD-LO by running it against `0c7f175` (pre-BUILD-LO) and current main** — almost certainly pre-existing (BUILD-LO's new scenario is default-off; its only default change is a behavior-preserving extraction). **Also verify the steps the host job never reached** (clippy `-D warnings` workspace + kernel, `cargo test --workspace`, shard-coverage) — the same `[skip ci]` masking may hide clippy/test debt. Fix the real causes (no deleting gates to fake green; quarantine only with an explicit reason). Confirm all 4 shards + host + SPDX green locally, push; I merge and watch `main` CI to green. Policy banked: **no `[skip ci]` on code changes**.
