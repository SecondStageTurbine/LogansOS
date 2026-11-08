<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# B1: the 0.5B as a boot module — run plan, failure bank, trap-mining

**Dispatch:** `Plans/DISPATCH-2026-10-19-b1-arthur-boot-module-model.md` (with Amendment 1 in
`Plans/DECISION-2026-10-19-arthur-first-boot-model.md`). **Design:** `docs/THINK-2026-10-19-arthur-first-boot-model.md`.
**ADR:** ADR-063 Amendment 1. **Split:** B1a (memory core, #70) and B1b (this model path), per
`Plans/NOTE-2026-10-19-b1-split-proposal-s1.md`.

## What runs where

| Gate | What it proves | How |
|---|---|---|
| Merge (QEMU, hermetic) | The source, the pin, the reclaim, and Arthur's three states | `arthur_boot_module_answers`, `_damaged`, `_absent` on the neomc fixture staged under the 0.5B name, no NeoFS disk |
| Acceptance (OptiPlex) | The same, with the real 485 MB model on silicon from the USB | the `arthur_boot_module_answers` bundle built with the real model, below |

The kernel pins both digests in a test build (`boot_model_logic::TEST_PINS`). A real model staged on a
test run is therefore still held to the real pin (`a32e4a1b…1ccafe`). A product build pins only the
real model.

### Building the OptiPlex image

**The product image (B1c).** This is the 0.1 USB: the normal desktop with an Arthur that thinks.

```
cargo xtask iso --with-model <path to model-qwen2.5-0.5b.neom, with .vocab beside it>
```

- The kernel is `desktop` + `atrium-pointer-forward` + `real-lm` + `model-boot-module`, and the userland
  is the full desktop with Arthur's first-boot intro (`desktop-intro`, #75). Only the product pins apply, so only the
  real model and the real vocab load.
- Arthur reads its model and vocab from boot modules.
- xtask refuses files that are not the pinned sizes (485,489,152 and 7,990,862 bytes) or whose BLAKE3
  is not the kernel's pin (B1c), before writing `boot/neoplutarch.iso`. The kernel checks the digests
  again at boot.

**The instrumented bundle (B1b).** It has a serial breadcrumb on every step, which is useful when the
product image fails in a way its log cannot localize:

```
NEO_STAGE_B_NEOM=<path to model-qwen2.5-0.5b.neom, with .vocab beside it> \
  cargo neo-test --build-only arthur_boot_module_answers --bare-metal-mode
```

This writes `target/neo-test/vmware/arthur_boot_module_answers/arthur_boot_module_answers.iso`, which
goes onto the USB stick. Both images carry:
- the kernel;
- the user ELFs;
- `/models/model-qwen2.5-0.5b.neom`, consumable;
- `/vocab/model-qwen2.5-0.5b.vocab`;
- `/licenses/` (NOTICE, OFL, Apache-2.0).

That is six Limine modules, against the kernel's eight.

The run procedure is `docs/runbook-baremetal-arthur-optiplex.md` (read it with
`git fetch origin && git show origin/main:docs/runbook-baremetal-arthur-optiplex.md`):
- **Detach the SATA model disk**, so the USB is the only source.
- Serial is primary (PuTTY on the PL's PC, saved to the fixed log path).
- The NVMe sink at 256 MiB corroborates.

**History.** B1b shipped only the instrumented bundle, because the real-LM `sys_generate` path was
compiled only under `neo-test-arthur-real-brain-boot`. B1c moved it into the product feature `real-lm`
(`Plans/NOTE-2026-10-20-b1c-product-generation-s1.md`).

## Minimum RAM

These are the terms in the load-time peak. The census lines give each one on a real run.

**Updated for Q8_0 (2026-11-08, `Plans/DISPATCH-2026-11-08-arthur-q8.md`).** Arthur's model is now the Q8_0 import.
Its module and its tensors are bigger than the Q4_K_M figures this section was written with. Both were measured by
`.github/workflows/model-pin.yml` (run 42525785126), whose Q4_K_M control reproduced the old figures exactly
(485,489,152 B; `buddy_bytes=761,761,792`). The table gives the Q8_0 figure first and the Q4_K_M one in brackets.

| Term | Size | Census field |
|---|---|---|
| Kernel heap, mapped up front | 305 MiB (was 768 MiB with the NeoFS staging buffer) | `heap_capacity` |
| Kernel thread stacks | 32 KiB each, as on `main` (B1c keeps them; 256 KiB only in the real-brain test builds) | `frames_in_use` |
| The inference stack (`real-lm`, B1c) | 256 KiB once, plus two unmapped guard pages; every real-LM forward runs on it | `CENSUS … inference_stack_size=`, `inference_stack_high_water=` |
| Primary-zone alignment trim (`MAX_ORDER` 16 under `real-lm`) | Up to 256 MiB − 4 KiB of the largest usable region (64 MiB − 4 KiB at order 14): 192 MiB more in the worst placement, 0 when that region starts 256 MiB-aligned | `[ kernel ] mm: primary zone … trim_pages=` |
| The model module, until reclaimed | 669,799,936 B (639 MiB) [Q4_K_M: 485,489,152 B, 463 MiB] | `BOOTMOD module … bytes=` |
| Tensor copies of the weights | Each tensor is one power-of-two buddy block, so the copies take more than the 463 MiB of weights: `output.weight` alone is 256 MiB, `token_embd.weight` 128 MiB, and the other 289 tensors (≈ 236 MiB of bytes) at most double. `neomc inspect` on the pinned Q8_0 measured **`buddy_bytes=1,197,969,408` (1,142.5 MiB)**: `token_embd.weight` now needs a 256 MiB block too. [Q4_K_M: 761,761,792 B (726.5 MiB), Janus, B1c] | `frames_in_use` delta, `model_before_take` → `model_built` |
| Vocab module (resident, never reclaimed) | 7,990,862 B (7.6 MiB) for the real vocab. The hermetic fixture's is 5,435,213 B; B1b's doc wrongly gave the fixture's size here | `BOOTMOD module … /vocab/…`, `BOOTMODULE digest … bytes=` |
| User ELFs and Arthur's heap | Arthur's 16 MiB vocab read buffer, its parsed vocab, plus the ELFs | `frames_in_use` at `arthur_boot_module_start` |

- **Load-time peak (Q8_0):** 305 + 639 (the module) + 1,142.5 (the tensors, buddy-rounded, measured) + 7.6 MiB
  (the vocab) is about 2.05 GiB, plus kernel and user memory. So a 2 GiB machine no longer loads Arthur's model.
  With the worst-case order-16 trim (192 MiB) and the kernel, Arthur and the desktop, plan on a 3 GiB floor. Every
  machine we run on has far more: the Zenbook S14 has 32 GB, the M710q 16 GB, and the preview VM gives the guest
  4096 MiB. The OptiPlex census is still the measured figure.
- **Load-time peak (Q4_K_M, before 2026-11-08):** 305 + 463 (the module) + 726.5 (the tensors, buddy-rounded,
  measured) + 7.6 MiB (the vocab) is about 1.47 GiB, plus kernel and user memory. The 2 GiB floor leaves about 545 MiB for
  the base, less a worst-case order-16 trim (192 MiB). The OptiPlex census is the measured figure. Before B1 the same machine needed 768 + 463 MiB for the heap and the tensors, plus the module
  that B1 adds. That stacking is Chad's C1, and the right-sized heap removes it.
- **After the reclaim:** the module's 639 MiB (463 MiB under Q4_K_M) returns to the allocator as the admitted
  zone (`admitted_pages`). The steady state is the heap plus the tensors: about 1.42 GiB under Q8_0.
- **The measured figure** is `max(frames_in_use × 4096 + unreclaimed module bytes)` over the census
  lines of the OptiPlex run. The QEMU fixture run cannot stand in for it: its model is 4,144 bytes. The
  hermetic run measured a heap high-water of 1,857,799 bytes against the 305 MiB tier. That is with the
  fixture, so it is not a 0.5B measurement.
- **Expected floor (Q8_0):** about 2.5 GiB when the largest usable region is 256 MiB-aligned; 2.7 GiB in the
  worst placement (the order-16 trim, B1c NOTE section 4). It was 2 GiB / 2 GiB + 192 MiB under Q4_K_M. The
  OptiPlex logs its real `trim_pages`.
- **Falsifier:** the OptiPlex census shows a peak above 2.35 GiB under Q8_0 (1.75 GiB under Q4_K_M). Then the floor moves, and B2's tier
  rule must account for it.

## Failure-mode bank (each discriminator is captured before it is needed)

| # | Likely failure even if B1 is right | Discriminator |
|---|---|---|
| F1 | Limine fails to load the ≈485 MB module (memory-map fragmentation, or a UEFI read error) | Limine's own error on screen and no first serial line. If the kernel starts, `BOOTMOD count=6` and one `BOOTMOD module` line per module (path, bytes, phys), logged before the capacity check and before any module byte is touched. |
| F2 | The firmware or Limine is too slow reading 485 MB from USB, and a watchdog or timeout fires | **Timed externally:** the kernel has no clock before it starts. The PL notes the time from selecting the boot entry to the first serial line (PuTTY's log timestamps, or a stopwatch), and compares it with the QEMU run (132 s whole-scenario under TCG for the fixture). |
| F3a | The reclaim is refused | `BOOTMOD reclaim … result=Err(Plan(<refusal>))` or `Err(Admit(<error>))`. Fail closed: the model still loads and Arthur answers; only the pages stay retained (`admitted_pages=0`). The refusal names the rule (coverage, foreign type, executable or module overlap). |
| F3b | The reclaim admits pages something still uses | This must not happen by construction (lease, tombstone, planner; CP-62). It would show as corruption after the reclaim: a crash or garbage reply after `CENSUS label=model_reclaimed`, or `refused_frees>0`. B1a's `boot_module_reclaim` proves the alloc/write/free/realloc cycle on admitted pages. |
| F4 | A digest mismatch on silicon only (a bad stick, or a truncated `dd`) | `BOOTMODEL digest … got=<hex> match=false`, then `ARTHUR model … state=damaged`. Re-hash `/models/model-qwen2.5-0.5b.neom` on the stick from Ubuntu with `b3sum` to tell the stick apart from the code. |
| F5 | Out of memory at load | `BOOTMODEL load_failed … err=Tensor(OutOfMemory)` and `ARTHUR model … state=load_failed`. The census lines `model_before_take` and `model_built` show which term ran out. |
| F6 | A wait for a NeoFS disk that is absent | The bundle mounts no NeoFS (`neofs_mount skipped source=boot-module`), so this cannot block it. On a product boot with `boot-neofs`, the OptiPlex NVMe gives `boot-neofs: no valid NeoFS superblock; boot continues` (Chad C4: NVMe, not the absent SATA). |
| F7 | More than eight modules | xtask refuses the image (`F7` in the error) before it is written. The kernel logs `BOOTMOD count=… capacity=8` before it would panic. |

## Trap-mining (raw sources, not summaries)

- **Limine 8.6.0 `CONFIG.md`** (raw from `limine-bootloader/limine` at tag `v8.6.0`, the pinned
  version):
  - The path suffix `#<hash>` is **BLAKE2b**, not BLAKE3.
  - `hash_mismatch_panic` defaults to panicking on a mismatch. A panic in the bootloader is a dead
    screen with no kernel log.
  - B1 therefore does not use the suffix. The kernel's own check degrades Arthur with a message instead.
- **Limine 8.6.0 `PROTOCOL.md`:**
  - Only USABLE and BOOTLOADER_RECLAIMABLE entries are guaranteed 4 KiB-aligned and non-overlapping.
  - EXECUTABLE_AND_MODULES entries may be unaligned and may overlap, and are "not authoritative" for
    module addresses.
  - This is exactly what the B1a planner assumes: it starts from the Module feature's exact range and
    retains edge pages.
- **Module size:** `CONFIG.md` and `PROTOCOL.md` document no module size limit.
- **Still to verify (no access from this session):**
  - Dell OptiPlex UEFI behaviour on large USB file reads.
  - Whether the Qwen2.5 repositories ship a `NOTICE` (`assets/licenses/README.md`).
- **Not relevant for B1:** xorriso and ISO 9660 behaviour near 2 and 4 GiB. The 0.5B is 463 MiB; this
  matters only once the 3B ships.

## Pitfall bank (`docs/concurrency-pitfalls.md`)

- **CP-62** (B1a) is the memory and lifetime entry:
  - the lease and tombstone stop a re-borrow after the reclaim;
  - the planner refuses unsafe pages;
  - admitted pages form a separate zone.
- **CP-60**, assert placement: the admit runs inside `with_frame_alloc`, with disjointness checked under
  that lock.
- B1b adds no cross-CPU path: the load runs once, in Arthur's syscall.
- The module registry's take is a CAS, so two concurrent loads cannot both take the module.
