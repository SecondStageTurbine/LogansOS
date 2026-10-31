<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: HarmonyOS / HongMeng — transferable lessons for NeoPlutarch

**Date:** 2026-07-03
**Source:** deep-research pass (71 agents; 21 sources fetched → 14 claims → 3-vote adversarial verify → 13 confirmed / 1 refuted). Primary sources: the **OSDI'24 peer-reviewed paper "Microkernel Goes General: … the HongMeng Production Microkernel"** (Huawei + SJTU IPADS) and **DarkNavy's AVSS kernel evaluation** (independent, May 2024). **Status:** research note; not a build. Records what's worth taking, with the marketing filtered out.

## 0. The headline (honest version)
HarmonyOS NEXT's transferable substance lives in its **HongMeng (HM) microkernel**, and — importantly — the research mostly **validates NeoPlutarch's existing bets** (monolithic + Rust memory-safety + I-14 fault-containment, *scoped* formal verification) rather than handing us big new features. That external validation is itself the payoff. The genuinely new *ideas* are design-guidance and a couple of small builds, not a feature to clone. **Two of the three things we most wanted — the deterministic-latency scheduler and the Harmony "intents" framework — produced NO verified claims this pass** (see §6); treat them as *not yet researched*, not *absent*.

**Re-scoping note (2026-07-04, monolithic-identity correction):** this THINK's transferability was partly framed assuming a microkernel. For our monolithic kernel (Bible 4.1, ADR-001): (a) the **IPC-frequency lesson** shifts from "kernel service IPC" (we have none — it's in-kernel function calls) to the **userspace↔kernel syscall boundary + Arthur's tool-call path** (still real — the Jarvis measurement stands); (b) the **graded-isolation / policy-free-paging** items are **largely N/A** (we don't isolate services into address spaces by design — Bible 4.1, see `THINK-2026-07-04-isolation-class-taxonomy.md`); (c) the **DarkNavy memory-safety lesson gets *stronger*** — it validates the monolithic-because-Rust choice: the strongest shipping microkernel OS's demonstrated weakness is memory-corruption, the exact class we close by construction.

Verified-fact vs marketing: Huawei's **"formally verified line by line / every line mathematically proven"** is **marketing gloss** — the real, credible scope is a **small seL4-class TEE/microkernel core (scheduling + IPC), proven *against a spec***, not the whole OS. And a stronger claim that HongMeng **replaces capabilities** with address-tokens was **refuted** — address-tokens *supplement* capabilities; capabilities remain foundational.

## 1. The single most transferable idea — HongMeng's cost re-framing
**What HM says (OSDI'24, verified 3-0):** the dominant microkernel overhead is **not** per-invocation IPC *latency* — it's **IPC *frequency*, *state double-bookkeeping* across OS services, and the *indirection cost of capabilities that hide kernel objects***.

**For us — a measurement discipline, not a redesign:**
- **IPC frequency** → audit how often **Arthur's agentic tool-call path** (NL → tool-call → cap-check → action) crosses a protection boundary *per action*; coalesce/batch capability-checked calls rather than micro-optimizing a single hop.
- **State double-bookkeeping** → wherever a governance invariant (`INFLIGHT`/`RECENCY`/`ARENA`/reservation/`TABLE`) is mirrored between kernel and a userspace service, that duplication is a *named HM antipattern* — prefer single-owner state.
- **Capability indirection cost** → this is the one that *conflicts* with our design (capability indirection is foundational for us). So it's a **measurement mandate, not a redesign**: instrument the cost before assuming it's free.
- **Effort:** a THINK + instrumentation pass, no kernel rewrite. **Slots into:** `complexity-governance.md` + OT-15 QoS accounting.

## 2. Graded isolation as a deployment knob (IC0/IC1/IC2) — maps onto our kill-switch
**What HM does (verified 3-0):** services run at a chosen isolation class — **IC0** (kernel-adjacent shims), **IC1** (mechanism-enforced *in-kernel* services), **IC2** (address-space-isolated userspace) — and the class is a **deployment-time knob**: "demote everything back to IC2 if new attacks emerge."

**For us:** the *concept* maps almost 1:1 onto our **kill-switch invariant** ("ship a disabled mode proving disabled ≡ prior layer bit-for-bit") — HM's "demote to IC2 under threat" *is* a kill-switch at the isolation-placement level. **But for a monolithic kernel, graded-isolation is largely N/A:** we don't isolate services into address spaces (Bible 4.1), so there's no mechanical lever to turn. Our isolation model IS Rust module discipline + I-14. See `THINK-2026-07-04-isolation-class-taxonomy.md` for the full analysis. **Reject IC1-style in-kernel co-location:** running services in kernel space for speed inflates the TCB and trades away memory-safety — and (see §4) memory-corruption is exactly HarmonyOS's demonstrated weak axis. We are Rust-and-FV-first; we take the kill-switch concept, not the in-kernel shortcut. **Effort:** resolved by the isolation-class taxonomy THINK (verdict: retain as design reference, no build warranted).

## 3. Policy-free kernel paging (adopt) · address-tokens (do not adopt speculatively)
**What HM does (verified 3-0):** mechanism in the kernel, **policy pushed to a userspace service** ("policy-free kernel paging"); **address-tokens** *supplement* capabilities as a perf shortcut for co-managing kernel objects on specific hot paths.
- **Policy-free paging** — clean microkernel orthodoxy that *reduces kernel policy code* (good for our TCB/FV goals). **Adopt cleanly** where our mm split allows.
- **Address-tokens** — a performance shortcut *around* capability indirection, i.e. around the exact indirection we deliberately commit to for security. **Do not adopt speculatively.** Only ever as a *measured* optimization for a proven-hot path, gated behind the kill-switch **and** the adversarial-audit gate — never as a default access-control substrate. (And do not represent HM as having abandoned capabilities — the "replace" claim was refuted.)

## 4. External validation of our differentiation thesis (DarkNavy, verified 3-0, medium-confidence)
Independent adversarial evaluation found HarmonyOS NEXT's microkernel gives it the **best defense among evaluated kernels against post-exploitation lateral movement** (reduced privileged-code footprint) — **while its memory-corruption defense lags** (it even *dropped* PAC+CFI that HarmonyOS 4.2 had). Isolation = demonstrated strength; memory-safety = demonstrated weakness.

**Why this matters for us:** it's third-party confirmation of NeoPlutarch's exact bet — **isolation-boundary integrity (our I-14 fault-containment invariant) buys measurable lateral-movement resistance**, and the residual weakness of the *strongest shipping microkernel OS* is precisely the class **we close by construction** (Rust + the adversarial-audit gate). Two uses: (a) cite as external evidence in our security-model narrative / I-14 justification; (b) the **PAC/CFI regression is a cautionary tale** — never let a perf/isolation refactor silently drop a memory-safety mitigation (mirrors our "`[skip ci]` debt compounds invisibly" lesson). *(Medium confidence: single secondary source, dated to a Developer Preview, kernel now closed.)*

## 5. Formal-verification honesty — calibrate our claims, and place Arthur outside the TCB
**Verified scope:** even the best-marketed FV OS proves only a **tiny high-value core** (scheduling + IPC), **against a spec** (seL4-style), not "every line." (Huawei: first OS to use FV in a device TEE — iTrustee, CC EAL4+ 2021; HongMeng core EAL6+ 2023.)
- **Claim discipline:** NeoPlutarch's FV/verification claims must always read **"property X proven against spec Y for component Z,"** never "the kernel is verified." This matches our byte-oracle / independent-spec rigor — keep it.
- **A genuinely useful architectural framing:** the seL4 model (verify the minimal core; *isolate + audit* everything else) validates our layered approach — and suggests we state explicitly that **Arthur (the AI) sits OUTSIDE the verified/trusted core, mediated only through capability-checked interfaces** (analogous to how HM keeps the rich stack outside the verified TEE core). This is already true of our design (Article 29.3, the cap-check floor) — worth stating as a first-class principle: *the AI is never in the TCB.*

## 6. Honest gaps — what this pass did NOT establish
- **Deterministic scheduling / "latency engine" (priority 2): NO verified claims.** An independent blog mentions a "Deterministic Latency Engine (sets task priorities + time limits in advance)," but nothing survived verification. This maps to **OT-15** (per-principal CPU-time QoS) and deserves a dedicated follow-up pass.
- **On-device AI / Harmony Intents (priority 3): NO verified claims.** Sources mention "Agent Framework 2.0 / Intent as a Service" (HarmonyOS 7), but unverified. **This is the closest analog to Arthur's agentic stack** (NL → intent → permission-checked action), so it's the highest-value follow-up — a focused second research pass specifically on the Harmony *intent framework* permission model.
- **Cross-service capability-delegation primitive:** the distributed permission model was deprioritized (correctly, N=1) but its underlying *delegation primitive* was never surfaced — a possible mine for our single-node capability graph if a future pass wants it.

## 7. Recommendation — what to actually do
Most of the value is **validation + discipline**, not new builds. Concretely:
1. **No urgent build.** HarmonyOS confirms our direction; it doesn't hand us a missing feature.
2. **Two small candidate builds, when a slot opens** (both low-priority, both slot into existing lanes): (a) an **isolation-class taxonomy THINK/ADR** wired to the kill-switch harness (§2); (b) an **Arthur cross-boundary IPC-frequency measurement** pass (§1) → feeds OT-15.
3. **One high-value follow-up research pass:** the **Harmony *intent framework*** (§6, priority 3) — the direct Arthur analog. Worth doing before we design the next agentic layer.
4. **Adopt now, for free:** the FV-claim discipline (§5) + the "**AI is never in the TCB**" framing as a stated principle; cite DarkNavy as external I-14 validation (§4).

*Filed as a research note. The two unresearched axes (scheduling, intents) are the real open threads — the intent framework especially.*
