<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: The GUI vision — "The Atrium" (Cerebro's independent vision)

**Date:** 2026-07-08
**Author:** Cerebro (main-tier coder)
**Source:** Plans/DISPATCH-2026-07-08-gui-vision-think-cerebro-and-jarvis.md
**Branch:** codex/gui-vision-think-cerebro
**Independence note:** This is one of two independent visions (the judge-panel pattern). I have NOT read Jarvis's THINK. This is my take, committed to a single paradigm on purpose — a vision that agrees with everything is a wasted pair.

**Inputs (read, not summarized):** Bible Article 1.5 (usability / five-year-old / conversation-first) + 1.6 (beauty as craft / grammar-not-uniformity / the Tahoe cautionary tale), `docs/bible.md:21-32`; OT-52 (our own cap-native compositor), OT-45 (the visual design language), OT-46 (Arthur the understated-butler persona), OT-44 (apps = cap-bundles / WASM, the ReactOS compatibility-treadmill caution), OT-26 (the NL→Action→cap-check agentic loop), OT-18/OT-19 (grant ergonomics / reversibility-not-refusal), OT-22 (the multi-principal sequencing guard), `docs/OPEN-THREADS.md`; the shipped surfaces — the v1 transcript console + `neo-shell`/`neo-arthur` (Phase-8 Arthur arc-2, `607c1e3`→`2066ee4`); the two duality THINKs (`THINK-2026-07-07-usability-five-year-old-design.md` P1-P7/F1-F8, `THINK-2026-07-08-poweruser-neo-shell-duality-design.md` S1-S8/T1-T4); `docs/design-system.md` (ADR-024 v1.1, incl. §12 The Threshold). **External prior art** cited inline and in Sources — read from primary sources, not through a summarizing fetch (OT-1 discipline).

**Scope:** Vision THINK — research + design only. No code, no QEMU, host-only. Bold on the paradigm; rigorous and cited on the plumbing/latency; honest on the forks. The PM synthesizes both visions (+ PL) into the ratified direction.

---

## 0. The vision in one breath

**The Atrium: a single living surface where you *summon* rather than navigate, *hold* capabilities rather than beg permission of dialogs, and *scrub* time rather than lose it — a place you and Arthur co-inhabit, instant enough to feel like light on glass.**

Not another desktop. The desktop — windows, icons, menus, pointer — is a filing-cabinet metaphor invented at Xerox PARC in 1973 for a machine that genuinely worked like a filing cabinet. Ours does not. Polishing rounded rectangles is a game Apple has been winning for fifty years; we will not out-Apple Apple. The only way to build "the best damn GUI anyone has ever seen" is to build the interface **only this OS can build** — and that interface falls out of three things no one else has all three of at once: **Arthur is native** (a capable agent lives in the OS), **capabilities are the substrate** (every surface is an unforgeable, revocable thing you actually hold), and **we own the whole stack** (Rust kernel → our framebuffer → our accel path — no X11, no Wayland, no Mesa, no legacy). The Atrium is what those three principles look like when you make them beautiful and instant.

---

## 1. ISC-1 — THE PARADIGM (the soul of it)

### 1.1 What the Atrium *is*

The Atrium is one continuous, spatial, agent-co-inhabited surface. It has no wallpaper-and-icons "desktop," no taskbar, no Start menu, no window-manager chrome. It has three primitives and one axis:

- **Cap-surfaces** — the nouns. Every visible thing (the transcript, a `neo>` pane, an app, a file preview, a granted view of another principal's work) is a **cap-surface**: a capability-gated region of the screen that *is* an unforgeable capability (OT-52: "a surface IS a cap-gated framebuffer region"). A cap-surface is not a window a process happens to own; it is a thing you hold. You can see it, move it, hand it to Arthur or to another person, and revoke it with a gesture — because underneath, that is literally what a capability is.
- **Intent** — the primary verb. You express what you want — spoken, typed, or pointed — and the right cap-surface *appears where your attention is*. You do not go to a menu, hunt an app, or open a file dialog. This is the transcript's already-shipped "gulf of execution collapsed" (five-year-old THINK, P1: *intent is the interface*) lifted off the text line and given a place in space.
- **Arthur** — the co-inhabitant. Not a chat window in the corner; a presence that shares the room, can summon and arrange and operate cap-surfaces on your behalf within your grants (ADR-069 yolo-within-guardrails), and is *felt* through how the surface behaves rather than worn as a cartoon face (§6).
- **Time** — the axis. The transcript is already a scannable history; the Atrium makes that history the spine of the whole space. Everything that happened is a point you can scrub back to, because everything destructive left an undo artifact (OT-19). The past is not gone; it is *up-and-to-the-left*.

### 1.2 The four moves (the paradigm as a verb list)

Every legacy GUI is built on four verbs the Atrium replaces:

| Legacy desktop | The Atrium | Enabled by |
|---|---|---|
| **Navigate** (find the app, drill the menu, open the file dialog) | **Summon** — say/point what you want; the surface comes to you | Arthur-native (OT-26) |
| **Permit** (click through consent dialogs, UAC prompts, "allow?") | **Hold** — you see and manipulate the capabilities you have; granting is handing a thing over, revoking is a gesture | caps-as-substrate (OT-18) |
| **Lose** (destructive actions are a cliff; undo is a hidden Ctrl-Z that may or may not exist) | **Scrub** — time is a first-class axis; destructive is reversible-by-construction, and you can *feel* it | reversibility (OT-19) |
| **Wait** (the beachball, the jank, the 100–200 ms of modern input lag) | **Instant** — input-to-photon budgeted like a real-time system, because we own the compositor and the scheduler | own-the-stack (Phase-6 + OT-52) |

**Summon, not navigate. Hold, not permit. Scrub, not lose. Instant, not eventually.** That quartet is the paradigm. Each move is impossible-to-do-honestly on a legacy stack and *native* on ours.

### 1.3 How the shipped surfaces grow into it (not a terminal-with-a-desktop)

The danger the dispatch names explicitly: the transcript and `neo>` grow into "a terminal with a desktop bolted on." Here is how they grow into a *world* instead.

- **The transcript is the spine, not a scrollback.** Today it is text scrolling up a framebuffer. In the Atrium it becomes the **timeline of the place** — a vertical river of what happened, where each turn can *bloom* into a live cap-surface. Arthur says "I found three files"; the three files are not three lines of text, they are three recognizable cap-surfaces you can pick from by pointing or by saying "the second one" (recognition-over-recall, five-year-old THINK P4, made spatial). The transcript stays the status display (P2: state is narrated) and the recognition surface (P4) — it just stops being flat.
- **`neo>` stays exactly what it is — and gains a body.** The power-user THINK's whole argument is that `neo>` must be deterministic, predictable, composable, never-adaptive (S1–S8). The Atrium does not touch that contract. `neo>` remains a precise deterministic shell; it simply lives as a first-class cap-surface you can summon anywhere ("drop to neo>"), and its structured output (S5) can *render* — `neo> ls` can return a row of cap-surfaces instead of a wall of text, still machine-parseable underneath, still pipeable. The duality (arthur> for "tell me what you want," neo> for "do this exactly") is preserved verbatim; the Atrium is the shared space both surfaces render into (T4: two-views-one-substrate, now literally one visual substrate).
- **The desktop is never the primary path.** Bible 1.5 is law: "a traditional GUI, where present, is a convenience layered on top, never the primary path." The Atrium honors this by making the *spatial surface itself* intent-summoned. There is no grid of icons you must learn to navigate; the space is populated by what you and Arthur summon, and it collapses back to the transcript-and-a-prompt when idle. A five-year-old boots in and sees a warm, near-empty room and a line that says, in effect, *tell me what you'd like.* A power user boots into the same room and types `neo>`.

### 1.4 Why "Atrium"

An atrium is the warm, light-filled central room of a home — the place you actually live in, open to the rest of the house. The PL's frame is *"it's where I would live when I boot up our OS."* The name is a design brief in one word: warm not corporate, a place not an appliance, lived-in not administered. It is the emotional core the five-year-old THINK keeps circling (the owner-lives-here feeling) given a shape.

---

## 2. ISC-2 — BEAUTY (the design language)

### 2.1 The position I'm taking on our aesthetic (a real fork, resolved)

There is a genuine tension in what's already banked, and a vision that ignores it is dishonest. `design-system.md` (ADR-024) commits to a **cyberpunk, neon-on-dark, information-dense** identity — built for the Phase-8 inference *dashboard*, where showing CPU/GPU/tensor-store metrics is the point. Bible 1.6 (banked later, PL 2026-07-05) sets the bar at **classic-Mac-grade craft** — restraint, timelessness, distinctness. These pull in different directions: cyberpunk maximalism vs Mac minimalism.

**My resolution: the craft *bar* is classic-Mac; the *character* is honest to what the OS is; and neon is used as light and meaning, never decoration.** The system already contains the proof that this reconciles — **§12 The Threshold**. The login greeter shows intelligence as *ambient light and weather, not a personality* — a sparse generative substrate-field, a single frosted-glass pane backlit *by* the field, breath-cycle motion, and the **Ignite** gesture where the field blooms and resolves into the working desktop: "one substrate waking up, not two clashing styles." That is exactly the meld we need at the scale of the whole GUI. The Atrium generalizes the Threshold: the serene, art-first, light-as-intelligence register is the *resting* state and the connective tissue; the dense, high-signal readouts are what a cap-surface looks like *when you ask it to report*. Beauty at rest, density on demand — the same substrate at two energies.

(One correction the vision must make: §12 justifies "no persona" by citing Article 23.1 — but 23.1 was **reversed** on 2026-06-14 when Arthur became constitutional (Article 29). The Threshold's instinct is still right — *intelligence as weather, not a cartoon face* — but it now means "Arthur is **felt**, not avatared," not "there is no assistant." I carry the instinct, drop the dead justification. See §6.)

### 2.2 Grammar, not uniformity (1.6 made concrete)

Bible 1.6's operative rule, learned from the macOS Tahoe / Liquid-Glass episode where Apple forced every icon into one rounded rectangle and made a once-beautiful system **blander and harder to recognize**: *distinct silhouettes are recognition, and enforced sameness is anti-beauty AND anti-usability.* The Atrium's design language is therefore a **grammar** — a shared physics — that guarantees coherence while *mandating* per-thing identity:

- **Shared grammar (the physics every surface obeys):** one light model (a single implied light source; Electric Blue is *light*, not fill — §12's discipline), one material family (luminous, backlit, frosted-depth-where-earned), one motion physics (springs, breath, bloom/recoil — §2.4), one spatial rhythm (the 8px grid already in the design system), one type system (§2.5).
- **Mandated distinctness (what the grammar must never erase):** every cap-surface *kind* has its own silhouette and material signature. The transcript does not look like a `neo>` pane does not look like a file preview does not look like an untrusted app. A five-year-old "reads shape and color before words" (1.6) — so shape and color must carry identity, not be sanded flat for tidiness. Distinctness is the five-year-old test applied visually; uniformity-for-neatness is the false trade 1.6 rejects.

### 2.3 The idea that makes our beauty *ours*: materials carry capability

This is the fusion of ISC-2 and ISC-1, and I think it is the single most beautiful thing only this OS can do. **The design language's deepest job is to make capability legible and reversibility felt.** Because a surface *is* a capability, its material can honestly express its capability state — something no other OS's chrome can do, because no other OS's chrome corresponds to anything real underneath:

- A surface you **fully own** is rooted and solid — it sits *in* the space, casting and catching light like the Threshold's auth pane.
- A surface you've been **granted, revocably** (Arthur borrowing the work drive; another principal sharing a view) shows a visible **tether** — a luminous leash to its owner — and a lighter, more provisional material. You can *see* it is on loan, and cutting the tether (a gesture) is the revoke (OT-18 D6 revoke-through-the-derivation-graph, made physical).
- A surface from an **untrusted app** (an OT-44 WASM cap-bundle) is visibly *held at arm's length* — a distinct material that reads "this is bounded," the "safe to install by construction" posture made visible. The five-year-old and the power user both see the trust boundary without a dialog ever appearing.
- A **destructive** action does not throw a red modal (§12: rejection is a recoil, not a red flash). It leaves a visible undo-artifact in the timeline — the "set aside, recoverable this session" of VISION Example 2, given a place you can point at.

Security, in every other GUI, is the ugliest part — nagging dialogs, consent theater, UAC. Here it is the *substrate*, so the interface can make it the most beautiful part: the honest, glowing, directly-manipulable structure of what you can do. That is the anti-consent-theater payoff of caps-as-substrate, cashed out as beauty.

### 2.4 Motion: everything is a live spring, nothing is a canned transition

The proven model to steal is **Apple Core Animation**: a retained-mode layer tree where you set a *target* value and the framework animates to it, at frame rate, on a separate thread, with a **model tree** (where you're going) and a **presentation tree** (where you visibly are right now) (Apple, *Core Animation Programming Guide* / *Core Animation Basics*). The crucial property is **interruptibility**: because the presentation tree holds in-flight values, a new intent mid-animation redirects from wherever the pixels currently are — you never wait out a transition, motion always yields to your next move. The Atrium adopts this wholesale. Concretely:

- **Bloom** — a surface coming into being gathers light and resolves (the Threshold Ignite generalized, ~600–900 ms eased). Summoning *feels* like the thing arriving, not a window popping.
- **Breath** — idle surfaces have a slow 4–6 s breath (Threshold §12.2); the room is alive, not frozen. Arthur working shows as a quickening of that breath (the v1 thinking-liveness indicator, evolved — §6).
- **Recoil** — "no" (a denied action, a wrong passphrase) is a contraction-and-cool, never a red flash. Beauty even in being told no (§12.5).
- **Redirect** — every motion is interruptible; the presentation-tree model guarantees your next intent is honored *now*, from the current pixels. This is also a latency property (§3): interruptibility is what makes a fast UI *feel* fast.

Motion is never decoration and never linear (§12.7: "no linear, nothing fast" for ambient; springs for interactive). It communicates state: becoming, thinking, denying, withdrawing.

### 2.5 Light, color, type — the specifics

- **Light & color:** Void (`#0a0e27`) base; Electric Blue (`#3c8ef4`) as the substrate's *light*, used semantically (the palette already assigns Lime=go, Amber=I/O, Magenta=fatal-only). The Atrium's restraint move vs raw cyberpunk: **negative space is mandatory** (§12: ≥70% near-Void at rest), and saturation is spent, not sprayed — neon marks *meaning* (a live inference, a warning, a fatal), calm structural Silver carries everything else. This is Rams' "as little design as possible" (Braun / *Ten Principles for Good Design* — good design is unobtrusive) meeting the substrate's honest glow.
- **Type as duality:** the arthur>/neo> duality becomes a **type duality**. Arthur's voice and the warm/human surfaces are set in a humanist face (IBM Plex Sans — §12.4 already uses it for the greeting, "so the threshold *feels* different from the working OS"); the precision surfaces (`neo>`, structured data, metrics) stay IBM Plex Mono. You can *feel* which register you're in before you read a word. Weight-not-size differentiation (design-system §2), line-height ≥ 1.4, WCAG-AAA contrast on body text (§9) — all inherited.
- **The craft references to hold ourselves to (cited, real):** classic Mac OS iconography (Susan Kare — distinct, legible silhouettes; the literal anti-Tahoe, and the thing 1.6 is nostalgic for); Aqua's restraint; the Big Sur→Tahoe materials arc *and its failure* (the cautionary tale we must not repeat); Apple visionOS materials (depth and vibrancy achieved with light and translucency, not drop-shadows); Dieter Rams / Braun (unobtrusive, long-lasting, "as little design as possible"). And our own §12 Threshold as the in-house proof that serene-and-dense can be one system.

Beauty is a first-class deliverable here, not a coat of paint: it is 1.5's usability made visible (1.6), and it is the thing that makes the owner want to live in the room.

---

## 3. ISC-3 — RESPONSIVENESS ("instant or it's not beautiful")

### 3.1 Why latency *is* beauty (not a separate axis)

1.6 says beauty is usability made visible. A surface that janks is unusable in that instant, and therefore ugly in that instant — the beach-balling Mac is not beautiful no matter how it's rendered. So the latency budget is not an engineering footnote under the design language; it is *part of* the design language. "Instant or it's not beautiful" is literally true.

The status quo is worse than people think. Dan Luu's 240–1000 fps measurements (*Computer latency: 1977–2017*, danluu.com/input-lag) found modern machines commonly sit at **100–200 ms** keypress-to-photon in a terminal — *worse* than a 1983 Apple IIe at ~30–50 ms. Decades of stacked abstraction (compositors, toolkits, GPU round-trips, driver layers) *regressed* input latency. And it matters below the numbers people assume: the Ng–Lepinski–Wigdor–Sanders–Dietz MSR study (*Designing for Low-Latency Direct-Touch Input*, UIST 2012) built a **1 ms** direct-touch testbed and showed users perceive and prefer latency reductions far below the tens-of-milliseconds range typical systems live at. The human floor is single-digit milliseconds; the industry ships two orders of magnitude worse. Owning the whole stack is the one position from which you can actually close that gap.

### 3.2 The budget

- **Target: sub-frame input-to-photon on a 120 Hz-class panel.** One frame at 120 Hz is 8.33 ms; the goal is to land direct-manipulation input-to-photon inside a single frame, and drive toward single-digit milliseconds — using **predictive input** to mask the residual and **present-pacing** to guarantee the frame lands.
- **Never a dropped frame under load.** Frame pacing (Android's *Frame Pacing* library is the production precedent: measure CPU/GPU time, pick swap intervals, deactivate pipelining for ultra-fast frames — "lower, more predictable input-to-screen latency") run against vsync as a **real-time compositor task**. Because we own the scheduler and already run residency/QoS governance for inference deadlines (Phase-7 / OT-15), the *same* deadline machinery can guarantee the compositor's frame deadline. This is a real own-the-stack advantage: on Linux the compositor competes as an ordinary process; here it is a bounded RT task the kernel cooperates with.
- **Predictive input.** Android's `MotionPredictor` (Jetpack Input) reduces *perceived* latency by extrapolating the input path and feeding provisional points to the renderer, then reconciling with the real event. We adopt the same pattern for pointer/touch/stylus, honestly labeled: predicted pixels are provisional and replaced on the real sample. This is how you get *below* the physical pipeline latency perceptually.

### 3.3 Why our architecture can hit numbers Linux/Windows structurally can't

The honest version — not "we're magic," but "we deleted boundaries they can't delete":

- **No Mesa/DRM/KMS/GBM/EGL round-trip.** On Wayland, a single frame crosses: client renders → exports a `dma-buf` FD → `wl_surface` commit over the socket → compositor imports the dma-buf via EGL/Vulkan (`EGL_EXT_image_dma_buf_import`) → composites → KMS **atomic commit** → scanout (Wayland *Architecture*; wayland-book *dmabuf*; Paalanen, "What does EGL do in the Wayland stack"). Every arrow is a process/trust/driver boundary. Our cap-surface's buffer is **already shared by capability** — there is no FD-to-`wl_buffer` handshake, no second security model in userland, no Mesa. Composition is owned by a **thin trusted compositor** cooperating with the kernel on our Phase-6 accel path.
- **Input is already cap-routed in-kernel.** The USB-HID path is cap-gated today (OT-52); input events route to the focused cap-surface *by capability check*. The same mechanism that makes input secure also makes its path short — no X11-style global input server snooping and re-dispatching.
- **Zero-copy is structural, not negotiated.** Wayland's dma-buf path *is* zero-copy — I won't claim we invented that. Our win is that zero-copy is a *property of the capability*, not the outcome of a protocol dance across a process boundary, so there is less to do per frame and fewer places to stall.

Honest floor (ISC-7 fork): if the GPU/accel path isn't up (early boot, unsupported hardware), we degrade to a **software-composited Tier-0** exactly as the Threshold degrades (§12.6) — still beautiful, still responsive for text, *never* blocking input. "Best-effort, yields to input latency" (§12.6) is the whole-GUI rule, not just the greeter's.

---

## 4. ISC-4 — THE COMPOSITOR ARCHITECTURE (co-designed with the beauty)

### 4.1 The surface model (refining OT-52)

- **A surface is a cap-gated framebuffer region** — an unforgeable capability, not a process-owned window. Holding the surface capability *is* the right to draw there; there is no separate "window handle" to forge or leak.
- **Client-allocated buffers, compositor owns final composition.** Clients (apps, `neo>`, Arthur) render into their own buffers; the compositor owns the final blit to scanout — the Wayland client-allocation model, which is correct and I keep it. The difference is the buffer is shared by capability, not by passing a dma-buf FD.
- **Input focus is cap-routed.** Only the holder of a region's input capability receives its events. Focus routing *is* a capability check — which means input security and input routing are the same code path (see §3.3).
- **Retained-mode scene of surfaces.** The compositor keeps a retained layer tree (Core Animation's model, §2.4) so composition is cheap, animations run compositor-side on the presentation tree, and only **damaged** regions recomposite (damage tracking — the standard compositor practice Wayland/Weston and every modern compositor use). Retained-mode + damage tracking is the proven path to "never drop a frame."
- **The protocol is a `libneo`/cap ABI, not a socket wire protocol.** Surfaces, buffers, input, and damage are expressed as capability operations in our ABI — the same ABI apps already use for syscalls (OT-44 H1). No serialized wire protocol, no socket, no XML-defined message catalogue.

### 4.2 Steel-manning Wayland — and killing it honestly

The dispatch demands I steel-man the alternative and either adopt or kill it with reasons. Here is the strongest case for adopting **Wayland's protocol** (explicitly *not* its stack), and why I still build our own.

**The case FOR a Wayland protocol layer:** it is a mature, well-designed surface/buffer/input model with a decade of hardening, and — the real prize — an entire ecosystem (GTK, Qt, SDL, Chromium, every Linux app) already speaks it. A Wayland front-end would be the fastest possible path to a large existing app catalogue.

**Why I kill it as the native path (four reasons, in order of weight):**
1. **It reimplements, in userland, the security we get for free in the kernel.** Wayland's client isolation is a userland construct. We would be running *two* security models — the kernel capability system and Wayland's — with the seams between them as the attack surface. Our cap model is finer-grained (a *surface* is a capability, not a *client-process*), unforgeable, and kernel-enforced. Adopting Wayland's model *on top* is strictly worse.
2. **Impedance mismatch with our app story.** Wayland assumes POSIX-ish clients with sockets and file descriptors. Our apps are **WASM cap-bundles** (OT-44) with a `libneo`/cap ABI. A Wayland layer would be a translation shim bolted between two models that don't share primitives.
3. **It drags the desktop metaphor in as primary.** `xdg-shell` — the part of Wayland that makes it a *desktop* — encodes windows, popups, decorations, minimize/maximize: exactly the WIMP assumptions Article 1.5 and OT-26 reject as the *primary* path. Adopting it biases us back toward the desktop we're trying not to build.
4. **The clients still need the legacy stack.** A real Wayland client renders through Mesa/EGL. Supporting them means importing precisely the DRM/KMS/GBM/EGL/Mesa stack we own-the-stack to avoid. We'd inherit the latency boundaries of §3.3 that our own protocol deletes.

**Decision:** build our own minimal **cap-native surface/buffer/input protocol** as the native path — strictly better for *our* apps (finer-grained, unforgeable, kernel-enforced, WASM-shaped, lower-latency). **Do not** adopt Wayland's protocol as the foundation.

**The honest concession (a bounded compat island, flagged as a fork):** *if* app-catalogue pressure ever demands it, a **Wayland-compat surface type** can exist later as a bounded, kill-switched **compat island** — a translation shim that runs a legacy toolkit as an *untrusted cap-bundle*, sandboxed like any other, never the foundation and never trusted. This is the Bible's standing posture on compatibility (OT-44: "compatibility only ever as a bounded, kill-switched concession, never the core strategy"; the ReactOS treadmill — ~30 years chasing Windows, still behind — is the cautionary tale). Whether we build even that island, or bet fully on WASM-native apps plus a from-scratch native toolkit, is a real PL fork (§7, Fork 1).

### 4.3 Fault containment (I-14) — flagged for the build

The compositor is *the* place every principal's surfaces coexist, so it is a first-class fault domain and the I-14 containment analysis (a standing requirement for any subsystem that a future build must discharge) applies:
- **(a) Lock isolation** — the compositor and its input/vsync path must hold **no** AI-governance lock (INFLIGHT/RECENCY/ARENA/reservation/TABLE). A janking or crashed client cannot stall governance.
- **(b) Liveness degradation** — a runaway client degrades to software/Tier-0 composition (§3.3), never a governance stall; correctness preserved, only frames slow.
- **(c) Memory isolation** — client buffers are cap-gated and (for any device-initiated DMA, e.g. a GPU client) routed through the fenced DMA arena under the proven VT-d/IOMMU containment (I-14 c1/c2). A client's buffer or DMA cannot reach governance or tensor memory.

This is a THINK, so this is a flag, not a proof — but the eventual compositor build (Slice 1/3, §7) is kernel-touching and multi-principal, so it inherits the full I-14 section + the mandatory QEMU gate.

---

## 5. ISC-5 — BEST-PRACTICE + BLEEDING-EDGE (grounded, cited)

Separated into what I'd take as *proven best practice* and what I'd take as a *bleeding-edge bet*, each from a real source read directly (OT-1 discipline: no summarizing fetch for exact numbers — perception thresholds below are stated conservatively against the primary sources).

**Proven best practice — adopt without hesitation:**
- **Retained-mode scene graph + compositor-side implicit, interruptible animation.** Apple Core Animation: persistent CALayer tree, target values animated at frame rate off the main thread, model-tree vs presentation-tree for redirectable in-flight animation. (Apple, *Core Animation Programming Guide* / *Basics*.)
- **Damage tracking + client-allocated zero-copy buffers.** The modern compositor consensus; Wayland's dma-buf model is the reference for client-allocated zero-copy. (Wayland *Architecture*; wayland-book *dmabuf*.)
- **Declarative/reactive UI with a reconciliation/diff model for app content.** SwiftUI / React / Flutter: describe the UI as a function of state, diff against the previous tree, mutate the minimum. This is the right model for cap-surface *contents* (not the compositor itself).
- **Present-pacing to vsync + predictive input.** Android *Frame Pacing* (predictable input-to-screen latency) and Android *MotionPredictor* (extrapolate input, reconcile on real sample). Production-proven, directly applicable (§3.2).

**Bleeding-edge bets — I'd take these:**
- **GPU vector text from outlines (the big one).** Eric Lengyel's **Slug** algorithm renders glyphs on the GPU directly from quadratic Bézier outline data — no glyph atlas, no signed-distance-field, crisp under arbitrary magnification/minification — and Lengyel **released the patent to the public domain** (sluglibrary.com). Compute-2D engines like **Vello / piet-gpu** (linebender) are the open-source expression of the same direction. **Bet: make a Slug/Vello-class GPU-outline renderer the *native* text path.** This is precisely what turns the transcript from a bitmap console into a resolution-independent, infinitely-zoomable *surface* — the single technical enabler of "the transcript grows a body" (§1.3). Text that stays razor-crisp at any zoom is the difference between a terminal and the Atrium.
- **Intent as a first-class OS input** (alongside pointer/touch/keys). No mainstream OS treats natural-language intent as a peer input channel wired to the compositor and the capability system; we can, because Arthur is native (OT-26). Bleeding-edge, and ours to define.
- **Capability-visualized-as-UI** (§2.3). There is *no prior art* to copy because no shipping OS has capabilities as the real substrate — this is a genuine research frontier we own. High-risk, highest-payoff: it's the thing that is uncopyable.
- **Agent-co-inhabited compositor** — Arthur allocating/arranging/operating cap-surfaces under the owner's grant, with every act visible and reversible (§6). Novel; the butler made spatial.
- **Predictive input pushed to the perceptual floor** — combine MotionPredictor-style extrapolation with our deleted-boundary pipeline (§3.3) to chase the single-digit-ms floor the Ng et al. testbed proved humans can feel.

---

## 6. ISC-6 — AI-NATIVE INTEGRATION (how Arthur *lives* here)

Arthur is not a chatbot window. The failure mode to avoid is the one every current OS falls into — bolting a chat bubble onto a desktop. Arthur is a **co-inhabitant of the room**, and the Atrium is designed around that from the substrate up.

- **Presence, not a face.** The Threshold's instinct — intelligence as *light and weather, not a cartoon character* — is right, and (post-Article-29) means **Arthur is felt, not avatared**. Arthur's presence is the way the substrate behaves: the room breathes when idle, the breath *quickens* when Arthur is working (the v1 thinking-liveness indicator, shipped, generalized from a spinner into ambient state), light gathers where Arthur is acting. The butler persona (OT-46 — understated, dry, deferential, "very good, and I've made it undoable") is a *voice and a register*, heard in the transcript and felt in the room, never anthropomorphized into a talking head. This threads the needle between "assistant is constitutional now" (Article 29) and "no surveillance-y cartoon persona" (the surviving half of §12).
- **Intent-as-input, everywhere, not into a box.** You speak or type intent to the room, not to a widget. Arthur maps it to an `Action` (OT-26), the kernel cap-check is the boundary (a wrong or adversarial action is denied / confirmed-away / undone — *never* silent damage, "safe to be wrong"), and the result *appears as a cap-surface where your attention is*. The gulf of execution is zero (five-year-old THINK P1) and now spatial.
- **Clarify-don't-guess as a visual affordance.** OT-26's clarify branch and the five-year-old THINK's recognition-over-recall (P4) fuse beautifully in space: when Arthur is uncertain ("the thing from yesterday"), it doesn't guess and it doesn't pop a modal — it **surfaces the candidates as recognizable cap-surfaces** ("the budget spreadsheet or the roofing quote"), and you pick by pointing or saying "the quote." Recognition beats recall, and pointing beats spelling — especially for the five-year-old who reads shape before words (1.6).
- **The agentic loop is a steerable, visible thing.** The OT-26/A3 multi-step loop gets a visual surface: you *watch* Arthur work across cap-surfaces, and "stop" / "never mind" are always heard (Nielsen #3, five-year-old THINK P3). The butler acts smoothly without asking twice (ADR-069 yolo-within-guardrails) — but every act it takes on your behalf lands in the timeline and is reversible (OT-19). Autonomy you can watch and rewind is autonomy you can trust.
- **The five-year-old test, literally.** A child **speaks** before they type (OT-46B voice input — flagged §7) and **recognizes** shape/color before words (distinct silhouettes, 1.6). The Atrium answers to voice + point without literacy. That is the north-star test (1.5) passing not as an aspiration but as the primary interaction.
- **The owner lives here.** The emotional core the whole thing serves: this is *home*, warm and personal and yours, with a competent, discreet co-inhabitant who looks after the place. That feeling is the actual product. The Atrium is its architecture.

---

## 7. ISC-7 — THE HONEST FORKS + THE BUILDABLE SPINE

### 7.1 Forks needing PL ratification (where I'd bet, and why it's genuinely a fork)

1. **Own-protocol-only, or a Wayland-compat island later?** *My bet:* own cap-native protocol as the native path (§4.2), and **defer** the compat-island decision — build it only under real app-catalogue pressure, bounded/kill-switched/untrusted, or skip it entirely and bet on WASM-native apps + a from-scratch native toolkit. Genuine fork because it trades ecosystem reach against staying pure to the cap model; the PL owns that strategic call.
2. **Aesthetic identity: cyberpunk-neon, or the classic-Mac-craft reconciliation?** *My bet:* the reconciliation (§2.1) — classic-Mac craft *bar*, neon as light-and-meaning, the Threshold generalized. Genuine fork because ADR-024 currently commits to cyberpunk maximalism and this softens it; the PL owns the character of the thing he'll live in.
3. **Is intent the primary input, or additive to pointer/keys?** *My bet:* **additive-first** — intent is a first-class *peer* of pointer/touch/keys, never a replacement (1.5: the GUI is a convenience on top; here the spatial surface and intent co-exist, and the power user's `neo>` muscle-memory is untouched, S2). Fork because someone will argue for intent-primary; I think that fails the power user and the noisy room.
4. **Multi-principal sequencing (OT-22 — a hard gate, not a preference).** The compositor is definitionally where multiple principals' surfaces coexist, so the multi-principal Atrium (whose surfaces, whose input focus, how cross-principal grants are visualized as tethers) is gated behind the OT-22 frontier and must be **sliced one pillar at a time**, never big-banged. The single-owner Atrium is the viable N=1 floor; multi-principal is a separable expansion on top.
5. **Voice input (OT-46B).** The literal five-year-old test needs speech-in (a child speaks before typing). It's a new on-device model type (STT, Whisper-class — the OT-16/DeltaNet-style substrate decision) and is honestly gated: prove the text/pointer Atrium first, then add voice. Flagged so the design *expects* it, not so it blocks.

### 7.2 The buildable spine (smallest-first, growing from the v1 console)

Each slice is additive, kill-switchable (**disabled ≡ the prior layer, bit-for-bit** — the standing complexity-governance rule), runs on real hardware, and grows from the one before. Kernel-touching slices inherit the QEMU gate + the I-14 section (§4.3).

- **Slice 0 — shipped.** v1 transcript console + `neo-shell`/`neo-arthur` (raw framebuffer text).
- **Slice 1 — the compositor draws *one* cap-surface.** The console becomes a *composited* GPU cap-surface: our cap-native compositor + one surface + damage tracking + vsync present-pacing on the Phase-6 accel path. **Proves the pipeline and the latency budget (§3) on real silicon.** Kill-switch: compositor-off ≡ today's raw framebuffer console. *(Kernel-touching — QEMU gate + I-14.)* This is OT-52 build-1.
- **Slice 2 — GPU vector text.** Swap bitmap glyphs for a Slug/Vello-class GPU-outline renderer. **Proves resolution-independent, zoomable type** — the leap from "terminal" to "surface" (§1.3, §5). Largely userland.
- **Slice 3 — two surfaces + cap-routed input focus.** The transcript + one `neo>`/app surface; input routes to the cap-holder. **Proves the surface/focus model and the input-security-is-input-routing win (§3.3, §4.1).** *(Kernel-touching — QEMU gate + I-14.)*
- **Slice 4 — intent summons a surface.** Speak/type intent → Arthur materializes a cap-surface (OT-26 loop + a visual surface). **This is the smallest slice that proves the Atrium is a paradigm and not a desktop** — "summon, not navigate" made real. Userland (rides the shipped OT-26 substrate).
- **Slice 5 — the design language + capability made visible.** Materials/motion/type (§2) applied; owned vs granted-with-tether vs untrusted silhouettes (§2.3); the Threshold generalized to the whole room. **Proves beauty + capability-legibility.**
- **Slice 6 — time-scrub / reversibility made visual.** The timeline becomes scrubbable; undo is a place you point at (OT-19 safe-to-be-wrong, felt). **Proves "scrub, not lose."**
- **Slice 7+ — the frontier.** Multi-principal (gated OT-22, one pillar at a time), the optional Wayland-compat island (Fork 1), voice input (OT-46B / Fork 5).

The smallest first GUI slice that *proves the paradigm* is **Slice 4** (intent summons a composited, cap-gated, GPU-text surface) — everything before it is the pipeline that makes Slice 4 instant and beautiful, and everything after it is depth.

### 7.3 The boldest paradigm bet (stated plainly, as the RETURN asks)

**Capabilities you can touch, intent that summons, time you can scrub — the Atrium.** The bet is that the winning move is not a prettier desktop but a *different kind of interface*, one where **security stops being dialogs and becomes the visible, beautiful, directly-manipulable, reversible structure of the world**, where you **operate by intent** with a co-inhabitant who looks after the place, and where the whole thing is **instant because we own the stack**. It is the one interface no legacy OS can build — because none of them has capabilities as the real substrate, a native agent, and the whole stack at once. We have all three. That is the unfair advantage, and the Atrium is what it looks like spent on beauty.

---

## Sources (read from primary sources, not a summarizing fetch)

**Internal (repo):** `docs/bible.md` Articles 1.5 / 1.6; `docs/OPEN-THREADS.md` OT-52, OT-45, OT-46, OT-44, OT-26, OT-22, OT-18, OT-19; `docs/design-system.md` (ADR-024 v1.1, esp. §12 The Threshold); `docs/THINK-2026-07-07-usability-five-year-old-design.md` (P1-P7 / F1-F8); `docs/THINK-2026-07-08-poweruser-neo-shell-duality-design.md` (S1-S8 / T1-T4); ADR-069 (yolo-within-guardrails); the Phase-8 Arthur arc-2 history (`607c1e3` → `2066ee4`).

**External prior art:**
- Ng, Lepinski, Wigdor, Sanders, Dietz — "Designing for Low-Latency Direct-Touch Input," UIST 2012 — https://www.tactuallabs.com/papers/designingLowLatencyDirectTouchInputUIST12.pdf
- Dan Luu — "Computer latency: 1977–2017" — https://danluu.com/input-lag/
- Apple — Core Animation Programming Guide / Core Animation Basics — https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/CoreAnimation_guide/CoreAnimationBasics/CoreAnimationBasics.html
- Wayland — Architecture — https://wayland.freedesktop.org/docs/book/Architecture.html ; Linux dmabuf (The Wayland Protocol) — https://wayland-book.com/surfaces/dmabuf.html ; Paalanen, "What does EGL do in the Wayland stack" — https://ppaalanen.blogspot.com/2012/03/what-does-egl-do-in-wayland-stack.html
- Eric Lengyel — Slug font-rendering library (patent released to public domain) — https://sluglibrary.com/ ; linebender — Vello / piet-gpu — https://github.com/linebender/vello
- Android — Frame Pacing library — https://developer.android.com/games/sdk/frame-pacing ; Motion prediction (Jetpack Input `MotionPredictor`) — https://developer.android.com/jetpack/androidx/releases/input
- Dieter Rams — Ten Principles for Good Design (Braun / Vitsœ) — design-craft reference for "as little design as possible."

---

## Gate + close

- **Docs-only ⇒ no CI run exists** (paths-ignored: `docs/**`, `**.md`). No push CI, no QEMU — correct for a research/design THINK.
- Ran the `scripts/worker-close-gate.sh` host set intent locally: this branch adds exactly one file (`docs/THINK-2026-07-08-gui-vision-cerebro.md`), no source touched, so the fmt/clippy/host-test legs are no-ops on this diff (skip-with-note per the docs-only carve-out); SPDX header present (CC-BY-4.0, first line). The self-stat below is my Step-4 self-inspection.
- **Independence honored:** I did not read `docs/THINK-2026-07-08-gui-vision-jarvis.md` or Jarvis's branch/worktree.
- **RETURN evidence:** `git diff --stat origin/main` = 1 file added, docs-only, no phantom deletions. Diffed this artifact against the RETURN summary before sending (accuracy axis).
- **PM synthesis:** the strongest vision wins; graft the best ideas from the other. My boldest bet (§7.3) and the ISC-1 Atrium paradigm are where I'd want the synthesis to start.
- **Disk reclaim:** after the PM merges (or declines) this branch, delete the isolated worktree `E:/AI Automation/AltimitNeo-cerebro-gui` (`git worktree remove`) — it carries no `target/`, so it's cheap, but reclaim it once the work is safe on `main`.
