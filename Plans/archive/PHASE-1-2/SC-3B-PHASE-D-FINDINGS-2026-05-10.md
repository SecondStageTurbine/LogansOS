# SC-3B Phase D Findings: Lock Ownership Probe

**Date:** 2026-05-10  
**Substrate:** `2d8f65da77b4ea10f3d53226724e9381961c48c0`  
**Dispatch:** `Plans/DISPATCH-2026-05-10-sc-3b-phase-d-lock-ownership-cerebro.md`  
**Result:** One NMI capture at run 43. Mechanism is not Hd/Hr/Hc; it is a preempted lock owner plus irq-off same-CPU waiter.

---

## 1. Substrate

- Entry HEAD: `2d8f65da77b4ea10f3d53226724e9381961c48c0`.
- Entry tracked tree: clean.
- Stashes preserved:
  - `stash@{0}: On main: sc-3b-fix-6-1-attempt`
  - `stash@{1}: On main: wedge-investigation-8-nmi-watchdog`
  - `stash@{2}: On main: wedge-investigation-2-instrumentation`
  - `stash@{3}: On main: h1-experiment-instrumentation`
- Applied `stash@{1}` as Phase D NMI base and resolved the expected `idt.rs` conflict by preserving the post-5b IPI handlers plus the NMI watchdog handler.
- Added throwaway instrumentation only:
  - `SwitchMutex` owner metadata with acquire/release tracking and asm release-path metadata clear.
  - `BLOCK_TRACE` frames in `block_current_into_with_guard`.
  - NMI lock multi-probe and owner readout.
  - Phase C `PROGRESS` / `WORKER` / channel dump frames retained.
- No commit landed.

Build gates:

- Instrumented `cargo k`: PASS, 18 warnings, delta `+0` against baseline 18.
- Post-restore `cargo k`: PASS, 18 warnings.
- Feature-build warning count for `smp_cross_cpu_rendezvous_stress`: 39 warnings, existing feature-gating noise.

Sibling SMP sanity at `NEO_SMP=4`:

- `smp_block_release_smoke`: PASS, `blocked_cpu=0 completer_cpu=1 resumed_cpu=1 release_path=asm`.
- `smp_send_blocks_first_then_completes`: PASS, `blocked_cpu=0 completer_cpu=1 resumed_cpu=1`.
- `smp_recv_blocks_first_then_completes`: PASS, `blocked_cpu=0 completer_cpu=1 resumed_cpu=1`.
- `smp_grant_cross_cpu`: PASS, `blocked_cpu=0 completer_cpu=1 resumed_cpu=1 grant_rights=0x101 handle_idx=0`.
- `smp_cross_cpu_migration_reap`: PASS, `blocked_cpu=0 resumed_cpu=1 exit_reaped=ok`.

Instrumented stress sanity at `NEO_SMP=4`:

- `smp_cross_cpu_rendezvous_stress`: PASS.
- Serial counts: `BLOCK_TRACE=4000`, `WORKER=4000`, `PROGRESS=20`, final `PASS=1`.

---

## 2. Run Summary

Artifact directory: `target/verify/sc3b-phase-d-nmi/neo-smp4/`

Results index: `target/verify/sc3b-phase-d-nmi/neo-smp4/results.jsonl`

Run classification:

- `PASS`: 42
- `TIMEOUT_IN_KERNEL_STRESS` with `nmi_status=OK`: 1
- First NMI capture: run 43

Timing:

- PASS runs: min `117.379s`, max `169.453s`, average `127.867s`.
- All runs: min `22.490s`, max `169.453s`, average `125.417s`.

Capture artifact:

- `target/verify/sc3b-phase-d-nmi/neo-smp4/SC3B-PHASE-D-run-043.state-dump.log`
- `target/verify/sc3b-phase-d-nmi/neo-smp4/SC3B-PHASE-D-run-043.cargo.log`

---

## 3. Run 43 Trace

Serial prefix:

```text
81: [ hog  ] HOG_TRIGGER seen, exiting
82: [ kernel ] sched: reaped 'hog' (id=6)
83: [ kernel ] sched: smoke test passed (5 ping, 5 pong, hog reaped, ticks=13)
84: NEO_TEST/1 event=BEGIN name=smp_cross_cpu_rendezvous_stress phase=dispatch
```

Worker / block trail before silence:

```text
85: NEO_TEST/1 event=WORKER name=stress-sender pair=0 iter=1 action=pre_block cpu_id=0
86: NEO_TEST/1 event=BLOCK_TRACE phase=channel_guard_held channel_id=0 thread_id=7 cpu=0
87: NEO_TEST/1 event=BLOCK_TRACE phase=scheduler_lock_acquired channel_id=0 thread_id=7 cpu=0
88: NEO_TEST/1 event=BLOCK_TRACE phase=installer_complete channel_id=0 thread_id=7 cpu=0
89: NEO_TEST/1 event=BLOCK_TRACE phase=release_token_handoff channel_id=0 thread_id=7 cpu=0
...
101: NEO_TEST/1 event=WORKER name=stress-sender pair=2 iter=3 action=pre_block cpu_id=2
102: NEO_TEST/1 event=BLOCK_TRACE phase=channel_guard_held channel_id=2 thread_id=11 cpu=2
103: NEO_TEST/1 event=BLOCK_TRACE phase=scheduler_lock_acquired channel_id=2 thread_id=11 cpu=2
104: NEO_TEST/1 event=BLOCK_TRACE phase=installer_complete channel_id=2 thread_id=11 cpu=2
105: NEO_TEST/1 event=BLOCK_TRACE phase=release_token_handoff channel_id=2 thread_id=11 cpu=2
106: NEO_TEST/1 event=WORKER name=stress-receiver pair=2 iter=3 action=pre_block cpu_id=3
107: NEO_TEST/1 event=WORKER name=stress-receiver pair=2 iter=3 action=post_wake cpu_id=3
108: NEO_TEST/1 event=WORKER name=stress-sender pair=2 iter=3 action=post_wake cpu_id=3
109: NEO_TEST/1 event=WORKER name=stress-receiver pair=3 iter=4 action=pre_block cpu_id=0
```

Load-bearing observation: pair 3 receiver emitted `pre_block` but emitted no `BLOCK_TRACE`. It did not reach `channel_guard_held`; it is stuck before acquiring the channel guard.

NMI IP:

```text
110: [ NMI ] cpu=0 rip=0xffffffffac516392 rsp=0xffffffffacc24ad8 rflags=0x0000000000000002 cs=0x8 ss=0x0 rbp_snapshot=0x0000000000000000
```

KASLR translation:

- Runtime image base from log: `virt=0xffffffffac4a8000`.
- Runtime RIP: `0xffffffffac516392`.
- Linked RIP: `0xffffffff8006e392`.
- Symbol:

```text
core::core_arch::x86::sse2::_mm_pause at /home/neo/.rustup/toolchains/nightly-2026-04-16-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src/../../stdarch/crates/core_arch/src/x86/sse2.rs:26
```

`rflags=0x2` means IF is clear. CPU0 is spinning with interrupts disabled, so the timer cannot preempt it to run another Ready thread on CPU0.

Scheduler snapshot:

```text
112: [ NMI ] sched cpu=0 probe=1 ptr=0xffffffffacc00c28 zombie_count=0 inner_locked=false
113: [ NMI ] sched cpu=0 current id=14 name=stress-receiver pid=0 state=Running queue=current rsp=0xffffffffacc24958
114: [ NMI ] sched cpu=0 ready_len=1 zombies_len=0
115: [ NMI ] sched cpu=0 ready[0] id=0 name=main pid=0 state=Ready rsp=0xffff80001ff9c618
116: [ NMI ] sched cpu=1 probe=1 ptr=0xffffffffacc00ba0 zombie_count=0 inner_locked=false
117: [ NMI ] sched cpu=1 current id=7 name=stress-sender pid=0 state=Running queue=current rsp=0xffffffffacc08b88
118: [ NMI ] sched cpu=1 ready_len=4 zombies_len=0
123: [ NMI ] sched cpu=2 probe=1 ptr=0xffffffffacc00cb0 zombie_count=0 inner_locked=false
124: [ NMI ] sched cpu=2 current id=2 name=ap-2-main pid=0 state=Running queue=current rsp=0xffff80001eaaad68
126: [ NMI ] sched cpu=3 probe=1 ptr=0xffffffffacc00d38 zombie_count=0 inner_locked=false
127: [ NMI ] sched cpu=3 current id=3 name=ap-3-main pid=0 state=Running queue=current rsp=0xffff80001ea9ad68
128: [ NMI ] sched cpu=3 ready_len=3 zombies_len=0
```

All scheduler locks were available to the NMI probe. This disfavours an AB/BA channel-vs-scheduler deadlock.

Channel state:

```text
132: [ NMI ] channel cpu=0 ch=0 probe=1 side=A state=Idle parked_id=none
133: [ NMI ] channel cpu=0 ch=0 probe=1 side=B state=Idle parked_id=none
134: [ NMI ] channel cpu=0 ch=1 probe=1 side=A state=Idle parked_id=none
135: [ NMI ] channel cpu=0 ch=1 probe=1 side=B state=Idle parked_id=none
136: [ NMI ] channel cpu=0 ch=2 probe=1 side=A state=Idle parked_id=none
137: [ NMI ] channel cpu=0 ch=2 probe=1 side=B state=Idle parked_id=none
138: [ NMI ] channel cpu=0 ch=3 probe=1 side=all state=Locked parked_id=none owner_cpu=0 owner_thread_id=0 owner_site="SwitchMutex::lock"
139: [ NMI ] channel cpu=0 ch=3 probe=2 side=all state=Locked parked_id=none owner_cpu=0 owner_thread_id=0 owner_site="SwitchMutex::lock"
140: [ NMI ] channel cpu=0 ch=3 probe=3 side=all state=Locked parked_id=none owner_cpu=0 owner_thread_id=0 owner_site="SwitchMutex::lock"
```

The channel lock is stable across all three probes. This disfavours Hc contention-only timing artifact.

Thread location note:

- The explicit `thread_locate` line did not emit because owner thread id `0` collides with the instrumentation's zero/sentinel convention.
- The scheduler snapshot independently locates thread id `0` as `cpu=0 ready[0] name=main state=Ready`.
- The non-null `owner_site="SwitchMutex::lock"` while the lock byte is held makes the owner metadata meaningful despite the id-0 sentinel ambiguity.

Source-path inference:

- `owner_site="SwitchMutex::lock"` is generic because the remaining channel debug helpers still used `self.state.lock()` rather than `lock_at(...)`.
- At this exact phase, the main test thread is in `wait_for_stress_endpoint_blocked(pair=3, send_first=false)`, polling `debug_endpoint_is_recv_blocked(EndpointSide::B)`.
- That helper takes `Channel.state` with preemption enabled.
- The worker IPC path then enters `without_interrupts(...)` before trying to acquire the same `Channel.state` lock.

The resulting shape is:

1. Main thread acquires channel 3 state lock in the debug poll.
2. Main thread is preempted while still holding that lock.
3. Stress receiver pair 3 runs on CPU0, disables interrupts, and tries to acquire channel 3 state lock.
4. It spins in `_mm_pause` with IF clear.
5. Main is Ready on CPU0 but cannot be scheduled because CPU0 interrupts are disabled.

This is a self-deadlock through lock-owner preemption plus an irq-off waiter, not a release-token failure.

---

## 4. Comparison To Phase C

Phase C run 005 showed:

- Last worker: `pair=3 iter=4 stress-receiver pre_block cpu_id=0`.
- CPU0 NMI in `_mm_pause`.
- Channel 3 locked.
- Stress workers accumulated Ready behind AP main threads.

Phase D reproduces the same front edge:

- Last worker: `pair=3 iter=4 stress-receiver pre_block cpu_id=0`.
- CPU0 NMI in `_mm_pause`.
- Channel 3 locked.

Phase D adds the missing lock-owner data:

- Channel 3 owner is CPU0 / thread 0 / `SwitchMutex::lock`.
- Thread 0 is `main`, Ready on CPU0.
- CPU0 current is thread 14 `stress-receiver`, running with IF clear.

The Phase C "channel locked" signal was not an asm release artifact. It was a real lock held by a preempted test orchestration thread.

---

## 5. Mechanism Classification

**Hd: deadlock-shape, AB/BA on channel + scheduler — disfavored.**

No scheduler `inner` lock was held in the NMI probe. CPU0/1/2/3 scheduler locks all reported `inner_locked=false`. There is no evidence of two threads each holding one lock while reaching for the other.

**Hs: stuck holder, single-thread non-progress — partially supported, but refined.**

The channel lock is held stably across three probes. However, the holder is not the currently spinning thread. The holder is thread 0 (`main`) in CPU0's ready queue. The stuck running thread is stress receiver id 14, spinning irq-off while waiting for the holder.

**Hr: release-on-switch ordering bug — disfavored.**

The lock owner is not a worker that passed `release_token_handoff`. Pair 3 receiver never emitted `channel_guard_held`, so it never reached `block_current_into_with_guard` at all. Earlier block paths emitted complete `release_token_handoff` trails and did not leave stale owners behind.

**Hc: contention-only timing artifact — disfavored.**

Channel 3 remained locked across probes 1, 2, and 3 with the same owner metadata. This is not a transient lock snapshot.

**New mechanism: irq-off waiter behind preempted same-CPU lock owner — supported.**

This capture supports a new class: a lock also acquired by irq-off IPC paths is being acquired by a preemptible path. If the preemptible owner is descheduled on the same CPU, an irq-off waiter can spin forever and prevent the owner from running.

This is the runtime instance of the broader rule: every `SwitchMutex` that can be acquired under `without_interrupts` must be acquired under the same irq/preemption discipline by all paths, or the lock can self-deadlock on a single CPU.

---

## 6. Recommendation

Produce a fix-design dispatch targeting irq-safe ownership discipline for `Channel.state` and any other `SwitchMutex` shared between irq-off and preemptible paths.

Minimum fix candidate:

- Convert the channel debug helpers used by SMP tests to acquire `Channel.state` under `without_interrupts`, or remove lock-taking polling from the test orchestration path.
- Audit production `Channel.state` call sites that still use plain `self.state.lock()`:
  - `fail_recv_grant`
  - `debug_endpoint_is_grant_blocked`
  - `debug_endpoint_is_send_blocked`
  - `debug_endpoint_is_recv_blocked`
  - `close_peer`
- For production paths, prefer an explicit irq-safe guard/helper over ad-hoc `without_interrupts` wrappers so the discipline is visible at the type/API boundary.

Secondary instrumentation lesson:

- Owner metadata should not use `0` as a sentinel because thread id `0` is valid (`main`). Use `u64::MAX`, or record an explicit `owner_valid` bit, if this probe is reused.
- Site labels should be added to remaining plain `SwitchMutex::lock()` call sites during future diagnostics; generic `SwitchMutex::lock` was enough here only because the source path was otherwise constrained.

---

## 7. Exit Status

- Backup completed before restore: `Z:\AltimitNeo\2026-05-10-111123`.
- Restored all tracked instrumentation files.
- Removed temporary wrapper: `tools/wedge-investigation/sc3b_phase_d_nmi.py`.
- Exit HEAD: `2d8f65da77b4ea10f3d53226724e9381961c48c0`.
- Exit tracked tree: clean (`git status --short -uno` produced no output).
- Stashes preserved exactly as entry:
  - `stash@{0}: On main: sc-3b-fix-6-1-attempt`
  - `stash@{1}: On main: wedge-investigation-8-nmi-watchdog`
  - `stash@{2}: On main: wedge-investigation-2-instrumentation`
  - `stash@{3}: On main: h1-experiment-instrumentation`
- Post-restore `cargo k`: PASS, 18 warnings.

No commit landed.
