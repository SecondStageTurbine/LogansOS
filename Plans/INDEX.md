# Plans Directory Index

This directory holds in-flight and recently-closed working artifacts: PM
dispatches to coders, PM decision notes, Bible amendment drafts, and
session handoffs. Files in `Plans/` are working artifacts, not normative
project documents — normative docs (Bible, ADRs, ANSPEC, phase plans)
live in `docs/`.

This index is updated by hand. It is not authoritative; the truth is in
the files themselves and in `git log`. The index is a navigation aid.

## Status Legend

| Status | Meaning |
|--------|---------|
| **closed** | The artifact's intended commit has landed on `main`. Kept for record. |
| **superseded** | Overtaken by a follow-up that did the same work differently. Kept for record of what was tried. |
| **draft** | A working artifact that produced its commit but stays in tree as the source of the prose. |
| **active** | The artifact is in flight; the coder is working on it now or the closure gate has not been met. |
| **historical** | Captures past state (handoffs, decision notes, helper files). Not in flight, not a dispatch. |

## Open Project Commitments

Project commitments are deadlined obligations the project has explicitly
made. Unlike artifacts (which are records of past or in-flight work),
commitments are forward-looking statements about what the project will
deliver and by when. They live here so they cannot quietly slide.

PM cannot unilaterally re-defer a commitment. Per Bible Article 24.1,
re-deferral requires an explicit project-lead decision, recorded as an
ADR or amendment. Each commitment lists its de-escalation rule.

| Commitment | Deadline | Source artifact | De-escalation rule |
|---|---|---|---|
| Bare-metal latency re-read (latency oracle Tier B magnitudes) | next OptiPlex boot | `docs/complexity-governance.md` §5 | QEMU TCG cycles are virtualized, not silicon; need real constant-rate-TSC magnitudes for the measured budget. |
| W2 on-air supplicant (host-tier WPA/EAPOL/CCMP) | Phase 7.5-W2 | Bible Article 13 Phase 7.5 ledger | The MT7601U SoftMAC driver (W1) handles USB-DMA + MAC; W2 is the host-side supplicant that writes WPA/EAPOL/CCMP frames. Host writes crypto regardless of FullMAC vs SoftMAC (Prometheus research lane, 2026-06-13). |
| `commit_element` W2 forward-assert | Phase 7.5-W2 | Bible Article 13 Phase 7.5 ledger / ADR-017 Amendment | The W2 supplicant must assert commit_element correctness against the WPA state machine — the forward assertion from the W0/W0b crypto core. |
| Post-brain-cascade Rule-4 consolidation | current active lane | `DISPATCH-2026-07-19-consolidation-post-brain-cascade-jarvis.md` / `docs/complexity-governance.md` §2 Rule 4 | The OT-59 / Rung-0 / NetDialCap / OT-16 B3 wave triggered the mandated no-feature digest. |

### Met Commitments

| Commitment | Met | Discharge artifact | Discharge basis |
|---|---|---|---|
| Latency oracle (§4 performance-falsifiability) | 2026-06-14 | `DISPATCH-2026-06-14-build-lo-latency-oracle-jarvis.md` / commit `caff787` / ADR-052 | Hot-path latency oracle + per-operation micro-benchmark (Tier A host + Tier B QEMU). All four budget claims **HOLD under measurement** (no budget blown). Numbers recorded in `docs/complexity-governance.md` §5. DISCHARGED. |
| I-14 subsystem fault containment — all vectors | 2026-06-13 | `DISPATCH-2026-06-13-phase-7.5-build-V-vtd-iommu-jarvis.md` / commit `7fe15bb` / ADR-050 | I-14(a)/(b)/(c1) proven BUILD-A/B on silicon; I-14(c2) proven BUILD-V (VT-d/IOMMU, QEMU Stage A + OptiPlex Stage B). **I-14 FULLY PROVEN.** Integrity-before-feature pre-mandate discharged. |
| CI as a live gate — self-hosted runner | 2026-06-14 | `DISPATCH-2026-06-14-ci-self-hosted-jarvis.md` / `DISPATCH-2026-06-14-self-hosted-runner-setup-prometheus.md` / commits `d17dc6a` et al. | CI moved to self-hosted runner (WSL2). Cheap/expensive shard split. CI #161 green, P4b daemon race found+fixed (`5c0b3bb`). `[skip ci]` only for docs — never code changes (`f228bc0`). DISCHARGED. |
| Phase 8 Rule-4 consolidation | 2026-06-17 | `DISPATCH-2026-06-17-phase-8-consolidation-jarvis.md` / ADR-056 | No-feature digest: clippy blocking, no_std userland workspace guard, shared KV layout factoring, ledger/open-thread/ANSPEC/Bible reconciliation. |
| Phase 8/9 Rule-4 consolidation | 2026-06-18 | `DISPATCH-2026-06-18-consolidation-phase-8-9-jarvis.md` / ADR-059 | Seven-build digest since `645fd4b`: allow audit, Q4_K depack inspection, ADR-057/058/059 + ANSPEC + Bible + OT register reconciliation, I-1..I-14 re-audit, lock/LOC ledger re-measurement. |
| Post-OT-23/I-15/Arthur Rule-4 consolidation | 2026-06-20 | `DISPATCH-2026-06-20-rule4-consolidation-jarvis.md` / ADR-062 | No-feature digest after CSPRNG 10.3, OT-18 E1/E2/E3, OT-24, I-15, q8/Q4_K AVX2, W2 bank, OT-23, and Arthur M1/M2 prep: grant-store helper consolidation, ADR/pointer reconciliation, OT-23/24/25 register prune, I-1..I-15 audit, fresh lock/LOC metrics. |
| Phase 7.5 CB-6 consolidation | 2026-06-29 | `DISPATCH-2026-06-29-phase-7.5-cb-6-consolidation-janus.md` / `RETURN-2026-06-29-phase-7.5-cb-6-consolidation-janus.md` / `d958f8f` | Runner-gated close pass: clippy pay/bank, confirmed-dead scaffold removal, invariant-ledger page-fit, status repair, and resolved-OT prune. |
| Test-runner intermittent wedge fixed | 2026-05-08 | `archive/PHASE-1-2/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md` | Commits `4c6abf4` (L) + `aac0c11` (A1). 300/300 verification across `smp_per_cpu_scheduler_smoke` (200/200) + `cap_revoke_local` (50/50) + `cap_survive_exit` (50/50) at NEO_SMP=4. |
| ADR ratification backlog (018, 019, 020) ratified + Bible amended to v1.5.0 | 2026-05-10 | `archive/PHASE-1-2/DISPATCH-2026-05-10-adr-ratification-and-design-md-prometheus.md` | Six-commit dispatch: ADR-018/019/020 ratified, Bible v1.4.0 → v1.5.0 (Article 8.5.4 amended), `docs/design.md` v0.3 → v0.4 (NeoPlutarch identity), ANSPEC files renamed, INDEX.md and PHASE-2.md swept. |

## ARCHIVED DISPATCHES

Per cleanup audit 2026-05-28 (`CLEANUP-AUDIT-2026-05-28-prometheus.md`), closed Phase 1–3 dispatches were moved to `Plans/archive/`. All content preserved; git history intact. Detailed per-artifact entries for archived files remain in `git log` (commit `4620f02` and the archive commit).

| Phase | Files | Archive dir |
|-------|-------|-------------|
| Phase 1-2 (May 6–12) | 210 | `archive/PHASE-1-2/` |
| Phase 3 BUILD-1–3 (May 12–15) | 111 | `archive/PHASE-3-BUILD-1-3/` |
| Phase 3 BUILD-4 (May 16–19) | 54 | `archive/PHASE-3-BUILD-4/` |
| Phase 3 BUILD-5 (May 20–21) | 36 | `archive/PHASE-3-BUILD-5/` |
| Phase 3 CLOSE (May 22–28) | 32 | `archive/PHASE-3-CLOSE/` |
| Phase 2 REOPEN (May 24–27) | 78 | `archive/PHASE-2-REOPEN/` |
| Phase 4 THINK (May 20–22) | 12 | `archive/PHASE-4-THINK/` |
| Evidence (Phase 2) | 60 | `archive/evidence/` |
| **TOTAL** | **593** | — |

## Active Artifacts

### Phase 7 policy arc — P1→P4b + C1 (closed 2026-06-10; the residency policy layer)

| `THINK-2026-06-08-phase-7-policy-driven-placement.md` | closed | The policy-layer survey that named the P1→P4 line: placement → eviction → proactive → concurrent. |
| `archive/PHASE-4-7/DISPATCH-2026-06-08-phase-7-build-P1-residency-placement-jarvis.md` | closed (`dbe741d`) | §9.9 residency-aware placement — the kernel decides where an op runs. ADR-036. Gate `accel_placement_residency`. |
| `archive/PHASE-4-7/DISPATCH-2026-06-08-phase-7-build-P2-eviction-jarvis.md` | closed (`197c41b`) | §9.10 LRU eviction under pressure — the kernel makes room. ADR-037. Gate `accel_evict_pressure`. Evidence `evidence/phase7-buildP2-qemu-PASS-evict-pressure.log`. |
| `THINK-2026-06-10-phase-7-build-P3-cost-aware-migration.md` | closed (ratified) | Path A vs B fork; structural reuse signal; bytes-only cost; the §6-precursor design. |
| `archive/PHASE-4-7/DISPATCH-2026-06-10-phase-7-build-P3-cost-aware-migration-jarvis.md` | closed (`007d1ab`) | §9.11 proactive retain + cold-demote. ADR-038. Gates `accel_residency_retain`/`_demote`. Evidence `evidence/phase7-buildP3-qemu-PASS-residency-{retain,demote}.log`. |
| `THINK-2026-06-10-phase-7-build-P4-concurrent-residency.md` | closed (ratified) | The concurrency fork: P4a primitive / P4b daemon split; the §6 race-failure matrix. |
| `archive/PHASE-4-7/DISPATCH-2026-06-10-phase-7-build-P4a-migration-reservation-jarvis.md` | closed (`9599438`) | §9.12 migration-reservation primitive — move-while-referenced is safe. ADR-039 + the lockdep. Gate `accel_migration_reservation` (NEO_SMP stress). Evidence `evidence/phase7-buildP4a-qemu-PASS-migration-reservation.log`. |
| `archive/PHASE-4-7/DISPATCH-2026-06-10-phase-7-build-P4b-residency-daemon-jarvis.md` | closed (`9746e8c`) | §9.12 daemon — the kernel manages memory while it works. ADR-040. Gate `accel_residency_daemon` (critical path 0-vs-≥1). Evidence `evidence/phase7-buildP4b-qemu-PASS-residency-daemon.log`. |
| `archive/PHASE-4-7/DISPATCH-2026-06-10-phase-7-build-C1-consolidation-jarvis.md` | closed (`5348aab`) | The first Rule-4 consolidation: digestion, ledger audit, decaying H (demand-driven), the fresh-reader test. ADR-041. Net −295 LOC. |
| `THINK-2026-06-10-phase-7-build-O1-slot-indirected-ownership.md` | closed (ratified) | The ownership fork: handles point, the kernel owns; GEM/fd-table/slotmap prior art; the O1a/O1b split. |
| `archive/PHASE-4-7/DISPATCH-2026-06-10-phase-7-build-O1a-slot-arena-jarvis.md` | closed (`dd78926`) | The generational slot arena (mechanism only): SlotKey, mint=fget, free=drain+bump, I-13 + amended I-4. ADR-042. Gate `accel_slot_arena`. Evidence `evidence/phase7-buildO1a-qemu-PASS-slot-arena.log`. |
| `archive/PHASE-4-7/DISPATCH-2026-06-10-phase-7-build-O1b-ownership-conversion-jarvis.md` | closed (`7db1368`) | The ownership conversion: Handle/Model on SlotKey, mint-on-use, the production payoff gate. ANSPEC §9.13 + ADR-043 (+the dedup-pin finding). Gate `accel_model_residency`. Evidence `evidence/phase7-buildO1b-qemu-PASS-ownership.log`. |
| `DISPATCH-2026-06-11-phase-7-build-O2-dedup-slot-rekey-jarvis.md` | closed (`db88652`) | The dedup-to-slot re-key: candidacy pins nothing; share-survives-movement; the default load path under governance — THE PHASE 7 SPINE CLOSES. ANSPEC §9.7 + ADR-044. Evidence `evidence/phase7-buildO2-qemu-PASS-dedup-rekey.log`. |
| `THINK-2026-06-11-phase-7.5-opening-connectivity-lane.md` | closed (ratified) | The lane opening: DMA isolation first; staged hybrid (fence now, VT-d at 7.5-V); USB-first; QEMU-first. |
| `DISPATCH-2026-06-11-phase-7.5-build-0-fenced-dma-arena-jarvis.md` | closed (`eef664a`) | The fenced DMA arena — the I-14(c1) guard: pre-registered region, arena-backed DmaBuffer, the FencedDmaAddr witness gate. ADR-045. Evidence `evidence/phase75-build0-qemu-PASS-dma-fence.log`. |
| `DISPATCH-2026-06-11-phase-7.5-build-A-xhci-usb-core-jarvis.md` | closed (`12b64de`) | xHCI + USB core: enumeration gate, structural fencing, I-14(a/b) proven, the F1–F7 blind-run bank. ANSPEC §10.7 + ADR-046. Stage B PASS on the OptiPlex: first real-device enumeration on silicon (Intel AX201 8087:0026) in 3 conclusive blind iterations. Evidence `evidence/phase75-buildA-qemu-PASS-stageA.log` + `evidence/phase75-buildA-optiplex-PASS-stageB.log`. |
| `DISPATCH-2026-06-11-phase-7.5-build-B-usb-ethernet-jarvis.md` | closed (`766478f`) | CDC-ECM on the 7.5-A seam → the existing `NetDevice`: the Phase-4 stack over USB with zero new networking code. **Silicon-closed 2026-06-13:** AX88179 real LAN lease + DNS round-trip over CDC-ECM on bare metal. ANSPEC §10.8 + ADR-047. |

### Phase 7 early builds — accounting, scheduler, tensor-sharing, 4X arc (closed 2026-06-07–08)

| `archive/PHASE-4-7/DISPATCH-2026-06-07-phase-7-build-1-accel-accounting-jarvis.md` | closed (`906a587`) | Phase 7 BUILD-1: per-(Device::Accel, ProcessId) accounting — resident_bytes, dispatch_count, dispatch_time. ANSPEC §9.5 + ADR-033. Measurement only, no admission/quota/sharing/isolation. |
| `archive/PHASE-4-7/DISPATCH-2026-06-07-phase-7-build-2-admission-scheduler-jarvis.md` | closed (`ed9bcbb`) | Phase 7 BUILD-2: admission-control inference scheduler — per-device queue, priority → under-quota → FIFO. ANSPEC §9.6 + ADR-034. Decision demonstrated (scenario); live-path wiring next. |
| `archive/PHASE-4-7/DISPATCH-2026-06-07-phase-7-build-2L-scheduler-livepath-jarvis.md` | closed (`1042219`) | Phase 7 BUILD-2L: scheduler live-path wiring — sys_inf_run routes through admission queue, block-on-defer/wake-on-admit. Enforceability seam closed. ADR-034 Addendum. |
| `archive/PHASE-4-7/DISPATCH-2026-06-07-phase-7-build-3-tensor-sharing-dedup-jarvis.md` | closed (`c1fe512`) | Phase 7 BUILD-3: capability-mediated tensor sharing + content-hash dedup. Arc<Tensor> N→1 refcount, BLAKE3 dedup via Weak table, explicit RO grant. ANSPEC §9.7 + ADR-035. |
| `archive/PHASE-4-7/DISPATCH-2026-06-07-phase-7-build-4-1-ppgtt-positive-jarvis.md` | superseded | Phase 7 BUILD-4-1: legacy-ring PPGTT positive test — **FALSIFIED on Gen9** (3 root mechanisms select=GGTT). Blocker record `BLOCKER-2026-06-08-phase7-build41-ppgtt-requires-execlists.md`. Spawned the 4X arc. |
| `THINK-2026-06-07-phase-7-inference-scheduler-governance.md` | closed (ratified) | Phase 7 opening THINK: inference scheduler / multi-tenant governance. Goal: enforceable cross-tenant governance over device-resident tensors. |
| `THINK-2026-06-07-phase-7-build-4-ppgtt-spatial-isolation.md` | closed (ratified) | BUILD-4 THINK: per-context PPGTT spatial isolation — the hard bare-metal Intel lift. Hardens temporal→concurrent. |
| `THINK-2026-06-08-phase-7-build-4X-execlist-lrc-bringup.md` | closed (ratified) | The 4X fork: legacy-ring PPGTT falsified → execlist/LRC path. Five-lane research, GuC-free bring-up plan. |
| `RESEARCH-2026-06-08-gen9-execlist-lrc-bringup.md` | draft | 5-lane deep-research: i915 + Fuchsia oracle cross-validation of the GuC-free execlist/LRC path for Gen9. |
| `archive/PHASE-4-7/DISPATCH-2026-06-08-phase-7-build-4X-1-execlist-bringup-jarvis.md` | closed (`7626f62`) | 4X-1: GuC-free execlist/LRC bring-up — context via ELSP, CSB switch-in→idle, seqno landed. Unblocked the device-boundary path on silicon. |
| `archive/PHASE-4-7/DISPATCH-2026-06-08-phase-7-build-4X-2-ppgtt-data-jarvis.md` | closed (`e78cc79`) | 4X-2: tensor data through per-context PPGTT — Add [4,6] via execlist context, lhs/rhs/output cleared from GGTT. §9.8 positive-proven on silicon. |
| `archive/PHASE-4-7/DISPATCH-2026-06-08-phase-7-build-4X-3-negative-boundary-jarvis.md` | closed (`c48d5d5`) | 4X-3: cross-tenant negative test — B's A64 access to A's tensor page-faulted. §9.8 spatial isolation → ENFORCEMENT-PROVEN. **All four governance lines real on hardware.** |
| `CLOSE-REPORT-2026-06-08-phase-7-device-boundary-4X.md` | closed | 4X arc close report: legacy-ring falsification → 5-lane research → execlist/LRC bring-up → negative test. Device-boundary line CLOSED. |
| `CLOSE-REPORT-2026-06-11-phase-7.md` | closed (`db88652`) | **Phase 7 CLOSE-REPORT.** Phase 7 marked Complete. Every governance line reaches production-default loads. Opened Phase 7.5. |

### Phase 7.5 connectivity lane — K, V, WiFi arc, research (2026-06-13–14)

| `DISPATCH-2026-06-13-phase-7.5-build-K-consolidation-jarvis.md` | closed (`e457a2b`) | Phase 7.5 BUILD-K: connectivity-lane consolidation — Rule-4 digest #2. ANSPEC §10.x renumbered, ledger audited 14/14, lock census re-measured (37 production statics). ADR-048. No dead code subtracted; additive documentation. |
| `THINK-2026-06-13-phase-7.5-build-V-vtd-iommu.md` | closed (ratified) | BUILD-V THINK: VT-d/IOMMU design — DMA-remapping only, fence domains for connectivity, identity domains for substrate. I-14(c2) arm plan. |
| `DISPATCH-2026-06-13-phase-7.5-build-V-vtd-iommu-jarvis.md` | closed (`7fe15bb`) | Phase 7.5 BUILD-V: VT-d/IOMMU driver — ACPI DMAR parse, domains, fault path. Stage A QEMU + Stage B OptiPlex/Comet Lake PASS. **I-14(c2) silicon-PROVEN; I-14 FULLY PROVEN.** ANSPEC §10.6 + ADR-050. |
| `RESEARCH-2026-06-13-wifi-target-fullmac-vs-softmac.md` | draft | Prometheus research: "FullMAC eliminates the supplicant" — **DISPROVED**. Host writes WPA/EAPOL/CCMP regardless. Target shifted to smallest USB-attached BSD-licensed SoftMAC chip: MT7601U. |
| `RESEARCH-2026-06-13-wifi-target-fullmac-vs-softmac-FINDINGS.md` | draft | WiFi target FINDINGS: MT7601U over iwlwifi (~5.5K LOC vs 80K+; BSD-3-Clause firmware; USB-DMA-contained). Article 24.1 ratified. |
| `PL-DECISION-2026-06-13-wifi-target-mt7601u.md` | closed | Article 24.1 PL decision: Intel iwlwifi → MediaTek MT7601U. Bible Article 9.10 + §13 7.5 line updated. ADR-017 Amendment. |
| `THINK-2026-06-13-phase-7.5-wifi-arc-mt7601u-supplicant.md` | closed (ratified) | WiFi arc THINK: MT7601U SoftMAC driver (W1) + host-tier WPA supplicant (W2). W0 = WPA2 crypto core, W0b = WPA3 SAE. |
| `DISPATCH-2026-06-13-phase-7.5-build-W0-wpa-crypto-jarvis.md` | closed (`f996057`) | Phase 7.5 BUILD-W0: WPA2 crypto core — PMK, PRF-SHA1, AES-CMAC, EAPOL-Key MIC, CCMP-TK, PMKID. 11 KATs green. ANSPEC §10.9 + ADR-051. SAE carved to W0b. |
| `DISPATCH-2026-06-13-sae-vector-sourcing-prometheus.md` | closed (`45b9389`) | Prometheus: source verified raw-source SAE/P-256 KAT from hostap + iwd, PWE cross-validated. Unblocks BUILD-W0b. |
| `RESEARCH-2026-06-13-sae-vector-sourcing-FINDINGS.md` | draft | SAE P-256 vector sourcing FINDINGS: hostapd + iwd KATs, PWE cross-validated. |
| `DISPATCH-2026-06-13-phase-7.5-build-W0b-sae-jarvis.md` | closed (`fcf233d`) | Phase 7.5 BUILD-W0b: WPA3 SAE (dragonfly/P-256, H2E) crypto + cross-source KATs. ANSPEC §10.9 + ADR-051 amended. **WPA2+WPA3 crypto core complete.** HnP-handshake noted for W2. |
| `DISPATCH-2026-06-12-branding-refresh-v2-jarvis.md` | closed (`9bd1b5a`) | Branding visual identity migrated to LogansOS — crest, logo assets, boot logo (160×160×4 with compile-time byte oracle), serial banner. Reproducible pure-stdlib generator at `tools/branding/generate.py`. LogansOS rebrand (text + visual) COMPLETE. |
| `DISPATCH-2026-06-14-mt7601u-bringup-trap-mine-prometheus.md` | closed (`0db24d8`) | Prometheus: MT7601U deep bring-up trap mine — raw-source verified sequence + 20-trap bank. eFUSE hard-fail flagged. FINDINGS folded into W1 THINK (`15d5b35`). |
| `RESEARCH-2026-06-14-mt7601u-bringup-trap-mine-FINDINGS.md` | draft | MT7601U trap-mine FINDINGS: raw driver source analysis, bring-up sequence, 20 identified traps. eFUSE cleared on the dongle in hand. |
| `THINK-2026-06-14-phase-7.5-build-W1-mt7601u-driver.md` | closed (ratified) | W1 THINK: MT7601U driver pre-stage skeleton — governance scaffolding, chip facts folded from Prometheus trap mine. ADR-054. |
| `DISPATCH-2026-06-14-phase-7.5-build-W1-mt7601u-driver-jarvis.md` | active (`a31efbe`) | Phase 7.5 BUILD-W1: MT7601U driver (scan) — dongle in hand, eFUSE cleared. **IN FLIGHT, Cerebro/Jarvis.** Ungated from CI #171 (`8d1f778`); running in parallel (separate hardware). |

### Phase 5 close — BUILD-5 embedding + bare-metal verification (2026-06-03–04)

| `archive/PHASE-4-7/DISPATCH-2026-06-03-phase-5-build-5-embedding-jarvis.md` | closed (`34e3291` family) | Phase 5 BUILD-5: Embedding op (opcode 9, I32 gather). Full op set 0..11. Layer 2 complete. |
| `CLOSE-REPORT-2026-06-03-phase-5-build-4-transformer-ops.md` | closed | BUILD-4 close: transformer ops verified at SMP=4. |
| `archive/PHASE-4-7/DISPATCH-2026-06-04-phase-5-inference-baremetal-jarvis.md` | closed | Phase 5 bare-metal verification: infer_transformer_forward PASS on 12-CPU OptiPlex via on-disk NEO_LOG readback (sha256 41702CDA...). Two SMP kernel defects found+fixed: TLB-shootdown timeout flake (`5a9c878`) + device-IRQ/IPI swapgs/per-CPU-GS hole (`f5aacff`). |
| `archive/PHASE-4-7/DISPATCH-2026-06-02-tlb-shootdown-timeout-flake-jarvis.md` | closed (`5a9c878`) | TLB-shootdown timeout flake fix — bare-metal SMP defect found during Phase 5 metal close. |
| `archive/PHASE-4-7/DISPATCH-2026-06-04-swapgs-device-irq-page-fault-jarvis.md` | closed (`f5aacff`) | Device-IRQ/IPI swapgs/per-CPU-GS hole fix — bare-metal SMP defect found during Phase 5 metal close. |
| `archive/PHASE-4-7/RETURN-2026-06-04-swapgs-device-irq-page-fault-jarvis.md` | closed | Jarvis return: swapgs/device-IRQ page fault root cause and fix. |
| `CLOSE-REPORT-2026-06-03-tlb-shootdown-timeout-flake.md` | closed | TLB-shootdown timeout flake close report. |
| `CLOSE-REPORT-2026-06-04-phase-5-build-5-embedding.md` | closed | BUILD-5 close: Embedding op, full op set 0..11. |
| `PHASE-5-CLOSE-PLAN-2026-06-04.md` | closed | Phase 5 close plan: end-to-end milestone (.neom from NeoFS + SYS_INFER_RUN forward pass) verified on QEMU at SMP=4 AND bare-metal on 12-CPU OptiPlex. |
| `CLOSE-REPORT-2026-06-04-phase-5.md` | closed | **Phase 5 marked Complete.** Two SMP kernel defects fixed during metal close. |
| `BLOCKER-2026-06-04-phase-5-inference-baremetal-smp4-page-fault.md` | closed | Blocker: bare-metal SMP=4 page fault during Phase 5 inference verification. Resolved by TLB + swapgs fixes. |

### Phase 6 — AI accelerator driver class (2026-06-04–07; Complete)

| `PM-DECISION-2026-06-04-phase-6-opening.md` | closed | Phase 6 opening: accel driver class, CPU fallback retained (Article 6.5). ADR-030 + THINK pass. |
| `THINK-2026-06-04-phase-6-build-2-virtio-accel.md` | closed (ratified) | BUILD-2 THINK: virtio-accel re-sliced — accel_sim (in-kernel logical multi-device), custom virtio-accel via vhost-user daemon, real GPU. |
| `THINK-2026-06-05-phase-6-build-4-real-gpu.md` | closed (ratified) | BUILD-4 THINK: real GPU bring-up plan. Intel Gen9.5 UHD 630 target. |
| `THINK-2026-06-07-phase-6-build-4b-op-set-equivalence.md` | closed (ratified) | BUILD-4b THINK: op set equivalence — forward-pass byte identity over mixed GPU/CPU. |
| `archive/PHASE-4-7/DISPATCH-2026-06-04-phase-6-build-1-accel-driver-class-jarvis.md` | closed (`9d8c6cf`) | Phase 6 BUILD-1: AccelDriver trait + registry, DeviceId/DeviceStorage residency, accel_cpu reference driver. Tensor migrate. Transparency proven at SMP=4. ANSPEC §9.2 amended. |
| `archive/PHASE-4-7/DISPATCH-2026-06-04-phase-6-build-2-accel-sim-jarvis.md` | closed (`f5076d6`) | Phase 6 BUILD-2: accel_sim — in-kernel reference accelerator on Device::Accel(1). Arena + lazy async completion. Non-default-device + real-copy + async-poll/wait paths exercised. |
| `archive/PHASE-4-7/DISPATCH-2026-06-05-phase-6-build-3a-virtio-accel-transport-jarvis.md` | closed (`305875e`) | Phase 6 BUILD-3a: virtio-accel transport — kernel virtio-pci driver + host vhost-user daemon. IRQ-driven completion. CI QEMU bumped to trixie + pinned v10.2.3. ADR-031. |
| `archive/PHASE-4-7/DISPATCH-2026-06-05-phase-6-build-3b-virtio-accel-ops-jarvis.md` | closed (`34e3291`) | Phase 6 BUILD-3b: virtio-accel real ops — full op set over frozen 3a transport. Byte-identical to accel_cpu by construction (#[path]-include). Migration clean. BUILD-3 complete. |
| `archive/PHASE-4-7/DISPATCH-2026-06-05-phase-6-build-4a-intel-gpu-bringup-jarvis.md` | closed (`ab8fa0d`) | Phase 6 BUILD-4a: Intel Gen9.5 GPU compute bring-up — [1,2]+[3,4]=[4,6] byte-exact on bare-metal UHD 630. GuC-free legacy-render-ring GPGPU. ADR-032 iteration log. Feature accel-intel, default-off, probe-gated. |
| `archive/PHASE-4-7/DISPATCH-2026-06-05-phase-6-build-4a-iter2-forcewake-jarvis.md` | closed | 4a iter2: forcewake fix. |
| `archive/PHASE-4-7/DISPATCH-2026-06-05-phase-6-build-4a-iter3-mmio-breadcrumbs-jarvis.md` | closed | 4a iter3: MMIO breadcrumbs. |
| `archive/PHASE-4-7/DISPATCH-2026-06-06-phase-6-build-4a-intel-finalizer-toolchain-prometheus.md` | closed | Prometheus: finalizer-built EU kernel toolchain — tools/intel-gen9-finalizer, pinned IGC/ocloc, ocloc-disasm byte oracles. |
| `archive/PHASE-4-7/DISPATCH-2026-06-06-phase-6-build-4a-iter4-bisect-jarvis.md` | closed | 4a iter4: bisect. |
| `archive/PHASE-4-7/DISPATCH-2026-06-06-phase-6-build-4a-iter5-eu-dispatch-bisect-jarvis.md` | closed | 4a iter5: EU dispatch bisect. |
| `archive/PHASE-4-7/DISPATCH-2026-06-06-phase-6-build-4a-iter6-state-base-address-jarvis.md` | closed | 4a iter6: STATE_BASE_ADDRESS — EU finally fetches. |
| `archive/PHASE-4-7/DISPATCH-2026-06-07-phase-6-build-4a-iter7-stateless-add-arg-payload-jarvis.md` | closed | 4a iter7: stateless add arg payload — GPU fault-address capture → precise wrong-GRF verdict. |
| `archive/PHASE-4-7/DISPATCH-2026-06-07-phase-6-build-4a-iter8-gpgpu-thread-payload-layout-jarvis.md` | closed (`ab8fa0d`) | 4a iter8: GPGPU thread payload layout — NEO indirect-data reframe. **Decisive unlock.** 8 bare-metal iterations to first compute. |
| `archive/PHASE-4-7/DISPATCH-2026-06-07-phase-6-build-4a-stateless-add-arg-abi-prometheus.md` | closed | Prometheus: stateless add arg ABI — ocloc-disasm verification of the iter8 payload. |
| `archive/PHASE-4-7/DISPATCH-2026-06-07-phase-6-build-4b1-intel-op-dispatcher-matmul-jarvis.md` | closed (`dc27459`) | Phase 6 BUILD-4b-1: Intel op dispatcher + matmul — OpKind router, 1216-B stateless-A64 matmul_f32 EU kernel. MATMUL_REL_TOLERANCE=2^-12 vs host f32. Stage-B OptiPlex PASS. |
| `archive/PHASE-4-7/DISPATCH-2026-06-07-phase-6-build-4b2-intel-gelu-and-close-jarvis.md` | closed (`fff256d`) | Phase 6 BUILD-4b-2: Intel GPU GELU + forward-pass-equivalence + tensor migration. Stateless-A64 GELU EU kernel (Gen9 hardware-tanh). Mixed-precision tolerance-reference model. **Phase 6 COMPLETE.** |
| `CLOSE-REPORT-2026-06-07-phase-6.md` | closed (`fff256d`) | **Phase 6 marked Complete.** Accel driver class (cpu/sim/virtio) + real Intel iGPU running matmul/add/GELU with mixed-precision fallback + migration, bare-metal verified. Post-Phase-6 hardening: stateful-BTI/DRM (ADR-023), full-SIMD32 mask back-port, pre-softmax forward-equiv sharpening. |

### CI infrastructure — host-test gates, sharding, self-hosted runner (2026-06-01–14)

| `archive/PHASE-4-7/DISPATCH-2026-06-01-ci-host-test-lint-gates-prometheus.md` | closed (merged) | Logic-test crates made workspace members + CI `host-checks` job. Closed all non-kernel fmt+clippy debt (56 files). |
| `archive/PHASE-4-7/DISPATCH-2026-06-01-ci-sched-capacity-pressure-jarvis.md` | closed | CI fix: sched_ready_queue_capacity_pressure scenario assertion failure. Resolved during Phase 5/6 CI work. |
| `archive/PHASE-4-7/DISPATCH-2026-06-05-ci-shard-and-qemu-base-prometheus.md` | closed | CI sharding + QEMU base update — trixie + pinned QEMU v10.2.3 for virtio-accel CI support. |
| `DISPATCH-2026-06-14-ci-greening-jarvis.md` | closed (`a902630`) | CI-greening: get the family green, re-establish CI as a live gate. CI #161 green; P4b daemon race found+fixed (`5c0b3bb`). Invariants preserved. |
| `DISPATCH-2026-06-14-self-hosted-runner-setup-prometheus.md` | closed (`881efb3`) | Stand up GitHub Actions self-hosted runner — WSL2, under AltimitNeo. Commits: `881efb3` (dispatch), `d17dc6a` (move CI), `6aea685` (cargo bin cache), `9e87ac0` (rustup PATH), `1154583` (Limine exposure). |
| `DISPATCH-2026-06-14-ci-self-hosted-jarvis.md` | closed (`d17dc6a`) | Move CI to self-hosted runner + cheap/expensive split. Gated on runner Idle. CI now live on self-hosted. `[skip ci]` only for docs — never code changes (`f228bc0`). |

### BUILD-LO — the §4 latency oracle (2026-06-14)

| `DISPATCH-2026-06-14-build-lo-latency-oracle-jarvis.md` | closed (`caff787`) | BUILD-LO: the §4 latency oracle — cost is now measured, gated, falsifiable. Hot-path micro-benchmark (Tier A host i9-9900K + Tier B QEMU). All four budget claims **HOLD**. Numbers in `docs/complexity-governance.md` §5. ADR-052. |

### Arthur amendment — Bible v1.11.0 (2026-06-14)

| `docs/bible.md` (v1.11.0) | amended (`0c7f175`) | **The integral-assistant amendment.** Article 23.1 reversed from its original prohibition; new Article 29 defines Arthur (Assistant for Real-time Tasks, Home automation, & User Requests). Articles 3, 6.6 amended. PL-directed under Articles 24.1/27. Version 1.10.0 → 1.11.0. |
| `docs/VISION-logansos-to-jarvis.md` (`f3b5194`) | draft | VISION doc: LogansOS → JARVIS (the north star). The committed direction for the integral assistant layer. |

### Phase 8 — inference maturity (opened 2026-06-14; Active)

| `THINK-2026-06-14-phase-8-opening-inference-maturity.md` | closed (ratified) (`1bf1327`) | Phase 8 opening THINK: inference maturity (JARVIS L1). Decode loop → KV cache → tokenizer → quant. Bible Article 13 Phase 8 entry added (`e96ac06`). |
| `DISPATCH-2026-06-14-phase-8-build-8.1-decode-loop-janus.md` | active (`f91a862`) | Phase 8 BUILD-8.1: autoregressive decode loop + sampling → Janus. **IN FLIGHT, Janus.** |
| `DISPATCH-2026-06-14-phase-8-gap-analysis-prometheus.md` | closed (`c491a59`) | Phase 8 real-model gap analysis (raw-source) → Prometheus: Qwen2.5-0.5B architecture, ops mapping, GGUF quant layout, tokenizer KAT, scoping. |
| `RESEARCH-2026-06-14-phase-8-real-model-gap-analysis-FINDINGS.md` | draft (`c491a59`) | Gap analysis FINDINGS: Qwen2.5-0.5B architecture decomposition, ops inventory, GGUF quantization layout, tokenizer knowledge-assertion-test. |
| `THINK-2026-06-14-phase-8-reground-8.2-to-8.5-gguf-importer.md` | closed (ratified) (`c7b8fff`) | Phase 8: re-ground 8.2→8.5 from gap analysis; ratify GGUF-importer format decision. Q4_K block size corrected to 144 bytes (`34f1ebb`). |
| `DISPATCH-2026-06-16-phase-8-build-8.3-integration-janus.md` | closed | Phase 8.3 integration: dequant-to-f32 matmul on real Qwen2.5-0.5B weights. |
| `DISPATCH-2026-06-16-phase-8-build-8.4-rope-swiglu-forward-janus.md` | closed | Phase 8.4: RoPE + SwiGLU real-weight forward scaffold. |
| `DISPATCH-2026-06-16-phase-8-build-8.5-gqa-kvcache-generation-jarvis.md` | closed | Phase 8.5: GQA attention, governed KV cache, and real Qwen top-1 generation. |
| `DISPATCH-2026-06-17-phase-8-consolidation-jarvis.md` | closed (`214811d`) | Phase 8 Rule-4 consolidation. ADR-056. Shared KV layout helper, blocking clippy/no_std gates, ledger reconciliation. |
| `DISPATCH-2026-06-17-arthur-slice-4-intent-action-janus.md` | closed (`8d7c780`) | Arthur arc-2 Slice 4: intent-to-capability-bounded action loop. One of the ADR-059 seven-build consolidation inputs. |
| `DISPATCH-2026-06-17-phase-9-build-9.1-quantized-matmul-scalar-jarvis.md` | closed (`c7f7e5d`) | Phase 9.1: scalar Q4_K x Q8_K matmul helper + q8_K activation quantizer. ADR-058. |
| `DISPATCH-2026-06-17-phase-9-build-9.2-wire-quantized-forward-jarvis.md` | closed (`93822e6`) | Phase 9.2: default-off scalar quantized forward wiring with dual f32/q4_K model storage. ADR-058. |
| `DISPATCH-2026-06-17-kernel-csprng-10.1-entropy-drbg-prometheus.md` | closed (`ca1bd4d`) | Phase 10.1: kernel CSPRNG entropy + ChaCha20 DRBG + KATs. ADR-057. |
| `DISPATCH-2026-06-17-input-path-framebuffer-keyboard-janus.md` | closed (`eacb7f7`) | Phase 8b input path: framebuffer console + USB HID input gate. |
| `DISPATCH-2026-06-18-phase-9-build-9.3-avx2-quantized-matmul-jarvis.md` | closed (`16c7ffb`) | Phase 9.3: host-tier AVX2 Q4_K x Q8_K equality gate; in-kernel AVX2 swap blocked on OT-23 YMM-state preservation. ADR-058. |
| `DISPATCH-2026-06-18-arthur-physical-console-integration-janus.md` | closed (`005b980`) | Arthur console QEMU scenario. One of the ADR-059 seven-build consolidation inputs. |
| `DISPATCH-2026-06-18-consolidation-phase-8-9-jarvis.md` | closed (`6c13773`) | Phase 8/9 Rule-4 consolidation over the seven builds since `645fd4b`. ADR-059 in this lane; zero features/invariants/unsafe. |
| `DISPATCH-2026-07-02-rule4-consolidation-jarvis.md` | active | Post-CB-6 Rule-4 clean-tree pass over A2/1.5B/OT-41/1.5B-boot: dead-scaffolding scan, clippy hygiene, ledger re-measure, OT-28/OT-41 status reconcile, and legibility/index refresh. |

### Arthur arc — memory, persona, SDK, CI (2026-07-03 to 2026-07-07)

| `docs/ADR-071-arthur-m4-memory-forgetting.md` | accepted (`e517a52`) | Arthur M4: memory forgetting and total wipe — `forget <category>`, `forget everything`, keys-burned-first, overwrite-pattern oracle. Closes OT-17 (memory arc). |
| `docs/VISION-arthur-persona.md` | draft | Arthur's character & voice — understated English butler persona, 9 register rules + 10 mechanism-mapped worked examples. Ratified OT-46(A). Authoritative tone reference for Arthur's text surfaces. |
| `docs/SDK-ABI.md` | draft (`5f77...`) | OT-44 H1: cross-SDK ABI reference — tier list (STABLE/STABILIZING/EXPERIMENTAL/BREAKS-FREELY), versioning policy, syscall + errno tables transcribed from source. |
| `docs/SDK-GUIDE.md` | draft (`5f77...`) | OT-44 H1: cross-development quickstart — toolchain pin, target spec, building the template, contrib-module bake-in, honest boundaries (no install-after-boot, static-only, cross-compile only). |
| `userland/template-app/` | drafted (`5f77...`) | OT-44 H1: minimal "Hello, LogansOS" crate — static-links `libneo`, workspace-compiled, lint-clean. Not wired into ISO/QEMU; developers copy it. |
| `docs/runbook-ci-runner-capacity.md` | drafted | CI runner capacity runbook — `.wslconfig` settings, apply procedure (drain → `wsl --shutdown` → verify), symptom table (exit-137 / long-cancel / shard-4 tail, OT-43 amplification link), threshold rationale. |
| `RESEARCH-2026-07-06-ot43-substrate-hang-trapmining-FINDINGS.md` | drafted | OT-43 prior-art trap-mining (Prometheus) — five documented patterns (A-E) ranked against our sched/virtio code, discriminator cross-reference for Jarvis's breadcrumbs. |
| `RESEARCH-2026-07-07-ot43-virtio-irq-completions-FINDINGS.md` | drafted | OT-43 fix v2 prior-art (Prometheus) — IRQ-driven virtio-blk completions reference: MSI-X wiring, EVENT_IDX suppression handshake, ISR pattern, I-14 ISR-lock boundary analysis. |

### `.neom` v3 / Atrium G1 / OT-63 / vault ADRs (2026-07-09 to 2026-07-20)

| `docs/ADR-072-neom-v3-layer-kind-schedule.md` | accepted (`3f62ec0`) | OT-16 B1: `.neom` v3 layer-kind schedule section — forward-graph shape moved into `.neom` data with its own ADR-028-family byte oracle; I-1 byte identity preserved through the walker parity KAT. |
| `docs/ADR-073-atrium-surface-protocol-v1.md` | accepted for G1 Slice 2 (`d7b6c30`) | Atrium G1 surface protocol v1 — role-typed surface verbs over IPC channels, SurfaceCap/BufferCap grant rendezvous; includes the Slice-3 cross-process buffer-lifetime (Qubes-C3 relocated-surface) append. I-14/I-15. |
| `docs/ADR-074-atrium-kernel-routed-input.md` | accepted for G1 Slice 4 (`ed9ab0d`) | Atrium G1 kernel-routed input — HID delivery through the atomic InputFocus route, no-snoop, handoff, bounded drop-oldest queue. Disjoint from ADR-075. |
| `docs/ADR-075-cap-bounded-password-vault.md` | accepted (`b3b35ca`) | Cap-bounded password vault (Phase A) — split operations, never master material; capability-attenuated under I-15. Disjoint from ADR-074. |
| `docs/ADR-076-scanout-dma-fence.md` | accepted for OT-63 GPU Slice 3 (`42e4e9c`) | Scanout DMA fence (I-14(c1)) — `arm_flip` is witness-only, taking an un-forgeable `FencedDmaAddr` minted only inside `dma_arena::fence()`; the synthetic (unfenced) path is private and does no DMA (Slice-2 kill-switch baseline). Slice-4 obligation: make `PendingFlip._scanout` non-optional at real `PLANE_SURF` programming. |
| `docs/ADR-077-gpu-p1-compute-service.md` | accepted (`b0710ad`) | GPU P1: minimal manifest-only Gen9 compute service + one project-authored linear-buffer separable-Gaussian glass-blur kernel via the proven ocloc path (Vello/WGSL/wgpu fenced to P2). `Handle::GpuCompute`/`GPU_COMPUTE_ENABLE` kill-switch; the graphics VT-d/fenced-physical-DMA boundary must precede any userland compute queue (PPGTT isolates contexts, not host memory). Codex cross-check pending before Slice 1. |

### Phase 5 — kernel clippy debt cleanup (2026-06-04)

| `archive/PHASE-4-7/DISPATCH-2026-06-04-kernel-clippy-debt-cleanup-prometheus.md` | closed | Prometheus: kernel clippy-warning burndown. Standing follow-on from Phase 5 BUILD-3 quality-gate audit. |

### Phase 7.5 — fmt pre-existing debt (2026-06-14)

| `fmt: cargo fmt cdc_ecm.rs` (`4b6a20c`) | closed | Pre-existing BUILD-B formatting debt — CI fmt gate catch on the self-hosted runner. |

### INDEX refresh dispatch (2026-06-14)

| `DISPATCH-2026-06-14-index-refresh-prometheus.md` | closed (`9db5dec`) | INDEX refresh through Phase 5/6 close, full Phase 7 arc, complexity-governance/I-14, Phase 7.5, Arthur amendment (Bible v1.11.0), Phase 8 opening, BUILD-LO, and CI/self-hosted-runner migration. Reconciled against Article 13. |

### Phase 5 BUILD-4 — transcendental transformer ops (Layer 2, 2026-06-01)

| File | Status | What it is |
|------|--------|------------|
| `archive/PHASE-4-7/DISPATCH-2026-06-01-phase-5-build-4-transformer-ops-jarvis.md` | closed (bounced, then reworked+merged) | → Jarvis: Softmax/Gelu/LayerNorm/RmsNorm + soft-float transcendentals. Bounced for accuracy (~5% error vs ≤2^-12 gate), reworked, merged. Phase 5 BUILD-4 complete per `CLOSE-REPORT-2026-06-03-phase-5-build-4-transformer-ops.md`. |
| `archive/PHASE-4-7/DISPATCH-2026-06-01-phase-5-build-4-BOUNCE-accuracy-jarvis.md` | closed (reworked) | REWORK: submitted BUILD-4 reported ~5% transcendental error vs the ≤2^-12 gate (~200× over). Reworked and merged; the bounce artifact is retained for record. |
| `archive/PHASE-4-7/DISPATCH-2026-06-01-ci-sched-capacity-pressure-jarvis.md` | closed | CI fix: sched_ready_queue_capacity_pressure scenario assertion failure. Resolved during Phase 5/6 CI work. |

### Phase 5 BUILD-3 — CPU inference executor (Layer 2, 2026-06-01)

| File | Status | What it is |
|------|--------|------------|
| `docs/ADR-029-phase-5-layer-2-cpu-inference-design.md` | accepted | Layer 2 design: D1 **soft-float** (no hardware FP — sidesteps the XMM-not-saved hazard, Article 6.5), synchronous `inf_run`, `match` op-dispatch, the writable-tensor API (gives BUILD-1 immutability teeth), kernel-allocates-outputs ABI, no 8.2 scheduling class (Phase 7), syscalls `0x120..0x12F`, and the BUILD-3/BUILD-4 slice (MLP ops now, transformer/transcendental ops next). Q1 (FP) decided soft-float; PL may reverse toward FXSAVE. |
| `archive/PHASE-4-7/DISPATCH-2026-06-01-phase-5-build-3-cpu-inference-executor-jarvis.md` | closed (merged `806756f`) | → Jarvis: graph executor + soft-float ops (`Identity/Reshape/Transpose/Add/Mul/MatMul/Relu`), writable-tensor API, `SYS_INFER_RUN=0x120`, end-to-end MLP forward pass. First branch through the rebase-before-close gate — arrived ff-clean (pm-merge-check confirmed). |
| `CLOSE-REPORT-2026-06-01-phase-5-build-3-cpu-inference-jarvis.md` | closed | BUILD-3 close: Layer 2 CPU inference — **Phase 5 goal milestone reached** (a `.neom` model runs a CPU forward pass). PM-verified in-container: MLP oracle bit-exact (host ai-ops-tests + QEMU `infer_mlp_forward` @ SMP=4), full host-checks gate green (fmt/clippy/test --workspace), kernel builds, `--locked` clean, `SYS_INFER_RUN=0x120` coherent, no-XMM by disassembly. Also fixed a latent BUILD-2 bug (neofs-mkfs embedded a relative-offset `.neom`). NOTE: worker pushed direct-to-main (protocol break); branch was rebase-clean so PM re-merged via the gate after full verification. |
| `PM-NOTE-2026-06-01-toolchain-bump-cargo-cve-followup.md` | historical | Rust 1.96.0 (stable) doesn't affect our pinned `nightly-2026-04-16` build. Carry-forward: next `rust-toolchain.toml` bump should land a cargo with the 1.96 fixes for CVE-2026-5223 / CVE-2026-5222. No change now. |
| `PM-NOTE-2026-06-01-quality-gate-audit.md` | historical | PM audit: no broken code shipped, but host tests + clippy/fmt run in no automated gate (orphaned test crates; CI runs only `neo-test`). Fix dispatched. Standing follow-on: kernel ~282 clippy-warning burndown. |
| `archive/PHASE-4-7/DISPATCH-2026-06-01-ci-host-test-lint-gates-prometheus.md` | closed (merged) | → Prometheus: logic-test crates made workspace members + CI `host-checks` job (cargo test --workspace + fmt --all --check + clippy; whole non-kernel workspace blocking, kernel clippy non-blocking). PM-merged with two integration fixes: dropped a premature `ai-ops-tests` member (Jarvis adds it in BUILD-3) that broke `cargo metadata`, and closed **all** non-kernel fmt+clippy debt in-container (56 files reformatted; neomc/neofs-mkfs/xtask/libneo/neo-worker clippy `-D warnings`-clean) so the gate lands fully green. Also added `clippy` to the toolchain pin + Dockerfile (root cause of the clippy gap). Standing follow-on: kernel ~282 clippy-warning burndown → then flip kernel clippy to blocking. |

### Phase 5 BUILD-2 — model object / `.neom` (2026-05-31, parallel tracks)

| File | Status | What it is |
|------|--------|------------|
| `docs/ADR-028-neom-v1-format-and-model-object.md` | accepted | The `.neom` v1 byte format (header/tensor/node/io tables, page+64B weight alignment, opcode set), load-time validation rules (Article 21.2), the kernel `Model` object design, and the §D8 cascade-revocation disposition for Layer 1 (snapshot load → models hold no live file handle → ESTALE on staleness, no global unmount rewrite). Single contract gating both BUILD-2 tracks. |
| `archive/PHASE-4-7/DISPATCH-2026-05-31-phase-5-build-2-kernel-model-object-jarvis.md` | closed (merged) | Track 2-kernel → Jarvis: `.neom` parse/validate, immutable weight tensors, `Handle::Model`, syscalls 0x110–0x112, §D8 staleness. OBSERVE map + ISC-1…ISC-12. |
| `archive/PHASE-4-7/DISPATCH-2026-05-31-phase-5-build-2-neom-tool-prometheus.md` | closed (merged) | Track 2-tool → Prometheus: host-side `.neom` v1 authoring tool + self-validator + valid/negative test fixtures. ISC-1…ISC-7. |
| `CLOSE-REPORT-2026-05-31-phase-5-build-2-kernel-model-object-jarvis.md` | closed | Kernel track close: `Model` object + `.neom` parse/validate, ISC-1…12 PASS. PM-verified in-container — kernel `cargo check` clean (compiles, 0 errors); `model_load_cascade` @ SMP=4 (loads a neomc fixture; ESTALE + unmount-not-blocked proven); QEMU + full `neo-test all` green per close report. ISC-12 clippy NOT verified clean here — kernel carries pre-existing clippy/warning debt (not BUILD-2-specific); canonical clippy deferred to CI. |
| `CLOSE-REPORT-2026-05-31-phase-5-build-2-neom-tool-prometheus.md` | closed | Tool track close: `tools/neomc` authoring tool + fixtures. PM-verified in-container — 26 tests pass, clippy clean, fixtures generate (1 valid + 8 negative, each rejected for the right reason). |
| `PM-NOTE-2026-05-31-phase-5-build-3-layer-2-recon.md` | historical | BUILD-3 (Layer 2 CPU inference) pre-design recon: hook surfaces, hard constraints (★ FPU/XMM not saved across context switches), staged Q1–Q8 for ADR-029. |
| `PM-NOTE-2026-05-31-phase-5-build-3-layer-2-recon.md` | historical | Pre-design recon for BUILD-3 (Layer 2, CPU inference backend): OBSERVE map of the surfaces Layer 2 hooks, hard constraints (★ FPU/XMM state not saved across context switches), and staged design questions Q1–Q8 for ADR-029 (written after Layer 1 merges). No design locked in. |

### Phase 5 opening — AI subsystem foundation (2026-05-31)

| File | Status | What it is |
|------|--------|------------|
| `PM-DECISION-2026-05-31-phase-5-opening.md` | closed | Phase 5 opening: goal milestone, scope/anti-scope, three-layer build sequence (tensor → model → CPU inference), and the **BUILD-1 PLAN** (tensor primitive, ISC-1…ISC-11 + anti-criteria). Phase 5 completed 2026-06-04. |
| `docs/ADR-027-phase-5-ai-foundation-design-substrate.md` | accepted | Phase 5 design substrate: tensor object (Article 20), dtype/device enums, tensor memory class (7.5), capability integration (12.1/20.2), syscall range (v1.10.0), native-format-only, governance-hooks-only, and the deferred ANSPEC-002 §16.3 cascade-revocation disposition. |
| `docs/bible.md` (v1.10.0) | amended | Article 6.1 AI syscall reservation narrowed 0x100..0x1FF → 0x100..0x17F; net formally owns 0x180.. (Phase 4 ABI). ABI-coherence amendment opening Phase 5. |
| `archive/PHASE-4-7/DISPATCH-2026-05-31-phase-5-build-1-tensor-primitive-jarvis.md` | closed (merged `37075a3`) | PM→Jarvis worker build order for BUILD-1: objective, locked decisions (syscalls 0x100–0x103, buddy-frame storage, drop required, immutability-enforcement deferred), the verified OBSERVE integration map, and the ISC acceptance gate. |
| `CLOSE-REPORT-2026-05-31-phase-5-build-1-tensor-primitive-jarvis.md` | closed | Jarvis BUILD-1 close: tensor primitive landed (`kernel/src/ai/`), capability-gated + refcounted + buddy-frame storage, syscalls 0x100–0x103, host logic tests + `tensor_create_query_drop` QEMU scenario PASS at SMP=4, full `neo-test all` green. PM-reviewed and merged to `main`; Phase 5 marked Active (Layer 0 done; Layers 1–2 remain). |

### Phase 4 close / governance (2026-05-30)

| File | Status | What it is |
|------|--------|------------|
| `docs/phases/PHASE-4.md` | closed | Phase 4 retrospective / formal close (normative). Per-build record (BUILD-1…BUILD-5, 92/92 ISCs), native ABI `0x180`–`0x18E`, course corrections (2 blockers), forward-marker ledger (discharged vs carried), 3 bare-metal deferrals, deviations. Phase 4 ships over virtio-net. |
| `PL-DECISION-2026-05-30-usb-ethernet-descope.md` | closed | Article 24.1 PL re-deferral: USB stack + USB Ethernet (ADR-017 §D3), bundled into Phase 4 at opening but not built across BUILD-1–5, moved to the Phase 7.5 connectivity lane. Phase 4 closes on virtio-net. Recorded in Bible v1.9.0 ledger. |
| `docs/bible.md` (v1.9.0) | amended | Article 13 ledger: Phase 4 marked Complete (QEMU-verified; bare-metal network validation deferred per ADR-009); USB deliverables re-deferred to Phase 7.5. v1.8.0 → v1.9.0. |

### Phase 4 BUILD-5 (closed 2026-05-30 — Phase 4 SHIP)

BUILD-5 (DNS + Happy Eyeballs + kTLS) closed — **Phase 4 ships.** Sub-commits
`aab4923` (5a) … `85d7a3d` (5e); 18/18 ISCs; full `neo-test all` NEO_SMP=4
green at ATTEMPTS=1; `phase4_https_fetch` passes (resolve → HE → TLS 1.3 →
decrypted HTTP body). Cardinal rule held: control plane hand-rolled, crypto
delegated (kernel `chacha20poly1305` AEAD; userspace `rustls`/`rustls-rustcrypto`
restricted to TLS13_CHACHA20_POLY1305_SHA256). Discharges FM-3-1. Lands to main
in one ff (`23f75a3`…`85d7a3d`).

| File | Status | What it is |
|------|--------|------------|
| `CLOSE-REPORT-2026-05-30-phase-4-build-5.md` | closed | PM disposition: BUILD-5 ACCEPTED on all 18 ISCs; **discharges Phase 4**. Two-pass (5a–5c crypto-boundary source audit + 5d/5e QEMU close). PM-verified no hand-rolled crypto (chacha20poly1305 crate + rustls-rustcrypto single-suite), WRITE-gated `enable_tls`, copy-then-zeroize (kernel + user buffer), synchronous-per-packet record path, fail-closed parsing. FM-3-1 discharged; FM-5-1…5-4 carried. |
| `archive/PHASE-4-7/DISPATCH-2026-05-30-phase-4-build-5-dns-he-ktls-jarvis.md` | closed | Jarvis dispatch. Delivered via `codex/phase-4-build-5-dns-he-ktls-jarvis` (`85d7a3d`). Five sub-commits, 18 ISCs, pause-for-PM-after-5c honored. |
| `PM-DECISION-2026-05-30-phase-4-build-5-opening.md` | closed | Opened BUILD-5, the Phase 4 ship. Q1 hand-rolled RFC 1035 stub + `ResolverConfig` (RDNSS + DHCP DNS); Q2 UDP/53 + EDNS(0) + TTL cache; Q3 HEv2 + RFC 6724 prefer-IPv6 dest (closes FM-3-1); Q4 hybrid kTLS; Q5 vetted AEAD (chacha20poly1305), no hand-rolled crypto; Q6 TLS 1.3 server-auth only, rustls + bundled root; Q7 in-harness DNS + loopback TLS for CI. Ratified and delivered. |
| `PLAN-2026-05-30-phase-4-build-5-dns-he-ktls.md` | closed | Per-sub-commit contract for 5a–5e (18 ISCs). Delivered. |
| `PL-RATIFICATION-2026-05-30-phase-4-build-5-pm-decision.md` | closed | PL ratification of all seven §9 points (Q1–Q7) + the 5a–5e split. |

### Phase 4 BUILD-4.5 (closed 2026-05-30)

BUILD-4.5 (IPv6 Neighbor Discovery / NDP) closed; lands with the BUILD-4 stack
in one fast-forward to main (`ea241ac`…`49cd238`). Sub-commits `6b94e24` (4.5a)
+ `49cd238` (4.5b); 8/8 ISCs; full `neo-test all` NEO_SMP=4 green at ATTEMPTS=1.
Discharges FM-4-1 (IPv6 NDP gap). On-link resolution now covers both address
families across UDP + TCP.

| File | Status | What it is |
|------|--------|------------|
| `CLOSE-REPORT-2026-05-30-phase-4-build-4.5.md` | closed | PM disposition: BUILD-4.5 ACCEPTED on all 8 ISCs. PM-verified the Q4 placeholder removal (`ipv6_destination_mac` gone; UDP+TCP route through `ndp::submit_or_queue_ipv6`), the NS→NA→cache→resolved-egress serial proof, and the green NEO_SMP=4 sweep. FM-4-1 discharged; FM-4.5-1 (real DAD) + FM-4.5-2 (Router Solicitation) carried. |
| `archive/PHASE-4-7/RETURN-2026-05-30-phase-4-build-4.5-jarvis.md` | closed | Jarvis BUILD-4.5 return: NDP cache/NS/NA, shared IPv6 UDP/TCP resolve+queue path, QEMU serial proof for NS out → NA in → cache populated → resolved UDP/TCP frame egress, and confirmation the TCP IPv6 placeholder is removed. |
| `PM-DECISION-2026-05-30-phase-4-build-4.5-opening.md` | closed | Opened BUILD-4.5 (IPv6 NDP) to close FM-4-1 ahead of BUILD-5. IPv6 ARP-parity build: hand-rolled `net/ndp.rs` mirroring `arp.rs`, smoltcp phy-only. Q2 minimal neighbor cache; Q3 BUILD-2 DAD unchanged; Q4 shared resolve/queue for IPv6 UDP+TCP egress; Q5 no Router Solicitation. Ratified and delivered. |
| `PLAN-2026-05-30-phase-4-build-4.5-ndp.md` | closed | Per-sub-commit spec: 4.5a `net/ndp.rs` (NS/NA codec + neighbor cache + solicited-node multicast), 4.5b IPv6 egress integration (replaced `tcp.rs ipv6_destination_mac` placeholder) + wire scenario + sweep. Delivered. |
| `archive/PHASE-4-7/DISPATCH-2026-05-30-phase-4-build-4.5-ndp-jarvis.md` | closed | Jarvis implementation dispatch. Delivered via `codex/phase-4-build-4.5-ndp-jarvis` (`49cd238`). |
| `PL-RATIFICATION-2026-05-30-phase-4-build-4.5-pm-decision.md` | closed | PL ratification of all five BUILD-4.5 §10 open points (Q1–Q5) as recommended. |
| `BUILD-4.5b-BARE-METAL-DEFERRAL-2026-05-30.md` | tracked | Bare-metal deferral for live host/NIC IPv6 NDP, real DAD (FM-4.5-1), Router Solicitation (FM-4.5-2), and full RFC 4861 NUD/Redirect/SEND scope. Not blocking BUILD-4.5 close. |

### Phase 4 BUILD-4 (closed 2026-05-30)

BUILD-4 (hand-rolled TCP + CUBIC + RFC 5961 + RFC 7323) closed; merged to main.
Sub-commit chain `438cae2`…`fb690c9`; 28/28 ISCs; full `neo-test all`
NEO_SMP=4 green at ATTEMPTS=1. Transport layer (UDP + TCP) complete.

| File | Status | What it is |
|------|--------|------------|
| `CLOSE-REPORT-2026-05-30-phase-4-build-4.md` | closed | PM disposition: BUILD-4 ACCEPTED on all 28 ISCs (two-pass review: 4a/4b foundation checkpoint + full close). Constraints honored: hand-roll, type fork (UDP `Socket` untouched), no smoltcp iface, syscalls 0x188–0x18B, panic-clean. FM-1 confirmed (reused `install_cap` lineage, no extension). Forward-markers: FM-4-1 IPv6 NDP neighbor-resolution gap (placeholder dest MAC; real IPv6 peers need NDP), FM-4-2 egress drain test shim. |
| `archive/PHASE-4-7/RETURN-2026-05-30-phase-4-build-4-jarvis.md` | closed | Jarvis BUILD-4 return. Seven sub-commits `438cae2`…`fb690c9`. 28/28 ISCs PASS. Hand-rolled TCP, type fork, no smoltcp iface, FM-1 needed no cap-subsystem extension. |
| `BUILD-4g-BARE-METAL-DEFERRAL-2026-05-30.md` | tracked | Bare-metal deferral: real remote-peer TCP handshake/data/close, real RTT, IPv6 NDP neighbor resolution. Not blocking. |
| `PM-DECISION-2026-05-30-phase-4-build-4-opening.md` | closed | Opened BUILD-4. PL-decided hand-roll + full scope; Q2 type fork; FM-1/2/3 discharge; CUBIC-only; seven sub-commits 4a–4g with 28 ISCs. Ratified and delivered. |
| `PLAN-2026-05-30-phase-4-build-4-tcp.md` | closed | Per-sub-commit spec for BUILD-4a–4g. Delivered. |
| `archive/PHASE-4-7/DISPATCH-2026-05-30-phase-4-build-4-tcp-jarvis.md` | closed | Jarvis implementation dispatch. Delivered via `codex/phase-4-build-4-tcp-jarvis` (`fb690c9`), merged to main. |
| `PL-RATIFICATION-2026-05-30-phase-4-build-4-pm-decision.md` | closed | PL ratification of BUILD-4 strategy, scope, and all six §10 open points. |

### Phase 4 BUILD-3 (closed 2026-05-30)

BUILD-3 (IPv4 + ARP + DHCPv4 + dual-stack RFC 6724) closed and merged to main
`ef6be5b`. Sub-commit chain `4a40e43`…`5d1edcd`; 21/21 ISCs; full
`neo-test all` NEO_SMP=4 green at ATTEMPTS=1.

| File | Status | What it is |
|------|--------|------------|
| `CLOSE-REPORT-2026-05-30-phase-4-build-3.md` | closed | PM disposition: BUILD-3 ACCEPTED on all 21 ISCs (reviewed against `5d1edcd`; 3a–3d re-verified by source read, 3e–3f on the QEMU sweep). Constraints honored: no smoltcp iface, no syscall ABI change, per-family tables preserved, restructuring under the §4.5 bound, panic/unwrap clean. Forward-markers: FM-3-1 family-directed RFC 6724 (prefer-IPv6 dest default → BUILD-5), FM-3-2 zero UDP checksum (RFC 768 IPv4-valid), FM-3-3 `net::tick` harness-driven not yet ISR-wired. |
| `archive/PHASE-4-7/RETURN-2026-05-30-phase-4-build-3-jarvis.md` | closed | Jarvis BUILD-3 return. Six sub-commits `4a40e43`…`5d1edcd`. 21/21 ISCs PASS. Hand-rolled ARP/DHCPv4, no smoltcp iface adoption, per-family tables preserved, zero-checksum IPv4 UDP egress. |
| `BUILD-3f-BARE-METAL-DEFERRAL-2026-05-30.md` | tracked | Bare-metal deferral: real DHCPv4 server lease, real ARP peer exchange, real IPv4 UDP reply reception, on-wire RFC 5227 conflict/probe/defend. Mirrors BUILD-2e. Not blocking. |
| `PM-DECISION-2026-05-30-phase-4-build-3-opening.md` | closed | Opened BUILD-3. Five load-bearing decisions Q1–Q5; §4.5 bounded restructuring; six sub-commits 3a–3f with ISCs. Ratified and delivered. |
| `PLAN-2026-05-30-phase-4-build-3-ipv4-dhcpv4-dual-stack.md` | closed | Per-sub-commit spec for BUILD-3a–3f. Delivered. |
| `archive/PHASE-4-7/DISPATCH-2026-05-30-phase-4-build-3-ipv4-dhcpv4-jarvis.md` | closed | Jarvis implementation dispatch. Delivered via `codex/phase-4-build-3-ipv4-dhcpv4-jarvis` (`5d1edcd`), merged to main. |
| `PL-RATIFICATION-2026-05-30-phase-4-build-3-pm-decision.md` | closed | PL ratification of all five BUILD-3 PM-DECISION §10 open points (Q1–Q5 + bounded restructuring) as recommended. |
| `PL-RATIFICATION-2026-05-30-phase-4-build-2-send-destination-abi-close.md` | closed | PL ratification of the BUILD-2 sendto/recvfrom ABI (0x186/0x187 + 0x182 connected-send) as-is. Closes BUILD-2 carried-forward open item (a). Next free socket syscall: 0x189. |

### Phase 4 opening (2026-05-28)

| File | Status | What it is |
|------|--------|------------|
| `PM-DECISION-2026-05-28-phase-4-opening.md` | closed | Opens Phase 4 (network stack). Phase 4 completed 2026-05-30 (BUILD-1…BUILD-5, 92/92 ISCs). USB Ethernet re-deferred to Phase 7.5 per `PL-DECISION-2026-05-30-usb-ethernet-descope.md`. |

### Phase 4 THINK v0.2 (ratified 2026-05-22)

| File | Status | What it is |
|------|--------|------------|
| `PHASE-4-THINK-v0.2-2026-05-22-network-stack-design-direction.md` | ratified | Phase 4 THINK v0.2. Supersedes v0.1 per Article 24.2. Ratified 2026-05-22 per `PL-RATIFICATION-2026-05-22-phase-4-think-v0.2-q1-q12.md` (Q1–Q12 plus pre-ratified Q-A, Q-B). Absorbs Cerebro pressure-test findings: 5 REFINABLE refinements, 1 CONSIDER-REVISITING resolved via BUILD-1.1, 6 new risks R11–R16. PLAN drafting gated on Phase 3 close. |

### Phase 4 BUILD-1 PLAN opening (2026-05-28)

| File | Status | What it is |
|------|--------|------------|
| `PM-DECISION-2026-05-28-phase-4-build-1-opening.md` | closed | Phase 4 BUILD-1 PLAN. Anchored on ratified THINK v0.2. Seven sub-commits (1a substrate, 1b IPv6 core, 1c UDP+cap, 1d loopback+stats, 1.1 cap+rotation, 1f extension-header posture, 1e bare-metal). Phase 4 completed 2026-05-30. |
| `PM-NOTE-2026-05-28-phase-4-build-1-speculative-tcp-shape-sketch.md` | closed | Companion to the PM-DECISION. Discharges THINK §4.1 v0.2 / R16 mitigation: layered-handle shape vs TCP RFC 9293 state machine. Outcome: layered shape composes with TCP; three forward markers for BUILD-3 PLAN. |
| `PL-RATIFICATION-2026-05-28-phase-4-build-1-pm-decision.md` | closed | PL ratification of the BUILD-1 PM-DECISION in chat 2026-05-28. Silent-ratification of all four §10 open points. Unblocks BUILD-1a dispatches. |
| `PL-RATIFICATION-2026-05-28-anspec-002-v0.1.1-amendment.md` | closed | PL ratification of ANSPEC-002 v0.1.1 amendment (§17, §18 additions). |
| `archive/PHASE-4-7/DISPATCH-2026-05-28-phase-4-build-1a-pre-implementation-research-cerebro.md` | closed | Cerebro pre-implementation research: smoltcp v0.13.0 surface, Hubris `task/net` wrap pattern, virtio-net publication-fence verification. **CRITICAL clause NOT triggered** — virtio-blk publication discipline verified clean. |
| `RESEARCH-2026-05-28-phase-4-build-1a-smoltcp-and-virtio-net-cerebro.md` | draft | Cerebro BUILD-1a research return. Recommends smoltcp 0.13.1, ~200-line wrap gate validated, virtio-blk audit CLEAN. Opens the Jarvis BUILD-1a gate. |
| `archive/PHASE-4-7/DISPATCH-2026-05-28-phase-4-build-1a-rust-virtio-net-prior-art-prometheus.md` | closed | Prometheus ecosystem prior-art survey. Closed 2026-05-28 per RESEARCH and PM-DISPOSITION. |
| `RESEARCH-2026-05-28-phase-4-build-1a-rust-virtio-net-ecosystem-prior-art-prometheus.md` | draft | Prometheus BUILD-1a ecosystem survey. Headline: do NOT depend on external `virtio-drivers` crate; extend virtio-blk's HAL/Transport/VirtQueue patterns. |
| `PM-DISPOSITION-2026-05-28-phase-4-build-1a-prometheus-consumption.md` | closed | PM consumption of Prometheus's BUILD-1a research. Dispositions OQ-1 through OQ-4. Six Jarvis directives. Two forward markers (FM-VirtIO-1, FM-VirtIO-2). |
| `archive/PHASE-4-7/DISPATCH-2026-05-28-phase-4-build-1a-netdevice-virtio-net-smoltcp-wrap-jarvis.md` | gated | Jarvis BUILD-1a implementation: `NetDevice` trait, virtio-net driver, smoltcp wrap shim, ISC-1a-3 frame round-trip smoke test. Five ISCs. Publication-fence discipline inherited from BUILD-5b `2fdb7cb`. **GATED on BOTH `RESEARCH-...-cerebro.md` AND `PM-DISPOSITION-...-prometheus-consumption.md` on `main`.** |
| `PM-NOTE-2026-05-28-cve-2026-31431-copy-fail-relevance-to-phase-4.md` | historical | Analysis artifact on the AF_ALG "Copy Fail" CVE (announced 2026-04-29). Confirms NeoPlutarch structurally not exposed. Confirmatory data point for THINK v0.2 R12; strengthens BUILD-5 PLAN-time case for synchronous-per-packet kTLS design. |
| `PM-NOTE-2026-05-28-cifswitch-relevance-to-phase-4.md` | historical | Analysis artifact on the CIFSwitch Linux LPE (heyitsas.im, published 2026-05-27). Confirms NeoPlutarch structurally not exposed on three independent grounds. General pattern lesson lands at BUILD-1c and BUILD-5. Companion to the Copy Fail PM-NOTE. |
| `archive/PHASE-4-7/DISPATCH-2026-05-28-archive-phase-1-3-dispatches-prometheus.md` | closed | Prometheus archive dispatch: moved 593 Phase 1–3 closed artifacts to `Plans/archive/`. Completed per cleanup audit 2026-05-28. |
| `PM-DISPOSITION-2026-05-28-cleanup-audit-clarify-decisions.md` | closed | PM disposition of cleanup audit CLARIFY items. |

### End-of-day handoff

| File | Status | What it is |
|------|--------|------------|
| `HANDOFF-2026-05-28-end-of-day-phase3-build5b-ahci-green-nvme-blocked-jarvis-dispatched.md` | historical | End-of-day handoff: AHCI lane green, NVMe lane blocked on log-sink DMA bug, Jarvis dispatched. |

## Maintenance

Last refreshed: **2026-07-02** (post-CB-6 Rule-4 consolidation pass; reconciled current cadence, CB-6 closure, Phase 8/9 Rule-4 closure, and the active 2026-07-02 Rule-4 dispatch).

This index gets updated when:

- A new dispatch lands in `Plans/` (add an `active` entry).
- A dispatch's commit lands on `main` (flip status to `closed`, add commit hash).
- A dispatch is superseded by a follow-up (flip status to `superseded`).
- A project commitment is made, met, or re-deferred (update the
  "Open Project Commitments" section).

The index is not a dependency for any other artifact. If it falls out of
date, that's an inconvenience, not a correctness issue.
