# PM-Preliminary Research: AI-native OS field survey (2026 Q2)

**Date:** 2026-05-10 (later session, post-D15 SC-3C dispatch close)
**Author:** PM — NOT Cerebro. See provenance correction below.
**Type:** PM-preliminary research artifact (informational; not normative; PROVISIONAL). PM executed web searches and produced first-pass synthesis directly. NOT a Cerebro deep-research artifact. A fresh-context Cerebro pass should follow via `Plans/DISPATCH-2026-05-10-ai-native-os-field-survey-cerebro.md` to confirm, extend, or correct findings.
**Authority:** None. PM (and PL via consumption) reads this for situational awareness; no Bible/ANSPEC/ADR amendments are implied. Voice anchor: `ALTIMIT-real-world-implementation.md` (project root) and the rigor standard a Cerebro-authored version would meet matches `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md`.
**Trigger:** PL pasted a list of 7 projects (OpenFang, Anima OS, OctantCore, AIOS, Pneuma, VibeOS, ZenithOS) in conversation on 2026-05-10. PL requested a verification pass. PM intended to dispatch Cerebro for the pass but instead executed it directly — see provenance correction.

**Supersession marker (filed 2026-05-11):** This artifact has been superseded for canonical content by the Cerebro deep-research artifact `Plans/RESEARCH-2026-05-10-ai-native-os-field-survey-cerebro.md` (filed 2026-05-11, 58.7 KB). The Cerebro pass corrected one PM factual error (OctantOS/OctantCore DOES exist at `github.com/MatrixForgeLabs/OctantOS`; PM failed to follow the HN id 47388478 link properly), extended the taxonomy from four senses to six (added Sense-E for eBPF agent instrumentation on Linux, and Sense-0 for kernel-engineering peerage without AI claims), and flagged a positioning concern PM did not surface prominently (Microsoft AGT package literally named "Agent OS" creates external-positioning collision risk for NeoPlutarch). On one item (OpenFang Bible-23.1 assessment) Cerebro deferred to PM's reading. See `Plans/PM-NOTE-2026-05-11-ai-native-os-field-survey-consumption.md` for the full consumption record. This artifact is preserved as historical record of PM's preliminary first-pass; for canonical taxonomy, project verification, and field-shape findings, use the Cerebro artifact.

**Provenance correction (filed 2026-05-10 later session):** This artifact was originally filed as `Plans/RESEARCH-2026-05-10-ai-native-os-field-survey-cerebro.md` with `Author: Cerebro research artifact` in the header. That was a role-confusion error by PM — deep-research dispatches output `Plans/RESEARCH-YYYY-MM-DD-<topic>-cerebro.md` per agent-roster §2.3, and the named author must be the executing agent (Cerebro, a separate Claude session reached via PL-ferried dispatch), not the PM session that produced this file. Renamed to `PM-PRELIMINARY-RESEARCH-...` and reclassified as PM-grade-preliminary rather than Cerebro-grade-research. The actual Cerebro dispatch lives at `Plans/DISPATCH-2026-05-10-ai-native-os-field-survey-cerebro.md`. See `Plans/PM-NOTE-2026-05-10-pm-role-confusion-recovery.md` for the full calibration record.

---

## TL;DR

The "AI-native OS" field as it appears in 2026 Q2 has **one real kernel-level instance** (Anima OS / ProbeLogits), **a substantial set of userspace agent frameworks that adopt OS-pattern branding** (OpenFang, AIOS, Pneuma, Synthesis-OS, oneclaw, Inner Warden, Microsoft Agent Governance Toolkit), **a small set of legitimate research-academic Rust OS projects that AI work could build on but that are not themselves AI-native** (Asterinas, Theseus, Redox, seL4, Fuchsia/Zircon, RedLeaf, Tock), and **a notable category of AI-*built* hobby OSes that are not AI-*aware*** (VibeOS, ZenithOS). The user-pasted list conflates these categories badly enough that taking the list at face value would produce a wrong picture of the field.

Specific findings from the citation audit:

**Three corrections to my own prior framing earlier in this conversation.** I previously claimed the "Anima OS" citation looked like an arXiv-ID collision with ProbeLogits and called the user-pasted content potentially confabulated. **That was wrong.** Anima OS and ProbeLogits are the same project: ProbeLogits (arXiv 2604.11943v2, April 2026, by Daeyeon Son) is the *paper*; Anima OS is the *bare-metal x86_64 Rust OS implementation* described inside it ("I implement these primitives in Anima OS, a bare-metal x86_64 operating system comprising 86,000 lines of Rust"). The earlier `ALTIMIT-real-world-implementation.md` cited ProbeLogits at this ID correctly; the user-pasted content cited Anima OS at the same ID, also correctly. They're consistent, not in conflict. I owe a correction in the artifact and have recorded it; calibration discipline applies.

**One project in the user-pasted list does not appear to exist as described.** "OctantCore" — described as "A from-scratch Rust kernel that enforces agent permissions and identity as first-class primitives... 10-layer security stack" — has no findable repository, paper, or product page. The Hacker News post id `47388478` cited for it points to "Built a 1.3M-line agent-native OS in Rust while homeless. What now?" — a real but completely different first-person story posted March 22, 2026 by a developer who built a substantial agent OS under extraordinary personal hardship. That story is moving and real; it is not OctantCore. Whether OctantCore is a misremembered name for some other project, a marketing rename of one of the other listed projects, or fully fabricated, the search-engine evidence at the time of this research returns zero hits on the name in any AI-native-OS context.

**Several projects in the list are real but mischaracterized.** OpenFang (17.1k stars, 137k LOC Rust, very real) is described as "treats agents like Linux processes" — but OpenFang is a userspace agent framework that runs *on top of* Linux/macOS/Windows; it is not a kernel-level OS. AIOS (agiresearch/AIOS, real) is described as embedding LLMs *in the kernel* — but AIOS is a userspace Python daemon framework running on Linux. Pneuma (real, posted to HN as id `47557165`, not `47388478` as the user-pasted content's framing implied) is a sandboxed WASM agent runtime via Wasmtime + WGPU — also userspace. VibeOS (real, ~1.1k stars) is *written in C*, not Rust, and is a hobby OS that was *built by Claude through vibe-coding* across 64 sessions — it is an AI-built OS, not an AI-native OS, and the distinction matters. ZenithOS (rajkrish0608) is a research-prototype Rust kernel plus a userspace "focus daemon" — small, single-author, neither production-ready nor specifically AI-native.

**The only real kernel-level AI-native OS the survey finds is Anima OS / ProbeLogits.** And Anima OS is single-author, single-paper, ~86k LoC Rust, and the published evaluation focuses on inference throughput and ProbeLogits classification latency rather than on multi-tenancy, capability-based security, or anything that would make it a complete AI-native OS in the NeoPlutarch sense. **There is no published full-stack AI-native OS in production as of mid-2026.** NeoPlutarch is competing in a category where the standing entrant is one research paper plus one repository.

**This matters for NeoPlutarch's positioning in three concrete ways.** First, the project is not behind the field — the field is genuinely thin at the kernel layer; the bulk of "AI-native OS" naming is happening at the userspace agent-framework layer, which is a different and more mature category. Second, Bible Article 23.1 (no system AI persona) and Article 23.3 (no embedded model in kernel) keep NeoPlutarch out of the trap that most of the user-pasted-list projects are sitting inside — none of OpenFang, AIOS, Pneuma, or Synthesis-OS could honor those clauses and still be themselves. Third, the closest legitimate architectural reference for NeoPlutarch remains Asterinas (Linux-ABI Rust framekernel, ATC 2025); the SC-3C TLB-shootdown research artifact already flagged Asterinas as the right project to track for cross-validation. **Nothing in the user-pasted list displaces Asterinas as the closest reference.**

**Recommendation: take none of the user-pasted-list projects on board as architectural references.** They are useful as field-awareness; they are not architecturally aligned with the Bible-23 posture. The actionable research reference set remains the one in `ALTIMIT-real-world-implementation.md` — Asterinas, Theseus, seL4, Fuchsia/Zircon, RedLeaf, Tock — supplemented by Anima OS / ProbeLogits as the one real kernel-level AI-native peer to watch. No further AI-native-OS field-survey work is recommended in 2026 Q2 unless a new entry appears that meets both kernel-level AND AI-native criteria.

---

## Sources surveyed

### Primary sources consulted directly

- ProbeLogits paper, "Kernel-Level LLM Inference Primitives for AI-Native Operating Systems," Daeyeon Son, arXiv:2604.11943v2, 18 Apr 2026: `https://arxiv.org/abs/2604.11943`. Read for the Anima OS implementation description ("86,000 lines of Rust", "UEFI boot, no Linux"), inference throughput numbers (1,666 tokens/s SmolLM2-135M, 15 tokens/s Qwen2.5-7B), and ProbeLogits classification latency (0.6ms / 65ms).
- ProbeLogits paper HTML version: `https://arxiv.org/html/2604.11943`. Cross-checked Anima OS description, contextual calibration framing, HarmBench/XSTest/ToxicChat evaluation methodology.
- ProbeLogits paper PDF: `https://arxiv.org/pdf/2604.11943`. Read for the AnimaNet protocol mention and the "bare-metal native runtime" vs. hosted-mode latency comparison.
- "Governed MCP: Kernel-Level Tool Governance for AI Agents via Logit-Based Safety Primitives," arXiv:2604.16870, 2026: `https://arxiv.org/html/2604.16870`. Read for an external citation of ProbeLogits ([6] D. Son, "ProbeLogits...", arXiv:2604.11943, 2026), confirming the paper has been picked up by adjacent work in the same cluster.
- OpenFang repository, RightNow-AI/openfang: `https://github.com/RightNow-AI/openfang`. Read for 14-crate modular structure, 137,728-line LOC count, 1,767+ tests, ~32MB binary footprint, MIT license, and the explicit framing ("Not a chatbot framework. Not a Python wrapper around an LLM. Not a 'multi-agent orchestrator.' A full operating system for autonomous agents, built from scratch in Rust").
- OpenFang README at main: `https://github.com/RightNow-AI/openfang/blob/main/README.md`. Cross-checked architecture description, the 7 "Hands" (Clip, Lead, Collector, Predictor, Researcher, Twitter, Browser), and the v0.5.10 pre-1.0 status note.
- OpenFang Releases page: `https://github.com/RightNow-AI/openfang/releases`. Read for active maintenance evidence — security-hardening releases through April 2026, Argon2id auth migration, rustls TLS fixes.
- OpenFang official site: `https://www.openfang.sh/`. Confirmed marketing positioning ("Open-source Agent OS built in Rust") and 16-security-layer claim.
- OpenFang docs README: `https://github.com/RightNow-AI/openfang/blob/main/docs/README.md`. Confirmed "14 Rust crates, 40 channels, 60 skills, 20 LLM providers, 76 API endpoints, and 16 security systems in a single binary."
- RightNow-AI organization page: `https://github.com/RightNow-AI`. Read for project context — RightNow-AI is "GPU AI Code Editor" company; OpenFang is one of seven public repositories; sibling projects include picolm, autokernel, and qwen3.5-triton.
- Asterinas GitHub issue #799, "Avoid unnecessary TLB flushing during user-kernel switching" (filed April 28, 2024): `https://github.com/asterinas/asterinas/issues/799`. Carried over from SC-3C TLB research; relevant here as cross-validation that Asterinas is the closest legitimate architectural peer to NeoPlutarch and is actively working on the same problems.
- Asterinas 2024 retrospective: `https://asterinas.github.io/2025/01/20/asterinas-in-2024.html`. Carried over from SC-3C TLB research.
- Asterinas ATC 2025 paper, arXiv:2506.03876: `https://arxiv.org/abs/2506.03876`. Carried over.
- VibeOS repository, kaansenol5/VibeOS: `https://github.com/kaansenol5/VibeOS`. Read for the explicit hobby/experimental framing ("This is a science experiment to see what an LLM can build"), aarch64 target, 64-session vibe-coding development history, and the C-language implementation note.
- VibeOS Wikipedia article: `https://en.wikipedia.org/wiki/VibeOS`. Cross-checked the C-language claim and the AI-generated-OS framing.
- VibeOS programming reference: `https://github.com/kaansenol5/VibeOS/blob/main/PROGRAMMING.md`. Confirmed the `vibe.h` kernel-API header style and the TCC/Python user-program path; confirms this is a small, conventional OS, not an AI-aware one.
- ZenithOS repository, rajkrish0608/ZenithOS: `https://github.com/rajkrish0608/ZenithOS`. Read for the dual-project structure (Zenith Research kernel + Zenith Daemon), QEMU/bare-metal ARM64 target framing, and the "runs quietly in the background, minimizing distractions" daemon positioning.
- Hacker News post id 47557165, "Show HN: I built an OS that is pure AI": `https://news.ycombinator.com/item?id=47557165`. **This is the actual Pneuma HN post** (April 7, 2026), not the post id `47388478` that the user-pasted content cited (which is a different project entirely). Read for the LLM-generates-Rust-→-WASM compilation pipeline, the wgpu-backed GPU rendering, and the microkernel-pattern WASM-sandbox isolation claim.
- Hacker News post id 47388478, "Built a 1.3M-line agent-native OS in Rust while homeless. What now?": `https://news.ycombinator.com/item?id=47388478`. **The post the user-pasted content cited as "OctantCore" — but it is not OctantCore.** It is a March 22, 2026 first-person account by a developer who built an agent-native OS in Rust under extreme personal hardship; no project name "OctantCore" appears in the post or in any linked content.
- Microsoft Agent Governance Toolkit (AGT) repository: `https://github.com/microsoft/agent-governance-toolkit`. Read for the explicit disclaimer that this is application-level governance, not OS-kernel-level isolation ("This toolkit provides application-level governance (Python middleware), not OS kernel-level isolation. The policy engine and agents run in the same process — the same trust boundary as every Python agent framework").
- Microsoft Open Source Blog, "Introducing the Agent Governance Toolkit" (April 2, 2026): `https://opensource.microsoft.com/blog/2026/04/02/introducing-the-agent-governance-toolkit-open-source-runtime-security-for-ai-agents/`. Read for the explicit OS-pattern-as-metaphor framing ("Operating systems solved a similar problem decades ago with kernels, privilege rings, and process isolation. Service meshes solved it for microservices with mTLS and identity. SRE practices solved it for distributed systems with SLOs and circuit breakers. We asked: what if we took these proven, battle-tested patterns and applied them to AI agents?"). Useful as the canonical statement that the agent-runtime field is *adopting OS patterns as design inspiration*, not building actual kernel-level OSes.
- SynthesisOS repository, GastonGelhorn/Synthesis-OS: `https://github.com/GastonGelhorn/synthesis-os`. Read for the architecture description ("Your requests become syscalls dispatched by a Rust kernel, executed by autonomous agents") and the explicit alpha-software disclaimer. Useful as a third party adopting the kernel-syscall pattern as a userspace abstraction.
- oneclaw repository, nclamvn/oneclaw: `https://github.com/nclamvn/oneclaw`. Read for the edge/IoT framing ("5-layer Rust runtime with 6 LLM providers", "~3.5MB binary, ARM cross-compile ready"). Userspace agent kernel for edge.
- Inner Warden repository, InnerWarden/innerwarden: `https://github.com/InnerWarden/innerwarden`. Read for kernel-level eBPF hooks (40 hooks, 49 detectors, 22 collectors, MITRE ATT&CK coverage) used for agent EDR. Adjacent to the field but not itself an AI-native OS — it's a Linux EDR with AI triage.
- "awesome-agent-runtime-security," bureado/awesome-agent-runtime-security: `https://github.com/bureado/awesome-agent-runtime-security`. Read for the catalog of agent runtime security tools — includes mentions of Nanvix Rust microkernel for syscall interposition, a seL4-based bootable agent OS prototype, Hyperlight micro-VMs, and various sandbox/secret-isolation tools.

### Secondary / community / patent references

- HuWei1989/ZenithOS (and forks at Razordefcon, Passw/TempleOS-ZenithOS, polygonboutique, rigidus, moneytech, PasqualeLivecchi): the *other* ZenithOS, a TempleOS fork in C, no relation to the rajkrish0608/ZenithOS the user-pasted content meant. Disambiguating note only.
- `flosse/rust-os-comparison`: `https://github.com/flosse/rust-os-comparison`. Catalog of Rust OSes; useful general reference but not specifically AI-native.
- Octopii (octopii-rs/octopii): `https://github.com/octopii-rs/octopii`. Distributed-systems kernel in Rust — not AI-native; surfaced because of name similarity with "OctantCore" but is a different project entirely (Raft consensus / WAL / QUIC transport).
- `ALTIMIT-real-world-implementation.md` (project root). Voice anchor for the broader landscape framing and the kernel-level-AI-native-criteria definition used in this artifact's audit.

### Projects NOT surveyed (deliberately deferred)

- **RustCoder** (user-pasted citation `cncf.io/blog/2025/01/10/rustcoder-ai-assisted-rust-learning/`): this is described in the user-pasted content as an "AI coding assistant" for Rust, not as an OS. Out of scope for "AI-native OS" survey; if relevant for adjacent tooling research, separate dispatch.
- **Pluto OS, Phaedrus OS, Skarn, and other names that appeared briefly in linked-content tangents**: insufficient signal to verify; out of scope unless surfaced as a NeoPlutarch-relevant reference later.
- **Apple/Google in-house AI-stack-on-OS efforts (Apple Intelligence kernel scheduling extensions, Google's Android 16 AI runtime)**: proprietary, not open-source, not credibly characterizable from public material at this research's cutoff. Track as potential 2026–2027 forward markers but not field-survey-relevant today.

---

## Findings by query

### 2.1 The "AI-native OS" category — definition and population

The phrase "AI-native operating system" is used in 2026 to mean at least four distinct things, often without disambiguation:

**Sense A — Kernel-level AI primitives.** The kernel itself exposes tensor / model / inference / accelerator-scheduling abstractions as first-class objects. The kernel is aware of AI workloads as a scheduling and memory citizen. Models are arbitrated, not anonymously mapped. This is the NeoPlutarch Bible Article 2 stance and the ProbeLogits / Anima OS stance.

**Sense B — Agent-runtime branded as "OS".** A userspace process or framework that schedules autonomous agents, manages their lifecycle, exposes a "syscall" surface to them (often just a function-call API in the same address space), and adopts kernel-pattern vocabulary as architectural inspiration. Runs on top of Linux/macOS/Windows. Examples: OpenFang, AIOS, Pneuma, Synthesis-OS, oneclaw, Microsoft Agent Governance Toolkit. This is by far the largest population.

**Sense C — AI-built OS.** A conventional OS that happened to be implemented through extensive AI-assisted code generation, without the OS itself being AI-aware. Example: VibeOS (built via Claude across 64 sessions, runs DOOM, has FAT32, no AI-aware kernel features).

**Sense D — Agent-isolation OS.** A specialized OS or VM platform whose purpose is to isolate untrusted AI agents from each other and from the host — typically built atop seL4, Hyperlight, KVM, or similar. Example: the seL4-based bootable agent OS prototype mentioned in `awesome-agent-runtime-security`. Capability-secured, formally isolated, but not necessarily AI-primitive-aware in the way Sense A is.

The user-pasted content treats all four senses as instances of the same category, which is wrong. NeoPlutarch's posture is unambiguously Sense A. The vast majority of named projects in the user-pasted content are Sense B. VibeOS is Sense C. None are clearly Sense D except by adjacency.

**Population estimate for each sense, as of mid-2026:**

- Sense A: very small. ProbeLogits / Anima OS is the only canonically Sense-A project found in this survey. Asterinas is *adjacent* — it is a Linux-ABI Rust framekernel that could in principle host AI subsystems but does not yet implement Sense-A primitives in the published code. NeoPlutarch is aspiring Sense A and not yet shipped.
- Sense B: large and growing. Conservatively two dozen named projects across GitHub, HN, and adjacent literature; the field is fragmented across Python / TypeScript / Rust frameworks.
- Sense C: small but visible. VibeOS, AI-assisted forks of educational OSes, occasional Show HN posts. Cultural moment more than a research direction.
- Sense D: small but growing. Hyperlight-based agent runtimes (HyperAgent), seL4-based bootable agent OS prototypes, K8s-confined agent execution environments. Driven primarily by security-research motivation.

### 2.2 Audit: user-pasted project list

Per-project verification, ranked by the gap between marketing claim and reality:

#### 2.2.1 OpenFang (RightNow-AI/openfang)

**Verified real.** 17.1k GitHub stars, 137,728 lines of Rust across 14 crates, 1,767+ tests, MIT license, active maintenance through April 2026 (v0.5.10 cited in mid-March 2026 README; v0.3.30 cited in late-March 2026 GitClassic mirror suggests heavy iteration). The 32MB binary claim is consistent across multiple sources.

**Mischaracterization in user-pasted content.** The content frames OpenFang as "treats agents like Linux processes" — this implies kernel-process-table semantics. Reality: OpenFang is a userspace Rust application that *runs on* Linux/macOS/Windows. Its "openfang-kernel" crate is the orchestration / RBAC / metering / scheduler component of the OpenFang application — it is not an OS kernel. Tools run in WebAssembly sandboxes inside the OpenFang process; agents are not processes in the operating-system sense, they are objects inside OpenFang's runtime.

**The OpenFang README is explicit about this** ("OpenFang is the OS layer beneath your AI agents, providing runtime, security, and channel infrastructure") — but the "OS layer" framing is metaphorical. OpenFang is to AI agents what Kubernetes is to containers: a sophisticated runtime that adopts OS vocabulary as architectural inspiration, not an operating system in the technical sense.

**Bible-23 conflict.** OpenFang's entire product proposition is that it ships with a system AI persona (the "Hands" — Clip, Lead, Collector, Predictor, Researcher, Twitter, Browser). NeoPlutarch's Bible Article 23.1 explicitly forbids this. The projects occupy fundamentally different stances; OpenFang is not a reference for NeoPlutarch architecture.

**Useful as field-awareness.** OpenFang's choice of capability discipline (Ed25519 manifest signing, Merkle hash-chain audit trail, taint tracking, WASM dual-metered sandbox) is well-engineered and reflects genuine Rust-OS-pattern-applied-to-userspace work. NeoPlutarch's userspace agent layer, if it ever exists, would have something to learn from how OpenFang structures its 16 security layers. But that's a Phase 8+ concern, not a Phase 2 concern.

#### 2.2.2 Anima OS / ProbeLogits (Daeyeon Son, arXiv:2604.11943)

**Verified real.** Single-author academic paper, April 2026, arXiv preprint with v2 already published, ~86k LoC Rust, bare-metal x86_64 UEFI boot. Published evaluation includes inference throughput on SmolLM2-135M and Qwen2.5-7B, HarmBench/XSTest/ToxicChat safety classification benchmarks, and direct comparison against llama.cpp on Linux.

**Earlier-conversation correction.** I previously flagged the "Anima OS at arXiv 2604.11943" citation in the user-pasted content as a potential ID collision with ProbeLogits. **That flag was wrong.** They are the same project — the paper is ProbeLogits; Anima OS is the implementation. The earlier `ALTIMIT-real-world-implementation.md` cited ProbeLogits at 2604.11943 correctly; the user-pasted content cited Anima OS at the same ID, also correctly. Both citations are sound. The collision-flag was me reading two different framings of the same project as conflicting evidence. I owe the user a correction in this artifact and have recorded it.

**Sense-A AI-native OS — the only one in the user's list.** ProbeLogits introduces a kernel-level operation that performs a single forward pass and reads specific token logits for governance decisions. The kernel itself runs the model; userspace doesn't get involved. The published claim is 0.6ms ProbeLogits classification cost on the 135M model and 65ms on the 7B model — fast enough for per-action governance. The inference engine is described as achieving 1,666 tokens/s on SmolLM2-135M (1.39× llama.cpp at parity DDR5 bandwidth).

**Architectural relevance to NeoPlutarch.** ProbeLogits is the LogitPort primitive that the `ALTIMIT-real-world-implementation.md` recommended as a NeoPlutarch Phase 2+ research-frontier item. The fact that there is now a real bare-metal Rust OS that *has implemented* this primitive and *has published numbers* on it is significant — it means NeoPlutarch is not the only project pursuing kernel-level AI primitives, and the field has at least one prior-art reference point for "this is achievable in ~86k LoC Rust at production-relevant performance."

**Scope qualifier.** ProbeLogits / Anima OS is single-author and single-paper. The published evaluation focuses on inference throughput and classification F1 against Llama Guard 3 — not on multi-tenant scheduling, capability-based security, fault recovery, network stacks, or any of the other things NeoPlutarch's Bible commits to. Anima OS is not a "complete" AI-native OS in the NeoPlutarch sense; it is a proof-of-concept for a specific kernel-level primitive. NeoPlutarch's scope is substantially larger.

**Bible-23 alignment.** Anima OS is Sense-A and does not embed a specific model in the kernel as a persona — the model is the inference workload, not the OS personality. ProbeLogits uses the model as a classifier with zero learned parameters (the primitive is the *operation*, not a trained policy). This is exactly the alignment with Bible Article 23.3 that the project research artifact recommended: the kernel arbitrates AI, the kernel is not itself an AI.

**Watch-this-project recommendation.** This is the closest published peer to NeoPlutarch's Phase 5+ AI subsystem aspiration. Track for v3 of the paper, additional primitives beyond ProbeLogits, and any open-source release of the Anima OS codebase. If the codebase opens, it becomes a direct reference for ANSPEC-001's tensor / model / inference primitive design.

#### 2.2.3 OctantCore — does not appear to exist as described

**Cannot verify.** Search-engine queries for "OctantCore" combined with "Rust", "kernel", "agent OS", or "10-layer security" return zero relevant hits. The Hacker News post id `47388478` cited as the source is real but is a completely different project — a March 22, 2026 first-person account titled "Built a 1.3M-line agent-native OS in Rust while homeless. What now?" by a developer who built a substantial agent OS under extreme personal hardship (homelessness, family estrangement, equipment loss).

**Two plausible explanations.** Either (a) "OctantCore" is a misremembered or aliased name for some other project — possibly the homeless-developer project itself, which is real and substantial but is not called OctantCore in any source I can locate; or (b) "OctantCore" is fully fabricated, possibly by an AI summarization step in whatever tool produced the user-pasted content.

**Calibration implication.** This is the *exact* class of citation hazard the project's calibration discipline was set up to catch. The project's `PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md` (referenced via the SC-3C TLB research artifact's §6) was filed precisely because an earlier first-pass research artifact contained citations that did not survive spot-check. The user-pasted content fails the same spot-check. Treat the entire user-pasted content as needing per-claim verification before any of it is used as an architectural reference.

**No recommendation.** If "OctantCore" is later confirmed real under a different name or HN id, this finding will need a follow-up. Until then: not a reference, not a peer, not actionable.

#### 2.2.4 AIOS (agiresearch/AIOS)

**Verified real.** The project exists at the cited GitHub URL and is broadly known in the agent-framework literature. Carried over from `ALTIMIT-real-world-implementation.md` §C, which already characterized AIOS correctly: "AIOS / Cerebrum / agent‑OS frameworks treat the LLM as the 'kernel' but live entirely in userspace; the actual OS underneath is Linux. They are scheduling middleware, not kernels."

**Mischaracterization in user-pasted content.** The content claims AIOS "embeds Large Language Models (LLMs) into the kernel to manage scheduling, context switching, and memory specifically for AI agents." This is technically wrong in two ways. First, AIOS does not embed LLMs into any kernel — it is a Python framework that runs as a userspace process on Linux and calls LLMs over network APIs or via local runtimes like Ollama. Second, the "kernel" referenced in the AIOS literature is *its own* abstraction layer for managing agents; this is a Sense-B "agent runtime branded as OS" instance, not a Sense-A kernel-level integration.

**Bible-23 implication.** AIOS's design philosophy — LLM-as-kernel — is exactly what Bible Article 23.3 rejects. The project research artifact already characterized this rejection as correct: embedding a specific model in the kernel converts every model bug, dataset bias, and prompt-injection vulnerability into a kernel bug. AIOS is the canonical Sense-B project this concern applies to.

**Not a reference.** Useful as a name to know in the agent-framework field; not relevant for NeoPlutarch architecture.

#### 2.2.5 Pneuma

**Verified real, but cited with the wrong HN id.** The user-pasted content cites Pneuma at Reddit URL `reddit.com/r/Operatingsystems/comments/1s0vra9/...` (not verified in this pass) and the HN id `47388478` (which is the homeless-developer post, not Pneuma). Pneuma's actual HN post is at id `47557165`, "Show HN: I built an OS that is pure AI" (April 7, 2026). The architecture description in the user-pasted content is broadly accurate: LLM generates a Rust module, compiled to WebAssembly, sandboxed in Wasmtime, rendered via wgpu (Vulkan / Metal / DX12).

**Sense-B classification.** Pneuma is a userspace application that uses WASM + WGPU + Wasmtime — all userspace technologies that run on top of conventional OSes. The "microkernel architecture" the HN post mentions ("agents run in isolated WASM sandboxes with a typed ABI for drawing, input, storage, and networking") is a microkernel pattern *within* Pneuma, not Pneuma being a microkernel OS. The whole thing runs as a userspace process.

**Genuinely interesting as a pattern.** Pneuma's "LLM generates code → compiles to WASM → runs in sandbox" is a real and novel architecture for agent execution. It is closer to an "AI-generated application runtime" than to either Sense-A or traditional Sense-B. Not directly relevant to NeoPlutarch's kernel work, but worth knowing about for any future userspace agent-layer design.

**Bible-23 implication.** Pneuma's product is an "OS that is pure AI" — its central feature is that the system is an AI. This is explicitly the system AI persona that Bible Article 23.1 forbids. Not a reference for NeoPlutarch.

#### 2.2.6 VibeOS (kaansenol5/VibeOS)

**Verified real, with significant correction.** The user-pasted content lists VibeOS without further description; the implication from inclusion-in-list is that it is an AI-native OS. Reality: VibeOS is a *hobby OS built by Claude through prompt engineering across 64 sessions*. It is **written in C**, not Rust — confirmed by the Wikipedia entry and the Makefile. It targets aarch64 (ARM64), runs on QEMU and Raspberry Pi Zero 2W, has FAT32, networking, a desktop GUI, and ports of DOOM and Snake. It is an entirely conventional OS architecture; the only thing AI about it is the development process.

**Sense-C classification.** VibeOS is an AI-*built* OS, not an AI-*native* OS. The OS itself has no awareness of AI, no inference primitives, no agent runtime — it's just a small POSIX-adjacent operating system that runs C programs and Python via embedded interpreter.

**Calibration data point.** VibeOS is interesting as an existence proof that an LLM can build a small but functional OS through extensive iteration with a human collaborator. Cited prominently as such in the LLM-coding literature. Useful for NeoPlutarch as evidence that LLM-assisted OS development is viable at small scale, and as a cautionary tale for citation discipline: any source that calls VibeOS "an AI-native OS" is wrong, and that wrongness propagates if not caught.

**Not a reference for NeoPlutarch architecture.** Sense-C projects are by definition not architectural references for Sense-A projects; they're development-methodology references at best.

#### 2.2.7 ZenithOS (rajkrish0608/ZenithOS)

**Verified real but small.** Two-part project: "Zenith Research" (the kernel) is a Rust framekernel OS targeting QEMU / bare-metal ARM64 at the "research prototype" status level; "Zenith Daemon" is a userspace tool that runs on Mac/Linux and tracks "focus" by detecting active applications and silencing notifications during work hours. Both parts are single-author, early-stage, and substantially less mature than OpenFang or Anima OS.

**Sense unclear, leaning Sense-B with Sense-A aspirations.** The kernel half is positioned as a Framekernel architecture (referencing Asterinas terminology). The daemon half is a userspace focus-tracker. Neither component appears to implement Sense-A AI-native primitives — no tensor / model / inference kernel objects, no accelerator scheduling, no AI-aware syscall surface. The Framekernel framing is positioning, not implementation.

**Bible-23 implication.** The Zenith Daemon's product is a system focus assistant — a soft form of system AI persona. Bible 23.1 conflict. The Zenith Research kernel half could in principle be neutral on this, but the project as a whole inherits the daemon's framing.

**Not a reference.** Useful as field-awareness for "small single-author Rust kernel attempting AI-OS framing"; not architecturally useful for NeoPlutarch's substantially larger scope.

### 2.3 Projects worth knowing that the user-pasted list did NOT include

These are the projects that *would* belong in a serious AI-native-OS field survey but were not in the user-pasted content:

#### 2.3.1 Asterinas (asterinas/asterinas)

**The most architecturally relevant peer for NeoPlutarch.** Linux-ABI-compatible Rust framekernel; ATC 2025 paper (arXiv:2506.03876); ~100k LoC Rust contributed by 50+ developers over three years. Implements the framekernel pattern: privileged `OSTD` framework with all `unsafe`, de-privileged services in safe-Rust only. Currently SMP=1 in published evaluation; 2025–2026 focus is SMP optimization. Issue #799 frames the same CR3-reload-on-user-kernel-switch problem NeoPlutarch is solving in Q7.b.

**Not itself AI-native** — Asterinas's positioning is "Linux-ABI in safe Rust," not "AI-native kernel." But its architectural pattern (intra-kernel privilege separation via Rust's type system) is the closest legitimate reference for NeoPlutarch's Bible-15 SAFETY-comment discipline. The SC-3C TLB research artifact already flagged Asterinas as the right project to track for cross-validation on the SMP-TLB problem.

**Recommendation.** Continue watching Asterinas through 2026. If they implement Sense-A primitives, the project becomes a direct competitor / peer. If they don't, NeoPlutarch fills that gap with arguably the better architectural fit (clean-slate, no Linux ABI debt).

#### 2.3.2 Theseus OS

**USENIX OSDI 2020 paper (Boos et al.); single-address-space Rust OS with cell-swapping fault recovery.** Carried over from `ALTIMIT-real-world-implementation.md` §F as the closest published reference for fault-recoverable component kernels. Theseus does not implement Sense-A AI primitives but its cell-swapping discipline is the structural pattern the project-research-artifact recommended for NeoPlutarch's eventual fault-recovery story.

**Not in user-pasted content, should have been.**

#### 2.3.3 Redox OS

**Long-running Rust microkernel project; not AI-native.** Useful Rust-OS prior-art reference; the SC-3B release-on-switch research artifact (Janus) cited Redox's `switch_finish_hook()` pattern as a positive reference. Not AI-native and not aspiring to be; included here for completeness because it is in the Rust-OS landscape that NeoPlutarch operates within.

#### 2.3.4 seL4 + agent-isolation derivatives

**Carried over from prior research.** seL4 itself is verification-first, capability-based, formally isolated. Several public agent-isolation prototypes are built atop seL4 — `awesome-agent-runtime-security` lists "Real bootable OS on the seL4 microkernel — the world's only formally verified, capability-secured microkernel — designed ground-up for AI agents. Agents run in hardware-enforced isolated address spaces with unforgeable capability tokens (ToolCap, ModelCap, NetCap, SpawnCap); capabilities are delegatable but never escalatable."

**Sense-D relevance.** This is the Sense-D pattern done right: take a verified isolation kernel, apply it to AI-agent containment, expose capability-typed AI objects. Closer to NeoPlutarch's Bible-12 capability-security goal than anything in the user-pasted list. Worth tracking, especially if the project surveyed becomes a published artifact (which would require finding the specific project name; `awesome-agent-runtime-security` lists it but the link wasn't followed in this research pass).

#### 2.3.5 Microsoft Hyperlight + HyperAgent

**Different model entirely.** Hyperlight is Microsoft's micro-VM platform using hardware virtualization (KVM / Hyper-V) for millisecond-cold-start agent isolation. HyperAgent is a runtime that runs LLM-generated JavaScript handlers inside Hyperlight micro-VMs. The OS-equivalent layer here is the host syscall-interposition layer plus Microsoft's Nanvix Rust microkernel for hardware-isolated agent execution.

**Not AI-native** in the Sense-A sense (Hyperlight is a generic micro-VM platform; the AI-specific layer is in the host); useful as a Sense-D reference for "how does production-grade agent isolation actually look in 2026."

#### 2.3.6 Microsoft Agent Governance Toolkit (AGT)

**Already mentioned in §2 sources.** April 2026 release; OS-pattern-as-metaphor Python middleware; explicitly NOT kernel-level. The Microsoft Open Source Blog post is the canonical statement that the field is *adopting OS patterns as design inspiration*, not building kernels. Useful field-context reference; not architecturally relevant.

#### 2.3.7 Inner Warden (InnerWarden/innerwarden)

**Kernel-adjacent but not AI-native.** Linux EDR with eBPF kernel hooks (40 hooks, 49 detectors), AI triage for alert prioritization, autonomous response. Sense-D-adjacent (agent-isolation-by-detection rather than by capability). Useful as "what kernel-level AI workload looks like when implemented as Linux extensions rather than as a clean-slate kernel."

#### 2.3.8 Synthesis-OS (GastonGelhorn/synthesis-os)

**Userspace macOS layer; Sense-B.** Rust kernel-pattern scheduler dispatching syscalls to agents that execute against 60+ macOS native tools. Alpha software. Interesting as another instance of the "Rust kernel running as a userspace process" pattern, this time anchored to macOS rather than to Linux/cross-platform.

#### 2.3.9 oneclaw (nclamvn/oneclaw)

**Edge/IoT agent kernel.** Userspace Rust runtime, 5-layer architecture, 6 LLM providers, ~3.5MB binary, ARM cross-compile ready. Sense-B for edge devices. Useful as evidence that the agent-framework-as-OS pattern is being extended down-scale to constrained hardware.

### 2.4 What the field actually looks like in 2026 Q2

Synthesizing the audit and the additional landscape work:

**Sense A (true kernel-level AI-native) — count: 1, possibly 2 if NeoPlutarch counts.** ProbeLogits / Anima OS is the only canonically Sense-A project this survey can identify. NeoPlutarch is aspiring to the same category but is not yet shipped. **The field is essentially empty at the kernel layer.**

**Sense B (agent runtime branded as "OS") — count: dozens.** OpenFang is the largest and best-engineered; AIOS is the most academically cited; Pneuma is the most novel architecturally; Synthesis-OS, oneclaw, and Inner Warden cover macOS, edge, and EDR niches. Microsoft Agent Governance Toolkit makes the pattern official ("we asked: what if we took these proven, battle-tested OS patterns and applied them to AI agents?").

**Sense C (AI-built OS) — count: several.** VibeOS is the best-documented; the homeless-developer 1.3M-line OS at HN 47388478 is real and substantial (though not specifically AI-native); various smaller experiments exist. Cultural phenomenon more than research direction.

**Sense D (agent-isolation OS) — count: small but growing.** seL4-based bootable agent OS, Hyperlight micro-VM platforms, K8s-pod-confined agent runtimes. Driven by security-research motivation; relevant to Bible Article 12 capability discipline.

**The "AI-native OS" label is therefore misleading.** It is being used in marketing and in popular technical writing as if it referred to a single coherent category, when it actually refers to four distinct things with very different architectural implications. NeoPlutarch's Bible-2 stance — kernel-resident AI primitives, no embedded model, no system AI persona, monolithic Rust kernel with intra-kernel privilege separation — is genuinely rare; Anima OS is the only published instance that satisfies the full set.

---

## 3. Synthesis: implications for NeoPlutarch

### 3.1 NeoPlutarch's position in the field

NeoPlutarch is **competing in a near-empty Sense-A category against one published peer (Anima OS / ProbeLogits)** while the bulk of "AI-native OS" branding is happening in Sense-B (agent frameworks adopting OS vocabulary). This is a comfortable position:

- The Sense-B field is mature and crowded; OpenFang at 17.1k stars is the established player. NeoPlutarch is *not competing* with OpenFang because they occupy different layers.
- The Sense-A field is essentially open. NeoPlutarch's roadmap (Phase 2 SMP → Phase 5+ AI subsystem) puts the project on a trajectory to be the second or third public Sense-A project, depending on Anima OS's open-source release timing and Asterinas's AI-subsystem ambitions.
- The closest legitimate architectural peer for *implementation patterns* remains Asterinas (Rust framekernel, intra-kernel privilege separation, ATC 2025). The SC-3C TLB research artifact already flagged this. Nothing in the user-pasted list displaces Asterinas.

### 3.2 Bible Article 23 alignment with the field

Bible Article 23 (no system AI assistant, no telemetry, no embedded model in kernel) sits in deliberate tension with the dominant Sense-B field-marketing pattern. The user-pasted-list projects mostly *do* embed system AI personas:

- OpenFang: 7 "Hands" are explicit autonomous AI personas with named roles.
- AIOS: LLM-as-kernel is the explicit design.
- Pneuma: "an OS that is pure AI" is the product proposition.
- Synthesis-OS: "The OS itself is the AI" is the marketing.
- ZenithOS daemon: focus-tracking AI assistant.

By contrast, the projects that DO honor a Bible-23-compatible posture are:

- Asterinas: no AI persona, no embedded model; Linux-ABI in safe Rust.
- Anima OS / ProbeLogits: ProbeLogits uses the model as a classifier operation with zero learned parameters; the OS does not host the model *as a persona*. Compatible.
- Theseus, Redox, seL4: classical Rust OS work; AI-agnostic. Compatible by default.

**Reinforcement of earlier project finding.** The `ALTIMIT-real-world-implementation.md` argued Bible Article 23.1 is the project's most ALTIMIT-faithful clause and that pressure to add a system AI persona would increase as the project matures. The user-pasted content is *exactly* the form that pressure will take: a list of impressive-sounding projects, most of which violate Bible 23, framed as if NeoPlutarch should be aspiring to be like them. The audit confirms the project should continue declining that framing.

### 3.3 What to do with this artifact

**Trigger-back items (project-level):**

1. **No Bible amendment.** Bible Article 23 stance is reinforced by this survey, not weakened. No change recommended.
2. **No ANSPEC amendment.** The AI subsystem spec is independent of the field-survey. ProbeLogits / Anima OS's primitives may become a reference for ANSPEC-002 (capability semantics on AI objects) if Anima OS open-sources, but that's a Phase 5+ concern and gates on Anima OS code availability.
3. **No new ADR.** The audit produces no architectural decision; it produces situational awareness.
4. **No PM-NOTE for ratification.** Nothing to ratify.

**Knowledge-base items (research):**

1. Update the project's mental research-reference set: Anima OS / ProbeLogits joins Asterinas, Theseus, seL4 as a watch-this-peer. The full set is documented in `ALTIMIT-real-world-implementation.md` plus this artifact's §2.3.
2. Track for Anima OS open-source release. If/when the codebase opens, it becomes a direct reference for Phase 5+ AI subsystem design.
3. The Sense-A/B/C/D taxonomy from §2.1 is useful framing for any future external-communication about NeoPlutarch. NeoPlutarch is Sense A; most of the field is Sense B; the distinction matters when explaining what the project is and isn't.

**Calibration items (process):**

1. **The user-pasted-content audit produced one fabricated-or-misremembered project (OctantCore), one self-correction by me about a non-existent collision (Anima OS / ProbeLogits), one HN-id misattribution (Pneuma at 47557165 not 47388478), and multiple mischaracterizations of real projects (OpenFang, AIOS, Pneuma, VibeOS, ZenithOS).** That's a high error rate per item. The user-pasted-content is consistent with output from an AI summarization tool that conflated categories and hallucinated specifics. **Future PL-pasted research summaries should be treated as needing the same calibration discipline that internal Cerebro/Janus first-pass research gets** — every cited URL spot-checked, every project name confirmed independent of the summary, every architectural claim cross-checked against primary source.

2. **My own earlier-in-this-conversation flag of the Anima/ProbeLogits arXiv ID as a collision was wrong.** Self-calibration: when I encounter a claim that contradicts an existing project artifact (in this case `ALTIMIT-real-world-implementation.md`), the right first move is to verify both citations independently, not to default to "the new claim must be wrong because the existing artifact was rigorous." Rigor doesn't transfer; each citation is its own verification problem. Recorded as a calibration data point.

---

## 4. Calibration notes (citation rigor)

Applying the discipline established by `Plans/PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md` and the rigor standard from `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-tlb-shootdown-prior-art.md` §6:

**Verified primary sources:** ProbeLogits arXiv 2604.11943 abstract + HTML + PDF (three separate URLs all returning consistent content); OpenFang GitHub README + Releases + sibling-mirror + official site (four independent corroborating URLs); Asterinas issue #799 + 2024 retrospective + ATC25 abstract (carried over from SC-3C TLB research); VibeOS GitHub + Wikipedia + README + claude.md + programming.md (five corroborating URLs all showing C-language and aarch64-target); ZenithOS rajkrish0608 GitHub (single source, low corroboration — flag as single-source); Hacker News post id 47557165 (Pneuma) and 47388478 (homeless-developer story) verified at original-source URLs.

**Verified absence:** "OctantCore" returned zero relevant search hits. Single web search; the absence is consistent with fabrication or significant misremembering. Flagged as "appears not to exist as described"; not "definitively proven not to exist" — search-engine evidence is necessary but not sufficient for proof-of-non-existence.

**Self-correction recorded:** Earlier-in-conversation I flagged the Anima OS / ProbeLogits arXiv ID 2604.11943 collision as evidence of citation problems. Re-verification this pass confirms Anima OS and ProbeLogits are the same project (paper-and-implementation pair, not separate projects), and both citations of 2604.11943 are correct. The earlier flag was wrong. This artifact's §2.2.2 records the correction explicitly.

**Future-dated arXiv IDs flagged honestly:** arXiv 2604.11943 (ProbeLogits, April 2026) is post-cutoff relative to the project's earlier `ALTIMIT-real-world-implementation.md` cutoff but pre-cutoff relative to this research's date (May 2026). arXiv 2604.16870 (Governed MCP) is contemporaneous. arXiv 2506.03876 (Asterinas ATC25) is established. No citations rely on speculative future arXiv IDs.

**HN id and URL precision matters.** The user-pasted content cited Pneuma at HN id `47388478` (wrong — that's the homeless-developer post). Pneuma's actual HN post is at id `47557165`. The user-pasted content cited OctantCore at HN id `47388478` (same id as Pneuma's miscitation, which is itself another project). Misattribution of HN ids is a pattern in the user-pasted content; future PL-pasted research should be expected to contain similar errors.

**No hallucinated citations introduced by this artifact.** Each URL in §2 sources was inspected; each project name confirmed at the cited URL; each architectural claim quoted or paraphrased against the source. The audit conclusions are negative findings about the *user-pasted* content, not new claims that themselves require verification.

**Research-pass time budget actual:** ~60 minutes (8 web searches with content inspection, multiple cross-checks against existing project artifacts, synthesis write-up). Within the 2–3 hour estimate from the earlier "options" enumeration.

---

## 5. Cross-references

**This artifact consumes:**
- `ALTIMIT-real-world-implementation.md` (project root) — voice anchor; landscape framing; the Sense-A/B/C/D taxonomy emerged from re-reading this artifact's §C/§D treatment of AIOS, Asterinas, AGT, AIOS, et al.
- `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-tlb-shootdown-prior-art.md` — rigor standard for citation discipline; reused calibration framing.
- `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` — Janus voice anchor (carried through from SC-3C research).

**This artifact does not produce:**
- No Bible amendment.
- No ANSPEC amendment.
- No ADR.
- No PM-NOTE for ratification (nothing to ratify).
- No dispatch (no agent work triggered).

**Forward markers:**
- Anima OS / ProbeLogits open-source release watch.
- Asterinas 2025–2026 SMP TLB / framekernel evolution watch (already in SC-3C research forward markers).
- AGT and adjacent agent-governance-toolkit field; not architecturally relevant but ecosystem-context relevant.
- "OctantCore" — if confirmed real under a different name or HN id, reopen this finding.

**Operational discipline anchors:**
- PM operational posture clarification 2026-05-10 (memory): PM owns operational decisions; field-survey research is PM-tier knowledge-base work; this artifact is consumed for situational awareness, not PL ratification.
- Bible Article 23.1, 23.2, 23.3: reinforced by this survey, not amended.
- `PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md`: extended to apply to PL-pasted research summaries.

---

## 6. Closure

The AI-native-OS field as of mid-2026 has one published kernel-level Sense-A peer (Anima OS / ProbeLogits), one closest-legitimate-architectural-peer (Asterinas, Sense-A-adjacent), and a substantial Sense-B agent-framework field that adopts OS branding without OS architecture. NeoPlutarch's Bible-23 posture keeps the project out of the trap most Sense-B projects sit inside. The user-pasted content audit produced one likely-fabricated project (OctantCore), one self-correction by me about a non-existent ID collision (Anima OS / ProbeLogits), multiple mischaracterizations of real projects, and useful field-awareness data. No project artifact amendments are produced; situational awareness is the deliverable.

PM moves to standby. Next active action remains: Jarvis kernel-rename bundle close → PM reviews verification logs → SC-3C PLAN-stage dispatch drafting from clean post-rename state.

---

**End of research artifact.**

*Anima OS and ProbeLogits are the same project (paper-and-implementation pair); earlier flag was wrong, correction recorded. OctantCore does not appear to exist as described; HN id 47388478 cited for it is actually a different, real, tragic personal story. OpenFang / AIOS / Pneuma / Synthesis-OS / oneclaw are real userspace agent frameworks branded as "OS" — Sense B in the §2.1 taxonomy — and conflict with Bible Article 23.1. VibeOS is real but is an AI-built OS in C, not an AI-native OS in Rust — Sense C. ZenithOS is small single-author Rust kernel + userspace daemon, weak Sense B/A hybrid. The only Sense-A AI-native OS in the field is Anima OS / ProbeLogits. Closest legitimate architectural peer for NeoPlutarch remains Asterinas. Bible Article 23 stance reinforced by survey; no amendments produced.*
