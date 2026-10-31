<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Arthur grant ergonomics — Slice E3-revoke (revoke-through-the-derivation-graph) → Oracle

**Date:** 2026-06-22
**From:** PM (Claude)
**To:** Oracle (main-tier; capability/kernel lane). New branch off `origin/main` HEAD (`6e1665d` or later): `codex/arthur-grant-revoke-e3-oracle`.
**Status:** DISPATCHED. **Completes the OT-18 grant lifecycle.** E1a (store) → E1b (spawn-time cap re-derivation, live boundary) → E2 (on-miss re-request) → E3-audit all landed. **E3-revoke** is the last piece: the user can **revoke** a previously-granted scope, and the revocation is enforced **by the kernel** (`revoke_handle` through the capability derivation graph), not by editing a JSON file. The keystone holds: **the kernel cap is the boundary (Article 29.3); `grants.json` is the UX/persistence/audit record, never the enforcement.**

## 0. The decision this build makes
Turn "grants can be created + audited but not withdrawn" into "**a granted scope can be revoked, and a compromised-or-confidently-wrong Arthur immediately loses that authority** — at the real kernel, transitively through anything it sub-derived." Revoke = `revoke_handle` on the cap neo-shell derived for that scope, which walks the derivation graph and kills the cap **+ its descendants**; the store entry is removed so the scope isn't re-derived next spawn.

## 1. References (read ALL before code)
```
git fetch origin
git show origin/main:Plans/THINK-2026-06-18-arthur-grant-ergonomics-design.md  # D6 (audit + revoke via revoke_handle through the derivation graph); D2 scopes; the keystone
git show origin/main:kernel/src/sched/process.rs        # revoke_handle (Phase-2 primitive — COMPOSE it, do not modify), derive_handle, ipc::grant; the derivation graph it walks
git show origin/main:userland/libneo/src/grant_store.rs # E1a: GrantConfig/entries; the store revoke removes the entry from
git show origin/main:userland/neo-shell/src/main.rs     # E1b spawn-wiring: where neo-shell derive_handle's + ipc::grant's the per-scope cap to Arthur (the act revoke reverses); the grant-management command surface
git show origin/main:userland/neo-arthur/src/main.rs    # Arthur holds the inherited per-scope caps; how it uses them in execute_action
# E3-audit (already landed) — the audit log revoke should also record (revoke-the-grant event)
```
Reuse `revoke_handle` (Phase-2, silicon-proven — frozen) and E1a's `grant_store`. You are WIRING revoke into the existing derive/grant/store machinery, not inventing a revoke mechanism.

## 2. ISCs
- **ISC-1 — neo-shell: revoke a scope.** Add the revoke path (a `/revoke <scope>` command or equivalent on the existing grant-management surface): neo-shell looks up the cap it derived for that scope and calls **`revoke_handle`** on it. This is the enforcement action.
- **ISC-2 — revoke-through-the-derivation-graph (the security crux + the discriminator).** `revoke_handle` must kill the derived cap **and every descendant** Arthur derived from it (delegation graph walk — the Phase-2 guarantee). Prove the **transitive** revoke: grant a FileRead scope → Arthur reads ✓ → (if Arthur sub-derived/delegated the cap, that holder also reads ✓) → **revoke** → **both** Arthur's cap **and** the descendant → **EPERM at the real kernel**. A revoke that misses descendants is a privilege-escalation hole — fail the build.
- **ISC-3 — store update is audit, not enforcement.** Remove the revoked scope from `grants.json` (so it isn't re-derived at next spawn) and record a revoke event in the E3 audit log. **But the enforcement is the kernel revoke:** prove the cap is dead **even if the store still listed it** (the boundary is the cap, per 29.3) — i.e. don't rely on the JSON edit for security.
- **ISC-4 — idempotent + minimal.** Revoking an unknown/already-revoked scope is a **no-op success** (not an error). Revoke affects **only** the named scope's cap + descendants — never other live grants (revoke the FileRead-on-X cap, and FileRead-on-Y still works). Fail-safe: a `revoke_handle` error degrades safely (the scope stays as-is, surfaced) — never silently leaves a half-revoked state that looks revoked but isn't.
- **ISC-5 — kill-switch + standards.** Kill-switch `arthur-grants` default-OFF ⇒ no revoke path, behavior ≡ pre-E bit-for-bit. **No new `unsafe`** (composes `revoke_handle`/`grant_store`). **No new invariant** (revoke-through-the-graph is `revoke_handle`'s existing Phase-2 guarantee — state this; if you think you need a new invariant, STOP and flag). Scope stays **FileRead/FileWrite only** (Process/Network/Model/Memory remain deferred — E4). SPDX; `fmt`/`clippy --workspace --all-targets -D warnings` via Docker (kernel clippy blocking). ADR-060 amendment (E3-revoke: the revoke path + the transitive-revoke proof).

## 3. Failure-mode + discriminator (QEMU scenario `arthur_grant_revoke`, NEO_SMP=4 — conclusive on pass AND fail)
1. **Headline:** grant FileRead on a test path → Arthur reads ✓ → revoke → Arthur read → **EPERM** (cap dead at the real kernel).
2. **Transitive (ISC-2):** if Arthur delegated/sub-derived the cap, the descendant also → **EPERM** after revoke (the graph walk reached it). If E-series doesn't expose Arthur-side sub-derivation, exercise the transitive property at the kernel level (derive a child cap in the scenario, revoke the parent, assert the child is dead) so the graph-walk guarantee is proven, not assumed.
3. **Isolation (ISC-4):** a *different* still-granted scope (FileRead on Y) **still works** after revoking X — no over-revoke.
4. **Store-vs-cap (ISC-3):** the cap is dead even if the store entry is stale (enforcement = kernel, not JSON).
5. **Kill-switch:** `arthur-grants` off ⇒ pre-E behavior.
- **Capture-before-you-need-it:** log each revoke's (scope, revoked cap id, descendants-walked count) on the serial console so a missed-descendant failure is self-diagnosing.

## 4. Scope guard
E3-revoke (the revoke path + store removal + audit event + the scenario) **only**. **Do NOT**: modify `revoke_handle`/`derive_handle`/`ipc::grant` (Phase-2 frozen — compose), build E4 (memory-scope) or map Process/Network/Model/Memory scopes, change E1a's store format, or widen the D7 default. No new invariant/`unsafe`.

## 5. Merge (PM)
**Close-gate: `scripts/worker-close-gate.sh`** (rebase LAST; full host set; SPDX; self-stat). Two hard rules: green CI URL (push-CI + the 4-shard QEMU `workflow_dispatch` for `arthur_grant_revoke` — **kernel-touching**), and the PM re-inspects your `git show --stat`. **Expect the PM to scrutinize the transitive revoke (ISC-2) hardest** — it's the security crux. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (**OT-18 E3 complete — grants revocable, boundary enforced through the derivation graph; E4 the only remaining E-slice**).

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main`. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-22-arthur-grant-revoke-e3-oracle.md` (+ the OT-18 THINK D6, `sched/process.rs` for `revoke_handle`, `libneo/src/grant_store.rs`, `neo-shell`/`neo-arthur` grant wiring). **Branch:** `codex/arthur-grant-revoke-e3-oracle` off `origin/main`.
- **One-line:** Build **OT-18 Slice E3-revoke** — the last enforcement piece of the grant lifecycle. Add a neo-shell **revoke** path that calls **`revoke_handle`** on the cap it derived for a scope; **revoke walks the derivation graph and kills the cap + all descendants** (the security crux — prove the *transitive* revoke: grant→read ✓→revoke→Arthur AND any sub-derived holder → **EPERM at the real kernel**). Remove the scope from `grants.json` + log a revoke audit event, **but the enforcement is the kernel cap, not the JSON** (prove the cap is dead even if the store is stale — Article 29.3). **Idempotent** (revoking unknown/already-revoked = no-op success); **minimal** (revoking X never touches a still-granted Y). **Kill-switch `arthur-grants` default-off ≡ pre-E bit-for-bit; no new `unsafe`/invariant** (compose `revoke_handle` — do NOT modify it); FileRead/FileWrite scope only (E4 deferred). QEMU `arthur_grant_revoke` @ NEO_SMP=4 (headline revoke→EPERM; transitive revoke; isolation of other grants; store-vs-cap; kill-switch); log each revoke's (scope, cap id, descendants-walked) so a missed-descendant is self-diagnosing. **Scope: E3-revoke ONLY — not E4, no Process/Network/Model/Memory scopes, don't modify the Phase-2 cap primitives.** **Close-gate: `scripts/worker-close-gate.sh`; RETURN the green push-CI + 4-shard QEMU URLs; PM scrutinizes the transitive revoke hardest.**
