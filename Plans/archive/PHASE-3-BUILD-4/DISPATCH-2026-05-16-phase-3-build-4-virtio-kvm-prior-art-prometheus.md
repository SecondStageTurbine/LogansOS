# Dispatch - Phase 3 BUILD-4 Virtio/KVM Timing Prior Art

Date: 2026-05-16
Agent: Prometheus
Status: AUTHORIZED RESEARCH
Authority:

- `Plans/PHASE-3-BUILD-4-STABILITY-FIX-2-BLOCKER-2026-05-16.md`
- `Plans/DISPATCH-2026-05-16-phase-3-build-4-stability-fix-3-attempt-logs-and-io-wake-cerebro.md`

## 1. Mission

Research prior art for the remaining BUILD-4 stability issue:

```text
phase3_neofs_fsync_filecap_stale_regression sometimes retry-times out under NEO_SMP=4.
Progress frames show the tail clusters between mount_done and seed_fsync_done,
where the first durable fsync drains ~10 dirty blocks plus one virtio FLUSH.
```

Find whether similar QEMU/KVM/virtio-blk timing tails have been reported, how
production kernels or hypervisor users address them, and which fixes map cleanly
to NeoPlutarch.

This is research only. No code changes.

## 2. Seed Sources

Start from these sources, then branch outward:

- KVM virtio-blk latency stack:
  `https://www.linux-kvm.org/page/Virtio/Block/Latency`
- KVM halt polling behavior:
  `https://www.ibm.com/docs/en/linux-on-systems?topic=linuxonibm/liaag/wkvm/wkvm_c_tune_kvm.html`
- Guest halt polling / HLT wake latency:
  `https://www.intel.com/content/www/us/en/developer/articles/technical/gaming-workloads-in-vms-through-guest-halt-polling.html`
- Virtio-blk IOThread / host contention:
  `https://developers.redhat.com/articles/2024/09/05/scaling-virtio-blk-disk-io-iothread-virtqueue-mapping`
- GitHub hosted runner environment variability:
  `https://docs.github.com/en/actions/reference/runners/github-hosted-runners`

## 3. Questions To Answer

Q1. Is this a known class of KVM/QEMU issue?

- Look for evidence around virtio-blk completion latency, vCPU scheduling,
  host/QEMU thread contention, and CI host timing variability.
- Separate primary sources from forum anecdotes.

Q2. What fixes are common?

Classify fixes into:

- guest scheduler fixes, such as waking I/O waiters at front of queue;
- guest idle policy fixes, such as halt polling vs immediate `hlt`;
- guest I/O batching fixes, such as draining multiple dirty blocks per fsync
  transaction without serial wake/sleep per block where safe;
- QEMU invocation fixes, such as `aio=native`, IOThreads, cache mode, or queue
  settings;
- CI policy fixes, such as longer timeout only for I/O-heavy scenarios or using
  self-hosted runners.

Q3. Which fixes fit NeoPlutarch now?

For each candidate, rate:

- mechanism fit;
- blast radius;
- whether it hides the timing problem or removes it;
- whether it preserves the security/durability claim of BUILD-3E;
- whether it should be a code fix, xtask/QEMU config fix, or test-scope fix.

Q4. What should PM authorize next if Fix 9 leaves retry-pass evidence?

Give a ranked recommendation. Current candidates:

- keep Fix 9 and accept low retry-pass with evidence;
- add bounded/adaptive guest idle polling rather than pure AP halt;
- reduce seed fsync I/O cost with a valid NeoFS disk size or scenario setup;
- batch fsync drain/flush behavior;
- change QEMU block options in xtask;
- isolate this as CI-runner flake and defer with documented timeout policy.

## 4. Output

File findings at:

```text
Plans/RESEARCH-2026-05-16-phase-3-build-4-virtio-kvm-timing-prometheus.md
```

Required structure:

- executive summary;
- sourced findings with links;
- exact quotes only where useful and short;
- applicability matrix to NeoPlutarch;
- ranked recommendation;
- explicit "do not do" list.

No commit unless PM asks.
