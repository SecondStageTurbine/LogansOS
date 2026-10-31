# NeoPlutarch — Design Vision v0.4

> *"Built to survive the next Pluto's Kiss. Built to think alongside you."*

A from-scratch operating system written in Rust, inspired in spirit by the ALTIMIT OS of the .hack universe. Not a Linux fork. Not a reskin. A real OS, built from the bootloader up, with **AI as a first-class kernel primitive** — not an app, not a service you can uninstall, not a sidebar.

---

## About This Document

This is the **vision document** for NeoPlutarch. It describes what NEO is, what it feels like to use, and why it exists. It is not the architecture specification.

The authoritative architectural references are:

- **The NeoPlutarch Bible** (`docs/bible.md`) — the project's constitution. Defines kernel architecture, syscall conventions, security model, phase plan, and engineering discipline.
- **ANSPEC-001** (`docs/specs/neoplutarch-ai-subsystem-spec.md`) — the AI/ML subsystem design specification. Defines tensors, models, inference contexts, the syscall surface, the inference scheduler, and the accelerator driver interface.
- **ADRs** (`docs/ADR-NNN-*.md`) — architecture decision records for individual choices.

Where this document and any of the above disagree, **the others win**. This document is descriptive of intent; they are normative.

If you are a contributor and want to know *what to build*, read the Bible and ANSPEC-001. Read this document to know *why we are building it*.

---

## 1. Identity

| | |
|---|---|
| **Name** | NeoPlutarch |
| **Codename (internal)** | `neoplutarch`, `neo` |
| **Tagline** | *"Intelligence as a kernel primitive."* |
| **Version scheme** | `0.MAJOR.MINOR` until first bare-metal boot, then `1.x` |
| **License** | `MIT OR Apache-2.0` per Bible Article 26 and ADR-009 |

### Design pillars

1. **Monolithic kernel, AI-native.** The kernel handles memory, scheduling, and inference as first-class primitives. AI is not a service over a socket; it is a syscall family. *(Bible Article 4.1, 2.4, 6)*
2. **Capabilities, not permissions.** No ambient authority. Every kernel object — tensor, model, inference handle, file — is referenced by a capability handle. *(Bible Article 12.1)*
3. **AI as a kernel-blessed primitive.** Inference is a syscall. Models are typed filesystem objects. Tensors are kernel objects with their own allocation class. *(Bible Article 2, 6; ANSPEC-001 §3–5)*
4. **Local-first cognition.** NEO never phones home. Inference runs on the user's hardware on the user's data and stays there, unless the user explicitly routes a request to a cloud provider through an application that holds the appropriate capability. *(Bible Article 23.2)*
5. **Real engineering, every commit.** No toy code, no throwaway shortcuts, no "fix it later" hacks. Every commit is intended to ship. *(Bible Article 1.3)*

### Aesthetic identity

Modern cyberpunk reimagining of the .hack ALTIMIT visual language. Dark backgrounds, electric-blue and silver accents (magenta reserved for fatal alerts), monospace-heavy typography, deliberate "data feed" motifs. The boot sequence is part of the OS's identity, not an afterthought — every boot looks like jacking in.

---

## 2. What NEO Feels Like

This section is the heart of the document. Architecture is in the Bible; this is the experience.

### 2.1 You boot, and the system is already thinking

You power on. Limine hands off to the kernel. The framebuffer comes up in NEO's signature flat-neon palette. Boot messages scroll past in monospace, each one a real event:

```
[ 0.000 ] neoplutarch v0.5.0 — kernel online
[ 0.012 ] mm: 64 GiB physical, 48-bit canonical, kaslr=enabled
[ 0.024 ] sched: 5 priority classes, 16 cores online
[ 0.041 ] ai: tensor allocator ready (host: 16 GiB reserved)
[ 0.058 ] ai: accelerators enumerated — gpu0 (24 GiB), gpu1 (24 GiB), npu0 (8 GiB)
[ 0.094 ] models: 7 .neom files registered in /sys/models
[ 0.108 ] models: warming qwen3-coder-80b on gpu0,gpu1 (dedup hit, 0 copies)
[ 0.221 ] inference: scheduler ready
[ 0.234 ] intent: classifier loaded on npu0
[ 0.241 ] the world is listening.
```

The last line is intentional. It's the moment NEO declares itself awake.

By the time the desktop renders, the model you'll use first is already resident in VRAM. Not because the OS guessed — because `neo-init` profiled your usage locally and pre-warmed it. Open NEO at 9am on a Monday and the coding model is hot. Open it at 9pm and the writing model is hot. You never wait for cognition to load.

### 2.2 You ask, and the system answers

`neo-shell` doesn't choose between command line and chat. It does both:

```
neo:~$ ls -la
[standard ls output]

neo:~$ find the report I was editing yesterday
> Searched edit history for documents modified 2026-04-29.
> 1 match: ~/work/Q1-roadmap.md (last edited 17:42)
> Open it? [y/N/explain]

neo:~$ explain
> I matched on:
>   - intent: "find the report"
>   - constraint: "yesterday" -> 2026-04-29
>   - source: NeoFS edit-history index
>   - filter: file.role == "report"
> Equivalent shell command:
>   neofs-query --modified 2026-04-29 --role report
```

Every conversational invocation shows what it did and the equivalent command. Bash with a brain that teaches you bash. The shell itself is an ordinary userspace program; it calls the kernel's inference syscalls the same way `cat` calls `read`.

### 2.3 Apps know what they're for, and the compositor listens

`neo-compositor` understands what windows *contain*, not just where they sit.

Apps declare a window-intent manifest: `{ kind: "draft-email", recipient: "...", subject: "..." }`, or `{ kind: "code-edit", language: "rust", project: "..." }`, or `{ kind: "reading", source: "..." }`. The compositor uses these to suggest tiling groups, surface related windows, and answer "what am I working on?" without screen-scraping. Privacy is structural: only what apps declare is visible.

Apps that don't declare intent still work. They just don't get the system's attention.

### 2.4 The Intent Bus makes NEO feel psychic, not just responsive

The Intent Bus is a publish/subscribe fabric for AI-derived events. **It is a userspace application built on top of the kernel's AI syscalls** — not a kernel subsystem. It runs as `neo-intent`, an ordinary process that holds capabilities for a small classifier model and a curated set of event sources.

- **Producers.** A small always-on classifier (running on the NPU or smallest available accelerator) watches redacted, capability-gated event streams: focused window, recent text input, recent file access. It emits structured intent events: `UserStartingEmail`, `UserSearchingForFile`, `UserAskingQuestion`, `UserResearchingTopic { topic }`.
- **Consumers.** Any process holding `CAP_INTENT_SUBSCRIBE` can subscribe. The mail app subscribes to `UserStartingEmail`. The shell subscribes to `UserAskingQuestion`.
- **Capability-gated, audited, visible.** Every active subscription appears in the status bar. The user can revoke any subscription instantly from the privacy panel.
- **Local-only by structure.** Intent events never leave the machine. The `neo-intent` process does not hold any capability that would let it route anywhere off-device.
- **Optional, not core.** The kernel does not depend on the Intent Bus. NEO boots and runs without it. But applications that subscribe become noticeably more alive — they react to what you're doing rather than waiting to be told.

The Intent Bus is a vision feature, scheduled for Phase 8+ in the application layer. The kernel substrate it depends on (inference syscalls, capability passing, event delivery) is on the Phase 5–7 critical path.

### 2.5 Privacy is structural, not promised

NEO is **local-first, cloud-optional** with structural enforcement.

- **Default routing: local.** Inference syscalls dispatch to local accelerators. The kernel has no concept of a cloud endpoint.
- **Cloud is an application-layer feature.** A userspace app that wants to use Anthropic, OpenAI, or any other remote model implements its own networking and presents that to the user as what it is — an outbound network connection. The kernel does not pretend a cloud call is "inference."
- **Cloud-routing apps require explicit capabilities.** An app that connects to a cloud provider holds `CAP_NETWORK` and whatever capability gates the user's API credentials. The user grants these per-app, visible in the privacy panel.
- **The privacy panel is always reachable.** `neo-privacy` shows: every active inference handle, every active intent subscription, every app holding network capabilities, every cloud request in the last N hours with timestamp/app/destination/payload-hash. The user can revoke any capability instantly.
- **No telemetry. Ever.** NEO itself never phones home. Updates are pull-based, signed, and explicitly initiated by the user.

This works because the kernel doesn't conflate "inference" with "network." A `sys_inf_run` call cannot become a cloud round-trip — those code paths don't exist in the kernel. If an app wants cloud cognition, it makes a network syscall and a parsing decision, and the user sees both.

### 2.6 Models are first-class objects

Models live on disk as `.neom` files (Bible Article 6.2, ANSPEC-001 §4.1). NeoFS tags them at the inode level — the kernel knows a `.neom` is a model and refuses to `mmap` it as executable code (Bible Article 10.3). Userspace tools (`neo-models`, `neo-import`) manage them:

```
$ neo-models list
NAME                    ROLE        DEVICE      SIZE     STATUS
qwen3-coder-80b         coding      gpu0,gpu1   42 GB    resident
llama-vision-13b        vision      gpu0        7 GB     cached
mistral-writer-22b      writing     -           12 GB    on-disk
intent-classifier-1b    classify    npu0        600 MB   resident

$ neo-models set-default --role writing mistral-writer-22b
$ neo-models load mistral-writer-22b
```

The kernel deduplicates weight tensors by content hash (ANSPEC-001 §8.2). Loading the same 70B model twice costs one copy. Loading a fine-tune of it costs one copy plus the diff (future optimization). The user sees this as: "models load instantly when something else has already loaded them."

Roles (`coding`, `writing`, `vision`, `classify`, etc.) are a userspace convention managed by `neo-models`, not a kernel concept. The kernel sees only `.neom` handles and capability grants.

### 2.7 Crashes are isolated, but the kernel is solid

Drivers other than the accelerator class are kept minimal in the kernel and the rest of the system — compositor, shell, userspace daemons including `neo-intent` — runs as ordinary processes. When a userspace process dies, `neo-init` reaps it; the system continues. The kernel itself is held to a higher bar: no `unwrap()` in steady-state paths, every `unsafe` block annotated with a `// SAFETY:` comment, formal review for anything touching MMIO or page tables (Bible Article 15).

The kernel does not aim to be unkillable through cleverness. It aims to be unkillable through correctness.

---

## 3. The Inference Experience

From an application's perspective, calling on intelligence looks like this:

```rust
// libneo userspace API, Phase 5+
use libneo::ai::{Model, InferRequest, Privacy};

let model = Model::open_by_role("reasoning")?;     // resolved via neo-models
let stream = model.infer(InferRequest {
    prompt: "summarize the document in this window",
    context: window_handle.intent_context(),
    privacy: Privacy::LocalOnly,
    deadline: Some(Duration::from_millis(800)),
})?;

for token in stream {
    print!("{}", token?);
}
```

Underneath, `Model::open_by_role` resolves to a `.neom` path via `neo-models` (userspace registry), then issues `sys_model_load` (kernel syscall, ANSPEC-001 §6.3.1). `model.infer` issues `sys_inf_stream_open` (ANSPEC-001 §5.3). Every token comes from the kernel's inference scheduler dispatching ops onto whichever accelerator the scheduler chose.

The app does not see the accelerator. The app does not see the backend. The app does not even see whether the model was already resident or had to be loaded from disk. It calls `infer`. Intelligence happens.

What the app *does* see, and what the user sees in the privacy panel:

- The capability handle the app is using (auditable: which model, which permissions).
- The deadline behavior (met or missed).
- The compute used (per ANSPEC-001 §5.5 `InfStats`).

This is the level of transparency NEO is built for: applications get cognition the same way they get heap allocation, and users get receipts.

---

## 4. The Conversational Shell

`neo-shell` is the demonstration that NEO's AI primitives are real, not a feature shelf.

It supports:

- **Standard shell operations.** Pipes, redirection, job control, history. POSIX-flavored where it doesn't conflict with Bible Article 1.4 (we are allowed to disagree with POSIX; we are not allowed to be gratuitously different).
- **Conversational invocation.** Natural-language commands resolve to either a direct shell equivalent (shown as `explain` output) or to a tool-using inference call against a coding-role model.
- **Receipts.** Every conversational call shows what it did, the equivalent command, and the source of any data it pulled in.
- **Per-conversation state.** Conversation handles persist across shell sessions if the user opts in (`neo-shell --resume`).

It does not:

- Run a chatbot persona that "personifies" the OS. NEO is not your friend; it is your tool. (Bible Article 23.1.)
- Fall back silently to cloud inference. If a coding model isn't available locally, it says so.
- Hide what it's doing. `explain` always works.

---

## 5. The Boot Sequence

The boot sequence is part of the OS's identity. Per Bible Article 1.3, every commit is intended to ship — including the cosmetic ones.

The current Phase 2 boot frame ends with the framebuffer wordmark + tagline byline and serial transcript. The Phase 5+ target boot frame is what §2.1 above describes: kernel online, AI substrate ready, models warming, intent classifier loaded, "the world is listening."

Boot timing is a tracked metric from Phase 2 onward (Bible Article 16.4). The aesthetic is not allowed to extend boot time meaningfully. We earn the visual by being fast.

---

## 6. What NEO Is Not

To keep this document honest:

- **Not a Linux distribution.** Not a fork, not a reskin, not POSIX-compatible by default. (Bible Article 3.)
- **Not a research microkernel.** We are building a system to use, not a paper to cite. (Bible Article 3.)
- **Not an AI assistant OS.** No built-in chatbot persona, no "Cortana for NEO." Applications can build assistants on top. (Bible Article 23.1.)
- **Not a cloud-only OS.** We target bare metal. Virtualization is a guest scenario, not a host one. (Bible Article 3.)
- **Not telemetry-bearing.** NEO never reports usage, never trains on user data, never phones home. (Bible Article 23.2.)
- **Not a training platform.** The kernel provides inference primitives. Training, if ever supported, is a separate subsystem. (ANSPEC-001 §1.3.)

---

## 7. Roadmap (Vision-Layer View)

The authoritative phase ledger is in Bible Article 13. This section maps phases to user-visible milestones.

### Phase 0–1.5: The Substrate (complete)

The kernel boots, schedules threads, runs userspace processes, services syscalls. No AI yet. The user sees a framebuffer logo and a serial log. This is the foundation; it is not the product.

**User-visible moment:** *the kernel boots, the logo renders, and the serial log shows a real operating system, not a hello-world.*

### Phase 2: Real Userland

Multi-process, capabilities, SMP, real allocators. `neo-shell` exists as a text shell with no AI. NeoFS lands. Users can write small Rust programs and run them.

**User-visible moment:** *I can ssh-equivalent into NEO, write a program, run it, see output. NEO is a usable Unix-shaped thing — except it isn't Unix.*

### Phase 3: The Filesystem Knows

NeoFS is content-addressed. `.neom` files are typed at the inode level. Block-level dedup means model files cost one copy on disk no matter how many places they appear.

**User-visible moment:** *I copy a 40 GB model file into three different directories. `df` says my disk usage went up by 40 GB total, not 120.*

### Phase 4: Networking

In-kernel network stack. IPv6 first. Sockets-equivalent (not BSD sockets). Userspace gets `connect()`-shaped APIs through `libneo::net`.

**User-visible moment:** *I can fetch a webpage. NEO is online.*

### Phase 5: First Light (the AI subsystem foundation)

The tensor primitive. The model object. CPU inference backend. `.neom` files load. A tiny MLP runs end-to-end. (ANSPEC-001 §12.1.)

**User-visible moment:** *I run `neo-infer` against a small model and get a real output. The kernel scheduled the ops. Nothing in userspace did the math except the CPU backend, which is kernel code anyway.*

### Phase 6: Accelerators

`virtio-gpu-compute` first (in QEMU), then one real GPU. Tensor migration between CPU and accelerator. Async inference syscalls. (ANSPEC-001 §12.2.)

**User-visible moment:** *I run a real LLM. Tokens stream. The GPU is doing the work. The kernel is dispatching.*

### Phase 7: The Inference Scheduler

Multi-tenant, priority-class, deadline-aware. Two apps doing inference on the same GPU without stepping on each other. Realtime-priority small models meeting their deadlines under contention. Deduplication landed. Models shareable across processes. (ANSPEC-001 §12.3.)

**User-visible moment:** *I'm running a chat in one window and a code completion in another and a real-time wake-word detector in the background, all on the same GPU. The wake-word never misses. The chat doesn't stutter.*

### Phase 8+: Userland Bloom

Window system. Compositor with intent manifests. The Intent Bus as a userspace daemon. The privacy panel. The conversational shell at full power. The first GUI apps.

**User-visible moment:** *NEO feels alive. Open it at 9am, the coding model is warm. Click on an email, the mail app is already drafting because intent told it. The world is listening.*

### Future Compatibility: Linux First, Windows Next

ADR-023 puts the Linux personality on the roadmap first. That path gives NeoPlutarch a practical compatibility bridge for developer tooling, POSIX-shaped software, and Windows games through Wine/Proton once the graphics/audio/input stack exists.

The long-horizon product roadmap now also includes a Windows personality as a strategic target. It is not current-phase work and it does not change the native ABI. It should become its own ADR/specification after the Linux personality and graphics foundation have enough shape to make the Windows compatibility boundary concrete.

The design rule is simple: compatibility at the boundary, NeoPlutarch in the core. Linux and Windows personalities may coexist, but native scheduler, memory, VFS, networking, graphics/accelerator, and capability/security services remain NeoPlutarch-shaped.

---

## 8. Open Questions

These are vision-layer questions; architectural questions are tracked in ANSPEC-001 §13 and per-phase ADRs.

- **Does NEO ship with default models?** A built-in default reasoning model would make the OS instantly usable, but locks us into a model-vendor relationship. Lean: ship with no default; first-boot wizard offers to download user-selected models.
- **What does the privacy panel look like, exactly?** A daemon? A compositor overlay? A status-bar applet? Probably all three with one source of truth.
- **How does conversation state persist across reboots?** KV-cache serialization is a real engineering question. Probably Phase 8+.
- **Is there a NEO app store?** If yes, what does signing look like? If no, how do users discover apps? Lean: no store, package format with signature support, discovery is a community problem.
- **Does NEO have a window manifest *language*, or just a struct?** Probably a struct with a registered enum of `kind` values, extensible by ADR.

---

## 9. References

- NeoPlutarch Bible (`docs/bible.md`) — the project's constitution.
- ANSPEC-001 (`docs/specs/neoplutarch-ai-subsystem-spec.md`) — AI subsystem design specification.
- Limine — https://limine-bootloader.org
- Phil Opp, *Writing an OS in Rust* — https://os.phil-opp.com
- The .hack universe and the original ALTIMIT OS — for the spirit, not the architecture.

---

*v0.4 — Identity update for the 2026-05-10 project rename (AltimitNeo → NeoPlutarch). Tagline canonicalized to "Intelligence as a kernel primitive." Vision/aesthetic narrative preserved from v0.3. Referential mentions of `.hack` and original ALTIMIT OS preserved as design-inspiration homage. v0.3 surface preserved in git history; structural carryover from v0.3 to v0.4 is otherwise identity-only.*

**Document end.**
