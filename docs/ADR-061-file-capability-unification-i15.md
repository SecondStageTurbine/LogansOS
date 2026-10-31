<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-061: File-capability unification — `Handle::File` is a first-class cap (I-15)

**Status:** Accepted
**Date:** 2026-06-19
**Dispatch:** `Plans/DISPATCH-2026-06-19-file-cap-unification-i15-cerebro.md`
**Design context:** `Plans/THINK-2026-06-18-arthur-grant-ergonomics-design.md` (OT-18 D4 — files must be derivable+grantable for the kernel-enforced Arthur boundary)
**Scope:** Bring `Handle::File` into the `CapRights` derivation/grant graph — derivable (attenuated), grantable, inheritable, with `FileRights` subset-checking — plus the new invariant **I-15** and its host KATs + QEMU proof. Does **not** change file I/O *semantics* (open/read/write/readdir behavior is unchanged), touch other handle kinds' rights, add new file rights beyond wiring the reserved meta bits, or build OT-18 E1b (the consumer — a separate dispatch). No new `unsafe`.

## Problem

Until this build, `Handle::File` was a **parallel rights system** that never entered the capability graph. `Handle::File::rights()` returned `CapRights::new(0, 0)` (`process.rs`), so:

- `derive_handle` (gated on `has_meta(DERIVE)`) **EPERM'd on every file handle** — a file cap could not be narrowed/attenuated;
- `ipc::grant` / `complete_grant` (gated on `has_meta(GRANT)`) **EPERM'd on every file handle** — a file cap could not be delegated;
- `sys_spawn` inheritance (gated on `has_meta(GRANT)`) **silently skipped** file handles;
- `Handle::with_cap` on a `File` **ignored** the requested rights (`let _ = rights;`) — even where a derivation reached it, no attenuation happened.

The `FileRights::{GRANT,DERIVE}_RESERVED` bits (`fs/mod.rs`, `1<<24` / `1<<25`) were reserved for exactly this unification and never wired. **Found by Genesis (OT-18 E1b STOP), PM-verified against the code, PL-ratified 2026-06-19 to build before E1b** — it is the keystone (Article 29.3): without it a delegated file cap cannot be narrowed or kernel-enforced, so OT-18's "neo-shell derives a narrow read-only file cap and grants it to Arthur" cannot be the real boundary.

## Decision

Make `Handle::File` a first-class capability via a pure, kernel-free, host-testable mapping (`kernel/src/sched/file_cap_logic.rs`) between `FileRights` and `CapRights`. `FileRights` **stays authoritative for the semantic file ops** (read/write/readdir gate on the handle's / `OpenFile`'s `FileRights`, unchanged); the mapping only surfaces the cap-graph meta-rights and lets `with_cap` attenuate.

Three entry points were wired (all gated by the kill-switch const, below):

1. **`Handle::rights()`** for `File` → `file_cap_logic::file_rights_to_cap(bits, enabled)` — surfaces meta + semantic so `has_meta`/`is_subset_of`/`intersection` gate file derivation/granting exactly as for a channel/process handle.
2. **`Handle::with_cap()`** for `File` → `file_cap_logic::attenuate_file_rights(...)` — the requested `CapRights` is decoded to `FileRights` and **intersected with the original** (child ⊆ parent, always — ISC-2, the bug Genesis flagged).
3. **`sys_open`** → `file_cap_logic::root_file_handle_rights(...)` — the opener is rooted with the file cap's owner meta-rights (`DERIVE`+`GRANT`) so it can derive/grant/inherit. The backing `OpenFile` keeps the user's semantic rights alone (file I/O semantics unchanged).

`derive_handle`, `ipc::grant`/`complete_grant`, spawn-inheritance, and `revoke_handle` then work on files **with no file-specific code** — they already operate on `CapRights` + the kind-agnostic `derivations` map; once `rights()` and `with_cap()` are honest for files, the existing graph carries them.

### The mapping (the hand-written oracle — asserted, not round-tripped)

`FileRights` is a 32-bit set; `CapRights` is `{ meta: u8, semantic: u8 }`. The mapping is the I-12-style byte oracle for this layout: hand-laid in `file_cap_logic.rs` and asserted by `tools/file-cap-logic-tests` against **literal** expected values (never a writer↔reader round-trip — a writer+reader built together agree even when both are wrong).

**Semantic (file op → `CapRights.semantic`) — identity on bits 0..=5:**

| `FileRights` | bit | `CapRights.semantic` |
|---|---|---|
| `READ` | `1<<0` | `1<<0` |
| `WRITE` | `1<<1` | `1<<1` |
| `LOOKUP` | `1<<2` | `1<<2` |
| `READDIR` | `1<<3` | `1<<3` |
| `EXECUTE` | `1<<4` | `1<<4` |
| `MMAP_RESERVED` | `1<<5` | `1<<5` |
| `MOUNT` | `1<<27` | `1<<6` |

The bits a real handle can carry (`READ..EXECUTE`) map to the **identical** semantic bit, so the userland derive/grant ABI passes `FileRights::READ.bits() as u8` directly — no translation in the syscall caller. Only `MOUNT` is relocated (bit 27 → bit 6) because the semantic byte is 8 bits.

**Meta (file meta-right → `CapRights.meta`):**

| `FileRights` | `CapRights.meta` |
|---|---|
| `DERIVE_RESERVED` (`1<<25`) | `DERIVE` (`1<<1`) |
| `GRANT_RESERVED` (`1<<24`) | `GRANT` (`1<<2`) **and** `REVOKE` (`1<<3`) |
| *(any semantic bit set)* | `USE` (`1<<0`) |

`USE` and `REVOKE` are **derived, not stored**: a handle with any file op carries `USE`; the authority to *grant* a cap is the authority to *revoke* it (the grantor is the revoker), so `GRANT_RESERVED ⇒ GRANT|REVOKE`. `cap_to_file_rights` (the decode used by `with_cap`) reconstructs only the two stored meta bits; `USE`/`REVOKE` re-derive on the next `rights()`. This keeps encode→decode exact on every bit a real handle can hold, which is what makes **spawn-inheritance** (which passes the parent's full `rights()` back through `with_cap`) preserve the parent bit-for-bit.

The mapping's local bit consts are tied to the authoritative `FileRights`/`meta_rights` definitions by `const _` static assertions in `sched::process` — a drift in either is a **compile error**, not a silent oracle skew (the grep-checkable + machine-checked half of the I-15 guard).

### I-15 — the new invariant

> **I-15: File handles are first-class capabilities.** A derived/granted/inherited file handle's `FileRights` is a **subset of its parent's**, and `DERIVE`/`GRANT` meta-rights gate file derivation/granting **as for every other handle kind** — no file-handle bypass of the capability graph.

**Why no existing row covers it:** I-1…I-14 govern the AI-governance substrate (residency/eviction/DMA-fence/IOMMU); none constrain the *capability graph's treatment of file handles*. I-15 is the cross-cutting property that OT-18's kernel-enforced Arthur boundary (and every future delegated-file-cap consumer) rests on. Added under Rule 2 (name + justify + guard, all in this build).

**Guard (shipped this build):**
- a **runtime assertion** in the attenuation path (`attenuate_file_rights`): `debug_assert!(child & !original == 0)` — a derived file right can never escape the parent set;
- the **static-assert** honesty check tying the mapping to `FileRights`/`meta_rights` (compile-time, grep-checkable);
- the **host KATs** (`tools/file-cap-logic-tests`): the oracle table, attenuation, an **exhaustive** subset-enforcement sweep (every parent × every requested `(meta,semantic)` → child ⊆ parent), meta-gating, the spawn round-trip, and the kill-switch equality;
- the **QEMU scenario** `file_cap_derive_grant` (below) — the live no-bypass proof against the real kernel at `NEO_SMP=4`.

### Kill-switch (Rule 1)

`FILE_CAPS_ENABLED = cfg!(feature = "file-caps")`, default **off**. With it off, `file_rights_to_cap → (0,0)`, `attenuate_file_rights` returns the original (ignores the request), and `root_file_handle_rights` adds no meta — i.e. **`Handle::File` behaves exactly as before this build, bit-for-bit**. The equality regression (`killswitch_off_*` KATs) proves this for all representable inputs. (Expressed as unit-test equality on the pure functions per Rule 1 "where expressible"; the production call sites read the compile-time const, so the disabled kernel is the pre-unification kernel.)

## The errno note (read before scrutinizing the QEMU scenario)

Attenuation bites at **two different errnos**, by design, because file I/O semantics are unchanged:

- A **read-only** file handle whose holder attempts `sys_write` is denied **`EACCES` (-13)** — this is the *existing* file-rights-insufficient errno (`file_lookup(fd, FileRights::WRITE)` → `EACCES`), not changed by this build. So the QEMU "child writes → denied" leg asserts **`EACCES`**, the real errno, not `EPERM`.
- A **cap-graph** refusal — deriving a right the parent lacks, or deriving/granting a handle without the `DERIVE`/`GRANT` meta — is **`EPERM` (-1)**, the capability-graph errno (`derive_handle`/`sys_grant`).

The dispatch's shorthand "writes → EPERM" means "writes are denied"; the implemented and asserted errno for the write-denial is `EACCES` (the file rights gate), and `EPERM` for the meta-gating/subset refusals. Keeping the write-denial at `EACCES` is required by the "no file-I/O-semantics change" scope guard.

A second consequence of unchanged semantics: a granted file cap shares the **same `Arc<OpenFile>`** as the grantor, so the `OpenFile`-level rights (set at open) still permit the underlying op; the **handle-level** `FileRights` attenuation (checked by `file_lookup`) is what enforces read-only on the grantee. The scenario opens `/scratch` `READ|WRITE` precisely so the `OpenFile` permits writes and the *handle* attenuation is the binding constraint under test.

## Gate

- **Host KATs** (`tools/file-cap-logic-tests`, 15 tests): mapping oracle; attenuation (`READ`-derive of `READ|WRITE` ⇒ read-only); exhaustive subset enforcement; meta-gating (a read-only leaf surfaces no `DERIVE`/`GRANT`); spawn round-trip exactness; **kill-switch equality** (off ≡ `(0,0)` / identity, swept).
- **QEMU `file_cap_derive_grant`** (`neo-test-file-cap-derive-grant`, pulls `file-caps`, `NEO_SMP=4`): neo-init opens `/scratch` (rooted with `DERIVE`+`GRANT`), derives a read-only cap (reads OK, **writes → `EACCES`**), proves meta-gating (re-derive → `EPERM`) and subset enforcement (derive `EXECUTE` the parent lacks → `EPERM`), then **grants a read-only cap to a neo-worker child** that confirms read-OK / write-`EACCES`, and finally proves a **non-grantable** file cap can't be granted (`EPERM`). Auto-joins the 4-shard `workflow_dispatch` QEMU gate (non-ignored, non-bare-metal).
- SPDX on every new file; `fmt`/`clippy --workspace --all-targets -D warnings` + blocking kernel clippy via Docker.

## Consequences

- **OT-18 E1b is unblocked**: a file cap can be derived narrow and granted, and the kernel cap is genuinely the grantee's boundary (the keystone is laid).
- The capability graph is now **kind-uniform**: every `Handle` variant participates in derive/grant/revoke/inherit by the same `CapRights` rules; files are no longer the exception.
- Complexity stays bounded: the new decision logic is **pure and host-tested** (Rule 3 tier), gated by a provable kill-switch (Rule 1), and adds exactly one named invariant with its guard (Rule 2). No new `unsafe`, no new lock, no file-I/O-semantics change.

## Deferred

- OT-18 E1b (neo-shell derives + grants Arthur a narrow file cap; the consumer) — separate dispatch, builds on this.
- Directory-prefix file-cap scoping and any change to file-op *matching* — out of scope (semantics unchanged here).
- `RETYPE_RESERVED` / `MMAP` semantic activation — reserved bits, not wired beyond the mapping's totality.
