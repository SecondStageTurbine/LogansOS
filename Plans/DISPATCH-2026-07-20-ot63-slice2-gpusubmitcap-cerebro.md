<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-63 GPU Slice 2 — complete GpuSubmitCap (cap object + submit gate) → Cerebro

**Date:** 2026-07-20
**From:** PM (Claude)
**To:** Cerebro (foundation branch `codex/ot63-slice2-gpu-submit-cap-cerebro` @ `f79f8b4`)
**Status:** DISPATCHED — **Slice 1 is now MERGED** (`48499ec`, the present-fence backend on `main`), so Slice 2 stacks on it. Kernel-touching ⇒ 4-shard `workflow_dispatch` QEMU gate + AAG. Completes the GpuSubmitCap you scoped in the foundation commit.

## FIRST — rebase the foundation onto merged Slice 1
Your foundation (`f79f8b4`: `gpu_submit_cap_logic::authority_allows` kill-switch truth table + host KAT, all 4 combos pinned) was cut **before** Slice 1 landed. **Rebase it onto current `origin/main`** (`48499ec` — has `present_backend.rs` + `arm_flip`/`on_present_vblank`), then build the follow-on on top. The pure logic + host KAT stay; the follow-on is the gating that needs the merged present backend.

## What Slice 2 completes (your own named follow-on)
Per your foundation commit — *"Handle::GpuSubmit variant + rights + the gpu-submit-cap cfg-gate on the present HardwareFlip submit + QEMU scenario"*:
1. **The cap object** — a `Handle::GpuSubmit { … , rights }` variant + `full_gpu_submit_rights()`, mirroring the existing cap patterns (`Handle::InputFocus`/`Buffer`/`Channel`, `net-dial-cap`). The GpuSubmitCap is the **scarce authority to arm a hardware-flip submit**.
2. **The submit gate** — the `HardwareFlip` submit path (`present_backend::arm_flip` / the `present()` HardwareFlip branch) is gated by `authority_allows(enforcement_enabled, caller_holds_gpu_submit_cap)`: enforcement-on + no matching cap ⇒ **refused** (no raw renderer reaches the flip); enforcement-on + cap-held ⇒ proceeds; enforcement-off ⇒ the kill-switch bypass. No path arms a hardware flip without passing the gate.
3. **QEMU scenario** — cap-held ⇒ submit proceeds (synthetic-vblank completes the fence, as Slice 1); enforcement-on + **no** cap ⇒ submit refused (clean error, no flip armed); and the kill-switch equality (enforcement-**off** ≡ the Slice-1 HardwareFlip path bit-for-bit — the cap gate compiles/runs to a no-op).

## Scope boundary — do NOT build here (later sub-slices)
- **The scanout/submit DMA fencing under VT-d (I-14(c))** is **Slice 3** — Slice 2 is the *cap + gate mechanism*, not the DMA path. State the I-14(c) seam (every gated submit will route its buffers through `dma_arena::fence` under the iGPU VT-d domain in Slice 3); don't implement fencing here.
- **The real Gen9 flip + vblank ISR on silicon** is **Slice 4-5** (bare-metal, OptiPlex/M710q). Slice 2 stays QEMU-provable on the synthetic vblank — **no bare-metal run, no modeset.**

## Kill-switch (Rule-1) + byte-oracle (I-12)
- **Kill-switch:** you already have the truth table (`authority_allows`). **Arm the regression** proving enforcement-off ≡ the pre-cap Slice-1 HardwareFlip submit bit-for-bit (not just the host-KAT logic — the *gate-in-the-present-path* disabled ≡ prior layer).
- **Byte-oracle:** if the GpuSubmitCap crosses any new on-wire boundary (a grant/transfer envelope beyond the existing cap-transfer machinery), hand-lay a fixture. If it rides the existing cap-grant path (no new wire layout), state that — no new oracle needed.

## I-14 + CP-bank (required sections)
- **I-14:** (a) the submit-gate check acquires **no** AI-governance lock; (b) a refused submit degrades cleanly (no stall — the caller gets an error, governance untouched); (c) **no new DMA in Slice 2** (the fenced scanout DMA is Slice 3 — state the seam). The GpuSubmitCap is the *authority* that Slice 3's fencing will enforce at the DMA layer.
- **CP-bank (`docs/concurrency-pitfalls.md`):** the submit gate sits on the `arm_flip`/`on_present_vblank` seam Slice 1 already proved (CP-2 no-IF-off-spin, the present-fence handshake). Confirm the cap-gate adds no new IF-off spin and no CP-29/I-16 regression; the real vblank-ISR wake-race is Slice 4, not here.

## AAG (PM, pre-merge — cross-model lenses)
Concurrency (the cap-gate on the present-submit seam, no new wake-race, no CP regression), ABI/cap (**the GpuSubmitCap genuinely gates submit — no `derive`/rights path lets a non-grantee obtain a submit-capable cap and arm a flip**; this is the Slice-4-no-snoop lesson carried forward — verify the cap can't be self-minted by a non-holder), completeness (the refused-submit + kill-switch-equality are falsifiable, armed — the gate can't be deleted with the scenario still green), memory-safety.

## Close
- `scripts/worker-close-gate.sh` (rebase-fresh on `48499ec`; full host-check set; SPDX; Step-4 self-stat). 4-shard `workflow_dispatch` gate — **one wave at a time**, stagger behind any live Jarvis/Oracle gate. RETURN the green push-CI + 4-shard URLs + the refused-submit + kill-switch evidence.
- **Disk reclaim** after PM-confirmed merge.

## Relay — what to tell Cerebro
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-20-ot63-slice2-gpusubmitcap-cerebro.md`. Slice-1 seam you build on: `git show origin/main:kernel/src/atrium/present_backend.rs` (`arm_flip`/`on_present_vblank`/`PresentMode`). The Rung-1 plan: `git show origin/main:Plans/DISPATCH-2026-07-19-ot63-rung1-gen9-flip-vblank-present-cerebro.md`. PM artifacts live on `origin/main`; `git show`, not a local open.
- **One-line:** OT-63 GPU Slice 2 — **Slice 1 is merged (`48499ec`)**, so **rebase your foundation branch `codex/ot63-slice2-gpu-submit-cap-cerebro` (`f79f8b4`) onto current `origin/main`** and complete the follow-on you named: the **`Handle::GpuSubmit` variant + `full_gpu_submit_rights()`** (mirror `net-dial-cap`/`InputFocus`) + **gate the `present()` HardwareFlip submit** on `authority_allows(enforcement_enabled, caller_holds_cap)` (enforcement-on + no cap ⇒ refused, no flip armed; cap-held ⇒ proceeds; enforcement-off ⇒ kill-switch bypass) + a **QEMU scenario** (submit-with-cap ✓, submit-without-cap refused, kill-switch enforcement-off ≡ Slice-1 HardwareFlip bit-for-bit). **Do NOT** build the scanout DMA fencing (Slice 3 — state the I-14(c) VT-d seam) or the real Gen9 flip/vblank (Slice 4-5 bare-metal). Arm the kill-switch regression (gate-in-present-path disabled ≡ prior). I-14 (gate takes no governance lock, refused-submit no stall, no new DMA) + CP-bank (no new IF-off spin on the arm_flip seam, no CP-29/I-16 regression). AAG will check the cap genuinely gates submit (no non-grantee derive path — the Slice-4 no-snoop lesson). 4-shard gate (stagger) + AAG.
- **RAG:** `present_backend arm_flip HardwareFlip submit`, `gpu_submit_cap_logic authority_allows`, `net_dial_cap Handle rights`, `full_input_focus_rights derive`, `dma_arena fence VT-d gpu`. Health-check first; note-and-fall-back if down (PL owns RAG startup).
