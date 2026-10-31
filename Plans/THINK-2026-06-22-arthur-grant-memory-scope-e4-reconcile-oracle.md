<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: OT-18 E4 Memory-scope — reconcile-first (a design beat, surfaced)

**Date:** 2026-06-22
**Author:** Oracle
**Branch:** `codex/arthur-grant-memory-scope-e4-oracle` (off `origin/main` `6cbddb7`)
**Dispatch:** `Plans/DISPATCH-2026-06-22-arthur-grant-memory-scope-e4-oracle.md` — §1 ("Reconcile-first… if the OT-17 memory-cap shape doesn't map cleanly onto the file-cap derive/grant/revoke path, **STOP and surface it** — a design beat, not a force-fit").
**Status:** **RECONCILE — surfacing a beat for PL/PM ruling before building deep.** No production code yet (this is the §1 gate).
**Inputs read:** OT-18 THINK (`THINK-2026-06-18-arthur-grant-ergonomics-design.md` D2/D6), OT-17 THINK (`THINK-2026-06-18-arthur-memory-substrate-design.md` D1/D3), `userland/libneo/src/grant_store.rs` (`entry_to_grant` → Memory `None`), `userland/neo-arthur/src/{main.rs,memory_logic.rs}` (M1/M2 store), `kernel/src/fs/mod.rs:867` (`open_root_temporary`), `kernel/src/sched/{process.rs,file_cap_logic.rs}` (Phase-2 caps + I-15).

---

## 1. The question

E4 is the *join* of OT-17 (per-category memory) and OT-18 (the grant lifecycle): make **Memory** a grantable/revocable scope enforced by the kernel cap (29.3), reusing the E1b/E3 derive→grant→revoke machinery. The reconcile gate asks: **does OT-17's memory map cleanly onto the file-cap path?**

## 2. What DOES map (the cap mechanics are sound)

- **Memory categories are NeoFS files.** `category_path` → `/neofs/.arthur/memory/<cat>.mem` for `fact|preference|event|relationship` (`memory_logic.rs:10,162`). Files ⇒ `Handle::File` ⇒ first-class caps under I-15 (`file_cap_logic.rs`). **Cap-addressable.** ✓
- **derive/grant/revoke apply unchanged.** A cap on a `.mem` file derives/attenuates/grants/revokes exactly like the E1–E3 file scopes — the Phase-2 primitives are kind-agnostic. A QEMU scenario proving *grant → read via held cap → revoke → EBADF + ungranted-category EPERM + read-grant ⇒ write EPERM* on a `.mem` file is buildable **exactly parallel to `arthur_grant_revoke`** (E3). ✓
- **The store model is additive.** `entry_to_grant`/`ScopeKind` can return `Grant::Memory{category, rights}`; the E1/E2/E3 parser/writer/revoke handle it additively (file scopes untouched). ✓

So the *capability mechanics* of a Memory scope are clean. The friction is in the **enforcement target** and the **two subsystems E4 must join**.

## 3. What does NOT map cleanly (the beat) — four points, one load-bearing

### 3a. Sovereignty contradiction with ratified OT-17 (the load-bearing one)
OT-17 D3 (**ratified**, §5.4) makes **Arthur sovereign over its own memory**: *"Arthur holds the parent cap with DERIVE + REVOKE. It decides what to grant and can revoke at any time"* — Arthur owns the `.arthur/memory/` dir cap and derives read-only per-category caps **outward to tools** (a health dashboard, a calendar app). Grantor = Arthur; grantee = a tool.

OT-18 E4 (as dispatched, §0) makes the **user sovereign over Arthur's memory**: *"the user controls (and can withdraw) Arthur's access to its memory categories"* — neo-shell derives a per-category cap and grants it **inward to Arthur**. Grantor = user/shell; grantee = Arthur.

These are **opposite** grantor/grantee directions and opposite ownership models for the *same* store. You cannot have both: either Arthur owns its memory and shares out (OT-17), or the user owns it and gates Arthur in (E4). E4 as written silently **overrides OT-17's ratified sovereignty stance**. That is a ratification decision, not a coder's force-fit.

### 3b. The two subsystems are disjoint features, never compiled together
`arthur-grants = ["arthur-act"]` (neo-arthur `Cargo.toml`) — it does **not** pull `arthur-memory`. The memory store (`MemoryRuntime`, `memory_logic`, the `.mem`/`.key` files) exists **only** under `arthur-memory`; the grant model (file caps, `grant_store` wiring) runs under `arthur-grants`. **No build, and no scenario, compiles both.** E4 must first *join* them (a combined feature/run-path), which is integration work, not the "reuse the exact machinery" the dispatch's tone implies.

### 3c. Memory access is ambient, and the cap-rooted open is explicitly unbuilt
`memory_logic` opens categories via `sys_open(category_path, OPEN_ROOT_TEMPORARY, …)` — **ambient root authority**. `kernel/src/fs/mod.rs:867` is blunt: *"Temporary BUILD-1 root-open ABI… The target model is capability-rooted open: `sys_open(dir_cap, path, …)`"* — i.e. there is **no `openat`/cap-relative open**; `sys_open` never consults a held cap. For revoke to bite (ISC-3: revoked ⇒ EBADF), Arthur's memory store must be **rerouted off ambient `sys_open` onto held cap handles** (as the file *action* surface already is via `ArthurGrantSyscalls`). Achievable to **parity with the file scopes** — but the ambient-`sys_open` bypass is a pre-existing, system-wide gap (a compromised Arthur can ambient-open any path); E4 inherits it, it does not close it.

### 3d. Lazy-create + M2 dual-file break a pre-derived per-category cap
Categories are created **on demand** at first `remember` (`open_category(create=true)` → `sys_open(OPEN_CREATE_REGULAR)`). The shell cannot derive a cap on a `.mem` file that does not exist yet, and there is no `openat(dir_cap, name, CREATE)` to create-through-a-cap. So a held per-category cap can gate **read/write of an existing category**, but not **creation of a new one**. And under M2, each category is **two** objects (`.mem` + `.key`) — one "category cap" must cover both (or be two caps).

## 4. Recommendation

The capability mechanics are sound (§2); the blockers are the **sovereignty ruling** (§3a) and the **subsystem join + ambient/lazy-create reality** (§3b–d). Two ways forward:

- **Option A — lean E4, buildable now (recommended).** Land the Memory scope in the grant *model* + prove the cap lifecycle at the kernel, without rerouting Arthur's live memory store:
  1. `entry_to_grant`/`ScopeKind` → `Grant::Memory{category, rights}`, parsed/serialized/revoked by `grant_store` (additive; host-KAT'd like E1–E3, file scopes unchanged).
  2. QEMU `arthur_grant_memory_scope` (NEO_SMP=4): on a pre-created `<cat>.mem`, *grant → read via held cap ✓ → revoke → EBADF*, ungranted category → EPERM (isolation), read-grant ⇒ write EPERM (attenuation) — every assertion load-bearing on the init exit code (the E3 no-banner standard). This *proves* "Memory is a grantable/revocable kernel-cap scope."
  3. Explicitly **defer** the production reroute (Arthur's `memory_logic` reading its live store through held caps) — it needs §3a's ruling + §3b's subsystem join + ideally the cap-rooted-open primitive. Document the boundary in ADR-060 E4.
  This satisfies ISC-1 + ISC-5 and the cap-mechanics of ISC-2/3, honestly scoping out the part that can't be built cleanly yet.

- **Option B — full E4.** PL/PM **ratify the sovereignty flip** (user-gates-Arthur-memory, superseding OT-17 D3), then join `arthur-grants`+`arthur-memory`, reroute `memory_logic` to held caps, and handle lazy-create (likely by landing the cap-rooted-open / `openat` primitive first — a Phase-2-level addition the E4 dispatch's scope-guard forbids touching). Substantially larger; needs the ratification + probably a precursor build.

**Recommendation: Option A** — it delivers the Memory scope + a real kernel-cap proof now, at exactly the integrity bar E3 set, while surfacing (not force-fitting) the sovereignty + cap-rooted-open questions for an explicit PL/PM ruling. If the PM wants B, that ruling (and likely the `openat` precursor) is the next beat.

**No code written pending the ruling — this is the §1 reconcile gate doing its job.**
