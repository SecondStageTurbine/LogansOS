<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: G1 compositor Slice 2 — the surface protocol verbs → Jarvis

**Date:** 2026-07-19
**From:** PM (Claude)
**To:** Jarvis (main-tier; the G1 compositor lane, parallel with Cerebro's OT-63 GPU lane)
**Status:** DISPATCHED — kernel-touching (IPC protocol over the existing cap primitives) ⇒ mandatory 4-shard `workflow_dispatch` QEMU gate + AAG. Slice 2 of the ratified G1 ladder (`docs/THINK-2026-07-10-g1-compositor-SYNTHESIS.md`); Slice 1 (Rung 0) merged as `daa5332`.

## What Slice 2 is
Rung 0 proved **one hardcoded surface** composites (create→map→commit→blit, the two kernel objects, the software present-fence, kill-switch ≡ v1 console). **Slice 2 generalizes that one-surface hardcode into the real, capability-gated surface PROTOCOL** — the small message vocabulary a userland client and the trusted compositor speak over an IPC channel to manage surfaces. Per the synthesis: **the protocol rides IPC channels — NO new control syscalls; a SurfaceCap IS a dedicated IPC channel** (a convention, not a new kernel object kind); the kernel stays mechanism-only (Article 6.6), all surface/scene/focus *policy* is userland.

## The verbs to implement (the ones Rung 0 didn't — present+fence stays Cerebro's lane)
Complete the verb table over the SurfaceCap channel, each with **caps-required stated** and the **wire envelope** (Chad §2.2) + cap-transfer-on-IPC (Chad §2.3):
- **create-surface** — **role-typed** (`WORKFIELD_ROOT` / `TEXT` per Chad; **G1 accepts exactly one full-screen root** — reject a second root); returns the SurfaceCap channel.
- **attach / import-buffer** — bind a `SharedPixelBuffer` (the Rung-0 object) to the surface as its content; the client-WRITE / compositor-READ sibling-derivation rights are Rung-0/Slice-3's — Slice 2 wires the *attach verb*, not the buffer lifetime mechanics.
- **commit** — publish the attached buffer as the surface's current frame (the compositor picks it up at composite time).
- **focus-request** — a surface asks for input focus; the compositor's focus **policy** decides (userland). **Slice 2 delivers the request verb + the policy hook; the actual kernel-routed event delivery is Slice 4** (do NOT wire HID routing here — that's InputFocus/Slice 4).
- **destroy** — tear down the surface; peer-death of the channel = surface-death (the convention) must also work (destroy-by-drop).

**Scope boundary — do NOT touch:** `present()` / the present-fence backend (**Cerebro's OT-63 Rung-1 lane — coordinate, do not collide**); the buffer *lifetime* mechanics (immutable descriptor / Pmap BorrowedBuffer / Arc pinning — **Slice 3**); HID input routing (**Slice 4**); damage/pacing (**Slice 5**). Slice 2 is the verb *interface* + dispatch + the one-root-and-multi-surface generalization only.

## Kill-switch + byte-oracle
- **Kill-switch (Rule-1):** compositor-off ≡ the v1 console bit-for-bit (the Rung-0 kill-switch extends — with the protocol feature off, the console path is byte-unchanged; armed regression, not asserted).
- **Byte-oracle (I-12 — the protocol is an on-wire layout):** the **wire envelope** (verb header + args) is a byte layout crossing the IPC boundary ⇒ a **hand-laid byte fixture** per verb, asserted independently of the writer+reader (mirror the Rung-0 descriptor byte-oracle). Include a role-tag fixture (WORKFIELD_ROOT vs TEXT) + the reject-second-root case.

## I-14 + OT-22-readiness
- **I-14:** the protocol is IPC over existing cap primitives — **no new DMA path** (buffer DMA is Slice 3's I-14(c)); state the (a) lock-isolation (the verb dispatch takes no AI-governance lock) + (b) a misbehaving client can't stall the compositor (bounded per-channel handling).
- **OT-22-readiness (assert at the protocol level, zero redesign expected):** namespaced surface ids + cap provenance on every verb, so a future multi-principal world drops in without a protocol redesign.

## Gate + close
- Host KATs for the verb logic + the byte-oracle fixtures; a **QEMU scenario** exercising the verb sequence (create-root → attach → commit → focus-request → destroy, + reject-second-root) end-to-end, + the kill-switch equality. `scripts/worker-close-gate.sh` (rebase-fresh; full host-check set; SPDX; Step-4 self-stat). Kernel-touching ⇒ 4-shard `workflow_dispatch` QEMU gate. **Runner note:** stagger behind Cerebro's OT-63 gate (one 4-shard wave at a time). RETURN pastes the green push-CI + 4-shard URLs + the byte-oracle evidence.
- **Disk reclaim (after PM confirms merge):** delete this build's tree once on `origin/main` + PM-confirmed.

## AAG (PM, pre-merge)
ABI/byte-oracle (the wire envelope per verb is byte-correct + genuinely independent), completeness (the verb table + the one-root invariant + kill-switch armed), memory-safety (channel/message handling, no new `unsafe` misuse), concurrency (per-channel verb handling takes no governance lock; peer-death/destroy race is clean).

---
## Relay — what to tell Jarvis
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-19-g1-compositor-slice2-protocol-verbs-jarvis.md`. Design of record: `git show origin/main:docs/THINK-2026-07-10-g1-compositor-SYNTHESIS.md` (§4 the merged shape + the verb table; Chad §2.2 wire envelope / §2.3 cap transfer). The Rung-0 code (`daa5332`) is the one-surface base you generalize. PM artifacts live on `origin/main`; `git show`, not a local open.
- **One-line:** G1 Slice 2 — generalize Rung 0's one-surface hardcode into the real capability-gated **surface protocol** over IPC channels (a SurfaceCap IS a channel; no new control syscalls): the verbs **create-surface (role-typed, exactly one full-screen root), attach/import-buffer, commit, focus-request (verb + policy hook only — NOT HID routing, that's Slice 4), destroy** (peer-death = surface-death). **Do NOT touch `present()`/the present-fence (Cerebro's OT-63 Rung-1 lane — coordinate), the buffer lifetime mechanics (Slice 3), or input routing (Slice 4).** Byte-oracle the wire envelope per verb (hand-laid fixtures, incl. role-tag + reject-second-root); kill-switch compositor-off ≡ v1 console bit-for-bit; namespaced ids + cap provenance for OT-22-readiness. Host KATs + a QEMU verb-sequence scenario + 4-shard gate + AAG.
- **RAG:** use it for the compositor/IPC/cap seams — health-check `Invoke-RestMethod http://127.0.0.1:8404/health`; e.g. `SharedPixelBuffer surface commit`, `atrium slice1 blit present`, `IPC channel cap transfer`, `InputFocus focus`, `libneo surface protocol`. If down, note it + grep/Read (never a blocker; the PL owns RAG startup).
