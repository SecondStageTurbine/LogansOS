<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Deferred-item trigger audit — make sure nothing banked is being silently dropped → Prometheus

**Date:** 2026-07-07
**From:** PM (Claude)
**To:** Prometheus (research / doc lane — host-only). **Branch:** `prometheus/deferred-items-trigger-audit` off `origin/main` HEAD.
**Status:** DISPATCHED — a governance-hygiene sweep of our OWN files. We have a LOT of DEFERRED / BANKED-GATE / OPEN items (OT-1…OT-48, plus ADR "deferred to Phase X" clauses, THINK "follow-on" notes, Bible open-questions). Each was banked with a **trigger** ("do this WHEN X"). The OT-43 logjam just broke and the roadmap is moving (3B about to land → A1 opens), so several triggers are about to fire — exactly when banked work gets silently dropped. **Find everything whose trigger is now met (so we DO it), confirm what's still correctly deferred (and why), and flag anything stale/superseded. Scope: docs research, host-only — NO code, no QEMU. Output: `Plans/RESEARCH-2026-07-07-deferred-item-trigger-audit-FINDINGS.md`.**

## What to do
- **Sweep every deferred/banked/gated item** — primarily `docs/OPEN-THREADS.md` (OT-1…OT-48), but ALSO grep the ADRs, THINKs, and Bible for "deferred", "follow-on", "when … lands", "banked", "gate", "revisit", "open question", "Phase-N", "post-N=1", "future". Don't miss items living outside the ledger.
- **For each, classify against TODAY's state** (OT-43 FIXED; memory arc OT-17 DONE; OT-34 threaded-matmul DONE; A1 fully spec'd — reliability + UX + bitmask-in-kernel ratified + v1=conversation+shell; 3B built and about to gate; SDK-v0 landed; consolidation done):
  - **TRIGGER NOW MET — should be scheduled** (its "when X" condition is satisfied but it isn't queued). THE PRIMARY DELIVERABLE. E.g. does anything key off "when the brain lands" / "when A1 opens" / "when a real interface exists"? List each with the trigger, why it's now met, and a one-line "what the next step is."
  - **STILL CORRECTLY DEFERRED** — trigger genuinely not yet met (name the specific unmet condition, so we know what we're waiting for). One line each — don't over-explain.
  - **STALE / SUPERSEDED** — the rationale changed, it was overtaken by a later decision, or it's a duplicate. Flag for reconciliation (don't fix it here beyond flagging).
- **Watch specifically for the "convergent-frontier" items (OT-22):** OT-15/16/17/18/19/20 and the userland-frontier pillars share a trigger (the multi-principal/untrusted-userland transition). Confirm none is being pulled forward prematurely AND none whose time has come is being missed — OT-22's "one pillar at a time" discipline is the guardrail; check it's being honored.
- **Sanity-check the AAG hardening follow-ons too** (the OT-43 kill-switch-regression + `compile_error!` combo-guard notes; any other "fold into next consolidation" items) — are they tracked somewhere they won't be lost?

## Deliverable
A prioritized FINDINGS: **(1) trigger-now-met list** (the actionable one — what we should schedule, most-ready first), (2) still-deferred (with the unmet condition), (3) stale/superseded (flag). Recommend, don't decide — anything needing a ratification is a PL/PM flag, not a resolution. This is an audit, not a re-litigation of past calls.

## Gate + close
- **Docs-only ⇒ no CI run exists** (paths-ignored) — say so; `scripts/worker-close-gate.sh` host set locally (note: it may run cargo checks that time out on a docs-only branch — skip-with-note is fine, as on the doc-reconciliation sweep). **Diff the RETURN against the committed artifact before sending** (accuracy axis — three clean closes running).
- **Disk reclaim** after merge.

---
## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-07-deferred-items-trigger-audit-prometheus.md` (+ `docs/OPEN-THREADS.md` end-to-end). **Branch:** `prometheus/deferred-items-trigger-audit` off `origin/main` HEAD.
- **One-line:** **Audit every deferred/banked/gated item (OT-1…48 + ADR/THINK/Bible "deferred/follow-on/when-X" clauses) against TODAY's state (OT-43 fixed, 3B about to land, A1 fully spec'd) — find what whose TRIGGER IS NOW MET but isn't scheduled (the primary deliverable), confirm what's still correctly deferred (with the specific unmet condition), and flag stale/superseded.** Watch the OT-22 convergent-frontier pillars (OT-15/16/17/18/19/20) — nothing pulled forward prematurely, nothing whose time has come missed. Recommend don't decide; PL/PM-flag anything needing ratification. Docs-only ⇒ no CI; close-gate host set (skip-with-note if cargo times out on docs-only); diff RETURN vs artifact.
- **RAG:** RAG (semantic code search) is available — and directly useful here for "where did we defer X / where is Y discussed": use it for conceptual questions (faster/more semantic than grep; grep still wins for exact strings like "deferred"). Health-check: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`. If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads (never a blocker). Optional self-serve start (PowerShell): `powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"` then `cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`. Query e.g. `$body=@{query='deferred until the brain lands'; top_k=8}|ConvertTo-Json; Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.
