<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NORTH STAR: The usability review LogansOS is trying to earn

**Date:** 2026-07-11
**Author:** Prometheus (from `Plans/RESEARCH-2026-07-11-zorin-usability-FINDINGS.md` ISC-5)
**Source:** The PL's prompt — make LogansOS the OS someone someday writes about the way Jack Wallen wrote about Zorin OS. This paragraph is that review, drafted in Wallen's register, with a falsifiable mechanism checklist beneath it.

---

## The paragraph

> I've been calling LogansOS the most intuitive operating system I've ever used for some time. It's the ideal OS for anyone who's tired of learning computer metaphors — you just tell it what you want, in your own words, and it does it. There's no desktop to learn, no menus to navigate, no commands to memorize. One reason LogansOS stands above the rest is that it understands you even when you speak in Windows or macOS terms — you say "open Task Manager" and it shows you what's running, gently naming what things are called here. Zero learning curve because everyone already knows how to talk. Another reason is that it never hits a dead end — if you ask for something it doesn't have, it suggests what it does and offers to make it undoable. It feels like the computer finally learned your language instead of forcing you to learn its.

---

## Mechanism checklist

| # | Sentence | Mechanism | Owner | Status |
|---|---|---|---|---|
| S1 | "the most intuitive operating system" | F1-F8 gate + real-user validation | All Arthur surfaces | **NOT YET** — needs real users |
| S2 | "tell it what you want, in your own words, and it does it" | OT-26 A1-A3 agentic loop (NL → ActionKind → cap-check → result) | OT-26 | **EARNED for read/nav** — unearned for arbitrary intent |
| S3 | "no desktop to learn, no menus, no commands to memorize" | Conversation-first paradigm, Article 1.5; v1 console | OT-26 / OT-41 | **EARNED** |
| S4 | "understands you even when you speak in Windows or macOS terms" | Familiarity translation layer (curated foreign-idiom → intent mapping) | **OT-55 (proposed)** | **NOT YET** — new OT needed |
| S5 | "you say 'open Task Manager' and it shows you what's running" | Translation layer: "Task Manager" → InspectProcess + butler naming | OT-55 + OT-26 A1 | **NOT YET** — depends on OT-55 |
| S6 | "gently naming what things are called here" | Butler register obligation: native term naming on translation-layer hits | OT-46 | **PARTIAL** — pattern exists, needs standing obligation |
| S7 | "never hits a dead end" | Redirect chain: clarify (D4) → catalog suggestion (OT-44 H2) → explain-never-vanish | OT-26 D4 + OT-44 H2 | **PARTIAL** — D4 shipped, catalog wiring needed |
| S8 | "offers to make it undoable" | OT-19 reversibility: copy-before-write, tombstone, butler mentions it | OT-19 A2 | **EARNED** |
| S9 | "the computer finally learned your language" | Translation layer + conversation-first = no foreign vocabulary required | OT-55 + paradigm | **EARNED in principle, NOT in practice** — needs OT-55 build |

---

## How to use this

This is the outward-facing sibling of F1-F8. *(Companion gate: **the Silent Test** — PL standing requirement 2026-07-11, banked in OT-52 — every core flow operable by pointer alone and keyboard alone, no speech or typing ever required; the review must be earnable by a mute user with no mic.)* F1-F8 gates individual surfaces ("can a five-year-old form the request?"). This paragraph gates the **whole product's reputation** ("would someone write about us this way?").

Revisit at:
- **v1 shipped:** Score which sentences are earned. Expect S3, S8; partial S2, S7; unearned S4-S6, S9.
- **Pre-1.0:** All sentences should be at least PARTIAL. S1 ("most intuitive") earns itself when someone writes it — we can't build it directly.
- **Post-real-users:** S1 either fires or the paragraph needs revision. This is the falsifiable bar.
