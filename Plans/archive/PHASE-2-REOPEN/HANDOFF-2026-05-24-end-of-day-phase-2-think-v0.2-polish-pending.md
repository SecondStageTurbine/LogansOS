# HANDOFF: End-of-day 2026-05-24, Phase 2 THINK v0.2 polish pending

- **Author:** PM (Claude/Cerebro role)
- **Date:** 2026-05-24
- **Repo HEAD:** `8bdb4be` on `origin/main` (https://github.com/SecondStageTurbine/NeoPlutarch)
- **Purpose:** Self-contained handoff for a successor AI continuing the NeoPlutarch PM role. Read end-to-end; no other context required.

---

## 1. Project basics

- **Project:** NeoPlutarch — a Rust microkernel-adjacent OS built from scratch (single-node, multi-arch goal, AI-first eventual posture).
- **Repository:** `github.com/SecondStageTurbine/NeoPlutarch`. Workspace at `E:\AI Automation\AltimitNeo\` on the PL's Windows machine.
- **PL (Project Lead):** SecondStageTurbine. Owns scope, bible amendments, ratifications, hardware sessions, external worker routing.
- **PM role:** What the successor inherits. PM dispatches and triages; PM does NOT execute worker-tier work. Operational + technical decisions belong to PM; scope-class / bible-amendment / major-pivot calls belong to PL.

## 2. Worker roster

- **Cerebro** = Claude. Lane: deep internet research, light coding, OBSERVE/THINK-substance development. The successor IS Cerebro for any work done in-session.
- **Jarvis** = Codex (CLI). Lane: kernel source, heavy coding work. Dispatched via external routing (PL pastes the tell-Jarvis text into Codex).
- **Prometheus** = Qwen3.6-27B. Lane: documenting, amending, document cleanup, trial for small coding. Dispatched externally same as Jarvis.

Worker dispatch sequence: PM authors dispatch file → PM commits → PL routes the tell-[worker] text externally → worker runs → worker files deliverable + commits → PL pastes the worker's return summary in chat → PM consumes via PM-DISPOSITION.

## 3. Operational discipline (binding)

- **Bible Article 14 cadence** for any phase work: OBSERVE → THINK → PL ratification → PLAN → /simplify gate → BUILD/VERIFY. The project is currently mid-cadence on Phase 2 PLAN (see §5).
- **Don't over-surface micro-decisions.** Make the call, document reasoning in the artifact, ferry on. PL signs off on ratifications, scope, bible amendments, major architectural pivots — not on (a)/(b)/(c) operational choices.
- **PM doesn't execute worker-tier inventory or audit work.** Day-1 of the pre-Phase-2 audit was a PM-tier execution slip that PL corrected mid-session ("you are just the PM"). Inventory/evidence-classification/failure-mode analysis is dispatch-class.
- **Every dispatch ends with a tell-[worker] text in chat** so PL knows what to route. This is the last thing PM does before considering a dispatch complete.
- **Commits land directly on `main`.** No PR workflow except for code-bearing changes (PR #1, PR #6 were the exceptions). Docs-only commits skip CI via the `paths-ignore` filter from commit `a434992`.
- **Status 1 from `git push` is cosmetic stderr decoration**, not an error. Standard pattern when push succeeds.

## 4. Infrastructure context

- **RAG semantic code search** at `http://127.0.0.1:8404`. REST API: `/search`, `/search/full`, `/health`, `/stats`. 7,500+ points indexed across the project (code + Plans + docs). Use aggressively for design-context discovery in dispatches and PM work. Documented in `E:\AI Automation\AltimitNeo\CLAUDE.md` §"NeoPlutarch RAG". Requires Ollama backend (`E:\AI Automation\RAG\start-rag-ollama.ps1`) running separately; if `/health` times out, Ollama isn't loaded.
- **CI/CD:** GitHub Actions; full QEMU integration suite at `NEO_SMP=4` as standard gate. `paths-ignore` skips runs for `Plans/**`, `docs/**`, `**.md`, `LICENSE*`, `.gitignore` changes.
- **Bare-metal target:** Dell OptiPlex 7080 (Intel, ≥4 cores). Currently used for boot smoke only — multi-CPU verification is the open Article 13 close-gate.
- **Serial console:** Adapter (Dell 6CC5P internal cable + DTECH FTDI USB-RS232 + Arnorin null modem F-F, $55.81) arrives 2026-05-25 — slipped one day from yesterday's expected. Needed for NVMe FLUSH durability diagnosis (BUILD-5b context, paused).

## 5. Current cadence position

**Phase 2 PLAN, mid-THINK.** The project has been "Phase 2 substantially complete since 2026-05-11" (BUILD-1 capability foundation, BUILD-2 buddy+slab allocators, BUILD-3 SMP retrofit Q1-Q8) but never formally closed. Phase 2 PLAN is producing the formal-close BUILD set plus defense-in-depth items.

| Sub-step | Status | Artifact |
|---|---|---|
| OBSERVE | ✅ Closed | `Plans/PM-DISPOSITION-2026-05-24-phase-2-observe-close.md` (`255372a`) |
| Scope-class ratifications | ✅ Ratified | `Plans/PL-RATIFICATION-2026-05-24-phase-2-observe-scope-class-items.md` (`ee5055e`+`7040a49`) |
| THINK v0.1 (question batch) | ✅ Ratified | `Plans/PHASE-2-THINK-v0.1-2026-05-24.md` (`46426d9`) + `Plans/PL-RATIFICATION-2026-05-24-phase-2-think-v0.1.md` (`c5017bf`) |
| THINK v0.2 (substance) | ✅ Accepted by PM | `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-think-v0.2.md` (`eac6737`) + `Plans/PM-DISPOSITION-2026-05-24-phase-2-think-v0.2-substance-close.md` (`8bdb4be`) |
| **THINK v0.2 polish** | **🟡 Dispatched, awaiting external routing** | `Plans/DISPATCH-2026-05-24-phase-2-think-v0.2-polish-prometheus.md` (`8bdb4be`) |
| THINK v0.2 polished ratification | ⏳ Future | Awaiting polish landing |
| PLAN | ⏳ Future | Awaiting THINK v0.2 ratification |

Doc-reconciliation workstream (PHASE-2.md status header, ADR-019 status, ADR-013 Q8.a supersession note) closed in parallel at `a796303` + `dbef8d9`; accepted by PM at `40688de`.

## 6. The seven THINK v0.2 ratified dispositions (load-bearing for PLAN)

PM accepted Cerebro's recommendations across the board. Two specific PM rulings:

- **Q5(a) re-routing concede:** INV-IPC-5 pairs with ADR-019 sub-phase C (Channel.state surface); INV-SYNC-5 pairs with Q7/A2 (ready-queue surface — A2 IS its structural fix). My initial OBSERVE-era read grouped both with ADR-019 B/C, which was wrong.
- **Q7 ring concur:** Bible Article 4.2 confines `unsafe` to hardware boundaries; intrusive linked list would have spent unsafe budget on scheduler data-structure plumbing. Fixed-capacity per-CPU ring of `Box<Thread>` (default 64/CPU, configurable, `-EAGAIN` at limit) achieves every ADR-016 constraint structurally in safe Rust for ~1 KB/CPU.

Summary of all seven:

1. **Q1 — `sys_close`: DEFER.** No Phase-2 use case in project; genuine driver is Phase 4 sockets / Phase 7 multi-tenant inference. Ship `sys_wait` + `sys_inspect`; CLOSE bit stays reserved.
2. **Q2 — `sys_wait`: blocking + WNOHANG**, first-consumer-wins (others get `-ESRCH`), observes D8 `exit_state` CAS externally, **BOUNDS** FM-D14 (full retirement via handle-presence-gated slot lifetime is a possible follow-on ADR — §9.6, not Phase-2-close blocker).
3. **Q3 — `sys_inspect`: ship thin.** Scalar snapshot only (run-state, exit_code, thread_count, handle_count, name) — no CapId/Handle fields → structural anti-leak (Bible 12.2). Useful in Phase 2 via spawn-minted INSPECT.
4. **Q4 — Perf harness: TIERED.** QEMU-TCG deterministic structural proxies gate per-commit; bare-metal OptiPlex characterizes absolute latency periodically; VMware-VM reproducible middle tier; ADR-019 50ns bench folded in. Three-tier regression policy. **PL operational ratification pending on (a) reference environment.**
5. **Q5 — ADR-019 B before C; pair C with INV-IPC-5; route INV-SYNC-5 to Q7/A2.** Gate sub-phase C at SC-3B 200/200 `NEO_SMP=4` standard; sub-phase B at lighter (full-suite-green + ADR-019 micro-bench).
6. **Q6 — Bare-metal multi-CPU protocol: SC-3B/3C/4 SMP scenarios must pass on ≥4 cores + full count.** Evidence vocabulary mirrors `PROCEDURE-2026-05-11-vmware-hypervisor-vm-verification.md` PASS criteria + per-AP GS-base sentinels. OptiPlex 7080 required; AMD cross-vendor non-blocking stretch. **PL operational ratification pending on (d) hardware target + (e) executor.**
7. **Q7 — ADR-016 A2 implementation: fixed-capacity per-CPU ring of `Box<Thread>`** (NOT intrusive linked list). Default 64/CPU, configurable at scheduler init, `-EAGAIN` at limit. INV-SYNC-5 becomes A2's acceptance test. **Sequencing: A2 before ADR-019 B before ADR-019 C.**

**§8 synthesis collapses to TWO BUILD subphases** (input to PLAN, not a PLAN decision yet):
- **Subphase 1 — Capability reserved-bit activation:** sys_wait + sys_inspect; absorbs audit (b) INV-CAP-11 (`cap_revoke_spawn_inheritance_descendants`).
- **Subphase 2 — Formal close + defense-in-depth:** A2 + INV-SYNC-5; ADR-019 B; ADR-019 C + INV-IPC-5; perf harness; bare-metal multi-CPU verification + remaining audit (b) F-004 (`smp_concurrent_reap_after_terminate`). Q4 + Q6 share a single hardware session.

## 7. Immediate pending actions

### 7.1 Two PL operational ratifications (small, fast)

PL needs to ratify (or amend) two operational items before the Prometheus polish ships with them as "ratified":

- **Q4(a) tiered perf harness shape.** PM rec: ratify as Cerebro proposed. Only design honoring Bible 16.4's "from Phase 2 onward" mandate AND the operational reality that bare-metal can't run per-commit and TCG absolute numbers are fiction.
- **Q6(d)/(e) bare-metal hardware target + executor.** PM rec: ratify as Cerebro proposed — OptiPlex 7080 required, AMD cross-vendor non-blocking stretch, PL executes a PM-authored procedure document modeled on `PROCEDURE-2026-05-11-vmware-hypervisor-vm-verification.md`.

Fast path: "yes to both" → Prometheus polish proceeds with both as ratified rather than PL-pending in the polished output.

### 7.2 Prometheus polish dispatch routing

Dispatch filed at `Plans/DISPATCH-2026-05-24-phase-2-think-v0.2-polish-prometheus.md` (commit `8bdb4be`). Tell-Prometheus text was provided in the previous chat turn — re-extract from there or from §10 of this handoff. Awaiting PL external routing to Prometheus.

### 7.3 NVMe FLUSH durability diagnosis (paused)

`Plans/NOTES-2026-05-23-nvme-driver-flush-and-completion-review.md` (commit `46a0848`) contains the leading hypothesis (Intel E6100p NVMe doesn't honor FLUSH for durability past the 10M-spin timeout ≈ 3ms). Three remediation candidates: (1) FUA bit on NVM_WRITE, (2) disable VWC via SET_FEATURES 0x06, (3) increase COMMAND_TIMEOUT_SPINS. Serial adapter arrives 2026-05-25 — when installed, walk through with PL: 6CC5P internal cable to KB_MS_SERIAL header, null modem connection, PuTTY/screen at 38400 8N1, boot AHCI scenario, capture serial. Resume diagnosis from that evidence.

## 8. Outstanding ADR/bible items recorded but deferred

- **ADR-016 A2:** Phase 2 conformance work per `PL-RATIFICATION-2026-05-24-phase-2-observe-scope-class-items §1.4` (supersedes HANDOFF-2026-05-11 §10.2's "Phase 3" reading). Implementation shape ratified in THINK v0.2 Q7. Lands in Subphase 2 first.
- **ADR-019 B/C:** Sub-phase A landed at `27d1b99`; B/C queued for Phase 2 close per THINK v0.2 Q5. Sub-phase B before C.
- **ADR-007 Implementation note 6:** PID recycling + stale-handle revocation. PL ratified NOT in Phase 2 scope (`PL-RATIFICATION § 1.3`); revisit when pid exhaustion becomes real workload concern (foreseeable lifetime essentially never at u64 monotonic).
- **Q2(e) full FM-D14 retirement** (handle-presence-gated `COMPLETED_EXIT_CODES` slot lifetime): possible follow-on ADR after Phase 2 close if residual-growth measurement surfaces a real concern. Phase 2 close ships the practical bound (`sys_wait` first-consumer-wins).
- **Q11 (Spectre-v2 / IBPB on context switch):** Ratified deferral to dedicated future-triggered ADR per `PL-RATIFICATION § 1.5`. Triggers: CPU-vulnerability event, broader CPU-vuln mitigation ADR, or measurement showing BTI surface is workload concern.
- **Three Cerebro-deferred-as-measurement-gated items:** CapId allocation under SMP (single `AtomicU64` fetch_add — per-CPU banks if contention measured), buddy SMP contention (single `Mutex<Option<BuddyAllocator>>` — per-order locking if slab fall-through becomes hot), DerivationEntry storage (heap `BTreeMap` — slab-back if fragmentation surfaces). All wait for Q4 perf harness evidence.

## 9. Audit chain (closed)

Pre-Phase-2 audit fully closed. Five PASS findings, four PARTIAL→(b) BUILD-N test items absorbing into Phase 2 BUILD subphases, two PARTIAL→(d) ADR-tracked deferrals, four DEFECT closed (F-001/F-002 in PR #6 `4dfb13f`; F-003/F-005 in `9a282f2`). Zero open blockers since `5884ffd`.

Audit (b) items to fold into Phase 2 BUILD subphases:
- `smp_concurrent_reap_after_terminate` (F-004) → Subphase 2 (formal-close verification)
- `ipc_rendezvous_translate_fault` (INV-IPC-5) → Subphase 2 with ADR-019 C
- `cap_revoke_spawn_inheritance_descendants` (INV-CAP-11) → Subphase 1 (cap reserved-bit activation)
- `sched_ready_queue_capacity_pressure` (INV-SYNC-5) → Subphase 2 with A2 (acceptance test)

## 10. Key artifacts and commit hashes (today's session)

Today's commits, oldest to newest:

```
321d50f plans: file close-report and handoff for CI caching + cap_survive_exit dispatch
321a5d7 plans: file NOTES placeholder for 180s cap_survive_exit flake post PR #6
46a0848 plans: file NVMe driver flush+completion review notes
a434992 ci: add path filtering, Docker Buildx cache, cargo cache (PR #1)
4dfb13f sched/ipc: fix cap_survive_exit cross-process zombify race (PR #6)
ff19f2c plans: dispatch Cerebro to complete pre-Phase-2 audit Day 2 work
bb1b679 plans: file Cerebro Day-2 audit findings
199fb51 plans: close pre-Phase-2 audit (PM-DISPOSITION) + dispatch Jarvis to fix F-005
9a282f2 fix ipc forced-kill parked waiter ownership (F-005 + F-003)
69df60b document f005 forced-kill closeout
5884ffd plans: file F-005 close PM-DISPOSITION - Phase 2 PLAN unblocked
df5bfb5 plans: dispatch Cerebro for Phase 2 OBSERVE
97972e1 plans: file Cerebro Phase 2 OBSERVE
255372a plans: file Phase 2 OBSERVE close PM-DISPOSITION + Prometheus doc-reconciliation dispatch
ee5055e plans: ratify Phase 2 OBSERVE scope items + draft THINK v0.1 + dispatch Prometheus polish
7040a49 plans: ratify Q11 deferral (Spectre-v2/IBPB)
46426d9 docs: Phase 2 THINK v0.1 polish (Prometheus)
4b2f80c docs: close report hash update for Phase 2 THINK v0.1 polish
96d0b46 plans: file Phase 2 THINK v0.1 polish close PM-DISPOSITION
c5017bf plans: ratify Phase 2 THINK v0.1 batch + dispatch Cerebro for THINK v0.2 substance
a796303 docs: Phase 2 doc reconciliation (Prometheus)
dbef8d9 docs: close report hash update for Phase 2 doc reconciliation
40688de plans: file Phase 2 doc reconciliation close PM-DISPOSITION
eac6737 plans: file Cerebro Phase 2 THINK v0.2 substance
8bdb4be plans: accept Cerebro Phase 2 THINK v0.2 substance + dispatch Prometheus polish ← current HEAD
```

Load-bearing paths for the successor:

- **Substance:** `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-think-v0.2.md` (`eac6737`) — the seven recommended dispositions in full.
- **PM rulings:** `Plans/PM-DISPOSITION-2026-05-24-phase-2-think-v0.2-substance-close.md` (`8bdb4be`) — PM concurrence, Q5 concede, Q7 concur, §9 dispositions.
- **Polish dispatch (in flight):** `Plans/DISPATCH-2026-05-24-phase-2-think-v0.2-polish-prometheus.md` (`8bdb4be`).
- **Scope binding:** `Plans/PL-RATIFICATION-2026-05-24-phase-2-observe-scope-class-items.md` — the four scope ratifications that constrain answer space (close-first, baseline-satisfies-TLB, no-PID-recycling, A2-is-Phase-2-conformance) + Q11 deferral.

Tell-Prometheus text for the polish dispatch (extract from the previous chat turn or reconstruct from the dispatch's §5 Notes-for-Prometheus):

> Prometheus — Phase 2 THINK v0.2 polish dispatch. Repo at `8bdb4be` on main. Spec: `Plans/DISPATCH-2026-05-24-phase-2-think-v0.2-polish-prometheus.md`. Inputs: `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-think-v0.2.md` (Cerebro substance) + `Plans/PM-DISPOSITION-2026-05-24-phase-2-think-v0.2-substance-close.md` (PM rulings). Output: `Plans/PHASE-2-THINK-v0.2-2026-05-24.md`, modeled on `Plans/PHASE-4-THINK-v0.2-2026-05-21.md`. Polish promotes "Recommended disposition" → "Disposition" (PM ratified); preserve reasoning + alternatives-considered (load-bearing for PL on Q5 + Q7); strip Cerebro worker-tier discipline notes + density notes + PM-tracking trailers. Surface Q4(a) and Q6(d)/(e) compactly as PL-operational. Expect ~80-120 lines polished. Shape-level deviations from explicit instruction go through NOTES-first per the v0.1 polish process correction (`Plans/PM-DISPOSITION-2026-05-24-phase-2-think-v0.1-polish-close.md` §2). Close report at `Plans/CLOSE-REPORT-2026-05-24-phase-2-think-v0.2-polish.md`. CI's `paths-ignore` skips docs-only.

## 11. Successor's first three moves

1. **Pull `origin/main` to local;** verify HEAD `8bdb4be`. Check `git status` for any unexpected local state.
2. **Check Prometheus polish dispatch status:**
   - If still pending external routing: re-extract tell-Prometheus from §10 above and confirm PL hasn't routed yet.
   - If polish has landed (look for `Plans/PHASE-2-THINK-v0.2-2026-05-24.md` and `Plans/CLOSE-REPORT-2026-05-24-phase-2-think-v0.2-polish.md`): consume the close report, verify acceptance criteria C1-C8, file `Plans/PM-DISPOSITION-2026-05-24-phase-2-think-v0.2-polish-close.md`, brief PL for ratification of the polished batch.
3. **Track PL ratification of Q4(a) and Q6(d)/(e):**
   - If PL has answered: incorporate into PM-DISPOSITION of polish landing; the polished doc's PL-pending items become ratified.
   - If PL hasn't answered: surface again in the polish-acceptance brief; both items have PM recommendations attached (§7.1).

After those: PL ratifies polished THINK v0.2 → PM enters PLAN sub-step. PLAN sub-step translates the seven answered questions into the two BUILD subphases per §8 synthesis: BUILD-subphase enumeration with goal milestones, dependencies, anti-scope, /simplify gates. Cerebro is the appropriate dispatch lane for PLAN substance development; Prometheus polishes the PLAN doc once substance is locked.

## 12. Constraints to not trip on

- **Bible Article 4.2** (unsafe at hardware boundaries) is decisive for Q7 ring vs intrusive list. Future scheduler/data-structure work should similarly avoid unsafe outside hardware boundaries; that's the project's unsafe-budget discipline.
- **Bible Article 8.5.3** (no heap allocation under scheduler lock) is what INV-SYNC-5 verifies and what A2 structurally enforces. The fixed ring is a *type-property* solution, not a discipline solution.
- **Bible 1.2 priority order: Correctness > Coherence > Performance.** Cerebro flagged this in Q4 reasoning — performance work without measurement evidence violates it. Hence the three measurement-gated deferrals.
- **Workspace-on-Windows quirks:** PowerShell is the default shell. UTF-8 BOM matters for commit message files (use `[System.IO.File]::WriteAllText` with explicit `UTF8Encoding $false`). `git push` returning status 1 is cosmetic stderr.
- **CLAUDE.md at `E:\AI Automation\AltimitNeo\CLAUDE.md`** is the operational reference document that Claude Code in WSL auto-reads. Successor should read it early — particularly §"NeoPlutarch RAG" for the RAG service discipline.
- **`ai_digest.py` in worktree** is a persistent untracked file PL leaves; not noise to worry about.

## 13. Context for the successor on PM judgment style

This session's PM (Claude) made the following calls worth carrying forward as style guidance:

- **Concede when worker is right.** Q5 concede was important; Cerebro's surface-matching was structurally cleaner than PM's geographic grouping. Future-PM should not be sticky on prior calls when worker reasoning is sharper.
- **Concur explicitly when worker is right** (Q7 ring) — don't just rubber-stamp; record the reasoning so the artifact trail captures why the choice was made.
- **Surface PL-tier items with PM recommendations** rather than asking PL to think from scratch. PL is "yes/no/amend" against PM's recommendation, not "design this for me."
- **Honest framing matters.** Cerebro's Q2(e) framing ("BOUNDS, not fully retires" FM-D14) is the kind of intellectual honesty that should propagate; don't oversell deliverables.
- **Process corrections recorded for forward propagation.** Two examples this session: (1) Day-1 PM-tier audit slip corrected to dispatch-class on Day-2; (2) Prometheus shape-deviation on v0.1 Q11 polish recorded as NOTES-first for future polish dispatches. Carry both forward.
- **Memory entries** persist across sessions. Three entries currently active (visible in userMemories): NeoPlutarch agent roster, PM operational posture, RAG service availability.

---

**End of handoff. Successor has everything needed to pick up at the Prometheus polish dispatch's external routing or return, whichever comes next.**
