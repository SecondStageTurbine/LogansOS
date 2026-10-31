<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-61 web browser — architecture + prior-art research → Prometheus

**Date:** 2026-07-20
**From:** PM (Claude)
**To:** Prometheus (research seat, cross-model from the Claude PM — the strongest research seat)
**Status:** DISPATCHED — **pure research, NO-GATE lane** (a FINDINGS doc is the whole deliverable; no kernel touch, no build, no QEMU). Runs now, parallel to the runner-contended kernel gates. The web browser is **OT-61: the PL's #1 day-1 app AND a release-gating requirement** ("an OS that can't browse the internet isn't done"). Before we commit real build effort to the biggest remaining lane, we need the architecture decision made from prior art — this is that pass.

## Why this exists
We now have the substrate a browser needs: the **G1 compositor** (surface protocol, cross-process buffers, kernel-routed no-snoop input — all on `main`), the **OT-63 GPU** path (present backend + GpuSubmitCap merged; scanout fencing + real Gen9 flip in flight), the **net stack** (TCP + `net/tls_record`), and the **capability model**. The browser is where they converge. But it's a huge multi-build effort with a load-bearing up-front decision — **engine strategy** — that determines everything downstream. Get that decision right from prior art before building.

## The framing question this research must answer
**What is the right browser architecture for a capability-secured, single-node, Rust `#![no_std]`-kernel OS with a cap-native GPU compositor — and can we adopt (not just study) an existing engine?**

The MPL angle is potentially decisive and must be nailed down first: **NeoPlutarch is MPL-2.0. Servo and WebRender are MPL-2.0.** If that holds (verify from the raw LICENSE files — the exact-source discipline, NOT a summarizing fetch), Servo/WebRender are **license-compatible to adopt**, not merely reference — which would change the whole calculus (adopt-and-adapt vs build-from-scratch). This is the single highest-value fact to confirm or refute.

## What to research (each: sources, the decision it informs)
1. **Engine strategy — the core decision.** Compare, with license + maturity + integration-cost + our-substrate-fit for each:
   - **Servo / WebRender** (Rust, MPL-2.0?) — the natural candidate: Rust, cap-boundary-friendly component model, WebRender maps to our OT-63 GPU. Assess how much of Servo is `std`-dependent (it targets hosted OSes) — what would porting the render/layout engine onto our userland cost? Is a subset (WebRender + a minimal layout) viable without the full browser shell?
   - **Ladybird** (C++, from-scratch, permissive) — reference for a clean-room independent engine's scope/effort; not Rust, so integration cost is high — reference-only likely.
   - **Servo-subset / own minimal engine** — a capability-native minimal engine (HTML/CSS render + a JS interpreter) built on our stack. Scope + effort vs adopting Servo.
   - **WebView-over-nothing / WASM-sandboxed** — is a WASM-based approach (render a WASM app that IS a browser) coherent here? Probably not for a real browser, but rule it in/out.
   - **Deliver a recommendation with confidence**, not just a survey.
2. **The capability-native security model (our differentiator — mine the Qubes lesson).** A browser is the ultimate multi-principal, hostile-content app. Map: **each tab/origin as a capability-bounded principal** (per-origin process isolation — the compartmentalization Qubes validated, `docs/RESEARCH-2026-07-20-qubes-prior-art-FINDINGS.md`), **no-snoop rendering** (each tab's surface via our G1 protocol; the compositor can't read another tab's pixels/input — the property we already built), the **network cap per origin**, and the **site-isolation** model (Chrome's process-per-site-instance as prior art, done cap-natively). This is where NeoPlutarch can be *structurally* more secure than mainstream browsers — spell out how.
3. **GPU + compositor integration.** How the browser rides OT-63 (WebRender/Vello on the Gen9 iGPU behind the SharedPixelBuffer seam) + the G1 compositor (a browser window is a surface; tabs are sub-surfaces or per-origin surfaces). The glass/Atrium design language applied to browser chrome.
4. **Networking + TLS.** Rides the existing net stack (`net/tcp`, `net/tls_record`) — assess the gap between what we have and what HTTPS fetch + HTTP/2 needs. What's the minimal networking delta.
5. **The honest MVP ladder.** What is the *minimal viable browser* (render a real HTTPS page: HTML + CSS + images + basic JS + links), and the build ladder from there (JS engine choice — adopt a Rust JS engine like Boa, or SpiderMonkey/V8 which are not our license/Rust). Stage it so the release-gating bar ("can browse the internet") is reachable in a bounded number of builds, with the full-featured browser as the long tail.

## Deliverable
`docs/RESEARCH-2026-07-20-ot61-browser-architecture-FINDINGS.md` (CC-BY-4.0 header), structured like the Qubes FINDINGS:
- **§0 Executive recommendation** — the engine decision (adopt-Servo-subset / own-minimal / other), confidence-tagged, with the **license verdict** (Servo/WebRender MPL-2.0 confirmed-from-raw-LICENSE? y/n) as the load-bearing input.
- **§1 Engine options** (the comparison table + the porting-cost analysis).
- **§2 The capability-native security model** (tab-as-principal, no-snoop, site isolation, per-origin net cap — the differentiator).
- **§3 GPU/compositor integration**, **§4 networking/TLS delta**, **§5 the MVP ladder** (staged builds to the release-gating bar).
- **§6 confidence flags + sources** (raw LICENSE files with URLs + SHA where a license verdict is load-bearing; engine docs; the site-isolation prior art).
- Flag anything that contradicts our ratified stack decisions (e.g. the Vello text-engine verdict, the own-compositor decision) — the cross-model check earning its keep.

## Discipline
- **License verdicts from RAW SOURCE** (the exact-bytes rule): pull each engine's actual `LICENSE`/`COPYING` from its repo (`curl`/`git`), read the literal SPDX, don't trust a summarizing fetch. A "we can adopt Servo" recommendation MUST cite the raw MPL-2.0 license.
- **Adopt vs reference:** MPL-2.0 (Servo/WebRender) = adoptable into our MPL tree; GPL/AGPL = reference-only-no-lift; permissive (MIT/Apache/BSD) = adoptable. Classify every candidate.
- This is a **design THINK**, not code — no gate. Cross-model note: Prometheus (Qwen) authors; a **Codex/Claude cross-check** runs before any build commitment (the convergence rule — a browser-engine decision is exactly the load-bearing call that needs a different-family second opinion).

## Close (no-gate research lane)
- Push to `Workers\prometheus` on branch `prometheus/ot61-browser-architecture`. **No CI gate, no kill-switch/byte-oracle** (design doc). SPDX header on the new doc is the only mechanical requirement. The PM reviews + **cherry-picks the FINDINGS to `main`** (Prometheus does not push to `main`). RETURN: the branch + commit + a 3–5 bullet summary of the load-bearing conclusions (esp. the engine recommendation + the Servo/WebRender license verdict).

## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-20-ot61-browser-architecture-research-prometheus.md`. Context: `git show origin/main:docs/RESEARCH-2026-07-20-qubes-prior-art-FINDINGS.md` (the compartmentalization model — tab-as-principal builds on it), `…:docs/THINK-2026-07-10-g1-compositor-SYNTHESIS.md` (the surface protocol a browser window uses), `…:docs/ADR-024-phase-8-design-system-and-tech-stack.md` (the ratified stack + glass). Work in `Workers\prometheus`.
- **One-line:** Research the **web browser architecture (OT-61, the #1 day-1 app + release-gating)**. Core decision: **adopt a Rust engine or build minimal?** — nail the **license verdict first** (NeoPlutarch is MPL-2.0; **Servo + WebRender are MPL-2.0** → if confirmed from the raw LICENSE files, they're *adoptable*, not just reference — the highest-value fact). Compare Servo-subset / own-minimal / Ladybird(ref) / WASM, with porting-cost + our-substrate-fit, and **recommend** one. Spell out the **capability-native security model** (each tab/origin a cap-bounded principal, per-origin net cap, no-snoop rendering via our G1 compositor — structurally safer than mainstream, the Qubes lesson). Cover GPU/compositor integration (WebRender on OT-63), the TLS/net delta, and an honest **MVP ladder** to the "can browse the internet" bar. Deliverable `docs/RESEARCH-2026-07-20-ot61-browser-architecture-FINDINGS.md`. **License verdicts from raw LICENSE files (curl/git, never WebFetch).** A Codex/Claude cross-check follows before any build commitment.
- **RAG:** `net tcp tls_record`, `SharedPixelBuffer surface protocol`, `InputFocus no-snoop`, `atrium compositor`, `capability derive grant per-principal`. Health-check `Invoke-RestMethod http://127.0.0.1:8404/health` first; if down/degraded, note it in the RETURN and fall back to grep/Read (PL owns RAG startup; never a blocker).
