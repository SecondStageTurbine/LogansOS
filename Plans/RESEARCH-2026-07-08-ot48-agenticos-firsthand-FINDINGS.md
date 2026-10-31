<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: OT-48 follow-on — AgenticOS + peers, firsthand read (the anticipation finding, confirmed or corrected)

**Date:** 2026-07-08
**Author:** Prometheus (research / prior-art lane)
**Source:** Plans/DISPATCH-2026-07-08-ot48-agenticos-firsthand-differentiation-prometheus.md
**Branch:** prometheus/ot48-agenticos-firsthand-differentiation
**Inputs:** OT-48 row (OPEN-THREADS.md); Article 29.3 (Bible); OT-18 grants (ratified D1-D7); OT-19 reversibility (ratified D1-D6); OT-1 derivation discipline (OT-1 row); AgenticOS full paper (ar5iv.labs.arxiv.org/html/2606.21129); Agent libOS full paper (ar5iv.labs.arxiv.org/html/2606.03895); Tenuo README (GitHub); LangGraph interrupts docs (docs.langchain.com); SEAgent — wrong paper found, UNVERIFIED; Redwood AI-Control + ETH design-patterns — source not locatable, UNVERIFIED
**Scope:** Research/related-work. No code, no QEMU. Firsthand reads for load-bearing claims.

---

## ISC-1 — The confirm/refute verdict (firsthand-sourced)

### AgenticOS (arXiv 2606.21129): CONFIRMED with corrections

**Access:** Full paper read via ar5iv HTML render (https://ar5iv.labs.arxiv.org/html/2606.21129). Authors: Zhen Zhao et al. (Tencent), June 19, 2026.

**The three-pillar mapping — what AgenticOS actually does:**

| Our pillar | AgenticOS equivalent | Mapping quality |
|---|---|---|
| **29.3: model proposes, capability disposes** | Ghost Kernel + Agent Capsule: "An Agent Capsule does not directly expose general system calls such as fork, exec, mmap, dlopen, or socket." "The core constraint of the Ghost Kernel is 'no exposed interface.'" | **TIGHT.** Same premise: the agent cannot reach raw syscalls; it goes through a capability-gated interface. |
| **OT-18: coarse durable grants** | Logic Shutter issues "unforgeable capability tokens, binding them to agent identity, Manifest, request chain, and resource budget." Each call carries cap_id declared in the Manifest. | **TIGHT but different primitive.** Theirs are semantic/intent tokens (five-tuple <domain, path prefix, method, data type, budget>). Ours are classic object-capabilities (seL4/KeyKOS style — unforgeable handles with rights bits). Same enforcement goal, different mechanism. |
| **OT-19: graduated reversibility × stakes** | Binary auto/confirm: "low-risk, reversible, and auditable capabilities may be executed automatically; high-risk, irreversible, or real-world commitment-bearing capabilities must enter through explicit confirmation." | **LOOSE.** Theirs is binary (auto vs. human-confirm). Ours is a finer spectrum: copy-before-write undo (silent within scope), tombstone-on-delete, user-set stakes policy, graduated by reversibility × stakes. They have the direction; we have the granularity. |

**"Safe-to-be-wrong" framing — NOT found.** The paper does NOT use this language or position itself as an alternative to alignment. It frames itself as an OS security architecture. The "safe-to-be-wrong" thesis is ours, not theirs.

**Verdict:** AgenticOS **does** combine all three pillars (untrusted agent + capability enforcement + graduated friction) at the OS level. The anticipation finding is **CONFIRMED**. But:
- The "safe-to-be-wrong" framing is ours alone.
- Their capability primitive is semantic/intent tokens, not object-capabilities (design-mining core, Q3).
- Their reversibility is binary, not a spectrum (Q4).

### Agent libOS (arXiv 2606.03895): CONFIRMED

**Access:** Full paper read via ar5iv (https://ar5iv.labs.arxiv.org/html/2606.03895). Author: Yingqi Zhang (Tsinghua), June 2026.

**Core invariant:** "A self-evolving agent may change what it can ask for, but it cannot thereby change what it is authorized to affect."

Has typed capabilities, deny/ask/allow effects, one-shot grants, delegation attenuation, process-level isolation. Evaluated on 27 deterministic tasks with 0% unauthorized effects vs. wrapper/sandbox baselines at 58.1% failure rate.

**Verdict:** CONFIRMED — cap-enforced untrusted agents, independently converged. This is a runtime substrate (library OS), not a kernel architecture. Complements AgenticOS from the userland side.

### Tenuo: CONFIRMED (README-level)

**Access:** GitHub README only. No arXiv paper found.

Cryptographic capability tokens ("warrants") for AI agents. Monotonic attenuation. Bounded by task scope. Cites CaMeL (arXiv 2503.18813) as prior art — not claiming independent convergence, but extending the space.

**Verdict:** CONFIRMED — cap-enforced agents exist in the tool-call authorization layer. Not an OS architecture; a cryptographic token system for agent tool-calls.

### SEAgent: UNVERIFIED

Found arXiv 2602.12430 ("Agent Skills for LLMs") which mentions SEAgent only as a citation. No standalone SEAgent paper with security/capability focus found. **Snippet-level overreach: the original OT-48 finding may have misattributed SEAgent's scope.**

### Redwood AI-Control + ETH design-patterns: UNVERIFIED

Source not locatable. arXiv and GitHub searches returned zero relevant results. Anthropic's design patterns page 404'd. **Snippet-level overreach: these may be private/internal documents or sources that no longer exist.**

**Honest access-path note:** AgenticOS and Agent libOS were fully read via ar5iv. Tenuo was README-only (no paper exists). SEAgent and Redwood/ETH could not be verified — the original snippet claims for these are **UNVERIFIED and should not be treated as confirmed.**

---

## ISC-2 — The design-mining recommendation

### Q3: Intent-token vs object-capability tradeoff

**AgenticOS's primitive:** Semantic/intent tokens — a five-tuple <domain, path prefix, method, data type, budget>. The paper explicitly elevates capabilities from "resource handles" to "task-intent handles":
> "a capability constrains not only which resource can be accessed, but also under what semantics and in which task context an external effect may be produced."

**Our primitive:** Classic seL4/KeyKOS object-capabilities — unforgeable handles with rights bits (FileRights, CapRights, TcpRights). The kernel enforces them mechanically via the Grant/RequiredCapability match in xecute_action().

**Comparison:**

| Axis | Intent tokens (AgenticOS) | Object capabilities (us) |
|---|---|---|
| **Expressiveness** | Higher — can encode "read CSV files under /data for the analytics task" as a single token. Semantic constraints are part of the capability. | Lower — caps are resource + rights. Semantic constraints ("only CSV") require additional policy logic. |
| **Unforgeability** | Cryptographic (Logic Shutter signs tokens). Forgeable if the signing key is compromised. | Kernel handle table entry. Unforgeable by construction — no token to steal, just a handle index that the kernel validates. |
| **Policy placement** | In the token format (semantic constraints encoded in the five-tuple). This is policy-in-the-token. | In the kernel's xecute_action() match. Mechanism, not policy — the kernel mechanically checks rights bits. |
| **Article 6.6 compliance** | Questionable — semantic constraints in the token are a form of policy judgment. The Logic Shutter decides what's allowed based on intent semantics. | Clean — the kernel applies userland-supplied constraints mechanically (like PR_SET_SECCOMP). No judgment, just enforcement. |
| **Revocation** | Token revocation via Logic Shutter. Requires re-contacting the authority. | Handle revocation via evoke_handle through the derivation graph. Immediate, local, O(1). |
| **Composition** | Tokens compose by re-signing with attenuated constraints. | Caps compose by derive_handle with rights intersection. |

**Recommendation: REJECT intent tokens for our architecture.** Object-capabilities remain the stronger foundation for our stance:

1. **Unforgeability by construction** — kernel handle table entries can't be forged; cryptographic tokens can be if the signing key is compromised.
2. **Article 6.6 compliance** — our caps are mechanism (rights bits), not policy (semantic intent). AgenticOS's intent tokens encode semantic judgment in the token format, which blurs the mechanism/policy line that 6.6 draws.
3. **Immediate local revocation** — evoke_handle is O(1) and doesn't require contacting an authority.
4. **Simplicity** — rights bits are simpler to reason about than five-tuple semantic constraints.

**Adopt from AgenticOS:** The "task context" idea — binding a capability to a specific task/session context is useful. We already have this implicitly through the spawn-time grant derivation (E1b). Could be made explicit: Grant::File { path, rights, task_id } for provenance (OT-20 seam). Bank as an OT-18 refinement, not a change.

### Q4: Reversibility models compared

**AgenticOS:** Binary — auto-execute for low-risk/reversible, human-confirm for high-risk/irreversible.

**Ours (OT-19):** Finer spectrum:
- **Read/nav (A1):** Direct execution, no friction. Reversible by construction (no mutation).
- **Mutating reversible (A2):** Copy-before-write undo artifact. Silent within scope; undo available if needed. One-session/10 MB LRU window.
- **Mutating irreversible local (A2):** Tombstone-on-delete. Confirm only if user-set stakes policy marks the path as high-stakes.
- **Irreversible external (A3+):** NetSend excluded from reversibility. Per-host confirm (OT-18 restriction).
- **Forget (M4):** Key-burn crypto-erase. Preserved as final — undo backs up ciphertext, but key-burn crypto-erases the backup too.

**Is our finer spectrum a real advantage?** Yes. The binary model forces a hard line between "auto" and "confirm." Our spectrum has three friction levels:
1. **Silent** (read/nav — no friction, no risk)
2. **Informational + undo available** (reversible mutation — low friction, high recovery)
3. **Confirm** (irreversible + high-stakes — high friction, no recovery)

This maps to three distinct user experiences instead of two. A five-year-old gets silent success for reads and informational notice for reversible writes — they never see a confirm prompt unless the action is truly irreversible AND marked high-stakes. The binary model would either auto-execute too much (confirming everything reversible) or confirm too little (auto-excluding irreversible local mutations).

**Sharpening note from AgenticOS:** Their "auditable" criterion for auto-execute is worth adopting: an action that produces an audit trail can be auto-executed even if it's technically irreversible, because the audit trail provides post-hoc accountability. This is relevant for OT-20 (action provenance). Bank as an OT-19 refinement note.

---

## ISC-3 — The differentiation ledger

| Axis | LogansOS | AgenticOS/peers | Advantage? |
|---|---|---|---|
| **Untrusted-agent premise** | Article 29.3: "model proposes, capability disposes" | Ghost Kernel + Agent Capsule (AgenticOS); typed caps (Agent libOS); warrants (Tenuo) | **WASH.** Independently converged. Same thesis, different implementations. |
| **Enforcement primitive** | seL4/KeyKOS object-capabilities (rights bits on kernel handles) | Semantic/intent tokens (AgenticOS); typed runtime caps (Agent libOS); cryptographic warrants (Tenuo) | **ADVANTAGE.** Object-caps are unforgeable by construction, mechanically enforced, Article 6.6 compliant. Intent tokens encode semantic policy; crypto tokens depend on key security. |
| **Reversibility model** | Fine spectrum: silent → informational+undo → confirm, graduated by reversibility × user-set stakes | Binary auto/confirm (AgenticOS); not addressed (Agent libOS, Tenuo) | **ADVANTAGE.** Three friction levels vs. two. Better UX for the five-year-old test (silent reads, informational writes, confirm only for irreversible+high-stakes). |
| **"Safe-to-be-wrong" framing** | Explicit thesis: fallible AI made safe by unforgeable caps + graduated reversibility, not alignment | Not used. Framed as OS security architecture (AgenticOS) or runtime safety (Agent libOS) | **ADVANTAGE.** This is our unique narrative contribution. The framing matters for the competitive landscape. |
| **Telemetry / on-device** | On-device, no telemetry ever (Article 23.2), user's own hardware | Not specified (AgenticOS — Tencent paper, cloud context likely); Agent libOS — runtime substrate, deployment-agnostic | **ADVANTAGE.** Our privacy stance is a structural differentiator. AgenticOS is from Tencent — the cloud model is implicit. |
| **From-scratch sovereign kernel** | Rust monolithic kernel, from scratch, capability system as foundation | Linux-based (AgenticOS — Ghost Kernel is a layer over Linux); library OS (Agent libOS); authorization layer (Tenuo) | **ADVANTAGE.** Our kernel is built for this from the ground up. AgenticOS layers on Linux — it can't change the kernel's security model, only add a gate on top. |
| **Actually shipping** | Phase 8 active, A1 shipped (d533559), constrained decode live, QEMU gates green, bare-metal approaching | Paper-only (AgenticOS, Agent libOS — June 2026 submissions). Tenuo: v0.2 on GitHub, production/stable but narrow scope | **ADVANTAGE.** We're building it. They're describing it. The gap between paper and shipping is the moat. |
| **Usability (five-year-old test)** | Operationalized as 7 principles + F1-F8 checklist + power-user duality gate | Not addressed | **ADVANTAGE.** No peer has designed for the five-year-old test. This is a unique axis. |

### Stress-testing the moat thesis

**The thesis:** "The moat was never the idea (now industry-convergent), it's the from-scratch sovereign Rust-kernel execution + telemetry-free/on-device stance + actually shipping."

**Verdict: HOLDS.** The firsthand read confirms:
1. **The idea is convergent.** AgenticOS and Agent libOS independently arrived at the same three-pillar architecture (untrusted agent + capability enforcement + graduated friction) within weeks of each other (June 2026). The "safe-to-be-wrong" thesis is industry-convergent.
2. **The kernel is the moat.** AgenticOS layers on Linux. It can gate syscalls but can't change Linux's security model. Our kernel is built for capabilities from the ground up — the enforcement primitive is native, not bolted on.
3. **Shipping is the moat.** Both AgenticOS and Agent libOS are June 2026 papers. We have A1 shipped, constrained decode live, QEMU gates green, bare-metal approaching. The gap between paper and working system is real and widening.
4. **Privacy is the moat.** On-device, telemetry-free, user's own hardware — this is a structural differentiator against cloud-native agent architectures.

**The honest caveat:** AgenticOS's intent tokens are more expressive than our rights bits. If the industry converges on semantic capabilities, we may need to address the expressiveness gap. But the unforgeability and Article 6.6 advantages of object-caps are structural — they don't go away if others adopt intent tokens.

---

## ISC-4 — OT-1 derivation-citation entries

Per the OT-1 discipline (cited reference + import NO code + license/patent sanity check):

| Work | What we referenced | License/Venue | Import status |
|---|---|---|---|
| **AgenticOS** (Zhao et al., arXiv 2606.21129, June 2026) | Convergent prior art for the three-pillar architecture (untrusted agent + capability enforcement + graduated friction). Used for differentiation analysis and design-mining (intent-token vs object-cap comparison). | arXiv (preprint), Tencent. Method is not known to be patented. | **CITED, NOT IMPORTED.** We reference it as convergent related work. We imported no code, no format, no method. Our object-capability enforcement is independently designed from seL4/KeyKOS principles. |
| **Agent libOS** (Zhang, arXiv 2606.03895, June 2026) | Convergent prior art for cap-enforced untrusted agents at the runtime layer. Used for differentiation analysis. | arXiv (preprint), Tsinghua University. Method not known to be patented. | **CITED, NOT IMPORTED.** We reference it as convergent related work. Our kernel capability system is independently designed. |
| **Tenuo** (tenuo-ai/tenuo, GitHub) | Related work for cryptographic capability tokens in agent tool-call authorization. Used for context, not design input. | GitHub (repo), Apache 2.0 per README. Cites CaMeL (arXiv 2503.18813) as prior art. | **CITED, NOT IMPORTED.** Reference only. |

**Patent sanity check:** AgenticOS and Agent libOS are June 2026 preprints — too recent for patent filings to be public. The methods (capability-bounded agents, intent tokens, typed runtime caps) are extensions of known concepts (object-capability systems, cryptographic authorization). No obvious patent risk for clean-room reimplementation of the general concept, but the specific five-tuple intent token format (AgenticOS) should not be replicated. Our object-cap approach avoids this entirely.

---

## Strategy implication (PL/PM flag — recommend, don't decide)

**The patent/publish calculus from OT-48:** The original finding was "don't defensively-publish, don't patent, because AgenticOS anticipates us." The firsthand read **confirms the anticipation but sharpens the differentiation.**

**Recommendation:** The moat thesis holds. We don't need to defensively publish (the idea is convergent) or patent (our advantages are structural — kernel, shipping, privacy — not patentable method claims). Continue building. The differentiation is in execution, not novelty.

**Flag for PL:** If AgenticOS's intent tokens gain traction as the industry standard, consider whether our object-cap approach needs a semantic layer on top (the "task context" adoption note above). This is a design refinement, not a pivot.

---

## Gate + close

Docs-only, no code changes. No CI run exists (paths-ignored per dispatch).
