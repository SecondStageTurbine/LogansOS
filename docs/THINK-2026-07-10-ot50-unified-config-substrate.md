<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: OT-50 — The unified config substrate (one typed, capability-gated store; every surface is a VIEW)

**Date:** 2026-07-10
**Author:** Oracle (main-tier coder — design lane)
**Dispatch:** `Plans/DISPATCH-2026-07-10-ot50-unified-config-substrate-think-oracle.md`
**Branch:** `codex/ot50-config-substrate-think-oracle` → this file
**Status:** Design THINK — host-only, no code, no QEMU. **Design-ahead-of-build:** settle the store's *shape* for PL ratification so the first real config-surface build is designed-right, not bolted-on. Recommend; the PL ratifies.

**Inputs (read firsthand):**
- `Plans/RESEARCH-2026-07-08-beginner-poweruser-os-substrate-FINDINGS.md` — P1 (one config substrate, two views) + the four banked lenses (P2/P3/P4/P5).
- `docs/OPEN-THREADS.md` — **OT-50** (the banked summary + the PL ruling) and **OT-51** (Cerebro's parallel update/rollback THINK — the seam I compose with).
- `docs/THINK-2026-07-08-poweruser-neo-shell-duality-design.md` (Prometheus) — §6 already states the anti-split-brain UX contract and §10 Fork 2 explicitly flags OT-50 as the unbuilt piece this THINK answers.
- The conforming seed, read in the code: `userland/libneo/src/grant_store.rs` + `docs/ADR-060` (grants.json); `userland/neo-arthur/src/reversibility_logic.rs` (stakes.json); `docs/ADR-061` (I-15 file caps); `docs/ADR-069` (posture / `/posture`).
- Bible Article 29.3 ("model proposes, capability disposes"), Article 1.5/1.6.

---

## 0. The one idea (everything below is an instance of it)

**There is ONE config store.** Arthur saying *"I've turned that off, sir"* and `neo> config get <key>` must **provably read the same key** — no hidden second registry, no *"Arthur's settings"* vs *"system settings."* Arthur, the `neo>` power path, and the future Atrium GUI are all **VIEWS over one typed, capability-gated store**; depth for a power user is *more keys visible*, never *a different mechanism*.

The verified positive prior art is macOS `defaults`: the GUI preference panels and the `defaults` CLI edit the **same** per-domain typed key-value store. The verified negative case is Windows's split-brain (registry vs the still-incomplete Settings/Control-Panel migration). Today's small surfaces — `grants.json`, `stakes.json` — already conform (owner-readable files Arthur mediates). This THINK generalizes that seed into the substrate every future config rides.

The whole design reduces to one sentence, lifted from the FINDINGS and made structural: **one substrate, N views, where the owner — never the system — chooses when to go deeper, no view holds a divergent copy, and opening the power path never turns an invariant off.**

---

## 1. What already exists — the conforming seed (design from reality, not first principles)

This is not a greenfield. Two config surfaces ship today, and they already embody the hard half of the one-store thesis. The design's job is to *name the shared mechanism they are both instances of*, then add a general domain — not to invent a new system.

### 1.1 `grants.json` — the strict, typed, cap-gated, byte-oracle'd seed (ADR-060, `libneo::grant_store`)

- **On-disk:** `.arthur/grants.json`, versioned restricted-JSON: `{ "version": 1, "grants": [ { "scope", "resource", "rights", "granted_by" } ] }`.
- **Typed:** `scope` is a fixed enum of six resource classes (`FileRead`/`FileWrite`/`Process`/`Network`/`Model`/`Memory`); `rights` is a `FileRights` bitset over `{read, write, lookup, readdir}`. A no-alloc, zero-copy parser (`parse_grants`) + writer (`serialize_grants`).
- **Independent byte oracle:** the format is pinned by a **hand-authored fixture asserted field-by-field against the loader** — never a writer↔reader round-trip (ADR-060's "Authoritative worked example"; the CLAUDE.md byte-oracle rule).
- **Kill-switch:** `arthur-grants`; `GrantConfig::Disabled` ⇒ no durable grants, **bit-for-bit ≡ pre-E1**. Malformed store ⇒ D7 least-privilege default (fail-safe: never crash, never widen authority).
- **The load-bearing boundary (Article 29.3):** `grants.json` is **NEVER the authority.** The kernel capability Arthur holds is the enforcement boundary *by construction*. The store is the *userspace policy mirror* that decides which kernel cap `execute_action` presents. A compromised Arthur that fabricates entries can't exceed its held caps — a grant with no backing kernel cap still `EPERM`s at the syscall (proven: `grant_in_set_without_kernel_cap_still_eperm`).
- **Already two views, one store:** neo-arthur (the conversation view) loads it; neo-shell surfaces it via the `arthur grants` builtin and `/revoke <resource>` (the deterministic view). **Both mediate the same file.** This is the pattern OT-50 generalizes — it is not aspirational, it is running.

### 1.2 `stakes.json` — the owner-set policy seed (OT-19 D3, `reversibility_logic.rs`)

- **On-disk:** `/neofs/.arthur/stakes.json`, an **owner-set** policy JSON. `stakes_policy_requires_confirm(policy_json, action, path)` reads a `confirm_destructive` bool plus per-action / `paths` string-lists.
- **Adaptable, not adaptive (P2 already honored):** the owner sets the file; Arthur applies it **mechanically**. Arthur never rewrites its own friction policy from observed behavior. This is the never-adaptive lens, shipped.
- **A rigor wart to note:** unlike grants.json's strict typed parser, stakes.json uses a **lenient substring reader** (`find_json_key`/`json_bool`/`json_list_contains`). It can be fooled by a key that appears as a substring, and it has no byte oracle. When it migrates into the unified store (§6c) it should be *raised to grants.json's rigor bar*, not left permissive.

### 1.3 I-15 — file handles are first-class capabilities (ADR-061), the gating primitive we reuse

- `Handle::File` is in the derive/grant/revoke/inherit graph. `FileRights` stays authoritative for file-op semantics; the mapping surfaces cap meta-rights (`DERIVE`/`GRANT`/`REVOKE`) and lets `with_cap` attenuate. **I-15:** a derived/granted/inherited file cap's rights are a subset of its parent's.
- **The two errnos, by design:** a **read-only** handle attempting write is denied **`EACCES` (-13)** (the file-rights gate); a **cap-graph** refusal (deriving a right the parent lacks, or without the meta) is **`EPERM` (-1)**. The config store rides these unchanged — it introduces no new cap primitive.
- **Kill-switch:** `file-caps`; off ≡ pre-unification bit-for-bit.

### 1.4 `/posture` — the never-adaptive lens, already enforced structurally (ADR-069)

`/posture [yolo|cautious|off]` is a **deterministic user command intercepted before model decode.** There is **no posture production in the constrained grammar**, so the model structurally cannot switch its own posture. Posture is **session state, never serialized** (no byte oracle owed). This is the template for the most safety-critical config keys (§5, lens 1): owner-command-only, not model-emittable.

### 1.5 The duality THINK already wrote the contract this THINK must satisfy

Prometheus's neo-shell duality THINK §6 states the UX contract verbatim — *"Arthur saying 'I've turned that off, sir' and `neo> config get` must show the same key"* — and §10 Fork 2 flags the gap precisely: *"v1 has no unified config store yet… Arthur mediates grants.json/stakes.json as files; neo> reads them as files… no typed config store with atomic read/write. This is P1 from the substrate FINDINGS — banked for PL ratification."* **OT-50 is the answer to that flag.** It also fixes neo>'s command→`ActionKind` "one source of truth" rule as the mechanism the config commands ride.

---

## 2. Prior art, and how I acquired it (the acquisition rule, honored)

**Provenance note (read first).** The dispatch's acquisition rule forbids routing exact prior-art claims through a summarizing `WebFetch` (it mangles exact claims the way it poisons a byte-oracle). I did **not** run a summarizing fetch. The Windows and macOS claims below rest on two directly-trustworthy sources: (a) the FINDINGS doc's **3-vote adversarial verification against primary sources** (Microsoft's own docs quoted *verbatim*; macos-defaults.com), re-verified 2026-07-08 primary-sources-only; and (b) stable, canonical knowledge of the `defaults(1)` man page + Apple's CFPreferences/property-list type system, which the FINDINGS' claim #19 and the duality THINK independently corroborate. I introduce **no new exact quote** beyond what the FINDINGS already verified. Where I lean on the `defaults` type/domain model for design shape (§3), it is design prior-art (how a store is structured), not a byte-exact fixture — so the mangling failure mode does not apply, and I attribute it plainly rather than fabricate a fresh quote.

### 2.1 macOS `defaults` — the positive case (one store, two views, typed, per-domain)

The `defaults` system is a set of **per-domain** typed key-value stores (an application's domain is keyed by its bundle identifier; a system-wide `NSGlobalDomain`/`-g` domain holds cross-app defaults). Values carry **declared types** from the property-list type lattice (boolean, integer, real, string, and the collection types), so the GUI can render the right control and the CLI can validate. **The GUI preference panels and the `defaults read`/`defaults write` CLI edit the same domains** — including keys with no GUI exposure at all. The GUI user and the terminal user edit the same truth; depth is *more keys visible*, not *a different mechanism* (FINDINGS P1 / claim #19, macos-defaults.com, 3-0).

The load-bearing lesson for us: **per-domain partitioning is NOT split-brain.** One *mechanism* + one *namespace* whose domains are partitions is exactly the verified positive case. Split-brain is *two different mechanisms* (or two copies of one key), not one mechanism with many domains.

### 2.2 Windows — the negative case (two mechanisms, mutate-in-place, danger as the power path)

Verified in the FINDINGS against Microsoft's own docs: the **Settings app vs Control Panel** migration is *still incomplete in 2025-26* (Microsoft: *"The Control Panel is in the process of being deprecated in favor of the Settings app"*; some operations still route to Control Panel only) — a 13+-year-old split still shipping piecemeal. The **registry** mixes user prefs with hardware/security state in one store, edited *"at your own risk"* with a reinstall warning (learn.microsoft.com). And an OS-pushed feature update once **deleted user files** and was pulled (Oct-2018 1809). The composite anti-pattern: **split mechanisms + mutate-in-place + the power-user config path carries a reinstall warning instead of being a safe first-class interface.** Every rule below is a refusal of one of these.

---

## 3. ISC-1 — the store shape

**Recommended shape: a typed, per-domain, file-backed, capability-gated key-value substrate — `libneo::config_store` — that generalizes the grants.json/stakes.json pattern.** Not a monolithic single-file registry (the Windows anti-pattern); not a brand-new mechanism parallel to the existing files (that *is* split-brain).

### 3.1 Typed

A config value has a **declared type** drawn from a **small fixed scalar lattice**: `bool`, `int` (bounded), `string` (bounded length), `enum` (a closed set of tokens), and `bounded-list` (of the above). Each **domain declares a schema**: `key → { type, constraints, disclosure-level, mutability }`. This is the `defaults`/plist per-value-typing model, deliberately restricted below plist's full richness (no `data`/`date`/arbitrarily-nested-dict in v1) so the parser stays no-alloc and the byte oracle stays tractable. `grants.json`'s record schema is a *richer specialized domain schema*; the general `settings` domain uses the simple lattice. Typing is what lets `config set` validate, `config get` round-trip, and a future GUI render a checkbox for a `bool` and a stepper for a bounded `int`.

*Contrast with today:* grants.json is already this rigorous; **stakes.json is not** (§1.2) — the unified store's decode discipline is the strict one, and stakes.json converges up to it (§6c).

### 3.2 Capability-gated — per-namespace (per-domain), riding I-15 unchanged

A **domain = one backing file = one file cap.** A surface that holds a `READ` file cap on a domain can read its keys; a `WRITE` cap (`FileRights::WRITE`) lets it mutate them; the write-denial is the **existing `EACCES`**, the cap-graph refusal the **existing `EPERM`**. No new cap primitive, no finer-than-file granularity invented — this is precisely how `grants.json` is gated today (Arthur holds a `FileRead` cap on `.arthur/`). Per-*key* gating is deferred to a future finer-grain build, needed only if a single domain ever mixes sensitivities; per-domain is right for v1 and matches `defaults`' per-domain grant unit.

The enforcement is the kernel file cap; the store contents are the **policy mirror** — the *same* Article-29.3 split grants.json already lives under, now generalized: **editing a config file by hand can never grant authority the kernel doesn't already enforce.** (This is what makes it safe to keep the files owner-readable — §5, lens 4.)

### 3.3 File-backed — extend the `.arthur/` owner-readable-file pattern

Domains are NeoFS files under **one namespace root** (`/neofs/.arthur/config/`), alongside the seed domains. Reads are **pull-through** (open → decode), writes are **write-through** (encode → write → close), so the file *is* the truth — no surface holds a memoized snapshot that can drift. (**PM correction, 2026-07-10:** this THINK stated NeoFS has no truncate primitive — that is now inaccurate. `NeoFsInode::truncate` landed on `main` as `add51e5` (2026-07-04, from `codex/neofs-truncate-janus`; validates rights/kind/size then `commit_regular_truncate`). So a shorter rewrite should **truncate directly**, not pad with trailing whitespace. The write-through discipline is unchanged; the build should use `truncate`. *Original claim, preserved: "NeoFS has no truncate primitive; a shorter rewrite pads with trailing whitespace the parser skips — the discipline grants.json's `/revoke` rewrite used."*)

### 3.4 The one-store invariant, stated precisely — and what structurally forbids a second registry

> **The one-store invariant.** There is exactly one config *mechanism* (`config_store`) and one *namespace root* (`/neofs/.arthur/config/`). Every config read or write by every surface (Arthur, `neo>`, the future Atrium) resolves to a `config_store` read/write of a domain file under that root, gated by a file cap. No surface has a private store; no surface caches a divergent copy; there is no in-kernel config table and no alternate serialization.

Four structural guarantees make "a second registry appears" impossible-by-construction rather than forbidden-by-policy:

1. **Config values live ONLY in files under the one root.** There is no other place a config value *can* be — no kernel table, no per-surface struct, no env-style side channel. A surface literally cannot obtain a config value except by opening a domain file, which requires a file cap.
2. **One gate for all surfaces.** Arthur's `execute_action` cap-check and neo>'s `execute_action` cap-check are the *identical function* over the *identical* NeoFS objects (already true for file/process/grant ops per the duality THINK §6). The config commands are new `ActionKind`s (`ConfigGet`/`ConfigSet`) through that same check — not a new pathway.
3. **No divergent cache.** Pull-through reads + write-through writes mean the file is the sole copy. A surface *cannot* answer `get` from a private snapshot because it has none.
4. **A grep-checkable + assertion guard, in the I-14/I-15 tradition.** The invariant's guard is: `config_store` is the *only* module that opens files under the namespace root, and no surface embeds its own settings storage. This is enforced the way SPDX/I-15's mapping-drift are — a grep guard (no direct `open("…/config/…")` outside `config_store`) plus a debug assertion on the namespace prefix. A "second registry" would have to route config through a file outside the root, which the guard flags at review the same way a missing SPDX header fails CI.

Per-domain partitioning does **not** violate this — domains are partitions of the one namespace reached through the one mechanism (the verified `defaults` positive case), not parallel registries.

---

## 4. ISC-2 — the VIEWS model (the load-bearing part) + the falsifiable anti-split-brain property

Three views, one store. Each view is a *renderer* of the same domain files through the same cap-checked API; none owns a store.

| View | Read path | Write path | What "Arthur turned it off" means |
|---|---|---|---|
| **Arthur** (conversation) | `ConfigGet` `ActionKind` → `config_store::get(domain, key)`, cap-checked in `execute_action` | `ConfigSet` → `config_store::set(...)`, write-through, cap-checked | Arthur executed `set(D.K, false)` on the owner's word → wrote the domain file |
| **`neo>`** (deterministic) | `neo> config get <D.K>` → same `ConfigGet` → same `execute_action` cap-check → same file | `neo> config set <D.K> <v>` → same `ConfigSet` | Same mechanism, plain-shell voice |
| **Atrium** (future GUI) | a cap-surface renders the domain schema (checkbox per `bool`, etc.), reading via `ConfigGet` | toggling a control calls `ConfigSet` | Same file, same cap; the GUI is a renderer holding a file cap like any tenant |

neo>'s commands map to the `ActionKind` enum as the single source of truth (the duality THINK's rule), so `neo> config get/set` and Arthur's `ConfigGet/ConfigSet` are the *same* actions with the *same* cap-check by construction — there is no separate "shell config" code that could drift from "Arthur config."

### 4.1 The falsifiable anti-split-brain property (this is where the guarantee lives)

> **Property (read-back identity).** For any domain `D`, key `K`, value `V`: after **any** surface commits `set(D.K, V)`, **every** surface's `get(D.K)` returns `V` — in the same session and across a fresh surface spawn — because all views decode the *identical* NeoFS domain file through the *identical* `config_store` decode, and no view holds a private cache.

**How it's tested (the build owes these — host + QEMU):**
- **Cross-surface, same session (the split-brain killer):** Arthur `set(settings.telemetry, false)` → `neo> config get settings.telemetry` returns `false`. **Negative control:** if any view memoized its own copy, this read-back diverges → the test goes red. This is the exact analog of grants' `grant_in_set_without_kernel_cap_still_eperm` and I-15's `child ⊆ parent` KATs — a property proven by a test that *can* fail on the wrong implementation, not a tautology.
- **Persistence across spawn:** set → fresh Arthur/neo> spawn → `get` returns the value (write-through hit disk).
- **QEMU `config_one_store` scenario:** at `NEO_SMP=4`, one surface sets, another gets the same value, read-back identical; cap-denied write is `EACCES`; the discriminator line (`domain`, `key`, `cap_fd`, `value`) makes a failure self-diagnosing.

### 4.2 The one honest edge — concurrent writers (flag, don't solve in v1)

Two surfaces writing the *same* domain file concurrently could race (last-writer-wins / a torn write on a crash mid-rewrite). v1 is effectively single-writer (one interactive owner), so this is not a v1 correctness bug — but the design should note that a domain write ought to be **atomic** (write-new-then-swap, or the NeoFS snapshot primitive banked in OT-19 D2) so a crash mid-write can't corrupt a domain. This ties directly into OT-51's atomicity discipline (§6a) and is flagged there, not solved here.

---

## 5. ISC-3 — the four banked lenses, embodied (the store *enforces* them, not just cites them)

**Lens 1 — never-adaptive-surface (Findlater & McGrenere, CHI 2004).** The store never self-morphs a UI **and never self-writes a value.** A config value changes *only* on an explicit owner action: an owner instruction Arthur relays (`ConfigSet`, logged in the action log), a `neo> config set`, or a GUI control the owner operated. There is **no code path that writes a config value from observed behavior** — enforced as a grep guard (no `config_store::set` call outside an owner-action handler), the structural mirror of stakes.json being owner-set-and-mechanically-applied today. The most safety-critical keys (posture, guardrail toggles) go further: **owner-command-only, not model-emittable**, the `/posture` template (§1.4) — the model can *suggest* "want me to turn X off?" but the *value* moves only on the owner's word. Suggest, never silently rearrange.

**Lens 2 — promotion-affordance (Shneiderman multi-layer; Apple HIG disclosure).** Layer-2 keys are **visible-but-quiet from layer-1 via disclosure states on ONE surface, never two parallel surfaces.** Each domain schema tags every key with a **disclosure level** (layer-1 essential vs layer-2 advanced). Arthur (layer 1) surfaces essential keys and, after a related action, names the deterministic equivalent — *"you can also `neo> config set …`"* — the disclosure button (the duality THINK's exact pattern). `neo> config get <domain>` lists essentials; `--all` reveals advanced keys (the twistie). The GUI shows essentials with advanced behind a disclosure control. **One store with per-key disclosure metadata — never a "basic.json" vs "advanced.json" split.** Depth = more keys visible, same mechanism.

**Lens 3 — explain-never-vanish (OT-44 H2 catalog/reject + butler register).** A declined or failed config write gets a **stated reason + a pointer**, never a silent redirect or silent no-op:
- *cap-denied:* names the exact missing cap + the grant path — `neo>`: `Denied: config set settings.x requires FileWrite on .../config/settings — grant it with: grant filewrite <domain>`; Arthur (butler voice): *"I haven't been given leave to change that yet, sir — a word and I'll have it."*
- *schema-invalid:* states the type/range — `settings.max_foo takes an integer 1–10; got "blue"` — never silently clamps or drops.
- *immutable key:* states it's immutable and why (e.g. a factory-locked invariant key), never silently ignores the write.

The store **never** silently substitutes or discards a write — the direct refusal of Windows's silent-substitution anti-pattern (FINDINGS P4).

**Lens 4 — power-is-not-a-jailbreak (ChromeOS lesson, P5).** More visible keys = **more granted capabilities, never fewer enforced invariants.**
- There is **no config key that disables the cap-check**, no "developer mode" key, no god-key that grants all caps — the substrate simply has no such key (as `neo>` has no `sudo`).
- Reaching advanced/sensitive keys = the owner granting the surface *more file caps to more domains*, never a posture that turns an invariant off.
- **Even the rawest power path — a power user hand-editing the owner-readable domain file — cannot escalate**, because the file is the *policy mirror*, not the authority (§3.2, Article 29.3 generalized): hand-editing `grants.json` can't grant a cap you don't hold; hand-editing a `settings` domain can't disable a guardrail the kernel enforces. This is *why* the store is safe to make owner-readable/editable — the property Windows's registry lacks (where the raw power path is exactly the dangerous one).

---

## 6. ISC-4 — the seams (compose, don't collide)

### 6a. The OT-51 rollback seam — config **spans both**: defaults *inside* the base, owner changes *beside* it as reversible deltas

> **PM note — SUPERSEDED by the ratified cross-model correction (2026-07-10; see OPEN-THREADS.md OT-50 seam clause).** A Qwen independent check (`docs/THINK-2026-07-10-config-rollback-seam-crossmodel-prometheus.md`) caught a shared same-model blind spot: **config is *identity, not state*, so it does NOT ride OT-51's reversible-delta ledger** as this section frames it. RATIFIED contract: config lives **beside** the deployment with its **own** independent append-only history, coupled to the system only by a monotonic schema contract; settings survive rollback; unknown keys are ignored-**and-preserved** (surgical write-back). Read the ledger seam clause + the cross-model THINK for the authoritative shape; the analysis below is retained as the design record.

OT-51's substrate move is *never mutate the running system — build the next state as a verifiable artifact beside the current one, flip a boot pointer, keep the old bootable*, with the power path expressed as **recorded reversible deltas on the pristine base**. So "roll back the system" and "keep my settings" need a defined interaction. Recommendation:

- **Factory/default config lives INSIDE the immutable base** (the schema + factory values), so a fresh boot with zero owner deltas has sane, complete config.
- **The owner's config is a layer of recorded deltas BESIDE the base**, in OT-51's *same* delta ledger — config deltas are a *species* of the same reversible-delta the OS layering uses. This is not a new subsystem; it is OT-51's ledger carrying one more kind of delta.
- **The defined interaction:** *"roll back the system"* = flip the OS-image pointer **while replaying the owner's config delta layer onto the new base** → settings survive an OS rollback (the explicit refusal of Windows's update-deleted-my-files anti-pattern). *"roll back my config"* = revert config deltas independently (OT-19 per-setting undo) without touching the OS image. **Two axes, one delta-ledger discipline.**
- **The genuine tension to hand to Cerebro's THINK (PM synthesizes):** a config delta may reference a key an *older* rolled-back base's schema doesn't define. So the config delta layer must be **schema-version-tagged** and **degrade gracefully against a mismatched base** — an unknown key is *held-but-inactive*, never a crash, exactly the forward-compat rule grants.json already has (unknown version/key → fail safe). I state this seam from my side; the config-in-base-vs-beside split and the schema-mismatch degradation are the OT-51 composition points for the PM to reconcile with Cerebro.

### 6b. The OT-17 memory boundary — config is NOT Arthur's memory

Both live under `.arthur/`, but they are different namespaces with different authorship and ownership. The clean test: **who authors it, and who holds the edit right?**

| | **Config store (this THINK)** | **OT-17 memory** |
|---|---|---|
| Content | declarative settings: toggles, policies, preferences | learned context: conversation history, learned facts, working context |
| Author | the **owner** | **Arthur** |
| Edit right | the owner edits; Arthur applies mechanically | Arthur curates; the owner's control is **forget / crypto-erase**, not per-key editing |
| Adaptivity | never learned/auto-set (lens 1) | accumulated over time by design |
| Precedent | stakes.json is the archetype (owner-set policy) | OT-18 E4 disposition: `Grant::Memory → None`; memory is Arthur's, sovereign (OT-17 D3) |

A config key is never "learned"; a memory is never a key the owner toggles. (A third neighbor under `.arthur/` is the reversibility ledger — `.arthur/undo` prewrite images — which is *neither* config nor memory; noting it so the three aren't conflated.)

### 6c. grants/stakes are the conforming seed — generalize, don't replace their semantics

The store **generalizes** the two existing files; it does not change what they mean.
- **grants.json** stays the cap-graph policy mirror — a `set` still records a durable grant; the enforcement stays the kernel cap; the config store never touches that boundary.
- **stakes.json** stays the reversibility confirm-policy — its keys still drive `stakes_policy_requires_confirm`.
- **What unifies is the *access path*, not the semantics:** both become **domains** of the one `config_store` mechanism — same namespace root, same per-domain cap-gating, same VIEWS access (Arthur + neo> + GUI through the one API), same explain-never-vanish + never-adaptive rules.
- **The additive migration (kill-switch-clean):** (1) add the new general `settings` domain through `config_store`, leaving grants/stakes *exactly as they are* on disk; (2) later, optionally re-express grants/stakes access *through* the config API as specialized domains — keeping their exact on-disk format, semantics, and byte-oracle — so there is provably ONE mechanism. **When stakes.json migrates, raise its lenient substring reader to grants.json's strict typed decode + a byte oracle** (§1.2 wart closed).

---

## 7. ISC-5 — format + byte oracle

The store **does** have an on-disk byte layout, so the standing rule applies (an independent hand-laid byte oracle, not a round-trip). Recommendation: **the general `settings` domain reuses grants.json's proven format discipline** — versioned restricted-JSON, no-alloc/zero-copy parser + writer, strings without escape sequences, unknown keys rejected, malformed → the safe default.

**Recommended layout (worked example — a 2-key `settings` domain):**

```json
{ "version": 1, "domain": "settings", "keys": [
  { "key": "telemetry_enabled",  "type": "bool", "value": false },
  { "key": "confirm_destructive", "type": "bool", "value": true }
] }
```

**The oracle table (the hand-laid decode the build asserts field-by-field, ADR-060 style):**

| Input (bytes in the domain file) | Loader output (`config_store::parse` ⇒ `ConfigEntry`) |
|---|---|
| `"version": 1` | (top-level) accepted; any other value ⇒ refuse → default |
| `"domain": "settings"` | domain tag `settings`; mismatch vs the opened path ⇒ refuse |
| `{ "key":"telemetry_enabled", "type":"bool", "value":false }` | `ConfigEntry { key:"telemetry_enabled", ty: Bool, value: Bool(false) }` |
| `{ "key":"confirm_destructive", "type":"bool", "value":true }` | `ConfigEntry { key:"confirm_destructive", ty: Bool, value: Bool(true) }` |
| a `"type":"int"` entry whose `"value"` is out of the schema's range | rejected at load with the stated reason (lens 3) — never clamped |

**The build owes the asserted oracle.** Per the design-ahead posture, this THINK proposes the *layout + worked decode*; the first `config_store` build must ship the **fixture asserted against literal expected values** (the writer+reader-agree-wrongly trap — a round-trip is not sufficient). I hand-laid the example above myself (our own format — no fetch involved), but the binding, exhaustive oracle is a build deliverable, called out here so it cannot be skipped. Type mismatches, out-of-range values, unknown keys/versions, and escape-sequence rejection each get a KAT, mirroring grants.json's format-rejection suite.

---

## 8. ISC-6 — smallest-first additive path + kill-switch

**The migration is additive, not a rewrite** — today's files already conform, so we add a mechanism and a general domain, we don't replace a registry.

**Sequence:**
1. **`libneo::config_store`** — typed per-domain `get`/`set` over the scalar lattice + the byte oracle + the kill-switch. **Host-testable**, exactly like grant_store E1 (no kernel touch). Includes the general `settings` domain schema with **one real, useful key** (candidate: a `telemetry_enabled=false` toggle, or lifting `confirm_destructive` here from stakes) — enough to prove the whole thesis on one key.
2. **`ConfigGet`/`ConfigSet` `ActionKind`s + neo> `config get/set`** wired through `execute_action`'s cap-check to the domain file at spawn. **Kernel-touching** (new ActionKind + cap-gated file access across the spawn boundary), **QEMU-gated**, exactly like grant_store E1b.
3. **The anti-split-brain gate:** the §4.1 read-back-identity host KATs + the `config_one_store` QEMU scenario (Arthur sets → neo> gets the same value → read-back identical; cap-denied write = `EACCES`).
4. **Generalize:** migrate stakes.json (raising it to strict decode), and optionally re-express grants.json access, as domains of the one store.

v1 delivers **two** views (Arthur + neo>) — the Atrium GUI is *designed-for* (§4) but built when the GUI phase opens (OT-52). Designing the third view now is what keeps it from being bolted on later.

**Kill-switch (`config-store` feature) — disabled ≡ today's scattered small surfaces, bit-for-bit.** Off ⇒ no general `settings` domain, no `config get/set` command or ActionKind; grants.json and stakes.json are read *exactly as today* (the grant_store loader and reversibility's stakes reader, unchanged). The **only** difference off→on is the feature flag — the `GrantConfig::Disabled` equality pattern generalized. So "disabled ≡ the previous layer, bit-for-bit" holds: off = today's world where config is scattered small files; on = the same files reachable through one API plus a general domain. The equality is a host KAT (feature-off build compiles out the config surface; the seed files behave identically).

---

## 9. ISC-7 — honest forks for PL ratification (recommendation + reason each)

The store shape is explicitly gated on PL ratification (the OT-50 row). The genuine forks:

**Fork A — dedicated store vs generalized-files.** ➜ **Recommend: generalized per-domain file-backed store** (`config_store` wrapping the `.arthur/config/` namespace; grants/stakes become seed domains). *Reason:* additive + kill-switch-clean (no rewrite), matches the verified `defaults` per-domain positive case, refuses the Windows monolithic-registry anti-pattern, and **builds nothing new** where it can reuse (I-15 caps, NeoFS files, grants.json's format discipline). A monolithic single-file registry is rejected (mixes concerns, the Windows failure); a brand-new mechanism parallel to the existing files is rejected (that *is* the split-brain we're preventing).

**Fork B — type-system strength.** ➜ **Recommend: per-key declared types from a small fixed scalar lattice** (`bool`/`int`/`string`/`enum`/`bounded-list`), per-domain schema. *Reason:* strong enough to validate + render a GUI control per type and to give `config get/set` a real contract; small enough for a no-alloc parser and a tractable byte oracle; matches plist's proven per-value typing without its unbounded richness; and it raises stakes.json's untyped-permissive read to rigor. Rejected: untyped strings (too weak — no validation, no GUI rendering, the stakes.json wart); full plist richness (unbounded, breaks no-alloc + the byte oracle).

**Fork C — per-key vs per-namespace caps.** ➜ **Recommend: per-namespace (per-domain) cap-gating, riding I-15 file caps unchanged** (a domain's backing file is the cap unit; `WRITE` cap to mutate; write-denial = `EACCES`). *Reason:* the file is already the cap unit (how grants.json is gated), no new cap primitive, matches `defaults`' per-domain grant unit. Per-key gating is deferred to a future finer-grain build, justified only if a single domain ever mixes sensitivities — flagged, not built.

**Fork D — the OT-51 seam (config-in-base vs beside vs both).** ➜ **Recommend: spans both** — factory defaults inside the immutable base; owner changes as schema-version-tagged reversible deltas beside it in OT-51's ledger, degrading gracefully (unknown key → held-but-inactive) against a mismatched base. *Reason:* a fresh boot needs complete defaults (in-base); an OS rollback must not silently revert owner settings (beside, replayed) — the refusal of Windows's update-ate-my-files case. **Compose with Cerebro's OT-51 THINK; the PM synthesizes.**

**Sub-fork (rigor convergence).** ➜ **Recommend: when stakes.json migrates into the store, raise its lenient substring reader to grants.json's strict typed decode + byte oracle.** *Reason:* one store, one decode discipline; the permissive reader is a latent misread risk and has no oracle.

**No fork here is a mutually-exclusive, irreversible decision the PL must make *before* the THINK can land** — they are design recommendations for ratification when the first config-surface build opens (per the OT-50 row's gating), which is why they are documented rather than surfaced as a blocking choice.

---

## 10. Sources

- `Plans/RESEARCH-2026-07-08-beginner-poweruser-os-substrate-FINDINGS.md` — P1 (one config substrate, two views) + P2/P3/P4/P5 lenses; claim #19 (macos-defaults.com, 3-0); the Windows split-brain re-verification (Microsoft docs, verbatim, 2026-07-08).
- `docs/THINK-2026-07-08-poweruser-neo-shell-duality-design.md` (Prometheus) — §6 the anti-split-brain UX contract; §10 Fork 2 the OT-50 flag; neo> command→`ActionKind` single-source rule.
- `docs/OPEN-THREADS.md` — OT-50 (banked summary + PL ruling), OT-51 (Cerebro's parallel rollback THINK), OT-19 D2 (NeoFS snapshot primitive), OT-52 (the Atrium GUI).
- In-repo primitives read firsthand: `userland/libneo/src/grant_store.rs` + `docs/ADR-060`; `userland/neo-arthur/src/reversibility_logic.rs` (stakes.json); `docs/ADR-061` (I-15); `docs/ADR-069` (posture); Bible Article 29.3 / 1.5 / 1.6.
- Design prior art (canonical, not summarizing-fetched — see the §2 provenance note): macOS `defaults(1)` / Apple CFPreferences & property-list type system (per-domain, per-value typing, GUI+CLI one store); Findlater & McGrenere CHI 2004; Shneiderman multi-layer interfaces; Apple HIG disclosure; ChromeOS developer-mode anti-pattern.

**Acquisition-rule compliance:** no summarizing `WebFetch` was used for any exact prior-art claim; Windows/macOS exact claims rest on the FINDINGS' 3-vote primary-source verification, and the `defaults` design-shape prior art is attributed canonically without a fabricated fresh quote (§2 provenance note).

---

## Gate + close

- **Docs-only ⇒ no CI run exists** (the workflow paths-ignores `docs/**`, `Plans/**`, `**.md`), so there is no push-CI URL to cite — stated per the CI-gate rule.
- **`scripts/worker-close-gate.sh`** run locally on this docs-only branch (skip-with-note if the host `cargo` set times out on a branch that touches no code — nothing to compile/test here beyond fmt/SPDX, which apply to source files, of which this branch adds none).
- **RETURN diffed against this committed artifact** before sending.
- **Disk reclaim** after the PM merges this branch to `origin/main` (docs-only; delete the `AltimitNeo-ot50` worktree once the commit is on `origin/main`).
