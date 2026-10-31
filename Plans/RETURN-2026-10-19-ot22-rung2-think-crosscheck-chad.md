<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: OT-22 rung-2 THINK cross-check (Chad)

**Reviewed:** merged THINK `e65da2d9` (#62), rung-1 implementation on `origin/claude/ot22-multiplex`, and the current present, futex, IPC, timer, idle, and xHCI paths.

## Claim verdicts

1. **§1 shipping-spin inventory: HOLDS.** Software presents publish completion synchronously before `fence_wait`; the deferred HardwareFlip path is the only present-fence yield loop. Futex and channel sends already park.
2. **§2 fence park/fire ordering: HOLDS-WITH-CHANGES.** The register → Acquire re-check → fired-checked park proof is sound, and current `publish_complete` call sites do not hold `WAIT_TABLE`. The implementation must preserve the stated ring-drop-before-fire order and explicitly choose whether losing the independent yield-cap backstop is acceptable.
3. **§3 `FENCE_IDLE`: FAILS.** Finding F1: `completed_epoch >= issued_epoch` does not mean no present for that buffer remains in flight on the existing full-ring fallback.
4. **§4 send-ready declined: HOLDS.** The desktop's `wait_any` + `try_recv` receivers never enter `RecvBlocked`; making their senders wait for that state creates the described mutual wait. Blocking `sys_recv` users exist elsewhere, but I found no paired sender that also needs multiplexing.
5. **§5 futex item: HOLDS-WITH-CHANGES.** Tagged keys and observer/non-consuming wake counts preserve existing real futex waiters, join wakes, and rwlock return counts. Finding F2 must be incorporated into the ABI validation algorithm. The infinite-deadline/lifetime caveat below also needs explicit contract text.
6. **§8 100 Hz power answer: HOLDS-WITH-CHANGES.** The periodic BSP PIC tick and plain `sti; hlt` claim is correct, and the added work is bounded. Finding F3 makes the proposed RAPL falsifier unsafe as written.

## Findings

### F1 — High — `FENCE_IDLE` reports idle while older flips remain queued

**Files:** `docs/THINK-2026-10-19-ot22-rung2-more-interests.md:97`; `kernel/src/atrium/buffer.rs:415`; `kernel/src/atrium/buffer.rs:518`; `kernel/src/atrium/buffer.rs:546`; `kernel/src/atrium/present_backend.rs:179`.

The present backend explicitly allows out-of-order publication: a full 16-entry ring makes the newest present complete synchronously, while older entries remain queued. Repro/interleaving on one buffer with draining suppressed:

1. HardwareFlip presents epochs 1 through 16; all are in `PENDING`, so `issued_epoch = 16` and completion is behind.
2. Present epoch 17 finds the ring full and calls `publish_complete(17)` synchronously.
3. Now `issued_epoch == completed_epoch == 17`, so the proposed kind-4 scan returns ready.
4. The ring still owns epochs 1 through 16 for that same buffer; later vblank drains them and `fetch_max` deliberately leaves `completed_epoch` at 17.

Thus the predicate means “the latest issued epoch has been logically satisfied,” not “no present on this buffer is in flight” or “safe to draw the next frame.” The rung needs a per-buffer pending/deferred count (increment only for an armed flip, decrement on its actual drain), or it must rename/redefine the interest and prove that queued older flips cannot consume mutable buffer contents after readiness. Add the full-ring/same-buffer sequence as a negative discriminator.

### F2 — High — the extension-array user pointer has no specified validation/copy step

**File:** `docs/THINK-2026-10-19-ot22-rung2-more-interests.md:147`.

The THINK bounds `ext_n`, validates each record's `uaddr`, flags, and item index, and correctly promises not to inspect args 4/5 unless kind 5 exists. It never says to validate the `ext` range itself or copy the records into kernel memory before decoding. Repro against a literal implementation of the text: a valid kind-5 item with index 0, `ext_n = 1`, and a non-canonical/unmapped `ext` makes the kernel read 16 bytes from an unchecked user pointer; this kernel's ring-0 fault path is fatal.

The design must require: detect kind 5 only after copying the ordinary items; reject `ext_n > ITEMS_MAX`; checked-multiply `ext_n * 16`; `validate_user_buf(ext, bytes)`; copy once under SMAP into a fixed kernel array; then decode/validate the copied records. The record array itself need not be aligned if the byte decoder is unaligned-safe; each decoded `uaddr` still must be 4-aligned, range-validated, and translated. This also closes a user TOCTOU between key registration and rescans of record metadata.

### F3 — Medium — CPUID vendor/model checks are not a safe `RDMSR 0x611` probe

**Files:** `docs/THINK-2026-10-19-ot22-rung2-more-interests.md:317`; `docs/concurrency-pitfalls.md:2051`.

`MSR_PKG_ENERGY_STATUS` can #GP when RAPL is absent, hidden, or disabled. The THINK calls CPUID vendor/model checks a probe, but this tree's #GP entry is fatal and explicitly cannot resume; the pitfall bank says an `rdmsr_safe` probe needs a returning save/fixup/`iretq` stub. Even a normally RAPL-capable OptiPlex model is not a fault-recovery mechanism.

The THINK's power-falsifier F2 measurement build must either implement and gate a recoverable `rdmsr_safe` path, or omit the RAPL leg unless platform bring-up has already established the MSR through a non-faulting mechanism. Its discriminator must include the absent/disabled-MSR control; it must never test availability by executing a potentially fatal `rdmsr` behind only a model allowlist.

## Refute checklist

1. **Lost wakeups (2a/2b/2c).** Replayed publish-before-register, publish-between-register-and-recheck, and publish-between-recheck-and-park. `WAIT_TABLE` total order plus the post-register Acquire/user-word re-check closes all three. If the waker locks first, the waiter later synchronizes and sees the publication; if registration locks first, the fire sets `fired` or takes the installed token. The same applies to futex after its user store. F1 is a readiness-semantic failure, not a missed fire.
2. **ISR context.** Enumerated `publish_complete`: Software present, HardwareFlip full-ring fallback, and `on_present_vblank` after the ring lock is dropped. Current `WAIT_TABLE` holders register/remove/fire/park only; none calls present. Same-CPU interruption is excluded by `IrqSafeMutex`; cross-CPU holders run bounded sections and do not wait on the present ring. Fire must continue to collect tokens under `WAIT_TABLE`, drop it, then enter the scheduler.
3. **Count rule.** `futex::wake` selects at most `count` real `FUTEX_QUEUE` tokens and computes its return value only from successful real wakes. Firing observers afterward cannot steal a slot or change the return. `thread_exit` uses the bit-63 join namespace, which cannot match a user physical futex key, and tagged wait keys add a second separation. The rwlock fallback therefore sees the same count. A mutex fast-path unlock that skips wake can leave an observer parked forever when its deadline is infinite; this is outside the stated event-word contract, but “only slow” should be replaced with “may block indefinitely unless every writer wakes or a finite deadline is supplied.” Likewise, a stale physical key's missed fire is not bounded when the deadline is infinite; require the mapping/key to remain stable for the wait.
4. **ABI.** Rung-1 `raw3` leaves R10/R8 unspecified, but the kind-first rule prevents old binaries' leftovers from being read. The byte oracle is correct little-endian. Index, `ext_n`, flags, `uaddr` alignment/range/translation, and unknown-kind rejection are sound after F2's missing outer-range copy is added.
5. **FENCE_IDLE.** Relaxed `issued_epoch` under `present_serial` plus Release/Acquire completion does not become permanently stale after a completed publish; register/recheck supplies the needed ordering. A concurrent new present can make a snapshot immediately obsolete, as any level readiness can. F1 is stronger: the existing out-of-order fallback can make the pair equal while older work is objectively still queued.
6. **Send-ready.** Checked blocking `sys_recv` uses and the desktop's parked loops. Blocking receivers exist in synchronous handshakes and tests, but their senders simply send; I found no current receiver/sender pair satisfying the falsifier (receiver blocks in `sys_recv` while the sender must multiplex). The desktop pair has the opposite shape and would deadlock exactly as §4 says.
7. **Power.** The BSP timer is fixed-period PIC IRQ0 at 100 Hz; no idle stop/reprogram path exists. Idle uses plain `enable_interrupts_and_halt`; APs have no timer. At quiet idle, each `xhci::drain_events` also writes ERDP, so keyboard + mouse pumps for two woken threads are about 12 baseline MMIO accesses, not eight (plus a conditional IMAN write), still not an order-of-magnitude error and still covered by the THINK's power-falsifier F1 TSC measurement. The RAPL safety issue is finding F3 above.
8. **Structural misses.** The full-ring/out-of-order completion interaction (F1), outer extension-buffer validation (F2), and the non-recoverable #GP path (F3) are the cross-subsystem seams the THINK did not close.

## Verification posture

Design review only; no run required. RAG health was `healthy` (`ollama=true`, `qdrant=true`). I used direct reads for the complete THINK and the concrete rung-1/present/futex/IPC/timer/xHCI implementations.
