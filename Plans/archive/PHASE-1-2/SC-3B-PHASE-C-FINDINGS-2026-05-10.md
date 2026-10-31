# SC-3B Phase C NMI capture findings

Date: 2026-05-10
Status: NMI capture obtained on run 005; tracked substrate restored clean

## Substrate

Starting and ending HEAD:

```text
2d8f65da77b4ea10f3d53226724e9381961c48c0
```

Stashes at start and exit:

```text
stash@{0}: On main: sc-3b-fix-6-1-attempt
stash@{1}: On main: wedge-investigation-8-nmi-watchdog
stash@{2}: On main: wedge-investigation-2-instrumentation
stash@{3}: On main: h1-experiment-instrumentation
```

Applied instrumentation:

```text
git stash apply stash@{1}
```

Adapted drift from the stash:

```text
sched.inner.is_locked() no longer exists on SwitchMutex.
Replaced with try_lock() inference:
let inner = sched.inner.try_lock();
let locked = inner.is_none();
...
if let Some(inner) = inner { ... }
```

Phase C deltas added on top of the stash:

```text
- NMI channel dump for stress fixture channels only, using try_lock().
- PROGRESS frames every 50 stress iterations.
- WORKER frames around stress sender/receiver IPC calls with cpu_id.
- Throwaway host wrapper for NMI-on-post-BEGIN serial silence.
```

Build gates:

```text
docker compose run --rm neo cargo k
PASS
`altimit-neo-kernel` (bin "kernel") generated 18 warnings
warning delta from baseline 18: +0
```

Sibling SMP sanity gates at NEO_SMP=4:

```text
NEO_TEST/1 event=PASS name=smp_block_release_smoke blocked_cpu=0 completer_cpu=1 resumed_cpu=1 release_path=asm
NEO_TEST/1 event=PASS name=smp_send_blocks_first_then_completes blocked_cpu=0 completer_cpu=1 resumed_cpu=1
NEO_TEST/1 event=PASS name=smp_recv_blocks_first_then_completes blocked_cpu=0 completer_cpu=1 resumed_cpu=1
NEO_TEST/1 event=PASS name=smp_grant_cross_cpu blocked_cpu=0 completer_cpu=1 resumed_cpu=1 grant_rights=0x101 handle_idx=0
NEO_TEST/1 event=PASS name=smp_cross_cpu_migration_reap blocked_cpu=0 resumed_cpu=1 exit_reaped=ok
```

Instrumented stress sanity:

```text
NEO_TEST/1 event=PASS name=smp_cross_cpu_rendezvous_stress iterations=1000 send_first=500 recv_first=500 cpus=4
Observed 20 PROGRESS frames and 4000 WORKER frames in the sanity serial log.
```

## Run summary

Run shape:

```text
Scenario: smp_cross_cpu_rendezvous_stress
NEO_SMP: 4
NMI trigger: 10s post-BEGIN serial silence
Early stop: first NMI capture
Artifact directory: target/verify/sc3b-phase-c-nmi/neo-smp4/
Results index: target/verify/sc3b-phase-c-nmi/neo-smp4/results.jsonl
```

Results:

```text
5 runs executed before early stop
4 PASS
1 TIMEOUT_IN_KERNEL_STRESS with NMI capture
0 FAIL
0 PANIC
0 empty serial logs
```

Timing:

```text
min elapsed: 21.657s
max elapsed: 116.053s
avg elapsed: 66.667s
```

Per-run summary:

```text
run 001: PASS, elapsed=116.053s
run 002: PASS, elapsed=68.369s
run 003: PASS, elapsed=64.699s
run 004: PASS, elapsed=62.557s
run 005: TIMEOUT_IN_KERNEL_STRESS, nmi_status=OK, elapsed=21.657s, returncode=-9
```

PASS frame on runs 001-004:

```text
NEO_TEST/1 event=PASS name=smp_cross_cpu_rendezvous_stress iterations=1000 send_first=500 recv_first=500 cpus=4
```

## Per-trace analysis

### Run 005

Serial prefix:

```text
[ kernel ] sched: smoke test passed (5 ping, 5 pong, hog reaped, ticks=13)
NEO_TEST/1 event=BEGIN name=smp_cross_cpu_rendezvous_stress phase=dispatch
```

No PROGRESS frames were emitted before the hang. That places the wedge before global iteration 50.

Last WORKER trail:

```text
NEO_TEST/1 event=WORKER name=stress-sender pair=0 iter=1 action=pre_block cpu_id=0
NEO_TEST/1 event=WORKER name=stress-receiver pair=0 iter=1 action=pre_block cpu_id=1
NEO_TEST/1 event=WORKER name=stress-receiver pair=0 iter=1 action=post_wake cpu_id=1
NEO_TEST/1 event=WORKER name=stress-sender pair=0 iter=1 action=post_wake cpu_id=1
NEO_TEST/1 event=WORKER name=stress-receiver pair=1 iter=2 action=pre_block cpu_id=2
NEO_TEST/1 event=WORKER name=stress-sender pair=1 iter=2 action=pre_block cpu_id=1
NEO_TEST/1 event=WORKER name=stress-receiver pair=1 iter=2 action=post_wake cpu_id=1
NEO_TEST/1 event=WORKER name=stress-sender pair=1 iter=2 action=post_wake cpu_id=1
NEO_TEST/1 event=WORKER name=stress-sender pair=2 iter=3 action=pre_block cpu_id=2
NEO_TEST/1 event=WORKER name=stress-receiver pair=2 iter=3 action=pre_block cpu_id=3
NEO_TEST/1 event=WORKER name=stress-receiver pair=2 iter=3 action=post_wake cpu_id=3
NEO_TEST/1 event=WORKER name=stress-sender pair=2 iter=3 action=post_wake cpu_id=3
NEO_TEST/1 event=WORKER name=stress-receiver pair=3 iter=4 action=pre_block cpu_id=0
```

Key observation:

```text
pair=3 iter=4 receiver emitted pre_block on cpu_id=0.
No matching post_wake for that receiver.
No pair=3 iter=4 sender pre_block was observed.
```

CPU0 NMI header:

```text
[ NMI ] cpu=0 rip=0xfffffffff52599e2 rsp=0xfffffffff5a28a88 rflags=0x0000000000000002 cs=0x8 ss=0x0 rbp_snapshot=0x0000000000000000
```

Symbol resolution:

```text
runtime kernel virt base: 0xfffffffff51ef000
link-time LOAD base:      0xffffffff80000000
runtime RIP:              0xfffffffff52599e2
linked RIP:               0xffffffff8006a9e2

addr2line:
core::core_arch::x86::sse2::_mm_pause
/home/neo/.rustup/toolchains/nightly-2026-04-16-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src/../../stdarch/crates/core_arch/src/x86/sse2.rs:26
```

Backtrace limitation:

```text
[ NMI ] backtrace saved_rsp=0xfffffffff5a28a88
[ NMI ] backtrace skipped rbp_out_of_current_stack_window rbp=0x0000000000000000
```

Scheduler snapshot:

```text
[ NMI ] sched cpu=0 ptr=0xfffffffff5a002e8 zombie_count=0 inner_locked=false
[ NMI ] sched cpu=0 current id=14 name=stress-receiver pid=0 state=Running queue=current rsp=0xfffffffff5a288d8
[ NMI ] sched cpu=0 ready_len=1 zombies_len=0
[ NMI ] sched cpu=0 ready[0] id=0 name=main pid=0 state=Ready rsp=0xffff80001ffa45f8
[ NMI ] sched cpu=1 ptr=0xfffffffff5a00cc0 zombie_count=0 inner_locked=true
[ NMI ] sched cpu=1 inner_snapshot=unavailable try_lock_failed
[ NMI ] sched cpu=2 ptr=0xfffffffff5a00978 zombie_count=0 inner_locked=false
[ NMI ] sched cpu=2 current id=2 name=ap-2-main pid=0 state=Running queue=current rsp=0xffff80001eac4d68
[ NMI ] sched cpu=2 ready_len=0 zombies_len=0
[ NMI ] sched cpu=3 ptr=0xfffffffff5a00870 zombie_count=0 inner_locked=false
[ NMI ] sched cpu=3 current id=1 name=ap-3-main pid=0 state=Running queue=current rsp=0xffff80001eab4d68
[ NMI ] sched cpu=3 ready_len=3 zombies_len=0
[ NMI ] sched cpu=3 ready[0] id=13 name=stress-sender pid=0 state=Ready rsp=0xfffffffff5a24bc8
[ NMI ] sched cpu=3 ready[1] id=12 name=stress-receiver pid=0 state=Ready rsp=0xfffffffff5a20bc8
[ NMI ] sched cpu=3 ready[2] id=11 name=stress-sender pid=0 state=Ready rsp=0xfffffffff5a1cbc8
```

Per-channel state dump:

```text
[ NMI ] channel cpu=0 ch=0 side=A state=Idle parked_id=none
[ NMI ] channel cpu=0 ch=0 side=B state=Idle parked_id=none
[ NMI ] channel cpu=0 ch=1 side=A state=Idle parked_id=none
[ NMI ] channel cpu=0 ch=1 side=B state=Idle parked_id=none
[ NMI ] channel cpu=0 ch=2 side=A state=Idle parked_id=none
[ NMI ] channel cpu=0 ch=2 side=B state=Idle parked_id=none
[ NMI ] channel cpu=0 ch=3 side=all state=Locked parked_id=none
```

NMI limitation:

```text
Only CPU0 emitted an NMI handler frame, matching prior watchdog limitations.
The CPU0 handler did read all per-CPU scheduler snapshots and the stress channel registry.
AP RIP/RFLAGS frames are still not available from this HMP NMI path.
```

## Interpretation

This capture does not support the simple H1-variant shape "worker parked in EndpointState::SendBlocked or EndpointState::RecvBlocked and peer never wakes it." Channels 0-2 were Idle, and channel 3 could not be inspected because its state lock was held.

The strongest direct facts are:

```text
The hang occurs very early: pair=3, iter=4, before any PROGRESS frame.
CPU0 current thread is stress-receiver id=14.
The last worker event is stress-receiver pair=3 iter=4 pre_block on CPU0.
CPU0 RIP resolves to _mm_pause.
Channel 3 is locked.
CPU1 scheduler inner lock is locked and unavailable to try_lock.
CPU3 is running ap-3-main while three stress workers are Ready.
No zombie/reap inconsistency is visible.
```

Most likely mechanism class:

```text
Channel/scheduler lock interaction or lock-holder stall during the pair=3 recv-first block path.
```

The inference is that CPU0 is spinning in a lock path after the pair=3 receiver enters the IPC operation. Because channel 3 is locked and the last observed worker is that receiver's pre_block frame, the channel lock is now the highest-value object to instrument. The CPU1 scheduler lock being held at the same snapshot is also load-bearing: this differs from Outcome B run 026, where all scheduler locks were available.

## Comparison to Outcome B and closure-gate signatures

Closure-gate retry run 010 and Sub-step-5b run 052:

```text
BEGIN emitted.
No terminal PASS/FAIL frame.
No BUG or panic.
Host eventually timed out.
```

Outcome B run 026:

```text
CPU0 interrupted in _mm_pause.
CPU2/CPU3 were running AP bootstrap threads while stress workers were Ready.
All scheduler inner locks were available.
Channel endpoint state was not dumped.
```

Phase C run 005:

```text
CPU0 interrupted in _mm_pause.
CPU2/CPU3 are again AP-main-running/ready-work-visible, but CPU3 has three ready stress workers.
CPU1 scheduler inner is locked.
Channel 3 is locked.
No parked SendBlocked/RecvBlocked endpoint was observed because the relevant channel could not be read.
```

Conclusion:

```text
This is not a clean Outcome C-A parked-thread confirmation.
This is not a clean Outcome C-C identical Outcome-B layout either.
It is Outcome C-B: scheduler-progress residual plus a newly visible channel/scheduler lock interaction.
```

## Recommendation

H1-variant:

```text
Unresolved but not confirmed. The load-bearing channel was Locked, not inspectable.
The simple lost-wakeup shape is disfavored for channels 0-2 and unproven for channel 3.
```

Hp-residual:

```text
Supported in part. CPU3 is running ap-3-main while ready stress workers exist, matching the broad scheduler-progress family.
Not identical to Outcome B because CPU1 scheduler lock is held and channel 3 is locked.
```

Mechanism-not-yet-named:

```text
Supported. The new signature is pair=3 iter=4 receiver pre_block -> CPU0 _mm_pause -> channel 3 Locked -> CPU1 scheduler inner Locked.
```

Recommended next dispatch:

```text
Instrument lock ownership, not more queue contents.

Add feature-gated owner metadata for the two relevant SwitchMutex users:
- Channel.state: owner_cpu, owner_thread_id, owner_site.
- PerCpuScheduler.inner: owner_cpu, owner_thread_id, owner_site.

On try_lock failure in NMI, print the recorded owner metadata.
Add narrow trace frames inside block_current_into_with_guard around:
- channel guard already held
- scheduler lock acquire attempt/success
- installer complete
- release-token handoff to context_switch_with_release2

Then rerun Phase C shape. The next useful fact is "who owns channel 3 and CPU1 scheduler lock at the snapshot," not another generic NMI queue dump.
```

## Exit status

Backup:

```text
Z:\AltimitNeo\2026-05-10-090353
Verification: OK (HEAD matches, commit count >= source, all stashes preserved)
```

Tree restore:

```text
git status --short -uno
[no output]

git rev-parse HEAD
2d8f65da77b4ea10f3d53226724e9381961c48c0

git stash list
stash@{0}: On main: sc-3b-fix-6-1-attempt
stash@{1}: On main: wedge-investigation-8-nmi-watchdog
stash@{2}: On main: wedge-investigation-2-instrumentation
stash@{3}: On main: h1-experiment-instrumentation
```

Clean-substrate build after restore:

```text
docker compose run --rm neo cargo k
PASS
`altimit-neo-kernel` (bin "kernel") generated 18 warnings
```

