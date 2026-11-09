<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH 2026-11-08: Arthur moves to Q8_0 (J.c.'s decision)

**Seat:** F (Claude). **Branch:** `claude/arthur-q8`, from `origin/main`. **Decision:** J.c., 2026-11-08: "Q8, always
go for the better option." **Evidence:** `docs/THINK-2026-11-08-arthur-quantization-choice.md`, PR #84 (relays
323/333/342).

## The change
Same upstream repo and revision as today, a different file:
- `Qwen/Qwen2.5-0.5B-Instruct-GGUF` @ `9217f5db79a29953eb74d5343926648285ec7e67`, file `qwen2.5-0.5b-instruct-q8_0.gguf`,
  675,710,816 bytes.
- Take its SHA-256 from the repo's LFS pointer at that revision (`git` or a raw `curl`, never a summarizing fetch). It
  starts `ca59ca7f`. The licence file is the same as today's.

1. **Import and pin.** Import with `neomc import` exactly as `scripts/preview/build-in-container.sh` does. Record the
   new `.neom`'s size and BLAKE3 in `kernel/src/boot_model_logic.rs` (`QWEN25_05B`), with a doc comment naming the Q8_0
   source. Keep the module name, so nothing else needs to move.
   - If this container can't download from Hugging Face, add a one-off CI job that downloads, checks the SHA-256,
     imports, and prints the size and BLAKE3. It's network-on, so it can't be a gate job.
2. **Preview build.** Change `MODEL_FILE` and `MODEL_SHA256` in `.github/workflows/preview-image.yml`, and the
   "Model:" line and the `neomc import` text in `scripts/preview/build-in-container.sh`.
3. **Kernel heap.** `HEAP_SIZE_REAL_BRAIN_05B` (768 MiB) was sized for the 485 MB Q4 staging buffer, and
   `HEAP_SIZE_REAL_BRAIN_05B_BOOT_MODULE` subtracts 463 MiB for it. Resize both from the new file size with the same
   headroom rule, and say the arithmetic in the doc comments. Update `tools/heap-policy-logic-tests` to match. The
   kill-switch equality must still hold: the default heap stays 4 MiB.
4. **Memory the VM and the targets need.** Check the preview VM's RAM (`cargo xtask vm-image` / the README's VM
   instructions) and the QEMU scenarios that boot the real model. Raise the RAM where the bigger model needs it, and
   note the Zenbook (plenty) and the smallest target we list.
5. **Outputs that are pinned.** Find every scenario or KAT that pins the real model's output (the boot reply "Hello! How
   can I help you?", any golden token IDs) and re-run it. Q8_0 gave the identical boot reply in the ladder; any other
   pinned text that changes is re-pinned with the new output quoted in the PR.
6. **The drift KAT.** Leave #84's KAT as the Q4 record. Note in the PR that a Q8 re-pin follows once #84 lands.
7. **Docs.** README's Status line says Q4_K_M; change it to Q8_0. Add a line to the THINK: "Decided: Q8_0 (J.c.,
   2026-11-08)."

## Gate
Kernel-touching (the heap policy and the pin), so it's full-alone:
- `scripts/worker-close-gate.sh`;
- a green 4-shard PR run;
- the personal-details scan before every push;
- a PR body with the before/after file size, the heap arithmetic and the RAM changes.

The PM routes the cross-family review. Post `PM:` on the PR with the head and the run.

## Discriminators
- A wrong pin: the product kernel refuses the module. The boot scenario's log shows the digest mismatch.
- The heap too small: the load fails with the staging-buffer allocation error. The census high-water in the log shows by how much.
- A model-output change: the scenario's needle fails with the new text, and it's re-pinned only if Q8's reply is the
  one in the ladder.
