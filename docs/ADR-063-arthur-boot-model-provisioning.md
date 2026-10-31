<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-063: Arthur Boot Model Registry and Boot Provisioning

## Status

Accepted for the `arthur_boots_thinking` QEMU plumbing and switch proof.

## Context

Arthur's real-infer path loads a NeoFS model through `sys_model_load` and then
calls the existing model-backed reply path. The boot image must not carry models
as Limine modules because that forces the payloads into boot-time RAM. Models
belong as NeoFS files and the normal build must stay lean: no model staging, no
`arthur-real-infer`, and dummy echo behavior.

At acceptance time, the native `.neom` and NeoFS substrate was still
fixture-sized. NeoFS v1 model reads used eight direct blocks, `neomc` still had
no GGUF importer, and the `.neom` validator capped native model files at 64
MiB. The local Qwen2.5-0.5B-Q4_K_M GGUF was 491,400,032 bytes, so production
Qwen brains required a later large-model provisioning slice: a real userspace
GGUF importer/output path plus a large NeoFS model-file representation.

ADR-064 resolves the NeoFS representation half by adding direct/single/double/
triple-indirect model-file reads and mkfs staging for `.neom` payloads plus
adjacent `.vocab` sidecars. The remaining production blocker is converting and
validating real model artifacts without checking blobs into git.

## Decision

Arthur now carries a small registry:

- `1.5b` -> `/neofs/model-qwen2.5-1.5b.neom` as the default model.
- `0.5b` -> `/neofs/model-qwen2.5-0.5b.neom` as the fallback model.

The `/model` slash command lists that registry and switches between entries by
loading the requested model before dropping the previous handle. If the requested
name is unknown or the load fails, the previous model remains active and Arthur
reports the failure.

`arthur_boots_thinking` is an opt-in QEMU scenario. It builds `neo-arthur` with
`arthur-real-infer`, generates valid `.neom` fixture models with `neomc`, bakes
both production filenames into NeoFS through `neofs-mkfs --extra-model`, and
keeps the legacy `/neofs/model.neom` entry for older model tests. The scenario
proves boot -> default NeoFS model load -> `sys_infer_run` -> model-backed reply
-> `/model 0.5b` -> fallback model load -> second model-backed reply through the
same userland Arthur path.

The default ISO path remains unchanged. Without the scenario/build feature, no
model file is staged and Arthur keeps dummy echo behavior.

## Consequences

- The QEMU proof exercises the real `.neom` model-object, inference syscall, and
  live drop/reload path without checking a model blob into git.
- It does not claim the 0.5B or 1.5B production brains have been converted into
  `.neom` files or bundled. ADR-064 supplies the NeoFS large-file staging path;
  model conversion and artifact validation remain separate.
- The registry names and NeoFS paths match the production decision: Qwen2.5-1.5B
  default, Qwen2.5-0.5B fallback.

## Amendment 1 (2026-10-20, B1): the 0.5B as a pinned, reclaimed boot module

**Supersedes, for the 0.5B on a fresh system only,** this ADR's rule that the boot
image must not carry models as Limine modules. Ratified by
`Plans/DECISION-2026-10-19-arthur-first-boot-model.md` (D1, D4, Amendment 1),
after `docs/THINK-2026-10-19-arthur-first-boot-model.md` Q1 found the kernel has
no USB mass-storage driver, so a model on the USB stick can only reach a fresh
system through the bootloader.

The original objection was boot-time RAM: a module stays resident for the life of
the kernel. B1 removes it:

1. **Reclaim (B1a).** After the model is built, the module's whole pages are
   admitted to the frame allocator as a separate zone. The pages are planned from
   the module's exact range against the memmap, the executable and every other
   module, and anything unsafe is retained (`mm/module_reclaim_logic.rs`,
   `BuddyAllocator::admit_external_region`). The module registry tombstones the
   model module when it is taken, so no lookup can read it after the reclaim
   (`boot_modules.rs`, CP-62).
2. **No staging copy (B1b).** `sys_model_load("/boot/models/<file>")` builds the
   model straight from the module's bytes. The 0.5B heap tier drops its 463 MiB
   staging buffer under `model-boot-module` (768 → 305 MiB). The forward pass's own kernel-heap use is small (quantized weights are not dequantized to f32 buffers; the KV cache is ~13 MiB at Arthur's 544-token maximum), and the census records the real high-water on silicon.
3. **Integrity (OT-73).** The kernel pins the `.neom` BLAKE3 per module
   (`boot_model_logic.rs`) and hashes every byte before any byte is used. A
   mismatch builds nothing and returns `EBADMSG`; an unpinned module is refused
   before it is taken. Limine's own `#blake2b` path suffix is not used: it would
   panic the boot on a mismatch (`hash_mismatch_panic` defaults on in Limine
   8.6.0), where the kernel check degrades Arthur with a message.
4. **Arthur.** Under `arthur-boot-module` the registry's 0.5B entry is
   `/boot/models/model-qwen2.5-0.5b.neom` and the vocab is read from the
   read-only `/boot/vocab/`. A missing, damaged or failed model is a named state
   with one sentence (THINK Q4); Arthur never exits for it.

**Unchanged.** The 1.5B and larger tiers still come from NeoFS. Without
`model-boot-module` the kernel, the heap and Arthur are exactly the previous
layer (kill-switch). The tier rule and the runtime heap remain B2.

**Evidence.** `arthur_boot_module_answers`, `arthur_boot_module_damaged` and
`arthur_boot_module_absent` (QEMU, hermetic fixture, no NeoFS disk), the host KATs
in `tools/module-reclaim-logic-tests`, `tools/heap-policy-logic-tests` and
`tools/arthur-logic-tests`. The acceptance bar is the OptiPlex run with the real
model (`docs/B1-boot-module-model.md`).

