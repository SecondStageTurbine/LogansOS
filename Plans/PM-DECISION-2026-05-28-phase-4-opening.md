# PM-DECISION — Phase 4 (Network Stack) — OPENING

**Date:** 2026-05-28
**Phase:** 4
**Status:** **OPEN / Active**
**Authority:** PL instruction 2026-05-28 ("Open Phase 4"), following Phase 3 formal close (`CLOSE-REPORT-2026-05-28-phase-3.md`) and the v1.7.2 ledger amendment marking Phase 4 Active.

## Phase 4 is open

Foundation phases 1–3 are complete (boot, memory, scheduling, userspace, IPC, SMP, capabilities, allocators, TLB, NeoFS/block/disk I/O — all bare-metal-verified). Phase 4 delivers the network stack, the last substrate phase before the AI-native phases (5–7) that justify the project.

## Scope (per Bible ledger line 217 + ratified THINK v0.2)

- In-kernel network stack; **IPv6 as default**, IPv4 supported (Bible Article 11.2).
- A **sockets-equivalent API that is our own**, not BSD sockets (Article 11.2) — designed and documented with rationale.
- **TLS** glue as a kernel concern to the extent the inference subsystem needs authenticated model downloads (Article 11.3); audited, minimal, no rolled-own primitives.
- **Bundled deliverables:** USB stack landing (Articles 9.7, 9.9) and USB Ethernet adapter support (Article 9.10, ADR-017 §D3) — Phase 4 is the landing point for these per the ledger.

## Cadence position (Article 14: OBSERVE → THINK → PLAN → BUILD/VERIFY)

- **OBSERVE** — done (`OBSERVE-2026-05-20-phase-4-network-stack-survey-cerebro.md`).
- **THINK** — done and **ratified**: `PHASE-4-THINK-v0.2-2026-05-22-network-stack-design-direction.md` (v0.1 superseded after Cerebro pressure-test per Article 24.2), ratified in `PL-RATIFICATION-2026-05-22-phase-4-think-v0.2-q1-q12.md`. This is the authoritative design input; not re-litigated here.
- **PLAN** — **next.** Write the BUILD-1 criteria (ISC-N items), break into commits, run the Article 14 `/simplify` gate (delete anything not needed for the goal milestone).
- BUILD/VERIFY — follows PLAN, per build.

## Build sequencing (from THINK v0.2 — reference, finalized at PLAN)

The THINK direction sequences Phase 4 as a stack-up: IPv6 + UDP foundation first, then IPv4, then TCP, then DNS + happy-eyeballs, then the TLS layer, with the USB / USB-Ethernet bundle landed where the driver dependency requires. PLAN will pin BUILD-1's exact scope and criteria against the ratified v0.2 answers (Q1–Q12).

## Anti-scope (opening guardrails)

- No AI subsystem work (Phase 5+).
- No WiFi (Phase 7.5 / ADR-017 — explicitly deferred; USB Ethernet is the Phase 4 connectivity path).
- No BSD-sockets compatibility shim in the native ABI (personality-edge only, Article 5.7).
- No rolled-own crypto primitives (Article 11.3).

## Immediate next step

PLAN-stage for Phase 4 BUILD-1. PM (Cerebro) to produce the BUILD-1 plan (criteria + commit breakdown + `/simplify` gate) from the ratified THINK v0.2, then dispatch implementation per roster (heavy kernel coding → Jarvis). Awaiting PL go to begin PLAN, or PL may direct depth/sequencing first.

---

**Phase 4 OPEN.** THINK ratified; PLAN is the next gate.
