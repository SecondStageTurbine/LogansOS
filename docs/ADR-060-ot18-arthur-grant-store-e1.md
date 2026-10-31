<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-060: OT-18 Slice E1 — Arthur grant store (`.arthur/grants.json`)

**Status:** Accepted
**Date:** 2026-06-19
**Dispatch:** `Plans/DISPATCH-2026-06-19-arthur-grant-store-e1-cerebro.md`
**Design:** `Plans/THINK-2026-06-18-arthur-grant-ergonomics-design.md` (ratified D1/D2/D4/D7)
**Scope:** The grant-store **format** + a no-alloc **loader** + the **D7 least-privilege default** + the **`arthur-grants` kill-switch** + the **authority-boundary proof**, all host-coverable. Does **not** include the kernel-cap re-derivation at spawn (`derive_handle` + `ipc::grant`), the neo-arthur startup load wiring, E2 (on-miss re-request), E3 (audit/revoke), or E4 (memory scopes) — see *Deferred*.

## Decision

Add `libneo::grant_store`: a versioned, human-readable `.arthur/grants.json` that records the user's durable coarse grants (OT-18 D1's front-loaded scopes), plus a loader that re-derives Arthur's in-process `Grant` set at startup. The store is **persistence/UX/audit**; it is **never** the authority.

### The authority boundary (the load-bearing statement, Article 29.3)

> The kernel capability Arthur holds is the enforcement boundary **by construction**. `grants.json` is a UX/persistence/audit record. The in-process `Grant` set the loader builds is the *userspace policy mirror* that decides which kernel cap `execute_action` presents to a syscall — it is not the enforcement layer.

A compromised Arthur that fabricates entries in `grants.json` (or ignores it) **cannot exceed the kernel caps it actually holds**: a `Grant` present in the set with **no backing kernel cap still fails `EPERM` at the syscall**. This is proven, not asserted — `tools/arthur-grant-logic-tests::grant_in_set_without_kernel_cap_still_eperm` builds the full grant set from a store that grants `FileRead /secret`, runs `execute_action` against a mock that models the kernel cap check, and shows the read fails `EPERM` because no cap is held; the contrast test (`grant_with_backing_kernel_cap_succeeds`) shows the *same* grant set succeeds only once the cap is held.

### Format (`grants.json` v1)

```json
{
  "version": 1,
  "grants": [
    { "scope": "FileRead", "resource": "/home/user/project", "rights": ["read"], "granted_by": "user" }
  ]
}
```

- **`version`** — top-level format version (`1`). Any other value refuses the store (forward-compatible: a future version switches parser) and degrades to the D7 default.
- **`scope`** — one of D2's six resource-class scopes: `FileRead`, `FileWrite`, `Process`, `Network`, `Model`, `Memory`. E1 *stores and reloads* all six; it wires only the **file** scopes into a `Grant` (`Grant::File`). `Process`/`Network`/`Model`/`Memory` are taxonomy seams that round-trip through the store but produce no `Grant` yet (E1 stays narrow — the D5 sequencing guard, OT-19 unbuilt).
- **`resource`** — the scoping dimension. For file scopes a path. **v1 matching is exact-path** — the unchanged `Grant::allows` semantics. Directory-*prefix* scoping (D2's stated intent) is a named follow-on that would change the action surface's matching, deliberately **not** E1.
- **`rights`** — an array of right tokens: `read`, `write`, `lookup`, `readdir`, resolved to a `FileRights` bitset.
- **`granted_by`** — the **OT-20 seam** (action provenance across principals). Present and single-Arthur (`"user"` / `"default"`) today.

v1 is a restricted JSON: strings carry no escape sequences (a backslash is rejected), unknown keys are rejected, and any malformed store degrades to the D7 default — it never crashes and never widens authority.

### Authoritative worked example (the independent byte oracle, I-12 / CLAUDE.md "byte oracle" rule)

The format is pinned by a **hand-authored** fixture asserted **field-by-field against the loader**, NOT a writer→reader round-trip (a writer+reader built together agree even when both implement the spec wrong). The canonical fixture and its decode:

| Input bytes (`grants.json`) | Loader output (`parse_grants` ⇒ `GrantEntry`) |
|---|---|
| `"version": 1` | (top-level) accepted; `1 != 1` would refuse → D7 |
| `{ "scope":"FileRead", "resource":"/home/user/project", "rights":["read"], "granted_by":"user" }` | `GrantEntry { scope: FileRead, resource: "/home/user/project", rights: READ (0x1), granted_by: "user" }` |
| `{ "scope":"FileWrite", "resource":"/home/user/project/notes", "rights":["write"], "granted_by":"user" }` | `GrantEntry { scope: FileWrite, resource: "/home/user/project/notes", rights: WRITE (0x2), granted_by: "user" }` |
| `{ "scope":"Network", "resource":"api.example.com:443", "rights":[], "granted_by":"user" }` | `GrantEntry { scope: Network, resource: "api.example.com:443", rights: 0x0, granted_by: "user" }` (taxonomy seam → `entry_to_grant` ⇒ `None`) |

`FileRead` ⇒ `Grant::File { path: "/home/user/project", rights: READ }`; `FileWrite` ⇒ `Grant::File { path: …, rights: WRITE }`; `Network`/`Process`/`Model`/`Memory` ⇒ `None` (deferred). This table is the oracle; `byte_oracle_parses_field_by_field` asserts exactly it.

### D7 least-privilege default

First boot / no store / corrupt store ⇒ Arthur's grant set is **only** `FileRead` on `.arthur/` (READ + LOOKUP + READDIR — read its own grant store and OT-17 memories, list the memory directory). **No write, no network, no process, no other-category memory.** D7 stays narrow (the D5 guard — OT-19 reversibility has not shipped, so the *default* file scope is not widened past `.arthur/`). A corrupt or absent store degrades to exactly this default — fail-safe, mirroring OT-17's ephemeral-fallback discipline.

### Kill-switch (`arthur-grants`)

The loader's `GrantConfig` mechanizes the kill-switch:

- `GrantConfig::Disabled` ⇒ **no** durable grants (spawn-inherited caps only) — bit-for-bit equivalent to pre-E1 Arthur; the store is never consulted and cannot leak in.
- `GrantConfig::Enabled(None)` ⇒ the D7 default (first boot / no store file).
- `GrantConfig::Enabled(Some(bytes))` ⇒ the D7 default **plus** the parsed durable grants; a malformed store ⇒ the D7 default only.

`kill_switch_disabled_consults_no_store_and_yields_no_grants` proves the equivalence: even a valid store yields zero grants when disabled; the *only* difference between off and on is the config flag.

## Invariants and unsafe

**No new complexity-ledger invariant.** The grant store composes existing primitives only: the parser is pure no-alloc/zero-copy logic, and the authority boundary is the already-proven kernel capability check (Article 29.3) exercised through the existing `execute_action`. **No new `unsafe`.** The loader touches no AI-governance lock.

## Deferred (the kernel-touching follow-on)

E1 is the host-coverable grant *infrastructure*. The remaining wiring is kernel-touching and therefore lands in a follow-on with the mandatory pre-merge QEMU gate (the fork the dispatch's gate anticipates):

1. **Re-derive at spawn (authority side).** neo-shell reads `grants.json` and, per durable grant, `derive_handle`s a narrow kernel cap scoped to the resource and `ipc::grant`s it to Arthur — plus the D7 `FileRead .arthur/` cap so Arthur can read its own store. This exercises `ipc::grant` of file caps across the spawn boundary (kernel-cap path) and cannot be conclusively host-verified.
2. **Load at startup (consumer side).** neo-arthur reads `.arthur/grants.json` (gated `arthur-grants`), calls `build_grant_entries`, and feeds the resulting `Grant` set to `execute_runtime_tool` instead of today's hardcoded demo grants; off ⇒ today bit-for-bit.
3. **QEMU `arthur_grants_persist` @ NEO_SMP=4.** grant a FileRead scope → write store → fresh Arthur spawn re-derives the cap + reads the in-scope file; off ⇒ D7 default only; corrupt store ⇒ D7 default, no crash.
4. The `arthur-grants` xtask feature mapping (so the QEMU ISO build enables the gate).

E2 (on-miss re-request), E3 (audit/revoke), E4 (memory scopes) remain later slices unchanged.

## Amendment: E2 — on-miss re-request + the grants.json writer

**Date:** 2026-06-19
**Dispatch:** `Plans/DISPATCH-2026-06-19-arthur-grant-rerequest-e2-genesis.md`
**Scope:** The re-request *decision logic* + scope inference + the `grants.json` writer + remember-the-denial + the `arthur-grant-rerequest` kill-switch. Pure userspace, host-only — no kernel touch, no QEMU.

### What E2 adds

When `execute_action` returns `ActionError::Denied { required }`, instead of stopping dead (E1a), Arthur can offer to request the missing scope (D3/D4):

1. **Scope inference (`infer_scope`):** `RequiredCapability::File { path, rights }` → the parent directory of `path` + the same rights; `FileWrite` if the `WRITE` bit is set, else `FileRead`. Network/Process/Model/Memory → `None` (not inferable — deferred to later slices).
2. **`ScopeRequest` + `handle_denied`:** produce a structured request the conversation layer can present, with three outcomes: Session / Always / No. The kill-switch (`ReRequestConfig::Disabled`) ⇒ `ReRequestOutcome::Disabled` — no escalation, no writer — ≡ E1a.
3. **Remember-the-denial (`DeniedScopes`):** a "No" records the `(resource, scope)` pair in a per-session set; `handle_denied` on an already-denied scope returns `AlreadyDenied` (anti-nag — never re-request the same scope twice in a session).
4. **The `grants.json` writer (`serialize_grants`):** serializes `GrantEntry` values into valid `grants.json` v1 bytes. The writer's output for the ORACLE fixture's 3 entries is **byte-for-byte identical** to the hand-laid `ORACLE` (the independent byte oracle — catches a writer+reader that agree with each other but both implement the spec wrong), and round-trips through `parse_grants`. "Always" appends the new entry to the existing store and serializes.
5. **"Session" records intent only.** The *live* cap acquisition (Arthur actually acquiring a kernel cap mid-session) is **deferred to I-15/E1b's runtime-grant path** — E2 records the grant *intent/persistence*, not a live kernel cap.

### Kill-switch (`arthur-grant-rerequest`)

Default-off. Off ⇒ the re-request functions are not compiled; on `Denied`, Arthur stops with "I can't do that" — no escalation, no writer — bit-for-bit ≡ E1a. The runtime kill-switch (`ReRequestConfig::Disabled`) provides the same behavioral gate when the feature is compiled in.

### Invariants and unsafe (unchanged from E1)

**No new complexity-ledger invariant.** E2 composes E1a's `grant_store` + `action.rs` only — pure decision logic + serialization, no new authority primitive. **No new `unsafe`.** No-alloc / no_std-clean (fixed-capacity arrays, caller-provided buffers). The authority boundary is unchanged: `grants.json` is persistence/UX/audit, never authority (Article 29.3).

### Verification (host-only — no QEMU)

Host suite `tools/arthur-grant-logic-tests` extended to 30 tests (18 E1a + 12 E2, gated on `arthur-grant-rerequest`): scope inference (File parent-dir cases incl. `/etc/shadow`→`/etc/`; Network/Process → not-inferable), remember-the-denial (anti-nag: second Denied on a remembered scope ⇒ no request), the writer byte-oracle (writer-bytes == hand-laid ORACLE + round-trip via `parse_grants`), kill-switch equality (off ⇒ no escalation), grant decisions (No → denial recorded; Session → intent only; Always → serialized + parsed back). Builds on AND off the feature; off ⇒ 18 E1a tests only, no E2 code compiled. Full close-gate host set green. Host-only (no `kernel/`, `boot/`, `tools/xtask/` touch) ⇒ no QEMU gate.

## Amendment: E3-audit — the inspection surface (D6 inspection half)

**Date:** 2026-06-20
**Dispatch:** `Plans/DISPATCH-2026-06-20-arthur-grant-audit-e3-genesis.md`
**Scope:** The D6 *inspection* half — list the grant set + the in-memory action log. **Revoke is DEFERRED** to a later OT-18 slice; ADR-061 now supplies the file-cap graph it needs, but E3-audit remains host-only inspection only. Pure userspace, host-only — no kernel touch, no QEMU.

### What E3-audit adds

1. **Grant-set listing (`build_grant_listing` + `format_grant_listing`):** from E1a's loaded entries + the D7 default, grouped by scope (FileRead, FileWrite, then the deferred taxonomy seams) in a stable order, showing `resource`, `rights` (as `read|write|lookup|readdir`), and **`granted_by`** (the OT-20 provenance seam). A structured form (`GrantListingEntry`) for the shell `arthur grants` builtin and a human-readable form (`format_grant_listing`) for the conversation "what can you do?" answer.
2. **Action log (`ActionLog`):** a per-session, fixed-capacity (64-entry) in-memory ring of `ActionLogEntry { kind: ActionKind, resource, succeeded, principal }`, appended as `execute_action` runs; listed newest-first with ring-eviction when full. **In-memory/session only — NO persisted on-disk format** (persistence is a later slice; avoiding it keeps E3 off the byte-oracle hook). The `principal` field is the OT-20 multi-principal seam (single-Arthur = `"arthur"` today).
3. **Revoke is DEFERRED — stated, not built.** Revocation = `revoke_handle` on the parent file cap, cascading through the derivation graph. That was blocked before ADR-061; it is now unblocked, but still not part of E3-audit. The seam is noted in code: `revoke_grant` lands in a later OT-18 revoke slice.

### Kill-switch (`arthur-grant-audit`)

Default-off. Off ⇒ no audit surface compiled, no action-log recording; grants are still enforced, just not inspectable — ≡ E1a/E2. The equality regression is structural: the code does not exist when the feature is off.

### Invariants and unsafe (unchanged from E1/E2)

**No new complexity-ledger invariant.** E3-audit composes E1a/E2 + `action.rs` only — pure presentation + in-memory logging, no new authority primitive. **No new `unsafe`.** No-alloc / no_std-clean (fixed-capacity arrays, caller-provided buffers). The authority boundary is unchanged: `grants.json` is persistence/UX/audit, never authority (Article 29.3).

### Verification (host-only — no QEMU)

Host suite `tools/arthur-grant-logic-tests` extended to 39 tests (18 E1a + 12 E2 + 9 E3-audit, gated on `arthur-grant-audit`): grant-set listing (grouping by scope in stable order, D7 default appears, `granted_by` shown, empty set, human-readable rendering), action log (record → list newest-first, ring-eviction when capacity exceeded, the principal field, human-readable rendering, empty log), kill-switch equality (off ⇒ no surface compiled ≡ E1a/E2). Builds on AND off the feature; off ⇒ 18 E1a tests only (or 30 E1a+E2 with `arthur-grant-rerequest`), no E3 code compiled. Full close-gate host set green (including feature-on clippy). Host-only (no `kernel/`, `boot/`, `tools/xtask/` touch) ⇒ no QEMU gate.

## Amendment: E3-revoke — revoke-through-the-derivation-graph (D6 revocation half)

**Date:** 2026-06-22
**Dispatch:** `Plans/DISPATCH-2026-06-22-arthur-grant-revoke-e3-oracle.md`
**Scope:** The D6 *revocation* half — the last enforcement piece of the OT-18 grant lifecycle. A previously-granted scope can be **revoked**, and the revocation is enforced **by the kernel** (`revoke_handle` through the capability derivation graph), not by editing a JSON file. Adds a QEMU scenario (no Phase-2 primitive change) ⇒ **kernel-touching, QEMU-gated**.

### The decision this build makes

Turns "grants can be created + audited but not withdrawn" into "**a granted scope can be revoked, and a compromised-or-confidently-wrong holder immediately loses that authority — at the real kernel, transitively through anything it sub-derived.**" The keystone holds (Article 29.3): the kernel cap is the boundary; `grants.json` is the UX/persistence/audit record, never the enforcement.

### What E3-revoke adds

1. **`grant_store::revoke_grant` (the store/audit half, host-tested):** parses the live `grants.json`, removes every entry matching `(scope, resource)`, and re-serializes the survivors through the E2 writer (`serialize_grants`). Returns `RevokeOutcome { removed, bytes_written, remaining }`. **Idempotent + minimal:** an absent scope yields `removed == false` (a no-op success), and only the exact `(scope, resource)` match is dropped — a sibling grant (e.g. FileRead on a *different* path, or FileWrite on the *same* path) survives byte-for-byte. This is **persistence/audit only — never enforcement.**
2. **neo-shell `/revoke <resource>` (the production surface, ISC-1):** the shell now **retains** the per-scope derived caps it hands Arthur (it no longer closes them after `ipc::grant`), keyed by their `grants.json` resource. `/revoke <resource>` looks up the retained cap and calls **`sys_revoke`** (→ `revoke_handle`) on it — the enforcement action. It then rewrites `grants.json` via `revoke_grant` so the scope is not re-derived next spawn, and logs the audit/discriminator line (`scope`, `cap_fd`, `store_removed`). Idempotent (unknown/already-revoked ⇒ no-op success); fail-safe (a `sys_revoke` error leaves the scope live and is surfaced — never a half-revoked state). `grants.json` has no truncate primitive, so the (shorter) rewrite is padded with trailing newlines to the old length — the v1 parser skips trailing whitespace, so a same-length overwrite leaves no parseable stale tail.
3. **The transitive guarantee is `revoke_handle`'s existing Phase-2 behavior — composed, never modified.** `revoke_handle` walks the derivation graph from the revoked cap and kills it **plus every descendant** (across processes — the `ipc::grant` edge is derived-copy: the recipient's CapId is linked to the donor's source CapId). A revoke that missed a descendant would be a privilege-escalation hole.

### Enforcement vs. store (the split, restated so it cannot drift)

- **Enforcement (security):** `sys_revoke`/`revoke_handle` on the retained cap → the cap **and** every descendant die at the kernel. A revoked handle is **removed** from the table, so the next op returns **`EBADF`** (the handle is gone — the kernel refusing the operation, with zero reference to `grants.json`). This holds even if the store still listed the scope (store-vs-cap, ISC-3).
- **Store (audit/persistence):** `revoke_grant` rewrites `grants.json`. Best-effort, never authority: the cap is already dead even if this rewrite fails, and a malformed store already degrades to the D7 default at next spawn, so a stale entry can never resurrect authority.

### QEMU scenario `arthur_grant_revoke` (NEO_SMP=4, kernel-touching)

A self-contained **neo-init** driver (no worker) exercises the file-cap derivation graph by syscall and proves the security crux at the real kernel. Arthur's *real* granted cap is USE-only (it cannot sub-derive), so — exactly as the dispatch directs — the multi-hop transitive property is exercised at the kernel level: open `/hello.txt` (rooted DERIVE+GRANT+REVOKE per I-15) → derive the per-scope cap `D_x` the shell would hand Arthur → derive a 2-deep descendant chain `D_x_child` → `D_x_grandchild` (the holders a delegated cap could itself spawn) → a separate scope `D_y` (independent open). Every cap reads (pre-revoke ✓). Then `sys_revoke(D_x)` and assert: `D_x`, `D_x_child`, **and** `D_x_grandchild` all read `EBADF` (the walk reached every depth — the transitive crux), while `D_y` (a separate tree) and `root_x` (the *ancestor* of `D_x` — revoke walks **down**, never up) still read OK (isolation + minimality). The pass criterion is the init exit code, so **every assertion is load-bearing** (no "process exited 0" theatre); distinct return codes localize a missed descendant, and the driver logs the discriminator (`scope`, `cap_fd`, `descendants_walked`) to serial so a failure is self-diagnosing. The cross-process `ipc::grant` revoke edge for non-file caps is already proven in-tree (`cap_revoke_cross_process`, the Phase-7 sharing test); E3-revoke proves the file-cap, multi-hop case the Arthur lifecycle needs.

### Kill-switch (`arthur-grants`)

The whole grant lifecycle — derive/grant at spawn (E1b) **and** revoke (E3) — lives under neo-shell's `arthur-grants`, which now pulls `libneo/arthur-grant-revoke`. Off ⇒ no grant wiring at all and no `/revoke` command, bit-for-bit ≡ pre-E. The pre-existing `arthur_grants_persist` scenario continues to pass unchanged (the feature-on, no-revoke baseline).

### Invariants and unsafe (unchanged from E1/E2/E3-audit)

**No new complexity-ledger invariant** — revoke-through-the-graph is `revoke_handle`'s *existing* Phase-2 guarantee, composed here, not a new cross-cutting invariant. **No new `unsafe`** (composes `revoke_handle`/`grant_store`). **No modification of the Phase-2 cap primitives** (`revoke_handle`/`derive_handle`/`ipc::grant` are frozen). Scope stays **FileRead/FileWrite only** (Process/Network/Model/Memory remain deferred — E4). The authority boundary is unchanged: `grants.json` is persistence/UX/audit, never authority (Article 29.3).

### Verification

Host suite `tools/arthur-grant-logic-tests` extended with 6 E3-revoke KATs (default-on via the crate's `arthur-grant-revoke` default feature, so they run under the standing `cargo test --workspace` close-gate): the rewrite byte-oracle (output of `revoke_grant` == hand-laid expected store, byte-for-byte, then round-trips via `parse_grants`), idempotent no-op for an absent scope, idempotent second-revoke, minimality (same-path other-scope + other-path survive), malformed-store → `Err(Malformed)` (no panic), buffer-too-small → `Err(BufferTooSmall)`. Plus the QEMU `arthur_grant_revoke` scenario (above). With OT-18 E3-revoke complete, **grants are revocable and the boundary is enforced through the derivation graph — E4 (memory scopes) is the only remaining E-slice.**

## Verification

Host suite `tools/arthur-grant-logic-tests` (18 tests, mounts `action.rs` + `grant_store.rs` against shims): the byte oracle, the D7 default (write-free, `.arthur/`-only), fail-safe degradation (missing/corrupt → D7), the kill-switch equivalence, format rejections (bad version, unknown scope/right, missing field, escape sequence, trailing garbage, empty, buffer overflow), and the authority boundary (grant-without-cap → EPERM; grant-with-cap → ok; absent-grant → Denied-before-syscall). Full close-gate host set green: `cargo fmt --all --check`, `cargo test --workspace --exclude neoplutarch-kernel`, `cargo clippy --workspace --all-targets -- -D warnings`, and the blocking kernel clippy. Host-only (no `kernel/`, `boot/`, `tools/xtask/` touch) ⇒ no QEMU gate required for E1.
