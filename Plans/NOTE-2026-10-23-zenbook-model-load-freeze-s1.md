<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE: the Zenbook ask-freeze, root causes and fix (S1, 2026-10-23)

**Lane:** `claude/zenbook-model-load-freeze`. **Diagnosis read first:** S2's
`Plans/NOTE-2026-10-22-zenbook-ask-freeze.md` (`claude/zenbook-ask-freeze` @ `cc7d5796`).
**Pitfall:** CP-74.

## The two problems

**(a) The load runs at IF=0.** S2 showed this. `sys_model_load` of `/boot/models/…` ran
`boot_modules::load_model` inside the syscall:
- BLAKE3 over the whole module;
- the model build, which hashes every weight again and copies it.

Nothing re-enabled interrupts, so CPU 0 took no tick and no xHCI MSI, and the WM and the pointer stopped.

**(b) Why it never finished on the Zenbook.** The overlay read
`cpu0 ticks/s=0 | model load-build weight 0/0` with Arthur 29 s into syscall `0x110`.
- **`load-build` means the hash had returned and matched.** The watchdog branch sets that phase after
  the digest check.
- **`weight 0/0` means the build never reached its first weight.** Everything before that weight is
  bounded metadata parsing (`validate_neom`, the config derivation, the tensor names). There is no
  loop over the weight bytes.
- **It is not BLAKE3 speed.** The kernel's BLAKE3 uses the prebuilt SSE2 assembly backend (`blake3`
  build output: `blake3_sse2_ffi`; the target has SSE2 statically).
  - In-kernel, `boot_model_load_stack` hashes about 560 MB in total (485 MB plus the smaller sizes),
    and the whole QEMU run took 16 s *under TCG emulation*, boot included. Silicon is far faster.
  - The portable Rust backend would be slow at opt-level 0: 15 MiB/s on the host, about 30 s for 485 MB.
    The kernel does not use it.
- **It is the stack.** Kernel thread stacks are 32 KiB `Box<[u128]>` in the heap, with **no guard page**
  (`sched/thread.rs`). `blake3::hash` recurses about log₂(n ÷ 1 KiB) deep. Measured on the guarded
  inference stack (`boot_model_load_stack`, new). Each depth is one `blake3::hash` over an owned,
  fully written heap buffer of that length:

| Input | BLAKE3 depth, opt-level 0 (`main`) | Fits 32 KiB? | Opt-level 3 (this branch) |
|---|---|---|---|
| 4,144 B (the hermetic fixture) | 8,368 B | yes | 4,800 B |
| 382,464 B (the KAT model) | 18,560 B | yes | 13,440 B |
| 8 MiB | 28,144 B | yes | 21,168 B |
| 64 MiB | 33,840 B | **no** | 25,520 B |
| 485,489,152 B (the real 0.5B) | **39,536 B** | **no** | 29,808 B |

The depth is the inference stack's watermark: the deepest byte written below the stack top during the
call, interrupt frames included. It is not a trace of every stack pointer, and it is not the whole
syscall's stack use. A depth over 32 KiB shows the 32 KiB box is too small. A depth under 32 KiB does
not show that the whole syscall fits.

At the real size, the hash alone runs past the 32 KiB box, before the syscall's own frames are counted.
The overrun writes into whatever heap memory lies below the stack. The next heap allocation (the build's
first `Vec`) then meets corrupted allocator state at IF=0. That matches `load-build weight 0/0` that
never moves, and the 15-minute freeze.
- **Why the OptiPlex survived:** a different heap layout under the stack.
- **Why QEMU never caught it:** every QEMU scenario's model is under 400 KB.
- **What would falsify this:** an overlay on the fixed image that still stalls in `load-build` with the
  load on the inference stack. That stack has a guard page, so an overrun there is a named `#DF`, not a
  silent hang.

## The fix

1. **`boot-model-preload` (kernel; in the product image `xtask iso --with-model`).**
   - `boot_modules::preload()` runs once in `main.rs`: after the scheduler, the APs, `boot-neofs` and
     the matmul thread count, and before userland.
   - For each pinned, present model it runs `load_model` on the **inference stack**: 256 KiB, a guard
     page, a canary and a high-water mark, and the closure runs at **IF=1**. It keeps an
     `Arc<Model>`.
   - `sys_model_load` of `/boot/models/…` then hands out a handle to that `Arc`
     (`install_shared_model_handle`). Every Arthur gets the same model, instantly.
     - On `main`, a second Arthur got `ENOENT`, because the first one consumed the module.
   - **B1c F8 kept:** a model whose pinned vocab is absent or fails its pin is left untaken
     (`BOOTMODEL preload … skipped=vocab`), exactly as Arthur's `compose` leaves it.
   - **The serial and boot-console line** `BOOTMODEL preload path=… ok= errno= stack_depth= ticks=`
     records the load's stack use and the ticks taken during it. The boot console is on screen until
     the desktop, so the PL sees it.
   - **Kill-switch:** without the feature, `main.rs` does not preload, `preloaded()` does not exist, and
     `sys_model_load` loads in the syscall exactly as before. Every existing scenario builds without it.
     That is a rollback, not a fix: with the feature off, the full-size load is still exposed (IF=0, on
     the 32 KiB thread stack). Turning it off is also not bit-for-bit the old kernel, because the BLAKE3
     opt-level and the WM polling (`desktop-pointer`) are not behind this feature.
2. **WM (`neo-init`, `desktop-pointer`):** between Arthur's `'G'` and `'M'` (Arthur loads its vocab and
   model there), the WM polls `sys_try_recv` and ticks Home's pointer (`poll_control`), instead of a
   blocking receive. Without `desktop-pointer` it is the blocking `recv_control`, as before.
3. **`[profile.dev.package.blake3] opt-level = 3`:** BLAKE3's tree driver is Rust. Optimizing it shrinks
   its frames; it does not change the backend.

## Lock safety of a load at IF=1 (the AAG item)

The build takes:
- `ARENA`, `DEDUP` and `REGISTRY`: plain spin locks (`SwitchMutex`), with no interrupt masking;
- the frame allocator and the heap, which mask interrupts themselves (`with_frame_alloc`,
  `IrqSafeHeap`).

A holder preempted at IF=1 deadlocks only a waiter that spins on the same lock **at IF=0 on the same
CPU**, for example a syscall. The preload finishes before any user process exists, so no syscall can be
that waiter. The kernel threads alive at that point (USB port service, boot-NeoFS) take none of the
three locks.

## Evidence (QEMU, local, `NEO_SMP=4`, one run at a time)

**The stack (`boot_model_load_stack`).** BLAKE3's depth at 485,489,152 B:
- 39,536 B at opt-level 0 (`main`);
- 29,808 B at opt-level 3 (this branch).

Opt-level 3 alone still leaves under 3 KB for the rest of an in-syscall load on 32 KiB, and the load's own
frames add more than that (22,209 B for the whole load against 18,560 B for the hash alone, on the KAT
model at opt-level 0). The guarded inference stack is the fix; opt-level 3 is margin.

**How the probe gets its input (review F1).** The first version hashed HHDM(1 MiB) onward, which is RAM
the probe did not own. In a valid layout that range covers live kernel data that the timer ISR writes
during the hash, so it broke `slice::from_raw_parts`'s contract. The probe now allocates one 485 MB
heap `Vec` (the scenario implies `real-lm` for the 768 MiB heap tier), fills it with a single
`write_bytes`, and hashes prefixes of it. No other code has its address. The 64 MiB and 485 MB
figures did not move; the two smallest moved slightly at opt-level 0 (9,072 → 8,368 B,
18,816 → 18,560 B).

**The tick discriminator.** The load is held between its hash and its build until CPU 0 has taken 20
ticks, or for at most 6·10⁹ TSC cycles.

| Scenario | `if_on` | CPU 0 ticks during the load | Co-runner runs during the load | Other |
|---|---|---|---|---|
| `boot_model_load_ticks` | `true` | 20 | 255,977 | `shared_second_handle=true`, `stack_depth=16640` |
| `boot_model_load_ticks_control` (the old IF=0 load) | `false` | **0** | **0** | — |

**Arthur through the preloaded handle.**
- **`arthur_boot_module_preload`:**
  - `BOOTMODEL preload path=/models/model-qwen2.5-0.5b.neom ok=true errno=0 stack_depth=19816`;
  - then `model_open … source=boot-module`, `ARTHUR model … state=ready` and Arthur's reply.
- **`arthur_boot_module_preload_vocab_damaged`:** `BOOTMODEL preload … skipped=vocab`, then
  `state=damaged` and `model_taken=false`.

**Regression:** the 16 desktop scenarios, the four `arthur_boot_module_*` runs, `real_lm_generate_boot`,
`inference_stack_overflow` and `boot_module_reclaim`. Results are in the PR comment.

## Pitfall-bank consultation (`docs/concurrency-pitfalls.md`)

| CP | Status | What, and the discriminator |
|---|---|---|
| **CP-74 (new)** | Introduced and guarded | A long computation in a syscall at IF=0 on an unguarded 32 KiB stack. `boot_model_load_ticks` and its control, `boot_model_load_stack` |
| **CP-64** | Guarded | A long IF=0 section while shootdown-ready. The load now runs at IF=1, so CPU 0 ACKs shootdowns (the same `interruptible` path the forward uses). `inference_stack_shootdown` is unchanged |
| **CP-65 / CP-66** | N/A | Force-kill of a thread holding `Arc`s. The preload runs on the boot thread (pid 0, never cancelled) before any user process exists. `sys_model_load` with preload only clones an `Arc` |
| **CP-62** | Unchanged | The module lease, reclaim and tombstone. `load_model` is called unchanged, and it takes, hashes, builds and reclaims exactly as before. `boot_module_reclaim` and the `BOOTMOD reclaim` needles |
| **CP-71** | N/A | Log flush at IF=0. The preload's prints run at IF=1 on the boot thread |
| **CP-22 / CP-73** | N/A | IF windows. `poll_control` is userland: a ring-3 spin with IF=1, always preemptible |
| **Lock order (SwitchMutex)** | Argued, not tested | `ARENA`, `DEDUP` and `REGISTRY` are taken at IF=1 during the preload. No IF=0 waiter exists before userland (section above). **This is the cross-family review's first item** |

## Still not covered

- **The real 0.5B on silicon.** The acceptance is the Zenbook run (the PL), with this image:
  - the boot console shows `BOOTMODEL preload … ok=true … stack_depth=` before the desktop;
  - the pointer keeps moving after a question;
  - a second Arthur answers too.
- **The pointer during the handshake** is exercised only by code review and the desktop scenarios'
  existing paths; no scenario moves the mouse during an Arthur start.
- **Boot time grows by the load** (hash and build of 485 MB). That is acceptable at boot. A background
  preload with a waiting `sys_model_load` is a possible follow-up; it needs a cancellable wait (CP-65).
