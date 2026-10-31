# Bible Amendment Draft: Articles 3, 5.6 (new), and 28.2 — Personality subsystems formalized

| | |
|---|---|
| **Type** | Bible amendment proposal |
| **Status** | DRAFT — PM-authored proposal, awaiting project lead ratification |
| **Date** | 2026-05-21 |
| **Authority** | Project lead (per Bible Articles 24, 27) |
| **Author** | PM |
| **Affects** | Article 3 fourth bullet (edit); Article 5 (new sub-article 5.6); Article 28.2 (cross-reference update); footer (version bump v1.6.0 → v1.7.0) |
| **Co-ratified with** | `Plans/ADR-023-DRAFT-2026-05-21-linux-personality-layer.md` |
| **Triggered by** | PL question 2026-05-21: "I don't want to force people to build something just for Plutarch, can we make it so we can easily port Linux apps and games to the OS?" — and PL choice of Option B (full ADR + Bible amendment) over Options A (forward-marker only) and C (shelve) per `Plans/PM-NOTE-2026-05-21-prometheus-dispatch-rescission-bible-ratification-stale-context.md` |

---

## Rationale

Bible Article 28.2 (Product Accessibility, ratified prior to v1.6.0) commits the project to a concrete goal: *"Run useful software. The POSIX compatibility shim (Article 3) means common command-line software with sane build systems can be ported with reasonable engineer effort. NeoPlutarch is not Linux, but it is not so alien that the existing software ecosystem is unreachable."*

Bible Article 3's fourth bullet (ratified at v1.0.0) authorizes that compatibility shim in a single sentence: *"A POSIX-compatible system. We may offer a POSIX compatibility shim for porting userland tools, but the native ABI is our own."*

The sentence permits the work but does not specify the mechanism, the binding model, or the isolation guarantees. Two failure modes follow from leaving this unspecified:

1. **Ad-hoc creep into the native ABI.** Without a defined mechanism, individual ports paper over individual missing Linux features by extending the native syscall surface. The native ABI degrades into "POSIX, but slightly different" — the worst of both worlds, and a direct violation of Bible Article 1.4 ("Parity is not a goal; rightness is").
2. **Undelivered accessibility commitment.** Article 28.2's promise of "reasonable engineer effort" porting acquires no architectural backing. Each Linux port becomes a bespoke negotiation.

ADR-023 (`Plans/ADR-023-DRAFT-2026-05-21-linux-personality-layer.md`) ratifies the architectural mechanism: a Linux **personality subsystem**, kernel-resident, opt-in per process, isolated from the native ABI by construction. This amendment encodes that mechanism as Bible-level project policy so future ADRs (a possible BSD personality; the Phase 9+ graphics personality; the Phase 8 PERSPEC-001 implementation contract) can cite it as authoritative rather than re-deriving it.

The amendment is additive on Article 5 (a new sub-article 5.6 — no existing 5.X content modified) and a minor edit on Articles 3 and 28.2 to cross-reference the new mechanism. The amendment does not modify any other article.

The project handles foreign-OS compatibility through the personality mechanism *only*; this amendment does not authorize ad-hoc shims in the native ABI. Per the bright-line discipline established in ADR-023 §Q3, additions outside the declared mechanism require a superseding ADR.

---

## Proposed amendment

### Amendment 1: Article 3 fourth bullet (edit)

**Current text (Bible v1.6.0):**

> A POSIX-compatible system. We may offer a POSIX compatibility shim for porting userland tools, but the native ABI is our own.

**Proposed replacement:**

> A POSIX-compatible system natively. The native ABI is our own. Foreign-OS compatibility, including POSIX/Linux, is provided through opt-in personality subsystems per Article 5.6 — not by adopting POSIX as the native shape, and not by extending the native ABI to mimic POSIX semantics piecemeal.

### Amendment 2: Article 5 — insert new clause 5.6 after existing 5.5

**Insert immediately after Article 5.5 (which ends "Fork is a 1970s artifact that fights every memory-management technique invented since."), before Article 6 begins:**

> 5.6 NeoPlutarch natively supports its own ABI. Foreign-OS binary compatibility, when provided, is delivered through opt-in personality subsystems that translate foreign syscalls to native kernel operations at the syscall boundary. Personality layers are kernel-resident, opt-in per process (bound at `exec` time and inherited by descendants), and isolated from the native ABI by construction: a personality-tagged thread cannot invoke a native syscall by number, and a native thread cannot invoke a personality syscall by number.
>
> Personality layers must not alter native ABI semantics. Native applications never observe the presence or absence of any personality. The native ABI evolves on its own schedule and does not negotiate with personality requirements.
>
> Personality subsystems live under `kernel/src/personality/<name>/`. Each personality is justified by its own ADR; the existence of one personality does not authorize others. Personalities are statically compiled (Bible Article 9.2 forbids loadable kernel modules) and may be conditionally compiled out via Cargo features for embedded or minimal builds.

### Amendment 3: Article 28.2 — cross-reference update

**Current text (Bible v1.6.0):**

> Run useful software. The POSIX compatibility shim (Article 3) means common command-line software with sane build systems can be ported with reasonable engineer effort. NeoPlutarch is not Linux, but it is not so alien that the existing software ecosystem is unreachable.

**Proposed replacement:**

> Run useful software. Foreign-OS compatibility is delivered through opt-in personality subsystems (Articles 3, 5.6). The Linux personality (ADR-023) is the first such subsystem; once delivered in Phase 8+, common Linux software runs on NeoPlutarch hardware with no per-application porting. NeoPlutarch is not Linux, but it is not so alien that the existing software ecosystem is unreachable.

### Amendment 4: Footer — version bump and amendment line

**Current footer (Bible v1.6.0):**

> Document: NeoPlutarch Bible
> Version: 1.6.0
> Status: Ratified
> Ratified: 2026-05-02 (commit aa2c949)
> Last amended: 2026-05-11 — Article 17 source tree diagram root renamed `altimitneo/` → `neoplutarch/` to align with the project rename ratified in Article 25.1. Articles 25.1 and 25.2 historical references retained as institutional memory per PM disposition in `Plans/DISPATCH-2026-05-11-bible-amendment-article-17-prometheus.md`. Earlier amendments preserved from v1.5.0 footer history.
> Authority: Project Lead

**Proposed replacement:**

> Document: NeoPlutarch Bible
> Version: 1.7.0
> Status: Ratified
> Ratified: 2026-05-02 (commit aa2c949)
> Last amended: 2026-05-21 — Article 3 fourth bullet edited, new Article 5.6 added formalizing personality subsystems as the mechanism for foreign-OS binary compatibility, and Article 28.2 cross-reference updated to point at the new mechanism. Co-ratified with ADR-023 (Linux binary compatibility via personality layer). v1.6.0 history: 2026-05-11 — Article 17 source tree diagram root renamed `altimitneo/` → `neoplutarch/` to align with the project rename ratified in Article 25.1. Articles 25.1 and 25.2 historical references retained as institutional memory per PM disposition in `Plans/DISPATCH-2026-05-11-bible-amendment-article-17-prometheus.md`. Earlier amendments preserved from v1.5.0 footer history.
> Authority: Project Lead

---

## Consequences

If ratified:

- **Articles 3, 5.6, 28.2 become binding project policy.** Future ADRs that propose alternative foreign-OS-compatibility mechanisms (e.g., a userspace-only libc shim, a VM-based hosting model) are pre-rejected by Article 5.6's "syscall boundary translation, kernel-resident, isolated from native ABI by construction" specification. Such ADRs would have to either supersede this amendment or fit within Article 5.6's bounds.
- **ADR-023 co-ratifies and becomes the first ADR-class document under Article 5.6.** The Linux personality is the first personality subsystem; the mechanism is permitted by Bible language for future personalities (BSD, Plan 9, etc.) but no such personality is authorized without its own ADR per Article 5.6's text.
- **Article 28.2's accessibility commitment becomes architecturally backed.** The Bible's product principle (Article 28) acquires a concrete mechanism (Article 5.6) for one of its key claims (Article 28.2).
- **Bible version bumps from v1.6.0 to v1.7.0.** This is a minor version bump per Article 27, matching the precedent for additive amendments that change one article meaningfully (5.6 added) and touch others lightly (3 and 28.2 cross-references). Patch-level bumps (e.g., v1.6.1) are reserved for typographical or formatting fixes; structural additions earn a minor bump.
- **No existing article semantics are reversed.** Article 3's fourth bullet edit replaces a permissive carve-out ("we may offer a shim") with a specification of *how* the carve-out works ("through personality subsystems per Article 5.6"). The permission remains; the mechanism is now named. Article 28.2's cross-reference edit updates pointer terminology only — the substantive promise ("software ecosystem is reachable") is unchanged.

If rejected:

- ADR-023's design intent stands but lacks Bible-level authority. The personality layer's existence remains permitted by Article 3's "POSIX compatibility shim" sentence but is not architecturally bounded by Bible policy.
- Future contributors may propose alternative foreign-OS-compatibility mechanisms without superseding Article 5.6 (because it does not exist) — increasing the risk of ad-hoc native-ABI extensions per the rationale section above.
- The Option B path PL chose is partially undone; the project effectively reverts to Option A (forward-marker ADR only), which PM had recommended.
- PL's standing decision should be recorded with rationale.

## Cross-references

- **Bible Article 1.4** — "Parity is not a goal; rightness is." Article 5.6's "isolated from native ABI by construction" enforces non-parity at the type/dispatch level.
- **Bible Article 3** (fourth bullet, amended) — points to Article 5.6 for the mechanism.
- **Bible Article 5.5** — "no fork() — process creation is spawn()." Article 5.6's personality layer emulates `fork` inside the personality, not in the native ABI; Article 5.5 holds.
- **Bible Article 6.6** — "kernel exposes hooks, does not implement policy." Article 5.6 follows the same posture for foreign-OS compatibility.
- **Bible Article 9.2** — "loadable kernel modules forbidden." Article 5.6 specifies personality subsystems are *statically compiled*, satisfying this constraint.
- **Bible Article 12.1** — "every kernel object is capability-handle-referenced." Linux FDs map to capability handles in personality dispatch.
- **Bible Article 18.1** — ADRs are mandatory for choices future-us might second-guess. ADR-023 is the first ADR under Article 5.6.
- **Bible Article 27** — amendment procedure. This amendment names affected articles, includes rationale, bumps version, awaits PL approval.
- **Bible Article 28.2** (cross-reference, amended) — points to Articles 3 and 5.6 for the mechanism that delivers the accessibility promise.
- **ADR-023** — `Plans/ADR-023-DRAFT-2026-05-21-linux-personality-layer.md`; co-ratified.
- **PM-NOTE 2026-05-21** — `Plans/PM-NOTE-2026-05-21-prometheus-dispatch-rescission-bible-ratification-stale-context.md`; the rescission record that surfaced the Option A/B/C choice and PL's selection of Option B.

---

**End of amendment draft.**
