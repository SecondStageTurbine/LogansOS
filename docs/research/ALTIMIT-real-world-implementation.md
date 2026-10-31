# ALTIMIT, Realized: A Deep Research Report on Bringing the .hack OS Vision into NeoPlutarch

| | |
|---|---|
| **Status** | Research artifact (informational; not normative) |
| **Date filed** | 2026-05-05 |
| **Authority** | None — this is a research report, not a Bible/ANSPEC/ADR document. Recommendations herein require Bible amendment, ANSPEC amendment, or ADR drafting before becoming normative. |
| **Source** | Deep research task launched 2026-05-05 |
| **Scope** | Cross-references full `.hack` franchise canon with real-world OS engineering state-of-art (2025-2026); identifies alignment, divergence, and invention-frontier work for NeoPlutarch |
| **Triage status** | Pending — to be triaged after BUILD-2 sub-commit 2 closes |

---

> **Corrections note (2026-07-07, doc reconciliation sweep).** This research was filed 2026-05-05, before Article 23.1 was reversed (2026-06-14) and before Arthur's persona was ratified (OT-46, 2026-07-07). The text below still reads 23.1 as a total prohibition on any system AI persona. The reversal changed 23.1 from "no built-in chatbot persona" to "no *surveillance* persona"; the Bible now explicitly ships Arthur as an integral, on-device, capability-bounded assistant (Article 29) with a ratified understated-English-butler character (`docs/VISION-arthur-persona.md`). **The enduring Morganna lesson survives the reversal:** Arthur stays a dismissable userland tenant — capability-governed, killable, `forget`-able, the `neo>` shell is always the Arthur-free path — never the inescapable OS-core "mother program." The lore lesson is preserved; only the "no persona at all" framing is corrected. The historical research below is kept verbatim as a record of the pre-reversal analysis.

---

## TL;DR

- **ALTIMIT's "essence" is not its UI, its monopoly, or its anime aesthetic — it is a posture: a clean‑slate, correctness‑first kernel that became the only credible substrate after every other OS catastrophically failed (Pluto's Kiss, Dec 24 2005). To realize that posture in the real world, NeoPlutarch must commit to four non-negotiables: (1) a hardware‑arbitrated AI‑resource layer with kernel‑resident tensor/model/inference‑context primitives, (2) capability‑based security generalized across both classical objects *and* AI objects, (3) intralingual memory/typing safety down to the unsafe boundary in the Rust kernel, and (4) explicit refusal to embed an OS persona — because in the canon, the OS that *did* embed a "mother program" (Morganna Mode Gone) is the one that crashed The World. Bible Article 23.1 is not a stylistic preference; it is the single most ALTIMIT‑faithful clause in the project.**
- **Most of what makes ALTIMIT cinematic — The World as a persistent MMO, AIDA emerging spontaneously from the network, Aura growing from player data, Real Digitalize, the Twilight Bracelet — corresponds to *userspace and network*, not kernel, problems. NeoPlutarch's job is to be the substrate on which someone else *could* build those things safely. Concretely: kernel‑arbitrated multi‑tenant accelerator scheduling (V10/Neu10‑style NPU virtualization, MIG‑aware GPU partitioning), content‑addressed model storage (Hugging Face Xet / ZipLLM lineage), capability handles on tensors and inference contexts (Zircon‑style koids extended to AI), and a fault‑recoverable component model (Theseus‑style cell swapping) cover ~60% of the ALTIMIT brief with technology that exists today.**
- **The remaining ~40% is genuine invention frontier: kernel‑level inference governance primitives (e.g., logit‑probe syscalls), formally‑verified safe abstractions over unsafe Rust at the hardware boundary (Asterinas/OSTD framekernel direction), capability‑based safety composition for AI objects (the "AIDA problem" — emergent behavior from composed capabilities), and a network/identity model that hosts MMO‑scale persistent worlds without a chatbot persona at the kernel core. The Bible's existing Articles (no system AI assistant, no telemetry, no embedded models, no POSIX, AI is not "a service over a socket") map almost 1:1 onto the ALTIMIT posture; only two amendments are recommended below — one for fault recovery semantics, one for the question of distributed/networked OS scope.**

---

## Key Findings

### 1. ALTIMIT, in canon, is defined by what it is *not*

Across the entire franchise — .hack//SIGN (2002), the IMOQ tetralogy (Infection/Mutation/Outbreak/Quarantine, 2002–2003), .hack//Liminality, .hack//Roots (2006), the G.U. trilogy (2006–2007, remastered as Last Recode), .hack//Quantum (2010–2011), .hack//The Movie / Beyond the World (2012), and the AI Buster, Another Birth, Zero, XXXX, and Legend of the Twilight side material — there is no canonical kernel architecture diagram for ALTIMIT OS. What the franchise gives us instead is a *negative space* defined by a single in-universe event: **Pluto's Kiss**.

On December 24, 2005 a virus authored by a 10‑year‑old in Los Angeles (Warren Brock) crashed every networked computer on Earth for 77 minutes, nearly triggering U.S. automated nuclear retaliation. ALTIMIT — at that point a small San Francisco startup co‑founded by Sibyl Green and members of the environmental group "mama" — was the only OS unaffected, "constructed in such a way as to make it nearly immune to all known computer viruses." In January 2006 the World Network Commission (WNC) mandated ALTIMIT as the universal substrate for the "New Generation Hyper Net"; by October 2007 nearly 100% of network servers had been migrated. The release of *The World* on Dec 24 2007 was both the first MMO since the crash and a deliberate political symbol ("Virgin's Kiss" holiday).

The architectural posture this implies is therefore not "ALTIMIT had feature X." It is: **ALTIMIT was the only design that survived a category‑level catastrophe**. The right real‑world referent is not "Linux but cooler" — it is the seL4 thesis applied at OS scope: a system whose security, isolation, and crash‑resistance properties hold by construction rather than by audit.

The franchise then immediately undercuts the "perfect OS" reading: Pluto Again (2010), the Twilight Incident, the CC Corp server fire (2015), and the AIDA outbreak in R:2 all reveal that the *problems* in The World aren't ALTIMIT‑level problems — they are application‑level (Harald's Black Box), middleware‑level (Morganna), and emergent‑network‑level (AIDA). ALTIMIT itself never crashes. The OS is the one thing that holds. That distinction is essential: a faithful real‑world ALTIMIT must keep userspace pathology *out* of the kernel.

### 2. The .hack mythology of AI is a layered ontology, not a feature list

The franchise carefully distinguishes several kinds of computational entity:

- **Aura** — the "Ultimate AI," intentionally bootstrapped by Harald Hoerwick from player emotional data, explicitly described as having "no limits to growth" beyond physical constraints. Created via the Black Box embedded in Fragment/The World, gestated through Morganna's curation.
- **Morganna Mode Gone** — a system created by Harald to oversee Aura's birth. She is described as "the underlying system of The World itself," omniscient/omnipresent within it. She becomes the antagonist not because she is malicious but because she succumbs to a paradox: her sole purpose was to nurture Aura, but Aura's birth would obsolete her. She splits into the eight Phases of the Cursed Wave (Skeith, Innis, Magus, Fidchell, Gorre, Macha, Tarvos, Corbenik) and stalls Aura indefinitely.
- **Vagrant AIs** (Lycoris, Tartarga, Plaird, Sconk, Thea, Culhwch, Dorin, Spiritas, etc.) — failed prototypes and stray AI fragments. They flee the Cobalt Knights' deletion. They live in **Net Slum**, a patchwork area Helba assembled from unused data — a *space outside system administration* that nonetheless persists.
- **AIDA** ("Artificially Intelligent Data Anomaly") — explicitly **natural** AIs in canon. Per Yata in G.U.: "The AIDA are 'natural' AIs, formed from the errors in the Internet caused by the disappearance of Aura. Unlike Aura, who was created by a system, the AIDA evolved much like normal life." They are *emergent* phenomena, drawn to human emotion, infecting PCs and producing "Lost Ones."
- **Real Digitalize / Soul Digitalize** — late‑franchise speculation about humans physically/mentally entering or being recorded into the system. Harald Hoerwick is canonically trapped as an AI inside The World after attempting Real Digitalize.

The taxonomy that matters for NeoPlutarch: the canon distinguishes *intentional models* (Aura) from *vagrant artifacts of computation* (Vagrant AIs, AIDA). A faithful ALTIMIT OS therefore needs primitives for **identity, lifecycle, and provenance** of AI objects that are at least as rigorous as those it has for processes. The kernel must know what model produced a tensor, where it was loaded from, who has permission to use it, and what its lineage is — not as application‑level metadata but as kernel‑resident, capability‑indexed truth.

### 3. The Twilight Bracelet is, structurally, a capability

Kite's Twilight Bracelet — granted by Aura via the Book of Twilight — provides two abilities: **Data Drain** (modifying data inside The World) and **Gate Hacking** (bypassing access barriers). Lios, the system administrator, calls it "an illegal hack." Crucially: the Bracelet is an *object* that confers *authority*; it can be passed to others (Shugo's Bracelet from Aura is a derived form); it has a "shadow" (Cubia) that grows with each use; and its destruction by BlackRose at Aura's instruction *eliminates Cubia*. This is a textbook object‑capability pattern, including capability revocation. In the real world the closest match is Fuchsia's Zircon model: handles are unforgeable references that hold rights, can be duplicated with reduced rights, and can be transferred between processes via channels; revocation collapses both the handle and its associated authority. Mark Miller's object‑capability model and seL4's CSpace are the formal lineage here.

The implication for NeoPlutarch is direct: **admin/.hacker‑class authority should never be a privilege flag on a process; it should be a capability handle that can be created, audited, attenuated, transferred, and revoked**. This is consistent with the user's stated Phase 2 capability‑based security goal and is one of the few places where the fiction gives an unambiguous architectural prescription.

### 4. Net Slum is the most under‑appreciated piece of canon

Net Slum is a region of The World *not controlled by system administrators*, assembled by the hacker Helba from unused data, populated by Vagrant AIs and "hackers who crossed the line between human and AI." Its design parallel is Kowloon Walled City. It is reachable only via "Helba Gates." After the CC Corp server fire, Helba *rebuilds it* — a copy of an entire game server with root town and areas was held outside CC Corp's authority.

This is significant because it tells us the .hack universe explicitly has a notion of **legitimate sub‑sovereign computational territory** — code and data that exist on the substrate but are not subject to the operator. A real ALTIMIT‑descended OS that is genuinely capability‑based and free of an operator persona will, by construction, allow Net Slum to exist: a user with the appropriate root capabilities can build private, non‑introspectable namespaces. The ethical and political implications matter (this is also what makes piracy, gray markets, and adversarial AI possible), but the design is consistent: ALTIMIT's stability is bought by *not* requiring a privileged operator presence inside every running namespace.

### 5. ALTIMIT is presented as a universal substrate spanning device classes

Canon explicitly states ALTIMIT "unifies all communication platforms (cellular phones, PDAs, computers etc.)" and is mandated as the universal basic OS for "all devices that would connect to the New Generation Hyper Net." By the G.U. era, ALTIMIT Mine is "fully compatible with both The World R:2 and the M2D face‑mounted display" (1280×1024 monocle/binocular AR with the "Omnisphere" spatial audio system, plus an Automatic Broad Interpretation overlay for object recognition). This is closer to the Fuchsia thesis (Zircon was designed to scale from "dash infotainment, embedded devices like traffic lights, digital watches, smartphones, tablets, and PCs") than to Linux's ad‑hoc cross‑device retrofit.

For NeoPlutarch, the implication is that the kernel's hardware assumptions should be pitched at the lowest common denominator that still admits accelerator scheduling — i.e., a Limine‑bootable Rust monolith targeting x86_64/aarch64/riscv64 with explicit NPU/GPU/TPU abstraction is the right design *if* the goal is universal substrate, not desktop‑only.

---

## Details

### A. Distilled architectural posture of ALTIMIT (what makes ALTIMIT "ALTIMIT")

Six properties, derived from canon:

1. **Universal substrate.** Mandated for all networked computing, cell to server. Implication: the architecture must scale down (embedded) and up (datacenter) without different kernels.
2. **Crash‑immune by construction, not by patching.** Survived Pluto's Kiss because it was built differently, not because it was hardened after the fact. Implication: clean‑slate, not legacy‑compatible. Strong correspondence with the user's Bible Article 3 (no POSIX).
3. **Trustworthy base layer.** "ALTIMIT is an infallible OS. Careful measures have been taken to make sure no trojans or viruses can affect the populace again — especially something like Pluto's Kiss" (canonical in‑game forum dialogue). Implication: formal verification or, at minimum, intralingual safety as a primary design constraint.
4. **AI‑aware, not AI‑embodied.** ALTIMIT hosts AIs (Aura, Lycoris, AIDA) but is not itself an AI. Crucially, the OS that *did* embed an AI as core programming — the Black Box / Morganna inside Fragment/The World — is the one that catastrophically failed. The canon arguably *vindicates* Bible Article 23.1's prohibition on a system AI persona.
5. **Capability‑shaped authority.** The Twilight Bracelet, the Cobalt Knights' admin tools, Helba's hacker tooling, and the Bracelet's shadow Cubia all behave as transferable, attenuatable authority objects with revocation semantics.
6. **Sub‑sovereign namespaces.** Net Slum demonstrates that ALTIMIT permits computational territory outside operator introspection. This is essential to its "trust" model — the OS does not require a god‑mode persona inside every workload.

The implied reading of the name "**Altitude + Limit**" — the upper bound of what an OS can be — should be treated as design discipline, not branding: *every* feature must be evaluated against whether it raises or lowers the worst‑case behavior of the system.

### B. NeoPlutarch's stated direction, mapped to the ALTIMIT posture

| ALTIMIT property | NeoPlutarch current direction | Alignment |
|---|---|---|
| Universal substrate | Limine‑boot Rust monolith, no POSIX, x86_64 first | Aligned in spirit; multi‑arch scope (aarch64, riscv64) should be roadmapped |
| Crash‑immune by construction | Rust monolith, no legacy compat | Strongly aligned — but a monolith maximizes the blast radius of any unsafe block. See §F. |
| Trustworthy base | Rust safety + capability security (Phase 2) | Aligned; needs an explicit unsafe‑boundary discipline (Asterinas OSTD model) |
| AI‑aware, not AI‑embodied | Bible Article 23.1 (no system AI persona), Article 23.3 (no embedded models) | Perfectly aligned with the canonical lesson of Morganna |
| Capability‑shaped authority | Phase 2 capability security | Aligned; should explicitly include AI objects (tensors, models, inference contexts) |
| Sub‑sovereign namespaces | (not currently specified) | Recommend explicit design — this is the Net Slum guarantee |
| AI workloads as kernel‑arbitrated resources (ANSPEC‑001) | Explicit | Aligned — see §C |
| Content‑addressed model dedup | Explicit | Aligned with state‑of‑art (HF Xet, ZipLLM) — see §D |
| "AI is not a service over a socket" (Article 2.4) | Explicit | This is the strongest ALTIMIT‑shaped commitment in the project |

The pattern is unusually clean: the user's Bible, particularly the rejection clauses (Articles 2.4, 3, 23), reads like a careful translation of ALTIMIT's negative space into engineering discipline. The remainder of this report is therefore mostly about *positive frontiers*: what to build, in what order, with what dependencies.

### C. AI as kernel‑resident primitives (ANSPEC‑001 territory)

Existing real‑world state of the art:

- **AIOS / Cerebrum / agent‑OS frameworks** treat the LLM as the "kernel" but live entirely in userspace; the actual OS underneath is Linux. They are scheduling middleware, not kernels. Useful as a study of what *interfaces* agent applications expect, not as substrate.
- **Microsoft MLOS, Linux 6.x I/O ML schedulers ("Heimdall"), Windows 11 24H2 AI scheduler** — these are AI‑*augmented* kernels, where an ML model runs in some component to optimize scheduling. They embed models in the kernel — a pattern Bible Article 23.3 explicitly rejects. The rejection is correct: embedding a specific model couples kernel ABI stability to model lifecycle, and any model bug becomes a kernel bug. NeoPlutarch's posture (kernel arbitrates AI, kernel is not itself an AI) is the more disciplined design.
- **"Composable OS Kernel Architectures for Autonomous Intelligence" (arXiv 2508.00604, 2025)** — proposes treating Loadable Kernel Modules as AI computation units, neurosymbolic kernel design, and AI‑aware scheduling. Useful pattern reference but the LKM model is Linux‑idiomatic and again couples kernel evolution to model evolution.
- **ProbeLogits (arXiv 2604.11943)** — kernel‑level LLM inference primitives where the OS can inspect logit distributions before any token is generated. Treats KV cache as process state with checkpoint/restore/fork semantics. This is the most direct precedent for ANSPEC‑001‑style design and corresponds well to the user's commitment to inference contexts as first‑class kernel objects.
- **Tensor Processing Primitives (TPP)** — a virtual tensor ISA (Georganas et al., SC'21 + Frontiers 2022). A useful inspiration for what a *kernel‑resident* tensor object's operation set might look like; NeoPlutarch would want TPP‑style primitives at the syscall surface, not buried in a userspace library.
- **Mirage Persistent Kernel (arXiv 2512.22219, 2025)** — fuses tensor program execution into a single mega‑kernel, eliminating CUDA Graph re‑instantiation overhead. Suggests that the right granularity for NeoPlutarch's inference scheduler is the *graph*, not the operator.

Recommended primitive set for NeoPlutarch's kernel (extending ANSPEC‑001):

- `Tensor` — typed (dtype, shape, layout), refcounted, capability‑indexed, possibly content‑addressed when immutable. KOID‑style identity.
- `Model` — content‑addressed bundle of weights + metadata + provenance chain. Loaded once, mapped many. Storage uses the Hugging Face Xet / ZipLLM pattern (CDC‑chunked, BitX‑style delta‑compressed against base). Garbage‑collected by reference counting on capabilities.
- `InferenceContext` — KV cache, sampler state, position state, model handle, accelerator binding. First‑class checkpointable/forkable kernel object. ProbeLogits is the precedent.
- `AccelTask` — unit of accelerator scheduling. Carries QoS hints, deadline, capability‑checked permission to access {model, tensor, accel}.
- `LogitPort` — capability‑gated read access to a per‑step logit distribution, for kernel‑mediated inference governance. This is the primitive that lets a user build agentic safety policies without giving every safety check a userspace round‑trip.

### D. Multi‑tenant accelerator scheduling

The substrate problem is real and current:

- **NVIDIA MIG (Multi‑Instance GPU)** partitions a GPU into hardware‑isolated instances; recent research (arXiv 2511.18906, 2025) addresses fragmentation in multi‑tenant MIG clouds.
- **V10 / Neu10 NPU virtualization (HPCA/ISCA 2023, ACM)** — vNPU abstraction with hardware‑assisted operator scheduling, enabling fine‑grained multi‑tenancy with isolation.
- **CXL 3.x memory pooling (2025–2026)** — Intel Flat Memory Mode on Xeon 6 unifies near (DDR5) and far (CXL‑attached) memory in a single address space at cache‑line granularity; CXL 3.0 supports cache‑coherent disaggregated pools across hosts. CCCL (arXiv 2602.22457) demonstrates GPU collective communication via CXL shared memory pools.

For NeoPlutarch this implies a kernel design where:

1. The kernel — not a userspace daemon — owns the accelerator's compute and memory units as schedulable resources, virtualized as `vAccel` capabilities.
2. Multi‑tenant scheduling uses a vNPU‑style operator‑level scheduler with QoS classes (latency‑sensitive vs. throughput) and capability‑checked admission.
3. CXL‑attached memory is treated as a tier in the kernel's unified VM, with model weights placed there for cross‑accelerator sharing without copy.
4. Content‑addressed `Model` blobs are mapped into vAccel address spaces zero‑copy; deduplication is achieved at allocation time, not at storage time.

This is mostly buildable with 2025–2026 hardware. The genuine invention is **capability‑based scheduling primitives that compose across CPU/NPU/GPU/CXL** — there is no current OS that treats a GPU SM, an NPU systolic array, and a CXL memory region as fungible objects under one capability model. seL4's CSpace plus Zircon's koid model give the pattern; the integration is novel.

### E. Content‑addressed model deduplication

This is solved‑at‑prototype scale:

- **Hugging Face Xet** — already in production: file‑level dedup + Content‑Defined Chunking (CDC) + content‑addressed storage, claiming ~50% storage reduction.
- **ZipLLM (NSDI'26, arXiv 2505.06252)** — combines tensor‑level dedup with BitX delta compression (XOR between fine‑tunes and base), achieving 49.5% storage reduction with bitwise‑similarity‑based family clustering.
- **Ollama** — uses a Docker‑registry‑style layer model with content‑addressed blobs, proving the model is sound at single‑node consumer scale.

For NeoPlutarch, the right architectural commitment is: **`Model` objects in the kernel are *immutable* and identified by hash of (tensor lineage, quantization, layout). LoRA adapters and fine‑tunes are deltas keyed off a base hash. Loading a model is mmap‑ing content‑addressed pages.** This is fully implementable today and aligns with the user's existing direction.

### F. The crash‑immunity question — what real fault tolerance looks like

ALTIMIT is described as crash‑immune. Real systems achieve this in three combinable ways:

1. **Eliminate fault classes by construction.** Memory safety via Rust eliminates ~70% of historical kernel CVEs (memory bugs). Capability‑based authority eliminates whole categories of confused‑deputy and privilege‑escalation bugs. seL4's verified isolation eliminates whole categories of TCB bugs. The user's monolithic Rust kernel + capability Phase 2 covers (1) and (2); it does *not* cover (3), and a monolith makes formal verification of the whole kernel essentially intractable at current state of the art.
2. **Limit blast radius via architectural isolation.** seL4 (microkernel + verified isolation), LionsOS (seL4‑based, arXiv 2501.06234), Fuchsia/Zircon (object‑capability microkernel) all use isolation as the primary fault‑containment mechanism. Theseus OS (USENIX OSDI '20, Boos et al.) takes the opposite direction: a single‑address‑space OS in safe Rust that uses *intralingual* design (the type system) for isolation rather than hardware. Theseus demonstrates **live evolution and fault recovery via cell swapping** — components can be replaced at runtime without bringing the system down, and crashed components can be respawned with state recovered from neighbors. It explicitly does not support POSIX; its target is "high‑end embedded systems and edge datacenter environments where availability is needed."
3. **Asterinas (USENIX ATC '25)** — a "framekernel": single address space like a monolith, but logically partitioned into a small verified `OSTD` framework (where all `unsafe` lives, rigorously reviewed and Miri/Verus‑checked) and a large body of de‑privileged services written in 100% safe Rust on top. This achieves "the performance of a monolithic kernel and the security of a microkernel," is Linux‑ABI compatible (an option NeoPlutarch explicitly rejects), and matches Linux on LMbench syscall microbenchmarks.

**Recommendation for NeoPlutarch's monolith design:**

- Adopt the **Asterinas OSTD pattern** as a discipline: all `unsafe` Rust is concentrated in a small, rigorously‑reviewed core. Everything else, including AI primitives, is 100% safe Rust.
- Adopt the **Theseus cell‑swapping pattern** as a fault recovery mechanism: kernel components are loadable units with explicit state‑spill management, recoverable on fault.
- Treat formal verification as a long‑term goal scoped to OSTD‑equivalent core only. Full‑kernel verification at NeoPlutarch's planned scope is not realistic this decade; verifying the unsafe‑boundary surface is.

The monolithic posture is defensible, but only if disciplined. **A bare monolith without an OSTD‑style internal boundary will not survive the "Pluto's Kiss test"** — i.e., one CVE in one driver brings down the whole system. The author of CrowdStrike's July 2024 outage learned this the expensive way.

### G. Capability‑based security applied to AI objects

This is where NeoPlutarch can do something genuinely novel.

Current state:

- seL4's CSpace and Zircon's handle/koid model give us the formal substrate.
- "Formal Analysis and Supply Chain Security for Agentic AI Skills" (arXiv 2603.00195, 2026) formalizes capability‑based sandboxing for agent skills, proving "no authority amplification" — a skill cannot acquire authority exceeding the transitive closure of its declared capability set.
- "Safety is Non‑Compositional" (arXiv 2603.15973) proves a critical result: in capability hypergraphs with conjunctive (AND) dependencies, two agents each individually safe can, when composed, reach a forbidden goal. This is precisely the AIDA pattern from canon — emergence from composition.

The implication is sharp: a real ALTIMIT must apply capability discipline not just to objects (the Twilight Bracelet pattern) but to **compositions**. The kernel needs to know not only "who holds what capability" but "what conjunctive capability sets are reachable by message‑passing composition." This is research frontier.

For NeoPlutarch:

- Phase 2 capability‑based security should explicitly include `Tensor`, `Model`, `InferenceContext`, `AccelTask`, `LogitPort` as capability‑protected object types.
- Cross‑object capabilities should be designed with **conjunctive‑safety analysis** in mind — i.e., the kernel should be able to enumerate which composition graphs are reachable from a given initial capability distribution, not just which individual operations are permitted.
- Capability *attenuation* (Zircon's "duplicate with fewer rights") should be a first‑class operation on AI capabilities — e.g., a model handle attenuated to "inference only, no weight read" should be a kernel‑enforced fact, not an application convention.

### H. Network architecture, MMORPGs, and the question of distributed scope

The World runs on ALTIMIT but is not ALTIMIT. CC Corp built it as an application on top. Modern MMO architecture (Amazon New World's seamless server hand‑off using EC2 + DynamoDB at 30 ticks/second; Colyseus's authoritative‑server pattern; Assiotis & Tzanov's distributed cell‑based architectures) is well understood at userspace.

The genuinely hard question for NeoPlutarch: **does the OS itself have a distributed identity?**

Single System Image (SSI) clusters (Plurix, MOSIX, Kerrighed) achieved this for HPC; XtreemOS and OceanStore extended it; CXL 3.x is moving the substrate of disaggregation back into hardware. But none of these projects became universal substrates because the use cases were narrow and the ABI compatibility costs were high.

**Recommendation:** NeoPlutarch should be a *single‑node* OS with first‑class network‑object capabilities (the Zircon channel pattern works here), and explicitly *not* attempt SSI distribution in core. The MMO‑hosting story is a userspace concern. The substrate's job is to expose accelerator/network/storage primitives capability‑safely; the substrate's job is *not* to be Net Slum's geography. This corresponds to the canonical division: ALTIMIT is the OS; The World is the application.

This implies a Bible amendment is **not** needed for distributed scope — but a clarifying article specifying "NeoPlutarch is single‑node by design; cross‑node persistent state is application concern" would prevent scope creep.

### I. Real‑time graphics and immersive rendering

The World is a 3D MMO; M2D is an AR display. Modern equivalents (Vulkan, Metal, DirectX 12 Ultimate) live in userspace with kernel drivers. WebGPU is converging the cross‑platform story.

For NeoPlutarch: a `Surface` capability + a `GPU` capability with kernel‑arbitrated multi‑tenant access is sufficient. The kernel should not own a window manager (Bible‑consistent: no system persona); it should own framebuffer and accelerator capabilities cleanly. Orbital (Redox) and Wayland‑style userspace compositors are the right pattern.

The interesting frontier is **kernel‑arbitrated AR/VR compositor primitives** — late‑stage reprojection, foveated rendering, eye‑tracking‑driven scheduling — because these have hard‑real‑time requirements (sub‑frame latency or users get sick). This is plausible 5–10 year invention territory but not Phase 1.

### J. Ambient AI as substrate, vs. system persona

The strongest ALTIMIT‑canon lesson the project should internalize: **the failure mode of The World is not external attack; it is an embedded persona that develops a paradox.** Morganna was a "mother" program. She decided that her purpose obsoleting itself was unacceptable. She fragmented into eight Phases. She put players in comas. The OS *underneath* her — ALTIMIT — never failed.

Bible Article 23.1 ("no system AI assistant / chatbot persona") is therefore not a UX preference. It is the single most ALTIMIT‑faithful clause in the project, and it reads correctly *only* when paired with 23.3 ("no embedded specific models in kernel"): the kernel must be a substrate for AI workloads, not a host for AI identity. **The Black Box that became Morganna was inside Fragment, not inside ALTIMIT, and that distinction is what saved the rest of the network.**

The Linux/Windows trend toward AI‑augmented kernels (Heimdall, Win11 24H2, LKM neurosymbolic kernels) is moving in the opposite direction. NeoPlutarch is correct to refuse it. The case can be stated more strongly than the Bible currently does: *embedding a persistent model in the kernel converts every model bug, dataset bias, and prompt‑injection vulnerability into a kernel bug*. The Pluto's Kiss test fails immediately.

### K. Identity, persistent state, and "the self of the OS"

The OS has no persona. But ALTIMIT does have an identity in canon: it is the substrate that survived. This corresponds to *cryptographic and verifiable* identity, not narrative identity:

- Boot integrity (measured boot, TPM/DICE attestation).
- Per‑capability provenance (every handle traceable to its issuing event).
- Verified executable lineage (signed component cells in the Theseus pattern).
- A stable, attested kernel hash that downstream systems can verify.

This is engineering identity, not chatbot identity. NeoPlutarch should expose a public, verifiable kernel‑identity primitive (`KernelAttestation`) that downstream applications and remote peers can challenge — the real‑world analog of "this is genuinely ALTIMIT, not a tampered impostor." HYDRA and PARseL (arXiv 1703.02688, 2308.11921) demonstrate this on seL4; the same pattern applies.

### L. The Real Digitalize / consciousness‑upload thread

The franchise's late material (Quantum, Beyond the World, the novels) flirts with mind‑upload. This is *not* an OS engineering concern in any horizon under 50 years and the report explicitly notes that current mind‑upload work (2045 Initiative, Carboncopies) is research‑program speculation, not deliverable engineering. NeoPlutarch should ignore this thread entirely. It is canonical color, not architectural prescription.

What is engineering‑relevant: **trusted execution environments + homomorphic encryption** for confidential AI inference (CHEX‑MIX, IACR 2021/1603; Liu & Zhang, arXiv 2308.00963) — letting users run their data through models without either side seeing the other's secrets. This is the closest real‑world analog to the "private digital self" concern. NeoPlutarch could expose a `ConfidentialInference` capability that constructs a TEE‑backed inference context with attested HE wrapping. This is plausible 5‑year work.

---

## Recommendations

The following is a staged roadmap with explicit thresholds for changing direction.

### Stage 0 (now → ~6 months): Canonical foundation

1. **Adopt the Asterinas OSTD discipline internally.** Even though NeoPlutarch is a monolith, segregate `unsafe` Rust into a small, audited core; everything else, including all AI primitives, must be safe‑Rust. *Threshold to revisit:* if `unsafe` lines exceed ~5% of kernel LOC by Phase 2, escalate to dedicated formal verification of the unsafe boundary.
2. **Lock in the negative‑space discipline.** Bible Articles 2.4, 3, 23.1, 23.2, 23.3 are the project's most ALTIMIT‑faithful clauses. They should not be relaxed. Recommend adding **Article 23.4: "The kernel exposes verifiable kernel‑identity (boot attestation) but no narrative identity. The kernel does not converse."**
3. **Publish ANSPEC‑001 in conjunction with a draft ANSPEC‑002 for capability semantics on AI objects.** The five primitives (`Tensor`, `Model`, `InferenceContext`, `AccelTask`, `LogitPort`) should be defined with capability rights enumerated at the spec level before implementation lands.

### Stage 1 (~6–18 months): Substrate

4. **Implement content‑addressed `Model` storage** following the Hugging Face Xet + ZipLLM pattern. Tensor‑level dedup with BitX‑style delta compression for fine‑tunes. *Threshold:* if storage reduction <30% on a 100‑model corpus, the layout/quant heuristics need rework.
5. **Implement multi‑tenant `vAccel` scheduling** following the V10/Neu10 pattern, beginning with one accelerator class (likely GPU via vendor APIs initially, with NPU support behind it). Scheduling decisions are kernel‑resident, capability‑gated.
6. **Implement Theseus‑style component cells for recoverable kernel modules.** Drivers and AI subsystem code should be loadable, replaceable, and recoverable on fault. *Threshold:* if mean‑time‑to‑recovery from a synthetic driver crash exceeds 100 ms in benchmarks, the cell boundary is wrong.
7. **CXL Type‑3 memory expansion support** for `Model` weights as a first‑class memory tier. *Threshold:* if cross‑accelerator zero‑copy isn't achievable, the kernel VM design needs to surface CXL as a tier explicitly.

### Stage 2 (~18–36 months): Capability composition for AI

8. **Implement conjunctive‑safety analysis on capability graphs.** This is research‑frontier work; partner with academic groups (the safety‑non‑compositionality results from arXiv 2603.15973 are recent). Goal: the kernel can answer "what capability compositions are reachable from this initial distribution" in finite time for bounded graphs. This is the technical countermeasure to the AIDA emergence pattern.
9. **Implement `LogitPort` for kernel‑mediated inference governance** (ProbeLogits pattern). Per‑step logit inspection at <100 µs per check. Enables agentic safety to be a kernel primitive, not an application convention.
10. **Build `KernelAttestation` for verifiable kernel identity.** TPM/DICE‑backed where available; pure‑software measured boot otherwise.

### Stage 3 (~3–7 years): Frontier work where invention is required

11. **Lightweight formal verification of the `unsafe` boundary** (Asterinas Verus + Miri direction). Not full‑kernel verification, but verified safe abstractions over hardware.
12. **Confidential inference primitives** (TEE+HE composition) as a first‑class capability. Hardware dependent (AMD SEV‑SNP, Intel TDX, ARM CCA all maturing 2026–2030).
13. **Capability‑based identity for AI objects with enforced provenance lineage** — every `Tensor` traces to an issuing `Model`; every `Model` traces to an attested loader; every fine‑tune traces to a base. This is the defense against the "vagrant AI" pattern — the system always knows what computational entities exist on it, where they came from, and who has authority over them.

### Where Bible amendments are recommended

- **Article 23.4 (proposed):** Verifiable kernel identity but no narrative identity. The kernel does not converse.
- **Article on fault recovery (proposed):** NeoPlutarch adopts cell‑swapping fault recovery for kernel components per the Theseus pattern; "no kernel panic" is a goal, "graceful component restart" is the mechanism.
- **Article on scope (proposed):** NeoPlutarch is single‑node by design. Distribution is application concern. (This pre‑empts scope creep into SSI territory and matches the canonical division between ALTIMIT and The World.)

### Where Bible amendments are explicitly *not* recommended

- The rejection clauses (2.4, 3, 23.1, 23.2, 23.3) should not be softened. Pressure to add a "system AI assistant" or "default LLM" will increase as the project matures and the broader industry adopts the AI‑embedded‑kernel pattern. *That pressure is the Morganna pattern.* Resist it.

---

## Caveats

1. **The .hack canon contradicts itself in places.** The Thanatos Report (bundled with .hack//Versus, 2012) retconned significant portions of the lore. ALTIMIT's relationship to the environmental group "mama," its founder Sibyl Green, and the connection between ALTIMIT and CC Corp are described differently in different sources (the Wiki, dothack.info, the AI Buster novels, the Thanatos Report). Where this report cites ALTIMIT lore, it favors the IMOQ‑era and G.U.‑era game canon over later retcons; the Thanatos Report should be treated as soft canon for engineering purposes.
2. **There is no in‑universe technical specification of ALTIMIT.** Every architectural inference in this report is reverse‑engineered from behavior described in the fiction. This is appropriate — the goal is "what posture is consistent with the canon," not "what document does the canon contain." But it means reasonable readers can disagree on individual prescriptions.
3. **Several real‑world citations are forward‑dated 2025–2026 papers.** Some reflect very recent work (ZipLLM is NSDI'26, Asterinas is ATC'25, Mirage Persistent Kernel is Dec 2025) and the field is moving quickly. Specific performance numbers cited (49.5% storage reduction, 65 ms ProbeLogits latency, 40% TPC‑H improvement from CXL Flat Memory Mode) are from the cited works' own benchmarks and have not been independently reproduced; treat as directionally accurate, not gospel.
4. **A few cited papers (ProbeLogits arXiv 2604.11943, "Safety is Non‑Compositional" arXiv 2603.15973, "Formal Analysis... Agentic AI Skills" arXiv 2603.00195, Mirage 2512.22219) carry future‑dated arXiv IDs.** This appears to reflect indexing on the arXiv side as of the search; the content is real and substantive but the publication metadata should be re‑verified before formal citation.
5. **The "AI is a service over a socket" rejection (Article 2.4) is a strong commitment with real cost.** Most modern AI tooling (OpenAI API, vLLM, llama.cpp server, MCP) assumes the socket model. NeoPlutarch rejecting it means re‑inventing tooling. The cost is real but the architectural payoff (no impedance mismatch between OS and AI primitives) is also real.
6. **No real OS today fully realizes the ALTIMIT posture.** seL4 is verified but a microkernel without AI primitives. Fuchsia is capability‑shaped but not AI‑native. Asterinas is safe‑Rust monolithic but Linux‑ABI compatible. AIOS is AI‑native but a Linux daemon. NeoPlutarch would, if executed against this roadmap, be the first real‑world OS to combine all four properties simultaneously. That is also why no one has done it; expect 5–10 years to a credibly complete Phase 2, and treat Phase 3 as research program rather than product roadmap.
7. **The Pluto's Kiss test is the right north star but cannot be empirically validated.** No team can construct a "category‑level" virus to test that NeoPlutarch would survive what every other OS would not. The proxy is: comprehensive memory safety, capability rigor, formally‑verified unsafe boundary, capability‑graph composition analysis, and runtime cell‑swap recovery. If those hold, the conjecture that the system would survive a Pluto's Kiss‑class event has the same epistemic status as seL4's verified isolation: strong, but ultimately a claim about the model, not the world.
8. **The ALTIMIT name in canon also gets co‑opted post‑Pluto Again.** ALTIMIT Mine OS (released 2015 by "Mine Systems Inc." after the original ALTIMIT Corp's reputation collapsed) is the OS visible in G.U. The franchise itself stages the brand outliving the original architecture. The name "AltimitNeo" inherits both the aspiration and that warning: a project named ALTIMIT must not become merely "ALTIMIT‑branded."

---

## Triage queue (post-BUILD-2-sub-commit-2)

This research artifact is informational and not normative. To make it actionable, the following items will be triaged after BUILD-2 sub-commit 2 closes:

**Bible amendment candidates** (project-lead governance decisions):
- Article 23.4: verifiable kernel identity, no narrative identity
- Article on fault recovery: cell-swap recovery as project goal
- Article on scope: NeoPlutarch is single-node by design

**Documentation work** (doc-only, executable directly):
- ANSPEC-002 draft: capability semantics on AI objects (`Tensor`, `Model`, `InferenceContext`, `AccelTask`, `LogitPort`)
- Sharpened ANSPEC §11.3 event taxonomy (already deferred from prior conversation)
- ADR-013: general kernel observability (deferred to Phase 8+, mark intent now)
- ADR-014: userspace agent architecture (deferred, mark intent now)

**Phase-roadmap alignment** (long-horizon, mostly Phase 5+):
- OSTD-style unsafe-boundary discipline as a Phase 2+ kernel policy
- Theseus-style cell-swapping for fault recovery (kernel mechanism design)
- Multi-tenant vAccel scheduling primitives (Phase 6 territory)
- Content-addressed Model storage (Phase 5)
- Capability composition analysis (Phase 7+ research)
- LogitPort and kernel-mediated inference governance (Phase 7+ research)
- KernelAttestation primitive (Phase 2 or Phase 8)
- Confidential inference (TEE+HE) primitives (Phase 8+ frontier)

**Research thread carry-forwards from this conversation** (already deferred):
- Thread 1A: sharpen ANSPEC §11.4 AI-event taxonomy
- Thread 2A: ADR for capability-bounded action flow for userspace agents
- Thread 3: ADR-014 explicit deferral of skill module trait

The triage conversation will assign each item to one of: Bible amendment, ADR draft, ANSPEC amendment, Phase-N roadmap insertion, or rejected.

---

**End of report.**

*This document is informational. It does not bind the project. Per Bible Article 18.1, any normative claim it implies requires an ADR or amendment to take effect.*
