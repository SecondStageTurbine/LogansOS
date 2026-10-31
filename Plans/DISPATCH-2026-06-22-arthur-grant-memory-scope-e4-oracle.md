<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Arthur grant ergonomics — Slice E4 (the Memory scope) → Oracle


> **SUPERSEDED 2026-06-22 — NOT BUILT.** PL ruling (`Plans/PL-DECISION-2026-06-22-arthur-memory-sovereignty.md`): Arthur stays sovereign over its own memory (OT-17 D3); the user-gates Memory scope this dispatch describes is dropped. The OT-18 user-grant E-series closes at E3. Kept for the record. See the reconcile THINK (`Plans/THINK-2026-06-22-arthur-grant-memory-scope-e4-reconcile-oracle.md`).
**Date:** 2026-06-22
**From:** PM (Claude)
**To:** Oracle (do this SECOND, after OT-32). New branch off `origin/main` HEAD (after OT-32 merges, rebase on it): `codex/arthur-grant-memory-scope-e4-oracle`.
**Status:** DISPATCHED. **The last E-slice — bring the Memory scope into the grant lifecycle you just completed for files.** E1–E3 wired FileRead/FileWrite through derive→grant→use→re-request→audit→revoke; `entry_to_grant` returns `None` for the **Memory** scope (deferred to E4). E4 makes **Memory** a real grantable/revocable scope, tying OT-18's grant model to **OT-17's per-category memory** (fact/preference/event/relationship — M1/M2 landed, durable + encrypted). Reuse the exact derive/grant/revoke machinery (Phase-2 primitives frozen) — this is the *same* lifecycle applied to the memory caps.

## 0. The decision this build makes
Turn "Arthur's durable memory has no grant-scoped authority model" into "**a Memory scope can be granted, audited, and revoked through the same kernel-cap lifecycle as files**" — so the user controls (and can withdraw) Arthur's access to its memory categories, enforced by the kernel cap (Article 29.3), not by JSON.

## 1. Reconcile-first (the design check — do this before building deep)
OT-17 D3 already envisioned **per-category memory caps (USE-only, derived/revocable)**; OT-18 D2 lists **Memory** as one of the six scope classes. E4 is the *join* of those two. **Before building, confirm the mapping is clean:** a Memory grant entry (`category`) → a `derive_handle` on that category's NeoFS object/dir (the OT-17 M-series memory store) → `ipc::grant` to Arthur → revocable via `revoke_handle` (E3). **If the OT-17 memory-cap shape doesn't map cleanly onto the file-cap derive/grant/revoke path (e.g., memory caps aren't `Handle::File`/`FileDir`-shaped, or the per-category store isn't cap-addressable yet), STOP and surface it** — that's a design beat (a short THINK), not a force-fit. Read:
```
git fetch origin
git show origin/main:Plans/THINK-2026-06-18-arthur-grant-ergonomics-design.md   # D2 (Memory scope), the keystone
git show origin/main:Plans/THINK-2026-06-18-arthur-memory-substrate-design.md     # OT-17 D1–D3: the per-category memory caps + NeoFS objects
git show origin/main:userland/libneo/src/grant_store.rs                            # entry_to_grant (Memory → None today), GrantEntry/ScopeKind
git show origin/main:userland/neo-arthur/src/  (memory_logic / the M1/M2 store)    # the per-category memory the scope grants access to
git show origin/main:kernel/src/sched/{process.rs,file_cap_logic.rs}              # derive_handle/revoke_handle + I-15 file caps (the cap the memory store rides)
```

## 2. ISCs
- **ISC-1 — Memory scope in the grant model.** `entry_to_grant`/`ScopeKind` handle **Memory** (a `Grant::Memory{category, rights}` or equivalent), parsed/serialized by `grant_store` (reuse the E1/E2/E3 store machinery — format additive, small-file equality for existing entries). FileRead/FileWrite unchanged.
- **ISC-2 — Derive + grant + revoke the memory-category cap.** At spawn (E1b path) neo-shell derives a cap on the granted memory category (attenuated, child ⊆ parent) and `ipc::grant`s it; `/revoke <memory-scope>` (E3 path) `sys_revoke`s it, killing the cap + descendants. Compose the Phase-2 primitives — **do not modify them.**
- **ISC-3 — Enforcement is the kernel cap (29.3).** Arthur reads/writes the granted memory category only via the held cap; revoked ⇒ EBADF/EPERM at the kernel even if the store is stale. Attenuation: a read-only Memory grant ⇒ Arthur can't write that category.
- **ISC-4 — kill-switch + standards.** Kill-switch `arthur-grants` off ≡ pre-E (Memory still ungranted) bit-for-bit; the file scenarios (`arthur_grants_persist`, `arthur_grant_revoke`) stay green. **No new `unsafe`/invariant** (composes the existing cap + memory-store machinery — state it; if a genuinely new invariant is needed, STOP and flag). Scope **Memory only** — Process/Network/Model remain deferred (a later E5). SPDX; fmt/clippy via Docker. ADR-060 amendment (E4).
- **ISC-5 — QEMU scenario `arthur_grant_memory_scope` (NEO_SMP=4, kernel-touching).** Grant a Memory-category scope → Arthur accesses that category ✓ → revoke → access → EPERM/EBADF at the real kernel; a *different* memory category Arthur was not granted → EPERM (isolation); attenuation (read-grant ⇒ write EPERM). Every assertion load-bearing on the exit code (no banner theatre — the E3 standard you just set). Log the discriminator (category, cap_fd) to serial.

## 3. Scope guard
The Memory scope through the existing grant lifecycle **only**. **Do NOT**: modify the Phase-2 cap primitives or the OT-17 memory store format (compose them), build Process/Network/Model scopes (later), or change the file scopes. If reconile (§1) shows a design gap, surface it instead of forcing. No new invariant/`unsafe`.

## 4. Merge (PM)
**Close-gate: `scripts/worker-close-gate.sh`** (rebase on OT-32+main LAST; full set; SPDX; self-stat). Kernel-touching ⇒ **4-shard QEMU** + the new scenario; RETURN both CI URLs. PM scrutinizes the attenuation + revoke (security crux) + the OT-17↔OT-18 mapping. PM merges → ledger (**OT-18 E4 complete — the Memory scope is grant/revoke-able; the E-series scope model is complete except Process/Network/Model, banked as E5 if/when needed**).

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main`.

---
## Relay — what to tell Oracle (do SECOND, after OT-32)
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-22-arthur-grant-memory-scope-e4-oracle.md` (+ OT-18 D2, OT-17 D1–D3, `grant_store.rs`, the M1/M2 memory store, `sched/process.rs`+`file_cap_logic.rs`). **Branch:** `codex/arthur-grant-memory-scope-e4-oracle` off `origin/main` after OT-32 merges.
- **One-line:** Build **OT-18 E4 — the Memory scope**, the last E-slice: extend the grant lifecycle you just finished for files (E1b derive/grant + E3 revoke) to **Arthur's per-category memory** (OT-17 M1/M2: fact/preference/event/relationship). **Reconcile FIRST** (§1): a Memory grant → `derive_handle` on the category's NeoFS object → `ipc::grant` → revocable via `revoke_handle`; **if OT-17's memory caps don't map cleanly onto the file-cap path, STOP and surface a design beat — don't force-fit.** `entry_to_grant`/`ScopeKind` handle Memory (reuse the E1/E2/E3 store machinery, additive); enforcement is the kernel cap (29.3 — revoked ⇒ EBADF even if the store's stale); attenuation (read-grant ⇒ no write). **Kill-switch off ≡ pre-E bit-for-bit; compose the Phase-2 primitives + OT-17 store, modify neither; no new `unsafe`/invariant; Memory scope ONLY** (Process/Network/Model = later E5). QEMU `arthur_grant_memory_scope` @ NEO_SMP=4: grant→access ✓→revoke→EPERM, ungranted-category→EPERM, read-grant→write-EPERM, **every assertion load-bearing** (the E3 no-banner-theatre standard). **Close-gate: `scripts/worker-close-gate.sh` (rebase on OT-32 LAST); RETURN push-CI + 4-shard QEMU URLs; PM scrutinizes attenuation + revoke + the OT-17↔OT-18 mapping.**
