<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE: `presents=0` during Arthur's generation (A6–A7), and what A8 should show (S1, 2026-10-26)

**Question (PM, after A7).** The overlay's `time` line still reads `shown=- presents=0 gap=<the whole
generation>` after the `think` fix (#126, #131). Is the desktop frozen while Arthur generates, or do the
counters miss the WM's presents? And why is `think` negative (`think=-4ms`)?

**Answer, from the code: the counters are right, and they do not show a freeze. They show that nothing
asked the WM to draw.** Whether the WM *could* have drawn is not on the overlay yet; A8 can tell, as
below.

## What the counters count

`kernel/src/ai/prefill_perf.rs`, fed by `watchdog::note_syscall` on every syscall entry:

- **`presents`, `shown`, `gap`**: `SYS_FB_PRESENT` calls made *after* the generation began
  (`GEN_BEGIN_TSC`) and before it ended. That is the syscall the WM presents with
  (`desktop_session.rs` `commit_present` → `sys_fb_present`), so the counter does see the WM's presents.
- **`think`**: Arthur's own last surface commit before the generation, relative to its start. Since #126
  (and #131) it is read from Arthur's pid's slot, so the WM's output commit no longer hides it.

## Why `think` is negative

`think=-4ms` means Arthur's "Thinking…" commit came **4 ms before** `sys_generate` began. That is the
expected order (`main.rs` `run_desktop_session`):

1. Arthur writes "Thinking..." and calls `commit_next()`. The WM copies the band, **presents it**
   (`commit()` → `scene.present`), and only then replies. So the "Thinking…" frame is on screen before
   Arthur's syscall returns.
2. Arthur then calls `respond_to_input` → `sys_generate`. `GEN_BEGIN_TSC` is stamped here.

A negative `think` is the healthy value. `think=-` would mean Arthur committed nothing before the
generation; a positive one would be a bug.

## Why `presents=0` is not by itself a freeze

During a generation Arthur is inside one `sys_generate` call. It commits nothing until the answer
(there is no streaming), so the WM has exactly two reasons to present:

- **the pointer moved** (window view: `pointer_tick` moves the sprite, damage = old ∪ new rect, then a
  present);
- **a minute boundary** (`desktop-clock` repaints the readout).

An 8.4 s generation with the hands off the pad and no minute boundary gives `presents=0` with a
perfectly live WM. A7's `answer=+3ms` points the same way: the WM presented Arthur's answer 3 ms after
the generation ended. A WM that had been starved for 8 s would have had to be scheduled first.

**Why a freeze is not expected from the code either.** The generation runs on CPU 0 (`gen=c0`), where
the WM also runs. But the forward runs at IF=1 on the inference stack, and its owner thread is
preemptible: `inference_stack_shootdown` and `inference_preemptible_liveness` prove a co-runner on the
same CPU runs during a forward. So the WM should get its 10 ms timeslices, and the pointer should move,
only slower.

## Discriminator for A8 (one photo)

During the next long generation (ask something that takes several seconds), **move the pointer
continuously until the answer appears**, then photograph the overlay:

| Overlay | Cursor during the generation | Meaning |
|---|---|---|
| `presents` > 0, `gap` well under the generation | moved | Live. `presents=0` before was "nothing to draw". |
| `presents=0`, `gap` = the whole generation | did not move | **Real freeze**: the WM got no CPU, or its pointer route stalled while c0 generated. |
| `presents=0` | moved | Impossible with the code above; would mean a present path the counter misses. Report it. |

If A8 shows the freeze row, the next step is the c0 schedule during a forward: the BSP's matmul partition
and barrier waits (S4's `par_matmul`), and whether the WM's runnable thread is passed over while the
inference-stack owner holds c0. A cheap addition then would be a "pointer records delivered during the
generation" count beside `presents`, which splits "input arrived but no present" (WM starved) from "no
input" (pad idle).

## The UX point that remains either way

Even with a live WM, the window shows only "Thinking…" for the whole generation (A7: 8.4 s), because the
answer arrives in one piece. Showing tokens as they come needs streaming. That in turn needs a kernel
generation session that keeps the KV cache between chunks: the separate lane already proposed in #108.
