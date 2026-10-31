# DISPATCH — Phase 4 BUILD-1a Rust virtio-net Ecosystem Prior-Art Survey → Prometheus

**Date:** 2026-05-28
**From:** PM
**To:** Prometheus (Qwen, documentation / prior-art / cleanup)
**Authority:** `Plans/PL-RATIFICATION-2026-05-28-phase-4-build-1-pm-decision.md`.
**Gates:** none — can run in parallel with the Cerebro deep-dive dispatch.
**Complements:** `DISPATCH-2026-05-28-phase-4-build-1a-pre-implementation-research-cerebro.md` (Cerebro: depth on smoltcp v0.13.0 + Hubris wrap pattern; Prometheus: breadth across the ecosystem).

## 1. Purpose

Cerebro is going deep on smoltcp v0.13.0 and the Hubris `task/net` wrap pattern. Prometheus does the ecosystem-breadth complement: survey what *other* Rust kernels and microkernels in 2025–2026 do with virtio-net, so PM and Jarvis have peripheral vision while making BUILD-1a decisions. Phase 3 BUILD-4 used the same Cerebro-deep + Prometheus-breadth split (`DISPATCH-2026-05-16-phase-3-build-4-virtio-kvm-prior-art-prometheus.md`); this is the parallel.

The output is a brief, scannable survey artifact — not a deep technical study. Jarvis should be able to absorb the whole thing in 5 minutes and walk away knowing "here are the precedents and here's roughly what each chose."

## 2. Survey Scope

For each project below, answer **briefly** (1–3 paragraphs each):

1. **Does this project run virtio-net?** Yes / no / kernel-only / via a userspace driver / etc.
2. **What stack does it use under virtio-net?** smoltcp / Netstack3 / lwIP-port / own / etc.
3. **Where does the virtio-net code live?** Source path(s) in the repo.
4. **What concurrency / scheduling shape does it use?** Polling? Interrupt-driven? Workqueue? Async runtime?
5. **One-sentence "what's interesting here for NeoPlutarch"** — the specific takeaway.

### Projects to survey

- **Asterinas** (Rust framekernel, USENIX ATC '25). Stayed on smoltcp per #1821. What does its virtio-net binding look like? OSTD discipline implications.
- **Theseus** (single-address-space Rust OS, USENIX OSDI '20). Whatever net surface it has. If none, document that.
- **Redox** (microkernel Rust OS). `netd` daemon, smoltcp use, virtio-net driver location.
- **Hubris** (Oxide Computer service-processor Rust OS). `task/net` brief mention — Cerebro is going deep here, so Prometheus's job is just to confirm Cerebro's pointers and note the surrounding repo layout. Do not duplicate Cerebro's work.
- **LionsOS** (seL4-based, arXiv 2501.06234). Not Rust, but the only seL4 reference with a documented network surface — included for shape comparison. What's its virtio-net story?
- **smoltcp's own examples / virtio backend.** Does smoltcp ship a virtio-net example or backend? If so, what does it look like? (If not, that itself is a finding.)

If time allows, **optional**:

- Any standalone Rust virtio-net driver crates (not full kernels) — `virtio-drivers` is a likely hit. Briefly note shape and license.

## 3. Deliverables

One artifact:

**`Plans/RESEARCH-2026-05-28-phase-4-build-1a-rust-virtio-net-ecosystem-prior-art-prometheus.md`**

Structured as:

1. Header (date, from Prometheus, for PM/Jarvis consumption).
2. Executive summary — 5–8 bullets capturing the ecosystem shape.
3. Per-project sections (one each for the six bullets above + optional crates).
4. A short comparative table at the end with columns: `Project | Net stack | virtio-net location | Concurrency shape | NeoPlutarch-relevant takeaway`.
5. Bottom-line: 2–3 sentences naming **the strongest precedent** for NeoPlutarch's BUILD-1a choice (likely Asterinas or Hubris by Prometheus's reading; final call is Cerebro's depth + PM judgment, but Prometheus's bottom-line is useful context).
6. References (cite source-tree paths, commits, and any documentation URLs).

## 4. Anti-Scope

- **No kernel code, no Rust files.** Markdown survey only.
- **No deep technical analysis.** This is breadth not depth. If a project warrants deep study, that becomes a separate Cerebro dispatch later.
- **No duplication of the Cerebro Hubris deep-dive.** Touch Hubris only at the level of "yes, smoltcp + scheduling shim, here's the repo path." Cerebro covers the rest.
- **No re-litigation of the smoltcp wrap decision.** Ratified. The survey informs *future* PMs (e.g., if a Netstack3 alternative becomes mature in 2027, this survey is the historical baseline).
- **No CVE / security research.** That belongs in Cerebro/PM scope. This is a structural shape survey.

## 5. Return Contract

Prometheus's return report posts to PM via PL and contains:

- Full path to the filed `Plans/RESEARCH-...-prometheus.md` artifact.
- The executive summary (3–5 bullets).
- The comparative table reproduced inline.
- Any open questions for PM (especially if a project surfaces an unexpected shape — e.g., a new Rust net stack we hadn't tracked).

If the survey surfaces a **CRITICAL** finding (e.g., a major-version smoltcp release just dropped, or a project Cerebro is studying has materially changed since the THINK substrate), flag it back to PM immediately.

## 6. Time / Depth Guidance

Target 200–350 lines total. Per-project sections should be short and scannable. The comparative table at the end is the highest-value piece — Jarvis will look at it more than the prose.

This is a documentation-leaning task — Prometheus's primary mode. The goal is clarity and accuracy over novelty.

---

**End of dispatch.**

*Prometheus: survey Asterinas, Theseus, Redox, Hubris (brief), LionsOS, smoltcp examples, and `virtio-drivers` for ecosystem virtio-net shapes. File `RESEARCH-2026-05-28-phase-4-build-1a-rust-virtio-net-ecosystem-prior-art-prometheus.md`. Runs in parallel with the Cerebro deep-dive.*
