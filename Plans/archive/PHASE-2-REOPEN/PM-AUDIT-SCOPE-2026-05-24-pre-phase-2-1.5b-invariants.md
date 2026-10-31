# PM-AUDIT-SCOPE: pre-Phase-2 audit of Phase 1.5b IPC, scheduler, and capability invariants

- Author: PM (Cerebro/Claude)
- Date: 2026-05-24
- Status: Ratified scope. Binding for the audit deliverable.
- PL ratification: 2026-05-24, this session ("lets go with your recommendations"),
  covering: (a) seed framing using `close_peer` cross-process zombify as the
  example that justifies the audit, (b) the 5-step method specified in §4,
  (c) the four-way disposition split specified in §6.
- Bible amendment: none required. Audit is a Phase 2 OBSERVE activity per
  Bible Article 14's algorithm; no new article needed.
- ADR: none required for the scope itself. Individual audit findings may
  produce ADRs at disposition time per §6.
- Scope class: PM-tier execution, bound by this PL-ratified scope.
- Replaces: the dangling memory reference to "pre-Phase-2 audit; deviations
  #10 + #7 + #8" from PL's working notes (2026-05-03), which could not be
  resolved to specific artifacts in the repo. This scope re-derives against
  current state rather than recovering the historical numbering.

---

## 1. Purpose

Validate that Phase 2's capability-system work can be built on Phase 1.5b's
IPC + scheduler + preliminary-capability foundation without inheriting
latent defects.

The audit produces a single artifact — a per-invariant findings list with
explicit disposition for each — that lets Phase 2 PLAN start with the
foundation's correctness boundary characterized rather than assumed.

The audit is not a re-run of Phase 1.5b's BUILD-6 e2e leak audit. It is a
meta-audit: what did 1.5b's audit method check, what did its vocabulary
let through, and what should we look at again with a different lens before
Phase 2 commits.

## 2. Seed and justification

On 2026-05-24, PR #6 (`4dfb13f`) shipped a fix for a cross-process zombify
race in `Channel::close_peer`: the `extracted_my` branch unconditionally
zombified any waiter parked on the closing process's side, without
checking whether the waiter's pid matched the exiting process. When init
received a granted capability into worker's side of a data channel and
parked there on `sys_send`, worker's exit cleanup would zombify init
itself — init's `sys_exit` never ran, and the kernel fell back to exit
code 255.

This defect lived undetected through Phase 1.5b's BUILD-6 e2e leak audit
and through 1.5b's phase-close verification. It was caught only when the
GHA disk-exhaustion masking issue was removed and CI could actually
surface the failure.

The defect is a small bug with a localized fix. The relevant question for
this audit is not "is there a `close_peer` bug" (resolved) but "what
shape of bug is this, and what other 1.5b invariants have failure modes
of the same shape that the existing audit vocabulary did not detect?"

Concretely, the defect's shape:

- Multi-CPU race (SMP-only).
- Cross-process resource manipulation (one process's exit acted on
  another's parked thread).
- Capability-handle-mediated (the grant put init's thread in a location
  that worker's exit-cleanup operated on; without the grant, the race
  would not exist).
- Invisible to leak-counting audits (no frame leaked, no channel leaked
  — the wrong process was simply terminated).

Any 1.5b invariant claim that depends on per-process locality of
exit-side actions, or that assumes single-CPU sequencing, or that is
verified only by leak counts rather than by-pid identity, is suspect
under the same lens.

## 3. What 1.5b's e2e leak audit checked, and what it missed

Phase 1.5b closed at commit `e9675c4` with BUILD-6's e2e leak audit as
its load-bearing verification. Its method, summarized:

- Run each unhappy-path kill test to completion.
- After teardown, assert that frame counts, channel counts, and process
  counts return to their pre-test baselines.
- A failure is a leak: more frames allocated than freed, more channels
  open than expected, a process still in the table.

This method is correct as far as it goes. It catches resource leaks
exhaustively. Its coverage shape is bounded by what it counts.

What it does not catch:

- **By-pid identity violations.** A test where the wrong process exits
  passes the leak audit as long as some process exits and counts
  rebalance. The `close_peer` bug is exactly this: init exited (so the
  process count rebalanced), but init's exit was caused by worker's
  cleanup rather than by init's own `sys_exit`. Audit blind to the
  caller.
- **Exit-code semantics.** Kernel fallback exit code 255 vs userspace
  `sys_exit(0)` vs userspace panic all produce the same count
  rebalancing. Audit blind to how the process exited.
- **Cross-CPU ordering.** All 1.5b's BUILD-6 tests appear to have run
  at SMP=1 or with a single user-thread footprint. Multi-CPU race
  windows did not exist in the test harness's execution shape.
- **Capability-mediated state changes.** The audit counts capabilities
  but does not assert what *each* capability authorizes. A capability
  that should only let process A close one side could in principle let
  it close the other; the leak audit does not check authorization
  geometry.

This list is not exhaustive. It is what's visible from the close_peer
seed. The audit method below is designed to surface other failure shapes
that share the same blind-spot vocabulary.

## 4. Method

### 4.1 Step 1 — Invariant inventory

Enumerate every IPC, scheduler, or preliminary-capability invariant
claimed anywhere in the project's normative or near-normative text.
"Invariant" means a claim of the form "the kernel guarantees X" or "X
always holds" or "X never occurs" — anything that future code is
expected to preserve.

Capture each claim verbatim, with source citation (file:line and commit
hash where applicable). No paraphrase at this step — paraphrase loses
the conditions that make the claim true.

Sources to scan are enumerated in §5.

### 4.2 Step 2 — Evidence classification

For each invariant in the inventory, classify the evidence currently
supporting it into one of three buckets:

- **PROVEN.** Formal proof, exhaustive test, or by-construction
  argument that rules out the failure mode for all reachable program
  states. Examples: type-system enforcement, exhaustive state-machine
  enumeration, mechanically-checked proofs. Phase 1.5b has very few of
  these; this is expected — they are rare in handwritten Rust kernels.
- **TESTED.** A positive test exists that exercises the invariant's
  claim, with a specific failure mode the test would catch. The test
  may have coverage gaps but it is not vacuous. Most of 1.5b's BUILD-N
  test suite produces TESTED-class evidence.
- **CLAIMED.** No specific test for this invariant exists. The claim
  is in the bible, ANSPEC, an ADR, or a commit message, and is upheld
  only by code review at the time of writing. The `close_peer`
  invariant (whatever its written form was) was CLAIMED.

Disagreements between sources (e.g. bible says X, code does Y) are
recorded as findings at step 4 regardless of classification.

### 4.3 Step 3 — Failure-mode analysis

For each CLAIMED and each partial-TESTED invariant (a TESTED invariant
where the test's vocabulary plausibly misses failure shapes), enumerate
the failure modes that the existing evidence does not detect.

Use the `close_peer` seed as the worked example for the analysis
discipline:

- Invariant (reconstructed): "An exiting process's channel-side
  closure cleanup acts only on resources owned by the exiting process."
- Evidence pre-PR-#6: CLAIMED. No SMP-aware cross-process-zombify test.
  Adjacent TESTED evidence (`q8c_close_peer_my_side_extract`) covered
  single-process zombify discipline only.
- Failure modes not detected by adjacent tests: (i) cross-process
  zombify via a grant that places a foreign thread on the exiting
  process's side; (ii) cross-CPU race where the exiting cleanup runs
  concurrent with the foreign thread's park; (iii) wakeup-with-EPIPE
  fallback never tested when the waiter is foreign.

For each invariant, the deliverable from this step is a bullet list of
concrete, code-grounded failure modes. Not abstract risks.

### 4.4 Step 4 — Findings

For each invariant: assign one of three finding labels.

- **PASS.** Evidence is PROVEN, or evidence is TESTED with no failure
  modes from §4.3 that remain undetected. Nothing to do.
- **PARTIAL.** Evidence is TESTED with at least one failure mode from
  §4.3 that the existing tests do not exercise. A new test is needed
  before Phase 2 commits. Finding records the gap and the recommended
  test shape.
- **DEFECT.** Code does not uphold the invariant. Finding records the
  defect (file:line, reproduction shape), the fix shape, and whether
  the fix is blocking for Phase 2.

A PARTIAL or DEFECT finding does not automatically block Phase 2 — it
gets a disposition per §4.5.

### 4.5 Step 5 — Disposition split

Each PARTIAL and DEFECT finding gets exactly one of four dispositions:

- **(a) Immediate kernel fix.** A small, localized correction with low
  risk of regression. Filed as a Codex/Jarvis dispatch with the audit
  finding as the rationale. Closes when CI passes.
- **(b) BUILD-N test work.** For PARTIAL findings where the fix is "add
  a test that would have caught this." Folded into the next available
  BUILD subphase (likely 1.5c BUILD-N+1 or a new BUILD-N inserted before
  Phase 2 PLAN).
- **(c) Bible amendment for a ratified deviation.** For findings where
  the invariant as written is wrong (e.g. too strong; never actually
  held) and the right disposition is to weaken or qualify the claim
  rather than fix code. PL-tier sign-off.
- **(d) Phase 2 ADR carrying a documented known-issue.** For findings
  where the defect is real but the fix is out of scope for the
  audit's window (cost too high, requires Phase 2 design context to
  fix correctly, or is genuinely Phase 2 work being foreshadowed). The
  ADR records the issue, the rationale for deferral, and the trigger
  for revisiting.

The audit deliverable lists every finding's disposition explicitly. No
finding ends in limbo.

## 5. Sources to scan for invariant claims

The Step 1 inventory draws from, in order of priority:

1. **`docs/bible.md` (TheAltimitNeoBible)** — Articles 4 (Kernel
   Architecture), 5 (Userspace Architecture), 6 (AI/ML Subsystem only
   insofar as it touches capability discipline), 7 (Memory Model), 8
   (Concurrency and Scheduling), 12 (Security Model). Article 6.4
   already names accelerator capability geometry; reuse the same lens
   for IPC + scheduler capabilities.
2. **`altimitneo-ai-subsystem-spec.md` (ANSPEC-001)** — §3.7
   (capabilities and handles), §10 (security), §10.2 (capability
   discipline). The AI capability surface and the IPC capability
   surface share a discipline; claims in either constrain the other.
3. **ADRs in `docs/adr/`** — enumerate at start of execution. Any ADR
   touching scheduler, IPC, processes, threads, locks, capabilities,
   or fault recovery is in scope. ADR-022 (cell-swap forward marker,
   per recent memory) is explicitly in scope.
4. **Phase 1.5b phase document** (referenced in handoff `e34c844`) —
   for the phase's own claim ledger and BUILD-N goal milestones.
5. **Phase 1.5b BUILD-N close commits** — commit-message claims of the
   form "guarantees", "ensures", "invariant", "never". These often
   capture invariants that did not make it into the bible or ADRs.
6. **`kernel/src/ipc/`, `kernel/src/sched/`, `kernel/src/cap/`
   (if present)** — module-level doc comments and function-level doc
   comments. Doc-comment claims are weaker than bible/ADR claims but
   they are claims and they bind future code.

Sources scanned are recorded in the audit deliverable so future readers
can verify completeness.

## 6. Disposition taxonomy detail

This expands §4.5 with concrete rules for which disposition fits which
finding shape.

| Finding type | Disposition default | Override condition |
|---|---|---|
| DEFECT, localized, no design questions | (a) immediate fix | If the fix touches the Phase 2 capability ABI, escalate to (d) |
| DEFECT, design questions, no clear fix | (d) Phase 2 ADR | If urgency demands acting before Phase 2 PLAN, escalate to PL |
| PARTIAL, missing positive test | (b) BUILD-N test work | If the test reveals a DEFECT, the finding upgrades and re-disposes |
| PARTIAL, invariant overstated in source | (c) bible amendment | If the overstatement is in an ADR, supersede the ADR instead |
| PASS with caveats | (none — note in audit) | If the caveat is a real gap, downgrade to PARTIAL |

Each disposition has a follow-up artifact:

- (a) → Codex/Jarvis dispatch in `Plans/DISPATCH-*` referencing the audit
  finding by ID.
- (b) → BUILD subphase planning item folded into the next available
  PLAN doc. If no PLAN is in flight, a tracking item in `Plans/NOTES-*`.
- (c) → PL-RATIFICATION request for the proposed amendment. Standard
  amendment PR workflow.
- (d) → ADR draft in `docs/adr/` with status "proposed" until Phase 2
  PLAN consumes it.

## 7. Deliverables

This audit produces one primary deliverable and zero or more secondary
deliverables.

**Primary:** `Plans/PM-AUDIT-2026-05-24-pre-phase-2-1.5b-invariants.md`
(filename mirrors this scope doc with the `-SCOPE` segment removed).
Contents:

- Frontmatter referencing this scope doc as the binding method.
- Sources scanned (per §5), with commit hashes for any source that has
  changed since the audit started.
- Inventory table: invariant ID, source citation, verbatim claim text.
- Evidence column per invariant: PROVEN / TESTED / CLAIMED, with the
  specific test name where TESTED.
- Failure-mode analysis per CLAIMED and partial-TESTED entry.
- Findings: PASS / PARTIAL / DEFECT per invariant.
- Disposition per PARTIAL and DEFECT, with the follow-up artifact path
  (per §6) named explicitly.

**Secondary:** the follow-up artifacts named in the dispositions.
Filed as the audit completes; not required to be filed atomically with
the primary deliverable.

## 8. Acceptance criteria

The audit is complete when:

- **C1.** Every source listed in §5 has been scanned (or marked
  unavailable with rationale).
- **C2.** Every invariant in the inventory has an evidence
  classification (§4.2).
- **C3.** Every CLAIMED and partial-TESTED invariant has at least one
  concrete failure-mode bullet from §4.3 — or an explicit note that
  no plausible failure mode was identified after analysis.
- **C4.** Every invariant has a finding label (PASS / PARTIAL / DEFECT).
- **C5.** Every PARTIAL and DEFECT finding has a disposition (§4.5)
  and a named follow-up artifact path.
- **C6.** The primary deliverable is filed at the path in §7 and
  pushed to `origin/main`.
- **C7.** A PM-DISPOSITION artifact is filed summarizing the audit's
  net findings and listing the blocking items (if any) for Phase 2
  PLAN.

The audit is *not* complete merely when the inventory is built. The
disposition column is load-bearing.

## 9. Out of scope

The following are explicitly excluded from this audit; if they surface
during execution, file them as separate concerns rather than expanding
this audit's scope:

- **Phase 2 capability-system design.** This audit looks at what 1.5b
  built; Phase 2 design is what Phase 2 PLAN does on top of the
  audit's output.
- **Memory-manager invariants** beyond what touches IPC/scheduler.
  Buddy allocator and slab allocator are Phase 2 work; their
  invariants get their own audit (or none) in Phase 2 OBSERVE.
- **Driver invariants** including the NVMe FLUSH durability hypothesis
  from `NOTES-2026-05-23-nvme-driver-flush-and-completion-review.md`.
  Driver durability is a Phase 3 concern; audit it there.
- **AI subsystem invariants from ANSPEC-001 §3.7/§10** *except*
  insofar as they constrain the general capability discipline. The
  AI-specific capability work happens at Phase 5+ and gets audited
  then.
- **The 180s `cap_survive_exit` flake** from
  `NOTES-2026-05-24-cap-survive-exit-180s-flake-post-ipc-fix.md`. It
  may surface a finding if the audit identifies a related invariant,
  but the flake itself is monitored separately and is not the audit's
  target.
- **License, naming, governance, project-management invariants.** The
  audit is technical.

## 10. Sequencing

The audit is a desk-research activity and does not compete with
hardware time. It runs in parallel with:

- BUILD-5b re-execution once the serial adapter arrives (2026-05-24,
  later today). PM pauses audit work to walk PL through serial
  install and first capture; resumes after.
- The NVMe driver review NOTES (`NOTES-2026-05-23-...`) and any fix
  dispatch that follows from the serial-capture diagnosis. Those are
  Phase 3 work; audit is pre-Phase-2 work; both are pre-Phase-2 work
  from the project's perspective and can interleave as PM-load
  permits.

Estimated PM effort: 1–2 days of focused inventory + analysis time,
plus an open-ended tail for disposition follow-up artifacts. The
primary deliverable should land within 3 calendar days of this scope
doc, conservatively.

Phase 2 PLAN does not start until the audit's primary deliverable is
filed and any (a)-disposition immediate fixes have landed. (b) test
work and (d) Phase-2-ADR deferrals do not block Phase 2 PLAN start;
they are absorbed into Phase 2's work.

## 11. References

- `Plans/CLOSE-REPORT-2026-05-24-ci-caching-and-cap-survive-exit.md` —
  the `close_peer` seed event.
- `Plans/HANDOFF-2026-05-24-ci-caching-and-scheduler-fix.md` — the
  preceding wrong-bug diagnostic chain (historical record).
- `Plans/NOTES-2026-05-24-cap-survive-exit-180s-flake-post-ipc-fix.md` —
  the post-fix flake monitored separately.
- Commit `4dfb13f` (PR #6) — the `close_peer` fix.
- Commit `e9675c4` — Phase 1.5b close.
- Commit `e34c844` — Phase 1.5b → 1.5c handoff.
- Bible Articles 4, 5, 6, 7, 8, 12 (per §5).
- ANSPEC-001 §3.7, §10 (per §5).
- Memory roster update 2026-05-10 (PM operational posture).
