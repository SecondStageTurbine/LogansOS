<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Qubes OS prior-art trap-mining — compartmentalization UX + the trusted GUI → Prometheus

**Date:** 2026-07-20
**From:** PM (Claude)
**To:** Prometheus (research seat, cross-model from the Claude PM — the convergence-signal rule; strongest research seat)
**Status:** DISPATCHED — **pure research, NO-GATE lane** (no kernel touch, no build, no QEMU; a FINDINGS doc is the whole deliverable). Runs *now*, parallel to the runner-contended kernel gates (OT-63 Slice 1 + the G1 Slice-2 re-spin are queued). This is a **prior-art trap-mining pass** in the spirit of the driver trap-mining standing rule, aimed at a *design* target instead of a chip.

## Why this exists
We have **never** studied Qubes OS — "Qubes" appears zero times in the tree; the only `Xen` mentions are an incidental FreeBSD TLB advisory, unrelated to Qubes' design. Yet Qubes is **the** reference system for security-by-compartmentalization, and we have been reasoning about several of its core ideas *from first principles* when a documented reference exists. Two live consumers need this **now**:

1. **The Atrium trusted GUI (highest value — live this week).** Qubes' signature is **per-security-domain colored window borders** + a trusted window manager where **no domain can snoop another domain's pixels, input, or clipboard**. That is a direct precedent for two things we just committed to: our **trust-state-as-tint glass** (ADR-024 Amendment 2026-07-19: owned=clear+blue / untrusted=heaviest+gray) and the **kernel-routed InputFocus no-snoop** property in the G1 compositor (the "Redox Orbital trap we don't inherit"). We reinvented Qubes' central GUI-security idea independently — mine how they actually did it, and where it bit them.
2. **I-14 subsystem fault containment.** Qubes is the canonical "a driver fault cannot escape its domain" system — it runs the network and USB stacks in isolated VMs (`sys-net`, `sys-usb`). Direct precedent for the I-14 containment invariant and OT-56 driver reincarnation (containment → recovery).

## The framing question — the ONE thing this study must foreground
**Qubes isolates via heavyweight virtualization** (the Xen hypervisor, VM-per-domain, dom0, hardware-enforced VT-x/VT-d) — **we isolate via lightweight capabilities in ONE monolithic kernel.** So Qubes is:
- **strong prior art for the GOALS + UX** of compartmentalization — the trust-domain GUI, the inter-domain policy model, the no-snoop guarantee, the mental model users navigate;
- a **cautionary reference for the MECHANISM** — VM weight (RAM per qube, boot latency, the dom0 TCB, the usability tax) is the *opposite* of our design, and is exactly why Qubes stayed niche.

Every finding must be tagged **TRANSFERS** (a goal/UX/policy idea we should adopt or adapt to caps), **CAUTION** (a Qubes mechanism/limitation we must NOT inherit, and why our cap model avoids it), or **OPEN** (a genuine tension where we don't yet have an answer). The value of the pass is precisely this sort.

## What to mine (each: the source, what LogansOS thread it feeds, the trap to extract)
1. **The GUI virtualization / secure GUI protocol** (the crown jewel). How Qubes' trusted GUI daemon (`gui-daemon` in dom0) composites untrusted-domain windows without letting a domain: read another's framebuffer, forge the colored border / trust decoration, capture global input, or abuse the clipboard/drag-drop. Extract: the **colored-border trust-decoration** design (feeds our trust-tint glass — including *how they stop a malicious domain from spoofing its own border/title*, which is the redundancy problem Janus already flagged for us); the **secure copy/paste** controlled-channel model; the **input-routing** trusted path (feeds InputFocus). **Trap:** document the known *weaknesses* — border-spoofing surface, the "which window has focus" trusted-indicator problem, side channels through window geometry, the performance cost of the copy-composite path (feeds our OT-63 glass budget).
2. **qrexec — inter-domain RPC + the central policy layer.** The `qrexec` policy files (`/etc/qubes/policy.d`), the ask/allow/deny model, service-based RPC. Feeds our **capability-gated IPC / grant policy** and the surface-protocol authority model. **Trap:** where Qubes' policy is *coarse* (whole-VM grants, ask-fatigue) — contrast with our per-capability derivation-graph revocation (OT-18 grant ergonomics).
3. **Split-GPG / split-SSH** — secrets live in a vault domain that never exposes the key, only signing/decryption *operations*, over qrexec. Feeds the **password-manager shelf app** and secret-handling design. **Trap:** the confirmation-prompt UX (per-use vs cached) and its ask-fatigue.
4. **Disposable VMs + the template/AppVM split** — immutable template root, per-app volatile overlay, disposable one-shot qubes. Feeds **OT-51 rollback / immutable base** and **OT-56 driver reincarnation**. **Trap:** the state/persistence model and where users lose data to the immutability.
5. **`sys-net` / `sys-usb` driver-domain isolation + inter-qube memory sharing (Xen grant tables).** Feeds **I-14** (a/b/c) directly. **Trap:** what Xen grant-tables give them that our `dma_arena::fence` + IOMMU containment must match, and the attack surface of the shared-memory GUI/network path.
6. **dom0 as the TCB + "Qubes Air" direction.** dom0 is a large trusted base and Qubes' biggest critique; "Qubes Air" was their move to shrink Xen-dependence. Feeds our **TCB-size / `unsafe`-audit** posture (OT-54) — the honest question: does our monolithic-kernel-with-caps have a *smaller or larger* effective TCB than Qubes' dom0, and where.

## Sources (read for PRINCIPLE — Qubes core is GPLv2; NEVER copy code into our MPL kernel)
Same no-GPL-lift discipline as the driver trap-mining rule: learn the design, do not lift the code. Authoritative:
- The **Qubes OS Architecture** whitepaper (Rutkowska & Wojtczuk, 2010) — the foundational design.
- The **official docs** (`qubes-os.org/doc/`): the GUI / "GUI virtualization" page, `qrexec`, the Admin API, split-GPG, disposable VMs, the template system, `sys-net`/`sys-usb`, the security-critical-code notes.
- **Joanna Rutkowska's essays** — "Software compartmentalization vs. physical separation," the "Qubes Air" post, the trusted-GUI writings.
- **Known limitations / CVEs** — the Qubes Security Bulletins (QSB) for the GUI/qrexec/Xen classes, and honest community critiques of the usability/RAM/TCB cost. The traps are as important as the design.
- Cross-check exact claims against the primary source; if a fact is load-bearing for a "TRANSFERS" recommendation, cite the doc/section — don't assert from memory.

## Deliverable
`docs/RESEARCH-2026-07-20-qubes-prior-art-FINDINGS.md` (CC-BY-4.0 header), structured:
- **§0 Executive TRANSFERS / CAUTION / OPEN table** — the cap-vs-VM sort, one row per mined idea, confidence-tagged (High / Med / Low, like the GUI-prior-art FINDINGS).
- **§1 The trusted GUI** (the deepest section — feeds the glass amendment + InputFocus; call out the border-spoofing/trusted-indicator traps explicitly, tie to Janus's colorblind-redundancy correction).
- **§2 qrexec / policy**, **§3 split-secrets**, **§4 disposable/template**, **§5 driver-domain isolation → I-14**, **§6 dom0/TCB honesty**.
- **§7 The five (or so) load-bearing conclusions** — what we should adopt/adapt, each tied to an OT thread (OT-52 compositor, OT-42 sovereign device, OT-18 grants, OT-51 rollback, OT-54 TCB, OT-56 reincarnation, I-14).
- Explicitly flag anything that *contradicts* a decision we've already ratified — that's the highest-value output (the cross-model rule earning its keep, same as Janus's glass cross-check).

## Close (no-gate research lane)
- **No kill-switch / no byte-oracle / no CI gate** — this is a design doc, not code. `scripts/worker-close-gate.sh` is N/A (nothing compiles). SPDX header on the new doc is the only mechanical requirement.
- **Push to a branch `prometheus/qubes-prior-art` (NOT main).** The PM reviews the FINDINGS and **cherry-picks the doc to `main`** (Prometheus does not push to `main` — confirmed standing rule). RETURN the branch + commit hash + a 3-5 bullet summary of the load-bearing TRANSFERS/CAUTION conclusions.
- **Disk reclaim:** trivial (a docs-only branch, no `target/`); delete the tree after the PM cherry-picks.

## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-20-qubes-prior-art-trap-mining-prometheus.md`. Context to read the same way: `git show origin/main:docs/RESEARCH-2026-07-19-gui-prior-art-glass-frost-FINDINGS.md` (your last GUI pass — this extends it), `git show origin/main:docs/ADR-024-phase-8-design-system-and-tech-stack.md` (the glass amendment this feeds), and the InputFocus/G1 synthesis `git show origin/main:docs/THINK-2026-07-10-g1-compositor-SYNTHESIS.md`. PM artifacts live on `origin/main`; `git show`, not a local open.
- **One-line:** We've never mined **Qubes OS** — the reference compartmentalization system — and it's a direct precedent for two live threads: (1) its **per-domain colored window borders + no-snoop trusted GUI** = our trust-tint glass + kernel-routed InputFocus (the highest-value angle), and (2) its **driver-domain isolation** = our I-14 containment. Produce a FINDINGS doc that sorts every idea **TRANSFERS / CAUTION / OPEN** against the core distinction — **Qubes isolates via heavyweight Xen VMs; we isolate via lightweight capabilities in one monolithic kernel** — so we steal the *goals + trusted-GUI + policy UX* and consciously reject the *VM-weight mechanism*. Mine the GUI protocol deepest (incl. the border-spoofing / trusted-indicator traps — tie to Janus's colorblind-redundancy point), plus qrexec/policy, split-GPG, disposable/template, `sys-net`/`sys-usb` isolation, and dom0-as-TCB honesty. **Read for principle only — Qubes core is GPLv2, NEVER copy code into our MPL tree** (same rule as driver trap-mining). Deliverable `docs/RESEARCH-2026-07-20-qubes-prior-art-FINDINGS.md` on branch `prometheus/qubes-prior-art`; PM cherry-picks to main. Flag anything that contradicts a ratified decision — that's the point.
- **RAG:** available for locating our own seams the study cross-references — health-check `Invoke-RestMethod http://127.0.0.1:8404/health` first; e.g. `InputFocus kernel routed`, `trust-state tint glass`, `I-14 fault containment driver`, `dma_arena fence IOMMU`, `capability grant revoke derivation`. If down/degraded, note it in your RETURN and fall back to grep/Read (never a blocker; the PL owns RAG startup).
