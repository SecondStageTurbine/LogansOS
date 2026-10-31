# SC-3B Outcome B NMI capture findings

Date: 2026-05-09
Status: Partial NMI capture obtained; no clean all-CPU IP capture

## Substrate

Starting HEAD:

```text
a61e318d93321c9e84732869f5de9da96abe3d5f
```

Tracked substrate before instrumentation:

```text
git status --short -uno
[no output]
```

Existing untracked artifacts were present before this dispatch:

```text
git status --short | Measure-Object
123 entries
```

Stash list at start:

```text
stash@{0}: On main: sc-3b-fix-6-1-attempt
stash@{1}: On main: wedge-investigation-8-nmi-watchdog
stash@{2}: On main: wedge-investigation-2-instrumentation
stash@{3}: On main: h1-experiment-instrumentation
```

Clean-substrate build:

```text
docker compose run --rm neo cargo k
PASS
`altimit-neo-kernel` (bin "kernel") generated 18 warnings
```

Applied NMI stash:

```text
git stash apply stash@{1}
```

Modified tracked files after applying the stash:

```text
kernel/src/arch/x86_64/idt.rs
kernel/src/drivers/console/serial.rs
kernel/src/sched/scheduler.rs
```

Patch adaptation reused from Phase B:

```text
The stash called sched.inner.is_locked(), but current SwitchMutex has no is_locked() method.
Adapted scheduler.rs inside the NMI instrumentation footprint to infer inner_locked from a single try_lock() attempt:

let inner = sched.inner.try_lock();
let locked = inner.is_none();
...
if let Some(inner) = inner { ... }
```

Patched-substrate build:

```text
docker compose run --rm neo cargo k
PASS
`altimit-neo-kernel` (bin "kernel") generated 18 warnings
```

No Fix 6.1 stash content was applied.

## Run summary

Run shape:

```text
50 boots of smp_cross_cpu_rendezvous_stress at NEO_SMP=4.
NMI monitor enabled with NEO_QEMU_MONITOR_PORT.
Host runner sent HMP `cpu N` then `nmi` for CPUs 0..3 after 10s post-BEGIN serial silence.
Artifact directory: target/verify/sc3b-outcome-b-nmi/neo-smp4/
```

Results:

```text
50 runs total
49 PASS
1 NMI_CAPTURE
0 FAIL
0 PANIC
0 silent-timeout-without-NMI
0 NMI_SENT_NO_DUMP
```

Timing:

```text
min elapsed: 7.671s
max elapsed: 108.538s
avg elapsed: 13.149s
```

PASS frame observed on all PASS runs:

```text
NEO_TEST/1 event=PASS name=smp_cross_cpu_rendezvous_stress iterations=1000 send_first=500 recv_first=500 cpus=4
```

Results index:

```text
target/verify/sc3b-outcome-b-nmi/neo-smp4/results.jsonl
```

NMI capture:

```text
run: 026
classification: NMI_CAPTURE
elapsed: 25.037s
trigger: 10.1s post-BEGIN serial silence
monitor_port: 5926
nmi_status: OK
returncode: -9 (host wrapper killed QEMU after capture)
serial_log: target/verify/sc3b-outcome-b-nmi/neo-smp4/run-026.nmi.serial.log
cargo_log: target/verify/sc3b-outcome-b-nmi/neo-smp4/run-026.cargo.log
```

## Per-trace analysis

### Run 026

The observable prefix matches the closure-gate failure class: normal boot, scheduler smoke passed, then scenario BEGIN with no terminal test frame before the observer intervened.

```text
NEO_TEST/1 event=BEGIN name=smp_cross_cpu_rendezvous_stress phase=dispatch
```

CPU0 NMI header:

```text
[ NMI ] cpu=0 rip=0xffffffffa908bb52 rsp=0xffffffffa9808a98 rflags=0x0000000000000002 cs=0x8 ss=0x0 rbp_snapshot=0x0000000000000000
```

Symbol resolution:

```text
runtime kernel virt base: 0xffffffffa9024000
link-time LOAD base:      0xffffffff80000000
runtime RIP:              0xffffffffa908bb52
linked RIP:               0xffffffff80067b52

addr2line:
0xffffffff80067b52
core::core_arch::x86::sse2::_mm_pause
/home/neo/.rustup/toolchains/nightly-2026-04-16-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src/../../stdarch/crates/core_arch/src/x86/sse2.rs:26
```

Backtrace limitation:

```text
[ NMI ] backtrace saved_rsp=0xffffffffa9808a98
[ NMI ] backtrace skipped rbp_out_of_current_stack_window rbp=0x0000000000000000
```

The NMI interrupted CPU0 in `_mm_pause`, but the frame-pointer snapshot was zero, so the stack walk did not identify the Rust caller. The function-context interpretation is therefore "spin-loop/pause site inside the currently-running stress-sender" rather than a precise call-site.

Scheduler snapshot from the CPU0 NMI handler:

```text
[ NMI ] sched cpu=0 ptr=0xffffffffa9800978 zombie_count=0 inner_locked=false
[ NMI ] sched cpu=0 current id=7 name=stress-sender pid=0 state=Running queue=current rsp=0xffffffffa9808d38
[ NMI ] sched cpu=0 ready_len=2 zombies_len=0
[ NMI ] sched cpu=0 ready[0] id=14 name=stress-receiver pid=0 state=Ready rsp=0xffffffffa9824df8
[ NMI ] sched cpu=0 ready[1] id=0 name=main pid=0 state=Ready rsp=0xffff80001ff9c6e8
[ NMI ] sched cpu=1 ptr=0xffffffffa9800cc0 zombie_count=0 inner_locked=false
[ NMI ] sched cpu=1 current id=9 name=stress-sender pid=0 state=Running queue=current rsp=0xffffffffa9810c98
[ NMI ] sched cpu=1 ready_len=2 zombies_len=0
[ NMI ] sched cpu=1 ready[0] id=8 name=stress-receiver pid=0 state=Ready rsp=0xffffffffa980cc38
[ NMI ] sched cpu=1 ready[1] id=3 name=ap-1-main pid=0 state=Ready rsp=0xffff80001ff72d68
[ NMI ] sched cpu=2 ptr=0xffffffffa9800768 zombie_count=0 inner_locked=false
[ NMI ] sched cpu=2 current id=1 name=ap-2-main pid=0 state=Running queue=current rsp=0xffff80001eb1ed68
[ NMI ] sched cpu=2 ready_len=2 zombies_len=0
[ NMI ] sched cpu=2 ready[0] id=11 name=stress-sender pid=0 state=Ready rsp=0xffffffffa9818c98
[ NMI ] sched cpu=2 ready[1] id=10 name=stress-receiver pid=0 state=Ready rsp=0xffffffffa9814c98
[ NMI ] sched cpu=3 ptr=0xffffffffa98008b8 zombie_count=0 inner_locked=false
[ NMI ] sched cpu=3 current id=2 name=ap-3-main pid=0 state=Running queue=current rsp=0xffff80001eb0ed68
[ NMI ] sched cpu=3 ready_len=2 zombies_len=0
[ NMI ] sched cpu=3 ready[0] id=13 name=stress-sender pid=0 state=Ready rsp=0xffffffffa9820c98
[ NMI ] sched cpu=3 ready[1] id=12 name=stress-receiver pid=0 state=Ready rsp=0xffffffffa981cc98
```

Per-CPU interpretation:

```text
CPU0 IP + symbol: 0xffffffffa908bb52 -> _mm_pause.
CPU0 state: stress-sender id=7 Running; stress-receiver id=14 and main id=0 Ready; scheduler inner unlocked.

CPU1 IP + symbol: not captured as an AP NMI frame.
CPU1 scheduler state: stress-sender id=9 Running; stress-receiver id=8 and ap-1-main id=3 Ready; scheduler inner unlocked.

CPU2 IP + symbol: not captured as an AP NMI frame.
CPU2 scheduler state: ap-2-main id=1 Running; stress-sender id=11 and stress-receiver id=10 Ready; scheduler inner unlocked.

CPU3 IP + symbol: not captured as an AP NMI frame.
CPU3 scheduler state: ap-3-main id=2 Running; stress-sender id=13 and stress-receiver id=12 Ready; scheduler inner unlocked.
```

The HMP NMI sequence returned OK, but the only actual NMI handler header was for CPU0. This matches the prior WEDGE-INV-8 limitation: AP IP/RFLAGS frames are not available even though CPU0 can inspect all per-CPU scheduler structures.

Inferred scheduler state:

```text
All scheduler inner locks were available to try_lock().
All zombie_count values were 0.
All zombies_len values were 0.
No scheduler lock-spin or reap/zombie inconsistency is visible.
Ready work exists on every CPU.
CPU2 and CPU3 were running AP bootstrap threads while stress sender/receiver threads were ready.
```

Inferred channel state:

```text
Not dumped by this NMI patch.
No EndpointState or pair/seq data is available from this capture.
```

Classification:

```text
Primary: Hn/H4-family scheduler progress or ready-queue starvation under Q4-driven stress layout.
Secondary: Hh remains possible as a sampling-rate/meta-cause, not directly visible in the trace.
Disfavored: scheduler lock spin, reap inconsistency, zombie leak, panic/BUG, frame leak.
Unresolved: H1-variant lost wakeup at channel level, because channel endpoint state was not captured.
```

This trace supports the "scheduler progress / stress topology" cluster more than the "lock held forever" cluster. It does not prove a kernel-level channel wakeup bug.

## Pattern across captures

Only one NMI-triggered run occurred.

```text
run 026: partial NMI capture, CPU0 NMI frame plus all scheduler snapshots
```

No cross-capture pair or iteration pattern can be established. The AP-IP limitation remains. The absence of scheduler locks and zombies is still useful negative evidence.

## Comparison to closure-gate run 010

Closure-gate retry run 010:

```text
NEO_TEST/1 event=BEGIN name=smp_cross_cpu_rendezvous_stress phase=dispatch
xtask: QEMU timed out after 180s for `smp_cross_cpu_rendezvous_stress`
```

Outcome-B NMI run 026:

```text
NEO_TEST/1 event=BEGIN name=smp_cross_cpu_rendezvous_stress phase=dispatch
[ NMI ] cpu=0 rip=0xffffffffa908bb52 ...
```

The prefix matches exactly at the serial-protocol level: boot succeeds, smoke test passes, scenario emits BEGIN, then no PASS/FAIL before external intervention. The difference is intentional: run 026 was interrupted after 10s of post-BEGIN silence, while closure-gate run 010 was allowed to reach the 180s xtask timeout. Therefore run 026 is a captured precursor to the same signature, not a proven 180s endpoint.

No `BUG:`, `KERNEL PANIC`, frame-leak failure, duplicate terminal frame, rights mismatch, EBADF/EPIPE marker, or FAIL frame was observed in either run.

## Recommendation

The data is sufficient to rule down several hypotheses, but not sufficient to design a precise kernel fix by itself.

Supported:

```text
Hn/H4-family scheduler-progress issue under the stress layout.
Reason: CPU0 interrupted in _mm_pause with stress-sender current; ready stress work exists across CPUs; no scheduler locks are held; no zombies are pending.
```

Disfavored:

```text
Scheduler lock spin: all per-CPU scheduler try_lock() calls succeeded.
Reap/zombie inconsistency: all zombie_count and zombies_len values were 0.
Panic/BUG/frame leak: none observed.
```

Still unresolved:

```text
H1-variant channel lost wakeup: channel endpoint state was not dumped.
Exact stress pair/seq at time of hang: no PROGRESS frames or pair counters were present.
AP execution context: AP NMI IP frames did not print.
```

Recommended next step if PM wants mechanism proof before fix design:

```text
Add second-pass instrumentation with PROGRESS frames and pair/seq/cpu_id tracking in smp_cross_cpu_rendezvous_stress.
Extend the NMI snapshot to dump stress fixture channel endpoint state if it can be done without blocking.
Preserve the same 10s post-BEGIN silence trigger.
```

Recommended next step if PM accepts the current evidence plus prior source audit:

```text
Proceed with a stress-test topology fix in the Hn/H4 direction, then re-run the original closure gate.
Frame it as fixing a verified scheduler-stress pathological layout, not as proving a production IPC wakeup bug.
```

## Exit status

At findings-write time, the NMI patch is still applied. Per dispatch §7, the next steps are:

```text
git restore kernel/src/
git status --short -uno
git stash list
git rev-parse HEAD
docker compose run --rm neo cargo k
powershell -ExecutionPolicy Bypass -File "Plans\\backup-to-synology.ps1"
```

