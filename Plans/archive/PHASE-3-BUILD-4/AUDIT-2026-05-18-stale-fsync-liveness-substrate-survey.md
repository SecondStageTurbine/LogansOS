# Substrate Audit - Stale-Fsync Liveness Lane

Date: 2026-05-18
Agent: Cerebro
Status: READ-ONLY AUDIT (no code changes)
Authority: `Plans/DISPATCH-2026-05-18-phase-3-build-4-stale-fsync-liveness-root-cause-cerebro.md`

Tree state: main, ~448 insertions uncommitted (Fix 8/9/11/12/14/15 + PROGRESS frames). No new edits this turn.

## 1. Existing stale-fsync PROGRESS frames

`kernel/src/test_harness.rs:6494-6610`. Frames emitted in order, with what each proves:

| Stage | Line | Proves |
|---|---|---|
| `mount_done` | 6500-6505 | NeoFS RW mount completed, selected SB slot + generation known. Cache exists, writeback worker spawned. |
| `seed_fsync_done` | 6510-6517 | Initial `neofs_3e_fsync_or_fail(seed)` returned. Includes `dirty_blocks` (should be 0), `drained_blocks` cumulative, `flush_required` (false), `flushes` cumulative. **This is the frame that never arrives in the failing runs.** |
| `hold_set` | 6521-6524 | `debug_set_writeback_hold(true)` succeeded - worker is parked. |
| `mutation_dirty` | 6532-6538 | Mutation write completed under hold; includes pre-stale dirty count + flush command baseline. |
| `stale_fsync_done` | 6555-6561 | Stale-cap fsync returned ESTALE without mutating durability state. |
| `refreshed_fsync_begin` | 6572-6579 | Refreshed handle generation advanced; about to call fsync. |
| `refreshed_fsync_done` | 6582-6589 | Final fsync drained the mutation; expect `dirty_blocks=0, flush_required=false`. |
| `PASS` | 6594-6608 | All invariants satisfied. |

**Failing pattern:** Last seen frame = `mount_done`. The next frame `seed_fsync_done` never appears. That places the stall inside `neofs_3e_fsync_or_fail(seed)` -> `fsync_regular()` -> `cache.drain_all()`. The seed write was 35 bytes (one block) so dirty_blocks is small - the stall is not "too much data," it's a liveness failure on a small-dirty drain.

## 2. Virtio request lifecycle map

`kernel/src/drivers/block/virtio_blk.rs`:

```
caller -> BlockDevice::submit(req)
   |
VirtioBlkDevice::submit_pending_and_wait(pending)            :482
   |- ensure_completion_worker()  -> spawns "virtio-blk-completion" kthread once :885-892
   |- without_interrupts {
        guard = self.inner.lock()
        if guard.in_flight.is_some(): return UNSUPPORTED  <- single-in-flight
        sched::block_current_with_token_and_guard(guard, |inner, token| {
          inner.submit_pending(pending, token)              :550
        })
      }                                                     :483-492

submit_pending(pending, token):                            :550-636
   write_request_header()
   write_descriptor(0, header, ...NEXT)
   if data: write_descriptor(1, data, ...NEXT); write_descriptor(2, status, WRITE)
   else:    write_descriptor(1, status, WRITE)
   write_avail_ring(slot, head=0)
   descriptor_publish_barrier()                            <- AcqRel store + arch barrier
   next_avail_idx += 1; write_avail_idx()
   notification_suppression_barrier()
   in_flight = Some(IoRendezvous { waiting_thread: Some(token), ... })
   if Flush: FLUSH_COMMANDS += 1
   IN_FLIGHT_DMA_BUFFERS = N
   notify_queue(...)                                       <- MMIO doorbell write

                -- thread now BLOCKED in sched --

[completion worker thread]                                  :894-899
loop {
   poll_primary_completions()  -> device.poll_completions() :500
   sched::yield_now()
}

poll_completions():                                        :500-534
   completion = inner.lock().poll_used()
   if let Some(completion):
      LAST_COMPLETION_CPU = current_cpu
      LAST_COMPLETION_TOKEN_CPU/SLOT = token.{cpu_id, slot}
      result = sched::wake_thread_token_front(token, return_value)   <- Fix 9
      LAST_WAKE_RESULT = encode(result)
      serial_println!(...completion bdf=... wake=...)      <- always-on log

poll_used():                                               :638-688
   if DEBUG_HOLD_COMPLETIONS: return None
   used_idx = read_used_idx(used)
   if used_idx == last_used_idx: return None             <- no completion
   used_ring_acquire_barrier()
   descriptor_id = read_used_elem_id(slot)
   last_used_idx += 1
   rendezvous = self.in_flight.take()                    <- consumes pending
   read status from DMA, set IoResult::{Ok|Io}
   rendezvous.done = true
   LAST_COMPLETED_REQUEST_TYPE / DMA_BUFFERS / HAS_DATA_BUFFER
   if Flush+OK: FLUSH_COMPLETIONS += 1
   IN_FLIGHT_DMA_BUFFERS = 0
   COMPLETIONS += 1
   mmio_read8(isr, 0)                                    <- ISR ack
   return Some(Completion { token, return_value, status, bdf, rendezvous })
```

**Single-in-flight invariant:** `submit_pending_and_wait` rejects with `UNSUPPORTED` if any request is already in `in_flight`. So drain_pass writes one block, waits, gets completion, then issues the next. This is the Case-F architectural shape.

## 3. NeoFS fsync/drain lifecycle map

`kernel/src/fs/neofs/inode.rs:251-263` + `kernel/src/fs/neofs/cache.rs:232-249`:

```
fsync(file) [VfsFile trait]                              inode.rs:713
   |
fsync_regular()                                          inode.rs:251
   if !writable: return Ok
   _entries = self.entries.lock()                         <- mount-wide serialization
   cache.drain_all()?
   return Ok

drain_all()                                              cache.rs:232
   _worker_pause = self.pause_worker()                    <- foreground_writers += 1, parks worker
   loop {
      report = drain_pass(device, inner, max_blocks=None)
      done = !flush_required && no dirty blocks
      if done: return total
      if report.drained_blocks == 0:
         sched::yield_now()                               <- BUSY-LOOP yield, no wake
   }

drain_pass(device, inner_lock, max_blocks=None)          cache.rs:476
   drained = 0
   loop {
      snapshot = inner_lock.lock().blocks.iter()
                   .filter(dirty)
                   .min_by_key(role.writeback_order)      <- Data=0,Inode=1,Root=2,FreeMap=3,SB=4
                   .map(|b| (b.block, b.bytes.copy, b.role))
      if snapshot.is_none(): break
      drop lock
      write_fs_block(device, block, &bytes)               <- * BLOCKING virtio submit+wait
      inner = inner_lock.lock()
      if still dirty + same role + same bytes:
         block.dirty = false
         drained += 1
   }
   if drained != 0: drain_batches += 1; drained_blocks += drained
   if flush_required && no dirty: device.flush()          <- * BLOCKING virtio FLUSH submit+wait
   if successful: flushes += 1; flush_required = false
   return DrainReport { drained_blocks }

write_fs_block -> write_bytes_at -> write_device_block_with_retry
   -> retry_busy_block_io(|| device.write_block(lba, &dma))
   -> up to BLOCK_IO_BUSY_RETRIES on Unsupported; any other Err -> BlockDevice error
   -> calls submit_pending_and_wait (single-in-flight virtio path above)
```

**Critical architectural fact:** drain is **strictly serial, one block at a time**. Every dirty block = one full virtio submit -> IRQ -> completion -> wake -> resume round-trip. cache.rs:19 declares `WORKER_DRAIN_BLOCKS_PER_CYCLE: usize = 1`. This is the Case-F architecture the dispatch §3 names.

## 4. Existing counters/state already available vs missing

**virtio_blk (all gated by `#[cfg(feature = "neo-test")]`):**

| Available | What it tells us |
|---|---|
| `debug_completion_count()` (COMPLETIONS) | Monotonic completions seen |
| `debug_flush_command_count()` (FLUSH_COMMANDS) | Monotonic FLUSH submits |
| `debug_flush_completion_count()` (FLUSH_COMPLETIONS) | Monotonic FLUSH completions |
| `debug_in_flight_dma_buffers()` | 0 = no outstanding; N>0 = request submitted, not yet completed |
| `debug_in_flight_waiting()` | True if rendezvous.waiting_thread is Some |
| `debug_last_wake_result()` | Last wake_thread_token_front returned Woken/Gone/Stale |
| `debug_last_completed_request_type()` | Last completed op type |
| `debug_last_completion_cpu()` / `_token_cpu` / `_token_slot` | Where last completion landed |

**Missing in virtio_blk:**
- Last *submitted* request type/LBA/sector
- Monotonic *submissions* counter (we have completions but not submissions; their delta = outstanding queue depth)
- Time stamp of last submission

**NeoFS cache (CacheStats via `debug_stats()`):**

| Available | Tells us |
|---|---|
| `dirty_blocks` | How many cache entries are still dirty |
| `drained_blocks` | Monotonic - how many cache->device writes succeeded |
| `drain_batches` | How many drain_pass invocations did work |
| `flushes` | Monotonic device-flush successes |
| `flush_required` | Whether a SB-dirty marker requires a device flush |
| `writeback_hold` | Whether worker is paused via debug_set_writeback_hold |
| `dirty_marks` (per-role) | Cumulative dirties marked by role |

**Missing in NeoFS cache:**
- Block number currently being written (which block of the snapshot is in-flight)
- Currently-active drain_pass iteration counter
- Time inside current drain_pass
- foreground_writers count (private to CacheInner)

**Scheduler:**

| Available | Tells us |
|---|---|
| `cpu_scheduler_ready(cpu_id)` | Per-CPU init state |
| `debug_total_zombies()` | Total reaped-pending |
| `debug_token_occupied(token)` | Whether specific token slot still has parked thread |
| `per_cpu::current_cpu_id()` / `current_thread_id` (set in block path) | Which thread is running on current CPU |

**Missing in scheduler:**
- Per-CPU ready queue depth (public accessor)
- Per-CPU current thread id (public; only via per_cpu locally)
- Per-CPU last reschedule IPI timestamp/count

## 5. OOB capture tooling inventory

**Kernel side (`kernel/src/arch/x86_64`):**

| Item | Location | Status |
|---|---|---|
| NMI handler (IDT vector 2, IST 1) | `idt.rs:227-232` | Exists; calls `dump("NMI", &sf)` then `halt_forever()`. Dumps RIP/CS/RFLAGS/RSP/SS only. **Terminal** (halts after dump). |
| `dump()` panic-style frame printer | `idt.rs` (used by all faults) | Exists; prints InterruptStackFrame. Does NOT walk scheduler/virtio state. |
| Per-CPU NMI broadcast helper | - | None; QEMU monitor `nmi` only hits BSP on x86_64 (sends INIT to all unless `vcpu N`). |
| GDB stub | - | No in-kernel stub; QEMU's `-gdb` works at the QEMU vCPU level for register/memory inspection. |

**xtask side (`tools/xtask/src/main.rs`):**

| Item | Location | Status |
|---|---|---|
| `NEO_QEMU_MONITOR_PORT` env | `:44, :1944-1953` | Wires `-monitor tcp:127.0.0.1:{port},server,nowait`. Default is `-monitor none`. |
| `NEO_QEMU_GDB_PORT` env | `:43, :1955-1959` | Wires `-gdb tcp::{port}`. |
| `run_with_timeout` | `:2150-2192` | On timeout: `child.kill()` SIGKILL immediately. No monitor query, no NMI inject, no `info threads`, no `info registers`. Pure substrate-blind kill. |
| Per-attempt + per-run serial logs (Fix 8/12) | `:run_test_scenario` family | Preserved; the only forensic substrate we have today. |

**Gap:** On timeout we get the serial tail and nothing else. We cannot distinguish:
- **A** (host froze): would show no new serial frames in the final N seconds before SIGKILL
- vs **B/C/D/E/F** (guest still running but stuck): would also show no new frames if nothing emits them periodically

The kernel needs to *emit something periodically while inside fsync* so the serial tail itself encodes the classification.

## 6. Proposed instrumentation seam (read-only plan)

**Smallest seam I'd implement (gated, off by default):**

### 6.1 Kernel: `neofs-diag-fsync-heartbeat` feature

A single new cargo feature `neofs-diag-fsync-heartbeat` (or hang it off existing `neo-test`). When enabled:

**Touch-point A: `kernel/src/fs/neofs/cache.rs` - `drain_pass` loop, after the snapshot acquire (line ~498).**

After taking `snapshot = (block, bytes, role)`, emit:
```
NEO_DIAG/1 event=FSYNC_DRAIN_ITER name=<scenario> block=<N> role=<R> dirty=<D> drained=<C> flush_req=<bool> completions=<X> flushes=<Y> in_flight_dma=<Z> last_wake=<W>
```
- `completions/flushes/in_flight_dma/last_wake` pulled from existing virtio_blk debug_ accessors (lock-free atomic loads).
- `dirty/drained/flush_req` pulled from a single locked `stats_locked(&inner)` already done in this scope.
- **Classification value:** every drain iteration emits one frame. If forward progress is happening, frames keep arriving with incrementing `drained` and `completions`. If stuck mid-write, frames stop AFTER the last one, and the last frame's `in_flight_dma > 0` + flat `completions` = Case B; `in_flight_dma == 0` + last_wake in {Gone, Stale} = Case C; `in_flight_dma == 0` + last_wake == Woken with no further drain frames = Case D.

**Touch-point B: `kernel/src/fs/neofs/cache.rs` - `drain_all` yield branch (line ~246).**

After `sched::yield_now()` when `report.drained_blocks == 0`, emit:
```
NEO_DIAG/1 event=FSYNC_DRAIN_YIELD name=<scenario> dirty=<D> flush_req=<bool> flushes=<Y> in_flight_dma=<Z>
```
- **Classification value:** if `dirty==0 && flush_req==true && flushes` flat = Case E (stuck in post-drain flush retry loop). If `dirty>0 && in_flight_dma==0` and we keep yielding = Case D or D-adjacent (worker park/wake mismatch).

**Touch-point C: `kernel/src/test_harness.rs` - already done.** The PROGRESS frames already bracket the drain region. The new diag frames live between mount_done and seed_fsync_done.

### 6.2 xtask: on-timeout monitor dump (env-gated, off by default)

In `run_with_timeout` (line 2175, before `child.kill()`), when `NEO_QEMU_MONITOR_PORT` env is set AND a new `NEO_QEMU_TIMEOUT_DUMP=1` env is set, open a TCP connection to that monitor port and issue:
```
info status
info threads
info registers -a
x/8i $rip
```
Sleep 100-200ms for QEMU to drain, then SIGKILL. Append the dump to the attempt's serial log (or a sibling `.timeout-dump.log` to keep the serial log clean).

- **Classification value:** if `info status` returns `running` + `info registers` shows all 4 vCPUs spinning in HLT or with same RIP across two snapshots = Case A (or A-adjacent: live but idle, no work). If RIPs are in fsync/drain code = Case B/C/D/E.

### 6.3 Why this is the smallest seam

- **No scheduler/virtio code path changes.** Reads existing atomics + reuses existing lock acquisitions.
- **No new locks, no new IPIs, no new IRQ handlers, no new kthreads.**
- **Off-by-default.** Builds without the feature flag are byte-identical to today.
- **Bounded frame rate.** One frame per dirty block (<= submission rate); one frame per yield (<= yield rate). Both are gated by I/O completion cadence, so when the system is stuck, the frames stop - which IS the diagnostic signal.
- **Reuses existing OOB substrate.** Doesn't require writing a GDB stub or changing the NMI handler; just wires the existing monitor-port env to fire on timeout.

## 7. Risk of perturbing the timeout tail

| Risk | Magnitude | Mitigation |
|---|---|---|
| Serial bandwidth saturation | LOW. One frame ~ 200 bytes. At 1 frame/block drained x ~16 blocks/sec under stress = 3.2 KiB/sec vs ~11 KiB/sec serial-stdio practical limit. ~30% headroom. | If observed under load, raise feature-gate to fire every Nth iteration instead of every iteration. |
| Lock-hold extension in drain_pass | LOW. The frame is emitted *after* the lock is released around `write_fs_block`. No additional lock acquisition introduced; the stats snapshot folds into the existing locked region that already runs to compute the snapshot. | Verify by reading drain_pass - confirmed: stats can be derived from same locked window. |
| Hides the bug by changing timing | **MEDIUM - this is the real risk.** Adding serial I/O inside the drain loop changes per-block latency. If the bug is a tight timing race, instrumented runs may stop reproducing. | Mitigation: run *uninstrumented* loops in parallel to confirm the failure rate is unchanged with the feature off. Only ANALYZE failing runs with the feature on. If instrumented runs never fail, the act of instrumentation itself becomes diagnostic evidence (timing-sensitive bug, supports Case F or Case D race). |
| QEMU monitor dump path adds Rust dependencies | LOW. Use `std::net::TcpStream` from xtask; already-linked stdlib. No new crates. | None needed. |
| On-timeout monitor dump perturbs Case A diagnosis | LOW. If host actually froze, monitor TCP connect will time out fast (set 1s connect timeout). Dump failure is itself an A-signal. | Set explicit connect/read timeouts; fall back to SIGKILL after 500ms. |

**Bottom line:** the dominant risk is the classic observer-effect - instrumentation hiding a timing bug. The mitigation is to keep the failure-rate baseline running uninstrumented (current Fix 15 substrate), and only enable diag frames on a *subset* of the loop runs. If failure rates diverge, that itself classifies toward Case D/F.

## 8. Open items for PM confirmation before instrumentation lands

1. Feature-gate name: `neofs-diag-fsync-heartbeat` vs piggyback on existing `neo-test`?
2. Frame rate policy: every-iteration vs every-Nth-iteration default?
3. Monitor-port-on-timeout: opt-in via new `NEO_QEMU_TIMEOUT_DUMP=1`, or always-on when monitor port is set?
4. Acceptable to launch parallel **uninstrumented** + **instrumented** loops on separate containers to control for observer-effect?

---

**Audit complete; no code changes made.** All six audit questions answered above.
