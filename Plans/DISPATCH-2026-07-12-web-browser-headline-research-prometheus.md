<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: The web browser — the PL's #1 day-1 app (DEEP RESEARCH + DESIGN, two-path evaluation) → Prometheus

**Date:** 2026-07-12
**From:** PM (Claude)
**To:** Prometheus. **Branch:** `prometheus/web-browser-headline-research`, output `Plans/RESEARCH-2026-07-12-web-browser-FINDINGS.md`.
**Status:** DISPATCHED — **deep research + design, HOST-ONLY, no code, no QEMU.** **The PL has named the web browser the #1 day-1 app and a RELEASE-GATING requirement — "an OS that can't browse the internet is not shippable."** This is elevated OUT of the round-2 shelf survey (OT-60) into its own program-scale study. It is NOT a shelf row and NOT optional: the first shippable LogansOS MUST browse the real modern web. The study's job is **the fastest honest path to a browser that renders real modern sites** — not whether to have one. PL posture (ratified this dispatch): **let the study decide the JS-engine fork** — lay out both paths with evidence and recommend; the PL ratifies at RETURN.

## The framing: work BACKWARD from "renders the real modern web" (a hard requirement, not an aspiration)
The ship must browse real JS-heavy sites (Gmail, banking, web apps) — treat that as a fixed constraint and find the shortest path to it. The one nameable obstacle: real sites need a **JS engine**, and the only **production-complete ones are C++** (SpiderMonkey, V8); pure-Rust JS (Boa/Kiesel/Nova) is real but **cannot run the modern web yet** — verify exactly how far it is, because if it can't, Path B is a scaffold, not the ship. So the study's spine is a **two-path evaluation** each costed from primary sources — but graded against the fixed requirement "renders the real web," NOT against "is it pure Rust." **A CPU HTML/md reader / Arthur-mediated fetch is a SCAFFOLD that may exist during bring-up — it is explicitly NOT an acceptable day-1 browser on its own. The endpoint is a real engine.**

## ISC-1 — Path A: Servo-embed (the real modern-web browser)
Servo (now under Igalia stewardship) is the live pure-Rust **browser engine**. Evaluate embedding it as LogansOS's browser:
- **Real current state** (firsthand: Servo's repo, `servoshell`, the embedding/`libservo` API, their roadmap/blog) — what does it render *today*, how stable is the embedding API, what's the maintenance velocity?
- **The dependency reality** — it uses **SpiderMonkey (C++)** for JS and **WebRender on `wgpu` (GPU)**. Audit the full dep tree: which parts are Rust, which are C/C++ (`mozjs`/SpiderMonkey, ANGLE, etc.), and the honest size/build cost. Frame the C++ JS engine explicitly against our ratified "no compat island" rule: is one capability-sandboxed JS engine a **bounded component** (like a driver) or a **compat island**? Argue it, don't assume.
- **What "embed Servo in LogansOS" concretely requires** — the GPU/accel path (see ISC-3), the compositor integration (OT-52), and the **`std` surface Servo assumes** (it leans hard on std/threads/net — how big is that gap vs. our libneo `no_std` userland + the std-analog rung?). Honest effort estimate (person-months-scale, phased).

## ISC-2 — Path B: the pure-Rust stack (sovereign, lighter)
- **Rendering:** `blitz` (Dioxus-org: Servo's `stylo` CSS + `taffy` layout + `vello` paint) — what HTML/CSS subset does it render today, GPU (Vello/wgpu) vs CPU-render feasibility, dep purity from the tree.
- **JS:** `boa` (+ audit `kiesel`, `nova` — the emerging Rust engines) — enumerate **honestly** what ECMAScript coverage + real-site behavior exists today and the trajectory. This is the load-bearing limiter of Path B.
- **Verdict:** what real sites work vs. don't (docs/web-lite/simpler sites vs. Gmail-class apps). Maximal sovereignty/purity; honest about not being a Chrome-equivalent for a while.

## ISC-3 — the GPU / acceleration prerequisite (shared leverage)
Both real-rendering paths need a GPU/accel path (WebRender→wgpu, or Vello→wgpu); we don't have one. Study: what a browser needs from our graphics stack, how it **pulls the GPU/compositor path (OT-52) forward** in the roadmap, and — the leverage point — that this same investment **unlocks the T2/T3 GUI app tiers** (round-1/round-2 shelf), so it is not browser-only spend. State the minimal accel surface a browser needs (is a software-rasterized floor viable for a reader, GPU required only for real-web?).

## ISC-4 — capability-sandboxing a browser (where LogansOS can be BETTER)
A browser is the single most dangerous app — it runs arbitrary untrusted remote code/content. Design how our capability model **contains** it: per-origin/per-tab capability scoping, a network cap with no ambient filesystem (downloads via an explicit cap only), the browser as the ultimate stress test of I-14 subsystem fault-containment and the sandbox. This is the differentiator — a **truly capability-confined browser** is something mainstream browsers bolt on (sandboxing/site-isolation) and we could have by construction. Sketch the model.

## ISC-5 — sovereignty / privacy / Arthur posture
- The browser must honor the sovereignty moat (Art. 1.7): no telemetry, owner-controlled, no phone-home. State how each path delivers that.
- Reframe the earlier "tabbed browser vs. Arthur-mediated fetch" fork as **both**: a real browser the owner drives AND Arthur able to drive/fetch/summarize through the same engine under caps. Which path serves the Arthur-integration better?

## ISC-6 — sequencing to the earliest REAL (modern-web-capable) browser — the deliverable that gates ship
The ship gate is "browses the real modern web," so sequence to THAT, aggressively-but-honestly: the critical-path ladder to a modern-web-capable engine (Path A or B), the **single highest-leverage first step**, and an honest earliest date. A CPU HTML/md reader + Arthur-mediated fetch may exist as a **bring-up scaffold** (something real on screen early) — state it as such, **never as the day-1 ship browser** (the PL's line: an OS that can't browse the internet isn't shippable). Because a real browser needs the GPU/accel path, **this study puts the GPU/accel path on the critical path to first release** — say so plainly and price it.

## ISC-7 — prior art (firsthand, the reality check)
- **Servo** (Igalia) — the live Rust engine; its embedding maturity + roadmap.
- **Ladybird** — the from-scratch independent browser (C++, exploring Swift): use its **timeline/effort** as the honest reality-check on "how long does building a browser take," even though it's not a Rust fit.
- **Blitz/Dioxus, Boa/Kiesel/Nova** — the pure-Rust stack's real state.
- How other small/independent OS projects ship (or don't ship) a browser.

## Deliverable — the recommendation + the ratifiable forks
A clear recommendation (Path A vs B vs a B-now/A-later phasing), the honest effort/timeline per path, the GPU-path pull-forward consequence, the capability-sandbox model, and the **forks the PL ratifies at RETURN**: (1) the JS-engine/sovereignty posture (bounded-C++-engine vs pure-Rust-only vs phased); (2) whether the browser justifies pulling the GPU/accel path forward now; (3) the day-1 interim floor.

## Scope guard
Research + design FINDINGS. No code, no QEMU, host-only. Every load-bearing claim (engine completeness, dep purity, GPU requirement, effort) from the **primary source read directly** — the repos' own `Cargo.toml`/`Cargo.lock`/docs, Servo/Ladybird/Boa's own docs — never a summarizing fetch or a blog claim (3-vote verify load-bearing claims). Don't re-open ratified decisions (Atrium identity; install-is-data; sovereignty moat). This is the ONE place the "no compat island" rule gets a genuine hard test (the JS engine) — surface it for PL judgment, don't pre-decide it. Recommend; the PL ratifies.

## Gate + close
- **Docs-only ⇒ no CI run exists** — say so; `scripts/worker-close-gate.sh` host set locally (skip-with-note). **Diff the RETURN against the committed artifact.**
- **RETURN:** the FINDINGS path + the two-path evaluation (each costed from the dep tree) + the JS-engine reality + the GPU-prerequisite/leverage + the capability-sandbox model + the sovereignty/Arthur posture + the sequencing-to-earliest-real-browser + prior art + the recommendation + the three ratifiable forks + cited primary sources.
- **Disk reclaim** after merge.

---
## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-12-web-browser-headline-research-prometheus.md` (+ `git show origin/main:docs/OPEN-THREADS.md` **OT-61** [this study], **OT-60** [the shelf — browser is elevated OUT to here], **OT-52** [the compositor/GPU path this pulls forward], **OT-44** [install-is-data cap-bundle + the SDK/ABI + H3 WASM sandbox], **OT-57** [the console for the interim reader]; `docs/SDK-ABI.md` [what userland provides today]; `docs/bible.md` Article 1.7 [the sovereignty moat the browser must honor]; `docs/NORTH-STAR-2026-07-11-usability-review.md`). **Branch:** `prometheus/web-browser-headline-research` → `Plans/RESEARCH-2026-07-12-web-browser-FINDINGS.md`.
- **One-line:** **Deep research + design — the web browser is the PL's #1 day-1 app; find the most-capable browser LogansOS can ship earliest, via a two-path evaluation the PL ratifies at RETURN.** **Path A — Servo-embed** (Igalia's live Rust engine; real modern web but pulls SpiderMonkey C++ JS + a GPU/`wgpu` path — audit the full dep tree + the `std` surface Servo assumes; argue whether one cap-sandboxed JS engine is a bounded component or a compat island). **Path B — pure-Rust stack** (`blitz` = stylo+taffy+vello for HTML/CSS + `boa`/`kiesel`/`nova` for JS — enumerate HONESTLY what real sites work vs. don't; JS coverage is the limiter). Plus: **the GPU/accel prerequisite** (both real paths need it; it pulls OT-52 forward but ALSO unlocks the T2/T3 GUI shelf — shared leverage), **capability-sandboxing a browser** (the most dangerous app → per-origin caps, network-cap-no-ambient-FS, the differentiator: a browser confined by construction), **sovereignty/no-telemetry + Arthur-drives-the-engine-too**, and **the sequencing to earliest-real-browser** (day-1 interim floor = CPU HTML/md reader + Arthur-mediated fetch so the slot isn't empty → the real engine). Prior art firsthand: **Servo (Igalia), Ladybird (effort/timeline reality-check, C++), Blitz/Dioxus, Boa/Kiesel/Nova.** Deliver a recommendation + three ratifiable forks (JS-engine posture; GPU-path pull-forward; day-1 interim floor). **Host-only, no code/QEMU; primary sources only (repos' Cargo/LICENSE/docs); 3-vote verify load-bearing; the JS engine is the ONE genuine test of "no compat island" — surface it for PL judgment, don't pre-decide.** Close-gate host set (skip-with-note); diff RETURN vs artifact.
- **RAG:** semantic code search is available — use it for our substrate facts (e.g. "compositor framebuffer gpu", "https fetch tls userland", "capability grant network", "wasm sandbox runtime", "SDK ABI userland syscalls"). Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`. If down/degraded: the PL owns RAG infra — note it in the RETURN and continue with direct file reads (never a blocker).
