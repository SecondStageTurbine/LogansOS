<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# The Strategic Honesty Ledger

> **A claim remains in the lowest category that honestly describes the evidence, regardless of how strongly we believe it.**

This document is the *strategic* falsifiability ledger — the peer of `docs/complexity-governance.md` (the
*technical* invariant ledger). Where that one keeps the engineering honest, this one keeps the **thesis**
honest: it separates what is **proven**, what is **structural** (true by construction, not measured), what
is **measurable but open**, what is a **strategic bet**, and what would **kill the thesis**. It exists
because of a specific late-stage failure mode: early projects die from lack of progress; mature ones die
because **every milestone gets read as proof of everything.** The ledger institutionalizes the discipline
that has so far been applied by hand — *governance ≠ adversarial isolation; demonstrated ≠ enforced;
structural claim ≠ measured advantage.*

**Origin:** PL red-team session, 2026-06-17 (the necessity + adoption probes). The five-section structure
and the kill-conditions section were specified by the PL as red team.

## Rules of the ledger (how the top sentence is operationalized)

1. **Every strategic claim carries a category and its evidence.** A claim with weaker evidence than its
   category demands is a **defect**, the same standing as a missing kill-switch or a stale invariant.
2. **Promotion requires the named evidence, not belief.** A claim moves up a category (Bet → Measurable →
   Structural → Proven) only when the specific evidence that category requires exists and is cited. Strength
   of conviction is never evidence.
3. **Demotion is automatic on contradiction.** If a measurement contradicts a claim, the claim drops — and
   if the project's marketing/docs still assert it, that is a defect to fix, not a debate to have.
4. **Re-audited every phase close.** The PM re-walks this ledger at each phase close (the Rule-4
   consolidation cadence). An un-updated ledger is a *failing* ledger (see KC-5).
5. **The categories are a ratchet against mythology, not a roadmap.** Being in §3 (Measurable, Open) is not
   a weakness to hide; it is the honest state of most superiority claims today, and saying so is the point.

---

## 1. PROVEN — no longer hypotheses (evidence required)

Standard: built, merged to `main`, gated green, and — where the claim is about hardware — observed on real
silicon. *"Demonstrated"* here means *the code path was exercised and the result observed*, which is a notch
**below** *"survived a live adversarial campaign"* (we have not run one — see the caveat).

| # | Proven claim | Evidence |
|---|--------------|----------|
| P1 | **Kernel-mediated, capability-bounded authority over every kernel object incl. tensors/models** — holding the handle is permission; no ambient authority. Revocation cascades transitively across processes. | Phase 2 capability system; Phase 5 AI syscalls (`syscall.rs` handle table); revocation cascade + "revoke wins" (ADR-043) code-verified in `Plans/THINK-2026-06-17-i9-i13-adversarial-verification.md` Probe B. |
| P2 | **Kernel-arbitrated accelerator access** — admission, per-tenant accounting, residency-aware placement, eviction under pressure — works between mutually-untrusting tenants. | Phase 7 **CLOSED 2026-06-11** (`Plans/CLOSE-REPORT-2026-06-11-phase-7.md`). |
| P3 | **Hardware-enforced device-memory isolation on real silicon.** A fenced device that DMAs outside its arena faults in the IOMMU. All four I-14 vectors silicon-proven. | BUILD-7.5-V VT-d (`7fe15bb`): on a real Comet Lake OptiPlex 7080, a fenced NVMe DMA below its arena faulted (`faults=1`). Software arena BUILD-0 (`eef664a`): in-arena admitted, out-of-arena refused, 0 escapes. |
| P4 | **Governance survives a from-scratch, un-firmware'd accelerator** — the hardest case for "can the kernel govern this." Completion gated on a self-planted seqno fence, never a device pointer. | Intel Gen9 PPGTT/execlist bring-up (BUILD-4X); seqno-fence completion verified in the adversarial log Probe E. |
| P5 | **Forward-pass compute on real quantized model weights** — GGUF import → Q4_K dequant → f32 → matmul on actual Qwen2.5-0.5B weights; RoPE + SwiGLU wired into the layer scaffold. | 8.2 (`8324b72`), 8.3a (`b44f0f9`), 8.3-integration (`7f1c718`), 8.4 (`c6712a9`). |
| P6 | **In-kernel autoregressive decode loop + sampler**, QEMU-verified. | 8.1 (`18d609e`), `ai_generate_decode` QEMU PASS at NEO_SMP=4. |
| P7 | **WiFi to real silicon (passive scan)** — 10 real BSSIDs off-air on a physical MT7601U, fence clean. | BUILD-W1 (`faa30ef`), ADR-054. |
| P8 | **First real-model token generation, governed in-kernel** — token-in → 24 GQA-attention layers + governed KV cache → logits → sample → next token, on Qwen2.5-0.5B-Q4_K_M; the generated **top-1 matches `llama.cpp`** (input "Hello" tok 9707 → top-1 271), and cached decode == uncached recompute (real assertion, not round-trip). | 8.5 (`0bfb044`), ADR-055. **Caveat:** host KATs + the top-1 oracle green per worker report + PM code-grounded review of the assertions; the pre-merge QEMU gate is **PM-independently confirmed green** (CI run 234 = `27659206737`, `workflow_dispatch` on `0b4a981`, conclusion success — full CI incl. the 4-shard QEMU on the exact code; GitHub-MCP recovered after bug #45208). Post-merge `main` CI (run 235, `0bfb044`) is the same tree and was queued at merge time — a formality to confirm, not a re-verification. |

**Explicitly NOT yet proven (do not let a nearby P-row imply these):** *multi-token sequence* correctness
vs a reference beyond the verified top-1 + the cached==uncached equivalence (P8 proves first-token-top-1 and
cache equivalence, **not** that a long generation matches `llama.cpp` token-for-token); inference *serving*
(no serving surface); **Arthur** (the assistant does not exist — the userland arc is ratified design, no one
has run it); any *comparative* property vs another stack (no benchmark — see §3, now *unblocked* by P8 but
still unrun); adversarial *isolation* under a live red team (the probes were code-grounded verification +
construction arguments, **not** an exploit campaign — see
the caveat below).

**Caveat on P1/P4 (the demonstrated-≠-enforced line).** The 2026-06-17 adversarial session verified five
attack classes *by reading the live code and by construction*, and reduced them to two small auditable cores
(`Plans/THINK-2026-06-17-i9-i13-adversarial-verification.md`). That is strong evidence of **legibility and
by-construction soundness** — it is **not** the same as "a funded red team tried to break it and failed."
Until the latter exists, the security claims live here as *demonstrated-by-construction*, and the
adversarial-isolation superlative stays out of §1.

---

## 2. STRUCTURAL CLAIMS — believed true by construction, not benchmarked

These are the claims Linux can challenge *architecturally* but not by adding a feature. They are not
measured advantages; they are *qualitative capabilities the layered stack cannot offer at any performance*.

- **S1 — The enforcement-boundary fusion is not replicable purely in userspace.** Capability policy +
  kernel-owned device memory makes placement/authority decisions *enforceable at the hardware boundary*; the
  CUDA/ROCm/Metal model hands the device to userspace and so can decide but not enforce. (Recorded
  pre-build: `Plans/THINK-2026-06-07-phase-7-inference-scheduler-governance.md` §12, Q4.)
- **S2 — Capability ownership of device memory is qualitatively different from CUDA context ownership.** MPS
  is *cooperative* isolation; MIG is *coarse* hardware partition; neither is per-tensor capability
  arbitration with kernel-owned VRAM.
- **S3 — Kernel authority exists at boundaries userspace cannot police** — the seqno fence (don't trust the
  device's completion pointer), the fenced DMA arena/IOMMU (fence the device from outside), the admission
  gate (nothing runs that the kernel didn't admit). All sit *outside* the device and need zero firmware
  cooperation.
- **S4 — The AI governance surface reduces to a small, auditable TCB** — five adversarial probes bottom out
  on two small cores (the claim-or-skip refcount gate; the kernel-owned-producer + seqno fence). A
  disaggregated Linux+K8s+CUDA+Triton stack provides the equivalent properties, where it provides them at
  all, across many separate subsystems with a far larger combined TCB.
- **S5 — The primitive is *tensor + capability + residency*; a *model* is a thin aggregate, not a
  first-class object.** The accurate statement of the AI-native premise is: *the kernel governs AI compute
  **and the memory it touches** as capability-secured resources.* ADR-001 / Article 2's "models are kernel
  objects" wording is **legacy framing** — Phase-7 (O1/O2; ADR-042/043/044) already made it false in code:
  `Model::weights → Vec<SlotKey>` is a named bundle over governed tensors, and the load-bearing primitives
  are the tensor (governed memory), the capability (authority), and residency. *Thought-experiment check
  (red-team A2, 2026-06-18):* delete "kernel-owned models" but keep kernel-owned authority + device memory +
  capability enforcement, and the governance thesis survives intact — confirming the primitive is the
  triple, not the model. (This is a framing clarification recorded here per the PM recommendation; the
  Bible/ADR-001 normative text is intentionally left unchanged — the code embodies S5, only the words lag.
  Source: `Plans/THINK-2026-06-18-arthur-destination-red-team.md` §5.)

**Hard caveat on this whole section.** *Structural ≠ adversarially-tested ≠ measured.* "True by
construction" means *the code path forecloses the failure*, not *an attacker failed to find another path*,
and not *it is faster/cheaper*. S1–S4 are the claims most likely to be quietly upgraded into measured or
proven superlatives; per the top sentence, they may not be, absent the evidence §1/§3 demand. The honest
necessity argument is **narrow**: the necessity clears only the enforcement core; tensor *policy* could live
in a userspace server (the seL4 stance), and the in-kernel scheduler is *monolithic by conviction*, layered
on top of the necessity, **not demanded by it** (same THINK, Q4 — "the phase must not over-claim 'all of
this must be in ring 0.'").

---

## 3. MEASURABLE CLAIMS (OPEN) — every claim data can kill

None of these is proven. Each is a hypothesis with a measurement harness but no head-to-head result. **No
benchmark vs Linux+vLLM/Triton/MPS exists today** (verified 2026-06-17). Claiming any of these as achieved
is a defect.

| Claim | Status | Measurement required |
|-------|--------|----------------------|
| Lower p99 token latency vs Linux+vLLM | **Unproven** | the 8.5 benchmark (same model, same silicon) |
| Higher GPU utilization | **Unproven** | head-to-head vs vLLM continuous batching |
| Better fairness under contention | **Unproven** | multi-tenant noisy-neighbor contention test |
| Better noisy-neighbor resistance | **Unproven** | adversarial co-tenant latency-degradation test |
| Smaller serving footprint | **Unproven** | end-to-end resident set vs Linux+K8s+Triton |
| Governance overhead is bounded/affordable | **Unproven (harness exists)** | backstop-only vs daemon-on delta on a real model (latency oracle, ADR-052) |
| Quantized matmul speeds up inference (q4_K×q8_K integer-dot vs dequant→f32→matmul) | **Unproven** | Phase 9.3: per-layer matmul latency + weight-bytes-read, ADR-052 oracle, Ryzen AVX2. *(Pre-registered via ratified D6, 2026-06-17. Note: the ~7.05× **weight-traffic reduction** is arithmetic/structural — the basis — NOT this row; this row is the realized **speedup**, which only measurement proves.)* |

**Gating fact, stated plainly:** these were **blocked on 8.5**; as of `0bfb044` (P8) **the block is lifted —
a real model now generates, so the benchmark is *runnable*.** But the block lifting promotes *nothing* here:
every row above stays **Unproven** until the head-to-head is actually run and the numbers exist. *The
milestone that unblocks the measurement is not the measurement.* Anyone reading "the model generates" as
"the model serves faster than vLLM" has committed exactly the category error this ledger exists to prevent.
Next concrete step: define and run the §3 benchmark (same model, same silicon, LogansOS vs Linux+vLLM) — a
real post-8.5 deliverable. The latency oracle (ADR-052) is the harness, not a result.

---

## 4. STRATEGIC BETS — the adoption bets (not engineering bets)

The technical thesis is becoming demonstrable; the **adoption** thesis is the real existential risk. The
constitution has chosen the *harder* form of it: Article 3 says LogansOS is **not** an appliance and **not**
a research demo — *"a usable system."* So the ecosystem trap (BeOS/OS-2/Haiku/Plan 9) is **accepted, not
escaped.** Each bet below: what it is, what falsifies it, and the leading indicator to watch.

- **Bet A — Models + tools replace apps.** The "app ecosystem" of an AI-native OS is *models and tools*
  (consumable from the industry — the 8.2 GGUF importer is this bet), not GUI applications we must
  originate. *If false:* general-purpose adoption becomes dramatically harder (back to the BeOS app
  problem). *Watch:* model-ecosystem growth, MCP/tool-standard adoption, agent-workflow share, fraction of
  LogansOS value delivered via Arthur vs via GUI apps.
- **Bet B — Personality layers are a bridge (Rosetta/WSL), not an identity (OS-2).** Foreign-OS compat
  (Article 5.6/41) lets foreign software run during bootstrap while the native ecosystem grows. *If false* —
  if compat becomes the identity — *LogansOS becomes a Linux-compatibility project and dies the OS-2 death*
  ("why run LogansOS to run Linux apps?"). This is the single most dangerous execution risk in the project.
  *Watch:* native-ABI software growth; % of real workloads on the native ABI vs through personality layers.
- **Bet C — AI-assisted development changes OS maintenance economics.** Haiku/BeOS died substantially of
  *insufficient engineer-hours* against a moving hardware target. A small team + AI maintaining a
  deliberately narrow hardware surface is a different cost structure. *If false:* driver + maintenance
  burden becomes fatal (the Haiku death). *Watch:* LOC maintained per contributor; time-to-driver-support;
  sustained velocity across phases.
- **Bet D — A customer segment values the integrated sovereign whole enough to switch OSes.** The switch is
  justified only by needing the *whole* (capability substrate + AI-native ABI + on-device assistant + small
  auditable TCB + no telemetry) as one certifiable property — not by any single feature. *If false:* no
  demand, and technical merit is irrelevant. *Watch:* an identifiable buyer (regulated / defense / edge /
  privacy-critical) that pilots it. **This is the master bet; D failing is terminal in a way A/B/C are
  not.**

---

## 5. KILL CONDITIONS — the section almost nobody writes

Explicit, falsifiable conditions under which a thesis is **dead** and must be dropped or the project
re-scoped. Writing these down is the strongest possible defense against milestone-mythology: a thesis you
cannot state the death of is a thesis you will defend past its death.

- **KC-1 — The serving-advantage thesis (perf).** *If* the 8.5+ benchmark shows worse latency **and** worse
  throughput **and** worse utilization than Linux+vLLM, **and** no target customer finds the
  security/sovereignty benefits compelling enough to pay the gap — *then* the "AI-native **serving**
  advantage" thesis is dead. Drop the perf claim. The project survives **only** on the security/sovereignty
  axis, and only if KC-3 has not also fired.
- **KC-2 — The native-ABI strategy (the OS-2 trap).** *If* >90% of real workloads run through personality
  (foreign-ABI) layers **5 years after the first usable release** — *then* the native-ABI strategy has
  failed and LogansOS is a Linux-compat distribution with extra steps. (N = 5 years from first usable
  release; revise N only with explicit justification, never silently.)
- **KC-3 — The adoption thesis (no buyer). [Master kill condition.]** *If* no identifiable customer segment
  values sovereignty / the integrated whole enough to switch OSes — *then* the adoption thesis fails
  **regardless of technical merit.** KC-1 and KC-2 are *survivable* if KC-3 has not fired (a buyer who needs
  the whole tolerates a perf gap and a thin native ecosystem for a while); **KC-3 firing is terminal.**
- **KC-4 — The idea-donor outcome (Plan 9).** *If* an incumbent (most likely Linux) ships capability-bounded
  accelerator governance with comparable guarantees **and** LogansOS has no live differentiated deployment —
  *then* LogansOS has become Plan 9: a successful idea-donor with no ecosystem. Honorable, not the goal.
  Recognize it explicitly and either pivot to the donor role or fold; do **not** continue as if building a
  product.
- **KC-5 — The discipline itself fails (the silent, most dangerous one).** *If* any claim ever migrates **up**
  a category without the evidence that category requires — a structural claim cited as "measured," a bet
  cited as "proven" — **and this ledger does not catch it at the next phase close** — *then* the honesty
  discipline has failed, which is worse than any single false claim because it is silent and compounding.
  **An un-updated ledger is a fired KC-5.** This is the one kill condition the ledger declares against
  *itself*.

---

## Maintenance

- **Owner:** the PM seat, re-audited at **every phase close** (the Rule-4 consolidation cadence).
- **Promotion/demotion:** per the Rules above — evidence moves a claim, conviction never does.
- **Companion docs:** `docs/complexity-governance.md` (the technical invariant ledger);
  `Plans/THINK-2026-06-17-i9-i13-adversarial-verification.md` (the by-construction security evidence
  feeding P1/P4/S3/S4); `Plans/CLOSE-REPORT-2026-06-11-phase-7.md` (P2).
- **The standing test:** if the top sentence survives five years of development, the project will remain
  intellectually honest even if its conclusions change. That is rarer, and more valuable, than a new
  operating system.
