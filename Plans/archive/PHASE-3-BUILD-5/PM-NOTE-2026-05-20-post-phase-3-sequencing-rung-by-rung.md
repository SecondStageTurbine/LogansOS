# PM-NOTE - Post-Phase-3 Sequencing: Rung-By-Rung Bible Ledger Order

Date: 2026-05-20
From: PM
Status: SEQUENCING DECISION RECORDED / NORMATIVE FOR POST-PHASE-3 THINK ORDERING
Subject: PL signaled rung-by-rung phase progression. Phase 4 THINK opens before Phase 5 THINK. This note records the decision so the next PM seat has clarity on the ordering.

Authority:

- `docs/bible.md` v1.6.0 Article 13 (phase ledger)
- PL signal 2026-05-20: "lets go up a ladder rung by rung"

## 1. Decision

After Phase 3 closes (BUILD-5b bare-metal validation closure being the
final closure gate), the next phase to enter THINK is **Phase 4
(Network stack, IPv6 default, sockets-equivalent API)**, not Phase 5.

Phase 5 (AI subsystem foundation) opens after Phase 4 closes.

## 2. Reasoning

### 2.1 The Bible ledger is the canonical order

Bible Article 13 names phases sequentially: Phase 0, 1, 2, 3, 4, 5, 6,
7, 8. Phase 4 is the next phase after Phase 3 closes. PL's signal
matches the Bible's ordering rather than skipping ahead.

### 2.2 Network stack composes downstream

Phase 5+ work assumes some networking exists. Examples:

- ANSPEC-001 §10.4 (model integrity) and ALTIMIT research §K
  (KernelAttestation) both reference authenticated model downloads,
  which need a network stack.
- Future Phase 6+ accelerator drivers may have firmware update paths
  that involve network resources.
- ALTIMIT research §H (MMO-scale persistent worlds) is explicitly
  userspace concern, but the network primitives Phase 4 lands are the
  substrate those applications would use.

Phase 4 first means Phase 5+ inherits the network substrate without
having to retrofit assumptions.

### 2.3 Skipping is tempting; PL's discipline is correct

PM's earlier mild recommendation was Phase 5 THINK first, on the
reasoning that "Phase 5 is the project's reason for existing." PL's
counter-signal is correct on a discipline level: skipping phases to
get to the exciting work is the kind of shortcut Bible Article 1.3
discourages. Rung-by-rung progression is honest engineering.

PM accepts the correction.

## 3. What Opens When

Recording the sequence explicitly so the next PM seat doesn't have to
infer:

| Lane | Opens when |
|---|---|
| BUILD-5a-NVMe | Active now (dispatched 2026-05-20). |
| BUILD-5b (bare-metal validation) | PL acquires bare-metal hardware (T480s 20L8 path or alternative). |
| Phase 3 formal close | BUILD-5b closes per Bible Article 16.3. |
| **Phase 4 THINK** | **Phase 3 formal closure**. |
| Phase 4 BUILDs (network stack implementation) | Phase 4 THINK closes; ratified design contract in hand. |
| Phase 4 formal close | All Phase 4 BUILDs closed; bare-metal validation per Article 16.3. |
| **Phase 5 THINK** | **Phase 4 formal closure**. |
| Phase 5 BUILDs (AI subsystem implementation) | Phase 5 THINK closes; ratified ANSPEC-001/002 still standing. |
| ...etc. | Per Bible Article 13. |

## 4. Phase 4 THINK Substrate Already Preserved

PL pre-loaded design input for Phase 4 in
`Plans/PM-NOTE-2026-05-20-phase-4-network-design-input-pl-preload.md`.
That note becomes part of Phase 4 THINK's authority chain when THINK
opens.

PM directs that future PM seats consult that note when dispatching
Phase 4 THINK to Cerebro.

## 5. ANSPEC-002 Q6 Re-Sequencing

ANSPEC-002 v0.1.1 §16.3 / §12 Q6 was deferred "to Phase 5 THINK" per
`Plans/PM-NOTE-2026-05-20-anspec-002-q6-deferral-to-phase-5-think.md`.

With Phase 4 inserting before Phase 5 THINK, Q6 effectively waits one
additional phase. PM does not re-edit the ANSPEC-002 marker text; Q6's
substantive deferral target is "the natural moment when AI-object
capability design happens," which remains Phase 5 THINK regardless of
when Phase 5 THINK opens.

No artifact change required.

## 6. Lane Status (Unchanged By This Note)

- **Jarvis:** BUILD-5a-NVMe active.
- **Cerebro:** idle. Available for Phase 4 THINK when it opens (post
  Phase 3 closure).
- **Prometheus:** idle.

## 7. Forward Triggers

When BUILD-5b bare-metal validation closes and Phase 3 formally
closes, the next PM seat should:

1. File a Phase 3 close artifact per Bible Article 18.2 if not already
   prepared.
2. Open Phase 4 THINK with Cerebro as the research-lane agent.
3. Include in Phase 4 THINK's authority chain:
   - `Plans/PM-NOTE-2026-05-20-phase-4-network-design-input-pl-preload.md`
   - This note (sequencing decision)
   - Bible Article 11 (current network posture)
   - Phase 3 close artifact (substrate state)

PM does NOT open Phase 4 THINK in this note. Phase 3 still has BUILD-5a-
NVMe and BUILD-5b ahead.
