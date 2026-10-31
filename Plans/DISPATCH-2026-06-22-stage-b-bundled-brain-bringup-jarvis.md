<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Stage-B — bundle the real brain + boot Arthur on the OptiPlex (the headline finale) → Jarvis

**Date:** 2026-06-22
**From:** PM (Claude)
**To:** Jarvis (owns the forward-path + Stage-A context; just closed OT-30). New branch off `origin/main` HEAD: `codex/stage-b-bundled-brain-bringup-jarvis`. **GATED ON F3 merging** (the shipped image needs the real-vocab path) — do not start until F3 is on `main`.
**Status:** DISPATCHED (F3-gated) — **the headline goal: burn the ISO, boot the OptiPlex, talk to Arthur on the real Qwen2.5 brain.** Every technical risk is now retired: L1/L2 store+serve the real 485 MB `.neom`+sidecar; F1+OT-30 run the mixed-quant forward correctly + in-band (Stage-A: 16-token llama.cpp parity, ~1.56 s/token on host); F3 tokenizes real text; OT-29 made heavy spawn→reap→spawn sound. **Stage-B is the integration + the bare-metal bring-up** — build the shippable ISO that bundles the real brain, prove it end-to-end in QEMU (the full stack, not the Stage-A host harness), then the PL boots it on silicon. **Bare-metal ⇒ this dispatch carries the mandatory blind-hardware failure-mode/discriminator section + a prior-art trap-mining pass.**

## 0. The decision this build makes
Turn "the real brain runs in a host harness (Stage-A)" into "**a booted NeoPlutarch ISO loads the bundled real Qwen2.5-0.5B from NeoFS and Arthur answers a real prompt coherently**" — first in a full-stack QEMU boot (the integration gate), then on the OptiPlex (the PL's bare-metal run, designed to be self-diagnosing on failure). **No model/GGUF/vocab blob in git** — staged from an on-disk path at image-build time (the bundled-model discipline).

## 1. References
```
git fetch origin
git show origin/main:tools/neofs-mkfs/src/main.rs           # bundled-model staging (--extra-model/--extra-file) + L2 large-file staging — bundle the real .neom + the .vocab sidecar
git show origin/main:tools/xtask/src/main.rs                # the ISO/image build + the boot-model scenario plumbing (prepare_neofs_storage_disk_with_payload_files) + the F1/F3 scenario fixtures
git show origin/main:kernel/src/arch/x86_64/syscall.rs      # sys_model_load — the .neom read path (now traverses L2 indirect blocks; NEOM_MAX_FILE_SIZE=2 GiB)
git show origin/main:userland/neo-arthur/src/main.rs        # Arthur boot: model load + (F3) vocab sidecar load + the forward/generation path; ARTHUR_MODEL_PATH / VOCAB_SIDECAR_PATH
git show origin/main:kernel/src/ai/forward_logic.rs         # F1+OT-30 forward (phase9-real-quant); the residency/arena load path
git show origin/main:docs/ADR-063-arthur-boot-model-provisioning.md  # the provisioning decision
git show origin/main:Plans/THINK-2026-06-21-forward-path-real-model-integration.md  # D4 residency/memory at real scale; D5 latency
# Stage-A harness (ai-ops-tests) — the host parity baseline the QEMU boot's reply should be consistent with
```

## 2. ISCs (the build — full-stack QEMU integration)
- **ISC-1 — Bundle the real brain into the shippable image.** Extend the ISO/NeoFS-image build to stage the **real 0.5B `.neom` (485 MB) + the real vocab sidecar** via L2 large-file staging (produce the `.neom` from the source GGUF with `neomc import`; **no blob in git** — stage from an on-disk path at build time). The shipped image ships with **`phase9-real-quant` ON, `arthur-real-vocab` ON** (the kill-switches flip to real for the product image; the fixture defaults stay for the existing scenarios).
- **ISC-2 — Arthur boots on the real brain.** At boot, Arthur loads the bundled `.neom` from NeoFS (`sys_model_load`, traversing L2 indirect blocks), loads the real vocab sidecar (F3), and serves a prompt through the F1+OT-30 forward + real tokenizer + Qwen chat template. Confirm the residency/arena path holds the 485 MB model at real scale (D4: ~700 MB peak for 0.5B — fits) — if the arena cap blocks it, surface it.
- **ISC-3 — Full-stack QEMU integration scenario `arthur_real_brain_boot`.** Boot the **actual bundled ISO** in QEMU (real `.neom` served from NeoFS, not a fixture; real vocab) → Arthur answers a fixed prompt → assert the reply is **coherent + finite + deterministic** (same seed ⇒ same tokens), and ideally **top-1 consistent with the Stage-A/llama.cpp baseline** for the first K tokens (the strongest check — reuse the Stage-A parity tokens). At `NEO_SMP=4`. This is the end-to-end proof that L1+L2+F1+F3+OT-30 compose in a real boot. (CI guest RAM must hold ~700 MB + the image — if that's too heavy for the shared runners, gate a **smaller real-quant model** end-to-end and make the full 485 MB the bare-metal proof; state which.)
- **ISC-4 — Serial breadcrumbs (capture-before-you-need-it — for the bare-metal run).** Emit a serial breadcrumb at **each** boot stage so a bare-metal failure is self-localizing: `neofs_mount ok` → `model_open path=… size=485489152` → `model_load_resident bytes=… ms=…` → `vocab_load tokens=151936` → `prompt_tokenize n=…` → `forward token=0 ms=…` → `decode done tokens=K ms=… per_token_ms=…` → `reply="…"`. Each stage prints before it could hang, so a freeze pinpoints the stage.
- **ISC-5 — standards.** SPDX; `cargo fmt --all --check`; clippy `-D warnings` (kernel blocking) via Docker. No new `unsafe`/invariant (composition of landed pieces). No kill-switch change to the *existing* scenarios (they stay on fixtures; the product image is the new ON config). ADR-063 amendment (Stage-B: the real brain bundled + booted). **No blob in git.**

## 3. Blind-hardware failure-mode + discriminator (the bare-metal OptiPlex run — mandatory)
The bare-metal run has no debugger — serial breadcrumbs (ISC-4) only. Design it conclusive on **pass AND fail**:
- **NeoFS won't mount on the real disk / the 485 MB read fails over the real block device** ⇒ breadcrumb stops at `neofs_mount` / `model_open`; the L2 indirect-block walk over real storage is the first untested-on-silicon path. Discriminator: log the block-read progress + the inode's indirect-level reads.
- **Model won't fit / arena OOM at real RAM** ⇒ stops at `model_load_resident`; log bytes-resident vs the arena cap. (0.5B ~700 MB should fit; confirm the OptiPlex RAM.)
- **Forward too slow on the real CPU** ⇒ `forward token=0 ms=…` reveals the real per-token latency (Stage-A was ~1.56 s/token on the dev host; the OptiPlex CPU may differ — this is the honest first real number; if painful, the `/model` 0.5B is already the floor).
- **Heavy-process spawn instability** ⇒ OT-29 fixed the slab stride bug, but this is its first *bare-metal* heavy-Arthur spawn; if it faults, the OT-29 regression + the slab poison (if armed) localize it.
- **Incoherent reply (correct pipeline, wrong output)** ⇒ the reply prints but is garbage ⇒ compare the breadcrumb token IDs against the Stage-A baseline to tell "tokenizer/template wrong" from "forward wrong" from "sampling wrong."
- **RDRAND/entropy or CPU-feature divergence** (the hermeticity lesson) ⇒ if anything in the path consumes RNG, the real CPU differs from TCG; log it.

## 3.5 Prior-art trap-mining (bare-metal bring-up)
Before the first bare-metal boot, mine the known traps for **booting a from-scratch kernel that mmaps/loads a ~500 MB file off NeoFS on real UEFI hardware** and seed them into the bank: real-UEFI-vs-OVMF memory-map differences (the model-load arena placement), the real block device's sector/DMA constraints for the large sequential read, ACPI/SMP bring-up on the specific OptiPlex, and the framebuffer/serial availability for the breadcrumbs. (Most of the *driver* traps don't apply — Stage-B adds no new device driver; the traps are boot/firmware/storage-scale, not a chip.) Each trap goes in the bank with its discriminator before the run.

## 4. Scope guard
Bundle the real brain + the full-stack boot + the bare-metal bring-up **only**. **Do NOT**: change the forward path (F1/OT-30 frozen — Stage-B *runs* it), the `.neom`/sidecar format (L1), the vocab path (F3), or NeoFS (L2); add a device driver; need F2 (0.5B uses the hardcoded config — 1.5B is a later slice). **No blob in git.** No new invariant/`unsafe`.

## 5. Gate + close (PM)
- **Full-stack QEMU `arthur_real_brain_boot` green** (the bundled real brain boots → coherent/finite/deterministic reply, parity-consistent with Stage-A) — the integration proof. **Kernel-touching ⇒ 4-shard QEMU `workflow_dispatch`.**
- **Bare-metal run = the PL's**, with ISC-4 breadcrumbs + the §3 discriminators making it diagnostic. Expect an **iterative bring-up loop** (the Phase-6 archetype — design every iteration to localize, not just to pass). The RETURN reports the QEMU integration result; the bare-metal result is the PL's run, reported back per iteration.
- **Close-gate: `scripts/worker-close-gate.sh`** (rebase LAST; full set; SPDX; self-stat — **confirm no GGUF/.neom/vocab blob**). RETURN the green push-CI + 4-shard QEMU URLs. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (**OT-28 — the real brain boots; Stage-B bare-metal bring-up loop open with the PL**).

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main`; staged GGUF/.neom stays out of git. See CLAUDE.md "Disk hygiene."

---
## ADDENDUM 2026-06-23 (PM) — decouple the #GP repro from the 485 MB load; scheduler stays untouched

Live status: Jarvis is in an iterative bring-up loop chasing a **#GP in the heavy Arthur forward path**, current hypothesis = **kernel syscall stack too small** (the real-model forward overflows it). Breadcrumbs confirm the path is *healthy but slow* (`vocab_load`/`model_open` reached; `arthur> hello world` printed) — not a hang. The 7-hour cost is the **repro cycle**: every iteration rebuilds the kernel and cold-loads the full 485 MB model *before* reaching the fault.

Two PM directions:

1. **Iterate the #GP fix against a SMALL real-quant model (or a fixture that hits the same syscall/forward stack depth), per ISC-3 — then confirm ONCE on the full 485 MB.** The syscall-stack-size fault is **size-independent**; paying the full cold-load on every cycle is pure tax. Use the dispatch's existing escape hatch (smaller real-quant end-to-end for the fast loop; full 485 MB reserved for the integration confirm + the bare-metal proof). This is the single change that turns the loop from hours to minutes.

2. **The timer-preempt-disable hack must NOT ship — confirmed good call removing it.** Disabling timer preemption (`TIMER_PREEMPT_DISABLED_FOR_STAGE_B` / `debug_disable_timer_preempt_for_stage_b`, cfg `neo-test-arthur-real-brain-boot`) *masks* a scheduling bug instead of fixing the #GP — correctly reverted. **Merge-review checkpoint (PM):** the final Stage-B diff must show `kernel/src/sched/scheduler.rs` **unchanged vs `main`**, with the fix isolated to the **syscall stack size + breadcrumbs**. A surviving scheduler mutation or a cfg-gated preempt-disable is a merge blocker. Syscall-stack-size bump is in-scope bring-up (no new `unsafe`/invariant); if the fix grows into new `unsafe` or an invariant change, surface it before close.

## Relay — what to tell Jarvis (once F3 is on `main`)
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-22-stage-b-bundled-brain-bringup-jarvis.md` (+ `neofs-mkfs`/`xtask` staging, `sys_model_load`, `neo-arthur` boot, ADR-063). **Branch:** `codex/stage-b-bundled-brain-bringup-jarvis` off `origin/main` **after F3 merges**.
- **One-line:** **The headline finale — bundle the real brain + boot Arthur.** Stage the **real 485 MB 0.5B `.neom` + vocab sidecar** into the shippable NeoFS image via L2 large-file staging (produce via `neomc import`; **no blob in git**); ship the image with **`phase9-real-quant` + `arthur-real-vocab` ON**; Arthur boots → loads the `.neom` from NeoFS (L2 indirect walk) + the real vocab → answers a prompt via F1+OT-30 forward + Qwen template. **Gate:** a full-stack QEMU `arthur_real_brain_boot` (the *actual bundled ISO*, real model from NeoFS — not a fixture) → coherent + finite + deterministic reply, **parity-consistent with the Stage-A llama.cpp tokens** (use a smaller real-quant model end-to-end if 485 MB is too heavy for CI guest RAM — full size is the bare-metal proof). **Serial breadcrumbs at every boot stage** (`neofs_mount`/`model_open size=…`/`model_load_resident ms=…`/`vocab_load tokens=151936`/`forward token=0 ms=…`/`decode per_token_ms=…`/`reply=…`) so the **bare-metal OptiPlex** run is self-diagnosing. **Blind-hardware discriminators** (§3): NeoFS-on-real-disk + the 485 MB read over the real block device is the first untested-on-silicon path; arena OOM; real-CPU per-token latency (the honest first silicon number — Stage-A was ~1.56 s/token); first bare-metal heavy spawn (OT-29); incoherent-reply triage via the breadcrumb token IDs. **Prior-art trap-mine** the boot/firmware/storage-scale traps (real-UEFI memory map, large sequential read, OptiPlex ACPI/SMP) — not driver traps (no new driver). **Scope: bundle + boot + bring-up ONLY — no forward/`.neom`/vocab/NeoFS changes, no driver, no F2 dep. No blob in git.** **Close-gate: `scripts/worker-close-gate.sh`; RETURN the green push-CI + 4-shard QEMU URLs + the integration reply; the bare-metal run is the PL's iterative loop.**
