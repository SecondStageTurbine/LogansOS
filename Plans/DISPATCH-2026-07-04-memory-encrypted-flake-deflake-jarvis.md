<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Kill the `arthur_memory_encrypted` attempt-1 flake — it is doubling QEMU load on every kernel gate → Jarvis

**Date:** 2026-07-04
**From:** PM (Claude)
**To:** Jarvis (main-tier coder; the CI-hermeticity lane is yours — this is the follow-on your `ci-hermeticity-deflake` build explicitly left as residual). **Branch:** `codex/memory-encrypted-deflake-jarvis` off `origin/main` HEAD.
**Status:** DISPATCHED — a **throughput** build, not a feature. The residual `arthur_memory_encrypted` contention flake cost **three attempt-2 QEMU runs today alone**: the OT-34 gate (`28706937808`), the M3 gate (`28707956034`), and the IPC gate (`28708106285`) each failed attempt-1 on it and passed attempt-2 unchanged. With ~3 self-hosted runners as the binding constraint and four main-tier coders opening kernel lanes, a scenario that reliably reds attempt-1 under contention **doubles the cost of nearly every kernel-touching merge**. Per the hermeticity standing rule, *"a green gate must mean the code is correct, never that the runner cooperated"* — the contrapositive holds too: **a red gate must mean the code is wrong, never that the runner was busy.** This scenario currently violates that.

## 1. Diagnose from the evidence that already exists (before changing anything)
Pull the **attempt-1 failure logs** of today's three runs (run IDs above — the attempt-1 logs are retained; the on-failure serial-log artifacts too, subject to OT-35's retention). Establish the actual failure signature. Known hypotheses to test against the logs — do NOT prejudge which:
- **H1 — entropy starvation under TCG.** `arthur_memory_encrypted` requires `kernel-rng`. This is the exact class the hermeticity build fixed for `phase4_https_fetch`/`kernel_rng_selftest` by pinning `-cpu max` (RDRAND under pure TCG advertises the CPUID bit but returns CF=0). **Check whether this scenario ever received the `-cpu max` treatment** — if not, this may be a one-line fix and the "contention" correlation is coincidental (a busy runner slows the guest enough to expose the entropy stall).
- **H2 — timeout margin under contention.** The scenario's wall-clock budget is calibrated to an idle runner; co-scheduled heavy shards (three concurrent gates today) starve it past the timeout. Fix = measured margin (from the logs: how close is a *passing* run to the limit?), not a blind bump.
- **H3 — shard co-location.** A specific heavy scenario sharing its shard spikes contention. Fix = re-shard.
- **H4 — a real (code) race that contention widens.** If the logs show a *kernel* misbehavior rather than a slow/starved pass, STOP and report to the PM — that is a bug bounce to the owning build, not a deflake.

## 2. ISCs
- **ISC-1 — Root cause named, with log evidence.** The RETURN states which hypothesis held, citing the attempt-1 log lines. "It passes on retry" is not a root cause.
- **ISC-2 — The fix, hermetic.** Whatever H holds: the scenario must pass attempt-1 on a **contended** runner. No `continue-on-error`, no retry-loop-in-CI masking (that hides real regressions — the failure-is-a-verdict discipline), no removing the scenario from the gate (it guards M2 encryption; it stays armed).
- **ISC-3 — The discriminator (capture-before-you-need-it).** If it ever flakes again, the failure must be self-diagnosing: add the breadcrumb that distinguishes entropy-stall vs timeout-starvation vs real-failure in the serial log (e.g. progress timestamps / an entropy-ready marker), so the next occurrence is a log-read, not a re-investigation.
- **ISC-4 — Prove it.** Attempt-1 green on your QEMU gate. If you can cheaply demonstrate robustness under contention (e.g. your gate running while another gate is live — likely free given current traffic), cite it; not required if the root-cause fix is structural (H1-class).
- **ISC-5 — OPTIONAL rider (only if your fix already touches `kernel/`): OT-36 boot-frame strings.** The boot frame still says `v0.0.1 — PHASE 1.5c` / `scheduler: cooperative ready` (stale since Phase 1.5; confirmed on the PL's 2026-06-22 bare-metal boot photo). If you are kernel-touching anyway, fold the string fix in (phase → 8/Arthur; verify the scheduler line against the live preemptive-SMP scheduler; version stays PL's call — leave `v0.0.1` unless the PL says otherwise) and mark OT-36 done in the ledger. If your fix turns out CI-config-only, SKIP this rider — don't become kernel-touching just for strings.

## 3. Scope guard
The scenario's *assertions* are untouchable (they guard M2). CI changes stay within the hermeticity discipline (no external-input dependence, no runner-variance dependence). If H4 (real bug), stop and bounce to the PM — do not patch around a genuine race.

## 4. Gate + close
- If `kernel/` is touched (harness/boot-frame) ⇒ **the 4-shard QEMU gate is mandatory**; if genuinely CI-yaml-only, push-CI suffices (state which applies). Runner note: two gates in flight as this is written — stagger yours after they drain.
- **Close-gate `scripts/worker-close-gate.sh`**; RETURN: root cause + log citations, the fix, attempt-1-green CI URLs, diff-stat, whether the OT-36 rider shipped.
- **Disk reclaim** after the PM confirms merge.

---
## Relay — what to tell Jarvis
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-04-memory-encrypted-flake-deflake-jarvis.md`. **Branch:** `codex/memory-encrypted-deflake-jarvis` off `origin/main` HEAD.
- **One-line:** **Root-cause and kill the `arthur_memory_encrypted` attempt-1 flake — it red-flagged attempt-1 on all three of today's kernel gates (runs 28706937808 / 28707956034 / 28708106285) and is doubling QEMU load on our ~3 contended runners.** Diagnose from the retained attempt-1 logs FIRST; hypotheses: H1 entropy starvation (`kernel-rng` scenario — check whether it ever got the `-cpu max` RDRAND treatment your hermeticity build gave `phase4_https_fetch`; possibly a one-line fix), H2 timeout margin under contention (measure headroom from passing runs, no blind bumps), H3 shard co-location, H4 a real race (⇒ STOP, report — that's a bug bounce, not a deflake). Fix must be hermetic: attempt-1 green on a contended runner; no retry-masking, no `continue-on-error`, scenario stays armed. Add the discriminator breadcrumb (entropy-stall vs starvation vs real-failure distinguishable from the serial log) so any recurrence is a log-read. **Optional rider only if already kernel-touching:** fold the OT-36 stale boot-frame strings (`PHASE 1.5c`/`cooperative`) fix. Kernel-touching ⇒ QEMU gate mandatory (two gates in flight — stagger). Close-gate; RETURN root cause + log citations + attempt-1-green URLs.
