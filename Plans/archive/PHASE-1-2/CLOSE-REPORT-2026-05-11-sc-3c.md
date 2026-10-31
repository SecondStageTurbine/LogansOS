# CLOSE REPORT — SC-3C (BUILD-3 sub-commit 4): Q7.b CR3 cache + Q7.c IPI TLB shootdowns

**Date:** 2026-05-11
**Owner:** PM
**Status:** **CLOSED.**
**Type:** Phase 2 BUILD-3 sub-commit 4 closure report (matches SC-3B precedent `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md`). Consumes verification reports from three Jarvis sub-commits + one Prometheus closure-integration; cites integrated PHASE-2.md / INDEX.md additions.

---

## 1. Closure summary

SC-3C closed on origin/main with three Jarvis sub-commits landing in sequence (4a → 4b → 4c) plus one Prometheus integration commit folding the closure documentation into PHASE-2.md and INDEX.md. Three doc-only Prometheus parallel commits (rename-mechanical execution, Bible amendment v1.6.0, SC-3C closure integration) and one PL operational commit (IrqSafeMutex bench CI handling) interleaved during the SC-3C wall-clock window; all landed clean via fast-forward.

**SC-3C is the final SMP-correctness sub-commit Phase 2 BUILD-3 needed for Q7.** The kernel now has:

- Per-CPU CR3 cache with compare-and-skip (Q7.b ratified per ADR-... wait, Q7.b doesn't have a dedicated ADR; the design landed via ADR-019/020 already-ratified machinery plus the SC-3C D-decisions in `Plans/SC-3C-THINK-2026-05-10.md` §6).
- Cross-CPU TLB invalidation via IPI vector 0xfc with INVLPG-or-full-flush hybrid granularity (N=33 Linux-measured threshold), broadcast targeting, 1ms bounded synchronous wait with panic-on-timeout, per-CPU single-slot shootdown payload via AtomicPtr (Q7.c).
- D14 missing-shootdown-detection harness verified live (FreeBSD-EN-18:07 silent-data-corruption failure mode is now detectable by the test framework).

Q8 (sub-commit 5 + sub-commit 6) is the next critical-path work: Q8.a/b capability table SMP composition + Q8.c blocked-thread-kill walk + process-exit sequencing.

## 2. Commits on origin/main

In chronological landing order (oldest first):

| SHA | Author | Type | Description |
|---|---|---|---|
| `e7782fc` | PL (SecondStageTurbine) | Operational CI fix | tools/xtask: keep IrqSafeMutex bench out of default CI. GitHub-runner host-load made the 6000ns ceiling fragile; bench reframed as explicit scenario (included in `neo-test all`, excluded from default `neo-test`). |
| `88c7bc2` | Jarvis | SC-3C 4a | kernel/sched: cache active CR3 per CPU; skip redundant CR3 writes (Q7.b). |
| `3272840` | Jarvis | SC-3C 4b | kernel/mm: introduce TLB shootdown machinery (Q7.c machinery; no callers). |
| `f44bf45` | Prometheus | Doc parallel | docs: complete AltimitNeo→NeoPlutarch rename stragglers per audit. |
| `ead8b19` | Jarvis | SC-3C 4c | kernel/mm: wire TLB shootdown into Pmap mutation sites (Q7.c call sites). |
| `6231cf0` | Prometheus | Bible amendment | bible: Article 17 source tree diagram root altimitneo/ → neoplutarch/ (v1.6.0). |
| `165a2e2` | Prometheus | Closure integration | docs/phases: integrate SC-3C closure section + Phase 2+ TLB forward markers. |

**Interleaving observation:** Jarvis SC-3C 4c (ead8b19) landed on top of Prometheus rename-mechanical (f44bf45) cleanly. The Jarvis verification report for 4c explicitly notes the rebase context: "finalized on top of `f44bf45` (`origin/main`), which already contains sub-commit 4b (`3272840`) plus the parallel rename-audit documentation commit." Hybrid-git posture worked as designed; agent lanes did not conflict.

## 3. Verification

### 3.1 Per-sub-commit gates (all PASS)

Three Jarvis verification reports filed:

- `Plans/SC-3C-SUB-COMMIT-4A-Q7B-CR3-CACHE-VERIFICATION-2026-05-11.md` (2.3 KB) — `cargo k` PASS; `cargo k --no-default-features` PASS; `q7b_cr3_cache_skip_count` PASS with same-process context-switch cache hit rate ≥ 50%; `cargo neo-test all` PASS.
- `Plans/SC-3C-SUB-COMMIT-4B-Q7C-TLB-SHOOTDOWN-MACHINERY-VERIFICATION-2026-05-11.md` (2.5 KB) — synthetic shootdown round-trip variants (None range, small INVLPG, large-falls-back-to-full) all PASS; timeout-panic scenario PASS.
- `Plans/SC-3C-SUB-COMMIT-4C-Q7C-CALL-SITES-VERIFICATION-2026-05-11.md` (4.0 KB) — full gate table including D14 scenarios.

### 3.2 D14 scenario results (SC-3C 4c, the load-bearing closure gate)

| Scenario | Result | Mechanism |
|---|---|---|
| `q7c_concurrent_munmap_smp` | PASS | `trap=page_fault mechanism=q7c_munmap_shootdown` — CPU A munmap + CPU B read produces clean fault, no use-after-free, no stale read |
| `q7c_process_exit_during_thread_run` | PASS | `exit_reaped=true teardown_shootdown=true q8c_deferred=true` — existing Q8.c-deferred exit path uninterrupted by new shootdown wiring |
| `q7c_missing_shootdown_detection` | PASS | `detected=stale_tlb_read_after_unmap negative_feature=neo-test-missing-shootdown-negative value=0xa7` — the FreeBSD-EN-18:07 failure mode is detectable by the test framework |

The third row is the most architecturally significant gate. The `neo-test-missing-shootdown-negative` feature deliberately suppresses the shootdown call after PTE invalidation; the harness then verifies that a CPU other than the one that performed the invalidation successfully reads through a stale TLB entry (value `0xa7`, the pre-unmap content). The test PASSES when the failure mode is detected — confirming the harness can catch the FreeBSD-EN-18:07-class silent-data-corruption pattern that motivated D14 ratification.

### 3.3 SMP stress (existing-substrate continuity)

`200/200 NEO_SMP=4 smp_cross_cpu_rendezvous_stress` PASS. Results under `target/verify/sc3c-4c-stress-neo-smp4-20260511-181228`. SC-3B closure gate equivalent preserved.

### 3.4 cargo fmt --check BLOCKED (tooling gap)

Jarvis transparently surfaced: `cargo-fmt.exe` is not installed for pinned toolchain `nightly-2026-04-16-x86_64-pc-windows-msvc`. This is a tooling-environment gap, not a code issue. Forward marker §6.1.

### 3.5 Closure integration verification

`Plans/SC-3C-CLOSURE-INTEGRATION-VERIFICATION-2026-05-11.md` (4.8 KB) — 12 placeholders resolved (8 in PHASE-2.md, 4 in INDEX.md); both PM format-precision fixes applied (Fourteen-D-decisions-plus-meta-D15 framing, Spectre-v2 BTB citation correction); 5/5 cross-references verified; SC-3B section untouched; no `kernel/src/*` touches.

## 4. Mechanism named

**None.** SC-3C is correctness-implementation work for ratified design (per SC-3C THINK §6 D1–D14 + ADR-018/019/020 already-ratified machinery). No new wedge mechanism beyond what ADR-018 / ADR-019 / ADR-020 already named. Q7.b compare-and-skip is a standard optimization; Q7.c shootdown machinery is canonical SMP correctness; no naming required.

## 5. Course corrections during BUILD

**One operational adjustment:** PL landed `e7782fc` between sub-commit 3 close (27d1b99) and SC-3C 4a start (88c7bc2) to keep the IrqSafeMutex bench out of the default CI suite. The IrqSafeMutex bench (the same one PM set the 6000ns regression ceiling on per `Plans/PM-NOTE-2026-05-11-adr-019-bench-threshold-environment-clarification.md`) was failing on GitHub-runner CI's host-load characteristics while passing on local Docker. PL's fix: include the bench in `neo-test all` (which Jarvis runs locally) but exclude it from default `neo-test` (which CI runs). The 6000ns ceiling remains correct for local Docker QEMU TCG; GitHub-runner is a third environment with different host-load that the 6000ns ceiling cannot reliably handle.

**Discipline implication captured in §6.2 forward marker:** PM perf-gate environment-anchoring should differentiate between local Docker QEMU TCG and GitHub-runner Docker QEMU TCG; same code, same kernel build, but different host-load → different effective regression-detector ceilings. Future perf-gates may need either (a) different thresholds per environment, or (b) explicit-scenario-only-include framing per PL's e7782fc pattern.

**No code course corrections in BUILD.** Jarvis's three sub-commits landed clean against the dispatch spec without surfacing any D-decision ambiguities requiring PM mid-flight resolution. The strictly-vs-loosely-accurate T2 cpumask design choice and the lazy-TLB freed_tables exception (both Phase 2+ forward markers required by the dispatch) are documented inline in `kernel/src/mm/tlb_shootdown.rs` per the dispatch §7 requirements.

## 6. Forward markers

### 6.1 rustfmt tooling gap

The pinned nightly `nightly-2026-04-16-x86_64-pc-windows-msvc` does not have rustfmt installed. `cargo fmt --check` is BLOCKED. Affects Jarvis kernel work and any future cargo-fmt-driven CI gates.

Not a code issue. Three options:
- Install rustfmt for the pinned nightly via `rustup component add rustfmt --toolchain nightly-2026-04-16-x86_64-pc-windows-msvc`.
- Pin a different nightly that ships rustfmt.
- Accept the gap and rely on rustfmt-equivalent linting elsewhere.

**No immediate action required.** Surface for PL operational decision when convenient.

### 6.2 Perf-gate environment-anchoring refinement

PM dispatch-shaping discipline updated by the e7782fc operational fix:

**Three QEMU TCG environments exist in this project:**
1. **Local Docker** (developer workstation) — Jarvis's bench environment. IrqSafeMutex overhead floor ~3085 ns; 6000 ns ceiling appropriate.
2. **GitHub-runner Docker** (CI default) — host-load worse than local Docker. 6000 ns ceiling unreliable; bench excluded from default suite.
3. **Bare-metal** (hypothetical future bare-metal CI per ADR-019 Q4) — does not exist yet; 50 ns native-hardware target applies.

PM perf-gate thresholds in future dispatches must specify which environment the threshold targets. Default ceiling for kernel work should be local-Docker-anchored; CI-default gates exclude environment-fragile benches; bare-metal characterization deferred to Phase 2+.

### 6.3 Inherited Phase 2+ forward markers

All forward markers landed in PHASE-2.md "Forward reference: Phase 2+ TLB management optimizations" subsection via commit `165a2e2`:

- PCID/ASID enablement (deferred per D2)
- T2 targeted shootdown with strictly-vs-loosely-accurate cpumask design choice (deferred per D4 + Cerebro D15 §3.1)
- Lazy TLB with freed_tables=true exception (deferred per D6 + Cerebro D15 §4)
- Shootdown batching / `mmu_gather` analogue (deferred per D7)
- AMD INVLPGB / Intel RAR hardware-broadcast (Phase 3+ per Cerebro D15 §3.4/§3.5; correct function name `broadcast_tlb_flush`, not `broadcast_flush_tlb_all`)
- Spectre-v2 BTB flush on context switch / IBPB analogue (Phase 2+ cluster; deserves own ADR; not SC-3C territory)

## 7. Calibration notes

### 7.1 Jarvis calibration 3 of 3 complete

This was Jarvis's third dispatch under Codex backing. Voice profile artifact `Plans/PM-NOTE-2026-05-11-jarvis-voice-profile.md` filed separately. Headline: Jarvis-under-Codex is calibrated; future kernel dispatches operate on standard PM dispatch shape without elevated calibration scrutiny.

### 7.2 Prometheus pattern observations

Three Prometheus parallel dispatches landed during SC-3C wall-clock window (rename-mechanical execution f44bf45; Bible amendment 6231cf0; closure integration 165a2e2). All three: clean fast-forward push, scope-adherent (no deferred items touched, no kernel touches, no scope creep), verification reports match dispatch spec, exemplary discipline on hybrid-git posture (no Plans/ artifacts committed without explicit dispatch authorization).

Prometheus lane is established and performing reliably; voice profile artifact not yet needed but may be filed in a future session if dispatch volume warrants.

### 7.3 Cerebro pattern observations

Two Cerebro dispatches completed earlier in the same wall-clock cycle (field survey and SC-3C D15 TLB-shootdown prior art). Both delivered on dispatch spec within budget. Future Cerebro dispatches can transition from explicit-structure specification to implicit-structure per agent-roster §3.2 — Cerebro voice profile firmed up sufficiently.

### 7.4 PM operational performance

Two PM dispatch-shaping errors caught and corrected during this cycle:
- Role-confusion 2026-05-10 (PM did Cerebro-shape research directly; corrected via rename + provenance markers).
- ADR-019 bench threshold theoretical-native-hardware target under QEMU TCG (corrected via 50→6000 ns environment-anchored revision).
- Now: e7782fc PL operational fix surfaces a third refinement (perf-gate environments are local-Docker / GitHub-runner / bare-metal triplet, not a single QEMU TCG environment). Internalized for future dispatch shaping.

No agent-roster amendments triggered. PM-NOTE filed: `Plans/PM-NOTE-2026-05-11-jarvis-voice-profile.md` (Jarvis calibration culmination).

## 8. What Q8 looks like

Per SC-3C PLAN dispatch §11.4 ("Sub-commit 5 (Q8.a/b — capability table SMP composition) becomes the next candidate; sub-commit 6 (Q8.c — blocked-thread-kill walk + process-exit sequencing) follows that"), the next critical-path PM operational deliverable is a Q8 THINK-stage artifact parallel to the SC-3C THINK pattern.

Q8 substrate signals to investigate during THINK:
- Capability table SMP composition (Q8.a/b): the existing capability infrastructure was designed under Q4/Q6 IPC retrofit (SC-3B) with single-CPU semantics; SMP composition needs the same kind of design space exploration as SC-3C did for TLB.
- Blocked-thread-kill walk (Q8.c): the `q7c_process_exit_during_thread_run` SC-3C scenario PASS already references `q8c_deferred=true` in its mechanism description. The existing exit path is the substrate.

Not gating anything immediately. PM authors Q8 THINK on next session bandwidth.

## 9. Cross-references

**Verification reports consumed:**
- `Plans/SC-3C-SUB-COMMIT-4A-Q7B-CR3-CACHE-VERIFICATION-2026-05-11.md`
- `Plans/SC-3C-SUB-COMMIT-4B-Q7C-TLB-SHOOTDOWN-MACHINERY-VERIFICATION-2026-05-11.md`
- `Plans/SC-3C-SUB-COMMIT-4C-Q7C-CALL-SITES-VERIFICATION-2026-05-11.md`
- `Plans/SC-3C-CLOSURE-INTEGRATION-VERIFICATION-2026-05-11.md`
- `Plans/RENAME-MECHANICAL-EXECUTION-VERIFICATION-2026-05-11.md`
- `Plans/BIBLE-AMENDMENT-V1.6.0-VERIFICATION-2026-05-11.md`

**Design + research artifacts (canonical):**
- `Plans/SC-3C-THINK-2026-05-10.md` — design (D1–D15).
- `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` (FINALIZED 2026-05-11).
- `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md` — canonical prior art.
- `Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md` — corrections + forward markers.

**Dispatches:**
- `Plans/DISPATCH-2026-05-11-sc-3c-plan-stage-jarvis.md` — Jarvis SC-3C BUILD dispatch.
- `Plans/DISPATCH-2026-05-11-sc-3c-closure-prep-and-rename-audit-prometheus.md` — Prometheus closure-prep + audit dispatch.
- `Plans/DISPATCH-2026-05-11-rename-mechanical-execution-prometheus.md` — Prometheus rename-mechanical dispatch.
- `Plans/DISPATCH-2026-05-11-bible-amendment-article-17-prometheus.md` — Prometheus Bible amendment dispatch.
- `Plans/DISPATCH-2026-05-11-sc-3c-closure-integration-prometheus.md` — Prometheus closure integration dispatch.

**Operational discipline anchors:**
- `Plans/PM-NOTE-2026-05-11-adr-019-bench-threshold-environment-clarification.md` — perf-gate environment-anchoring (now refined to triplet per §6.2 above).
- `Plans/PM-NOTE-2026-05-11-jarvis-first-dispatch-calibration.md`, `Plans/PM-NOTE-2026-05-11-jarvis-voice-profile.md` — Jarvis calibration.
- `Plans/PM-NOTE-2026-05-11-prometheus-closure-prep-and-audit-consumption.md` — Prometheus consumption record.

**Integrated documentation (on origin/main):**
- `docs/phases/PHASE-2.md` SC-3C section (commit `165a2e2`).
- `docs/phases/PHASE-2.md` "Forward reference: Phase 2+ TLB management optimizations" (commit `165a2e2`).
- `Plans/INDEX.md` 11 SC-3C-related entries (commit `165a2e2`).
- `docs/bible.md` v1.6.0 (commit `6231cf0`).

**Precedent:**
- `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` — SC-3B closure report (the precedent this report follows).

---

## 10. Closure statement

SC-3C closed. Phase 2 BUILD-3 sub-commit 4 complete. Kernel now has per-CPU CR3 cache + cross-CPU TLB invalidation with D14 missing-shootdown-detection verification. All Jarvis sub-commits clean; all Prometheus parallel work clean; one PL operational fix (e7782fc) for CI environment-anchoring landed cleanly. Three forward markers captured (rustfmt tooling gap, perf-gate triple-environment discipline, six pre-existing Phase 2+ TLB markers). Jarvis calibration 3 of 3 complete; voice profile filed.

Next critical-path: Q8 THINK-stage (sub-commit 5 Q8.a/b capability table SMP composition; sub-commit 6 Q8.c blocked-thread-kill walk + process-exit sequencing).

---

**End of close report.**

*SC-3C closed at commit `ead8b19` (sub-commit 4c) plus closure integration at `165a2e2`. Q7 SMP correctness complete. Q8 next.*
