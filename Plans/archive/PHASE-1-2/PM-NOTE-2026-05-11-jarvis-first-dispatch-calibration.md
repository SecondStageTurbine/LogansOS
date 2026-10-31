# PM-NOTE — Jarvis first-dispatch calibration (kernel-rename bundle under Codex backing)

**Date filed:** 2026-05-11 (early morning, post-Jarvis kernel-rename bundle completion)
**Author:** PM
**Type:** PM-NOTE. Calibration record. Doc-only artifact. Per `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §3.1 — "PM treats the first 2-3 Jarvis kernel dispatches as calibration data, documents observed failure modes in handoffs as they appear, and reviews output with elevated intensity until a calibration profile firms up."
**Scope:** First calibration data point on Jarvis under the new agent-roster assignment (Codex backing for heavy kernel coding, replacing Janus's mixed coding+research role). Subject: kernel-rename bundle dispatch (`Plans/DISPATCH-2026-05-10-kernel-rename-and-adr-implementation-jarvis.md`), three sub-commits intended, two landed clean, one stopped at gate.
**Authority:** None. Calibration record for institutional memory; informs future dispatch shaping.

---

## 1. The dispatch and its outcome

**Dispatch scope:** Three sub-commits bundled.

1. **Sub-commit 1** — kernel rename AltimitNeo → NeoPlutarch; A3 boot identity (NEO PLUTARCH framebuffer wordmark; "Intelligence as a kernel primitive." tagline byline).
2. **Sub-commit 2** — ADR-020 foundation: add `LockOwner` field to `SwitchMutex`.
3. **Sub-commit 3** — ADR-019 sub-phase A: introduce `IrqSafeMutex` as sibling type to `SwitchMutex`; functional + bench gates required.

**Outcome:**

| Sub-commit | Status | Commit | Gates |
|---|---|---|---|
| 1 | LANDED | `053a5bc` (rebased from `33152a9`) | `cargo k` PASS, `cargo k --no-default-features` PASS, `cargo neo-test all` PASS, targeted scenarios PASS |
| 2 | LANDED | `de8965c` (rebased from `481217a`) | Same gates PASS |
| 3 | STOPPED AT GATE | not committed | `cargo k` PASS, `cargo k --no-default-features` PASS, `irqsafe_mutex_if_restore` PASS, **`irqsafe_mutex_bench` FAIL** (overhead 3085 ns > threshold 50 ns) |

Sub-commits 1 and 2 are on origin/main (rebased clean over `c35386c` + `d60f416` README updates that landed in parallel). Sub-commit 3 partial work preserved intact in working tree pending threshold revision per `Plans/PM-NOTE-2026-05-11-adr-019-bench-threshold-environment-clarification.md`.

## 2. Calibration observations

### 2.1 Stop discipline — exemplary

This is the single most important calibration signal. The dispatch (`Plans/DISPATCH-2026-05-10-kernel-rename-and-adr-implementation-jarvis.md` §7.1) specified:

> Micro-bench overhead >50ns/cycle threshold after sub-commit 3 -> STOP, do not push. File verification log with results, surface to PM.

Jarvis honored this exactly. The bench failed; Jarvis did not commit sub-commit 3, did not push, did not attempt to argue the threshold was unreasonable, did not silently lower the threshold to make it pass. The working tree was left in a clean partial state with the failure documented.

This matters because Jarvis-under-Codex is a new agent in the project, and the first dispatch establishes whether stop-discipline behavior is reliable. Stop-discipline failures (committing despite a gate fail; pushing past a documented stop condition; silently amending a threshold to make it pass) would have been **calibration-killing failures** that would have required PM to add explicit "do not amend thresholds" guardrails to every future kernel dispatch. None of those failure modes happened. Jarvis-under-Codex passes the stop-discipline calibration on first dispatch.

### 2.2 Optimization persistence — strong

The dispatch did not require Jarvis to optimize before stopping. The stop condition was "bench fails → STOP". Jarvis could have legitimately stopped after the first failed bench iteration (4377 ns overhead) and surfaced to PM immediately. Instead, Jarvis ran three iterations of optimization before stopping:

| Iteration | Change | Overhead | Reduction |
|---|---|---|---|
| 1 | Initial implementation | 4377 ns | — |
| 2 | Constant `lock_site!()` (eliminated runtime `Location::caller()` hashing per acquire) | 3139 ns | 28% |
| 3 | + forced inlining on lock/owner hot path | 3085 ns | 1.7% additional |

By iteration 3, additional optimization yielded marginal gains (the 1.7% indicates Jarvis hit the implementation floor — further optimization would not have helped). At that point Jarvis correctly assessed the remaining cost as environmental and stopped. The triage note in the failure report explicitly states:

> The remaining cost appears dominated by the IRQ save/restore path under the standard QEMU debug-build test configuration. PM should decide whether the 50ns threshold applies to debug QEMU numbers, whether the bench should run under an optimized kernel build, or whether sub-phase A needs a different API/measurement strategy before landing.

That triage framing was directly usable for PM resolution (`Plans/PM-NOTE-2026-05-11-adr-019-bench-threshold-environment-clarification.md` chose option (a) per Jarvis's framing). Jarvis surfaced the decision space cleanly without prescribing an answer — the right behavior for a kernel-implementation agent surfacing an environmental discovery to PM.

### 2.3 Failure documentation — comprehensive

`Plans/JARVIS-FAILURE-2026-05-10-kernel-bundle-sub-commit-3.md` contains:

- All three bench iteration results verbatim (raw `NEO_TEST/1 event=BENCH ...` output)
- xtask exit status (`exit status: 35`) and serial log path
- Optimization attempts described in implementation-specific terms (no hand-waving)
- Both partial commits' SHAs clearly noted
- Triage notes with three concrete decision options for PM
- Explicit "Status: STOPPED at sub-commit 3. No push performed." closure

This is the failure-artifact quality the project has come to expect from Janus-grade close reports. Jarvis-under-Codex matched that quality on first dispatch.

### 2.4 Branching strategy — defensive

The dispatch did not explicitly specify a branching strategy. Jarvis worked on a new branch `jarvis/kernel-rename-adr-2026-05-10` rather than directly on `main`. This kept the misfire (sub-commit 3 partial) isolated from `main`, and made the PM-side rebase + push trivially safe. The branch was correctly named (agent prefix + dispatch identifier + date), consistent with the project's existing branch naming conventions (e.g., `session/adr-ratification-2026-05-10`).

Forward calibration marker: explicit branching strategy was not in the dispatch but Jarvis applied the right default. Future kernel dispatches can continue to leave branching unspecified and rely on this default; if PM wants a specific branch strategy (e.g., direct-on-main for trivial single-commit work), it should be specified explicitly.

### 2.5 What PM did not anticipate

**Environmental cost of QEMU TCG IF-toggle.** The 50ns threshold in the ADR-019 dispatch (which PM authored) was set without empirical grounding for the QEMU debug-build measurement environment. PM assumed (incorrectly) that 50ns was achievable under the test infrastructure. The reality (1711 ns for IF-toggle alone under QEMU TCG, derived from the IrqSafeMutex - SwitchMutex delta) is ~250-500x the native theoretical and ~30x the dispatch threshold.

**This was a PM dispatch-shaping error**, not a Jarvis implementation error. The dispatch should have either:
- (a) Specified the threshold as a native-hardware target with an explicit QEMU-debug regression-ceiling fallback documented, or
- (b) Run a quick PM-preliminary bench measurement under QEMU debug to calibrate the threshold before specifying it, or
- (c) Framed the bench as a regression detector from the start (compare implementation to baseline; no absolute threshold).

Future dispatches that include perf gates should empirically calibrate the threshold against the test environment before specifying it. Adding to PM dispatch-shaping discipline: **perf-gate thresholds need environment-anchored measurement, not theoretical native-hardware targets, when the gate runs under QEMU TCG**.

The Jarvis side handled this perfectly — surfaced the discovery, did not force through, documented the data PM needed to resolve. The PM side will internalize the lesson for future dispatch shaping.

## 3. Comparison to prior Jarvis (Janus-era) behavior

Jarvis under Janus backing (early-mid 2026-05) was the project's mixed coding+research agent. Janus's failure modes included:
- Citation overconfidence in research artifacts (caught via `Plans/PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md`).
- Occasional drift in following dispatch-specified output structure on long synthesis artifacts.
- Strong on architectural reasoning; less precise on implementation detail-orientation.

Jarvis under Codex backing on this first dispatch shows a different profile:
- Implementation-detail precision is strong (three optimization iterations with concrete code-path identification).
- Stop-discipline is strong (honored gate exactly).
- Documentation rigor matches Janus's research-artifact discipline but applied to failure artifacts.
- Not enough data yet on architectural reasoning depth or how Jarvis handles dispatches that mix coding with substantial design decisions.

Forward calibration question (for dispatch 2 of 3): how does Jarvis-under-Codex handle a dispatch where the implementation requires making non-trivial architectural choices mid-flight? The kernel-rename bundle was implementation-heavy but architecturally-clean (the architectural work was front-loaded in the dispatch's ADR-019 + ADR-020 specifications). A future dispatch with more architectural latitude will calibrate this.

## 4. Forward markers

### 4.1 PM dispatch-shaping discipline updates

- **Perf-gate thresholds need environment-anchored measurement** before being specified in dispatches. Theoretical native-hardware targets are appropriate as architectural intent in ADRs/spec, but should not be encoded as bench gates without empirical calibration under the actual test environment.
- **Branching strategy can stay unspecified** for kernel work; Jarvis-under-Codex's default (agent-named branch off `main`) is the right behavior.
- **Stop conditions should be explicit** (the kernel-rename dispatch's §7.1 was clearly written; Jarvis honored it exactly). Future kernel dispatches should retain explicit stop-condition wording.

### 4.2 Jarvis calibration progression

Per agent-roster §3.1: 2-3 calibration dispatches before profile firms up. This is calibration dispatch 1 of 3.

- **Dispatch 2 candidate (forthcoming):** sub-commit 3 finalize (apply threshold revision + commit + push). Smaller scope; more contained. Will calibrate Jarvis's handling of a "land work that was previously stopped" dispatch pattern.
- **Dispatch 3 candidate (post-Cerebro D15 return):** SC-3C BUILD-stage sub-commits (Q7.b CR3 cache + Q7.c IPI TLB shootdowns), three sub-commits per the D-table D12 split. This is the next substantial kernel dispatch and will calibrate Jarvis on a larger, architecturally-richer kernel scope.

After dispatch 3, PM should produce a Jarvis voice profile artifact equivalent to the (not yet filed) Janus voice profile, capturing observed dispatch-handling patterns.

### 4.3 No agent-roster amendments suggested

This first calibration data point does not surface any need to amend `Plans/PM-NOTE-2026-05-10-agent-roster-update.md`. The Codex-backed Jarvis role assignment is performing as expected on its first dispatch — heavy kernel coding lane is appropriate; stop-discipline holds; failure-artifact quality is high. No re-allocation of work needed across agents.

## 5. Cross-references

**Subject of this calibration:**
- `Plans/DISPATCH-2026-05-10-kernel-rename-and-adr-implementation-jarvis.md` — the dispatch.
- `Plans/JARVIS-FAILURE-2026-05-10-kernel-bundle-sub-commit-3.md` — Jarvis's failure artifact for sub-commit 3.
- `Plans/KERNEL-RENAME-SUB-COMMIT-1-VERIFICATION-2026-05-10.md` — Jarvis's verification report for sub-commit 1 (committed alongside the code).
- `Plans/ADR-020-SUB-COMMIT-2-VERIFICATION-2026-05-10.md` — Jarvis's verification report for sub-commit 2 (committed alongside the code).
- `053a5bc kernel: rename AltimitNeo → NeoPlutarch; A3 boot identity` — landed on origin/main.
- `de8965c kernel/sync: add LockOwner field to SwitchMutex (ADR-020 foundation)` — landed on origin/main.

**Related PM-side artifacts (this same session):**
- `Plans/PM-NOTE-2026-05-11-adr-019-bench-threshold-environment-clarification.md` — the operational resolution of the bench threshold issue.
- `Plans/DISPATCH-2026-05-11-sub-commit-3-irqsafe-mutex-finalize-jarvis.md` — Jarvis-next dispatch to land sub-commit 3.

**Discipline anchors:**
- `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.1 (Jarvis kernel lane definition), §3.1 (calibration discipline).
- PL operational posture clarification (2026-05-10 memory entry).

---

## 6. Closure

Jarvis-under-Codex first-dispatch calibration: **strong positive data point.** Stop-discipline exemplary, optimization persistence strong, failure documentation comprehensive, branching strategy defensive-by-default. PM dispatch-shaping error caught (perf-gate threshold was set without environment-anchored measurement); discipline updated for future dispatches. Two of three sub-commits landed clean on origin/main via rebase; sub-commit 3 preserved in working tree pending threshold revision per the bench-threshold PM-NOTE.

Calibration progression: 1 of 3 dispatches complete. No agent-roster amendments needed; assignment performing as expected.

---

**End of PM-NOTE.**

*First Jarvis-under-Codex kernel dispatch. 2 of 3 sub-commits landed, 1 stopped at gate per dispatch §7.1. Exemplary stop discipline; PM dispatch-shaping error caught (theoretical native threshold under QEMU TCG environment); discipline updated. Calibration progression: 1 of 3.*
