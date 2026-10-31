<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Arthur grant ergonomics — Slice E3-audit (the inspection surface) → Genesis (GLM-5.2)

**Date:** 2026-06-20
**From:** PM (Claude)
**To:** Genesis (GLM-5.2, OpenCode CLI coder). New branch off `origin/main` HEAD: `codex/arthur-grant-audit-e3-genesis`.
**Status:** DISPATCHED. **A bounded, host-only userspace slice — the D6 *inspection* half.** E3 (D6) is audit + revoke; **revoke needs `revoke_handle` on a file cap, which is I-15-blocked (same dependency as E1b), so this slice is the AUDIT/inspection surface only.** "What can Arthur do?" (list the grant set) and "What did Arthur do?" (the action log). Revoke is deferred to when I-15 + E1b land. Pure userspace presentation + logging logic, host-testable.

## 0. The decision this build makes
Give the human a **control surface to see Arthur's authority** — list the grants Arthur currently holds (grouped, human-readable) and the recent actions it took (the single-Arthur audit trail). This is the N=1 "what's it doing / what can it do" window. **Inspection only — no revocation here** (revoke = `revoke_handle` through the file-cap derivation graph, which doesn't exist until I-15; deferred, stated).

## 1. References
```
git fetch origin
git show origin/main:Plans/THINK-2026-06-18-arthur-grant-ergonomics-design.md   # D6 (§8 audit+revoke): conversation primary ("what can you do?"/"what did you do today?") + shell secondary (arthur grants/log); OT-20 granted_by seam
git show origin/main:userland/libneo/src/grant_store.rs                          # E1a/E2: GrantEntry, ScopeKind, build_grant_entries/default_grant_entries (the loaded grant set you list), ARTHUR_HOME, granted_by (OT-20 seam)
git show origin/main:userland/libneo/src/action.rs                              # Action/ActionKind (what the action log records), execute_action_with
git show origin/main:userland/neo-shell/src/main.rs                             # where an `arthur grants`/`arthur log` builtin would hang (thin)
```

## 2. ISCs
- **ISC-1 — List the grant set ("what can you do?" / `arthur grants`).** From the loaded grant entries + the D7 default (E1a's `build_grant_entries`/`default_grant_entries`), produce a listing **grouped by scope** (FileRead/FileWrite/…): a human-readable form (the conversation answer) and a structured form (the shell `arthur grants`). Show `resource`, `rights`, and **`granted_by`** (the OT-20 provenance seam — "user" / "default"). The *logic + formatting* is the deliverable; keep NL rendering thin.
- **ISC-2 — The action log ("what did you do today?" / `arthur log`).** A per-session, **fixed-capacity in-memory ring** of recent action descriptors (`ActionKind` + resource + outcome), appended as `execute_action` runs; list it on request, newest-first, with ring-eviction when full. **In-memory/session only — do NOT add a new persisted on-disk format** (persistence is a later slice; avoiding it keeps E3 off the byte-oracle hook). Each entry carries a principal field (single-Arthur now — the OT-20 multi-principal seam).
- **ISC-3 — Revoke is DEFERRED (I-15-blocked) — state it, don't build it.** Revocation = `revoke_handle` on the parent file cap cascading through the derivation graph; file caps don't exist until I-15. **Do NOT implement revoke** (it'd be a no-op or a fake). Note the seam in code + ADR: "revoke lands with I-15/E1b." E3-audit is inspection-only.
- **ISC-4 — Standards + kill-switch.** **Kill-switch (e.g. `arthur-grant-audit`, or extend `arthur-grants`) default-OFF ⇒ no audit surface compiled, no action-log recording; grants are still enforced, just not inspectable ≡ E1a/E2** (ship the equality regression). No-alloc/no_std-clean where it lands in libneo. No new `unsafe`; **no new invariant** (composes E1a/E2 + action.rs — if you smell one, STOP and flag, like the E1b catch). SPDX. `fmt`/`clippy --workspace --all-targets -D warnings` via Docker. ADR-060 amendment (E3-audit: list + log; revoke deferred).

## 3. Gate (host-only — no QEMU)
- **Host KATs:** grant-set listing (grouping by scope, the D7 default appears, `granted_by` shown, empty/full sets) · action-log (record → list newest-first · ring-eviction when capacity exceeded · the principal field) · **kill-switch equality** (off ⇒ no surface/log ≡ E1a/E2) · builds on AND off the feature.
- `cargo clippy --workspace --all-targets -D warnings` green via Docker.
- **No QEMU** — pure userspace presentation + in-memory logging (the E1a/E2 precedent). State it in the RETURN.

## 4. Scope guard
The list-grants surface + the in-memory action log + the kill-switch **only**. **Do NOT**: implement **revoke** (I-15-blocked — deferred); add a new **persisted** format (action log is in-memory/session); build E1b (spawn-wiring) or E4 (memory-scope); build a real NL/conversation engine (thin rendering of the structured data); change E1a/E2's format/loader/writer; touch the kernel. No new invariant/`unsafe`.

## 5. Merge (PM)
**Close-gate: run `scripts/worker-close-gate.sh`** (rebase-freshness vs current `main` — **the base moves fast; rebase LAST**, right before the gate, so you arrive ff-clean; the exact CI host-checks; SPDX; self-stat). Two hard rules: **"green" without a CI URL is not done** (paste the green push-CI URL — host-only, no QEMU, say so), and the PM re-inspects your commit's own `git show --stat`. **The PM will check the kill-switch equality + that revoke is genuinely deferred (not faked).** PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (OT-18 E3-audit; revoke lands with I-15). 

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main` and confirmed. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Genesis (GLM-5.2)
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-20-arthur-grant-audit-e3-genesis.md` (+ the THINK's D6 §8 + E1a/E2's `grant_store.rs`). **Branch:** `codex/arthur-grant-audit-e3-genesis` off `origin/main`.
- **One-line:** Build **OT-18 E3-audit — the inspection surface** (bounded, **host-only, no kernel touch**). (1) **List the grant set** ("what can you do?" / `arthur grants`): from E1a's loaded entries + D7 default, grouped by scope, showing `resource`/`rights`/**`granted_by`** (OT-20 seam) in a human-readable + a structured form. (2) **Action log** ("what did you do today?" / `arthur log`): a per-session **fixed-capacity in-memory ring** of `ActionKind`+resource+outcome, appended as `execute_action` runs, listed newest-first with ring-eviction; **in-memory only — NO new persisted format.** (3) **Revoke is DEFERRED** — it needs `revoke_handle` on a file cap (I-15-blocked, same as E1b); **do NOT build it**, just note the seam. **Kill-switch default-off ⇒ no surface/log, grants still enforced ≡ E1a/E2** (equality regression). No new `unsafe`/invariant (compose E1a/E2 + action.rs — STOP and flag if you smell an invariant). **Host-only — no QEMU.** **Scope: list-grants + in-memory action-log + kill-switch ONLY — NOT revoke (deferred), NOT a persisted format, NOT E1b/E4, no kernel touch.** **Close-gate: `scripts/worker-close-gate.sh` (rebase LAST — base moves fast); RETURN pastes the green push-CI URL** (host-only, no QEMU).
