<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# PHASE 5 CLOSE PLAN — AI Subsystem Foundation

**Date:** 2026-06-04
**From:** PM/Cerebro
**Status:** ✅ CLOSED (2026-06-04) — all three gates green. Gate 1: CI green (`a7580d2`). Gate 2: `infer_transformer_forward` PASS on the OptiPlex (12 CPUs) via NEO_LOG readback, extracted sha256 `41702CDA…`. Gate 3: `Plans/CLOSE-REPORT-2026-06-04-phase-5.md` + bible ledger Active→Complete + PL sign-off. Phase 5 complete.
**References:** docs/ADR-027 (design substrate), docs/ADR-029 (Layer-2 design), bible Article 13 ledger, bible Article 6.1/6.5/20/21.

---

## Where we are

Phase 5 (AI subsystem foundation) is **functionally complete**:
- **Layer 0 — Tensor primitive** (BUILD-1): syscalls 0x100–0x103. ✅ closed.
- **Layer 1 — Model object + `.neom` + `neomc`** (BUILD-2): syscalls 0x110–0x112, §D8 lazy-staleness. ✅ closed.
- **Layer 2 — CPU inference backend** (BUILD-3 executor + BUILD-4 transformer ops + BUILD-5 Embedding): `SYS_INFER_RUN` 0x120; the full op set `0–11` (Identity, Add, Mul, MatMul, Relu, Gelu, Softmax, LayerNorm, RmsNorm, Embedding, Reshape, Transpose). ✅ all closed.
- **Milestone reached:** `infer_mlp_forward` and `infer_transformer_forward` load a `.neom` from a real NeoFS disk (`SYS_MODEL_LOAD`) and run a full forward pass (`SYS_INFER_RUN`) in one path, QEMU-verified at `NEO_SMP=4`.

The phase ledger (bible Article 13) is still **Active**. This plan closes it.

---

## The close checklist

### Gate 1 — CI green on `main` @ the BUILD-5 merge (`a7580d2`)
- [x] **DONE (2026-06-04).** CI run for `a7580d2` is green: SPDX, host-checks (fmt / `test --workspace` / `clippy -D warnings`), and the QEMU Integration Tests job — including the new `infer_embedding_forward` and `infer_embedding_oob_rejected` scenarios.
- This is the automated confirmation that the full op set (incl. Embedding) holds in the env that exposed the TLB flake. *(Cheap; just read the run.)*

### Gate 2 — One inference-on-metal boot (the only un-proven-on-metal layer)

**🟡 UNBLOCKED — fix merged; one step left (the OptiPlex boot).** The SMP=4 page fault was a real trap-entry GS bug (`current_cpu_id` ran `mov eax, gs:[24]` with GS-base=0 — a device-IRQ/IPI entering a kernel path without `swapgs`; confirmed at `idt.rs:69-73`, where the block-MSI/IPI vectors used plain `set_handler_fn` with no swapgs, unlike the timer's `timer_isr_entry`). **Fixed** in `f5aacff` (merged to `main`): NVMe/AHCI MSI + TLB-shootdown/reschedule IPI + spurious now use swapgs-aware entry stubs mirroring the timer (CS.RPL branch + symmetric swap), with a `swapgs_irq_ring3` regression pin (forces IPIs at a ring-3 CPU; observed `tlb_ipi_ring3_entries=125`). The local `infer_transformer_forward --bare-metal-mode` SMP=4 reproducer is now **green** (PASS via `NEO_LOG`, no `cr2=0x18`), no regression (normal SMP=4 suite + bare-metal SMP=1 still green). See `Plans/BLOCKER-2026-06-04-…-smp4-page-fault.md` (resolved) and `Plans/DISPATCH-2026-06-04-swapgs-device-irq-page-fault-jarvis.md`.

**Remaining for Gate 2:** the actual **OptiPlex boot** — build from `main` (≥ `f5aacff`) and run `infer_transformer_forward --bare-metal-mode` on the box per the AHCI runbook (serial cable), confirming the 3 pass criteria below on real hardware. *Residual to track (not a blocker): the NMI-during-`swapgs` window exists in these stubs exactly as in the pre-existing timer stub — a kernel-wide hardening item if/when a paranoid-entry approach is adopted.*

---

The 2026-06-03 OptiPlex run already metal-proved the inference **foundation** (real SMP at 12 CPUs, real NeoFS disk I/O = the `.neom` load path, real MM/TLB under load). What it did **not** run is the AI **compute** — `SYS_INFER_RUN` and the soft-float ops. One boot closes that, building on the 6/3 foundation.

**Vehicle:** `infer_transformer_forward` — it exercises the whole path end-to-end (disk `.neom` load → `SYS_INFER_RUN` → MatMul/Softmax/LayerNorm/Gelu/etc. → output bytes vs oracle).

**Plumbing (small, one-time):** `infer_transformer_forward` is not yet a `--bare-metal-mode` citizen (only the AHCI/NVMe NeoFS exemplars are). Wire it in — mirror the AHCI exemplar in `tools/xtask/src/main.rs`:
- add `infer_transformer_forward` to `bare_metal_log_sink_target_feature` (on-disk `NEO_LOG` sink), and
- add its disk pairing to `prepare_bare_metal_storage_disks` (a NeoFS disk staged with the model + a log-sink disk).
*(Alternative if you'd rather not plumb: an observational serial-cable boot of a custom image — but it still needs the model on physical media, so the plumbing is the cleaner path. ~half-day.)*

**Run (on the OptiPlex, per the established AHCI bare-metal runbook):**
```
cargo neo-test infer_transformer_forward --bare-metal-mode
# deploy the bundle + paired disks to the box; attach COM serial cable; boot
```

**Pass criteria (all three):**
- [ ] Boots clean — all APs up (`smp: AP cpu_id=N ready`), **no `tlb_shootdown.rs` or any kernel panic** on serial.
- [ ] `infer_transformer_forward` reaches **PASS** via the `NEO_LOG` readback — i.e. the forward-pass output bytes match the oracle on real hardware (this is the proof that `SYS_INFER_RUN` + the soft-float ops execute correctly on metal, not just QEMU).
- [ ] Capture the serial log + extracted `NEO_LOG` (hash it, like the TLB smoke) for the close-report record.

*If it surprises us:* deterministic no-XMM soft-float should be bit-identical metal-vs-QEMU, so any divergence is a real finding — capture the output bytes vs the oracle and bring it back.

### Gate 3 — Formal close (PL decision)
Once Gates 1 and 2 are green:
- [ ] **Phase 5 close report** — `Plans/CLOSE-REPORT-2026-…-phase-5.md` (phase-level, like the Phase 3 one): the three layers, the milestone, QEMU + bare-metal evidence (with the Gate-2 hash), and the deferred-items list.
- [ ] **Bible Article 13 ledger:** flip Phase 5 from **Active** → **Complete** (QEMU-verified + bare-metal-verified inference path), add a footer amendment entry, version bump per Article 27 if article text changes (ledger-only keeps version).
- [ ] **PL sign-off** recorded in the ledger entry (matches the Phase 2/3 close pattern).
- [ ] Open the next phase's opening decision (see below).

---

## Explicitly NOT required for close (deferred by ADR-027 anti-scope)

These are out of Phase 5 scope and must not gate the close:
- **Embedding widening** — higher-rank index tensors (this build is rank-1 indices → rank-2 table, the milestone shape).
- **`.neom` attribute mechanism** (D11) — deferred until a model needs a non-default (epsilon, padding_idx, etc.).
- **`SYS_TENSOR_SHARE` (0x102)** — stub; full sharing is later.
- **Accelerator drivers** — Phase 6.
- **Inference scheduler / multi-tenant serving** — Phase 7.
- **Governance policy / safety filters** — hooks only (Article 6.6), policy is userspace/later.

## What "Phase 5 Complete" asserts

A `.neom` model loads as a kernel object from disk and runs a correct CPU forward pass — MLP and transformer block — end to end through the native AI syscall family (0x100–0x120), capability-gated, no GPU, **verified on QEMU and real hardware**. "Correct and slow" (Article 6.5), achieved.

---

## Next phase (for the close decision, not this plan)
Per ADR-027 anti-scope, the foundation done, the candidates are **Phase 6 (accelerator drivers)** or **Phase 7 (inference scheduler / multi-tenant serving)** — both re-enter the SMP/concurrency long-pole class (see the TLB-shootdown close report's forecast). Sequencing is a PL/PM-DECISION at close time.
