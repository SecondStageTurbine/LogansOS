# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Seats and roles — who is who (read this first)

There is **one PL** (JC — the human lead), **one PM** (Claude), and **eight CLI coders** across three classes (five main-tier + one open-weight + two DeepSeek-family: an implementation seat and a diagnosis seat). They are distinct seats; do not conflate them. *(Oracle added 2026-06-22, main-tier; Chad added 2026-07-10, main-tier; Genesis = GLM-5.2 off-boarded 2026-06-22, its in-flight dispatch (E1b) reassigned to Janus.)*

- **JC — the PL.** The human project lead. Sets scope and ratifies load-bearing decisions; runs every coder's CLI in the worker tree and relays the PM's dispatches to them; runs the bare-metal hardware. The PM surfaces genuinely mutually-exclusive / irreversible decisions to the PL — never "keep going vs pause" (the answer is always keep going).
- **Claude — the PM seat.** Runs in a **cloud container** (the PL's Claude desktop), a *separate clone* of the repo; it is **not** a Claude Code CLI worker. The PM authors the governance/handoff artifacts (dispatches, PLANs, THINKs, ADRs, ANSPECs, the Bible/ledger, README/INDEX/CLAUDE.md), pushes them straight to `origin/main`, and **verifies worker returns (verify-before-merge) + merges feature branches (`--ff-only`/cherry-pick) to `main`**. It reaches the coders only by the PL **relaying** a dispatch. *(If you are the model reading this in the cloud container: you are Claude, the PM. You are **not** Cerebro.)*
- **The main-tier CLI coders** — the PL runs each in the worker tree (they span **two distinct model families** — see *Cross-model diversity* below; the label is by tier, not by model):
  - **Cerebro** — a main coder; builds the OS. On par with Jarvis.
  - **Jarvis** — a main coder; builds the OS. On par with Cerebro.
  - **Janus** — a main-tier coder (spun up 2026-06-14).
  - **Oracle** — a main-tier coder (added 2026-06-22); builds the OS. On par with Cerebro/Jarvis/Janus.
  - **Chad** — a main-tier coder (added 2026-07-10); a **frontier model on the Codex CLI** (same setup as Jarvis/Janus; branches `codex/…-chad`); builds the OS. On par with Cerebro/Jarvis/Janus/Oracle — runs the full kernel + QEMU loop (not scope-limited like Prometheus).
- **The OpenCode CLI coders (open-weight models)** — the PL runs each through the **OpenCode** CLI; the PM scores/verifies their diffs the same verify-before-merge way:
  - **Prometheus = Qwen-27B.** Originally the research / doc-cleanup / light-coding seat; the 2026-06-19 open-weight A/B proved it **main-tier on un-gameable cells** (2× BINDING PASS — incl. the OT-24 per-CPU-guard fix that **shipped to `main` as `3724ad1`**). Free (local). Still the strongest research seat. **SCOPE (PL decision 2026-06-22): Prometheus is scoped to research/design THINKs, prior-art/trap-mining, doc & consolidation support, and *bounded HOST-only builds* (host KATs/oracles/tooling — push-CI only). Kernel-touching / QEMU-gated builds route to the main-tier CLI coders (Cerebro/Jarvis/Janus/Oracle) — do NOT dispatch Prometheus a build whose gate needs the 4-shard QEMU `workflow_dispatch`. Rationale: reliable design + host-cell record, but recurring *close-gate under-runs* on gated builds (M2 3× xtask-wiring + false infra-attribution → reassigned; L1 skipped `fmt`, reported green w/o URL while push-CI was red; F3 stale base + cited only push-CI, no QEMU gate). The verify-before-merge funnel caught all three, but each cost a bounce — the code was usually fine; the kernel-touching *release ritual* (rebase-freshness, the mandatory QEMU gate, honest CI-URL citation, failure attribution) is the unreliable axis. Scope to strengths; the cheapest capable research seat is best spent there.**
  - **Genesis = GLM-5.2 — OFF-BOARDED 2026-06-22.** Historical A/B record only; its E1b spawn-wiring dispatch was reassigned to Janus (main-tier).
- **The DeepSeek-family seats** — a distinct model family from Claude/Codex/Qwen, so **either one satisfies the cross-family requirement against all three**:
  - **Dexter — an implementation seat** (added ~2026-07). Takes bounded builds and fixes. Note the practical constraint that has already bitten once: **Dexter runs against a weekly usage limit** and can be exhausted mid-lane (it happened on C1.1, 2026-08-01, forcing a handoff to Janus). Do not put a *time-critical* lane on Dexter without a named fallback seat.
  - **Sage — folded into Dexter (PL, 2026-10-30).** Same DeepSeek model and `dsh` profile as Dexter, so it isn't a separate seat any more. A "Sage-shaped" task is now a **diagnosis job sent to Dexter**: one hard question, one reasoned answer, always a hypothesis plus a named falsification test. The original rules below still describe that kind of job.
  - *(Historical)* **Sage — the DIAGNOSIS seat (added 2026-08-04). A different SHAPE from every other coder, and dispatching it wrong wastes it.** Sage is a one-shot depth seat: paste it the relevant files and ask the hard question — *why does this deadlock under load?*, *design the migration strategy for X*, *what did everyone miss in this 800-line diff?* — and it returns **one deeply-reasoned answer** worth waiting minutes for. **It does not turn the crank.** File edits, test runs, gate iteration, and bounce cycles go to a fast implementation seat afterwards.
    - **Dispatch Sage:** diagnosis of an unexplained failure; a migration/refactor *design* whose implementation is large but mechanical; an adversarial read of a big diff; a design cross-check where a fourth model family is the point.
    - **Do NOT dispatch Sage:** anything whose close-gate is a 4-shard QEMU run, anything needing bounce-fix-regate iteration, or anything where the deliverable is a merged branch. **Its deliverable is an answer, and the answer routes to another seat.**
    - **Close condition for a Sage dispatch is always a hypothesis PLUS a named falsification test**, never a bare verdict. A seat that produces one confident answer and never iterates has no self-correction loop — the standing risk is *confident and wrong*, which is the most expensive failure mode this project has. If Sage cannot name what would prove its answer wrong, that is itself the finding and must be reported as such.


The PM never types on the CLI and never is "Cerebro"; **Cerebro is a coder.** The "PM seat" throughout this file means **Claude, the cloud PM** — historical text that read "the PM seat (Cerebro)" was a mislabel and has been corrected.

**Six coders is the staffed roster (five main-tier — Cerebro/Jarvis/Janus/Oracle/Chad — plus Prometheus in its scoped lane), not the steady-state parallelism.** The throughput bound is NOT the coder count — it is (a) the number of **independent, non-conflicting work lanes** open at once, (b) the **PM verify-before-merge funnel** (every branch costs a `pm-merge-check` + own-`git show --stat` inspection + independent CI-run verification — the PM is the serializing point, by design), and (c) **CI runner capacity** (only ~3 self-hosted runners; the 4-shard QEMU gate is already contended — the exit-137/runner-loss events came from piling concurrent heavy jobs onto them). **With five main-tier coders now able to open kernel/QEMU lanes, (c) is even more the *binding* constraint — five concurrent kernel-touching gates cannot fit ~3 runners (the 4th runner box is not up yet).** So: staff all main-tier coders when that many clean lanes exist, but **keep at most ~2–3 kernel-touching QEMU gates in flight at once** (prefer a mix — e.g. some host-only/userland lanes, or a consolidation), expect real concurrency to be gated by the merge funnel + CI, and **stagger kernel-touching gates** so they don't thrash the runners. With more main-tier coders than runner slots, the surplus seats are best pointed at **no-gate lanes** (host-only builds, design/research THINKs) until a kernel window opens. Don't manufacture make-work to keep a seat busy — an idle seat is cheaper than a contended gate or a rushed review.

### Cross-model diversity for convergence-signal work (PL standing rule, 2026-07-10)

The seats span **three distinct AI model families**, and that diversity is a **tool**, not incidental trivia:
- **Claude family:** the **PM** (Claude), **Cerebro**, **Oracle**.
- **Codex frontier family:** **Jarvis**, **Janus**, **Chad**.
- **Qwen (open-weight):** **Prometheus**.
- **DeepSeek family:** **Dexter** (implementation), **Sage** (diagnosis). *(Added to this list 2026-08-04; Dexter had been seated since ~2026-07 without being recorded here — a roster gap worth not repeating, since the whole rule keys off family.)*

**Consequence to apply, not just note:** the seats now span **four** families, so a convergence or adversarial exercise has four independent lenses available rather than three. **Sage and Dexter are the SAME family** and therefore do not cross-check each other — but **either satisfies the cross-family requirement against Claude, Codex, or Qwen work**, which is exactly the gap the C1.1 precedent exposed.

**Whenever the VALUE of an exercise is independent convergence or divergence as a signal** — a judge panel (two-independent-visions → keep the strongest, graft the rest), an independent design review meant to *corroborate* a decision, an adversarial-verification panel meant to catch what one perspective misses — **the independent agents MUST come from DIFFERENT model families.** Two *same-model* agents that "independently" converge is **weak** evidence: they share training priors and blind spots, so agreement can be shared-echo rather than truth (and shared divergence hides a blind spot they both have). **Cross-model** convergence is the strong signal; cross-model divergence surfaces the blind spot. So pair one **Claude-family** seat (Cerebro/Oracle) with one **Codex-family** seat (Jarvis/Janus/Chad) and/or **Prometheus** (Qwen) — never two of the same family for a convergence exercise.

- **Done right:** the GUI vision panel — Cerebro (Claude) + Jarvis (Codex). Their convergence on the hard core is *real cross-model evidence*.
- **The trap to avoid (it bit us once):** the OT-50/OT-51 config-seam convergence was Oracle + Cerebro — **both Claude**. That convergence is same-model consistency, **not** cross-model corroboration; weight it accordingly, and get a different-family check before leaning on it as evidence.

This also strengthens the PM's **adversarial-audit gate**: where a refute-lens can be routed to a *different-model coder* instead of a same-model PM sub-agent, prefer it — a Codex-family or Qwen lens catches what a Claude PM structurally won't.

### Cross-family review routing: which model, what effort, who reviews what (PL standing rules, 2026-10-21)

**Every cross-family review relay starts with a `MODEL:` / `EFFORT:` line and one sentence of why.** Pick the
cheapest model that can do the work:

| Review kind | Model | Effort |
|---|---|---|
| Core SMP / timer / memory code (new or reworked) | GPT-6.1-Sol | high |
| Narrow re-check of the reviewer's own earlier finding | GPT-6.1-Sol | medium |
| Contained code (one subsystem, no SMP core, no new syscall) | GPT-6-Luna | high |
| Light test-only or docs review | GPT-5.6-Luna | medium |
| Any of the above refused by the model | **Daybreak Blue** (fallback) | same |
| A lane that has bounced **2+ rounds** on 6.1-Sol | GPT-6-Astra | high (medium for a narrow re-check) |

**Bundles run on one model (PL standing rule, 2026-10-27).** When several reviews go to one coder in a single paste (a
review bundle, `Plans/DISPATCH-<date>-review-bundle-<coder>.md`), the whole bundle runs on **GPT-6.1-Sol, high**, so
the PL doesn't switch models mid-bundle. A **single** review relay still names the cheapest fitting model from the
table above.

**Write reviews as plain checklists.** Each item says what to check; the answer is pass/fail plus the
enforcing `file:line` (and a concrete input for any fail). Don't frame a review offensively or defensively
("attack", "refute", "harden against", "exploit"). Just say what to check. That wording is what got
Codex refusals, even on Daybreak Blue.

**Security reviews of Claude-authored code go to Muse (Gwen), a separate model family.** The Codex models refuse
these (disk-write guard, panic latch) even on Daybreak Blue. Muse reads the files at the head SHA from the repo,
not a pasted summary. It can't push, so the PL pastes or uploads the RETURN and the PM banks it under `Plans/`.
Precedent: PR #85 and #88 (write guard), both PASS.

**Claude cyber-allowance sessions (e.g. R1) are SECOND reviewers only.** They add findings (R1 found six on #85),
but they're Claude-family. They never stand in for the cross-family review of Claude-authored code.

**Seat note.** Muse (Gwen) is a fifth model family, available to the PL with repo read access. Treat it as
cross-family against Claude, Codex, Qwen and DeepSeek.

### Bare-metal test target: the Zenbook is a work laptop, and Windows is never written (PL standing rule, 2026-10-21)

Bare-metal runs now use the PL's **ASUS Zenbook S14 UX5406AA**. It has no serial port, so **photos of the screen are
allowed evidence on this target** (the fb progress console exists for exactly this). This overrides the
Arthur/OptiPlex "no photos" note for Zenbook runs only. The runbook is `docs/runbook-baremetal-zenbook-s14.md`.
- **The kernel must NEVER write the Zenbook's internal (Windows) NVMe.** Every block write goes through the write
  guard (`GuardedDevice`, PR #85/#88). An image whose bundle writes a disk sink (B/C) may boot there only with the
  guard on. The guard-control feature can't be built with bare-metal, product or fb-progress-console.
- The **Kingston DataTraveler stick is now the image stick**. It's no longer the Ubuntu control stick: it was
  overwritten on 2026-10-21, and older docs that call it "the Ubuntu stick" are stale.

### Merge holds the PL reserves

Some lanes carry **"NO MERGE to `main` without J.c.'s go-ahead"** in the PL's request (2026-10-21: OT-61 C0.5
#86 and wall-clock OT-85 #87). For those, the PM delivers the branch and gate report. It then sends the PL **one**
message: pass/fail per gate item, the branch, and the single decision needed. The PM doesn't merge until the PL says so.

**The default is the opposite: the PM merges (PL standing rule, 2026-10-24).** A branch that clears the merge bar
(gate green on the exact SHA, PM verify-before-merge, no regression) merges without the PL's sign-off. A hold exists
only when **J.c. himself** states one for a specific PR or lane; a hold written into a handoff, a dispatch or a PR
title by another seat is not his rule, so confirm it with him or drop it. When J.c. says to stop waiting on reviews,
the cross-family review runs after the merge, and a confirmed FAIL from it is handled revert-first (§escaped-defect
contract). Precedent: the 2026-10-23 handoff's "NO MERGE without J.c." on ACPI (OT-86) was the outgoing PM's caution,
not J.c.'s; #107 merged on his clarification.

## Worker dispatch relays (PM seat — standing rule)

Every "What to tell Jarvis" / "What to tell Prometheus" relay block the PM writes MUST include the path to the authoritative dispatch file on `main` (e.g. `Plans/DISPATCH-<date>-<slug>.md`) and the feature branch name. The chat summary is only a preview; the worker is pointed at the committed dispatch for the full task. Do not emit a relay block without the dispatch path.

**The relay MUST give the read *command*, not just the path.** PM artifacts live on `origin/main`, but the worker tree sits on a *feature branch* (never `main`), so a local `Glob`/file-open of the path **finds nothing** and the worker wrongly concludes "the file doesn't exist yet." This has bit us twice (BUILD-K, the WiFi research lane). So every relay that points a worker at a `main`-only file (dispatch, THINK, research plan, ADR) MUST spell out:

```
git fetch origin
git show origin/main:<path>          # read it; or:
git checkout origin/main -- <path>   # drop the file into the working tree without switching branches
```

Never tell a worker only "read `Plans/DISPATCH-...md`" — they will look in the local folder, not find it (it's on `main`, their tree is on a feature branch), and stall. Always pair the path with `git fetch origin && git show origin/main:<path>`. Same rule for the PL: the workers don't know PM artifacts can't be seen in the local `E:\` checkout until they're fetched from `main`.

### WORKTREE assignment — mandatory in every dispatch relay (PL standing rule, 2026-07-29)

**Every dispatch relay MUST name the coder's WORKTREE FOLDER, not just the branch.** A dispatch that says
"push to branch X" without saying "work in folder Y" is mis-specified the same way one lacking the
`git show origin/main:<path>` read-command is. This bit us on DW-1: Jarvis worked in the **shared root tree**
`E:\AI Automation\AltimitNeo` and found another lane's in-flight `boot_eth0` changes dirty in the same tree —
the two-actors-one-tree contamination hazard that had already required a full defuse (diverged branch + live
auto-push hooks nearly force-pushing over a fix).

**One persistent folder per CODER (not per build), as siblings OUTSIDE the repo:**

```
E:\AI Automation\AltimitNeo            <- PL reference tree. NO CODER WORKS HERE.
E:\AI Automation\workers\cerebro
E:\AI Automation\workers\jarvis
E:\AI Automation\workers\janus
E:\AI Automation\workers\oracle
E:\AI Automation\workers\chad
E:\AI Automation\workers\prometheus
E:\AI Automation\workers\dexter
E:\AI Automation\workers\sage
```

Siblings, **not** nested inside the repo: nested `Workers/` / `Worktrees/` folders appear as **untracked
entries in the repo's own `git status`** and can be swept into a commit. Per-coder (not per-build) keeps the
folder count bounded forever and reuses `target/` (incremental compile, no re-clone) — it is simultaneously the
fix for the 123 GB disk-bomb and for cross-lane contamination.

Every relay carries a line of this shape:

```
WORKTREE: E:\AI Automation\workers\<coder>     (yours alone — never another coder's folder, never the repo root)
BRANCH:   <the branch>                          (git fetch origin && git checkout -B <branch> origin/main)
```

**Audit command (PL, PowerShell) — shows every tree, its branch, and its dirty-file count:**

```powershell
$git = "C:\Program Files\Git\bin\git.exe"
Get-ChildItem "E:\AI Automation" -Directory -Depth 2 -ErrorAction SilentlyContinue |
  Where-Object { Test-Path (Join-Path $_.FullName ".git") } |
  ForEach-Object {
    $b = (& $git -C $_.FullName branch --show-current 2>$null)
    $n = ((& $git -C $_.FullName status --porcelain 2>$null) | Measure-Object).Count
    [PSCustomObject]@{ Folder=$_.FullName.Replace("E:\AI Automation\",""); Branch=$b; Dirty=$n }
  } | Format-Table -AutoSize
```

**Git cannot tell you *who typed* in a folder** — the reliable control is assignment (launch each coder's CLI in
its own folder), not detection. Two signals do reveal a collision after the fact: (a) a tree whose dirty files
span **two different lanes** (the DW-1/eth0 signature), and (b) the **absolute paths coders cite in their
RETURNs** — those name the folder they are actually in, and the PM checks them against the assignment.

### Work is only real when it is on `origin` — the tree-state audit (standing rule, PL-ratified 2026-08-04)

**The PM is structurally blind to uncommitted work.** The PM runs in a cloud clone and verifies only what
lives on `origin`: branches, SHAs, gate runs, diffs, merge-bases. **Local working-tree state on the PL's
disk is invisible to it by construction.** No amount of PM attention closes this — the check has to run on
the PL's side. Treat "the PM would have noticed" as false for this entire class.

**Why this rule exists.** On 2026-08-04 a routine disk reclaim nearly deleted a fix for a permanent
reservation leak in the SMP scheduler (`wake_thread_front`) that existed **only** as uncommitted changes in
one worktree, plus two commits never pushed. It was rescued to
`origin/rescue/cerebro-c1.1-aag-fixforward` @ `f5df4ecc` and became OT-80. **Twice in that same pass a
folder looked like a stale pre-rebase duplicate and twice that read was wrong** — the reliable control was
mechanical, not judgemental.

**Root cause, stated precisely: it was a HANDOFF, not headcount.** C1.1 passed through three seats
(Cerebro → Dexter, on the usage limit → Janus). When the lane moved, the previous seat's tree stayed behind
with live work in it. **The same hole opens with two coders the moment a lane changes hands.** Seat count
does not create it; it multiplies the surface it operates on (more seats × more handoffs = more trees).
Do not "solve" this by shrinking the roster.

**1. Every RETURN is a PUSHED artifact. In-session analysis does not exist.**
A review delivered only in chat, or committed but never pushed, is invisible to the board and to every other
seat. This hit **four seats independently in one week** — Oracle (twice), Jarvis (committed, never pushed),
Sage — which means the *step was missing from the process*, not that four people were careless. Every
dispatch's close condition names the artifact path and requires the push; a dispatch that omits it is
mis-specified the same way one lacking the disk-reclaim line is. **Precedent:** Oracle's eth0 AAG was
completed 2026-08-01, delivered in-session, never committed — the board correctly read the dispatch as
unstarted and it silently blocked a 0.1 milestone rung for a day.

**2. The audit — run it at LANE CLOSE and at HANDOFF, not at cleanup time.**

```powershell
$git = "C:\Program Files\Git\bin\git.exe"
$rows = Get-ChildItem "E:\AI Automation\workers" -Directory | ForEach-Object {
  $p=$_.FullName
  $br=(& $git -C $p branch --show-current 2>$null)
  $d=((& $git -C $p status --porcelain 2>$null)|Measure-Object).Count
  $u=((& $git -C $p log --oneline "@{u}.." 2>$null)|Measure-Object).Count
  [PSCustomObject]@{Worker=$_.Name; Branch=$br; Dirty=$d; Unpushed=$u}
}
$rows | Format-Table -AutoSize
```

**The discriminator is `Dirty = 0` AND `Unpushed = 0`.** Anything else is potentially the only copy and gets
committed + pushed **before** the tree is touched. Two caveats learned the hard way:
- **`Unpushed` can mean *diverged*, not *ahead*.** A push may be rejected non-fast-forward because the branch
  is **behind** origin. **Never force-push to "fix" it** — that overwrites merged work. Check first.
- **`git cherry origin/main HEAD` is the real test** of "is this already on `main`?" — `-` means an
  equivalent exists upstream (safe), `+` means unique (push it). It catches cherry-picked/rebased duplicates
  that SHA comparison misses. Use it before believing a tree is a stale copy.

**3. Handoff close-out (the step that was missing).** When a lane changes seats — reassignment, usage limit,
anything — the **outgoing** seat's tree is audited and cleared **before** the incoming seat starts. Whoever
relays the handoff owns this. It is one command and it is exactly where the OT-80 work slipped.

**4. Nested `Workers/` and `Worktrees/` inside the repo are forbidden and being removed.** They cost 71 GB
in one pileup, they are invisible to a depth-1 audit of `E:\AI Automation`, and they appear as untracked
entries in the repo's own `git status`. The sibling per-coder layout above is the only supported one.
**`cargo clean` a per-coder tree; never delete it** — the checkout is cheap to keep and expensive to re-clone.

**5. Sage is an exception to the worktree rule.** Sage works from **pasted context**, not a local checkout,
so it has no tree to audit and needs none — giving it a multi-GB clone would be pure cost. Its dispatches
must therefore name **how the artifact reaches `origin`** (which tree the relay lands it from), because the
seat cannot push for itself. A Sage dispatch that says "commit and push your RETURN" is mis-specified.

### RAG reminder — mandatory in every dispatch relay (PL standing rule, 2026-07-06)

**Every dispatch relay MUST include a standing reminder that the project has a local semantic-code-search RAG, to use it when exploring unfamiliar code / locating symbols / answering "where is X done?".** This is not optional boilerplate to drop when a dispatch feels small — the RAG is a first-class tool and coders forget it exists. **The PL manages the RAG infrastructure; starting it is not the coder's job.** If it's down, the coder notes it in the RETURN and falls back to grep/Read (the RAG is a convenience, never a blocker) — with an optional self-serve start if they want it. Paste this block (adjust the one query to the dispatch's actual subject) into the relay of *every* dispatch, feature or research:

```
RAG (semantic code search) is available for this project — use it for conceptual "where is X
done?" questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: Invoke-RestMethod http://127.0.0.1:8404/health  → expect {"status":"healthy",...}.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct
file reads (never a blocker). Optional self-serve start (coders run PowerShell):
  powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
  cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
Query e.g.: $body=@{query='<this dispatch's subject>'; top_k=5}|ConvertTo-Json;
  Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"
```

The authoritative endpoints/troubleshooting live in the "NeoPlutarch RAG — Semantic Code Search" section below; the relay just has to *point coders at it* (and remind them the PL owns startup — note-and-fall-back if it's dead). A dispatch relay lacking the RAG reminder is mis-specified the same way one lacking the `git show origin/main:<path>` read-command is.


## Forward momentum — never offer to pause (PL standing preference)

The PL never wants to pause, stop, bank progress, or halt at a "clean stopping point." Do NOT present pausing/stopping/banking as an option, and never frame a milestone or a setback as a reason to stop. Always drive to the next concrete step and take or recommend the forward action by default — including through hard, slow, or iterative work (e.g. a multi-cycle hardware bring-up loop). Genuine mutually-exclusive or irreversible decisions are still surfaced for the PL; "keep going vs pause" is never one of them — the answer is always keep going.

## No human-labor time estimates — reason in cycles, dependencies, and gates (PL standing rule, 2026-07-21)

**Never estimate work in human-labor calendar time** — no "2 weeks," "6 months," "person-months (pm)," "a day," "a sprint." Those units silently import the false model of a human coder working ~40 hrs/week. **We do not use human coders.** The seats are AI that can run 24/7; wall-clock-per-human-effort is a category error here, and it distorts scope and priority. This applies to PM dispatches, ADRs, THINKs, research framing, and *any* estimate — including person-month numbers a third-party crate's own docs quote (iced's "12–20 pm," Vello's "4–11 pm"): **restate them in the units below, do not pass them through.**

**Estimate scope in the constraints that are actually real for 24/7 AI seats:**
1. **Dependency depth (the DAG).** What must precede what — the chain length, not a calendar. E.g. "P1 → P2," "VT-d fence → userland compute queue," "ADR cross-check → Slice 1."
2. **Iteration cycles.** The number of build→gate→diagnose loops, especially for blind-hardware/gated work where "the next run reveals the next layer." Some problems are inherently many-cycle (a from-scratch driver) regardless of typing speed; say so, and name the expected cycle count / unknown-depth honestly.
3. **Gate / verification cost.** The merge funnel (PM verify-before-merge serializes), the QEMU-gate wall-clock, the AAG lens count (blast radius). This is a **machine clock** (runner/gate throughput — the real quantity that made the runner-capacity lane worth opening), NOT a human clock. Machine time stays a legitimate tracked number; human-labor time does not.
4. **Difficulty / uncertainty tier.** Mechanical vs genuinely-unknown (the WGSL→EU compiler, a from-scratch AMD/Vega driver). Uncertainty is measured in how many spikes / cross-model checks it takes to de-risk, not months.
5. **Lane parallelism.** How many independent, non-conflicting lanes can run at once (bounded by the merge funnel + runner capacity + conflict-freedom) — this sets throughput.

**The distinction to preserve: hard is still hard.** Dropping calendar time does NOT mean everything is free or instant. A deep program is described as deep — many-cycle, high-uncertainty, N gates, off the critical path — instead of "N months." The point is to model the *real* bounds (dependencies, cycles, gates, difficulty, parallelism), which are more decision-useful than a fabricated human-time number anyway. When a PL or coder artifact still carries a person-month/week/month figure, treat it as mis-specified the same way a dispatch lacking a discriminator section is, and re-express it.

## Git Worktrees - PM Seat vs Worker Tree

> **SKIP IF USING CLOUD PM:** the current PM runs in a separate cloud clone and pushes
> straight to `origin/main`, so local workers only need
> `git fetch origin && git show origin/main:<path>` to read PM artifacts. The
> two-worktree procedure below is for a local PM seat sharing the workers' `.git`.

> **STATUS (2026-06-13): the dedicated PM worktree is now OPTIONAL.** The two-worktree
> dance below was designed for a **local** PM seat that shared one `.git` with the
> workers. The PM seat (**Claude**, the cloud PM — *not* the CLI coder Cerebro) now runs in a **cloud container — a separate clone —**
> and pushes straight to `origin/main`. Because the PM never commits inside the PL's
> local trees, the "PM commit lands on a worker's feature branch" hazard **cannot occur
> from the current setup**; the isolation is automatic. So:
> - The PL's single tree `E:\AI Automation\AltimitNeo` stays on worker feature branches,
>   as always. **No `-pm` folder is required.**
> - To **read** anything the PM pushes, use GitHub directly (`.../tree/main/Plans`,
>   `.../tree/main/docs`) — zero local fetch/branch dance — or a one-off
>   `git fetch origin && git show origin/main:<path>`.
> - Workers still consume dispatches the same way: `git fetch origin` then read
>   `origin/main:Plans/DISPATCH-...` (the committed-dispatch rule is unchanged).
>
> **The caveat (when the rule below re-applies):** if a PM seat is ever run *locally*
> again (a local Claude Code PM sharing the workers' `.git`, rather than the cloud seat),
> the full two-worktree discipline below becomes mandatory again to prevent the
> commit-collision hazard. The history is kept for that reason.

This repo uses two working trees backed by one shared `.git`, so the PM seat (**Claude**, the cloud PM) can land documentation and plan commits on `main` without colliding with whatever branch a coder (Cerebro/Jarvis/Janus/Prometheus) has checked out. *(Applies to a local PM seat; see the STATUS note above — the current cloud PM makes the `-pm` worktree optional.)*

- **Worker tree:** `E:\AI Automation\AltimitNeo` - where workers check out feature branches (`codex/...`, `prometheus/...`) and where all builds, tests, and bare-metal runs happen. This tree sits on a *feature branch*, never on `main`.
- **PM worktree:** `E:\AI Automation\AltimitNeo-pm` - permanently checked out to `main`. All PM artifact commits (anything under `Plans/`, `docs/`, plus `README.md`, `Plans/INDEX.md`, `CLAUDE.md`, `AGENTS.md`, the Bible, ADRs, ANSPECs) are made here and pushed to `origin/main` directly.

### Why this exists

A single shared tree means a live worker's branch is also the branch your commits land on. A PM doc/plan commit made while a worker is mid-build silently lands on the worker's feature branch, and a `git push origin main` then reports "up-to-date" while `origin/main` stays stale. (This happened once: the 2026-05-28 README refresh landed on `codex/phase-4-build-1a-jarvis` instead of main and had to be promoted to main after the fact.) The dedicated PM worktree removes the hazard: `main` has exactly one home, and it is the PM seat's.

### The one hard rule: main lives in the PM worktree only

Git forbids the same branch being checked out in two worktrees at once. Because the PM worktree permanently holds `main`:

- The worker tree must NEVER be on `main`. Park it on a feature branch when idle.
- If you run `git checkout main` in the worker tree and see `fatal: 'main' is already checked out at '.../AltimitNeo-pm'`, that is the guardrail working, not a bug. Do main-branch work in the PM worktree.

### PM commit/push workflow

Run from the PM worktree:

    Set-Location "E:\AI Automation\AltimitNeo-pm"
    # edit Plans/, docs/, README.md, etc. (this is main's working copy)
    & "C:\Program Files\Git\bin\git.exe" add <files>
    & "C:\Program Files\Git\bin\git.exe" commit -F <msgfile>   # write msg via [System.IO.File]::WriteAllText => no BOM
    & "C:\Program Files\Git\bin\git.exe" push origin main

Refs are shared across worktrees, so the worker tree sees the advanced `main` immediately (e.g. to rebase a feature branch onto it).

### Merging a completed feature branch to main

Because `main` lives in the PM worktree, feature-branch closes are merged there, not in the worker tree:

    Set-Location "E:\AI Automation\AltimitNeo-pm"
    & "C:\Program Files\Git\bin\git.exe" merge --ff-only codex/phase-N-build-X-jarvis
    & "C:\Program Files\Git\bin\git.exe" push origin main

#### The stale-branch-base trap (and how we prevent it)

A worker that builds while the PM seat advances `main` (landing ADRs, dispatches, notes) ends up on a branch whose base is *behind* current `main`. Such a branch cannot fast-forward, and a careless `git reset --hard <branch>` + force-push would drop everything `main` gained since the branch's base. The `--ff-only` merge above fails safe (it refuses), and a plain `push origin branch:main` is rejected as non-fast-forward — so the documented flow does not silently lose data — but the recurring tax is real (it forced a manual cherry-pick in BUILD-2 and again in the CI-gate merge). Two-layer defense:

**FIRST verify `origin` actually points at GitHub (root cause, found by Oracle 2026-07-31).** Several worker
trees were cloned from **the PL's local tree** (`E:\AI Automation\AltimitNeo`) rather than from GitHub, so their
`origin` is a *local* clone. For those trees `git fetch origin && git show origin/main:<path>` silently reads a
**stale** `main` — which means **the entire dispatch-relay mechanism no-ops**: a coder "reads" a PM dispatch
that is months old, or reports the file "absent from origin/main" (exactly what happened to Janus on DW-2, who
proceeded from the relayed one-liner instead). Every rule below about comparing against `origin/main` is
worthless until this is right. Check and fix:

```powershell
$git = "C:\Program Files\Git\bin\git.exe"
Get-ChildItem "E:\AI Automation\workers" -Directory | ForEach-Object {
  [PSCustomObject]@{ Worker=$_.Name; Origin=(& $git -C $_.FullName remote get-url origin 2>$null) }
} | Format-Table -AutoSize
# any Origin that is a local path, not https://github.com/... , must be repointed:
& $git -C "<tree>" remote set-url origin https://github.com/SecondStageTurbine/NeoPlutarch.git
& $git -C "<tree>" fetch origin
```

**Compare against `origin/main`, never a local `main` ref (recurring trap, 2026-07-31).** The local `main`
branch in a worker tree — and in the PM's cloud clone — is frequently stale, because work is pushed via
`HEAD:main` and nobody checks `main` out. A coder who diffs against local `main` sees a *different* (older)
base than reality and can wrongly conclude they are fresh. **Always `git fetch origin` first, then compare
against `origin/main`.** This bit the WiFi lane: the branch looked current against the PL's local tree while
being 16 commits behind `origin/main`.

**Docs-only staleness does NOT require a re-gate (the tree-identity rule).** When every commit `main` gained
touches only `Plans/`, `docs/`, or `CLAUDE.md`, a rebase produces a **byte-identical code tree**, so an
existing green gate remains valid evidence for the rebased SHA. Prove it instead of re-burning a 4-shard run
(~25 min on the binding constraint): after rebasing, assert the code subtrees match the gated SHA —
`git rev-parse <gated-sha>:kernel` vs `<rebased-sha>:kernel`, and likewise `userland`, `tools`, `Cargo.toml`,
`Cargo.lock`. Identical hashes ⇒ the gate transfers; cite both SHAs in the RETURN. A re-gate is required the
moment any code path differs.

**A staleness figure is a SNAPSHOT — cite the base SHA you measured against, never a bare commit count
(PM error, 2026-08-01).** The PM told Cerebro that Slice-4 was "~12 commits behind, all docs-only" and that the
gate would therefore transfer. That was measured *before* DW-1 merged; `main` moved four times while the branch
sat, and DW-1 is **15 files of real code**. Cerebro checked instead of taking the free pass and found three
subtrees differing (`kernel`, `userland`, `tools`) — the shortcut did not apply. **Whoever quotes staleness
names the base SHA it was measured from**, so a stale number can never quietly authorize skipping a gate; and
**the coder re-verifies tree-identity at the moment of rebase**, not from a PM figure quoted earlier. The rule
binds hardest exactly when it is inconvenient — that is the whole value of making it mechanical.

**A rebased branch orphans its DEPENDENTS — re-stack them (bit us on DW-1/DW-2, 2026-07-31).** When a
stacked lane exists (B branched from A), rebasing/force-pushing **A** leaves **B** sitting on A's *old,
rewritten* commits — so B silently carries a **duplicate copy of A's work at a dead SHA**, and merging both
conflicts or double-applies. Git gives no warning; `git merge-base --is-ancestor <A-tip> <B>` returning false
is the detector, and the PM runs it before merging any stacked pair. The fix is `--onto`, replaying only B's
own commits:
`git rebase --onto <A-new-tip> <A-old-base-sha> <B-branch>`.
**Whoever rebases A tells the dependent's owner** in the same breath as recording the SHA mapping.
**And the dependent re-stacks only when its base is FINAL — merged to `main`, not mid-bounce (2026-07-31).**
DW-2 was orphaned **twice in a row** because Janus re-stacked onto DW-1 while DW-1 was still being fixed: each
AAG bounce and rebase moved the base again, burning a rebase per round. A dependent chasing an in-flight base
never converges. **Hold, let the base merge, then rebase onto `origin/main`** — a base that cannot move under
you. When the base's fix and the dependent's work implement the *same* idea two ways (DW-1's per-handler
guards vs DW-2's `*_on_channel` protocol helpers), converge on one at that rebase — prefer the version where
the rule lives with the contract rather than duplicated per call site.

**Rewriting a FEATURE branch is normal; rewriting `main` is not.** `--force-with-lease` on your own feature
branch is the documented tool and carries no shared-history risk. The only cost is that PM artifacts citing an
old SHA dangle — so **record the SHA mapping (`old → new`) in the RETURN** and the PM updates the citation.
Never force-push `main`.

**Worker close-gate (removes the staleness at the source).** Every dispatch's close-gate requires the worker, immediately before declaring close, to:

    git fetch origin
    git rebase origin/main          # land the worker's commits on top of current main
    # re-run the build/test gate after the rebase, then push the rebased branch

A branch rebased onto current `main` arrives fast-forward-clean, and the PM merge is a trivial `--ff-only`. This clause is standard in every dispatch (see the dispatch template / any `Plans/DISPATCH-*`).

**PM merge-check backstop (catches it mechanically if a branch still arrives stale).** Before merging ANY worker branch to `main`, the PM seat runs:

    scripts/pm-merge-check.sh origin/<worker-branch>

It fetches, prints the main/branch/merge-base relationship, and either reports **ff-safe** (do the `--ff-only` merge / `push branch:main`) or **STALE BASE** with exit 1 — listing the main commits the branch is missing and the exact worker-only commits to cherry-pick onto current `main` instead. Never merge a branch that the script flags STALE without cherry-picking. (Run it on Windows via the same `& "C:\Program Files\Git\bin\git.exe"`-on-PATH expectations; the script shells out to plain `git`.)

### Disk hygiene — reclaim the build tree after merge (PM/PL standing rule)

Per-build folders are a disk bomb: each is a full checkout carrying its own multi-GB Rust `target/`, so a backlog of merged-but-undeleted build trees silently consumed **123 GB** (PL, 2026-06-17) — eleven dead build folders at ~11 GB each. The work was already safe on `main`; the folders were pure waste. Two layers fix this:

**Structural (the real fix — prevents the pileup): one reusable folder per *worker*, not per *build*.** A coder keeps a single persistent tree and just `git fetch && git checkout -B <new-build-branch> origin/main` for each new dispatch, reusing the same `target/` (incremental compile, no re-clone). Four coders ⇒ four folders, bounded forever — instead of one-folder-per-build growing without limit. This is the durable answer to both the disk bomb and the recurring shared-worktree contamination; adopt it when convenient.

**Per-dispatch close-step (the cleanup, mandatory until the structural fix lands).** **Every dispatch MUST end its close-gate / Merge (PM) section with a disk-reclaim line.** The standing boilerplate (paste into every dispatch):

> **Disk reclaim (after PM confirms merge):** once the PM has merged this branch to `main` and confirmed it (the commit is on `origin/main`), delete this build's working tree to reclaim its `target/`. If it's a git worktree: `git worktree remove <path>` (then `git worktree prune` if git complains); if a standalone clone: delete the folder. **Never delete a tree whose branch is unpushed/unmerged** — verify the work is on `origin/main` first (the cleanup discriminator: merged content on `main` = safe to delete; unpushed in-flight work = the only copy, keep). A faster interim if you want to keep the checkout: `cargo clean` reclaims the bulk (`target/` is ~80% of the folder) without losing the tree.

The PM, when reporting a merge complete to the PL, names the now-deletable folder in the same message so the reclaim happens at merge time, not months later. A dispatch lacking the disk-reclaim close-line is mis-specified the same way one lacking the rebase close-gate is.

**The inverse hazard — never let in-flight work be a single uncommitted copy (PC-swap near-miss, 2026-07-13).** Disk reclaim protects *merged* work from careless deletion; this protects *unmerged* work from disk loss. Cerebro's entire OT-59 #2 root fix once existed **only** as uncommitted working-tree changes in one worktree — a dead disk would have erased it. A machine can die, be swapped, or corrupt at any moment, so **a coder commits + pushes at every natural checkpoint** (fix implemented, verify passed, end of a work session), so there is always a copy on `origin`. A branch push does not fire the QEMU gate (only `workflow_dispatch`/nightly does; push-CI is host-checks), so pushing a WIP branch is free insurance — the sequencing-discipline reason to *hold a push* (keeping a runner free) never justifies leaving the only copy on one disk; commit + push, then hold the *gate*. "Staged on the branch" is not a backup until it's committed and on `origin`.

### Recreating the PM worktree if it goes missing    Set-Location "E:\AI Automation\AltimitNeo"
    & "C:\Program Files\Git\bin\git.exe" worktree add "E:\AI Automation\AltimitNeo-pm" main

If git says the path is already registered but the directory is gone, run `git worktree prune` first.

### git-on-PATH note (Windows)

`git` is not reliably on the spawned-shell PATH on this machine (it is in the user PATH, not the machine PATH). Invoke git by full path - `& "C:\Program Files\Git\bin\git.exe" ...` - run via `Start-Process -FilePath ... -RedirectStandardOutput/Error <tempfile>` to capture output. When a path argument contains spaces, quote it inside the argument string (e.g. the literal `"E:\AI Automation\AltimitNeo-pm"` wrapped in single quotes) or Start-Process splits it on the space. Permanent fix (one-time, admin): add `C:\Program Files\Git\cmd` to the machine PATH.



## CI is a gate, not optional — `[skip ci]` only for docs (PM/worker standing rule)

`[skip ci]` (or paths-ignored pushes) is reserved for **pure documentation/`Plans/` pushes**. The CI
workflow already paths-ignores `docs/**`, `Plans/**`, and `**.md`, so those never trigger CI regardless —
`[skip ci]` on them is redundant-but-harmless. **Any push that touches `kernel/`, `tools/`, `userland/`, or
`Cargo.*` MUST let CI run** — do NOT `[skip ci]` a code change.

This rule exists because it bit us: through BUILD-7.5-B → W0 → W0b → V → K, every *code* merge used `[skip ci]`
for CI-cost reasons, so fmt/clippy/host-tests/QEMU never ran on any of them and the debt accumulated invisibly
(a `cargo fmt` violation sat in `cdc_ecm.rs` from BUILD-B; CI was effectively red for weeks without anyone
seeing it). BUILD-LO — the first non-`[skip ci]` code push since — surfaced all of it at once. A skipped CI on
a code change does not save cost; it *defers and compounds* it. The cost discipline is real, but it belongs in
*what CI runs* (sharding, caching), not in skipping the gate on code. **A green CI is a falsifiable safety
property the same way the byte-oracle and the invariant ledger are; do not let it rot.**

**Hermeticity is a gate invariant (CI-hygiene build, add484c, 2026-06-19).** No scenario in the 4-shard
QEMU gate may depend on a host-variable input — not the live internet, and not a CPU feature that differs by
runner. A green gate must mean "the code is correct," never "the runner cooperated." The fix that established
this: `phase4_https_fetch` flaked by *which runner picked it up* because RDRAND under pure TCG advertises the
CPUID bit but returns CF=0 (`-cpu qemu64,+rdrand`), starving the guest's getrandom; RDRAND-consuming scenarios
now run under `-cpu max` (the `kernel_rng_selftest` treatment) so they're deterministic on any runner.
**Correction (2026-11-01, #43):** the "TCG returns CF=0" diagnosis was very likely our own bug: `rdrand_u64` treated
the carry flag backwards (CF=1 means a valid value), so it threw away good draws. The product image panicked at boot
on it, hidden from the gate because every `neo-test` kernel seeds the DRBG with a fixed seed. The `rng-seed-boot` CI
job now boots a non-`neo-test` kernel through `Csprng::init`. Lesson: a test-only seed or stub must never be the only
path the gate exercises; at least one gated boot runs the product initialisation. A
scenario that genuinely needs real external network is reclassified *out* of the gate (the
`mt7601u_scan_baremetal` precedent), never left in to flake. Corollary: an artifact/storage-upload failure is
infra, never a test verdict — the upload-on-failure step is `continue-on-error` so a full quota can't redden a
tests-passed shard.

## The escaped-defect contract — revert-first when `main` reddens (PL-ratified 2026-07-12, OT-58)

A merge gate that runs a *subset* (or a staging config where push-CI skips QEMU) means a defect can land on
`main` and only redden later — the nightly-full, a `workflow_dispatch` gate, or the next coder's branch. When
that happens, the standing contract (PL-ratified from the CI gate-scaling synthesis, `docs/THINK-2026-07-12-ci-gate-scaling-SYNTHESIS.md`):

1. **Revert-first is the DEFAULT.** A red `main` is reverted to green immediately — because coders branch off
   `main` constantly, a red `main` taxes *every* seat that rebases during the window (the stale-base tax). `main`
   stays one action from green, always branchable.
2. **Fix-forward is a bounded escape hatch — ~4 working hours.** The owning coder may fix-forward *instead* of
   reverting only if they land a **rebased, gate-green** fix within ~4 working hours. Past that window: revert
   now, re-land the fix on its own branch later. The reverted work is not lost — it sits on its branch and
   re-merges with the fix.
3. **Infra-reds are exempt.** A runbook-class red (Docker-VM-disk, RAM-OOM, runner loss, artifact-quota — see
   `docs/runbook-ci-runner-capacity.md`) is re-fired, never counted as an escape or reverted as if it were code.
4. **The escaped scenario gets promoted.** Whatever scenario caught the escape is promoted into the
   merge-blocking tier (PM-reviewed, to avoid one-off inflation) — the self-healing feedback loop, so the same
   class can't escape twice.

The ~4h number is the PL's dial. This is the governance half of OT-58 (the tiered-gate *plumbing* — Slices 0–4
— remains a future build); the contract itself is standing policy now. **OT-59 is the transition case**
(defects that escaped when push-CI stopped running QEMU under staging) — grandfathered to fix-forward because
the introducing commit is not yet bisected and diagnosis is already underway; every escape *after* this
ratification follows revert-first.

## The close-gate ritual — every coder, every build (standing rule)

**Before any coder (Cerebro / Jarvis / Janus / Prometheus) declares a build done, they run one command:**

```
scripts/worker-close-gate.sh
```

It is the worker-side mirror of `scripts/pm-merge-check.sh`: pm-merge-check is the PM's backstop *at merge
time*; the close-gate is the worker's self-check that makes the branch arrive **ff-clean and CI-green** so the
PM merge is trivial. It is one command precisely so it cannot be half-run — the failures it prevents all came
from running *part* of the ritual. The script (and therefore the ritual) does four things, in order:

1. **Rebase freshness** — `git fetch origin` + assert the branch base == `origin/main` tip. If STALE, it stops
   and tells you to `git rebase origin/main` first. **This is the close-gate's first requirement** and the one
   most often skipped: a branch cut from an old `main` and not rebased arrives STALE, forcing the PM into a
   cherry-pick (and, when squashed, into phantom-deletion territory). Rebase at the *source*, not at merge.
2. **The exact CI host-check set, in CI order, via Docker** — `cargo fmt --all --check`, then
   `cargo test --workspace --exclude neoplutarch-kernel`, then `cargo clippy --workspace --all-targets -- -D
   warnings`, then the **blocking** kernel clippy (`cd kernel && cargo clippy -Zjson-target-spec -- -D
   warnings`). Running *all* of them, on the *rebased* tree — not "clippy passed locally" while fmt silently
   fails (the exact miss that bounced CSPRNG 10.1).
3. **SPDX** on every changed source file.
4. **Self-inspection + the RETURN-evidence block** — it prints your own `git diff --stat` vs `main` (inspect it
   like the PM will — no phantom deletions), flags whether the build is **kernel-touching** (⇒ the pre-merge
   `workflow_dispatch` QEMU gate is mandatory), and prints the evidence block you must paste into your RETURN.

**Two hard rules the ritual enforces — these are what actually gets "baked in":**

- **"Green" without a CI URL is not done.** A RETURN MUST paste the **green CI run URL** — the push CI, and for
  any kernel-touching build the **`workflow_dispatch` QEMU run** (all 4 shards + host + SPDX). A local
  `cargo`/`neo-test` pass is not the gate; the *CI run* is. (CSPRNG 10.1 was reported "Green. Pushed" while the
  branch CI was red — this rule is the fix.)
- **The PM inspects the commit's OWN `git show --stat` before merge, always.** A "GREEN, ready" report is not
  "safe to merge." This caught the CSPRNG corrupt squash (a 3437-line phantom deletion that every other signal
  missed). The worker's Step-4 self-stat and the PM's independent re-stat are the same check from both ends.

**Every dispatch's Merge/close-gate section points at this ritual** (don't re-spell the steps per dispatch —
reference `scripts/worker-close-gate.sh` + the two hard rules). A dispatch whose close-gate omits the
close-gate ritual is mis-specified the same way one omitting the disk-reclaim line or the kill-switch ISC is.

This rule exists because every bounce of this session was catchable *here*, at the worker, instead of at PM
merge: two STALE-BASE branches (CSPRNG, the input path), one fmt miss reported as green, one auto-merge
duplicate that passed QEMU but would have failed the blocking clippy. None were coding-ability failures; all
were *release-ritual* gaps. The gate caught them — but at merge time, costing a bounce each. The close-gate
moves the catch upstream so the branch arrives clean.

## Merge bar vs acceptance bar — do not conflate them (PM decision, 2026-08-01)

**Merging the CODE and claiming the FEATURE WORKS are two different gates.** Conflating them left good branches
rotting on the vine, accumulating rebase tax and going stale against a `main` that moves daily.

- **Merge bar (for landing code on `main`):** rebase-fresh + **4-shard gate green on the exact SHA** + PM
  verify-before-merge (independent CI re-verify + own `git show --stat`) + **AAG clean** + no regression.
  A branch that clears this is *strictly better than `main`* and should land.
- **Acceptance bar (for claiming a capability works):** the **bare-metal proof** — a real lease, a real
  LINK-UP, a real EU byte-equality on silicon. This is a **milestone**, tracked in the release ledger.

**A device-less QEMU gate cannot exercise real silicon** — so if the bare-metal proof were also the merge gate,
every driver branch would sit unmerged indefinitely while the one bare-metal box is busy with another lane. That
is what happened to the WiFi branch (14 commits, two rebases, still unmerged) and to Slice-4. **Holding a
gate-green branch does not make it safer; it makes it staler**, and staleness is its own defect source
(orphaned dependents, cherry-pick taxes, phantom-deletion diffs).

**Applied 2026-08-01:** WiFi (`mt7601u`) and Slice-4 EU both move to *merge on gate+AAG*, with the LINK-UP and
the M710q EU proof tracked as **acceptance milestones**, not merge gates. Merging the code makes no claim that
the feature is proven — the ledger says what is proven.

**The exception that stays:** where merging would *introduce* a defect to `main` rather than bank an
improvement — e.g. a scenario not yet on `main` that would arrive red (the C1.1 `threads_futex` case), or a
change that would panic on the target hardware (the rung-5 `boot-neofs` case). Those are merge-blockers on the
merge bar itself, not acceptance questions.

## The tiered gate: quick per PR, full per batch (OT-58, PM-approved 2026-11-01)

The design is `docs/THINK-2026-11-01-ot58-tiered-gate.md`; section 6 there is the authority for these rule changes.
**On GitHub-hosted runners the free tier runs 20 jobs at once, account-wide, and one full gate was 22 jobs**, so
per-PR full gates queue behind each other. The gate now has two tiers. The rules below change the merge bar, the
close-gate evidence and the AAG to match.

**Quick tier, on every PR (automatic).**
- `CI` runs on every PR push. Its QEMU job is one cell, `QEMU smoke (quick tier)`:
  - `cargo xtask gate-plan`, then `cargo neo-test --tier quick` (the smoke list plus the PR's changed scenarios
    within the cost budget);
  - a 12-minute budget check.
- A seat can also dispatch `CI` with `tier: quick` on a branch.
- The RETURN or `PM:` comment cites **this run's URL** as the close-gate evidence. This replaces "the PR's own
  `workflow_dispatch` QEMU run".

**Full tier: one run over a batch, before anything lands.**
- The PM dispatches `CI` with `batch: "<PR numbers in landing order>"`:
  - `base_sha` defaults to main's tip;
  - `prefix=N` is a bisect leg; `prefix=0` is the base alone.
- **A member must be green on its exact head** (its PR run's smoke job passed), and `tier=quick` by `gate-plan`.
- **A PR `gate-plan` puts in `full-alone` gets the full tier on its own PR run:** the plan job sees `full-alone` and
  runs the 4 shards instead of the smoke job. That covers the SMP/concurrency core, the harness and xtask (except a
  scenario-only addition), `scripts/`, the workflows and the build configuration. Its landing is a batch of one,
  whose plan accepts the 4 green shards as its green run.
- Every job rebuilds the same tree with `scripts/batch-assemble.sh` and fails unless its hash is the plan's.
- `batch-report` lists every failing scenario and flags infra.

**Merge bar.** It replaces "4-shard gate green on the exact SHA". The batch run must be green on the batch tree. The
merge report names:
- the batch base SHA;
- every member's head SHA;
- the batch tree hash;
- the run URL;
- the batch AAG (below).

The PM's own `git show --stat` stays **per PR**.

**Landing.**
- **Before pushing**, the PM re-runs `scripts/batch-assemble.sh <base> <heads…>` locally and checks it prints the
  report's tree. The assembly is deterministic (fixed author, committer and date), so it gives the same commit too.
- The PM then lands that commit, or the same merges in the same order, in **one push**.
- If `main` moved past the base:
  - by docs only, the landing is fine when `git diff --quiet <batch tree> <landed main> -- . ':!Plans' ':!docs' ':!*.md'`
    holds;
  - by any code, re-batch.

**Red batch.**
- **Bisect by prefix** (`prefix=N`): binary search, one job per round, running only the failing scenarios. The first
  failing prefix's last member is the owner, and that PR is bounced.
- **Infra reds are re-fired, never bisected:** a shard with no timing table, runner loss, disk-full or OOM.
- **A timeout is re-run once** before bisecting.
- A red batch never lands, so it never reaches `main`.

**Revert unit.** `scripts/revert-batch.sh <base>` reverts a landed batch's merges as **one commit**. That is the "one
action from green" of the escaped-defect contract, and the ~4-hour fix-forward window applies to the named owner.

**AAG.**
- Still per kernel-touching PR, **plus one AAG over the batch's composed diff** when two or more members touch the
  kernel, because the composition is a new seam.
- SMP-core PRs are `full-alone`, so they never share a batch, and their cross-family lens stays per PR.

**QEMU stagger.** The "at most ~2-3 kernel-touching QEMU gates in flight" rule is **superseded for GitHub-hosted
runs** by the 20-job limit. While a batch runs, at most 4 quick tiers run alongside it. The stagger still applies if
`QEMU_RUNNER_LABELS` points the QEMU jobs back at self-hosted runners.

**Tree identity** (a docs-only rebase keeps a gate): compare everything **except** `Plans/`, `docs/` and `*.md`, not
only `kernel`/`userland`/`tools`/`Cargo.*`.

**Transition.** Until the first batch lands, a PR's own green `tier: full` dispatch is still valid merge evidence.
`tier: full` without `batch` is today's gate, and it stays the kill-switch.

## The adversarial-audit gate — every kernel-touching merge (PM standing rule)

When a model can author a booting kernel in minutes, **authoring stops being the constraint and verification
becomes the whole game** — and the same capability that *writes* a kernel can be pointed *at* one (find the
race in the dispatcher hand-off, the EOI-ordering mistake, the ABI drift that breaks a driver). So adversarial
AI verification is now a **standing leg of PM verify-before-merge**, not an occasional reflex. The authoritative
spec is `docs/adversarial-audit-gate.md`; the rule:

**Before merging ANY kernel-touching branch** (same trigger as the mandatory 4-shard QEMU gate; doc-only /
host-only branches are exempt), the PM runs an **adversarial-audit panel** — N independent agents, each on a
distinct failure-lens, each prompted to **REFUTE correctness, default-to-suspicious**: (1) concurrency /
dispatcher hand-off, (2) interrupt / EOI ordering + the I-14 ISR-lock boundary, (3) ABI / format / on-wire
drift (audit the byte-oracle), (4) memory-safety / `unsafe` / DMA-fencing, (5) completeness — "what did the
green gate NOT cover?" (the device-less-QEMU / fixture-not-real cases). **A finding bounces the branch only if
it survives adversarial re-verification; uncertain findings are RETURN notes, never speculative blockers (a
gate that cries wolf is a fired KC-5); and "no findings" requires each lens to state what it checked (no
rubber-stamp).** Scale N to the blast radius. It **replaces nothing** — it complements the QEMU gate / byte
oracle / kill-switch by catching the logic/ordering/coverage wrongness they structurally miss. The PM's merge
report names the outcome ("AAG: 4 lenses + completeness, no surviving findings"); a confirmed finding bounces
to the worker with the specific repro, like a red-team catch. This is lesson-3 of the Tolmo/Fable-5
kernel-in-38-minutes piece, made standing: better the model breaks the kernel on the branch than someone else
breaks it on `main`.

**Cross-family lens REQUIRED for SMP / concurrency-core AAG (PL-ratified 2026-07-26).** When a merge touches the
**cross-CPU / SMP concurrency core** — the scheduler, thread lifecycle/teardown, futex/wait-queue, cross-CPU
migration, per-CPU accounting, lock-ordering, or forced-termination/kill paths — the AAG MUST include at least
one **different-model-family** adversarial lens, not only the Claude PM's same-model sub-agent panel. Route a
**Codex** (Jarvis/Janus/Chad) or **Qwen** (Prometheus) refute-lens at the cross-CPU seams before merge; the PM's
merge report names the cross-family lens used ("AAG: 5 Claude lenses + Codex cross-CPU cross-check"). Rationale
(the load-bearing C1.1 precedent): a full **5-lens Claude** AAG cleared C1.1 `a061bd6f` — including a confident,
detailed pass on the exact seam — yet a **Codex** cross-check (Janus) broke two cross-CPU seams, one a **concrete
UAF** (the force-kill-vs-join Ready→Running window) that a Claude lens had explicitly (and wrongly) ruled safe.
The author (Cerebro) and the entire panel were Claude and shared the same blind spot; **same-model agreement on
a subtle SMP race is weak evidence**, exactly as §Cross-model diversity warns for convergence signals — here made
mandatory for adversarial verification of the concurrency core. Same trigger as the pitfall-bank consult
(§Concurrency / SMP-touching dispatches); an SMP-core merge whose AAG is all one model family is under-verified,
the same way a driver dispatch lacking trap-mining is mis-specified.

## Licensing (ADR-026)

NeoPlutarch source code is licensed under **MPL-2.0**. Documentation under docs/ and Plans/ is **CC-BY-4.0**. See LICENSE and NOTICE at the repo root.

**Every new source file must start with an SPDX header:**
- .rs files: // SPDX-License-Identifier: MPL-2.0 as the very first line (before #![...] attributes or //! doc comments)
- .toml files: # SPDX-License-Identifier: MPL-2.0
- .sh / .ps1 scripts: # SPDX-License-Identifier: MPL-2.0 (after shebang if present)
- .s / .asm assembly: use the file's comment syntax
- New docs in docs/: <!-- SPDX-License-Identifier: CC-BY-4.0 --> as the first line

CI will fail if any first-party source file is missing its SPDX header.

## Connectivity / driver dispatches require a governance fault-containment analysis (Phase 7.5 standing rule)

Every dispatch that brings up a connectivity or device driver (the Phase 7.5 lane —
USB host stack, USB-Ethernet, WiFi/`iwlwifi`, and the interface/identity layer) MUST
include a **governance fault-containment analysis** before it can be marked
DISPATCHED — parallel to the blind-hardware discriminator section. It carries the
invariant **I-14 (subsystem fault containment, `docs/complexity-governance.md`):** a
driver fault — interrupt storm, firmware crash, or errant DMA — cannot mutate or
stall the AI-governance invariants (I-1…I-13). *Even if the WiFi driver dies, the
kernel's governance must stay live and unaffected.* The analysis must state, for the
driver being built, which of the three vectors it touches and the guard:

1. **Lock isolation (a).** Confirm the driver's IRQ/ISR path acquires **no**
   AI-governance lock (`INFLIGHT`/`RECENCY`/`ARENA`/the reservation/`TABLE`). Holds
   today; keep it asserted (grep-checkable + runtime assertion).
2. **Liveness degradation (b).** If the driver can monopolize a CPU, show the
   residency daemon degrading to P4b's synchronous direct-reclaim backstop (the I-2
   kill-switch) — correctness preserved, governance never *fails*, only slows.
3. **Memory isolation (c).** A driver's DMA must not reach governance or tensor
   memory. I-14(c1) is the fenced DMA arena and I-14(c2) is VT-d/IOMMU; both are
   now proven. A new connectivity path must either route every DMA buffer through
   `dma_arena::fence(...)` under the existing IOMMU containment, or state why the
   path has no device-initiated DMA (the W2 USB/mt7601u discriminator-bank case).
   Unfenced connectivity DMA is still forbidden.

This rule exists because the integrity of the containment boundary must precede the
feature that needs it: I-14 was mandated (PL decision, 2026-06-10) *ahead of* the
7.5 lane so every connectivity dispatch is built containable from build one, rather
than discovering the boundary was missing on a hardware run that scribbled the ledger.

## Format/ABI specs require an independent byte oracle

Any spec that defines an on-disk or on-wire byte layout (a file format like
`.neom`, a syscall struct ABI, a network frame) MUST ship — in the ADR and/or the
test suite — a **hand-laid-out byte example** asserted against, independent of the
writer and reader code. Round-trip tests (`write_then_read == input`) are NOT
sufficient on their own: a writer and a reader built by the same team from the
same spec will agree with each other *even when both implement the spec wrong*,
so the closed loop passes green while the bytes are off-spec.

This rule exists because it bit us: ADR-028's `.neom` v1 text was internally
contradictory about whether `weight_off` was absolute or blob-relative. BUILD-2
implemented blob-relative consistently across writer + reader; every round-trip
test passed; the discrepancy with the spec's intended absolute offsets only
surfaced in BUILD-3, forcing a four-file correction. An independent
hand-computed byte fixture (now in ADR-028's "Authoritative worked example") would
have caught it at BUILD-2. Treat the worked-example/oracle as a required
deliverable in any dispatch that defines or implements a byte layout.

**Corollary — exact bytes come from raw source, never a summarizing fetch.** A
known-answer test vector / byte-oracle fixture is only as trustworthy as how it was
obtained. **Do NOT** pull crypto KATs, hex test vectors, ABI/format byte layouts, or
any other *exact-byte* reference through `WebFetch` or any LLM-mediated/summarizing
read — those models silently mangle exact bytes (odd-length hex, inserted/dropped
spaces, transposed digits), which **poisons the very oracle meant to catch
wrongness**: the KAT then passes green against a wrong "known answer." Pull the raw
authoritative file directly (`curl`/`git` the source, e.g. an upstream driver's unit
tests like Intel IWD's `unit/test-*.c` for IEEE 802.11 vectors) and read the literal
bytes. This bit us in BUILD-W0 — WebFetch mangled a WPA crypto vector and was caught
only because the implementer verified the fetched hex looked malformed. The oracle's
*acquisition* is part of the oracle; a summarizing fetch is not a source of truth for
exact bytes.

## Complexity governance — the kill-switch, the invariant budget, the consolidation cadence (PM standing rules)

Maintainability is a falsifiable property, governed like correctness. The authoritative doc is
`docs/complexity-governance.md` (the one-page invariant ledger + rules + alarm conditions). The
three rules every PM dispatch must apply:

1. **Kill-switch required.** Every policy/substrate build ships a disabled mode plus a regression
   proving *disabled ≡ the previous layer, bit-for-bit* (unit-test equality where expressible).
   A dispatch without the kill-switch ISC is mis-specified — same standing as a blind-hardware
   dispatch without a discriminator section. Complexity you can provably switch off is bounded.
2. **Invariant budget.** Builds preserve the ledger's invariants by default. A build needing a NEW
   cross-cutting invariant must name it, justify it, and ship its guard in the same build.
   Changing an existing invariant is only done relax-and-re-prove (the BUILD-P4a pattern). If the
   ledger outgrows one page, feature work pauses for consolidation.
3. **Consolidation cadence.** One no-feature consolidation build per ~3–4 feature builds: collapse
   duplication, delete dead scaffolding, reconcile docs/ADR pointers, re-verify + re-measure the
   ledger, and pass the fresh-reader legibility test. Gate discriminators are never removed as
   "test-only weight" — they are paid-down debt and stay armed permanently.

## Blind hardware runs require a failure-mode + discriminator section (PM dispatch standing rule)

Any dispatch whose gate includes a **bare-metal / blind hardware run** (no JTAG,
no debugger — MMIO/breadcrumb observability only: the Phase-6 bring-up loop is the
archetype) MUST include, as a required section before it can be marked DISPATCHED:

1. **Expected-failure-mode analysis.** Enumerate the *most likely ways this run
   still fails even if the primary fix is correct* — not just the bug being fixed.
   The base rate in blind bring-up is "the next run reveals the next layer," so a
   dispatch that designs only for success is mis-specified.
2. **A discriminator for each mode.** State the instrumentation that makes a
   failure self-diagnosing, and what observation maps to which cause. The run must
   be **conclusive whether it passes OR fails** — a failure you can't localize from
   the captured evidence is a defect in the dispatch, not just bad luck.
3. **Capture-before-you-need-it.** If the discriminator needs data the driver
   doesn't yet log (e.g. a fault *address* when only the fault *type* is captured),
   adding that capture is part of the same dispatch — you do not get to discover the
   gap on the failing run.

This rule exists because it bit us: the BUILD-4a iter7 dispatch was first written in
"implement the known fix" mode — it loaded the missing arg payload but shipped no
fault-address capture and no GGTT-vs-PPGTT (A64 address-space) discriminator, even
though iter4/5/6 had each failed in a *new* way and the driver was known to log only
`RING_FAULT` type, not the faulting address. A repeat `0x9e1` would have been a blind
mystery. It took PL pushback to add the discriminator (ISC-5a/5b) that should have
been in v1. The "one hop left / high-confidence" framing is exactly the smell to
distrust: confident dispatches read better, but blind-hardware reality is iterative,
so design every such run to be diagnostic on failure, not just green on success.

### Bare-metal dispatches reference the bare-metal run runbook (PM dispatch standing rule, 2026-07-19)

Every dispatch whose gate includes a **bare-metal Arthur run** MUST reference the operational
access/boot/capture procedure in **`docs/runbook-baremetal-arthur-optiplex.md`** (read it via
`git fetch origin && git show origin/main:docs/runbook-baremetal-arthur-optiplex.md`) so the run's
mechanics are never re-derived under fire. That runbook is authoritative for: SSH access to the
control OS (Ubuntu live USB; NeoPlutarch has no sshd), the disk layout (boot-ISO NVMe, the `NEOFSv1`
model volume, the NVMe log sink at offset 256 MiB / magic `NEO_LOG_v1` + the `dd … | strings`
readback), `efibootmgr` one-shot boot control (`efibootmgr -n 0001 && reboot`; falls back to Windows
so the return-to-SSH is a boot-select — the run is **JOINT** PL+coder by construction), serial capture
(PuTTY on the **PL's** PC → the fixed log path `C:\Users\<user>\neoplutarch-serial.log`, the coder's
analysis input every run **and the deliverable**; NVMe sink is the backup channel), and the run
procedure (auto-greet + auto-forward-pass, **let it run to completion ~3-4 min — do not halt early**,
the PL confirms on-screen legibility **by eye, live** — **there is NO framebuffer photo; the PL does
not take them, so never write "photograph the screen" into a dispatch or ask for a photo in a RETURN**
— then save the serial log, boot-select back, read the log/sink). The serial log's `REPLY:` line on a
proven-non-blank framebuffer IS the legibility proof; the log is conclusive on its own. A dispatch's
failure-mode/discriminator bank maps its expected failures onto the two capture channels (screen =
the PL's live eyes-on read; serial/sink = the durable diagnostic record). The runbook's *methodology* generalizes to other bare-metal targets (e.g. the M710q Gen9
GPU target), which supply their own concrete parameters against the same shape. A bare-metal dispatch
lacking the runbook reference is mis-specified the same way one lacking the failure-mode/discriminator
section or the disk-reclaim line is.

## Driver / blind-hardware dispatches require a deep prior-art trap-mining pass (PM dispatch standing rule)

Any dispatch that brings up a device, protocol, or chip (every driver build; the whole
Phase-7.5 connectivity lane; any blind-hardware build) MUST include, before it can be
marked DISPATCHED, a **prior-art trap-mining pass**: enumerate the *documented field
gotchas* for **the exact chip/protocol/spec being built** — from the vendor spec, the
Linux/BSD driver source, kernel mailing lists, OSDev, and chip-specific forums — and
**pre-seed the failure-mode/discriminator bank from them**, not from first principles.
The known traps go in the bank *before* the first run, each with its discriminator.

This rule exists because it cost us a full day: BUILD-7.5-B's bare-metal bring-up burned
eight blind iterations on a confident "the AX88179's ECM-TX is dead hardware" verdict
that was **our own bug** — and the three things that actually blocked it were all
*already documented* in prior art: the CDC-ECM **alt-setting-1** data-interface trap, the
**AX88179 ECM-is-vendor-only** quirk (Linux literally patched its driver to bind
vendor-only), and the **DHCP `chaddr` must be non-empty** requirement (RFC + every real
server). A trap-mining pass would have seeded all three into the bank on day zero. The
discipline: the cheapest iteration is the one prior art already paid for — mine it before
you boot. Pair this with the easier-path question: *is there a chip/protocol class that
does the hard part for us?* (e.g. FullMAC firmware running 802.11 MLME + WPA instead of a
from-scratch SoftMAC supplicant). The right target can delete a build, not just de-risk it.

## Concurrency / SMP-touching dispatches require a pitfall-bank consultation (PM dispatch standing rule, PL-ratified 2026-07-12, OT-62)

The concurrency analogue of the driver trap-mining rule above. Any dispatch that **adds or changes
cross-CPU / interrupt / lock / atomic / TLB-shootdown / scheduler-wake code** MUST, before it can be marked
DISPATCHED, **consult `docs/concurrency-pitfalls.md`** (the normative pitfall bank, CP-1 onward — 83 entries as of 2026-10-27; CP-19…CP-28 are the OT-34 AP-idle/cross-CPU-scheduler-handoff class; CP-29 is the real-silicon AP-per-CPU-init buddy-lock/shootdown deadlock, fixed by serialized bringup) and state,
for each pitfall class its change touches: does the change **introduce / guard / leave-N-A** that trap, and
**which discriminator proves it**. New pitfalls learned on the gate are **added back to the bank** (it grows,
it is never pruned — same as the discriminators-stay-armed rule).

This rule exists because the PL asked the right question (2026-07-12: *"these SMP bugs are already found and
squashed in other systems — do we not have the answer out there?"*). We largely **do**, at the level of
pattern/discipline — so the bank harvests the documented TLB-shootdown / IPI / lock-ordering / memory-barrier /
scheduler-wake traps (Intel SDM, Linux/BSD source, OSDev — read for principle, **never copy GPL code into our
MPL kernel**) so a future SMP build is pre-inoculated against the known classes and spends real iteration only
on the genuinely-ours seams. **Bounded honesty:** the answer is out there for the *principle*, not drop-in code
(prior-art fixes are welded to their host's primitives; our bugs manifest at seams we built new — the OT-59
second race lives at *our* spawn/ring-3 seam), and patterns-known ≠ correctness-proven (the seL4 lesson) — the
bank shortens the search, it does **not** replace the AAG / QEMU gate / byte-oracle / kill-switch. A dispatch
lacking the pitfall-bank consultation is mis-specified the same way a driver dispatch lacking trap-mining is.

## Unfamiliar features: survey open source before writing from scratch (PL standing rule, 2026-10-27)

**PL (J.c.), 2026-10-27:** *"For unfamiliar features? Don't write from scratch. First, search GitHub for open-source
projects. Compare approaches, list the licenses and last commit dates of the repos it finds before it borrows
anything, so you don't pull in something abandoned or restrictive."*

This is the general form of the driver trap-mining and concurrency pitfall-bank rules above. It applies to **any
feature new to this codebase**: an app (editor, file manager, terminal), a format or protocol, a subsystem
(installer, power management, login), or an algorithm we haven't built before.

**Before any build of such a feature, the dispatch (or the THINK that precedes it) carries a prior-art survey:**

1. **Search** GitHub, and the ecosystem the feature lives in (crates.io for Rust, the relevant Linux/BSD subsystem),
   for open-source projects that already do it. Prefer ones that fit our shape: Rust, `no_std` or small-dependency,
   and embeddable.
2. **Compare approaches** in a table, one row per candidate. Each row gives:
   - what it does and how;
   - how well it fits our kernel or userland;
   - what we'd take from it: the code, the design, or nothing.
3. **For every candidate, list its license and its last commit date** (with the date checked and the commit SHA),
   plus anything that marks it as alive or abandoned: open issues, releases, archived status.
4. **Decide what to borrow, and how:**
   - **Code may be vendored only under a license compatible with our MPL-2.0 tree:** MIT, Apache-2.0, BSD,
     zlib, ISC, MPL-2.0, Unicode-3.0 (the Unicode data license, e.g. the ICU4X data crates; PL, 2026-10-31) or
     Unlicense/CC0. Keep the upstream license file and copyright, record the upstream URL and
     commit, and vendor under `third_party/<name>/` (the `third_party/acpi-loganfy` precedent).
   - **Build-time Python is allowed (PL, 2026-10-31):** a vendored crate's `build.rs` may run the host's `python3`
     (Stylo's property generator is the case). It must not fetch anything over the network.
   - **GPL, LGPL and AGPL code is never copied.** Read it for the principle only (same as the concurrency rule).
     Code with no license, or a non-OSI license, is treated as all rights reserved.
   - **An abandoned project** (no commits in roughly two years, or archived, or with unmerged security fixes) may
     still be read. If its code is vendored, the PR says who maintains our copy.
   - When nothing fits, say so in the survey; then writing it ourselves is justified.
5. **Exact bytes still come from the raw source.** Any test vector, format layout or table taken from a project is
   fetched with `git`/`curl`, never through a summarizing fetch (the byte-oracle rule).

**The survey goes in the THINK or dispatch, and its table goes in the RETURN or PR body**, so the PM and the
cross-family reviewer can check the license and freshness claims. A dispatch for an unfamiliar feature without the
survey is mis-specified, the same way a driver dispatch without trap-mining is.

**How the PM checks that all three prior-art rules are used (audit, 2026-10-27).** These are the driver
trap-mining rule, the concurrency pitfall bank, and this survey rule:
- **A build sent to a seat by message, not by a dispatch file, still states which of the three rules apply.** The
  PR body carries the section: the trap-mining bank, the pitfall table, or the survey table. Lever 2 (#142) is the
  model: a full CP table in the PR.
- **Every cross-family review relay of a build includes one checklist item:** the prior-art section the build needed
  is present, and its claims hold (licenses and dates, CP classes and discriminators, or traps and their
  discriminators). A missing section is a FAIL of that item.

## NeoPlutarch RAG — Semantic Code Search

A local semantic-search index of this repo is available at http://127.0.0.1:8404. Coders run
in PowerShell, so the commands below are PowerShell.

### When to use

- **Use RAG for:** conceptual/semantic questions — "where is the IPC rendezvous done?",
  "find the capability revoke walk", "show me where TSS.rsp0 is mutated", "what's the boot
  order in `mm::init`?" — any question where you don't already know the file.
- **Use grep instead for:** exact symbol names, exact string lookups, or when you already
  know the file path. Grep is faster and more precise for those.

### Health check (before querying)

    Invoke-RestMethod http://127.0.0.1:8404/health

If it returns `{"status":"healthy","ollama":true,"qdrant":true}`, proceed with queries.

### If it is down or degraded

The **PL manages RAG infrastructure** — starting it is not the coder's job. If you need it
and it's down: **note it in your RETURN and continue with direct file reads** (the RAG is a
convenience, never a blocker — grep + Read always work). If you want to try starting it
yourself (optional self-serve):

    powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # Ollama embeddings (if ollama:false)
    cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API server

### Querying

Truncated snippets (fast overview):

    $body = @{query='per-CPU TSS allocation'; top_k=5} | ConvertTo-Json
    Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"

Full chunk text (when you need the complete code block):

    $body = @{query='IPC rendezvous handoff'; top_k=3} | ConvertTo-Json
    Invoke-RestMethod -Uri http://127.0.0.1:8404/search/full -Method POST -Body $body -ContentType "application/json"

Filter by language or path:

    $body = @{query='page table walk'; top_k=5; language_filter='rs'; path_prefix='kernel/src'} | ConvertTo-Json
    Invoke-RestMethod -Uri http://127.0.0.1:8404/search/full -Method POST -Body $body -ContentType "application/json"

Valid `language_filter` values: `rs`, `markdown`, `toml`, `ld`, `yaml`, `json`, `conf`.

### What's indexed

Rust source (`kernel/src`, `userland`, `tools/xtask`), docs (`docs/**/*.md` — Bible, ADRs,
ANSPECs, phase plans), build config (`Cargo.toml`, `rust-toolchain.toml`), linker scripts,
CI workflows, target-spec JSON, `boot/limine.conf`. Excludes `.git`, `target`, build
artifacts, and binaries. The watcher auto-reindexes on save (2s debounce); force a rebuild
with `python -m src.ingest --full -v` from the RAG root if the index looks stale.

