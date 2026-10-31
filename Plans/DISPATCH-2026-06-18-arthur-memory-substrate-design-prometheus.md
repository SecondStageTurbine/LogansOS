<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: design THINK — Arthur's persistent-memory substrate (OT-17) → Prometheus

**Date:** 2026-06-18
**From:** PM (Claude)
**To:** Prometheus (the research/design seat — the SAE-vector and CSPRNG-entropy design lanes are the precedent). New branch off `origin/main` HEAD: `prometheus/arthur-memory-substrate-design`.
**Status:** DISPATCHED. **Design only — a ratifiable THINK, no code.** The most important unbuilt frontier in the register (OT-17, the PL's "highest long-term importance"), and the single most privacy-sensitive thing the system will ever hold: **Arthur's memory of you.** This designs *where it lives, what governs access to it, and how it is forgotten* — ahead of the build, per OT-22 (the frontier is paid one pillar at a time, design-first).

## 0. The decision this build makes
None in code. It produces a **ratifiable design THINK** (options → tradeoffs → a recommended D1…Dn, the CSPRNG/Phase-9 THINK shape) for the substrate that holds Arthur's durable personal context, so that when the build comes it has a PL-ratified design instead of an improvised one. **No implementation. No syscalls added. No NeoFS change.** A design artifact.

## 1. The problem (why the slot arena is the wrong home)
The generational slot arena (ADR-042) is correctly **tensor-optimized**: RAM-resident, generational/reclaimable, dedup-able, read-mostly — right for *model weights*, wrong for *durable personal context*. Arthur's memory (facts about the user, preferences, relationships, project state, conversation history) is the opposite resource: **small, durable across reboot, mutable, and privacy-critical.** Today Arthur's context is a deliberately ephemeral userspace sliding window (OD-6). The durable substrate and — more importantly — its **capability/privacy model** are undesigned.

## 2. References
```
git fetch origin
git show origin/main:docs/OPEN-THREADS.md                                  # OT-17 (this), OT-18 (grant ergonomics — the boundary), OT-22 (slice the frontier)
git show origin/main:Plans/THINK-2026-06-18-arthur-destination-red-team.md  # §3 the find; the road->traveler framing
git show origin/main:docs/bible.md                                          # Article 23.2 (on-device, NO telemetry), Article 10 (NeoFS), Article 29 (Arthur)
```
Constraints that are *non-negotiable* and frame the whole design: **Article 23.2** (on-device, on the user's data, **no telemetry ever**); **Article 29.3/29.4** (Arthur is userland, capability-bounded, model-agnostic); **the capability primitives that exist** (`process::derive_handle` attenuation, `ipc::grant`, `process::revoke_handle`, revoke-through-the-derivation-graph — OT-20 confirmed these are built).

## 3. The design questions to resolve (the THINK's spine — give each an options→tradeoffs→recommendation)
- **D1 — Where do memories durably live?** NeoFS objects (Article 10) under a capability vs. a dedicated store vs. a structured DB-like layer. Durable, survives reboot, mutable. Recommend one; say why not the others.
- **D2 — What *is* a "memory" (the object model)?** The schema/granularity — opaque blobs? typed records (fact / preference / event / relationship)? an append log + derived index? What Arthur writes vs. what it reads back. Keep it minimal-sufficient (Article 15.7).
- **D3 — The capability/privacy model over the memory (the load-bearing one).** Who can read Arthur's memory of the user? A single user-held capability? Per-category caps (so a tool granted "calendar" can't read "health")? How an Arthur process is *granted* read/write to memory, attenuated (`derive_handle`) and revocable (`revoke_handle`). **This is where OT-17 meets OT-18** — scope it to *access to memory*; the broader grant ergonomics (friction across all Arthur actions) stays OT-18's separate THINK, cross-link it.
- **D4 — Encryption at rest.** Is memory encrypted on disk (now that CSPRNG 10.1 exists for key material)? Threat model: a stolen disk, a compromised non-Arthur process. Recommend a posture (and if "later," say so explicitly with the residual risk named).
- **D5 — Forgetting: revocation + erasure.** "Arthur, forget that" and "delete everything you know about me" must be real and verifiable (the privacy promise, Article 23.2). Soft-delete vs. cryptographic erasure (tie to D4) vs. tombstones. How erasure interacts with NeoFS (and any backups).
- **D6 — Failure / corruption semantics.** Memory must fail safe: a corrupt/missing memory store degrades Arthur to *no memory* (the ephemeral OD-6 window), never to a crash or a wrong-user's-memory. The kill-switch shape: memory-substrate-off ≡ today's ephemeral Arthur, bit-for-bit.

## 4. Discipline for the research
- **Prior-art survey, raw-source.** Survey how on-device assistants/agent frameworks handle durable memory + privacy (structured-memory designs, vector-store-vs-structured tradeoffs, the "right to be forgotten" mechanics) — but **per CLAUDE.md, never pull exact schemas/byte layouts through WebFetch**; cite raw sources. Design-level prose is fine to summarize; any concrete format/spec detail comes from the raw source.
- **One pillar (OT-22).** This THINK covers the **memory substrate + access-control to memory** only. It does NOT design: the general grant ergonomics across all actions (OT-18), multi-Arthur memory sharing/provenance (OT-20 — note the connection, defer), or the model's context-engineering (userland, separate). Name the boundaries explicitly.
- **No over-design (Article 15.7).** Minimal-sufficient: the smallest memory model that makes Arthur usefully remember a real user, with a real privacy/forget guarantee. Reject machinery that doesn't earn its place.

## 5. Deliverable + close (PM/PL)
A single ratifiable **`Plans/THINK-2026-06-18-arthur-memory-substrate-design.md`** with §1 problem, §2 constraints, §3 the design space, §4 D1–D6 with options→tradeoffs→**recommendation**, §5 the kill-switch/fail-safe shape, §6 the OT-18/OT-20 boundary, and a **§10 ratification block (D1…D6 stated for PL silent ratification)** — the CSPRNG/Phase-9 THINK shape. **No code, no CI gate** (it's a `Plans/` doc — paths-ignored). Close: push the branch; PM reviews for honesty + minimal-sufficiency + the non-negotiable constraints (23.2 no-telemetry, fail-safe, real forgetting); PL ratifies D1–D6; *then* a build dispatch follows — one slice, kill-switched, per OT-22. Until ratified, OT-17 stays OPEN.

**No disk-reclaim / close-gate ritual** — this is a design doc, not a code build (the close-gate ritual and `workflow_dispatch` gate apply to code branches; a `Plans/` THINK has neither).

---
## Relay — what to tell Prometheus
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-18-arthur-memory-substrate-design-prometheus.md`. **Branch:** `prometheus/arthur-memory-substrate-design` off `origin/main`.
- **One-line:** Produce a **ratifiable design THINK for Arthur's persistent-memory substrate** (OT-17 — the most privacy-sensitive thing the system will hold). **Design only, no code.** Resolve D1 *where memories durably live* (NeoFS + a capability, vs alternatives), D2 *the memory object model* (minimal-sufficient — what a "memory" is), D3 *the capability/privacy model over memory* (who can read Arthur's memory of the user; attenuated/revocable via the built `derive_handle`/`grant`/`revoke_handle` primitives — this is where OT-17 meets OT-18, scope to memory-access only), D4 *encryption at rest* (CSPRNG 10.1 now exists for keys; state the threat model), D5 *forgetting* (real, verifiable erasure — the Article 23.2 privacy promise), D6 *fail-safe* (corrupt/missing memory ⇒ degrade to today's ephemeral window, never crash or leak the wrong user's memory; kill-switch off ≡ today). **Non-negotiables:** on-device, **no telemetry ever** (23.2); Arthur is userland (29.4); minimal-sufficient (15.7). **One pillar (OT-22):** memory + memory-access only — defer general grant ergonomics (OT-18) and multi-Arthur memory (OT-20), cross-link both. Prior art raw-sourced, **never WebFetch for exact formats**. Deliverable: `Plans/THINK-2026-06-18-arthur-memory-substrate-design.md` with a §10 D1–D6 ratification block. PM reviews → PL ratifies → *then* a build slice follows.
