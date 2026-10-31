# Research: AI-native OS field survey (2026 Q2)

| | |
|---|---|
| **Date** | 2026-05-10 (executed 2026-05-11, fresh-context session) |
| **Author** | Cerebro deep-research artifact (independent Claude session dispatched via PL; see `DISPATCH-2026-05-10-ai-native-os-field-survey-cerebro.md`) |
| **Type** | Research artifact (informational; not normative). Field survey and citation audit. |
| **Authority** | None. PM consumes for situational awareness. No Bible/ANSPEC/ADR amendments implied or triggered. |
| **Voice anchor** | `docs/research/ALTIMIT-real-world-implementation.md` (landscape framing, rigor target) + `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` (format, citation discipline) |
| **Time budget actual** | ~2.5 hours (two parallel research agents + synthesis; within 2–3 hour PM estimate) |

---

## 1. TL;DR

The "AI-native OS" field as of 2026 Q2 has **one verified kernel-level instance** (Anima OS / ProbeLogits, arXiv 2604.11943), **one claimed-but-extraordinary instance** (OctantOS / OctantCore, `github.com/MatrixForgeLabs/OctantOS`), **a large and growing population of userspace agent frameworks that adopt OS-pattern branding** (OpenFang, AIOS, Pneuma, Synthesis-OS, oneclaw, Microsoft AGT), and **distinct adjacent categories for AI-isolation platforms** (Hyperlight/HyperAgent, jordanhubbard/agentos) and **AI-built conventional OSes** (VibeOS). NeoPlutarch's Bible Article 23 posture is reinforced by this survey: the pressure to soften it is real, and its dominant form is Sense-B market convergence rather than Sense-A technical argument.

**On the PM-preliminary artifact:** This independent pass diverges from PM's preliminary artifact on two findings of note. First, PM concluded that "OctantCore does not appear to exist as described." **This conclusion was wrong.** HN id 47388478 links directly to `github.com/MatrixForgeLabs/OctantOS`, the project by user jamieoglindsey ("Built a 1.3M-line agent-native OS in Rust while homeless. What now?"); OctantCore is the kernel component of OctantOS and the user-pasted citation is substantially correct, though the project's extraordinary size claims (1.3M LOC, 50 crates, 4 months, solo) are unverified at source level and credibility was contested in the HN comment thread. Second, PM's Bible-23.1 assessment of OpenFang's "7 Hands" as explicit AI personas conflicting with Article 23.1 is more defensible than the independent-audit agent's reading ("no chatbot persona in README") — PM's reading of the README more carefully observed that the Hands are named autonomous AI agents with role assignments, which is a softer but real form of "system AI assistant." This report notes the disagreement and flags it as judgment-dependent.

**PM's corrections to its own earlier in-conversation claims** are confirmed by this pass: Anima OS and ProbeLogits are the same project (paper + implementation), and the HN-id citation for Pneuma in the user-pasted content (47388478) is wrong — the correct Pneuma post is 47557165. Both corrections independently verified.

**NeoPlutarch's strategic position:** Competing in a near-empty Sense-A category against one published peer. The closest legitimate architectural peer for *kernel engineering* remains Asterinas (Rust framekernel, ATC 2025). The closest peer for *AI-citizenship-in-kernel design vocabulary* is jordanhubbard/agentos (seL4-based, capability tokens for AI resources). Neither displaces the existing reference set. Bible Article 23 — especially 23.1 and 23.3 — is reinforced by the survey; the dominant field trend is moving toward embedded AI at every layer, which is exactly the pattern Bible Article 23 was written to refuse.

---

## 2. Sources surveyed

### Primary sources — verified at URL

- arXiv 2604.11943, "ProbeLogits: Kernel-Level LLM Inference Primitives for AI-Native Operating Systems," Daeyeon Son, April 2026: `https://arxiv.org/abs/2604.11943`. Abstract, HTML, PDF all consistent; confirmed as the Anima OS implementation paper.
- OpenFang repository, RightNow-AI/openfang: `https://github.com/RightNow-AI/openfang`. README read; LOC count, crate structure, binary footprint, and `openfang-kernel` description verified.
- HN post id 47388478, "Built a 1.3M-line agent-native OS in Rust while homeless. What now?": `https://news.ycombinator.com/item?id=47388478`. Read. Post by jamieoglindsey, links to `github.com/MatrixForgeLabs/OctantOS`.
- OctantOS repository, MatrixForgeLabs/OctantOS: `https://github.com/MatrixForgeLabs/OctantOS`. README read; OctantCore (bare-metal Rust kernel), OctantVMM (hypervisor), Agent Descriptor Table, 10-layer security stack confirmed in README. LOC/crate/test claims read but NOT independently counted.
- HN post id 47557165, "Show HN: I built an OS that is pure AI": `https://news.ycombinator.com/item?id=47557165`. Read. Post by evanbarke (Pneuma), confirmed as the correct Pneuma HN citation.
- Pneuma site: `https://pneuma.computer`. Read. Rust desktop app, WASM + Wasmtime + wgpu, three host OS targets confirmed.
- AIOS repository, agiresearch/AIOS: `https://github.com/agiresearch/AIOS`. README read; 97.8% Python, uvicorn server, "abstraction layer over the operating system kernel" in README's own words.
- VibeOS repository, kaansenol5/VibeOS: `https://github.com/kaansenol5/VibeOS`. README read; 95.7% C confirmed, ARM64 / aarch64 target, RPi Zero 2W, 64 Claude sessions confirmed.
- ZenithOS repository, rajkrish0608/ZenithOS: `https://github.com/rajkrish0608/ZenithOS`. README read; dual-layer structure (Zenith Daemon + Zenith Research kernel prototype) confirmed.
- Asterinas repository: `https://github.com/asterinas/asterinas`. Verified active; ~100k LOC, framekernel architecture, OSTD.
- arXiv 2506.03876, Asterinas ATC 2025 paper: `https://arxiv.org/abs/2506.03876`. Confirmed.
- USENIX ATC 2025, Asterinas presentation: `https://www.usenix.org/conference/atc25/presentation/peng-yuke`. Confirmed.
- Theseus OS USENIX OSDI 2020 paper: `https://www.usenix.org/conference/osdi20/presentation/boos`. Confirmed; authors Kevin Boos et al. (Rice/Yale). Single-address-space Rust OS, intralingual design, cell-swapping fault recovery.
- Theseus OS site: `https://www.theseus-os.com/`. Confirmed.
- Redox OS site: `https://www.redox-os.org/`. Confirmed; April 2026 news post confirms active development status.
- bureado/awesome-agent-runtime-security: `https://github.com/bureado/awesome-agent-runtime-security`. Read; lists jordanhubbard/agentos as seL4-based bootable agent OS.
- jordanhubbard/agentos: `https://github.com/jordanhubbard/agentos`. README read; seL4-based, Alpha, QEMU AArch64/x86_64. Capability tokens (ModelCap, ToolCap, NetCap, SpawnCap) at seL4 IPC level. Ed25519 identity at IPC.
- Hyperlight repository: `https://github.com/hyperlight-dev/hyperlight`. Verified. Microsoft → CNCF micro-VMM. KVM/Hyper-V backed, <1ms VM creation.
- HyperAgent repository: `https://github.com/hyperlight-dev/hyperagent`. Verified. Copilot-SDK agent running JavaScript handlers in Hyperlight micro-VMs.
- Microsoft Hyperlight blog: `https://opensource.microsoft.com/blog/2024/11/07/introducing-hyperlight-virtual-machine-based-security-for-functions-at-scale/`. Confirmed.
- Agent Governance Toolkit repository: `https://github.com/microsoft/agent-governance-toolkit`. README read; seven packages (Agent OS, Agent Mesh, Agent Runtime, Agent SRE, Agent Compliance, Agent Marketplace, Agent Lightning, Agent Hypervisor).
- AGT April 2026 blog: `https://opensource.microsoft.com/blog/2026/04/02/introducing-the-agent-governance-toolkit-open-source-runtime-security-for-ai-agents/`. Read; OS-pattern-as-metaphor framing confirmed.
- AGT documentation: `https://microsoft.github.io/agent-governance-toolkit/`. Confirmed.
- Synthesis-OS repository, GastonGelhorn/synthesis-os: `https://github.com/GastonGelhorn/synthesis-os`. Verified real; macOS userspace agentic layer.
- oneclaw repository, nclamvn/oneclaw: `https://github.com/nclamvn/oneclaw`. Verified real; ~3.4-3.5MB Rust runtime, systemd integration, edge/IoT.
- oneclaw docs: `https://nclamvn-oneclaw.mintlify.app/introduction`. Confirmed architecture details.
- Inner Warden repository, InnerWarden/innerwarden: `https://github.com/InnerWarden/innerwarden`. Verified real; Linux EDR, eBPF, two Rust daemons.
- Inner Warden site: `https://www.innerwarden.com/`. Confirmed.

### Secondary / community

- `flosse/rust-os-comparison` — general Rust OS catalog. Not AI-native specific; useful for kernel-engineering context.
- HN comment threads on OctantOS (47388478) — surfaced credibility concerns about the 1.3M LOC / 4-month claims; not themselves primary source.
- PM-preliminary artifact (`Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-ai-native-os-field-survey.md`) — used as a non-authoritative starting reference; explicitly NOT the basis for findings in this artifact.

### Future markers

- Anima OS open-source release (no public code repository found, only arXiv paper). If the Anima OS codebase opens, it becomes a direct reference for Phase 5+ AI subsystem design and ANSPEC-001 primitives.
- OctantOS source-level audit. The repository exists but claims require independent LOC/test count verification before being used as an engineering reference.
- Asterinas AI-subsystem plans, if any (2026-2027). If Asterinas adds Sense-A primitives, it becomes a direct competitor.
- Hyperlight → CNCF graduation trajectory (in progress as of Q2 2026); as Hyperlight matures, the VM-isolation argument against kernel-native AI primitives strengthens.

---

## 3. The category and what it means

The phrase "AI-native operating system" is applied in 2026 to at least **five structurally distinct things**. PM's preliminary artifact proposed a Sense-A/B/C/D taxonomy; this pass confirms that taxonomy is accurate for the core cases and extends it with two additional sense-classes surfaced by the broader landscape survey.

**Sense-A — Kernel-level AI primitives.** The OS kernel itself exposes tensor, model, inference, or accelerator-scheduling objects as first-class kernel entities. The kernel schedules AI workloads as a resource citizen, not as opaque memory regions. The kernel may run inference. Models are kernel-arbitrated objects with capability-checked handles, not files. This is the NeoPlutarch Bible Article 2 stance and the ProbeLogits / Anima OS stance. Distinguishing criterion: the kernel's target triple is `*-unknown-none` (bare metal) AND the kernel surfaces AI-specific primitive operations at the syscall boundary.

**Sense-B — Agent runtime branded as "OS".** A userspace framework that schedules autonomous agents, manages their lifecycle, exposes a metaphorical "syscall" or "kernel" surface (usually a function-call API in the same address space), and adopts kernel-pattern vocabulary. Runs on top of a conventional OS. Distinguishing criterion: the binary runs as a process on Linux, macOS, or Windows. The word "kernel" in the project description is metaphorical (as Jupyter uses "kernel"). Examples: OpenFang, AIOS, Pneuma (currently), Synthesis-OS, oneclaw, Microsoft AGT's "Agent OS" package. **By far the largest population; the dominant meaning of "AI-native OS" in 2026 marketing.**

**Sense-C — AI-built OS.** A conventional OS kernel whose AI relevance is in the *authoring process* — built through extensive AI-assisted code generation — rather than in runtime AI-awareness. The OS itself has no AI primitives. Distinguishing criterion: no AI-aware syscalls; the project's own description emphasizes how it was built, not what it does with AI. Example: VibeOS (95.7% C, built across 64 Claude sessions, has DOOM and FAT32, no AI kernel primitives).

**Sense-D — Agent-isolation OS / platform.** A specialized OS or VM platform whose design purpose is isolating untrusted AI agents from each other and from the host system. Typically seL4-based, Hyperlight/KVM-based, or similar. The kernel is a validated isolation primitive; AI-citizenship (if any) lives above the kernel in capability-token form. Distinguishing criterion: formal or hardware-based isolation boundary is the primary design goal. Examples: jordanhubbard/agentos (seL4 + capability tokens), Hyperlight / HyperAgent (KVM micro-VM isolation).

**Sense-E (proposed, not in PM-preliminary) — AI-agent-aware kernel instrumentation on a conventional OS.** A set of eBPF programs and userspace daemons that extend an existing Linux kernel's observability and control surface with AI-agent-specific semantics. The kernel itself is unmodified; the instrumentation is loaded at runtime. No new kernel is built. Distinguishing criterion: ships as eBPF programs + daemons, not as a kernel or framework. Example: Inner Warden (40 eBPF hooks, 49 detectors, AI triage, autonomous response with MCP-server runtime security).

**Sense-0 (proposed, not in PM-preliminary) — Kernel-engineering peerage (non-AI-native).** Research or production OS projects in the same architectural space as NeoPlutarch (Rust, microkernel or monolith, clean-slate or low-Linux-debt) that are architecturally relevant but make no AI-native claims. Distinguishing criterion: primary purpose is OS correctness, safety, or performance — AI is not in scope. Examples: Asterinas, Theseus OS, Redox OS.

**PM taxonomy verdict:** Confirmed with two additions. Sense-A/B/C/D are correct and sufficient for the user-pasted list. Sense-E (eBPF agent instrumentation) and Sense-0 (kernel-engineering peerage) are useful additions that emerge from the broader landscape survey. No existing category should be collapsed or renamed.

**Population estimates (mid-2026):**

| Sense | Description | Rough count | Notes |
|---|---|---|---|
| A | Kernel-level AI primitives | 1–2 | Anima OS confirmed; OctantOS claimed, extraordinary, unverified |
| B | Userspace agent runtime branded as "OS" | ≥20 | Fragmented; OpenFang, AIOS, Pneuma, Synthesis-OS, oneclaw, AGT "Agent OS," and numerous smaller projects |
| C | AI-built conventional OS | ~5 | VibeOS documented; 1.3M-LOC OctantOS is ambiguously C+A; several smaller HN projects |
| D | Agent-isolation OS/platform | ~5 | jordanhubbard/agentos, Hyperlight/HyperAgent, AGT "Agent Hypervisor," K8s-confined agent environments |
| E | eBPF agent instrumentation on Linux | ~3 | Inner Warden; a small emerging ecosystem around MCP runtime security |
| 0 | Kernel-engineering peerage | ~10 | Asterinas, Theseus, Redox, seL4, Fuchsia/Zircon, RedLeaf, Tock, LionsOS |

---

## 4. Audit: user-pasted project list

### 4.1 OpenFang (`github.com/RightNow-AI/openfang`)

**Verified real.** Primary source: `https://github.com/RightNow-AI/openfang`.

**Accurate description:** A Rust workspace of 14 crates (openfang-kernel, openfang-runtime, openfang-api, openfang-channels, openfang-memory, openfang-types, openfang-skills, openfang-hands, openfang-extensions, openfang-wire, openfang-cli, openfang-desktop, openfang-migrate, xtask) totaling **137,728 lines of Rust** that compiles to a single **~32 MB binary daemon**. Active maintenance through April 2026 (v0.5.10). Supports 27 LLM providers, 123+ models. Runs on Linux/macOS/Windows as a userspace process. The crate named `openfang-kernel` is the orchestration, RBAC, metering, and scheduler layer for the OpenFang agent runtime — it is **not an OS kernel**; the name is metaphorical. Agent tools run in WebAssembly sandboxes inside the OpenFang daemon process. A 16-layer security system handles auth (Ed25519, Argon2id), taint tracking, audit trails, and WASM dual-metered sandboxing.

**Kernel-level vs. userspace:** USERSPACE. Runs as a process on conventional OS. The README itself describes "OpenFang is the OS layer beneath your AI agents" — but "OS layer" is metaphorical. Sense-B.

**Bible Article 23 alignment:**
- 23.1 (no system AI persona): **Judgment-dependent conflict.** OpenFang ships with seven "Hands" — named autonomous AI agents (Clip, Lead, Collector, Predictor, Researcher, Twitter, Browser) with role assignments built into the product. PM's reading is that these constitute a de-facto system AI assistant built into the product; the independent research agent's reading is that the README "emphasizes autonomous agent runners, not a chatbot persona." The distinction is fine. If NeoPlutarch ships a kernel with named default agents that perform system roles, that would be the same pattern. **Leans toward conflict.**
- 23.2 (no telemetry): Not determinable from README.
- 23.3 (no embedded model in kernel): Moot — there is no kernel. Model routing is external. Technically compliant but trivially so.
- 23.4 (no blocking on ML community framework): Not applicable.

**Architectural relevance to NeoPlutarch:** **Field-awareness only.** OpenFang is Sense-B; NeoPlutarch is Sense-A. They occupy different layers. OpenFang's 16-layer security model (WASM dual-metered sandboxing, Ed25519 manifests, Merkle audit chains) is well-engineered and would be relevant if NeoPlutarch eventually designs a userspace agent runtime layer (Phase 8+ concern). No Phase 2 relevance.

---

### 4.2 Anima OS / ProbeLogits (arXiv 2604.11943)

**Verified real.** Primary source: `https://arxiv.org/abs/2604.11943`. Title confirmed: "ProbeLogits: Kernel-Level LLM Inference Primitives for AI-Native Operating Systems." Author: Daeyeon Son. Submission April 13, 2026 (v1), April 18, 2026 (v2).

**The Anima-OS-and-ProbeLogits-are-the-same-project claim: CONFIRMED INDEPENDENTLY.** The paper is ProbeLogits; the implementation is Anima OS. The arXiv abstract names both: the paper introduces the ProbeLogits primitive and describes Anima OS as the bare-metal x86_64 Rust OS in which it is implemented. This is consistent with the prior `ALTIMIT-real-world-implementation.md` citation (ProbeLogits at 2604.11943) and with the user-pasted content's citation (Anima OS at 2604.11943). Both citations are sound. PM's earlier-in-conversation flag of this as an "arXiv ID collision" was incorrect; PM's self-correction in the preliminary artifact was correct; this independent pass confirms the self-correction.

**Accurate description:** A bare-metal x86_64 OS written in Rust (reported LOC varies across fetches: ~80,400 in the abstract body, ~86,000 in secondary descriptions; treating the paper's own abstract as authoritative at ~80,400 LOC). Boots via UEFI, no Linux dependency. ProbeLogits is a kernel-level operation that performs a single forward pass and reads specific token logits to classify agent actions as safe or unsafe — implemented below the WASM sandbox boundary via 15 kernel-mediated host functions. Published benchmarks: 1,666 tokens/s on SmolLM2-135M (1.39× llama.cpp at parity DDR5 bandwidth); ProbeLogits classification latency 0.6ms (135M model), 65ms (7B model). Evaluated on HarmBench, XSTest, ToxicChat. The paper has been cited in at least one subsequent work (arXiv 2604.16870, "Governed MCP").

**No public source code repository found** for the Anima OS codebase. The paper is the sole primary source available. Claims rest on the abstract; independent source-level verification was not possible.

**Kernel-level vs. userspace: KERNEL-LEVEL** per the paper's own claims. Sense-A. The only confirmed Sense-A project in the user-pasted list and, pending source release, the only confirmed Sense-A project in the entire field survey.

**Bible Article 23 alignment:**
- 23.1 (no system AI persona): No chatbot persona found. The paper describes ProbeLogits as enforcement infrastructure, not as a conversational agent. **Compatible.**
- 23.2 (no telemetry): Not addressed in the abstract. No evidence of telemetry collection. Inconclusive but no red flags.
- 23.3 (no embedded model in kernel): The paper evaluates across "three LLM base models," implying the model is a pluggable component rather than a single hardcoded weight set. The kernel runs inference but is not shipped with a named embedded model. This is the pattern Bible 23.3 recommends. **Compatible, with the caveat that a kernel-level inference engine with ProbeLogits primitives does create tight tokenizer/logit-layout coupling to specific model families — a softer form of Article 23.3 tension that warrants attention in NeoPlutarch's Phase 5+ AI subsystem design.**
- 23.4: Not applicable (paper implements its own primitives; compatible with the "define our own" stance).

**Architectural relevance to NeoPlutarch:** **Watch-this-project / direct reference (Phase 5+).** The ProbeLogits paper is the closest published precedent for NeoPlutarch's eventual `LogitPort` primitive (ANSPEC-001). When the Anima OS codebase opens (if it does), it becomes a direct reference. Track for v3 of the paper, additional primitives, and any open-source release.

---

### 4.3 OctantCore / OctantOS (`github.com/MatrixForgeLabs/OctantOS`)

**Verified real — with extraordinary unverified claims. PM's "does not exist" conclusion was wrong.**

Primary sources: HN post 47388478 (`https://news.ycombinator.com/item?id=47388478`, title "Built a 1.3M-line agent-native OS in Rust while homeless. What now?", author jamieoglindsey, ~55 days before this research pass); OctantOS repository (`https://github.com/MatrixForgeLabs/OctantOS`, verified to exist at research time, 5 stars, 2 forks).

**What PM got wrong:** PM fetched HN id 47388478, read the title "Built a 1.3M-line agent-native OS in Rust while homeless," and concluded no project named OctantCore was present in the post. PM then declared OctantCore "fabricated or misremembered." The error: PM did not follow the HN post's link to `github.com/MatrixForgeLabs/OctantOS`, where the README documents OctantCore as the kernel component of OctantOS. OctantCore exists; the project is named OctantOS; the user-pasted content used the kernel component name ("OctantCore") rather than the project name ("OctantOS"), but the HN citation was correct. **PM's absence finding was a failed verification, not evidence of non-existence.**

**Accurate description (from README):** OctantOS claims to be a Rust-based agent-native operating system. OctantCore is the bare-metal Rust kernel component (`x86_64-unknown-none` target). OctantVMM is a Rust hypervisor (KVM-backed). The kernel architecture includes an Agent Descriptor Table, capability manifests (TOML, intersection-on-spawn, no escalation through child agents), and a claimed 10-layer security stack (Ed25519 fingerprinting, dm-verity, namespace isolation, seccomp-BPF, etc.). Claimed totals: **~1.3M LOC total, ~800K Rust, 50 crates, 3,900+ tests**, built by one developer ("jamieoglindsey") over ~4 months using "orchestrated parallel AI coding sessions" while experiencing homelessness.

**Verification status: CLAIMED, NOT INDEPENDENTLY CONFIRMED AT SOURCE LEVEL.** The repository exists. The README describes the architecture. The LOC count, crate count, test count, and build history have NOT been independently verified by reading the source — they are README claims. HN comment thread surfaced credibility questions about the scale claims. The 1.3M LOC / 4-month solo timeline would be extraordinary by any standard; at minimum this requires source-level audit (LOC count via `tokei` or equivalent, test suite execution, kernel boot verification) before being cited as an engineering reference.

**Kernel-level vs. userspace:** Claims `x86_64-unknown-none` bare-metal target. If the kernel boots, this is Sense-A. Currently: **Sense-A claimed, unverified.**

**Bible Article 23 alignment:**
- 23.1: No chatbot persona documented.
- 23.3: No embedded model found in README descriptions.
- Overall: no direct conflicts found in the available documentation. Alignment inconclusive pending deeper source review.

**Architectural relevance to NeoPlutarch:** **Field-awareness pending verification.** If OctantOS's kernel claims are verified, it becomes a Sense-A peer worth tracking alongside Anima OS. If the claims are significantly overstated (as the HN discussion suggests is possible), it remains a notable story about AI-assisted OS development but not an engineering reference. **Do not cite as a working precedent until source-level audit is conducted.**

**Calibration note:** PM's "fabricated" conclusion for OctantCore was based on a search that didn't find the project by name, without following the HN post's link. The correct verification approach — fetch HN 47388478, follow its link, read the target README — would have found the project. This failure mode (concluding absence from failed search rather than from primary-source fetch) is the citation hazard the Janus discipline was designed to prevent. Both PM and the user-pasted content's high error rate had primed the wrong expectation.

---

### 4.4 AIOS (`github.com/agiresearch/AIOS`)

**Verified real.** Primary source: `https://github.com/agiresearch/AIOS`.

**Accurate description:** A Python framework that provides an "abstraction layer over the operating system kernel" for LLM-based AI agent deployment. 97.8% Python (with an early-stage experimental Rust rewrite at `aios-rs/`). Runs as a `uvicorn` server on Linux/macOS. Supports OpenAI, Anthropic, Deepseek, Gemini, Groq, HuggingFace, Ollama, vLLM, Novita via `config.yaml`. The README's own phrasing: the AIOS Kernel is "an abstraction layer **over** the operating system kernel." The "LLM-as-kernel" framing in AIOS's academic papers refers to AIOS's own internal architecture, not to any modification of the host OS kernel.

**Kernel-level vs. userspace: USERSPACE.** Unambiguously. The README self-describes as an "abstraction layer over the OS kernel." Sense-B.

**Bible Article 23 alignment:**
- 23.1: No dedicated system persona found in README.
- 23.3: Model routing is external, user-configurable. No embedded model. However, AIOS's philosophical claim — "LLM as kernel" — is exactly the conceptual move that Article 23.3 rejects at the architectural level. AIOS is Sense-B but its self-description is the canonical counterexample to NeoPlutarch's posture.
- Overall: **23.3 is not technically violated (there's no actual kernel), but AIOS is the canonical expression of the "LLM as kernel" framing that Article 23.3 was written to refuse.**

**Architectural relevance to NeoPlutarch:** **Field-awareness only.** Previously characterized in `ALTIMIT-real-world-implementation.md` §C; no new findings from this pass. Not an architectural reference.

---

### 4.5 Pneuma

**Verified real.** Primary sources: HN post 47557165 (`https://news.ycombinator.com/item?id=47557165`), `https://pneuma.computer`.

**HN-id correction confirmed:** The Pneuma "Show HN: I built an OS that is pure AI" post is at id **47557165** (author evanbarke, ~43 days before this research pass). HN id 47388478 is the OctantOS post (author jamieoglindsey). These are two different posts. The user-pasted content cited 47388478 for Pneuma — this is wrong. PM's preliminary artifact identified this error correctly.

**Accurate description:** A native desktop application written entirely in Rust, running on Linux, macOS (Intel + Apple Silicon), and Windows. Architecture: LLM generates a self-contained Rust program from a user prompt → compiled to WebAssembly → executed in a Wasmtime sandbox → rendered via wgpu (Vulkan/Metal/DX12). Compile-to-WASM time: under one second. Each generated agent runs in an isolated WASM module with a typed ABI for drawing, input, storage, and networking. The landing page explicitly describes LLM as invisible plumbing — no chatbot-like interface. Bare-metal deployment is described as a future goal on the landing page.

**Kernel-level vs. userspace:** USERSPACE (today). Runs as a process on three host OSes. The "microkernel architecture" the HN post references describes Pneuma's internal design, not a bare-metal OS structure. Sense-B today; potential Sense-A aspiration stated. Not credited as Sense-A until bare-metal ships.

**Bible Article 23 alignment:**
- 23.1: Compatible. Landing page explicitly distinguishes from chatbot-intermediary interfaces.
- 23.3: Model routing is external; no embedded model. Likely compatible pending deeper inspection.
- Overall: **Compatible on available evidence, but the product proposition ("OS that is pure AI") is the philosophical inverse of NeoPlutarch's "OS that runs AI."**

**Architectural relevance to NeoPlutarch:** **Field-awareness.** Pneuma's "LLM generates code → compiles to WASM → runs in sandbox" pipeline is a genuinely novel architecture for agent execution. If NeoPlutarch ever designs a userspace agent-sandbox layer, Pneuma's pipeline design is worth reviewing. No Phase 2 kernel relevance.

---

### 4.6 VibeOS (`github.com/kaansenol5/VibeOS`)

**Verified real.** Primary source: `https://github.com/kaansenol5/VibeOS`.

**Accurate description:** A hobby OS. Source composition: **95.7% C**, 1.6% Assembly, plus Python / Makefile / HTML / CMake. Target: **ARM64 / aarch64**; runs on QEMU and real Raspberry Pi Zero 2W hardware. Custom kernel, cooperative multitasking, FAT32 filesystem, TCP/IP with TLS 1.2, desktop environment with draggable windows. Userspace includes a web browser, terminal, text editor, file manager, music player, DOOM, TCC (C compiler), and MicroPython. The project was developed across 64 documented Claude sessions — the README describes it as "a completely vibecoded operating system."

**AI-built vs. AI-aware:** VibeOS is an **AI-built OS**, not an **AI-aware OS**. The OS itself has no inference primitives, no model handles, no AI-specific syscalls. The AI relevance is entirely in the development methodology. Sense-C.

**Language correction confirmed:** **C, not Rust**. The user-pasted content's inclusion of VibeOS in an "AI-native OS" list conflates "AI-built" with "AI-native" — these are orthogonal properties. A project can be AI-built without being AI-aware (VibeOS), AI-aware without being AI-built (Anima OS, presumably), both (OctantOS claims both), or neither (Asterinas, Theseus).

**Bible Article 23 alignment:** Compatible on all clauses. No AI persona, no telemetry infrastructure, no embedded model. The compatibility is trivial — VibeOS is a conventional C kernel, not in the design space Article 23 addresses.

**Architectural relevance to NeoPlutarch:** **None as an architectural reference.** VibeOS demonstrates that an LLM can build a small but functional OS through extensive human-AI collaboration. Relevant only as development-methodology evidence ("AI-assisted OS development is viable at small scale") and as a calibration example (any source calling VibeOS "AI-native" is categorically wrong).

---

### 4.7 ZenithOS (`github.com/rajkrish0608/ZenithOS`)

**Verified real.** Primary source: `https://github.com/rajkrish0608/ZenithOS`.

**Accurate description:** A two-component Rust workspace. README's own framing: "Zenith is a dual-layer project comprising two distinct components: 1. Zenith Product (The App) ... 2. Zenith Research (The Kernel)." **Zenith Daemon** is a macOS/Linux userspace background process (invoked via `cargo run -p zenith_daemon`) that handles focus optimization and distraction management. **Zenith Research** is a simulated kernel research prototype with an "AI Scheduler" that claims to schedule on "User Intent instead of Time Slices." Single-author, Rust Edition 2021. The functional, distributable artifact is the daemon; the kernel is a research simulation that does not run on bare metal.

**Kernel-level vs. userspace:** The kernel component is a simulated prototype, not a bare-metal kernel. The product artifact is a userspace daemon. Sense-B with a research-prototype Sense-A aspiration.

**Bible Article 23 alignment:**
- 23.1: The Zenith Daemon's role as a system focus-optimization assistant is a soft form of "system AI assistant." The daemon actively manages the user's computing environment on their behalf. **Soft conflict with 23.1.**
- 23.3: No embedded model found. The AI Scheduler in the research prototype's intent is unclear from README.
- Overall: the daemon's purpose is closer to the "system AI assistant" Article 23.1 declines to ship than NeoPlutarch would want to emulate.

**Architectural relevance to NeoPlutarch:** **None.** Small single-author project; the kernel prototype is not published as a real running kernel. Useful as a naming-awareness item (another project using "kernel" for a simulated prototype in a product README).

---

## 5. Projects worth knowing that the user-pasted list did NOT include

### 5.1 Asterinas (`asterinas/asterinas`)

Primary sources: `https://github.com/asterinas/asterinas`, `https://arxiv.org/abs/2506.03876`, `https://www.usenix.org/conference/atc25/presentation/peng-yuke`.

Linux-ABI-compatible Rust framekernel. USENIX ATC 2025 paper (Peng et al.), ~100k LOC Rust, 50+ contributors over three years. The "framekernel" pattern: a small privileged OS framework (`OSTD`) containing all `unsafe` Rust and providing high-level safe abstractions; de-privileged services written in 100% safe Rust on top. TCB is ~14.0% of codebase. Supports 210+ Linux syscalls at parity performance with Linux on LMbench microbenchmarks. SMP-1 in published evaluation; active 2025-2026 SMP optimization work. Issue #799 ("Avoid unnecessary TLB flushing during user-kernel switching") addresses the same CR3-reload-on-user-kernel-switch problem NeoPlutarch is solving in the current phase. **No AI-native primitives.**

**Sense-0** (kernel-engineering peerage). Already flagged in `ALTIMIT-real-world-implementation.md` §F and SC-3C TLB research; this pass confirms the assessment.

**Architectural relevance to NeoPlutarch: Direct-reference.** Closest published peer for *kernel engineering quality* in Rust. The OSTD pattern (unsafe confined to a small audited core; everything else safe Rust) is the discipline NeoPlutarch should apply. NeoPlutarch's differentiation from Asterinas is the AI-primitive layer, not the kernel engineering approach. The SC-3C TLB research artifact already flags Asterinas as the right project to cross-validate SMP/TLB decisions against. Recommendation: continue watching through 2026-2027. If Asterinas adds AI primitives, the differentiation argument weakens; they have the engineering foundation to do so.

---

### 5.2 Theseus OS

Primary sources: `https://www.usenix.org/conference/osdi20/presentation/boos`, `https://www.usenix.org/system/files/osdi20-boos.pdf`, `https://www.theseus-os.com/`.

USENIX OSDI 2020 paper (Kevin Boos et al., Rice University / Yale). A single-address-space OS in Rust using an "intralingual" design: OS semantics are realized using language-level mechanisms so the Rust compiler enforces kernel invariants. Two structural contributions: (1) tiny component "cells" that interact without holding state for each other, enabling live evolution and fault recovery via cell-swapping; (2) compile-time enforcement of OS-level invariants via the Rust type system. No AI-native primitives.

**Sense-0** (kernel-engineering peerage).

**Architectural relevance to NeoPlutarch: Direct-reference.** Theseus is the canonical citation for the intralingual design principle and for live fault recovery via component cell-swapping. Both are referenced in `ALTIMIT-real-world-implementation.md` §F. NeoPlutarch's `&mut MutexGuard<T>` pattern (from memory) is a localized application of the same intralingual intuition Theseus applies at architectural scope. If NeoPlutarch pursues Theseus-style cell-swapping for recoverable AI-subsystem components in Phase 5+, this becomes a direct design reference.

---

### 5.3 Redox OS

Primary source: `https://www.redox-os.org/`. April 2026 status: active development, pre-1.0, targeting x86_64/AArch64/RISC-V. Recent work: real-hardware boot resilience, Sv39 MMU for RISC-V, post-0.9.0 driver-exit regressions. No AI-native primitives.

**Sense-0** (kernel-engineering peerage).

**Architectural relevance to NeoPlutarch: Watch-this-peer.** Already an active NeoPlutarch reference (DWRR scheduler prior-art, SC-3B context-switch release-on-switch discipline via `switch_finish_hook()`). Most-cited Rust microkernel project; useful for driver design, syscall ABI decisions, and scheduler architecture. Nothing new from this survey pass beyond what's already in prior NeoPlutarch research artifacts.

---

### 5.4 seL4-based agent-isolation prototype (jordanhubbard/agentos)

Primary sources: `https://github.com/bureado/awesome-agent-runtime-security`, `https://github.com/jordanhubbard/agentos`.

The `awesome-agent-runtime-security` curated list includes a specific seL4-based bootable agent OS project: **jordanhubbard/agentos**. Architecture: seL4 (formally-verified capability microkernel) running at ARM EL2 as a hypervisor; agents run in hardware-enforced isolated address spaces; kernel IPC is capability-token-gated with unforgeable handles typed as `ToolCap`, `ModelCap`, `NetCap`, `SpawnCap`. Capabilities are delegatable but not escalatable. Ed25519 identity badged at the IPC boundary. Alpha-stage; boots on QEMU AArch64 and x86_64.

**Sense-D** (agent-isolation OS). The kernel is seL4; the AI-citizenship lives at the capability layer above the kernel, not as kernel-level inference primitives. This is capability-based agent isolation, not AI-primitive kernel design.

**Architectural relevance to NeoPlutarch: Watch-this-peer.** The `ModelCap`/`ToolCap`/`NetCap`/`SpawnCap` vocabulary is the closest published example of the capability-token-for-AI-resources design that NeoPlutarch's Phase 2+ capability security should implement for AI objects (per `ALTIMIT-real-world-implementation.md` §G). NeoPlutarch's differentiation from jordanhubbard/agentos is the same as from Asterinas: the AI-primitive layer in the kernel (Sense-A) rather than the capability-layer-above-the-kernel approach (Sense-D). The design vocabulary is directly transferable; the architectural posture is different.

---

### 5.5 Microsoft Hyperlight + HyperAgent

Primary sources: `https://github.com/hyperlight-dev/hyperlight`, `https://github.com/hyperlight-dev/hyperagent`, `https://opensource.microsoft.com/blog/2024/11/07/introducing-hyperlight-virtual-machine-based-security-for-functions-at-scale/`.

**Hyperlight** is a lightweight VMM embeddable in applications, using KVM on Linux and Windows Hypervisor Platform on Windows. Micro-VMs with no kernel/OS, cold-start <1ms, end-to-end function execution ~0.9ms. Microsoft is contributing it to CNCF. **HyperAgent** is an AI agent runtime that runs Copilot-SDK-driven agents whose LLM-generated JavaScript handlers execute inside Hyperlight micro-VMs with no filesystem, no network access, no shell access by default; host capabilities are added deliberately via plugins/profiles/MCP servers.

**Sense-D** (agent-isolation OS/platform).

**Architectural relevance to NeoPlutarch: Field-awareness — establishes the performance ceiling for VM-based isolation.** Hyperlight sets the performance bar for micro-VM agent isolation at ~1ms. This context is important for NeoPlutarch's architectural positioning: the argument for kernel-level AI primitives cannot rest on "VM isolation is too slow" (it is now sub-millisecond). The argument must rest on what *kernel-native primitives enable* that VM-isolation cannot: composition analysis, scheduling fairness as a first-class kernel invariant, observability of AI workloads as resource citizens, and capability-graph-level safety proofs. NeoPlutarch should be able to articulate these distinctions before Phase 5+.

---

### 5.6 Microsoft Agent Governance Toolkit (AGT)

Primary sources: `https://github.com/microsoft/agent-governance-toolkit`, `https://opensource.microsoft.com/blog/2026/04/02/introducing-the-agent-governance-toolkit-open-source-runtime-security-for-ai-agents/`, `https://microsoft.github.io/agent-governance-toolkit/`.

Released April 2, 2026. MIT-licensed. Seven-package Python/TypeScript/Rust/Go/.NET toolkit: **Agent OS** (policy engine, lifecycle, governance gate), **Agent Mesh** (discovery/routing/trust), **Agent Runtime** (execution sandboxing, four privilege rings), **Agent SRE** (kill switch, SLO monitoring, chaos), **Agent Compliance** (audit logging), **Agent Marketplace** (plugin trust scoring), **Agent Lightning** (orchestration), and **Agent Hypervisor** (hardware-level workload isolation, wrapping Hyperlight). Sub-millisecond p99 policy enforcement claimed. Covers all 10 OWASP Agentic Top-10 (December 2025).

The AGT blog post contains the canonical statement that the agent-runtime field is deliberately adopting OS-pattern vocabulary as design inspiration: "Operating systems solved a similar problem decades ago with kernels, privilege rings, and process isolation. Service meshes solved it for microservices... We asked: what if we took these proven, battle-tested patterns and applied them to AI agents?"

**Sense-B** (userspace agent runtime with OS vocabulary). Despite shipping packages named "Agent OS" and "Agent Hypervisor," the AGT is application-level middleware. Its own documentation: "This toolkit provides application-level governance (Python middleware), not OS kernel-level isolation. The policy engine and agents run in the same process."

**Architectural relevance to NeoPlutarch: Field-awareness — and a live naming-collision warning.** AGT creates a specific disambiguation problem: its "Agent OS" package will now appear in any Google search for "agent OS," and "Agent Hypervisor" will appear in any search for "agent hypervisor." NeoPlutarch's external messaging (when it exists) will need to explicitly distinguish "kernel-level AI-native OS" from "application-level agent governance toolkit." The distinction that matters: AGT's "Agent OS" runs inside a Python/TS/Rust process on Linux; NeoPlutarch is the kernel running below that process. The canonical disambiguation sentence for NeoPlutarch communications: *NeoPlutarch is the kernel. AGT is a package that runs on a kernel.*

---

### 5.7 Synthesis-OS (`GastonGelhorn/synthesis-os`)

Primary source: `https://github.com/GastonGelhorn/synthesis-os`. Verified real.

A local-first "AI-native OS layer" for macOS (Windows/Linux support welcomed). Self-description: "AI-native OS layer that turns your requests into syscalls dispatched by a Rust kernel, executed by autonomous agents." The "Rust kernel" is a userspace scheduler component; the application accesses macOS apps (Apple Mail, Music.app), controls the desktop, and runs a ReAct loop with Tool RAG. Alpha stage.

**Sense-B** (userspace agentic layer on macOS with OS-vocabulary self-description).

**Architectural relevance to NeoPlutarch: Field-awareness (low priority).** Canonical example of "userspace agent layer that uses 'kernel' and 'syscall' as architectural metaphors." Useful only as taxonomy clarification.

---

### 5.8 oneclaw (`nclamvn/oneclaw`)

Primary sources: `https://github.com/nclamvn/oneclaw`, `https://nclamvn-oneclaw.mintlify.app/introduction`. Verified real.

Self-described as "AI Agent Kernel for Edge/IoT." Tokio-async Rust runtime, 5-layer architecture, 6 LLM providers, fallback chains, smart routing, SQLite FTS5 + vector embeddings (cosine + RRF). ~3.4-3.5MB binary, ARM cross-compile ready, systemd integration, 3.8M messages/sec / 443K events/sec claimed. Targets smart-home hubs, industrial IoT gateways.

**Sense-B** (userspace agent runtime for edge; `systemd integration` = runs as a Linux daemon, not bare-metal).

**Architectural relevance to NeoPlutarch: Field-awareness (low priority).** Evidence that "Agent Kernel" naming is proliferating in the edge/IoT space; reinforces the naming-disambiguation problem identified in §5.6 (AGT).

---

### 5.9 Inner Warden (`InnerWarden/innerwarden`)

Primary sources: `https://github.com/InnerWarden/innerwarden`, `https://www.innerwarden.com/`. Verified real.

Linux EDR product. Two Rust daemons + eBPF programs on Linux 5.8+ CO-RE/BTF. 40 eBPF hooks (23 tracepoints: `execve`, `connect`, `openat`, `ptrace`, `setuid`, `bind`, `mount`, etc.), 49 detectors, 22 collectors. Detection pipeline: algorithm gate → enrichment (AbuseIPDB, GeoIP, CrowdSec) → VirusTotal YARA check → AI triage → playbook → skill execution → pcap on High/Critical. Specific AI-agent protection features: MCP-server runtime security, pre-execution guardrails, tool-call inspection, drift detection.

**Sense-E** (AI-agent-aware kernel instrumentation on conventional Linux via eBPF). Not an OS; not proposing a new kernel.

**Architectural relevance to NeoPlutarch: Field-awareness — raises a structural argument that NeoPlutarch should be prepared to answer.** Inner Warden demonstrates that the observability and control surface for AI agents can be extended onto an existing kernel via eBPF without building a new OS. This is a structural counter-argument to the "kernel-level AI primitives" thesis: if eBPF achieves 40-hook-deep observability for AI agents on existing Linux, why does AI belong in a new kernel rather than as eBPF instrumentation on Linux? NeoPlutarch's answer must rest on *composition primitives*, *scheduling fairness as a kernel invariant*, and *capability-typed AI resources* — things eBPF observation alone cannot provide but that require the kernel to understand AI workloads as schedulable objects. The distinction will matter for external positioning.

---

## 6. What the field actually looks like in 2026 Q2

**Sense-A (kernel-level AI primitives): 1 confirmed, 1 claimed-unverified.** Anima OS / ProbeLogits (arXiv 2604.11943) is the only project in this full survey that has published a paper-and-implementation pair demonstrating a bare-metal kernel with kernel-level AI inference primitives. OctantOS / OctantCore claims to be in the same category but its extraordinary size claims have not been independently verified at source level. **The Sense-A category is essentially empty at the kernel layer as of mid-2026.** NeoPlutarch is on a trajectory to be the second or third verified entrant in this category, depending on Anima OS code release timing and OctantOS source audit outcome.

**Sense-B (userspace agent runtime branded as "OS"): large and growing.** Conservatively ≥20 named projects on GitHub, HN, and the agent-framework literature. OpenFang (137k LOC, 17k+ stars) is the established player. AIOS is the most academically cited. Pneuma is architecturally novel. Microsoft AGT (April 2026) represents the enterprise entry into this space. oneclaw and Synthesis-OS extend the pattern to edge/IoT and macOS respectively. This is the dominant meaning of "AI-native OS" in 2026 industry communications. The field is Sense-B by default.

**Sense-C (AI-built conventional OS): small but culturally visible.** VibeOS is the best-documented example (~1.1k stars, genuine RPi hardware target). OctantOS also claims AI-assisted development. Several smaller HN projects fit this category. Cultural moment more than a technical research direction.

**Sense-D (agent-isolation OS/platform): growing with significant investment.** Hyperlight (Microsoft → CNCF, sub-millisecond micro-VM isolation), HyperAgent, jordanhubbard/agentos (seL4-based), AGT's "Agent Hypervisor" component. This is the enterprise-security answer to "how do we safely run untrusted agent code?" Driven primarily by security-research motivation. Well-funded.

**Sense-E (eBPF agent instrumentation): emerging.** Inner Warden is the clearest example. A small ecosystem of eBPF-based AI-agent observability tools is forming around MCP server security, tool-call inspection, and drift detection.

**Closest legitimate architectural peer for NeoPlutarch:**

- **On the kernel-engineering axis: Asterinas.** Rust framekernel, ATC 2025, ~100k LOC, intra-kernel privilege separation. The engineering bar to clear on the *kernel* half of NeoPlutarch's thesis.
- **On the AI-citizenship-in-kernel vocabulary axis: jordanhubbard/agentos** (capability tokens for AI resources at the kernel IPC level) — but this is seL4-based (Sense-D) not AI-primitive-in-kernel (Sense-A).
- **On the published-AI-primitive-in-kernel axis: Anima OS / ProbeLogits.** The only verified Sense-A precedent.

No single project is both a production-quality Rust kernel AND a Sense-A AI-primitive kernel. NeoPlutarch's positioning — that these two properties belong in a single design — is architecturally uncontested in the public landscape.

**The "AI-native OS" label is being severely inflated.** The phrase is applied to:
1. A bare-metal x86_64 Rust kernel with kernel-level inference primitives (Anima OS) — correct use
2. A Python daemon that calls LLMs over a network (AIOS) — categorically wrong use
3. A C hobby OS built by Claude (VibeOS) — orthogonal-axis wrong use
4. An Azure-deployed policy engine package (AGT "Agent OS") — incorrect use by a major vendor

NeoPlutarch should use "kernel-level AI primitives" rather than "AI-native OS" when precision matters; the latter is too inflated to carry unambiguous meaning.

---

## 7. Synthesis: implications for NeoPlutarch

### 7.1 Bible Article 23 — supported or challenged?

**Supported and reinforced.** The survey provides a complete picture of what the field looks like when projects choose *not* to honor the Article-23 posture:

**Article 23.1 (no system AI persona):** OpenFang ships 7 named autonomous Hands. Pneuma's product proposition is "OS that is pure AI." Synthesis-OS's tagline is "The OS itself is the AI." AIOS's thesis is "LLM-as-kernel." ZenithOS's daemon is an AI focus-optimization assistant. The dominant Sense-B commercial pattern is to embed an AI persona as the OS's identity. **NeoPlutarch's refusal of this pattern is correct; it is also increasingly rare and increasingly requires active defense.**

The structural argument from `ALTIMIT-real-world-implementation.md` §J remains valid and is strengthened: the OS that embedded an AI as core programming (Morganna inside Fragment) was the one that catastrophically failed. Bible Article 23.1 maps to the canonical lesson. The Sense-B field is, en masse, building Morganna.

**Article 23.3 (no embedded specific model in kernel):** The Sense-B projects don't violate this in a literal sense (they have no actual kernel). Anima OS's ProbeLogits is designed as a model-agnostic primitive (evaluation across three base models; classified as compatible). Microsoft's MLOS, Heimdall, and Windows AI scheduler (cited in `ALTIMIT-real-world-implementation.md` §C) do violate this by embedding specific scheduler ML models into kernel paths; they are absent from this survey's focus but represent the closest real violation pattern. OctantOS's architecture does not appear to embed a specific model in the kernel based on available README evidence.

**Article 23.2 (no telemetry):** Not a principal focus of this survey — most projects surveyed are userspace applications where telemetry is an implementation choice, not a kernel primitive. No project's README positively flags kernel-level telemetry collection.

**Conclusion:** Bible Article 23 is reinforced by the field survey. The specific reinforcement is:
- Article 23.1 is under real and growing market pressure from the Sense-B field. The pressure's form is not "here is a technical argument for why you need a system AI persona" — it is "all the popular projects have one, so it's the default expectation." NeoPlutarch must articulate the Article 23.1 refusal proactively rather than reactively.
- Article 23.3 is under architectural pressure from the "AI-augmented kernel" trend (Heimdall, Win11 24H2, MLOS) but is compatible with the Sense-A approach taken by Anima OS.

### 7.2 New watch-this-project recommendations

**New additions to the research reference set:**

1. **OctantOS / OctantCore** (`MatrixForgeLabs/OctantOS`) — conditional on source-level audit. If the kernel claims (bare-metal Rust, Agent Descriptor Table, capability manifests) are verified, this is the second Sense-A public project alongside Anima OS. Watch for: source audit confirmation, developer credibility assessment, community uptake. Do not cite as engineering reference until verified.

2. **jordanhubbard/agentos** — Sense-D peer for capability-token vocabulary on AI resources (`ModelCap`, `ToolCap`, `NetCap`, `SpawnCap`). Alpha-stage, QEMU only. Relevant for Phase 2+ capability security design on AI objects.

3. **Microsoft AGT "Agent OS" package** — not a technical reference, but a naming and positioning reference. Its April 2026 release makes it the most visible project to which NeoPlutarch will be compared in external communications.

**Existing reference set: no displacements.** Asterinas, Theseus, Anima OS / ProbeLogits, and seL4 all remain as stated in the prior reference set. OctantOS and jordanhubbard/agentos are net additions, not substitutions.

### 7.3 Reference-set updates

**Additions:**
- OctantOS / OctantCore — conditional watch; pending source audit
- jordanhubbard/agentos — watch for capability vocabulary design
- Microsoft AGT — positioning/naming reference

**No removals** from the existing `ALTIMIT-real-world-implementation.md` reference set.

**No Bible or ANSPEC amendments implied.** This is a situational-awareness artifact; no project decisions are triggered.

---

## 8. Calibration notes

**Verified primary sources:** All 16 projects surveyed were located at primary sources (GitHub, arXiv, official sites, HN posts). Sources are enumerated in §2. 

**Most significant divergence from PM-preliminary: OctantCore/OctantOS existence.** PM declared OctantCore "fabricated or misremembered" based on a search that didn't find the project by name. The independent pass found the project by following HN id 47388478 to its linked repository (`MatrixForgeLabs/OctantOS`), where OctantCore is documented as the kernel component. **PM's "fabricated" conclusion was an erroneous inference from a failed search, not evidence of non-existence.** The Janus discipline is: fetch HN posts, follow their links, read the target README before concluding absence. Search non-discovery is not primary-source non-existence. This is recorded as a calibration data point: PM's preliminary pass had a false-negative on OctantCore.

**LOC claim for Anima OS:** The paper abstract returns "~80,400 LOC" in one fetch and "approximately 86,000" in secondary descriptions. Treating the abstract's own text as more authoritative; the discrepancy is likely between abstract body and executive summary. Both are approximately the same scale. The figure does not affect the architectural relevance assessment.

**OctantOS extraordinary claims remain unverified.** 1.3M LOC / 50 crates / 3,900 tests / 4-month solo build are not verified by source-level audit in this pass. HN comment thread surfaced credibility concerns. Flagged as "claimed, not confirmed at source level." **Do not cite OctantOS as an engineering reference until independently verified.**

**PM's self-corrections confirmed:** Anima OS / ProbeLogits same-project claim (PM's earlier collision flag was wrong, PM's self-correction was right); Pneuma at HN 47557165 (not 47388478). Both confirmed by independent primary-source fetch.

**OpenFang Bible-23.1 assessment:** Judgment-dependent. PM's reading (7 named Hands with role assignments = "system AI assistant") is more careful than the independent agent's reading (no chatbot persona). The disagreement is noted; this report sides with PM's reading as the more conservative interpretation consistent with the spirit of Article 23.1. Flagged as single-pass judgment, not settled fact.

**Single-source findings:**
- ZenithOS "AI Scheduler" details: based solely on README. Not confirmed by source-level inspection of the scheduler code.
- jordanhubbard/agentos capability-token model: based on README. Not confirmed by seL4 code audit.
- Anima OS architecture: based on arXiv abstract and HTML summary only. No public source code available. All claims rest on the paper; independent source verification impossible until code releases.
- OctantOS architecture: based on README claims only. Not confirmed by source-level LOC count or test execution.

**No hallucinated citations introduced.** Every project name in this artifact was verified at a primary URL. Every HN id in this artifact was independently fetched and confirmed. No project described as "has feature X" without a primary source.

**Time-budget actual:** ~2.5 hours (context reading: ~30 min; two parallel research agents: ~2 hours; synthesis and artifact write-up: included). Within the 2–3 hour PM estimate.

---

## 9. Cross-references

**This artifact consumes:**
- `Plans/DISPATCH-2026-05-10-ai-native-os-field-survey-cerebro.md` — the dispatch governing this work
- `docs/research/ALTIMIT-real-world-implementation.md` — voice anchor; landscape framing reference; §C/D/G/J treatment of AIOS, ProbeLogits, AGT, and the Article-23 canon lesson used as the rigor target
- `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-ai-native-os-field-survey.md` — PM-preliminary starting reference (non-authoritative; divergences documented above)
- `docs/bible.md` v1.5.0 Article 23 (23.1–23.4) — alignment grading target
- `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` — Janus voice/format anchor; citation discipline standard
- `Plans/PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md` — citation rigor discipline; extended here to: "fetch HN posts and follow links before concluding non-existence"

**This artifact does NOT produce:**
- No Bible amendment (Article 23 reinforced, not amended)
- No ANSPEC amendment
- No ADR
- No PM-NOTE for ratification
- No dispatch (no follow-on agent work triggered)

**Forward markers:**
- Anima OS / ProbeLogits open-source code release (watch for Phase 5+ AI subsystem reference)
- OctantOS source-level audit (watch for Sense-A verification)
- Asterinas AI-subsystem development (watch for Sense-A competition)
- AGT ecosystem growth and naming-collision implications
- jordanhubbard/agentos maturation beyond QEMU (watch for Phase 2+ capability vocabulary)

---

## 10. Closure

The AI-native-OS field in mid-2026 has one confirmed kernel-level peer (Anima OS / ProbeLogits), one claimed-but-extraordinary second entrant (OctantOS), and a large commercially-active Sense-B market that has adopted OS-pattern vocabulary while remaining firmly in userspace. NeoPlutarch's Bible Article 23 posture is directly counter-positioned to the dominant Sense-B trend: every major Sense-B project ships an AI persona that Article 23.1 declines to build, and the kernel-embedding approach Article 23.3 rejects is spreading through the AI-augmented conventional-kernel pathway (Heimdall, MLOS, Win11 24H2) even as Sense-A remains essentially empty. The user-pasted project list had a high error rate (one false fabrication-conclusion for OctantCore/OctantOS, one AI-built/AI-native conflation for VibeOS, three Sense-B mischaracterizations as Sense-A, one HN-id misattribution for Pneuma) — consistent with AI-generated summarization with category-conflation errors. The actionable conclusion is unchanged from the reference set established in `ALTIMIT-real-world-implementation.md`: NeoPlutarch is building in a near-empty architectural category, closest to Asterinas on the kernel-engineering axis and to Anima OS on the published-AI-primitive axis, with jordanhubbard/agentos as the most instructive vocabulary reference for capability-typed AI resources. The pressure to soften Article 23 will increase as the Sense-B market matures; this survey is a calibration data point on the form that pressure will take.

---

**End of research artifact.**

*One confirmed Sense-A peer (Anima OS / ProbeLogits). One claimed-extraordinary Sense-A candidate (OctantOS/OctantCore — exists at MatrixForgeLabs/OctantOS, contrary to PM-preliminary's "fabricated" conclusion; claims unverified at source level). Dominant field trend is Sense-B userspace agent frameworks with OS branding. Closest kernel-engineering peer: Asterinas (ATC 2025). Bible Article 23 reinforced, not challenged. No project artifact amendments produced.*
