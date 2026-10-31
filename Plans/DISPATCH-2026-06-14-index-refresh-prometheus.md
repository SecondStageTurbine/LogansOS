<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: INDEX.md refresh — bring the catalog current (Phase 5 → Phase 8) → Prometheus

**Date:** 2026-06-14
**From:** PM (Claude)
**To:** Prometheus (document-cleanup lane; NO kernel code)
**Status:** DISPATCHED. **Pure doc hygiene — `Plans/INDEX.md` only.** Low-risk (INDEX is a navigation aid, "not a dependency for any other artifact" per its own maintenance note), but badly overdue: its newest entries stop at **2026-05-28 (Phase-4 BUILD-1a era)**. It never got updated through Phase 5/6 close, the entire Phase 7 arc, the whole Phase 7.5 connectivity lane, the Arthur amendment, or Phase 8. Bring it current.

## 0. The authoritative sources to reconcile against (do NOT invent status)
INDEX must agree with what's actually on `main`. The current, PM-maintained records are your ground truth — read them and make INDEX match:
- **`docs/bible.md` Article 13** — the phase ledger + its append-log. This is the **authoritative build history** (every build, with commit hashes, status, and the close notes). It is current.
- **`docs/complexity-governance.md`** — the audit log (BUILD-C1 → … → CI-greening) + §4/§5.
- **The `Plans/` folder itself** (`git ls-files Plans/`) and `git log --oneline` — the artifacts that exist and what landed.
Where a build's commit hash or status is uncertain, take it from Article 13 (not memory).

## 1. The scope to capture (the arc since 2026-05-28)
Add/flip entries so the catalog reflects, at minimum:
- **Phase 5** (AI subsystem foundation) — Complete. **Phase 6** (accel driver class, real Intel GPU) — Complete.
- **Phase 7** (governance) — Complete: BUILD-1/2/2L/3, the **4X arc** (4X-1/2/3, device-boundary isolation), the **policy layer** P1/P2/P3/P4a/P4b, the **C1** consolidation, the **ownership arc** O1a/O1b/O2, and the Phase-7 CLOSE-REPORT.
- **Complexity-governance regime** — `docs/complexity-governance.md` + the **I-14** fault-containment pre-mandate.
- **Phase 7.5** (connectivity) — BUILD-0 (fenced DMA arena), A (xHCI/USB core), B (CDC-ECM USB-Ethernet), **K** (consolidation), **V** (VT-d/IOMMU, I-14 fully proven), the branding refresh, **W0** (WPA2 crypto), **W0b** (WPA3 SAE), and **W1** (MT7601U driver — *in flight, Cerebro*). Plus the WiFi-target amendment (Article 24.1: iwlwifi→MT7601U) and the SAE-vector + trap-mine research lanes.
- **The Arthur amendment** — Bible **v1.11.0** (Article 23.1 reversed, new Article 29), the VISION doc reframe.
- **Phase 8** (inference maturity) — the opening THINK, **BUILD-8.1** (decode loop — *in flight, Janus*), the **real-model gap analysis** FINDINGS, the **GGUF-importer** format ratification, BUILD-8.2→8.5 grounded slicing.
- **Cross-cutting** — **BUILD-LO** (the §4 latency oracle), the **CI-greening** + **self-hosted-runner migration** (and its dispatches), the Magistrate easter egg.

For each: a one-line "what it is" + status (`closed` with commit hash for merged builds, `active`/`in-flight` for live ones, `superseded` where applicable), matching the existing INDEX row format.

## 2. The "Open Project Commitments" section
Refresh it against reality:
- **Latency oracle (§4)** → **DISCHARGED** (BUILD-LO, 2026-06-14, ADR-052).
- **I-14 (subsystem fault containment)** → **FULLY PROVEN** (silicon, BUILD-V).
- **CI as a live gate** → restored on a self-hosted runner (note the `[skip ci]`-only-for-docs rule).
- Carry forward the still-open ones (e.g. the bare-metal latency re-read; the W2 on-air supplicant; the `commit_element` W2 forward-assert; any consolidation due).

## 3. Optional secondary (flag, don't force)
If `Plans/` has grown unwieldy with closed/superseded artifacts, **propose** (don't execute unprompted) an archive sweep to `Plans/archive/` like the 2026-05-28 Phase-1–3 archive — but the **primary deliverable is the current INDEX**; archiving is a follow-up only if you flag it's needed.

## 4. Deliverable + gate
Updated `Plans/INDEX.md`, internally consistent and reconciled against Article 13 + the `Plans/` folder. Gate: a fresh reader can find every major artifact and its status from INDEX, and no INDEX entry contradicts Article 13. Doc-only → `[skip ci]` (it's a `.md`/`Plans/` push, paths-ignored regardless).

## 5. Scope guard
`Plans/INDEX.md` only (this is doc cleanup). **Do NOT** touch code, the Bible, ADRs, or other docs; **do NOT** invent build status or commit hashes — reconcile against Article 13 / `git log`; **do NOT** archive/move files unprompted (§3 — propose first). If Article 13 and the `Plans/` folder disagree on something, **flag it in the RETURN** rather than guessing.

---

## Relay — what to tell Prometheus
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-14-index-refresh-prometheus.md`
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-14-index-refresh-prometheus.md
  git show origin/main:docs/bible.md            # Article 13 = the authoritative build history to reconcile against
  ```
- **Branch:** `prometheus/index-refresh` off `origin/main` HEAD.
- **One-line:** Refresh `Plans/INDEX.md` — it's stale back to 2026-05-28 (Phase-4 era). Bring the catalog current through Phase 5/6 close, the full **Phase 7** arc (policy P1–P4b, C1, ownership O1a/b/O2, 4X, close), the **complexity-governance/I-14** regime, the whole **Phase 7.5** lane (0/A/B/K/V/W0/W0b/W1 + branding + the MT7601U amendment + research lanes), the **Arthur amendment** (bible v1.11.0), **Phase 8** (opening THINK, 8.1, gap analysis, GGUF-importer decision, 8.2→8.5), **BUILD-LO**, and the **CI-greening + self-hosted-runner** migration. **Reconcile against `docs/bible.md` Article 13** (the authoritative ledger — don't invent status/hashes) and `git ls-files Plans/`. Refresh the "Open Project Commitments" (latency oracle → DISCHARGED; I-14 → fully proven; CI → live on self-hosted). Doc-only, `[skip ci]`. `INDEX.md` only — flag, don't force, any archive sweep. Deliver the updated `Plans/INDEX.md`.
