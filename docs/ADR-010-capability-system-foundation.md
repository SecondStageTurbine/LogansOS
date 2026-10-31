# ADR-010: Capability System Foundation

| | |
|---|---|
| **Status** | Accepted; BUILD-1 implemented |
| **Date** | 2026-05-04 |
| **Authority** | Project Lead |
| **Supersedes** | — |
| **Superseded by** | — |
| **Related** | Bible Article 12.1 (capability discipline, Phase 2) · Article 12.2 (no ambient authority) · Article 5.5 (spawn-only, no fork) · ADR-004 (IPC channels — Handle pattern this extends) · ADR-007 (ProcessHandle + sys_kill — first cap variant) · ADR-011 (single-CPU IPC handoff invariants) · ANSPEC-001 §3.7 (capability-handle pattern), §10.2 (capability discipline) |

---

## Context

Bible Article 12.1 mandates capability-based security from Phase 2: every kernel object referenced by a per-process handle, holding the handle is permission, no ambient authority. Phase 1.5b (BUILD-3 + BUILD-5) shaped the Handle enum (Channel + Process variants) as the **forward-compatible substrate** for this — the comment at the enum definition explicitly tags it as the "Phase 2 capability extension point — do not flatten."

Phase 2 must now deliver the discipline the substrate was designed for. Specifically:

- **Rights** — handles must carry permission bits, and the bits must be subsetable (a holder may grant a strictly-reduced version of their cap to another process).
- **Operations** — mint (already exists via `sys_chan_create`, `sys_spawn`), derive (rights-reduction), grant (cross-process transfer over IPC), revoke (invalidate cap and all derivations).
- **Tracking** — derivation must be recorded so revoke can find descendants.

What this ADR locks in is the **shape** of those primitives. Phase 2 BUILD-1 implements them; Phase 5+ extends the Handle enum with Tensor / Model / InferenceTicket / File variants that inherit this shape.

Five orthogonal design axes drive the implementation. Each is decided independently below.

## Decision

### Q1 — Addressing model: flat per-process HandleTable + per-process derivation log

Each process keeps its current `Vec<Option<Handle>>` as the addressing surface (handle index = capability address). Added: a per-process derivation log mapping `CapId → Option<CapId>` (child cap → parent cap, `None` for root-minted). Revoke walks the log to find descendants.

```rust
struct Process {
    // ... existing fields ...
    handles: Mutex<HandleTable>,
    /// Phase 2: derivation log. Maps cap-id of derived cap to its parent cap-id.
    /// `None` parent = minted directly (no parent). Used by revoke to walk
    /// descendants. O(N) scan over the log to answer "what are X's children?"
    /// — fine for Phase 2 scale (<100 caps/process). Phase 5 may need a
    /// parent→children index if cap counts explode.
    derivations: Mutex<BTreeMap<CapId, Option<CapId>>>,
}
```

| Alternative | Rejected because |
|---|---|
| seL4-style CSpace (hierarchical capability spaces with CDT) | NEO's expected authority graph for Phase 2 (parent-supervises-children + IPC-grant) doesn't justify hierarchical addressing. Flat tables already work; CSpace is over-engineered for the actual use case. seL4 verifies CSpace+CDT formally; we don't, so simpler is also safer. |
| Global derivation table (cross-process) | Re-litigates the BUILD-3 phantom-win lesson at a different layer: kernel structures cross-cutting Process lifetime are bug-prone. Per-process log keeps cleanup local. |

`CapId` is a kernel-monotonic `u64` (atomic counter). Never recycled for Phase 2; recycling considerations belong with Phase 2 SMP work.

### Q2 — Operations: mint + grant + revoke + derive

Phase 2 ships four capability operations. **Copy and move are deferred** to Phase 3+ as they are pure conveniences without a Phase 2 use case.

| Op | Phase 2 syscall | Semantics |
|---|---|---|
| Mint | (existing) `sys_chan_create`, `sys_spawn` | Kernel creates fresh root-minted caps; `derivation[new_cap_id] = None`. |
| Derive | `sys_derive(h, new_meta, new_semantic)` | Holder of cap with `DERIVE` meta-right creates a child cap with strict-subset rights, installed in the same process's handle table at a new index. `derivation[child] = Some(parent)`. |
| Grant | `sys_grant(channel_h, src_h, rights_subset)` + recipient's `sys_recv_grant(channel_h, max_rights)` | Atomic cross-process transfer over an existing IPC channel. Donor must hold `src_h` with `GRANT` meta-right. Recipient gets a fresh handle index with rights = donor_rights ∩ rights_subset ∩ max_rights. New child cap recorded in recipient's derivation log. |
| Revoke | `sys_revoke(h)` | Holder of cap with `REVOKE` meta-right invalidates the cap and all its descendants. Walks the per-process derivation log; for cross-process descendants (granted out), invalidates entries in those processes' tables too. Phase 2 single-CPU acceptable; Phase 2 SMP BUILD-3 hardens the cross-process walk. |

| Alternative | Rejected because |
|---|---|
| Mint + grant + revoke only (no derive) | Forces Phase 5 to special-case rights-reduction (model-RW → model-R for inference clients). With derive, Phase 5 inherits rights-reduction uniformly. |
| Full seL4 set including copy + move | Copy and move are conveniences without Phase 2 use cases. Move particularly is a no-op at the flat-table layer (same cap, different index — no semantic change). Defer until use case appears. |

### Q3 — Rights model: two-tier (uniform meta + per-variant semantic)

Rights are stored uniformly across all Handle variants but carry different bit meanings per variant.

```rust
#[derive(Copy, Clone)]
pub struct CapRights {
    pub meta: u8,      // USE | DERIVE | GRANT | REVOKE — uniform across variants
    pub semantic: u8,  // variant-specific bit assignments
}

// Meta-rights — uniform across all variants.
pub mod meta {
    pub const USE:    u8 = 1 << 0; // may invoke the cap's primary operation
    pub const DERIVE: u8 = 1 << 1; // may create child caps with reduced rights
    pub const GRANT:  u8 = 1 << 2; // may pass cap to other processes
    pub const REVOKE: u8 = 1 << 3; // may revoke this cap (and descendants)
}

// Semantic rights for Channel variant.
pub mod channel_rights {
    pub const SEND:  u8 = 1 << 0;
    pub const RECV:  u8 = 1 << 1;
    pub const CLOSE: u8 = 1 << 2; // Reserved per Principle 2; sys_close ships Phase 2 BUILD-2+.
}

// Semantic rights for Process variant.
pub mod process_rights {
    pub const KILL:    u8 = 1 << 0;
    pub const WAIT:    u8 = 1 << 1; // Reserved per Principle 2; sys_wait ships later in Phase 2.
    pub const INSPECT: u8 = 1 << 2; // Reserved per Principle 2; sys_inspect ships later in Phase 2.
}
```

Phase 5+ variants (Tensor, Model, InferenceTicket, File) define their own semantic-rights bits in their own modules. Meta-rights stay the same across all variants — that's what makes derive/grant/revoke uniform.

| Alternative | Rejected because |
|---|---|
| Uniform fixed bitfield (R/W/X/GRANT mapped to each variant's semantics) | Forces semantic mappings ("X on a Process?") that age badly. Category mistake. |
| Per-variant rights with no uniformity | The "policy drift" anti-pattern — every cap operation needs per-variant logic. Auditing capability behavior becomes "read N variant implementations." |

The two-tier split puts the orthogonal concerns where they belong: meta-rights drive the universal cap operations; semantic-rights specify what `USE` actually does for a given variant.

### Q4 — Cap behavior at process exit: survive-exit with Arc-lifetime resource cleanup

When a process exits, its handles drop, but **caps held by other processes that were derived from this process's caps remain valid as Handles**. The underlying resource is reaped per the existing Arc-lifetime rules: a Channel persists as long as either side has a Handle; a Process Arc drops when the last reference (registry or otherwise) goes away.

**Invocation against a now-dead resource fails with the appropriate errno**, not a kernel fault:
- Channel handle whose peer is gone → `sys_send`/`sys_recv` returns `-EPIPE` (existing BUILD-3 path).
- ProcessHandle whose target exited → `sys_kill` returns `-ESRCH` (existing BUILD-6 path).
- Phase 5+ variants: same shape — fail-on-invoke, not fail-on-lookup.

| Alternative | Rejected because |
|---|---|
| Revoke-on-exit cascade (donor exit kills derived caps in recipient processes) | Breaks the supervisor pattern: neo-init grants caps to workers, neo-init exits for legitimate reasons, workers lose everything. Not "secure," just broken. |
| Keep-Process-alive (cap holders pin donor's Process Arc) | Lifetime cycles when A holds B's cap and B holds A's cap; the gnarliness propagates. |

Survive-exit is the only option that composes cleanly with both the existing IPC behavior (BUILD-3 EPIPE) and the existing termination behavior (BUILD-6 ESRCH). Documenting it explicitly here so Phase 4+ supervisor patterns aren't relitigated.

### Q5 — Subset semantics for derive: bit-subset everywhere

Derive's rights-reduction is uniform bitwise subset: `child_rights ⊆ parent_rights` is `(child & parent) == child` — for both meta and semantic bytes.

```rust
impl CapRights {
    pub fn is_subset_of(&self, parent: &Self) -> bool {
        (self.meta & parent.meta) == self.meta &&
        (self.semantic & parent.semantic) == self.semantic
    }
}
```

| Alternative | Rejected because |
|---|---|
| Per-variant subset validator (`fn is_subset(child, parent) -> bool` per variant) | Becomes the place where every cap-discipline question gets fought. Auditing across 6+ Phase 5 variants becomes "read 6 validator functions" instead of "trust the bitmask." |
| Bit-subset for semantic, identity for meta | Breaks capability discipline: GRANT must be subsetable (otherwise grant is uncontrollable — recipient can grant infinitely). Meta MUST be reducible via derive. |

**Discipline:** rights bits must be designed independent. If a variant surfaces a real implication relationship (e.g., "CLOSE implies SEND/RECV are also valid"), the right fix is to redesign — make CLOSE its own Handle operation, not a rights bit. If Phase 5 surfaces a forced exception that can't be redesigned, that's an ADR amendment then.

## ABI summary

### Type changes

```rust
// kernel/src/sched/process.rs

/// Kernel-unique capability identifier. Monotonic u64 from a global atomic;
/// never recycled in Phase 2. Phase 2 SMP work decides whether per-CPU
/// allocation banks are needed.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct CapId(pub u64);

#[derive(Clone)]
pub enum Handle {
    Channel {
        channel: Arc<Channel>,
        side: EndpointSide,
        cap_id: CapId,
        rights: CapRights,
    },
    Process {
        target: ProcessId,
        cap_id: CapId,
        rights: CapRights,
    },
    // Phase 5+: Tensor, Model, InferenceTicket, File slot in here.
}

pub struct Process {
    // ... existing fields ...
    pub derivations: Mutex<BTreeMap<CapId, Option<CapId>>>,
}
```

### New syscalls

| # | Name | Args | Returns |
|---|---|---|---|
| 8 | `SYS_DERIVE` | `h: u32, new_meta: u8, new_semantic: u8` | `i64`: new handle index, or negative errno |
| 9 | `SYS_REVOKE` | `h: u32` | `i64`: 0 on success, or negative errno |
| 10 | `SYS_GRANT` | `channel_h: u32, src_h: u32, rights_subset: u16 (packed meta\|semantic)` | `i64`: 0 on success (blocks until recipient `sys_recv_grant`s), or negative errno |
| 11 | `SYS_RECV_GRANT` | `channel_h: u32, max_rights: u16 (packed)` | `i64`: new handle index (blocks until grantor `sys_grant`s), or negative errno |

### New errnos

```rust
/// `-EPERM` — capability operation denied (missing meta-right, or rights
/// subset violation in derive/grant).
pub const EPERM: i64 = -1;
```

**Errno numbering convention (project-wide rule, recorded here for citation).** POSIX-shaped errnos use POSIX numbers: EPERM=-1, ENOENT=-2, ESRCH=-3, EBADF=-9, ENOMEM=-12, EFAULT=-14, EINVAL=-22, EPIPE=-32, ENOSYS=-38. NEO-specific errnos with no POSIX analog use numbers outside the POSIX range — negative numbers below -100, or a documented separate range when one comes up. Rule of thumb: when the failure mode matches POSIX semantics, use POSIX numbers; otherwise pick a NEO number deliberately. Future ADRs cite this convention rather than relitigate it. libneo wrapper authors don't need to memorize a NEO-specific errno space when POSIX semantics already match.

### Mint sites get rights/cap_id

`sys_chan_create` mints two channel caps with full rights `CapRights { meta: USE|DERIVE|GRANT|REVOKE, semantic: SEND|RECV|CLOSE }` and fresh `CapId`s. `sys_spawn` mints a ProcessHandle with `CapRights { meta: USE|DERIVE|GRANT|REVOKE, semantic: KILL|WAIT|INSPECT }`. Both record `derivation[cap_id] = None` (root-minted).

### Grant rendezvous

`sys_grant` + `sys_recv_grant` use the existing IPC rendezvous machinery. Specifically: the same channel state machine (EndpointState) gains a `GrantBlocked { src_cap_id, src_rights, donor_pmap, waiter }` variant analogous to `SendBlocked`. The handoff atomicity is provided by the same single-CPU mechanism documented in ADR-011 (front-queue + bounded handoff quantum). **No new blocking primitive.** Phase 2 SMP must replace this alongside the IPC handoff invariants — same migration, same time.

The recipient's effective rights are computed by **three-way intersection** at rendezvous time:

```
actual = grantor_actual ∩ rights_subset ∩ max_rights
```

where each operand is a bitwise-AND of meta and semantic bytes:
- `grantor_actual` — the rights the grantor *holds*, looked up at rendezvous time. NOT what they declare.
- `rights_subset` — the rights the grantor *offers* via `sys_grant`'s third arg.
- `max_rights` — the rights the recipient *accepts* via `sys_recv_grant`'s second arg.

This is **defense in depth** (Principle 1 below). Both sides bound the cap independently; the kernel intersects all three. A misconfigured grantor that offers more rights than they hold gets clipped to what they hold; a recipient that declares a tight `max_rights` gets only what they accepted. Neither side has to trust the other's declaration.

If the result has `meta == 0` (no operations possible) the kernel returns `-EPERM` to both sides — granting an unusable cap is treated as caller error.

## Non-decisions (deferred)

- **Copy / Move ops** — Phase 3+ if a use case appears. Move is a no-op at the flat-table layer; copy is a convenience.
- **Per-variant subset validator** — defer until a Phase 5 variant surfaces a forced implication relationship that can't be redesigned. Not preempting.
- **Cross-process revoke under SMP** — Phase 2 BUILD-3 (SMP foundation) hardens the lock-ordering. Phase 2 BUILD-1 ships the single-CPU correct version.
- **`sys_wait(handle)` and `sys_inspect(handle)`** — Phase 2 sub-deliverables, but their ADRs are separate. WAIT and INSPECT semantic-right bits exist here so the rights vocabulary is forward-compatible; the syscalls land later.
- **Cap-id recycling and 64-bit exhaustion** — at 1ns per CapId allocation, 2^64 CapIds takes ~584 years. Not a Phase 2 problem.
- **Forwarding caps (cap → cap pointer)** — seL4 has this for some patterns; NEO defers. If Phase 5 needs it, that's an ADR.

## Implementation notes (load-bearing — read before changing)

1. **Derivation log is per-process, not global.** Re-litigates the BUILD-3 phantom-win lesson otherwise. Cleanup of the log happens at process exit, atomic with the existing handle-table drain. No global structure outliving Process Arcs.

2. **`CapId` allocation is a global atomic monotonic counter.** This IS a global structure, but it's append-only (no entries get removed; cap_ids are never recycled in Phase 2). The single-CPU implementation is `AtomicU64::fetch_add(1)`. Phase 2 SMP work decides whether per-CPU banks are needed for contention; the API surface is unchanged.

3. **Grant is derived-copy, not move.** The donor keeps the source cap. The recipient receives a fresh child `CapId` with parent = donor source `CapId` and rights = donor_actual ∩ rights_subset ∩ max_rights. The child install is atomic with rendezvous completion: there is no observable state where `sys_recv_grant` returns a handle before the child is installed, or `sys_grant` reports success without a matching recipient handle. Move remains deferred per Q2.

4. **Rights subset check is uniform bit-and.** No per-variant logic in `CapRights::is_subset_of`. If a future variant surfaces a forced implication relationship, that's an ADR amendment, not a per-variant validator escape hatch.

5. **Survive-exit is the cap lifetime model.** Caps held by other processes outlive their donor's exit. Invocation against a dead resource returns the appropriate errno (`-EPIPE` for gone channel peers, `-ESRCH` for gone process targets). The handle stays valid as a handle (can be passed, derived from, etc.) — only invocation fails. Documented here so future supervisor patterns aren't relitigated.

6. **Revoke is per-process plus cross-process descendant invalidation.** A revokes a granted-out cap → kernel walks A's derivation log to find local descendants AND walks granted-out cap's `cap_id` through other processes' tables. The cross-process walk requires acquiring those processes' handle-table locks. Phase 2 single-CPU + `without_interrupts` is correct; Phase 2 SMP BUILD-3 must define lock ordering.

7. **WAIT and INSPECT bits are reserved per Principle 2 (reserved-bit pattern).** `Handle::Process { target }` mints with full process semantic rights (`KILL|WAIT|INSPECT`) in BUILD-1. WAIT and INSPECT are defined in the rights vocabulary and can be carried through derive/grant, but have no gating effect until `sys_wait` and `sys_inspect` land in a later Phase 2 BUILD. At that point, the bits start gating the new syscalls. Zero ABI break. Future ADRs can cite this as the canonical reserved-bit example.

8. **`sys_close` deferred to Phase 2 BUILD-2 or later, per Principle 2.** CLOSE is reserved in the channel rights vocabulary but `sys_close` does not ship in BUILD-1. Today's "close" is implicit at process exit via `on_thread_reap → close_channel_peers`. When explicit close is needed (likely Phase 2 BUILD-2 once a use case appears), the CLOSE bit starts gating the new syscall — zero ABI break. **Discipline:** the CLOSE bit is enforcement, not decoration. When `sys_close` (or any future userspace-invoked close-equivalent) lands, it MUST gate on `semantic & CLOSE != 0` — without that gate, the rights vocabulary's reservation is theatre. Kernel-internal cleanup paths (`on_thread_reap → close_channel_peers` at process exit) are NOT capability operations; they operate on Channel state directly during cleanup and are exempt from the rights check by construction. The line is: userspace-invoked close-equivalents check the bit; kernel-cleanup paths don't.

9. **Capability composition discipline.** Capability operations compose. Each new syscall that operates on capabilities adds layers: handle lookup, rights check, variant dispatch, operation-specific validation, dispatch to the underlying primitive. BUILD-1's `sys_send` does four logical steps. `sys_grant` does six. Phase 5's `sys_inf_run` will do more (handle lookup + USE check on model + per-input-tensor lookup + READ check + per-output-tensor lookup + WRITE check + scheduler dispatch). Each layer must be justified by a real correctness, security, or accounting property. None should be removed for performance alone, but none should exist by reflex either.

   **Rule for future syscall additions.** When adding a syscall that operates on capabilities, the implementation PR description must explicitly enumerate each layer of indirection (handle lookups, rights checks, variant dispatches, validations) and justify each. Reflexive abstraction inherited from "every syscall looks up handles, checks rights, dispatches" is the bloat path. The justification can be one line per layer; the discipline is the enumeration, not the prose.

   **Audit trigger.** When a syscall's handler exceeds eight logical layers, surface to the lead before merging. Eight is not a hard ceiling; it is the threshold at which "is this layer count justified or has the abstraction crept" becomes worth a deliberate review. Most syscalls will land under this threshold; the ones that don't are worth a closer look.

   This rule applies to all capability variants — Channel, Process, future Tensor, Model, InferenceTicket, File. The composition pattern is uniform; the discipline must be too.

## BUILD-1 amendment

Phase 2 BUILD-1 resolved two ambiguities in this ADR before implementation:

1. **Grant is derived-copy, not move.** The original implementation note said the donor cap was removed from the donor table during grant. That was drift. `sys_grant` keeps the donor source cap installed and creates a fresh child cap for the recipient with `parent = donor_source_cap_id`. This is the only shape compatible with Principle 1's three-way rights intersection and with Phase 5's "RW holder grants R-only handles to many clients" pattern. Move remains a separate deferred operation per Q2.

2. **Revoke frees handle slots.** `sys_revoke(h)` sets every revoked handle-table slot to `None`; those slots are recyclable by normal lowest-free-index allocation. Subsequent invocation through the same userspace handle index returns `-EBADF`. Tombstones were rejected for BUILD-1 because diagnostics do not justify permanent table growth.

BUILD-1 also clarified spawn-time inheritance: inherited handles are derived-copy grants at process creation. The child receives fresh `CapId`s with parent links back to the parent's source caps, and the parent must hold `GRANT` on each inherited source. Without fresh child `CapId`s, the derivation walk cannot distinguish parent and child slots during revoke.

## K1a-0 amendment (2026-10-27): the graph lock and re-parenting

Two defects in the revoke walk, both found while building K1a (OT-66's capability-rooted open) and fixed
before it, because logout's revocation (OT-66 §3.5) depends on revoke being complete:

1. **CP-87.** Implementation note 6 left SMP lock ordering to a later build. The walk and the removal
   were separate steps, and a child installed between them survived. Now one global `CAP_GRAPH` lock spans
   every revoke, every install that names a parent (which re-checks that the parent is still installed;
   a grant instead reads its source in the same `CAP_GRAPH` section as its install), every close and
   every process teardown. Order: `Channel.state → CAP_GRAPH → PROCESSES → handles →
   derivations`.
2. **CP-88.** Q1's per-process derivation log lost a cap's edge when the cap was closed or its holder
   exited, cutting its children off from every ancestor. **New rule:** when a close or a teardown removes a
   cap's edge, its children are re-parented to its nearest ancestor that is not also being removed, in
   the same `CAP_GRAPH` section, before any other operation can observe the removal. (The code removes
   the closing cap's own edge first, then re-parents, all under the lock; wording corrected 2026-10-29
   after Muse's review of #166.) Q4 (survive-exit) is unchanged: a donor's exit still kills nothing in its recipients. The
   recipients' caps stay revocable by the donor's ancestors, which is what revoke has always promised.

The per-process logs stay (Q1). Since K1b (2026-10-29) they are reachable only through
`process::edges::Derivations`, whose every read and write takes the caller's `CapGraph` guard, so no path
can see or change an edge without holding `CAP_GRAPH`. The discriminator is `cap_graph_revoke`, and the kill-switch is
`neo-test-cap-graph-unlocked-control` (test only), which restores the previous layer and leaks on every
leg.

## Principles (named for future-ADR citation)

Three principles crystallized during ADR-010's design conversation. They generalize beyond capabilities — Phase 2 SMP, Phase 5 tensor caps, Phase 7 inference scheduling, and any future ADR involving multi-actor operations or forward-looking rights vocabularies will hit the same shapes. Future ADRs cite these by number rather than re-deriving them.

### Principle 1: Defense in depth on cap-rights

When transferring a capability across a trust boundary, the kernel intersects bounds from BOTH sides PLUS the donor's actual rights:

```
actual = donor_actual ∩ donor_offered ∩ recipient_max
```

The donor declares what they're offering (`donor_offered`); the recipient declares what they'll accept (`recipient_max`); the kernel uses what the donor actually holds (`donor_actual`). All three intersected; the kernel never trusts a declaration over a held fact.

This eliminates classes of bugs:
- Donor accidentally offers more than intended → bounded by `donor_actual`.
- Donor's rights changed (e.g., revoked) between intent and rendezvous → bounded by current `donor_actual`.
- Recipient code ends up position-dependent on donor behavior → bounded by `recipient_max`.

Phase 5 forward-compat: Tensor caps need this hard. A model-RW holder grants to inference clients; clients should declare R-only on the recipient side even if the grantor offers more. The bounded pattern handles this with no per-variant logic. Generalizes to any future operation where rights cross a trust boundary.

### Principle 2: Reserved-bit pattern

Rights vocabularies are forward-looking ABI surfaces. When future operations are anticipated, **reserve their rights bits NOW** even though the gating syscalls don't exist yet:

- The bit is defined in the rights enum at issuance time.
- Setting the bit at mint/derive/grant succeeds but doesn't gate anything (the syscall it would gate doesn't exist).
- When the syscall lands in a later BUILD, the bit starts gating it. Zero ABI break.

The alternative — adding bits later — forces a rights-model amendment and risks pre-existing caps being incompatible with new bit positions. Reserve early; activate when the syscall lands.

Today's reserved bits: Channel.CLOSE, Process.WAIT, Process.INSPECT. Phase 5 will want analogues for Tensor (KV-cache append, dedup-eligibility hints) and Model (inference-only vs gradient-capable). Establish the pattern now.

### Principle 3: Errors at the causing actor

When a multi-actor operation fails, the failure errno is reported to the actor whose action *caused* the failure. Other actors see the symmetric "operation didn't complete" errno — typically `-EPIPE` for IPC-style operations.

Examples:
- Grantor lacks GRANT meta-right → grantor gets `-EPERM`; recipient sees `-EPIPE`.
- Grantor's cap was revoked between `sys_grant` and rendezvous → grantor gets `-EBADF`; recipient sees `-EPIPE`.
- Recipient's `max_rights` clears everything the grantor offered (intersection is `meta == 0`) → both sides get `-EPERM` (the failure is caused jointly by the rights mismatch).

Each actor's failure model stays flat: they handle errnos their own actions can produce, plus the universal "operation didn't complete" signal. Future Phase 5 cap operations (failed tensor migrate, failed model dedup, failed inference dispatch) follow the same shape.

## Cross-references

- **ADR-004 §"Process-exit cleanup"** — cap-survive-exit composes with the existing close_channel_peers cleanup. The peer-wake EPIPE path runs unchanged when the underlying Channel's last Handle drops.
- **ADR-007 §Implementation note 6** — ESRCH semantics for stale ProcessHandle. Cap-survive-exit makes this the canonical answer for cap-on-dead-process across all variants.
- **ADR-011 — Single-CPU IPC handoff invariants and Phase 2 SMP replacement targets** — `sys_grant` + `sys_recv_grant` use the same handoff mechanism documented there. SMP retrofit is shared.
- **ANSPEC-001 §3.7** — capability-handle pattern for tensors. Phase 5 inherits this ADR's two-tier rights structure; tensor-specific semantic-right bits (READ/WRITE/SHARE) slot into the per-variant byte without touching meta-rights.
- **Bible Article 12.1** — capability discipline mandate satisfied by this ADR for Channel + Process; Phase 5+ extends.

## Test scenarios (Phase 2 BUILD-1 verification)

The integration test suite from Phase 1.5c gains `cap_*` scenarios:

1. **`cap_mint_and_use`** — anti-regression. `sys_chan_create` and `sys_spawn` still produce caps with full default rights; existing IPC and kill behavior unchanged.

2. **`cap_derive_subset`** — holder of cap with `{meta: USE|DERIVE, semantic: SEND|RECV}` calls `sys_derive(h, USE, SEND)`. New handle returned; new cap has reduced rights; original unchanged. SEND on the derived cap works; RECV on the derived cap returns `-EPERM`. Derivation log records `child_cap_id → parent_cap_id`.

3. **`cap_derive_rights_violation`** — derive with rights NOT a subset of parent (e.g., parent has SEND only, child requests SEND|RECV). Returns `-EPERM`. No handle installed.

4. **`cap_derive_missing_meta`** — derive against a cap without `DERIVE` meta-right. Returns `-EPERM`.

5. **`cap_grant_basic`** — A holds channel cap with `GRANT` meta. A grants a derived cap to B over their existing IPC channel. B's `sys_recv_grant` returns a new handle. B can use the cap; rights are the intersection of A's offer and B's max.

6. **`cap_grant_missing_meta`** — A grants a cap without `GRANT` meta-right. A's `sys_grant` returns `-EPERM`. B's `sys_recv_grant` returns `-EPIPE` — per Principle 3, the failure errno is reported to the actor that caused it (A); B sees the symmetric "rendezvous failed" signal that B's code already handles for any other peer-side failure mode.

7. **`cap_revoke_local`** — A holds a derived cap with `REVOKE`. A calls `sys_revoke(child)`. Subsequent invocations against the child handle return `-EBADF`. Parent unchanged.

8. **`cap_revoke_cross_process`** — A grants to B. A revokes the original. B's derived cap also becomes invalid (next invocation returns `-EBADF`). B's handle index is freed.

9. **`cap_revoke_descendants_walk`** — A grants to B; B derives a local child from the granted cap; A revokes original. Both B's granted cap and B's derived child become invalid.

10. **`cap_survive_exit`** — A grants to B; A exits. B's cap remains valid as a handle but invocation fails with the appropriate errno (`-EPIPE` or `-ESRCH` depending on cap variant).

The existing `expect_pass_but_fail` meta-test remains the runner self-check, and the cap scenarios are part of the default suite. A green default suite therefore proves both that the cap scenarios ran and that the runner still catches deliberately red tests.

## Future Phase 5+ extension

When Tensor / Model / InferenceTicket / File variants land:

- Add the variant to the `Handle` enum (additional arm; existing arms unchanged).
- Define semantic-right bits in a per-variant module (`tensor_rights::READ`, `tensor_rights::WRITE`, etc.). Apply Principle 2 (reserved-bit pattern) for any anticipated future operations — reserve bits at the variant's introduction, gate them when the syscalls land.
- Provide variant-specific syscalls that check `meta & USE != 0` before performing the operation. Failures inside the variant-specific implementation report errnos per Principle 3 (errors at the causing actor).
- Mint sites for the variant default to full rights; rights-reduction happens via the universal `sys_derive`.
- Cross-process transfers (e.g., model sharing) use the universal `sys_grant` / `sys_recv_grant` with the three-way intersection from Principle 1 — no per-variant grant code.
- No changes to derive/grant/revoke/USE/DERIVE/GRANT/REVOKE semantics — those compose uniformly.

The deduplication-by-content-hash machinery from ANSPEC §8.2 is orthogonal to capability discipline; it operates on the underlying storage layer, not on caps.

---

**Decision recorded.** Per Bible Article 27, this ADR is amendable by superseding ADR. Phase 2 BUILD-1 implements the decisions above; deviations during BUILD-1 either fold back into this ADR via amendment or land as superseding ADRs.
