<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: Deferred-item trigger audit — what's ready, what's waiting, what's stale

**Date:** 2026-07-07
**Author:** Prometheus (docs/research lane)
**Source:** Plans/DISPATCH-2026-07-07-deferred-items-trigger-audit-prometheus.md
**Branch:** prometheus/deferred-items-trigger-audit
**Inputs:** OPEN-THREADS.md (OT-1 through OT-48, 67 lines); sweep of Plans/ and docs/ for "deferred", "follow-on", "when.*lands", "banked", "revisit", "Phase-N", "gated on" patterns; Bible Article 1.5/1.6; OT-26 THINK; reliability FINDINGS (RESEARCH-2026-07-07-arthur-agentic-reliability-FINDINGS.md); usability THINK (THINK-2026-07-07-usability-five-year-old-design.md)
**Scope:** Audit only. No code, no QEMU. Recommend, don't decide.

---

## TODAY's state (the audit baseline)

- **OT-43 FIXED (v2, 2026-07-07):** Memory-substrate hang dead. M1/M2/M3/M4 all landed.
- **OT-17 DONE:** Memory arc closed. Per-category encryption, forget, key-burn — all landed.
- **OT-34 DONE:** Multi-threaded matmul landed (deterministic column-partition fork-join).
- **OT-36 DONE:** Boot-frame stale self-report fixed.
- **A1 fully spec'd:** Reliability FINDINGS (XGrammar + bitmask-in-kernel + compact GBNF) + usability THINK (7 principles + 8-item checklist) both on main.
- **3B built and about to gate:** Qwen2.5-3B config-swap is the next brain step.
- **SDK-v0 landed:** Cross-SDK formalized (SDK-ABI.md, template-app, SDK-GUIDE.md).
- **OT-46 RATIFIED:** Arthur persona adopted as standing design lens. VISION-arthur-persona.md authoritative.
- **OT-26 RATIFIED:** Agentic action layer design THINK landed, A1 build dispatchable.
- **Phase 8 active:** Arthur arc in progress.

---

## LIST 1 — TRIGGER NOW MET but not scheduled (PRIMARY DELIVERABLE)

These items had a "when X" condition that is now satisfied. They should be considered for scheduling.

### 1A. OT-26 A1 build dispatch (the big one)
**Trigger:** "A1 build dispatchable" — substrate built/landing (action surface + execute_action cap-check + OT-18 grants + I-15 file-caps + OT-19 reversibility).
**Why now:** All substrate dependencies are landed. The reliability FINDINGS provide the constrained-decoding technique (XGrammar), integration mechanism (bitmask-in-kernel), and grammar shape (compact GBNF). The usability THINK provides the UX principles, clarify pattern, and first-boot onboarding. A1 is fully specified and ready to dispatch.
**Next step:** Dispatch A1 build (read/nav: NL → ReadFile/ListDir/InspectProcess → cap-check → NL result). Pairs with OT-46 persona for clarify/error strings.

### 1B. OT-19 build dispatch (reversibility mechanism)
**Trigger:** "Build dispatchable; the trilogy is fully ratified."
**Why now:** OT-17 (memory), OT-18 (grants), and OT-19 (reversibility) form the Arthur-maturation trilogy — all three are RATIFIED. OT-19's D2 mechanism (userland copy-before-write, .arthur/undo/, tombstone-on-delete) is designed and depends on nothing unbuilt. It's the safety net for A2 (mutating actions).
**Next step:** Dispatch OT-19 build as a Phase-8 slice. Sequences before A2 (which depends on OT-19's undo artifact).

### 1C. OT-16 forward-graph generalization — sequencing point approaching
**Trigger:** "Queued as a Phase-9 slice AFTER the 1.5B/3B config-swap."
**Why now:** The 3B config-swap is about to gate. Once it lands, OT-16's sequencing condition is met. The THINK is ratified (layer-kind schedule, 7 load-time validation invariants, double kill-switch). B1→B2→B3 plan is dispatchable.
**Next step:** When 3B gates, queue OT-16 B1 (walker-parity) for Phase-9. Don't pull forward before 3B lands — the sequencing is intentional.

### 1D. OT-44 H2 capability-native distribution — trigger approaching
**Trigger:** "Gated on the OT-22 multi-principal frontier, one pillar at a time."
**Why now:** H1 (cross-SDK) is DONE. H2 depends on OT-22's userland-frontier opening. OT-22 is BANKED-GATE — not yet open. But as A1/A2 progress, the question of third-party apps becomes real. The POSIX-personality question (ADR-023) is a named OT-22 pillar.
**Next step:** Flag for PL: when A1/A2 are built and the "what can third parties build?" question becomes concrete, consider opening OT-22's first pillar. Don't pull forward prematurely — OT-22's "one pillar at a time" discipline is the guardrail.

### 1E. OT-7 latency-oracle bare-metal run
**Trigger:** "Awaits the PL's Ryzen/OptiPlex rig window."
**Why now:** The benchmark harness is landed (305b04c). OT-34 (threaded matmul) is DONE — this is the measurement that turns the threading speedup from claimed to proven. The 3B brain is about to land — latency measurements are more valuable with the bigger model.
**Next step:** When the PL's OptiPlex window opens, run OT-7 bare-metal. This is the only thing blocking the §3 ledger row from "Unproven" to measured.

### 1F. OT-41 console AAG carry-forward — bare-metal visual proof
**Trigger:** "The definitive remaining proof is the OptiPlex console run showing Arthur's reply legible on screen."
**Why now:** All code nits are closed. The interactive console is merged (c50436b). QEMU proves real HMP key injection, real ReplyMode::Model, non-blank framebuffer, serial reply, kill-switch soundness. Only the physical visual proof remains.
**Next step:** PL bare-metal run on the OptiPlex. Bundle with OT-7 if possible (same hardware trip).

---

## LIST 2 — STILL CORRECTLY DEFERRED (with specific unmet condition)

### 2A. OT-1 license open questions (Q1-Q3)
**Unmet condition:** Before any public release. No release imminent.
**Status:** Correctly deferred. The provenance statement deliverable (2026-07-05 addition) is a standing discipline, not a build blocker.

### 2B. OT-4 VT-d deny-by-default
**Unmet condition:** First untrusted PCIe bus master dispatch (accel-driver-class lane).
**Status:** Correctly deferred. No untrusted PCIe master exists today.

### 2C. OT-5 per-step capability re-validation for long-running tensor ops
**Unmet condition:** New long-running governed-tensor op dispatch.
**Status:** Correctly deferred. Inference path already complies via per-pass mint.

### 2D. OT-6 pipelined-ring slot reclaim gated by per-slot seqno fence
**Unmet condition:** Accel ring goes from single-in-flight to pipelined multi-descriptor.
**Status:** Correctly deferred. Single-in-flight today.

### 2E. OT-9 kernel observability (ADR-014 Q1-Q7)
**Unmet condition:** "Phase 8+ when the Arthur arc allows." Off the Arthur critical path.
**Status:** Correctly deferred. A1/A2/A3 take priority.

### 2F. OT-10 native-NIC bare-metal network validation
**Unmet condition:** Hardware-network lane / deployment that needs a native NIC.
**Status:** Correctly deferred. USB-Ethernet proven; native NIC is low priority.

### 2G. OT-11 GPU stateful-BTI / DRM-expressibility hardening
**Unmet condition:** GPU path hardened for multi-tenant.
**Status:** Correctly deferred. Phase 6 closed without this; post-Phase-6 hardening.

### 2H. OT-12 per-worker worktrees
**Unmet condition:** PL/coders adopt when convenient (structural hygiene).
**Status:** Correctly deferred. Not a build blocker, operational improvement.

### 2I. OT-13 lock-order ceiling
**Unmet condition:** New lock needs ordering above 31.
**Status:** Correctly deferred. USB_CORE=31 is the current ceiling; no pressure.

### 2J. OT-15 scheduler QoS / per-principal CPU-time DoS-resistance
**Unmet condition:** "Gate the serving-surface / userland build where untrusted principals first become real."
**Status:** Correctly deferred. No untrusted principal exists today. Pairs with OT-22.

### 2K. OT-16 forward-graph generalization (see also 1C)
**Unmet condition:** 3B config-swap must land first (Phase-9 sequencing).
**Status:** Approaching trigger (1C) but not yet met. Correctly deferred until 3B gates.

### 2L. OT-20 accountability over many autonomous principals
**Unmet condition:** "When (or before) a second concurrent Arthur/agent principal becomes real."
**Status:** Correctly deferred. One Arthur today. Substrate is ready; accountability layer is the frontier.

### 2M. OT-22 userland-frontier sequencing guard
**Unmet condition:** Phase-8 arc-2 opens (multi-principal/untrusted-userland transition).
**Status:** Correctly deferred as BANKED-GATE. The guard itself — not a build, but a discipline constraint on future builds.

### 2N. OT-27 W2 WiFi follow-ons
**Unmet condition:** PL bare-metal wlan0 cold-lease run (OT-39).
**Status:** Correctly deferred. QEMU proven; silicon proof pending.

### 2O. OT-28 brain last-mile — bare-metal Stage-B
**Unmet condition:** Physical Stage-B on the OptiPlex (ship real .neom + sidecar ISO, prove Arthur on hardware).
**Status:** Correctly deferred. L1/L2/F1/F2/F3/1.5B boot-smoke all landed. Remaining proof is physical.

### 2P. OT-31 exit()/reap lock-release window (unconfirmed SMP hazard)
**Unmet condition:** Audit + construct repro or prove safe. Low priority — no known repro.
**Status:** Correctly deferred. OT-29 proved it wasn't the slab corruption cause.

### 2Q. OT-33 speculative decoding
**Unmet condition:** "Revisit IF a large GPU-hosted target exists."
**Status:** Correctly deferred. Wrong regime at our scale (3x gap, CPU scalar path).

### 2R. OT-39 CB-1b AAG carry-forward — bare-metal wlan0 cold lease
**Unmet condition:** PL's bare-metal run.
**Status:** Correctly deferred. QEMU/slirp proven; silicon proof pending.

### 2S. OT-42 sovereign personal super-device (multi-device federation)
**Unmet condition:** (a) in-flight federation research lands, (b) future PL ratification, (c) Phase-N opens multi-device axis.
**Status:** Correctly deferred as Phase-N strategic direction. Single-device N=1 stands.

### 2T. OT-45 visual design language (Article 1.6)
**Unmet condition:** Future GUI/visual phase opens.
**Status:** Correctly deferred. v1 is conversation+shell, GUI deferred. 1.6 remains a standing lens on small v1 surfaces.

### 2U. OT-46B voice (TTS/STT)
**Unmet condition:** Phase-N build. New model type (same substrate decision as OT-16/DeltaNet).
**Status:** Correctly deferred. Persona (OT-46A) is adopted; voice is Phase-N.

### 2V. OT-47 deep-SMP stress gate (NEO_SMP=8)
**Unmet condition:** Runner capacity not tight; CI-lane build when a window opens.
**Status:** Correctly deferred. Additive coverage, not a merge blocker.

### 2W. OT-48 IP / patentability candidates
**Unmet condition:** PL decision on whether to pursue offensive patents vs defensive publication.
**Status:** Correctly deferred. Strategic fork, not a build.

### 2X. OT-18 E5 (Process/Network/Model grants)
**Unmet condition:** "Open a scoped E5 only when Process/Network/Model grants become real."
**Status:** Correctly deferred. FileRead/FileWrite closed at E3. E5 triggers when A2/A3 need Process/Network/Model scopes.

### 2Y. OT-34 follow-on: bare-metal OT-7 read + wiring thread_count to online core count
**Unmet condition:** PL's OptiPlex window (OT-7 read); Arthur boot path integration.
**Status:** Correctly deferred. Column-partition matmul is DONE; remaining items are measurement and wiring.

### 2Z. ADR-023 linux-personality-layer sub-decisions
**Unmet condition:** Phase 8+ opens graphics-stack details, Mesa port, audio personality.
**Status:** Correctly deferred. Forward-marker shape (ADR-022 precedent).

### 2AA. ADR-017 WiFi support strategy — power management, userspace tooling
**Unmet condition:** PM subsystem lands (suspend/resume); Phase 8+ (userspace WiFi management).
**Status:** Correctly deferred. W1/W2 prove the driver; PM and userland are later.

---

## LIST 3 — STALE / SUPERSEDED (flag for reconciliation)

### 3A. OT-8 Arc-aware slab integration — RESOLVED, should be removed
**Status:** Marked "RESOLVED — removed as dead scaffolding" (Oracle, 2026-07-03). Both statics + configs + per-CPU magazines + SlabObject impls removed. The row should be deleted at next register audit.

### 3B. OT-32 Magazine::Drop wrong pointer — RESOLVED, should be removed
**Status:** Marked "RESOLVED — fix verified on main" (ad1495b). Both host KATs exercise Drop for real. The row should be deleted at next register audit.

### 3C. OT-35 CI artifact-storage quota — SELF-HEALING, consider removing
**Status:** Marked "SELF-HEALING — note for next time." Recurrence fix already in ci.yml (3-day retention + on-failure-only). This is a process note, not an open thread. Consider removing at next audit or converting to a runbook entry.

### 3D. OT-36 boot-frame stale self-report — DONE, should be removed
**Status:** Marked "DONE (2026-07-05 Rule-4 consolidation)." Version line and diagnostic feed updated. The row should be deleted at next register audit.

### 3E. OT-43 memory-substrate hang — FIXED, should be cleaned up
**Status:** Marked "FIXED (v2, 2026-07-07) — the memory-substrate hang is DEAD." The row is long and contains investigation history that's no longer actionable. Consider condensing to a single-line close note at next audit, keeping only the fix evidence and the breadcrumb-bank follow-on.

---

## OT-22 convergent-frontier watch (OT-15/16/17/18/19/20)

**The guard:** OT-22's "one pillar at a time" discipline — every userland-frontier dispatch resolves one pillar, kill-switched, with Rule-4 consolidation between.

**Current state of the pillars:**
- **OT-15 (scheduler QoS):** OPEN, gated on untrusted principals becoming real. NOT triggered. Correctly deferred.
- **OT-16 (data-driven graphs):** SCOPED + RATIFIED, queued Phase-9 AFTER 3B config-swap. Sequencing intact. NOT pulled forward prematurely.
- **OT-17 (memory substrate):** DONE. Memory arc closed. Not an OT-22 pillar — it's an Arthur-maturation pillar that completed independently.
- **OT-18 (grant ergonomics):** RATIFIED, E1-E3 landed. E5 deferred until Process/Network/Model grants become real. Sequencing intact.
- **OT-19 (reversibility/disagreement):** RATIFIED, build dispatchable. Completes the trilogy with OT-17/OT-18. Depends on nothing from OT-22. NOT pulled forward prematurely — it's an Arthur-maturation slice, not a userland-frontier expansion.
- **OT-20 (multi-principal accountability):** OPEN, gated on second Arthur principal. NOT triggered. Correctly deferred.

**Verdict:** The "one pillar at a time" discipline is being honored. No pillar is pulled forward prematurely. OT-17/18/19 completed as Arthur-maturation slices (Phase-8 arc-1), which is the correct sequencing — they don't expand the userland frontier, they make Arthur more capable within the N=1 floor. OT-15/20 remain correctly deferred until untrusted principals become real. OT-16 is correctly sequenced after 3B.

**Recommendation:** No action needed on the OT-22 guard. The sequencing is sound. When A1/A2 ship and the question of third-party apps or second Arthur principals becomes concrete, revisit OT-22's trigger for arc-2.

---

## AAG hardening follow-ons (OT-43 kill-switch + compile_error! combo-guard)

**Status:** The OT-43 dispatch notes mention "ship the 4-discriminator breadcrumb bank" and "close the -cpu max allowlist gap." These are tracked in the OT-43 row itself. No separate tracking needed — Jarvis's dispatch covers them.

**Other "fold into next consolidation" items:** OT-39's two cheap code nits (kill-switch memory-ordering asymmetry, wire-parse robustness) are marked DONE (2026-07-05 Rule-4 consolidation). Tracked in the OT-39 row. No separate tracking needed.

---

## Summary

| Category | Count | Items |
|----------|-------|-------|
| **Trigger NOW met — should be scheduled** | 6 | 1A (OT-26 A1), 1B (OT-19 build), 1C (OT-16 approaching), 1D (OT-44 H2 approaching), 1E (OT-7 bare-metal), 1F (OT-41 bare-metal) |
| **Still correctly deferred** | 25 | 2A-2AA (all with specific unmet conditions named) |
| **Stale / superseded — flag for cleanup** | 5 | 3A (OT-8), 3B (OT-32), 3C (OT-35), 3D (OT-36), 3E (OT-43) |

**Priority ordering for List 1:**
1. **OT-26 A1 build** — the headline deliverable. Fully spec'd, substrate landed, reliability + usability research complete.
2. **OT-19 build** — completes the Arthur trilogy. Safety net for A2.
3. **OT-7 + OT-41 bare-metal runs** — bundle with same OptiPlex trip. Measurement that turns claims into evidence.
4. **OT-16** — queue when 3B gates. Phase-9, not Phase-8.
5. **OT-44 H2** — flag for PL when A1/A2 make third-party apps relevant.

**OPEN-THREADS.md cleanup needed:** OT-8, OT-32, OT-35, OT-36 are resolved/done and should be removed at next register audit. OT-43 should be condensed to a close note.

---

## Gate + close

Docs-only, no code changes. No CI run exists (paths-ignored per dispatch).
