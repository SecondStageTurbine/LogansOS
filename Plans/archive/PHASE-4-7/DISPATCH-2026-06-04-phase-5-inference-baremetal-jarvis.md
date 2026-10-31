<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 5 Gate-2 — inference-on-metal verification → Jarvis

**Date:** 2026-06-04
**From:** PM/Cerebro
**To:** Jarvis (worker tree, feature branch — e.g. `codex/phase-5-baremetal-inference-jarvis`)
**Status:** DISPATCHED — ready to BUILD + run
**Gates:** This is **Gate 2** of `Plans/PHASE-5-CLOSE-PLAN-2026-06-04.md`. (Gate 1 = CI confirm, being checked separately; Gate 3 = formal close, after this.)
**Requires:** the OptiPlex bare-metal box + the established AHCI bare-metal runbook (the same path used for the 2026-06-03 TLB-shootdown smoke).
**Current `main` tip:** `a65b85b` (BUILD-5 Embedding merged at `a7580d2`).

---

## 1. Objective

Prove the CPU **inference path runs correctly on real hardware**, not just QEMU. The 2026-06-03 OptiPlex run already metal-proved the *foundation* (real SMP at 12 CPUs, real NeoFS disk I/O = the `.neom` load path, real MM/TLB under load) — but it ran the `phase3_ahci_neofs_mount_and_readwrite` storage scenario, so **no AI code executed**. This dispatch runs `infer_transformer_forward` on metal to cover the one un-proven layer: `SYS_INFER_RUN` + the soft-float ops.

Deliverable: a clean `infer_transformer_forward` PASS on the OptiPlex, with captured evidence (serial log + extracted `NEO_LOG` + hash), feeding the Phase 5 close report.

## 2. Why a small build is needed first

`infer_transformer_forward` is **not yet a `--bare-metal-mode` citizen**. xtask gates `--bare-metal-mode` to scenarios that have an on-disk `NEO_LOG` sink + disk pairing — currently only the AHCI/NVMe NeoFS exemplars. Running `cargo neo-test infer_transformer_forward --bare-metal-mode` today is rejected. So step 1 is to wire it in, mirroring the AHCI exemplar.

### Touchpoints (verified by Cerebro)
Both in `tools/xtask/src/main.rs`:

1. **`bare_metal_log_sink_target_feature(scenario_name)`** — currently:
   ```
   "phase3_ahci_neofs_mount_and_readwrite" => Ok("neo-test-log-sink-target-nvme"),
   "phase3_nvme_neofs_mount_and_readwrite" => Ok("neo-test-log-sink-target-ahci"),
   _ => Err("--bare-metal-mode is only supported for the AHCI/NVMe NeoFS readwrite exemplar scenarios, ...")
   ```
   Add an `"infer_transformer_forward" => Ok("neo-test-log-sink-target-…")` arm. Pick the log-sink controller that does **not** collide with the disk the model is loaded from (the AHCI exemplar puts the model on AHCI and the log sink on NVMe, or vice-versa — keep the model disk and the log-sink disk on distinct controllers).

2. **`prepare_bare_metal_storage_disks(scenario_name)`** — add an `"infer_transformer_forward"` arm that stages:
   - a **NeoFS disk carrying the transformer model**, reusing the *same* fixture + NeoFS staging the QEMU `infer_transformer_forward` scenario already uses (find its `prepare_valid_transformer*` fixture + `prepare_neofs_storage_disk_with_model` call in the scenario's QEMU disk setup and reuse it), placed on the controller the OptiPlex actually has (AHCI — the proven path), and
   - a **`prepare_log_sink_storage_disk(...)`** disk for the `NEO_LOG` readback.
   Mirror the AHCI exemplar's device-vector shape.

This is **test-infra only** — no kernel logic changes. The kernel already emits the scenario's `NEO_TEST` PASS/FAIL frame via `serial_println`; the log-sink target feature is what mirrors that frame into the on-disk `NEO_LOG` region for readback.

## 3. ISC

- **ISC-1 — wiring.** `infer_transformer_forward` accepted by `--bare-metal-mode`: log-sink target feature + disk pairing added; `cargo neo-test infer_transformer_forward --bare-metal-mode` builds the bundle without the "only supported for AHCI/NVMe" rejection. Model disk and log-sink disk on distinct controllers.
- **ISC-2 — QEMU regression.** `infer_transformer_forward` still passes in normal (QEMU) mode at `NEO_SMP=4` — the wiring must not perturb the existing scenario.
- **ISC-3 — metal run.** Boot the bundle on the OptiPlex (AHCI runbook), COM serial cable attached.
- **ISC-4 — evidence.** Capture the serial log and the extracted `NEO_LOG`; record the extracted-log sha256 (as the TLB smoke did).

## 4. Pass criteria (the Gate-2 bar — all three)

- [ ] **Boots clean** — all APs up (`smp: AP cpu_id=N ready` for each), and **no `tlb_shootdown.rs` or any kernel panic** on serial through model load + inference.
- [ ] **`infer_transformer_forward` PASS via `NEO_LOG` readback** — the forward-pass output bytes match the scenario's oracle **on real hardware**. This is the actual proof: `SYS_MODEL_LOAD` (disk) → `SYS_INFER_RUN` → MatMul/Softmax/LayerNorm/Gelu/… → correct output, executed on metal.
- [ ] **Evidence captured** — serial log + extracted `NEO_LOG` + sha256, for the Phase 5 close report.

*If the output bytes diverge from the oracle on metal:* that's a real finding (deterministic no-XMM soft-float should be bit-identical metal-vs-QEMU). Capture the actual vs expected bytes and bring it to PM — do not adjust the oracle.

## 5. Close gate

- `git fetch origin && git rebase origin/main` before close (rebase onto current `main`); re-run host-checks after.
- Host-checks green (`cargo fmt --all --check`, `cargo test --workspace --exclude neoplutarch-kernel`, `cargo clippy --workspace --exclude neoplutarch-kernel -- -D warnings`).
- ISC-2 QEMU regression check passes.
- The three Gate-2 pass criteria met, with evidence.
- Push to your **feature branch, not `main`** — PM merges via `scripts/pm-merge-check.sh` + `--ff-only`. Report the metal PASS + evidence hash; that feeds directly into the Phase 5 close report (Gate 3).

## 6. Scope guard

- This is the inference path's metal sign-off **only**. Do **not** widen Embedding, add attribute parsing, or touch kernel op logic. Pure test-infra wiring + a hardware run.
- One scenario (`infer_transformer_forward`) is sufficient — it exercises the full op pipeline end-to-end. You may optionally also bare-metal `infer_embedding_forward` if it's cheap once the wiring exists, but it is not required for the gate.
