<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE — Zenbook: the screen and pointer freeze after asking Arthur (code-path diagnosis, S2, 2026-10-22)

**Symptom** (image A from `6f09e826`, RETURN `a4119c32`): USB input works. After Enter on a question in Home, the
screen froze and the mouse stopped. Generation is on the CPU (about 3–4 min per answer on the OptiPlex).

**Short answer: the freeze is the model load at `IF=0`, not the generation.** Pressing Enter spawns a fresh Arthur,
which loads the 485 MB boot model inside a single syscall with interrupts masked. CPU 0, which runs every user
process (the WM included) and takes the timer and the xHCI MSI, is dead for the duration of the load. The
generation that follows is preemptible and should leave the pointer alive (§3).

## 1. What runs after Enter (`main` `1772d7e4`)

1. **WM** (`userland/neo-init/src/desktop_session.rs`): `home::read_intent` returns the line, and `arthur_session`
   **spawns a new Arthur** (`spawn_arthur`). The WM then does **blocking** receives for the handshake:
   `recv_request(session)` (create), `recv_request(surface)` (import), `recv_request(surface)` (focus), then
   `recv_control(session, b'G') || recv_control(session, b'M')` (`:1234`), and later `recv_control(session, b'D')`.
   Until `'M'` arrives the WM neither polls the pointer nor presents: only the request loop at `:1325` polls
   (`try_recv_request` + `pointer_tick`).
2. **Arthur** (`userland/neo-arthur/src/main.rs`): creates its surface, sends `'G'` (`:478`), then loads the vocab
   sidecar (`try_load_sidecar`, about 8 MiB) and **`preload_boot_model()` → `self.model()` → `sys_model_load`**
   (`:530–536`, `:2572`), and only then sends `'M'` (`:847`).
3. **Kernel** `sys_model_load_impl` (`kernel/src/arch/x86_64/syscall.rs:2461`) → `boot_modules::load_model`
   (`kernel/src/boot_modules.rs:326`):
   - BLAKE3 of the whole 485 MB module (`:343`);
   - `Model::from_neom_bytes` (`ai/model.rs:95`): for every weight, `share_or_allocate_weight` does a second
     content hash (`ai/sharing.rs::content_hash`, BLAKE3) and copies it into tensor storage (`Tensor::from_desc_and_bytes`);
   - three `boot_census` calls.
   **None of this re-enables interrupts.** The syscall entry masks IF (SFMASK), and unlike the file read path
   (`syscall.rs:5725`, `with_interrupts_enabled`) or the forward (`mm/inference_stack.rs:360`), `sys_model_load`
   never re-enables it. The kernel is a dev-profile build (no `opt-level` override for these crates).

With `IF=0` on CPU 0 for the whole load: **no LAPIC tick** (no preemption, `TICKS` stops), **no xHCI MSI** (the
mouse's reports queue in the controller), and every other CPU-0 thread (WM, port service, boot thread) is stopped.
The 15 APs only run matmul partitions (`run_ap_bootstrap`), never user work, so nothing on screen can move. That is
the freeze. It ends when the load returns; the WM then gets `'M'`, hands over the intent, and shows Arthur's window.

## 2. Prediction for the PL's 5-minute wait (falsifiable)

- The pointer and screen come back **before the answer appears**: when Arthur's window opens (the load finished),
  not when generation ends.
- The answer then streams in while the **pointer keeps moving** (perhaps sluggishly: §3).
- A **second question** in the same Arthur session (typed in Arthur's window, no new spawn) should **not** freeze.
  A new Arthur (after closing it and asking from Home again) loads the model again and freezes again.

**If the pointer stays frozen through the whole generation** instead, this note is wrong about the second half, and
§3's suspects are next.

## 3. The generation itself: why it should not freeze

- `SYS_GENERATE` → `inference_syscall` → `real_lm_generate` → `inference_stack::try_run` →
  **`interruptible` = `with_interrupts_enabled`** (`mm/inference_stack.rs:264, 360`): the forward runs at `IF=1`.
- The timer preempts kernel-mode threads (`timer.rs` → `sched::on_tick` → `yield_now_recording(TimerPreemption)`),
  unless `PREEMPT_DISABLE_DEPTH` is non-zero or a handoff deferral is pending. Nothing on the generate path disables
  preemption (its only user is `process.rs:2281`, spawn).
- `par_matmul` (`threaded-matmul`, in `desktop`): the BSP runs partition 0 and spins on the barrier, preemptible
  (`IF=1`); APs claim partitions from their idle loop with interrupts on.
- The WM's request loop polls `try_recv_request` + `pointer_tick` and is **not** blocked on Arthur during
  generation (Arthur waits for each commit's reply before generating, `showcase_surface.rs::commit_serial`, so the
  WM's `send_reply` never parks on a rendezvous).
- **Cost:** CPU 0 is shared at 100 Hz between Arthur's generation, the WM's spin-poll (desktop has no
  `desktop-park` / `wait-multiplex`, so it never parks: CP-59), the port-service thread and the boot thread. The
  pointer gets roughly every other 10 ms slice: responsive but jerkier than at idle.

Ruled out, with the line that rules it out: inference holding IF=0 (`interruptible`); a lock held across generation
that the WM needs (the inference stack's `Held` and `PAR_BUSY` are inference-only); the HID IRQ path (MSI on CPU 0
is delivered whenever IF=1); the USB port-service thread (sleeps or yields every 25 ticks).

## 4. Fixes (for the PM to route; none is in this note)

1. **Load the model with interrupts on, or not on the click path.** Either:
   - (a) wrap `boot_modules::load_model`'s hash and build in `with_interrupts_enabled` + an inference-style
     owner (an AAG item: the arena, tensor storage and frame-allocator locks it takes must be safe to hold while
     preempted, as `inference_stack` was made safe in B1c); or
   - (b) **verify and build the boot model once at boot**, before the desktop, and let `sys_model_load` of the
     boot path hand out a handle to the built model. Simpler, and it also makes every later Arthur spawn instant.
   - Also add `blake3` to `[profile.dev.package.*] opt-level = 3`, like the other crypto crates, so the hash stays
     cheap however it runs.
2. **WM: never block on a child's startup.** Poll the `'G'`/`'M'`/`'D'` handshake the way the request loop polls
   (`try_recv` + `pointer_tick`), so a slow Arthur start can never freeze the pointer (helps once 1 is done).
3. **Optional:** desktop builds without `wait-multiplex` spin the WM on CPU 0 (CP-59); enabling the park would
   give the generation thread most of CPU 0 while the WM idles.

## 5. Evidence to collect on the next run

- Time from Enter to Arthur's window appearing (the load) vs. to the answer finishing (the generation).
- Whether the pointer moves between those two moments.
- If the serial/NVMe sink is ever available: the `BOOTMODEL take / digest / loaded` and `CENSUS label=model_*`
  lines time-bracket the load.

## 6. Update: the freeze lasted over 15 minutes (fan busy), and the watchdog overlay

The PL waited more than 15 minutes with no answer and no pointer. A load at `IF=0` would have to take that long to
explain it alone, so this is not settled from the code. **The fan is not evidence of a spin:** every AP runs
`run_ap_bootstrap`'s busy-yield loop forever (it never halts), so the fan is busy even at an idle desktop.

**The overlay (feature `freeze-watchdog`, in the product ISO only — not `desktop`, whose scenarios read back every pixel; PR stacked on #93).** The highest-numbered AP redraws three
lines in the bottom-left corner once a second, from its idle loop. It writes pixels straight into the framebuffer,
takes no lock and needs nothing from CPU 0. Reading one photo:

| Line | Field | Meaning |
|---|---|---|
| 1 | `t=Ns cpuK` | Seconds since the overlay started, drawn by CPU K. **If `t` stops advancing between two photos, the drawing AP itself is stuck** (for example in a matmul partition that never ends). |
| 1 | `cpu0 ticks/s=` | CPU 0's timer ticks in the last second: about 100 is healthy. **0 = CPU 0 has interrupts masked** (or its timer stopped): an `IF=0` spin or hang. |
| 1 | `aps alive=A/B` | APs that went round their idle loop in the last second. Fewer than all = some APs are inside long work or stuck. |
| 1 | `matmul jobs=J wait=S` | Forward fork-join jobs so far; `wait=S` = the BSP is waiting on AP partition S right now. **`jobs` frozen with `wait≠0` across photos = the barrier never completes** (a partition never claimed or never finished). `jobs` climbing = the forward is making progress. |
| 2 | `model <phase> weight i/n` | `load-hash` / `load-build` (weight i of n) / `load-done`: where the model load is. |
| 2 | `gen#G step s/m` | The G-th generation, phase `prefill` / `decode` step s of m / `gen-done`. Steps climbing = decoding; stuck at `prefill` = the prefill never finishes. |
| 2 | `in: xhci_irq= kbd= mouse=` | USB interrupts and HID reports. IRQs and reports climbing while the screen is frozen = input arrives but nobody consumes it. |
| 3 | `sys: pP=0xNR@As` | For each process, the last syscall it entered (number) and how long ago. **Arthur at `0x121` (generate) or the model-load syscall with a large age = it is in that call.** The WM with a large age at `0x5` (recv) = the WM is blocked on Arthur. |

Two photos about 10 s apart after the freeze say spinning (counters move) vs deadlocked (nothing moves, `ticks/s`
still 100) vs CPU 0 masked (`ticks/s=0`), and where. QEMU check: over a live desktop it reads `cpu0 ticks/s=72`
(TCG), syscall ages 0 s, `kbd=23`; after the kernel halts it reads `ticks/s=0` and the ages grow.
