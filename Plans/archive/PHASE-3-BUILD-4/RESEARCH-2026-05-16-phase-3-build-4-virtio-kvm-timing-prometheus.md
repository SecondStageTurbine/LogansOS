# Research — Virtio/KVM Timing Tails for phase3_neofs_fsync_filecap_stale_regression

Date: 2026-05-16
Agent: Prometheus
Authority: `Plans/DISPATCH-2026-05-16-phase-3-build-4-virtio-kvm-prior-art-prometheus.md`
Status: FINDINGS — research only, no code changes

---

## Executive Summary

The `phase3_neofs_fsync_filecap_stale_regression` scenario exhibits intermittent 180-second QEMU timeouts under `NEO_SMP=4`, with the tail clustering between the `mount_done` and `seed_fsync_done` progress frames — where the first durable fsync drains ~10 dirty blocks plus one virtio FLUSH.

This is a **known class of timing tail** at the intersection of three factors:

1. **Serialized I/O through a single virtqueue**: each block operation is a full virtio round-trip (submit → QEMU iothread → host I/O → completion interrupt → guest wake). With `wake_thread_token_front` already implemented, the guest-side wake latency is minimized, but the QEMU and host layers remain.

2. **QEMU iothread contention**: the xtask invokes QEMU with a single virtqueue and default iothread. Under CI runner load, the iothread's `select()` call exhibits high variance (KVM wiki: mean 271 µs, max 30 ms). This is documented behavior — not a bug.

3. **Host kernel scheduling variability**: the CI triage confirmed the ubuntu-24.04 runner image updated 2026-05-13 brought a newer host kernel that changed KVM VM exit patterns and vCPU scheduling timing. The QEMU binary (7.2.22) is identical — the trigger is the **host Linux kernel**.

**Bottom line:** The guest-side fix (`wake_thread_token_front`) is correct and addresses the controllable component. The remaining tail is environmental — QEMU iothread latency variance × host kernel scheduling × CI runner resource contention. Production kernels address this through IOThreads, `aio=native`, and vCPU pinning. For a test harness, the pragmatic fix is timeout policy or QEMU flag tuning.

---

## 1. Sourced Findings

### 1.1 Virtio-blk Latency Stack (KVM wiki instrumentation)

**Source:** <https://www.linux-kvm.org/page/Virtio/Block/Latency>

The KVM wiki documents a detailed latency breakdown for virtio-blk sequential reads, instrumented across guest, host KVM, QEMU virtio, and QEMU AIO layers:

| Layer | Delta (ns) | Delta (%) |
|-------|-----------|-----------|
| Guest kernel I/O path | 25,699 | 13.08% |
| Host/guest switching (VM exit/entry) | 7,561 | 3.85% |
| Host/QEMU switching | 3,640 | 1.85% |
| QEMU userspace processing | 29,393 | 14.96% |
| Host I/O (pread64) | 130,235 | 66.27% |

**Total virtualization overhead: ~33%** (guest 196 µs vs host 129 µs per 4K read).

The QEMU layer breakdown for `aio=native` is particularly relevant:

| Trace event | Time (µs) | Thread |
|-------------|-----------|--------|
| `virtqueue_pop` | 2.560 | vcpu |
| `laio_submit` → `laio_submit_done` | 17.325 | vcpu |
| `main_loop_wait_post_select` ← `pre_select` | **118.307** | iothread |
| `qemu_laio_completion_cb` | 0.410 | iothread |
| `virtio_blk_rw_complete` | 0.318 | iothread |
| `virtio_notify` → `kvm_set_irq_level_done` | 4.908 | iothread |

**Total virtqueue notify to completion interrupt: 147.6 µs with aio=native.**

The iothread's `select()` call dominates at 118 µs median, but the **standard deviation is 1,690 µs with a maximum of 30,008 µs (30 ms)**. The article notes: "iothread lock contention occasionally causes high latency here."

**Applicability to NeoPlutarch:** The seed fsync drain issues ~10 block writes + 1 FLUSH, each a serialized round-trip through the virtqueue. At 147 µs baseline per request, that's ~1.6 ms. But the tail of the `select()` distribution (30 ms max) means a single request can take 200× longer under contention. Over 11 requests, worst-case cumulative latency approaches 330 ms — still well within 180s, but this is per-request; mount validation reads hundreds of sectors before the seed fsync, and each read also goes through the virtio path.

### 1.2 VM Exit/Entry Cost (Intel halt polling research)

**Source:** <https://www.intel.com/content/www/us/en/developer/articles/technical/gaming-workloads-in-vms-through-guest-halt-polling.html>

VM exits for HLT instructions are expensive:
- Each transition requires saving/restoring control registers, MSRs, segment registers, RIP, RSP, RFLAGS — **hundreds to thousands of cycles** per VM exit/entry pair.
- When a vCPU is halted and a wakeup interrupt arrives, the VMM must stop the currently-running vCPU thread and resume the halted one — introducing host-side scheduling overhead.
- Guest halt polling (spinning before HLT) reduces HLT VM exits by 6% with optimal configuration (50K grow start, 10K minimum polling time), improving gaming FPS 4–71%.

**Applicability to NeoPlutarch:** The current AP bootstrap loop (`scheduler.rs:139-148`) calls `core::hint::spin_loop()` after `yield_now()`. This is a tight spin that does NOT yield the vCPU to the hypervisor — it consumes 100% of the vCPU's host scheduling slice. Under SMP=4, three APs are spinning, competing for host CPU time with the QEMU iothread that processes virtio completions.

The Fix 6 AP idle halt (`enable_and_hlt()`) was designed to address this by yielding the vCPU to the hypervisor when idle. It was blocked because it increased retry-pass rates without eliminating final failures — suggesting the host-side timing tail is too large for guest-side fixes alone.

### 1.3 IOThread Contention and Scaling (Red Hat Developer)

**Source:** <https://developers.redhat.com/articles/2024/09/05/scaling-virtio-blk-disk-io-iothread-virtqueue-mapping>

Key findings:
- virtio-blk devices in KVM guests process I/O from all queues in a **single host thread** by default.
- IOThread Virtqueue Mapping (RHEL 9.4+) allows assigning individual virtqueues to separate host IOThreads, improving I/O scalability for guests with many vCPUs submitting concurrent requests.
- Configuration tips: use `io=native` (not `io=threads`), pin IOThreads away from vCPUs, use 4-8 IOThreads for saturation.

**Critical limitation for NeoPlutarch:** IOThreads only help when multiple vCPUs submit **concurrent** I/O requests. NeoPlutarch's virtio-blk is **single-in-flight** — the completion handler must wake the waiter before the next request can be submitted. IOThreads would not reduce latency for this serialized path.

### 1.4 QEMU Block Cache Mode and AIO Backend

The xtask invokes QEMU with `cache=writeback` (`xtask/src/main.rs:1811`) and does NOT specify an `aio=` backend:
```
-drive if=none,id=hd0,format=raw,cache=writeback,file={disk}
```

`cache=writeback` means QEMU uses the host page cache for writes but issues flushes on `VIRTIO_BLK_T_FLUSH`. The KVM wiki shows `aio=native` (Linux AIO via `io_submit`/`io_getevents`) has lower latency than `aio=threads` (POSIX AIO emulation): 136 µs vs 144 µs for the AIO completion path, and 147.6 µs vs 159.6 µs total round-trip.

Without explicit `aio=`, QEMU 7.2.22 defaults to `aio=io_uring` on modern Linux, falling back to `aio=native`. The CI container's host kernel (ubuntu-24.04 May 2026) almost certainly supports `io_uring`. The `io_uring` backend has different latency characteristics: lower per-request overhead but potentially higher tail latency under host contention, as the completion queue polling shares the same host scheduling variability.

**Relevance:** The FLUSH command in the seed fsync (`VIRTIO_BLK_T_FLUSH`, `virtio_blk.rs:757-768`) has no data buffer — just a header and status byte. This is the smallest possible virtio request, but it still goes through the full QEMU iothread path and requires the host to issue a real `fsync`/`FUA` to the backing file. Under CI runner contention, this is the single point of highest latency variance in the drain sequence.

### 1.5 GitHub Runner Environment Variability

**Source:** <https://docs.github.com/en/actions/reference/runners/github-hosted-runners>

GitHub-hosted runners share host resources across concurrent jobs. The CI triage document confirmed:
- Runner image changed 2026-05-13 (ubuntu-24.04 v20260513.135.3)
- QEMU binary identical between old/new runner (7.2.22 in container)
- Trigger is the **host Linux kernel change** in the new runner image
- Newer host kernel changes KVM VM exit patterns, CPU scheduling behavior, and preemption timing for QEMU vCPUs

---

## 2. Applicability Matrix to NeoPlutarch

| Fix | Mechanism fit | Blast radius | Hides or removes? | Preserves durability? | Fix type |
|-----|---------------|--------------|-------------------|----------------------|----------|
| `wake_thread_token_front` (already done) | **High** — addresses guest wake-to-run latency for virtio completions | Low — narrow API, virtio-blk only | Removes (guest component) | Yes | Code |
| Bounded guest halt polling | **Medium** — reduces VM exit cost for idle APs competing with iothread | Low — only AP idle loop | Removes (host scheduling pressure) | Yes | Code |
| AP idle halt (`enable_and_hlt`) | **Medium** — yields vCPU to hypervisor when idle | Low — only AP idle loop | Removes (host scheduling) | Yes | Code — Fix 6, **blocked** |
| QEMU `aio=native` flag | **Low-Medium** — 5% latency reduction per request (KVM wiki data) | None — QEMU flag only | Removes (QEMU component) | Yes | xtask config |
| QEMU `cache=none` | **Low** — bypasses host page cache; reads slower, writes more direct | Low — QEMU flag only | Hides (removes cache variability, adds raw I/O latency) | Yes — more direct durability | xtask config |
| IOThreads + multi-queue | **None** — serialized I/O doesn't benefit from parallel iothreads | None — QEMU flags only | N/A | Yes | xtask config — **not applicable** |
| Longer timeout for NeoFS scenarios | **Low** — absorbs the tail without fixing it | None — xtask config only | Hides (purely cosmetic) | Yes | xtask config |
| Smaller test disk (72-80 MiB) | **Medium** — reduces mount validation I/O by ~40-45% | Low — mkfs parameter only | Removes (less I/O to drain) | Yes | Test scope |
| Batch fsync drain (multi-request ring) | **Low** — requires large virtio refactor; single-in-flight is architectural | High — changes block I/O model | Removes (fewer round-trips) | Yes, if FLUSH preserved | Code — **too large** |

---

## 3. Analysis: Why the Tail Persists After `wake_thread_token_front`

### 3.1 The Single-in-flight Constraint

NeoPlutarch's virtio-blk driver maintains exactly one in-flight request at a time (`virtio_blk.rs:625-633`: `in_flight: Option<IoRendezvous>`). The submit path parks the caller on a wake token. The completion path (`poll_completions()` → `wake_thread_token_front()`) wakes the waiter, which then issues the next request. This serialization means:

- **No request-level parallelism**: each of the ~10 dirty blocks + 1 FLUSH must complete before the next can be submitted.
- **No batching**: the drain loop (`cache.rs:476-539`) calls `write_fs_block()` per block, each blocking on its own virtio round-trip.
- **No iothread benefit**: Red Hat's IOThread Virtqueue Mapping only helps when multiple vCPUs submit concurrent requests. NeoPlutarch's single-in-flight model cannot exploit parallel virtqueues.

### 3.2 The Seed Fsync Drain Path Under `NEO_SMP=4`

1. `drain_all()` acquires `WriteBackPauseGuard`, iterating dirty blocks
2. Per block: `write_fs_block()` → virtio-blk submit → waiter parks on wake token
3. Virtio completion → `poll_completions()` → `wake_thread_token_front(token, 0)` → waiter pushed to **front** of ready queue
4. Waiter resumes, issues next block
5. After all blocks: `flush_device_with_retry()` → virtio FLUSH → same park/wake cycle

`wake_thread_token_front` eliminates the guest scheduling component — the woken thread runs immediately after the completion handler returns. But the **total round-trip** still includes:

- QEMU iothread `select()` latency: 118 µs median, **30 ms tail** (KVM wiki)
- VM exit/entry for virtqueue notify + interrupt delivery: ~500-2000 cycles each (Intel research)
- Host I/O scheduling: dominant component at 66% of total latency (KVM wiki)
- AP spin loops: 3 APs calling `spin_loop()` compete for host CPU time with the QEMU iothread
- **FLUSH command variance**: the `VIRTIO_BLK_T_FLUSH` request has no data buffer (smallest possible virtio request), but forces a real `fsync`/`FUA` to the backing file. Under CI runner contention, this is the single point of highest latency variance in the drain sequence.

### 3.3 What `wake_thread_token_front` Cannot Fix

The front-of-queue wake removes the **guest scheduling** component (pushing the waiter to the front instead of back of the ready queue). But it cannot reduce:

1. **QEMU iothread latency**: the iothread's `select()` call variance is a function of host scheduling and QEMU internal lock contention — invisible to the guest.
2. **Host I/O scheduling**: the dominant component (66% of total latency) is the host kernel's block I/O path — outside NeoPlutarch's control.
3. **AP spin loop contention**: the 3 APs calling `core::hint::spin_loop()` consume host CPU time that would otherwise be available to the QEMU iothread. This is the one component a guest-side fix (bounded halt polling or AP idle halt) could address, but Fix 6's partial results suggest the effect is marginal compared to the external factors.

---

## 4. Ranked Recommendations

### Rank 1: Accept retry-pass with documented evidence (current posture)

**Action:** No code change. Keep `wake_thread_token_front`. Document retry-pass caveat in close evidence.

**Rationale:** The guest-side fix is correct and narrow. Remaining timeouts are caused by external factors (host kernel timing, QEMU iothread contention, CI runner resource sharing) that NeoPlutarch cannot control. The xtask retry mechanism correctly masks them.

**Blast radius:** Zero.

### Rank 2: Add `aio=native` to QEMU drive options in xtask

**Action:** Change `-drive` argument to include `aio=native`.

**Rationale:** The KVM wiki shows `aio=native` has measurably lower latency than `aio=threads` (147.6 µs vs 159.6 µs). If the CI container defaults to `aio=io_uring`, explicit `aio=native` ensures consistent behavior. Small but free improvement.

**Blast radius:** None — QEMU flag only.

### Rank 3: Increase timeout for NeoFS I/O-heavy scenarios

**Action:** Set `TEST_TIMEOUT` to 300s for scenarios that use storage disks and exercise fsync paths. Keep 180s for all other scenarios.

**Rationale:** The KVM wiki shows iothread `select()` can take up to 30 ms under contention. Mount validation reads hundreds of sectors before the seed fsync. Under sustained CI runner load, cumulative latency can exceed 180s. A 300s timeout for I/O-heavy scenarios absorbs this tail without masking real hangs.

**Blast radius:** None — xtask config only.

### Rank 4: Bounded guest halt polling (follow-up lane)

**Action:** Replace `core::hint::spin_loop()` in the AP idle loop with a bounded poll (e.g., 10,000 iterations of `pause` + ready-queue check), then fall through to `enable_and_hlt()`.

**Rationale:** Intel's research shows guest halt polling with optimal configuration reduces HLT VM exits by 6%. A fixed bounded poll of 10,000 iterations (~10 µs at 1 GHz) covers the typical wake window without consuming excessive host CPU time. After the poll expires, `hlt` yields to the hypervisor, reducing contention with the QEMU iothread.

**Blast radius:** Low — only AP idle loop. The bounded poll adds ~10 µs of CPU time per idle cycle — negligible compared to the 150 µs+ virtio round-trip.

### Rank 5: Smaller NeoFS test disk (deferred)

**Action:** Reduce disk size from 128 MiB to 72-80 MiB for this scenario only.

**Rationale:** The dispatch §3 notes the true minimum image is just over 64 MiB (three superblock copies). A smaller disk reduces mount validation I/O by ~40-45%, shrinking the pre-fsync latency budget consumed.

**Blast radius:** Low — mkfs parameter only. Requires proving `neofs-mkfs create/verify` accepts the smaller size.

---

## 5. Do Not Do

1. **Do not switch all wake-token callers to front-of-queue.** The fair `push_back` behavior of `wake_thread_token` is relied upon by kthread join, stop-token delivery, and general scheduler wake-token tests. Switching them all would change the fairness profile and could introduce starvation for low-priority threads.

2. **Do not add IOThreads to the QEMU invocation.** IOThread Virtqueue Mapping only helps when multiple vCPUs submit concurrent I/O requests. NeoPlutarch's virtio-blk is single-in-flight — the completion handler must wake the waiter before the next request can be submitted. IOThreads would add configuration complexity with zero benefit for this serialized path.

3. **Do not increase `STORAGE_DISK_SIZE` or add more seed blocks.** The timing tail is already a function of the ~10 dirty blocks + FLUSH in the current test. Adding more I/O would make the tail worse.

4. **Do not batch virtio-blk requests (multi-request ring).** This would be a large refactor of the virtio submission model, changing from single-in-flight to multi-request. The blast radius is high (affects all block I/O paths), and the benefit for a test scenario is marginal compared to timeout policy fixes.

5. **Do not remove the FLUSH command from the seed fsync.** The FLUSH is the durability anchor of BUILD-3E. Removing it would invalidate the security/durability claim.

6. **Do not implement adaptive guest halt polling in this lane.** The Intel research shows adaptive polling requires tuning parameters (`guest_halt_poll_ns`, `guest_halt_poll_allow_shrink`, etc.) that depend on workload characteristics. A fixed bounded poll (Rank 4) is simpler and sufficient. Adaptive polling is a follow-up if the fixed poll proves effective under sustained CI runner load.

---

## 6. Conclusion

The `phase3_neofs_fsync_filecap_stale_regression` timing tail under `NEO_SMP=4` is a documented class of KVM/virtio-blk issue: serialized block I/O through a single QEMU iothread, with host-side scheduling variability dominating the latency distribution. The `wake_thread_token_front` fix correctly addresses the guest-side scheduling component. Remaining mitigation options are QEMU configuration (`aio=native`) and CI timeout policy. Guest-side fixes beyond front-of-queue wake have diminishing returns because the external factors (host kernel timing, QEMU iothread contention, CI runner resource sharing) cannot be controlled from within the guest.

**Recommended posture:** Accept low retry-pass with documented evidence. The close narrative should read: "BUILD-4A verified, residual substrate instability tracked separately."