<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK — OT-22 rung 2: more wait interests (fences, futex words; send-ready declined)

**Status:** design, no kernel code. **Author:** S2 session (Claude). **Date:** 2026-10-19.
**Builds on:** `docs/THINK-2026-10-19-ot22-readiness-multiplex.md` (the base THINK) and the rung-1 build
(#58, `SYS_WAIT_ANY` 0x1B5, `kernel/src/sched/wait_any.rs`).
**Gate before code:** cross-family lens (Codex), `Plans/DISPATCH-2026-10-19-ot22-rung2-think-codex-crosscheck.md`.
**Concurrency-core:** yes (new wakers: the present-completion ISR and `futex_wake`). The pitfall bank applies,
and the AAG needs a cross-family lens.

**Revised 2026-10-19 after Chad's Codex RETURN**
(`Plans/RETURN-2026-10-19-ot22-rung2-think-crosscheck-chad.md`):

| Finding | What was wrong | Where it is fixed |
|---|---|---|
| F1 High | `FENCE_IDLE` read "idle" with older flips still queued on the full-ring path | Now gated on a per-buffer `flips_in_flight` count, with a negative leg (§3, §7) |
| F2 High | The extension array's user pointer had no validation or copy step | Specified (§5.1) |
| F3 Medium | A CPUID check was used as a probe for a potentially fatal `RDMSR` | Now requires `rdmsr_safe`, or F2 is omitted (§8) |

Two smaller corrections from the same review:

- the futex contract's wording, plus the stale-mapping case, now tied to CP-61 (§5.3);
- the MMIO count (§8).

The other verdicts held: §1, §2 (with changes), §4, the count rule, and the Q5 answer.

**Numbering.** The base THINK called the first build "rung 0" and listed its follow-ups as "rung 1" (more
interests) and "rung 2" (an asynchronous input producer). The build shipped as **rung 1**, so this document
uses that count:

- rung 1 = #58;
- **rung 2 = this THINK** (more interests);
- rung 3 = the asynchronous input producer;
- rung 4 = the readiness ring.

## For the PL (six lines)

1. Rung 1 lets the desktop sleep on messages, keys and timers. This rung asks which *other* waits still spin.
2. Today, none in a shipping build: presents complete on the spot, so a frame never waits. The one spin
   appears when presents go to the display hardware (the "hardware flip" path being built for the GPU box).
3. Fix that spin inside the existing "wait for my frame" call: it sleeps until the frame lands, with no new
   interface for programs.
4. Two more "wait on this too" options are designed (a frame landing; a shared memory word changing). Build
   each only when a program needs to wait on it *and* something else at once.
5. A third, "wake me when I can send", is turned down: with how the desktop receives messages it would leave
   two programs each waiting for the other.
6. Power (the open question from the base THINK): the 100-per-second check is fine for now. The processor
   already wakes 100 times a second for the timer whatever we do. It stops being fine only if the kernel
   ever learns to stop the timer when idle. A test that would prove this wrong is named in §6.

## 1. What still spins once rung 1 lands (measured against the code)

| Wait | Where | Spins today? | Why / why not |
|---|---|---|---|
| Present fence | `SharedPixelBuffer::fence_wait` (`atrium/buffer.rs`), called from `SYS_PRESENT_FENCE_WAIT` by every desktop frame loop (`desktop_session.rs`, `dw1_desktop_wm.rs`, `arthur_compositor.rs`) | **Only under `PresentMode::HardwareFlip`** | `main.rs` sets `Software`, where `present` `Release`-stores `completed_epoch` before returning, so `fence_wait`'s first check passes. Under HardwareFlip the epoch lands later, from `present_backend::on_present_vblank`: the timer ISR stands in for vblank in QEMU, and Slice 4's Gen9 flip-done ISR is the real one. Until then the waiter runs a `yield_now` loop with IF on. That is a hot spin for up to a tick, because a lone BSP thread's yield returns at once. |
| GPU compute fence | `SYS_GPU_COMPUTE_FENCE_WAIT` | No | It returns the token's sequence. The CPU floor completes synchronously; there is no wait. |
| Futex | `futex::wait_on_key` | No | Already parks (token + guard). The gap is *multiplexing*: a thread cannot wait on a futex word and a channel at once. |
| Send | `Channel::send` | No | Parks `SendBlocked`. The gap is again multiplexing, and §4 shows why it should stay closed. |

So rung 2 has **one real spin** (the HardwareFlip fence) and **two multiplexing gaps** (fence-or-something,
futex-or-something). It has no live consumer for either gap today: the WM's frame loop waits on the fence
alone, and the only futex users are `browser-runtime` threads, none of which wait on IPC at the same time.
That decides the build order (§7).

## 2. 2a — the fence waits by parking, inside `fence_wait` (no ABI change)

The smallest step that ends the one spin. `SYS_PRESENT_FENCE_WAIT` keeps its ABI, its return codes and its
`timeout_ticks` clamp. Only the inside of the `HardwareFlip`-pending branch changes:

1. Check `fence_satisfied(completed_epoch.load(Acquire), wait_epoch)`. Satisfied: return, exactly as today
   (Software mode always ends here, so the kill-switch path is untouched by construction).
2. `register(pid, [fence_key(buffer)], deadline = now + timeout_ticks)`. This is the rung-1 table, with an
   entry tagged FENCE (§5.2).
3. Re-check the fence (CP-14). Satisfied: unregister and return.
4. `park` (the `fired` check under the table guard; the token is installed and the guard released on the new
   stack). The wake comes from the fence fire or from the deadline tick. Loop back to step 1.

**The fire.** `publish_complete` does `fetch_max(epoch, Release)` and *then* calls
`wait_any::fire_fence(fence_key(self))`, behind a `FENCE_WAITERS` count, so an unwatched publish costs one
relaxed load. It runs from the present-completion ISR (today the BSP timer handler; in Slice 4 the Gen9
ISR), or synchronously in `present` on the full-ring I-14(b) fallback.

**Why the order is safe.** The waiter writes (registers, under `WAIT_TABLE`) and then reads `completed_epoch`.
The publisher writes `completed_epoch` and then takes `WAIT_TABLE`. The mutex's total order decides it:

- If the publisher takes the lock after the waiter's registration, it finds the entry and sets `fired`.
- Otherwise its epoch store happened-before its unlock, so the waiter's re-check after its own lock sees
  the epoch.

This is the rung-1 channel argument with `Channel.state` replaced by the epoch atomic.

**What changes in the ISR.** `present_backend`'s module doc promises "the only lock is the ring's
`IrqSafeMutex` leaf". After 2a the ISR takes a second leaf, `WAIT_TABLE`:

- It is taken *after* the ring lock is released, never nested with it.
- It is an `IrqSafeMutex`, so no same-CPU holder can be interrupted by this ISR (CP-37).
- It is not a governance lock (I-14a holds).
- Its critical section is a walk of 16 slots (CP-3).

The doc must be updated in the same build; this is a changed promise, not a new invariant.

**What 2a gives up.** `fence_wait` has two backstops today: the tick deadline, and a yield-count cap "if the
tick source ever freezes". A park has only the first. Every other timed park in the kernel (futex, rung-1
`wait_any`) already rests on the tick alone, and a frozen BSP tick also stops preemption. Even so, this is a
real removal, and it is open question 1. The conservative variant parks only once `TICKS` has advanced past
a boot threshold, and keeps the yield loop before that.

## 3. 2b — `WAIT_KIND_FENCE_IDLE` (4): a fence as one of several items

For a caller that must wait on a fence *and* something else: a future WM that keeps input live while a flip
is in flight, or a compositor juggling several clients' buffers.

- **Item:** the rung-1 8-byte shape, `u32 handle` (a buffer handle; USE meta only, the same rights as
  `SYS_PRESENT_FENCE_WAIT`), `u32 kind = 4`. No epoch field is needed.
- **Ready when** `flips_in_flight == 0 && completed_epoch >= issued_epoch`: no present on this buffer is
  in flight. This is level-triggered. It is exactly "may I draw the next frame?", and it avoids widening the
  item to carry an epoch.
- **Why the count (revised after Chad F1).** The first draft used `completed_epoch >= issued_epoch` alone.
  That is wrong on the full-ring path:
  1. HardwareFlip presents epochs 1-16 and all queue.
  2. Epoch 17 finds the ring full and completes synchronously (`publish_complete(17)`).
  3. Now `issued == completed == 17`, and the predicate says idle while 1-16 still sit in `PENDING`
     holding this buffer. `fetch_max` correctly keeps `completed_epoch` at 17 when they drain later.

  The epoch pair means "the latest present is logically satisfied", not "nothing is queued".
- **The count.** Each buffer gains `flips_in_flight: AtomicU32`:
  - `present` increments it, `Release`, only when it *arms* a flip into the ring; the synchronous fallback
    never touches it;
  - `on_present_vblank` decrements it, `Release`, for each entry it drains, *after* that entry's
    `publish_complete`, then calls `fire_fence`.
- **Waker:** `fire_fence`, after each drained entry's decrement and after every synchronous
  `publish_complete`.
- **Scan:** three atomic loads (`Acquire`), no lock.

One consequence to name: a present issued *after* the scan but before the park makes the buffer busy again.
The wait then parks, the next publish fires it, and the re-scan sees "idle" only once *every* flip in flight
has drained. That is the intended meaning, and a fixed epoch would not give it.

## 4. Send-ready: declined, and why

The base THINK listed "send-ready" as a rung-2 interest. On this channel design it is unsound for the callers
that would use it.

- A rendezvous `send` completes without parking only if the peer is `RecvBlocked`, that is, parked inside
  `sys_recv` (`ipc/mod.rs`, `send`'s first match arm). So "send-ready" can only mean "the peer is parked in
  recv".
- Every desktop receiver now waits with `SYS_WAIT_ANY` + `SYS_TRY_RECV` (rung 1). It is **never**
  `RecvBlocked`, because `try_recv` does not park.
- So a sender waiting for send-ready on the WM's surface channel would wait on a state the WM never enters.
  Meanwhile the WM waits for recv-ready, which needs the sender to be `SendBlocked`, a state the sender
  avoided by waiting instead of sending. **Both park until a deadline: CP-57's deadlock, rebuilt from the
  other side.**
- The sender does not need it. A plain `send` parks `SendBlocked`, which *fires* the receiver's rung-1
  recv interest (`fire_waiters` in `send`). The receiver wakes, `try_recv` completes the rendezvous, and the
  sender resumes. The case send-ready was meant for is a sender that must also watch input while its message
  waits. That calls for a buffered mailbox (the DI-1b configure-mailbox shape, CP-57) or a combined
  "send, and wait on these too" operation. Both are their own design.

**Kind 6 is reserved, unbuilt.** Falsifier for this verdict: a receiver that legitimately parks in
`sys_recv` *and* a sender that must multiplex. Neither exists in the tree.

## 5. 2c — `WAIT_KIND_FUTEX` (5): a futex word as an item

For a threaded (`browser-runtime`) process whose event loop waits on IPC *and* a word another thread
updates: an eventcount, a completion counter.

### 5.1 ABI: no renumbering, one extension array

A futex item needs a `u64` user address and a `u32` expected value, which do not fit the 8-byte item. Options:

| | Shape | Verdict |
|---|---|---|
| (i) | A v2 item format (16 or 24 bytes) chosen by a flag | Rejected: two item layouts, with every existing caller re-validated |
| (ii) | A new syscall `SYS_WAIT_ANY2` | Rejected: a second entry point with the same body |
| (iii) | **Kind 5 items index a side array of 16-byte records passed in the 4th/5th syscall args** | **Chosen** |

- **Call:** `SYS_WAIT_ANY(items, n, deadline_ns, ext, ext_n)`. Rung-1 callers pass three arguments. The
  kernel reads `ext`/`ext_n` **only if some item has kind 5**, so a rung-1 binary's leftover registers are
  never interpreted.
- **Validation, in this order (added after Chad F2):**
  1. Copy the ordinary items into the kernel (rung 1's path).
  2. Only then look for kind 5.
  3. Reject `ext_n > ITEMS_MAX` (EINVAL).
  4. Compute `ext_n * 16` with a checked multiply.
  5. `validate_user_buf(ext, bytes)`.
  6. Copy the records **once**, under SMAP, into a fixed kernel array.
  7. Decode and validate only the copy.

  An unchecked `ext` would make the kernel read through a user pointer; ring-0 faults are fatal here.
  Copying once also closes a user TOCTOU: the records cannot change between the registration and the
  rescans. The decoder is unaligned-safe, so the array needs no alignment. Each decoded `uaddr` is still
  4-aligned, range-validated and translated.
- **Kind-5 item:** `u32 handle` = index into `ext` (EINVAL if `≥ ext_n`), `u32 kind = 5`.
- **Record:** 16 bytes, little-endian:
  - `u64 uaddr` (4-aligned, validated like `SYS_FUTEX_WAIT`'s);
  - `u32 expected`;
  - `u32 flags`, which must be 0; anything else is EINVAL, which keeps room for later flags.
- **Byte oracle** (required: an on-wire layout). A record with `uaddr = 0x0000_7fff_1234_5678`,
  `expected = 0xaabb_ccdd`, `flags = 0` is exactly:
  `78 56 34 12 ff 7f 00 00 dd cc bb aa 00 00 00 00`.
  A host KAT in `wait_any_logic` asserts the decode against these hand-laid bytes, independent of libneo's
  encoder.
- **Unchanged:**
  - kinds 1-3 and their numbers;
  - the 8-byte item;
  - `n ≤ 8`;
  - the ready mask (bit i = item i);
  - deadline semantics;
  - `EINVAL` for an unknown kind. A newer libneo on an older kernel gets EINVAL for kind 4/5 and falls back.

### 5.2 Keys are tagged

Rung 1 stores bare `u64` keys: a channel pointer `| side`. Rung 2 adds fence keys (a buffer pointer) and futex
keys (a physical address). These cannot collide numerically:

- kernel heap pointers have bit 63 set; physical addresses are below 2^52;
- distinct allocations are at least 8 bytes apart, and the low tag bits are under 8.

Even so, "cannot collide" should hold by construction, not by address-space folklore. So each entry stores
`(tag, key)` with `tag ∈ {CHANNEL, FENCE, FUTEX}`, and each `fire_*` matches only its own tag. This costs
one byte per key.

### 5.3 Readiness, waker, and the count rule

- **Ready when** `*uaddr != expected`: the word moved. This is the futex's own wait condition, inverted.
  It is read in the scan with SMAP opened for the one load, in the caller's own address space.
- **Key:** `uaddr` translated to its physical address at registration (as `sys_futex_wait_impl`), with the
  `Arc<Pmap>` dropped before any park (CP-30).
- **Waker:** `futex::wake(key, count)` collects and wakes its `count` real waiters as today. *After*
  dropping `FUTEX_QUEUE` it calls `wait_any::fire_futex(key)`, behind a `FUTEX_ITEM_WAITERS` count. The lock
  order is `FUTEX_QUEUE` → (drop) → `WAIT_TABLE` → scheduler, never nested.
- **The count rule.** A `wait_any` futex item is an **observer**: `fire_futex` fires every observer on the
  key and **never consumes any of `count`**. `futex_wake`'s return value still counts only real waiters, a
  PRESERVED ABI that Rust's rwlock relies on. Otherwise a mutex unlock's `futex_wake(1)` could land on an
  observer that returns for another item and never takes the lock, and the real waiter would stay parked: a
  stolen wakeup.
- **The caller's contract** (wording corrected after Chad's review). An observer is fired only by
  `futex_wake`. A word whose writer skips the wake on its fast path (an uncontended mutex unlock) never fires
  it. Such a wait **may block indefinitely unless every writer wakes or a finite deadline is supplied**; it
  is not merely slow. So a futex item is for **event words whose writers always wake** (eventcounts), not
  for mutexes. libneo documents this; the kernel cannot check it.
- **Ordering (CP-20).** The writer stores the word in user mode, then enters `futex_wake`, which takes
  `FUTEX_QUEUE` and then `WAIT_TABLE` (locked RMWs). The waiter registers under `WAIT_TABLE` (a locked RMW),
  then loads the word. The same mutex-total-order argument as §2 applies.
- **Teardown.** The existing `wait_any::purge_pid` covers it. The table holds plain keys, never an `Arc`
  (CP-30).
- **The mapping can change under the wait: CP-61's class.** A futex item names a *virtual* address, and the
  registration watches the *physical* key it translated to. Rung 1's handle bug (Jarvis rung-1 F1, CP-61)
  has the same shape: a mutable name and a key captured at scan time. An unmap, or a remap of the page,
  leaves a stale key. A spurious fire is absorbed by the level-triggered rescan. A **missed** fire is **not
  bounded when the deadline is infinite**. 2c adopts rung 1's two-sided guard where it can:
  - **Before the park:** the second scan re-translates every `uaddr` and compares the physical key with the
    registration's. A difference means start over, as rung 1's `CapId` check does.
  - **After the park:** there is no `HandleTable`-style chokepoint for unmaps yet. So the contract requires
    **a stable mapping for the life of the wait, or a finite deadline**. A pmap-unmap hook that fires the
    futex keys of unmapped frames would close it mechanically. That is a later decision, with its cost on
    every unmap.
- **Gating:** kind 5 exists only with `browser-runtime` (the futex's own gate) and the new
  `wait-multiplex-futex`.

## 6. Pitfall-bank consultation

| Pitfall | Touched by | Introduce / guard / N-A | Discriminator |
|---|---|---|---|
| CP-14 lost wakeup | 2a, 2b, 2c: three new wakers | **Guarded**: every interest uses rung 1's register → re-check → `fired`-checked park, and each waker publishes *before* it fires (the epoch store; the user's word store) | the fence and futex legs below; the rung-1 race hook (`RACE_SPIN`) reused for a store-and-wake inside the register→park window, over K iterations, with park/fire counters balanced |
| CP-15 / CP-21 stranded wake (EOI-only IPI; busy AP) | 2a/2b when the Gen9 flip-done MSI lands on an AP | **Guarded for latency, not proven for placement**: waiters are BSP-resident (rung 1's assertion stands); an AP-side ISR wakes a BSP token via `wake_thread_token`'s IPI, and the BSP's own tick re-schedules within one tick even if the IPI only EOIs | the rung-1 BSP-residency assertion; Slice 4's bare-metal run must log the flip ISR's CPU (capture-before-you-need-it) |
| CP-16 wake under a foreign lock | `publish_complete` (after the ring lock), `futex::wake` (after `FUTEX_QUEUE`) | **Guarded**: collect under `WAIT_TABLE`, drop it, then `wake_thread_token`; never nested with the ring lock or `FUTEX_QUEUE` | lockdep levels for the two new edges; review |
| CP-22 / CP-28 idle `sti; hlt` | none | **N/A**: the idle loop is untouched; a fence or futex wake reaches an idle BSP as any wake does | existing guards |
| CP-60 lazily captured "kernel" state | the new keys | **Guarded by shape**: keys are captured per wait from the caller's own handle and pmap, stored as plain integers, and never dereferenced. There is no `Lazy`/`Once` in the new code. A reused address can only spuriously wake | review item: no `Lazy`/`Once`/`static` initialised from live CPU or process state in the diff |
| CP-2 / CP-3 (`fence_wait`'s current pattern) | 2a | **Changed**: the IF-on yield loop becomes a park; no lock is held across it; the ISR's new `WAIT_TABLE` section is bounded | the 2a leg plus the kill-switch run |
| CP-30 / CP-31 teardown, token capacity | all | **Guarded** as rung 1: plain keys, purge on death; a full token table returns EAGAIN, and 2a falls back to the yield loop for that wait | rung 1's kill leg, re-run with fence and futex items |
| CP-37 IF discipline | `WAIT_TABLE` from a new ISR | **Guarded**: `IrqSafeMutex` | rung 1's IF witness |
| CP-61 a wait names a mutable slot, registers on what it held | 2c (a virtual address names a physical key) | **Guarded before the park** (the second scan re-translates and compares); **after the park, by contract** (a stable mapping or a finite deadline) until an unmap hook exists | the 2c rebind leg: remap the page between the scans, and the wait must start over |
| CP-57 server-initiated send / mutual wait | send-ready | **Declined** (§4): building it would introduce the mutual wait | none needed while kind 6 stays unbuilt |
| CP-58 periodic rescuer hides a missing wake | 2a, 2c | **Guarded**: each leg runs with nothing else able to end the wait in time, and a control compiles the fire out | the two controls below |

## 7. Discriminators, controls, kill-switches, build order

**Build order** (dependency DAG; each step is one kernel-touching 4-shard gate with an AAG that includes a
cross-family lens):

- **2a** (fence park) stands alone. It is the only step with a spin to end, and it is worth building once
  HardwareFlip runs outside test scenarios (Slice 4).
- **2b** (fence item) needs 2a's `fire_fence`. Build it when a caller multiplexes a fence.
- **2c** (futex item) is independent of 2a/2b and needs the extension array. Build it when a threaded process
  multiplexes a futex word.

**2a discriminator — `fence_wait_parks`.**

- **Setup:** HardwareFlip (the synthetic vblank on the tick). A client presents and fence-waits K frames with
  a long timeout.
- **Pass:** every wait returns 0. The fence wakes count K, and none of them is a tick wake. Each wake lands in
  the tick of its publish. The yield count inside `fence_wait` is 0.
- **Control `wait-multiplex-no-fence-fire`:** the publish fire is compiled out. The waits now end only by the
  deadline, and the leg must FAIL (`reason=fence_wake_missing`).
- **Software-mode check:** the existing present scenarios (`atrium_hw_present`, the desktop boots) pass with
  `parks == 0` on the fence path. Software completes before the first check, so 2a must not park there.

**2b legs (in `wait_any_core`).**

- **Wake:** items `{FENCE_IDLE(buffer), channel}`, no deadline. A HardwareFlip present from another thread
  must wake the waiter with only the fence bit set, in the publish tick.
- **Full ring (Chad F1, a negative discriminator):** on one buffer with the drain held off:
  1. queue 16 flips;
  2. make the 17th complete synchronously;
  3. a `FENCE_IDLE` scan must report **busy**;
  4. release the drain: the waiter wakes only after the 16th entry drains.

  A build using the epoch pair alone must FAIL this leg.

**2c legs (in `wait_any_core`, `browser-runtime`).**

- **Observer wake:** thread A waits `{FUTEX(word, 0)}` with no deadline. Thread B stores 1 and calls
  `futex_wake(word, 1)`. A must wake with bit 0 set within one tick.
- **The count rule:** thread C also parks in `futex_wait(word, 0)`. B's `futex_wake(word, 1)` must wake C
  *and* fire A, and must return 1.
- **The race:** B's store and wake land inside A's register→park window, K iterations, counters balanced.
- **Control `wait-multiplex-no-futex-fire`:** A only times out, and the leg FAILs
  (`reason=futex_observer_wake_missing`).

**Kill-switches.**

- `wait-multiplex-fence` off: `fence_wait` is today's yield loop bit for bit, and kind 4 is EINVAL.
- `wait-multiplex-futex` off: kind 5 is EINVAL, `ext` is never read, and `futex::wake` is unchanged.
- Both off equals rung 1.
- Host KATs: the kind table, the extension-record decode against the hand-laid bytes above, and EINVAL for
  kinds 4/5 with the features off.

**Invariant budget.** No new cross-cutting invariant. Two lock edges are added to the ledger's order:

- ring lock → (drop) → `WAIT_TABLE`;
- `FUTEX_QUEUE` → (drop) → `WAIT_TABLE`.

The `present_backend` "one ISR lock" promise is restated as "two leaf locks, never nested".

## 8. Base THINK §7 Q5: is a 100 Hz pull at idle acceptable on the bare-metal power budget?

**Answer: yes, for the kernel as it is. Rung 3 (an asynchronous input producer) need not come first.** It
becomes a prerequisite only together with a tickless idle.

What the code says:

1. **The BSP already wakes 100 times a second.**
   - The tick is PIC IRQ0, periodic, BSP-only (`timer.rs`), and nothing ever stops or reprograms it when
     idle.
   - The idle thread's halt is a plain `sti; hlt` (`idle_thread_entry` → `enable_interrupts_and_halt`).
     There is no MWAIT and no C-state selection.
   - So an idle BSP leaves C1 at 100 Hz *with or without* the input pull. The pull does not raise the wake
     rate; it only adds work to wakes that happen anyway.
2. **The added work per tick is small and bounded.**
   - `wait_any::on_tick_expire` walks 16 slots under one lock.
   - About two threads wake: the WM loop and Arthur both name input items.
   - Each woken thread does:
     - a context switch;
     - the HID pumps (per pump, `xhci::drain_events` does an MMIO write of `USBSTS`, an MMIO read of
       `IMAN`, an `ERDP` write, a conditional `IMAN` write, and one RAM read of the event TRB);
     - a scan of at most 8 items;
     - a re-register and a park.
   - **Estimate, unmeasured:** around 12 baseline uncached MMIO accesses (corrected after Chad's count;
     the first draft missed the `ERDP` write) plus four context switches per tick. That is still tens of
     microseconds in a 10 ms tick, an added C0 share well under 1%.
3. **The APs are unaffected.** They have no timer at all and halt until an IPI. Desktop threads are
   BSP-resident.

When the answer flips: if the kernel ever stops the tick when idle (a one-shot LAPIC deadline, a "tickless"
idle) or selects deeper C-states via MWAIT, the input pull becomes the *only* thing forcing 100 wakes a
second. At that point rung 3 must land first, or with it.

**What the existing witness cannot tell us.** `desktop_idle_parks`' idle-tick share samples *at tick
instants*. A thread woken by the tick and re-parked before the next one is invisible to it. So that witness
cannot falsify this answer either way.

**What would falsify it** (a measurement build, not part of rung 2):

- **(F1) Busy time per tick, by TSC.**
  - **Capture:** stamp `rdtsc` at BSP timer entry and at the next idle `hlt` entry, and average busy time
    per tick over a 300-tick quiet window with the desktop parked.
  - **Prediction:** at most 0.5% of the tick period on the OptiPlex 7080.
  - **Falsified if** it exceeds 5%, or if it exceeds a no-pull control by more than 2 percentage points.
    The control is the same desktop with its input items removed (channels only, no deadline), so input is
    dead and this is measurement only.
- **(F2) Package energy, by RAPL.** `MSR_PKG_ENERGY_STATUS` (0x611) is available on the 7080's Intel CPU.
  - **Capture:** read it over a 30 s idle window three ways: parked, spinning (`desktop-park` off), and
    no-pull.
  - **Prediction:** parked ≈ no-pull within noise, and spinning clearly higher.
  - **Falsified if** parked is clearly above no-pull. Then the pull itself costs power and rung 3 moves
    ahead.
  - **Probe rules (revised after Chad F3).** `RDMSR 0x611` #GPs where RAPL is absent, hidden or disabled,
    and this kernel's #GP is fatal: it cannot resume. A CPUID vendor or model check is **not** a probe; a
    normally RAPL-capable model is no fault-recovery mechanism.
    - F2 is built only on a recoverable `rdmsr_safe`: a #GP fixup stub that returns an error and resumes
      (save, fixup, `iretq`), per the pitfall bank's #GP entry.
    - Without that stub, F2 is omitted and F1 (TSC) stands alone.
    - Its discriminator includes the absent-MSR control: under QEMU TCG, which has no RAPL, `rdmsr_safe`
      must return "unavailable", the run must report `rapl=unavailable`, and it must never fault.

Any F1/F2 run on bare metal is a blind-hardware dispatch. It must reference
`docs/runbook-baremetal-arthur-optiplex.md` and carry its own failure-mode and discriminator section, for
example: TSC not invariant, the RAPL MSR absent, the window disturbed by the serial log.

## 9. Open questions for the Codex lens

1. Is dropping `fence_wait`'s yield-cap backstop (§2) acceptable once every other timed park rests on the
   tick alone? Or should 2a keep the yield loop until `TICKS` passes a boot threshold?
2. Can `publish_complete` run with `WAIT_TABLE` already held on the same CPU? For example, a synchronous
   full-ring publish reached from a path that holds the table. Today no table holder calls into present;
   confirm there is none.
3. Is the observer rule (§5.3) enough to keep every existing `futex_wake` caller's semantics, including the
   `thread_exit` join wake on `join_key`? User futex items can never name a join key (bit 63), but confirm no
   join path fires observers.
4. Is the send-ready verdict (§4) right? Is there any receiver in the tree that parks in `sys_recv` while its
   sender would need to multiplex?
5. Is "idle ⇔ `completed_epoch >= issued_epoch`" (§3) racy against `present`'s epoch issue order (the issue
   increments before the blit)? Can a scan see a torn pair that reads "idle" with a present in flight?
