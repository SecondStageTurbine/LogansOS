# PM-NOTE - Post-BUILD-4B Bible-Amendment Triage Packet

Date: 2026-05-20
From: PM
Status: PM RECOMMENDATION TO PL / NO AMENDMENTS PROPOSED

Authority:

- `docs/research/ALTIMIT-real-world-implementation.md` Recommendations §"Where
  Bible amendments are recommended"
- `docs/bible.md` v1.6.0 Articles 4.5 (single-node), 6.6 (inference-governance
  hooks; no narrative identity by implication), 9.2 (loadable kernel modules
  deferred), 23.1/23.2/23.3 (negative-space constraints), 27 (amendments)
- `Plans/PM-NOTE-2026-05-20-prometheus-dispatch-rescission-stale-context-error.md`
  §5 (initial triage list)
- `Plans/PM-DISPOSITION-2026-05-20-phase-3-build-4B-close-consumption.md` §7.3
  (commitment to surface the packet post-BUILD-4B closure)

## 1. Commitment Being Discharged

PM committed at BUILD-4B closure to surface the post-closure Bible-amendment
triage packet to PL. Per the rescission-note correction, the original
three-candidate list shrank:

| Original candidate | Status |
|---|---|
| Article 4.5 — single-node by design | **Already in Bible.** No action. |
| Article 23.4 — verifiable identity, no narrative identity | Optional. See §3. |
| Theseus-style cell-swap fault recovery | Live candidate. See §2. |

This note discharges the commitment by walking each remaining candidate and
making a recommendation.

## 2. Candidate A: Theseus-Style Cell-Swap Fault Recovery

### 2.1 What the research report recommends

ALTIMIT research report §F, Recommendation (Stage 0–1):

> **Article on fault recovery (proposed):** AltimitNeo adopts cell-swapping
> fault recovery for kernel components per the Theseus pattern; "no kernel
> panic" is a goal, "graceful component restart" is the mechanism.

Theseus OS (USENIX OSDI '20, Boos et al.) is a single-address-space OS in
safe Rust that uses intralingual design for isolation rather than hardware,
and supports **live evolution and fault recovery via cell swapping** — kernel
components can be replaced at runtime without bringing the system down, and
crashed components can be respawned with state recovered from neighbors.

### 2.2 The tension with current Bible Article 9.2

Bible Article 9.2 says:

> Driver loading is static at compile time for Phase 1–3. Loadable kernel
> modules are explicitly deferred and may never arrive — we would rather
> ship a clean monolith than reinvent the Linux kernel module disaster.

Theseus-style cell-swap is not Linux LKM. Theseus cells are crate-granular
units with explicit state-spill management and hot-swap support, designed
in safe Rust from the start. The mechanism is different from LKM. But the
*philosophical* commitment — kernel components can be replaced at runtime —
is what 9.2 currently refuses.

Adopting Theseus cell-swap would require either:

a. amending Article 9.2 to carve out an exception for Theseus-style cells
   (distinguishing them from LKM); or
b. amending Article 9.2 to soften the "may never arrive" stance; or
c. arguing that Theseus cells are not what 9.2 was forbidding in the first
   place (which requires re-reading 9.2 — currently it's broad enough to
   cover any runtime-replaceable kernel component).

None of these are casual edits.

### 2.3 What the engineering work would require

Theseus cell-swap is a substantial architectural commitment:

- Kernel components need to be loadable units, not just compile-time-linked
  crates.
- Each cell needs explicit state-spill management — a defined boundary at
  which a cell's mutable state can be serialized for restart-recovery.
- Inter-cell communication needs to survive cell-replacement events.
- The kernel's panic path needs to become a graceful-restart path for the
  affected cell.
- Memory ownership across cells needs careful design (Theseus uses a
  single-address-space model with intralingual safety; NeoPlutarch is
  monolithic Rust with disciplined module boundaries, which is similar but
  not identical).

None of this is on any current phase roadmap. Phase 3 (NeoFS), Phase 4
(networking), Phase 5–7 (AI subsystem) are the named upcoming phases. None
of them require cell-swap to land.

### 2.4 What fault recovery looks like *today* without cell-swap

The current Bible explicitly leans on Rust's safety as the primary fault-
elimination mechanism (Article 4.2, Article 15.2). The kernel panics on
unrecoverable errors (Article 15.3 permits panics only in init code; steady-
state code must handle errors). There is no Linux-style "soft lockup
recovery" or "OOM killer" (Article 7.6 explicitly refuses the OOM-killer
pattern; ANSPEC-001 §8.5 reinforces).

The current posture is "make the bug impossible, don't recover from it."
That's an opinion. Theseus's posture is "the bug will happen anyway; recover
gracefully." Both are defensible. NeoPlutarch's current posture composes
better with the project's small-team monolithic-kernel scale; Theseus's
posture composes better with very-long-uptime production systems.

### 2.5 PM Recommendation: DO NOT AMEND

PM recommends not amending the Bible to adopt Theseus cell-swap at this
time. Reasoning:

a. **No current implementation lane is blocked by the absence of cell-swap.**
   Phase 3 through Phase 7 plans don't require it. Amending the Bible to
   commit to an architectural pattern that won't be exercised for years is
   exactly the kind of speculative scope commitment Bible Article 1.3
   discourages ("we'll fix it later" hacks are forbidden, but the inverse —
   speculative architectural commitments — is similarly poor discipline).

b. **The Article 9.2 tension is real and would require multiple coordinated
   amendments.** Cell-swap is genuinely different from LKM but the broad
   reading of 9.2 forbids both. Resolving the tension cleanly is itself an
   ADR-class artifact.

c. **The right time to revisit is when fault recovery becomes load-bearing.**
   The earliest plausible moment is Phase 4 (network stack) — a kernel
   network stack that can lose connection state on driver fault is
   noticeably worse than one that can gracefully restart the affected
   driver. Phase 5+ AI workloads where long-running inference contexts can
   be invalidated by accelerator-driver faults is another candidate moment.

d. **The research-report recommendation is sound; the timing is wrong.**
   The reasoning in research report §F (especially the OSTD-style unsafe-
   boundary discipline and the Theseus cell-swap pattern as complementary
   fault recovery) is engineering-credible. Adopting both in coordinated
   form is the right long-horizon direction. But "right direction" doesn't
   imply "now."

PM recommends instead: file an **ADR-class artifact** capturing the Theseus
cell-swap consideration as a forward marker for Phase 4+ revisit. This
preserves the option without committing to architectural work that's not
on the roadmap.

Suggested artifact: `docs/ADR-022-fault-recovery-direction-forward-marker.md`
or similar; PL chooses ADR number and name. PM can draft it as a small
Prometheus task if PL signals.

## 3. Candidate B: Article 23.4 Substrate-Identity-Vs-Narrative-Identity

### 3.1 What the research report recommends

ALTIMIT research report §J, Recommendation (Stage 0):

> **Article 23.4 (proposed):** Verifiable kernel identity but no narrative
> identity. The kernel does not converse.

The framing: distinguish substrate identity (cryptographically verifiable —
boot attestation, kernel hash, capability provenance) from narrative
identity (conversational, persistent, model-backed). Forbid the latter at
the kernel layer.

### 3.2 The framing is already substantively in the Bible

Article 6.6 (added 2026-05-06) reads:

> 6.6 The kernel exposes inference-governance hooks but does not implement
> governance policy. ... The kernel does not ship a default safety filter,
> alignment layer, or content classifier. It ships the primitives by which
> userspace builds those things, capability-gated like every other kernel
> object.
>
> This clause exists because the alternative — a kernel-resident governance
> policy that runs on every inference by default — is the failure mode
> Articles 23.1 and 23.3 already foreclose, in a less-obvious form. A
> "default safety filter the kernel applies on every inference" is a system
> AI persona by another name and an embedded specific model by another
> name; both are forbidden. 6.6 is the positive version of that
> prohibition: the kernel exposes the seam on which governance can be
> built; the kernel does not stand on that seam itself.

This is stronger than the proposed 23.4 framing. 6.6 is the positive
constructive version (here's what the kernel does); the proposed 23.4 is
the negative prohibitive version (here's what the kernel doesn't do).

The substantive constraint is already locked in.

### 3.3 What 23.4 would add

A 23.4 amendment would add:

- explicit verifiable-substrate-identity framing (boot attestation, kernel
  hash, capability provenance as identity — the engineering-identity
  version);
- explicit "kernel does not converse" clause as an Article-23 sibling.

The first addition (substrate identity) is a forward-marker for the
KernelAttestation primitive that the research report §K recommends as
Stage-2 work. ANSPEC-002 already references it. The Bible can adopt the
framing without committing to implementation.

The second addition is rhetorical reinforcement of what 6.6 already
forecloses. It's belt-and-suspenders.

### 3.4 PM Recommendation: DO NOT AMEND

PM recommends not adding Article 23.4 at this time. Reasoning:

a. **Article 6.6 already does the load-bearing work.** The substantive
   prohibition on kernel-resident governance policy / system persona is
   already in the Bible at v1.0.4 (May 2026). Adding 23.4 as a sibling
   restates without strengthening.

b. **The KernelAttestation forward marker is better served by an ADR than
   by a Bible amendment.** The Bible commits the project to outcomes,
   not implementation primitives. KernelAttestation is implementation;
   it belongs with ANSPEC-001 or a future ANSPEC-003, not in the Bible.

c. **Bible amendment discipline.** Article 27: "The bible is not a casual
   document. Amendments are deliberate." Adding amendments that are
   substantively redundant with existing articles weakens the
   precedent for amendment discipline. Better to leave 6.6 standing
   and not dilute its weight.

If PL wants the substrate-identity framing in writing as a project
commitment, an ADR is the right home, not a Bible article.

## 4. Net PM Recommendation: NO BIBLE AMENDMENTS

After triage:

- Article 4.5 already in Bible (single-node scope). Complete.
- Article 6.6 already covers the substantive substrate-identity / no-
  narrative-identity constraint. No 23.4 amendment needed.
- Theseus cell-swap is a sound forward direction but premature to
  amend; file as ADR forward marker if PL wants the option preserved.

PM directs that the bible-amendment triage commitment is now discharged.
No amendments are proposed at this time. PL retains the right to direct
amendment work directly.

## 5. Suggested Follow-Up Artifacts (Optional, PL Decision)

If PL agrees with the no-amendment recommendation, three small artifacts
could be filed at PL's discretion:

a. **ADR-022 (or next ADR number) — Fault Recovery Direction Forward Marker.**
   Captures the Theseus cell-swap consideration, the Article 9.2 tension,
   and the conditions under which the project would revisit (Phase 4+
   network fault recovery, Phase 5+ accelerator fault recovery). Small
   Prometheus task.

b. **ANSPEC-001 forward-marker addition** for KernelAttestation primitive.
   Add to ANSPEC-001 §13 Open Questions a Q6 (or next number) referencing
   research report §K and noting that the primitive belongs to Phase 2 or
   Phase 8 work depending on TPM/DICE availability on target hardware.
   Small Prometheus task.

c. **No-op on 23.4.** No artifact needed; Article 6.6 stands.

PM does not authorize any of these without PL signal. They are surfaced
here so PL has the complete forward picture.

## 6. PM Posture

The bible-amendment triage commitment is discharged. PM-NOTE filed. PL
acknowledgment closes the triage loop.

Next standing forward items:

- BUILD-4C (FAT32 r/o) is dispatched to Jarvis as of today.
- Q6 (ANSPEC-002 §16 mount interaction) deferred to Phase 5 THINK per
  separate PM-NOTE.
- BUILD-5 bare-metal target selection pending PL hardware decision.
- Documentation hygiene sweep (BUILD-4B close artifact §1, ANSPEC-002
  §14 version-number, ANSPEC-002 §16.3 marker text) pending opportunistic
  Prometheus window.
