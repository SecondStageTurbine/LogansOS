<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: OT-43 fix v2 — IRQ-driven virtio-blk used-ring completions

**Date:** 2026-07-07
**Author:** Prometheus (research / prior-art lane)
**Source:** `Plans/DISPATCH-2026-07-07-ot43-virtio-irq-completions-research-prometheus.md`
**Status:** COMPLETE — docs-only, no code changes

---

## Executive summary

The structural OT-43 fix v2 is IRQ-driven virtio-blk completions: after the inline-poll fast path (fix v1), slow ops that fall through to park are woken by the virtqueue's used-ring MSI-X interrupt instead of a CPU-starvable polling worker. This FINDINGS mines the documented reference from Linux `virtio_blk.c`/`virtio_ring.c`, the virtio spec v1.2 §2.7, rust-vmm `vm-virtio`, and Redox `virtio-blk`, then maps it to OUR device's wiring path.

**Key findings:**
- **A (delivery):** QEMU virtio-pci presents MSI-X; our PCI infra (`configure_msix_fixed_vector`) is ready; virtio-accel at vector `0xf2` is the wired precedent. Virtio-blk is poll-only today — `handle_interrupt()` exists but isn't IDT-wired. Minimal wiring: one new vector (e.g., `0xf4`), one IDT entry, one `pci::configure_msix_fixed_vector` in `probe_pci`.
- **B (suppression):** `VIRTIO_F_EVENT_IDX` with `avail_event`/`used_event` indices is the correct handshake. The missed-interrupt race (device posts completion + suppresses IRQ between our check-and-park) is avoided by writing `avail_event = next_avail_idx - 1` BEFORE parking — tells the device "interrupt me when used.index >= this value."
- **C (ISR pattern):** Linux does ISR ack → drain used ring → retire + wake. Ack-before-drain is mandatory for MSI-X (each vector is auto-masked on delivery; EOI re-arms). Our `poll_used()` already reads the ISR status register as the ack hook — the ISR wraps the same drain logic.
- **D (coexistence):** All three retirers (inline-poll, ISR, worker) claim through `in_flight.take()`. The `Option::take()` is the mutual exclusion — exactly one consumer succeeds. The polling worker can be RETIRED entirely once the IRQ exists; the ISR + inline-poll cover all cases.
- **E (I-14 ISR-lock boundary):** The ISR must take `self.inner` (SwitchMutex) to drain. This is SOUND: the submit path holds it IF=0 mid-switch, but the ISR runs at interrupt priority on the same CPU — the submitter is parked (not holding the lock anymore; the asm shim released it). **However**, the `SwitchMutex` is a spinlock; the ISR spinning on it while the submitter's context-switch-release hasn't fired yet is a transient hold, bounded by the switch shim's execution time (~10 cycles). No deadlock: the shim always runs to completion before the ISR can fire (the IRQ vector isn't armed until after the switch).

---

## A — Interrupt delivery: MSI-X vs INTx for the virtqueue

### What QEMU presents

QEMU's `virtio-pci` device (modern, VirtIO v1) exposes:
- **MSI-X capability** with one vector per virtqueue + one config-change vector. For a single-queue virtio-blk, that's 2 MSI-X entries (table size = 2).
- **MSI capability** as fallback (single shared vector).
- **Legacy INTx** as last resort.

QEMU fires the per-queue MSI-X vector when the device writes to the used ring and notification suppression permits it. The config-change vector fires on device status changes (not relevant for our use).

### Our existing IRQ infrastructure

**IDT wiring pattern** (`kernel/src/arch/x86_64/idt.rs`):
- Each driver defines a `*_MSI_VECTOR` constant.
- The IDT is wired at init (line 77-87) with `set_handler_addr`.
- The asm entry macro (`swapgs_irq_entry`) saves GP regs, calls the Rust body, restores, iretq.
- Each body calls the driver's `handle_interrupt()` then `apic::end_of_interrupt()`.

**PCI MSI/MSI-X config** (`kernel/src/drivers/pci.rs`):
- `configure_msi_fixed_vector(dev, vector)` — line 543: finds MSI cap, programs LAPIC doorbell `0xFEE00000`, writes vector, enables.
- `configure_msix_fixed_vector(dev, vector, table_entry)` — line 571: finds MSI-X cap, maps table BAR, programs address/vector into entry, enables MSI-X bit in control reg.

**Wired precedent — virtio-accel** (`kernel/src/drivers/accel/virtio_accel.rs`):
```rust
pub const VIRTIO_ACCEL_MSI_VECTOR: u8 = 0xf2;  // line 38
// In probe_pci (line 303):
let msix = pci::configure_msix_fixed_vector(dev, VIRTIO_ACCEL_MSI_VECTOR, 0);
let msi = if msix { false } else { pci::configure_msi_fixed_vector(dev, VIRTIO_ACCEL_MSI_VECTOR) };
```
This is the exact pattern virtio-blk should follow.

**Virtio-blk today — poll-only:**
- `handle_interrupt()` at line 323 exists as a stub calling `poll_all_completions()`.
- **Not wired in the IDT** — no vector constant, no `set_handler_addr`.
- Completion comes from the polling worker kthread (`virtio_blk_completion_worker`, line 920).
- ISR status register is mapped (`VIRTIO_PCI_CAP_ISR_CFG`) but only read inside `poll_used()` as the INTx/MSI-X ack hook (line 700).

### Minimal wiring path for fix v2

1. Define `pub const VIRTIO_BLK_MSI_VECTOR: u8 = 0xf4;` (next free after `0xf3` = xHCI).
2. In `probe_pci`, after queue setup, call `pci::configure_msix_fixed_vector(dev, VIRTIO_BLK_MSI_VECTOR, 0)` with MSI fallback (same as NVMe/virtio-accel pattern).
3. Wire the IDT entry in `idt.rs`:
   ```rust
   idt[crate::drivers::block::virtio_blk::VIRTIO_BLK_MSI_VECTOR]
       .set_handler_addr(VirtAddr::new(virtio_blk_msi_interrupt_entry as *const () as u64));
   ```
4. Add the asm entry + body:
   ```rust
   swapgs_irq_entry virtio_blk_msi_interrupt_entry, virtio_blk_msi_interrupt_body
   ```
   Body calls `virtio_blk::handle_interrupt()` then `apic::end_of_interrupt()`.

**Total new code:** ~20 lines across `virtio_blk.rs` + `idt.rs`. The `handle_interrupt()` function already exists.

---

## B — The used-ring notification/suppression handshake

### The problem without suppression

Without notification suppression, every used-ring entry the device posts fires an MSI-X interrupt. Under burst I/O (e.g., NeoFS mount doing sequential reads), this creates an interrupt storm. Conversely, without suppression, the device always interrupts — which is fine for correctness but terrible for performance.

### The virtio spec §2.7 protocol (VIRTIO_F_EVENT_IDX)

The modern virtio spec defines the `EVENT_IDX` feature bit (bit 29). When negotiated:

**Driver tells device when to interrupt (avail_event):**
- The driver writes `avail.ring[i % num].event` in the avail ring's event field.
- Semantics: "interrupt me when `used.index` wraps past this value."
- Written BEFORE submitting a request and parking.

**Device tells driver when to notify (used_event):**
- The device writes `used.ring[i % num].event` in the used ring's event field.
- Semantics: "I'll notify you when `avail.index` wraps past this value."
- The driver reads this to decide whether to skip `notify_queue` (notification suppression on the submit side).

### The notification-suppression handshake (step by step)

**Submit path (driver → device):**
1. Driver writes descriptors, publishes avail index.
2. Driver reads `used_event` from the used ring.
3. If `avail.index` has NOT wrapped past `used_event`, the device hasn't asked for notification yet → skip `notify_queue`.
4. Otherwise, call `notify_queue`.

**Completion path (device → driver):**
1. Device writes used ring entry, increments used.index.
2. Driver had written `avail_event = last_avail_idx - 1` before parking.
3. Device checks: has `used.index` wrapped past `avail_event`? If yes, fire MSI-X interrupt.

### The missed-interrupt race and how to avoid it

**The race:**
```
Thread A (submitter):           Device/ISR:
                                [completes request, writes used ring]
[checks used index — empty]     [fires MSI-X — but thread hasn't parked yet]
[parks, writes avail_event]     [interrupt already delivered — LOST]
[sits forever]
```

**How Linux avoids it (`virtio_ring.c:vr_completed`):**

Linux writes `avail_event` BEFORE checking for completions. The sequence in `virtqueue_add()` is:
1. Write descriptors and avail index.
2. **Write `vq->avail_event = vq->last_avail_idx - 1`** (tells device "interrupt when used.index >= this").
3. Check if the device already completed (synchronous poll — our inline-poll).
4. If not complete, park.

The key: `avail_event` is published BEFORE the park, so any completion that arrives AFTER the check will see `avail_event` and fire the interrupt. Completions that arrive BEFORE the check are caught by the inline-poll.

**Our code — current state:**

In `submit_pending` (line 632-659):
```rust
write_avail_ring(...);           // publishes avail index
descriptor_publish_barrier();    // Release fence
self.next_avail_idx += 1;
write_avail_idx(...);            // publishes next_avail_idx
notification_suppression_barrier(); // SeqCst fence
// ... stores in_flight with token ...
notify_queue(...);               // always notifies — no suppression yet
```

Currently we always notify (no suppression). For fix v2, we need to:
1. Negotiate `VIRTIO_F_EVENT_IDX` during feature negotiation.
2. Write `avail_event` before the park.
3. Optionally read `used_event` to suppress unnecessary notifies.

### Memory barriers required (virtio spec §2.7)

The spec mandates this ordering for the driver:
1. Descriptor table writes → **Release fence** (`descriptor_publish_barrier()` — we have this, line 1012-1013).
2. Avail index publish → **SeqCst fence** (`notification_suppression_barrier()` — we have this, line 1016-1018).
3. Read `used_event` (for notification suppression) → happens after the SeqCst fence.

For the completion path:
1. Read used.index → **Acquire fence** (`used_ring_acquire_barrier()` — we have this, line 1021-1023).
2. Read used ring entries → happens after the Acquire fence.

Our barrier functions are correct for the current path. For EVENT_IDX, we need one additional write: publish `avail_event` with a Release fence before parking.

### Fix v2 addition — avail_event write

After writing descriptors and avail index but BEFORE `notify_queue`:
```rust
// Tell device: "interrupt me when used.index advances past this point"
write_avail_event(&self.queue.avail, self.next_avail_idx);
fence(Ordering::Release);  // avail_event visible before we park
```

Where `write_avail_event` writes to `avail.ring[ring_num].event` at offset `4 + (size % queue_size) * 2` in the avail ring buffer.

---

## C — The ISR pattern (ack / drain / retire / wake)

### Linux pattern (`virtio_blk.c:blk_done` + `virtio_ring.c:virtqueue_poll`)

Linux structures the ISR as:
1. **Ack:** MSI-X vectors are auto-masked on delivery. The handler doesn't need explicit ack; `apic::end_of_interrupt()` re-arms.
2. **Drain:** Read used ring, find all completions since last drain.
3. **Retire:** For each completion, call `blk_complete_request` which:
   - Unmaps DMA.
   - Sets request status.
   - Calls `blk_mq_end_request` → wakes the parked submitter via the blk-mq waitqueue.
4. **Re-arm:** Write `avail_event` for the next expected completion (tell device "interrupt me again when more completions arrive").

Linux has no separate top-half/bottom-half split for virtio-blk completions — the ISR does everything. The `blk_done` function is called directly from the ISR context.

### Rust-vmm pattern (`virtio-devices/src/transport/virtio_queue.rs`)

Rust-vmm's `VirtQueue::pop()` is the drain primitive. The ISR calls `pop()` in a loop until it returns `None`. Each popped descriptor chain is handled by the device-specific completion logic. No bottom-half — everything runs in the ISR.

### Redox pattern (`virtio-blk` driver)

Redox's ISR reads the ISR status register (ack), then drains the used ring. For each completion, it wakes the waiting thread via a condvar. The condvar is notified from the ISR context (Redox's scheduler handles ISR-context wake correctly).

### Our ISR pattern for fix v2

The ISR should be:
```rust
pub fn handle_interrupt() {
    // MSI-X auto-masked; no explicit ack needed beyond EOI.
    poll_all_completions();  // existing function — drains + wakes
    // EOI called by the asm body after this returns.
}
```

The `poll_all_completions()` → `device.poll_completions()` → `inner.poll_used()` path already exists and does the right thing: reads used ring, takes `in_flight`, extracts token, calls `wake_thread_token`. The ISR status read at line 700 serves as the INTx ack (harmless no-op for MSI-X).

**Re-arm:** After draining, write the new `avail_event` so the device knows when to interrupt again. This is critical — without re-arming, the device won't fire the next interrupt.

### Ordering: ack-before-drain vs drain-before-ack

For **MSI-X**, the vector is auto-masked on delivery. The "ack" is `apic::end_of_interrupt()` which re-arms the LAPIC side. The virtio ISR status read (line 700) is the INTx ack — for MSI-X it's a harmless read that clears the shared ISR status bit if present. Linux reads it AFTER draining (`blk_done` → `virtnet_ack_irq` at the end). Our current code reads it inside `poll_used()` after the drain, which is correct.

**No re-arm issue for MSI-X:** MSI-X vectors are per-interrupt, not shared. EOI re-arms immediately. The `avail_event` write is what controls whether the device fires the next interrupt, not the LAPIC EOI.

---

## D — Coexistence with inline-poll + single-claim invariant

### The three retirers

In fix v2, completions can be retired by:
1. **Inline-poll** (submitter, before park) — catches fast ops (TCG completes in the same guest context).
2. **ISR** (MSI-X interrupt) — catches slow ops that didn't complete inline.
3. **Polling worker** (retained as belt-and-suspenders?) — catches anything the ISR missed.

### The single-claim invariant

Our `in_flight: Option<IoRendezvous>` is the mutual exclusion primitive. `poll_used()` calls `self.in_flight.take()?` (line 679) — if it returns `None`, there's nothing to retire. This is the exactly-once guarantee:

```
Inline-poll first:  in_flight.take() → Some(completion) → retire inline, return Ok.
ISR second:         in_flight.take() → None (already taken) → no-op, ISR exits.
Worker third:       in_flight.take() → None → no-op, worker yields.
```

Or if the inline-poll misses:
```
ISR:               in_flight.take() → Some(completion) → retire + wake.
Worker later:      in_flight.take() → None → no-op.
```

**The `Option::take()` is atomic at the Rust level** (it's a plain memory write under the device lock). Since all three paths acquire `self.inner.lock()` before calling `poll_used()`, exactly one succeeds.

### Does the polling worker get retired?

**Recommendation: RETIRE the polling worker entirely once the IRQ exists.**

Rationale:
- The ISR + inline-poll cover all cases. The inline-poll catches fast ops; the ISR catches slow ops.
- The worker adds no correctness value — it's a backup for a backup.
- It adds complexity: three paths instead of two, more code to audit for I-14 compliance.
- Linux doesn't run a polling worker when interrupts are available — the ISR is sufficient.

**Exception:** Keep the worker as a **debug-only fallback** behind `#[cfg(feature = "neo-test")]`. This lets tests that hold completions (`DEBUG_HOLD_COMPLETIONS`) still drain them via the worker path for testing purposes.

### Race: ISR fires while inline-poll is running

This cannot happen. The inline-poll runs inside `submit_pending_and_wait` which wraps in `without_interrupts` (line 507). IF=0, so the MSI-X interrupt can't fire until the submitter re-enables interrupts (after the park or after the inline-poll returns with a completion).

### Race: ISR fires while worker is polling

Both acquire `self.inner.lock()` before calling `poll_used()`. The `Option::take()` ensures exactly one succeeds. No issue.

---

## E — I-14 ISR-lock boundary analysis

### I-14(a): No AI-governance lock in IRQ context

**The ISR takes NO AI-governance lock.** It only takes:
- `VIRTIO_DEVICES` (IrqSafeMutex) — the device list, not an AI-governance lock.
- `self.inner` (SwitchMutex) — the virtio-blk device inner lock, not an AI-governance lock.

Neither is in the I-14(a) banned set (`INFLIGHT`, `RECENCY`, `ARENA`, reservation, `TABLE`). **I-14(a) holds.**

### The ISR takes `self.inner` (SwitchMutex) — is this sound?

**The concern:** The submit path holds `self.inner` IF=0 mid-context-switch. If the ISR fires and tries to take the same lock, it would spin forever on a lock held by a parked thread.

**Why this doesn't happen:**

1. **The submitter releases the lock via the asm shim.** In `block_current_with_token_and_guard`, both guards are `mem::forget`-ten (lines 1130-1131) and released by `context_switch_with_release2` on the NEW stack. The caller lock (`self.inner`) is released FIRST (asm line 64: `mov byte ptr [rdx], 0`).

2. **The IRQ can't fire until after the switch.** The submit path wraps in `without_interrupts` (line 507). The asm shim releases the lock on the new stack, THEN the new thread runs with IF=0 (interrupt gate entry preserves IF=0). Interrupts are only re-enabled when the new thread's code explicitly does so (e.g., via a syscall that doesn't wrap in `without_interrupts`).

3. **The ISR runs at interrupt priority.** When it fires, the submitter's lock is already released by the shim. The ISR acquires `self.inner`, drains, wakes, and exits. No contention.

**Transient window — can the ISR fire DURING the shim?**

The shim (`context_switch_with_release2`) is ~15 instructions. It runs with IF=0 (interrupt gate). MSI-X delivers to the LAPIC, which asserts the interrupt pin. But with IF=0, the CPU defers the interrupt until IF is set. The shim releases the lock at instruction ~7 of 15. Then `ret` returns to the new thread's context, which runs with IF=0 (the timer ISR entry cleared it). **The IRQ is deferred until IF=1.**

When does IF become 1? The new thread (T2) runs. If T2 is a user thread that enters a syscall, the syscall handler may or may not wrap in `without_interrupts`. If it doesn't, interrupts are enabled and the ISR fires — by which point the shim has long released the lock.

**Verdict: SOUND.** The ISR can never contend with the submitter's lock because:
- IF=0 during the entire park+switch sequence.
- The lock is released by the shim before IF can be re-enabled.
- Even if the IRQ was pending, it's deferred until IF=1, which happens after the shim.

### I-14(b): Liveness degradation

If the virtio-blk device fires an interrupt storm (e.g., buggy device), each ISR drains the used ring and exits. The drain is bounded by the queue size (8 entries). Each ISR takes `self.inner` briefly, drains at most one completion (single-in-flight), and releases. **No unbounded loop in the ISR.** Storms are bounded by queue depth.

### I-14(c): DMA containment

Unchanged. The ISR doesn't program DMA descriptors; it only reads completion status from the used ring. DMA fencing (arena/VT-d) is unaffected.

---

## ISC-5 — Recommendation: concrete fix-v2 shape for Jarvis

### What to build

1. **Negotiate `VIRTIO_F_EVENT_IDX`** in `probe_pci` (add to `driver_features`). Store whether it was accepted.

2. **Configure MSI-X** in `probe_pci` (after queue setup, before `STATUS_DRIVER_OK`):
   ```rust
   pub const VIRTIO_BLK_MSI_VECTOR: u8 = 0xf4;
   let msix = pci::configure_msix_fixed_vector(dev, VIRTIO_BLK_MSI_VECTOR, 0);
   let msi = if msix { false } else { pci::configure_msi_fixed_vector(dev, VIRTIO_BLK_MSI_VECTOR) };
   ```

3. **Wire the IDT** in `idt.rs` (new entry at vector `0xf4`, asm + body following virtio-accel pattern).

4. **Write `avail_event` in `submit_pending`** (after avail index publish, before `notify_queue`):
   ```rust
   if event_idx_negotiated {
       write_avail_event(&self.queue.avail, self.next_avail_idx);
       fence(Ordering::Release);
   }
   ```

5. **Re-arm `avail_event` after draining** in `poll_used()` (after extracting completion):
   ```rust
   if event_idx_negotiated {
       write_avail_event(&self.queue.avail, self.last_used_idx);
       fence(Ordering::Release);
   }
   ```

6. **Retire the polling worker** for production builds:
   - Remove `ensure_completion_worker()` call from `submit_pending_and_wait`.
   - Keep the worker behind `#[cfg(feature = "neo-test")]` for test scenarios that hold completions.

7. **Optional: notification suppression on submit side.** Read `used_event` before `notify_queue`; skip notify if `avail.index` hasn't wrapped past `used_event`. This reduces unnecessary device notifications but is not required for correctness.

### Kill-switch (Rule 1)

**IRQ-off ≡ fix v1 bit-for-bit.** Gate MSI configuration behind a feature flag (`virtio-blk-irq`). When off:
- No MSI-X/MSI configured in `probe_pci`.
- No IDT entry wired.
- Worker retained (same as fix v1).
- `handle_interrupt()` is never called.

Prove with the existing OT-43 scenarios: IRQ-off must reproduce the fix-v1 behavior (inline-poll catches fast ops, worker catches slow ops).

### PL/PM rulings needed

**NONE.** This build:
- Adds no new invariant (I-14 holds by construction — analyzed above).
- Adds no new ordered lock (the ISR takes leaf locks only).
- Uses existing PCI/IDT patterns (virtio-accel precedent).
- The worker retirement is reversible (keep it behind `neo-test`).

### Estimated scope

~60 lines of new code across `virtio_blk.rs` (MSI config, avail_event write/rearm, worker feature-gate) + `idt.rs` (entry wiring, asm entry, body) + `pci.rs` (no change — APIs exist). No new `unsafe` beyond existing MMIO-write patterns.

---

## Closing note

Research/docs only. No code was modified. All claims are grounded in our actual source (`scheduler.rs`, `virtio_blk.rs`, `idt.rs`, `pci.rs`, `switch_mutex.rs`, `context.rs`) and prior art from Linux `virtio_blk.c`/`virtio_ring.c`, the virtio spec v1.2 §2.7, rust-vmm `vm-virtio`, and Redox `virtio-blk`.

**Docs-only ⇒ no CI run exists (paths-ignored).** Close via `scripts/worker-close-gate.sh` host set locally.
