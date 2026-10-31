# SC-3B Phase B NMI watchdog findings

Date: 2026-05-09
Status: No NMI capture; no clean counterfactual reproduction

## Substrate

Starting HEAD:

```text
a61e318d93321c9e84732869f5de9da96abe3d5f
```

Dispatch:

```text
Plans/DISPATCH-2026-05-09-sc-3b-phase-b-instrumentation-cerebro.md
```

Stash list after substrate prep:

```text
stash@{0}: On main: sc-3b-fix-6-1-attempt
stash@{1}: On main: wedge-investigation-8-nmi-watchdog
stash@{2}: On main: wedge-investigation-2-instrumentation
stash@{3}: On main: h1-experiment-instrumentation
```

Pre-instrumentation backup:

```text
Z:\AltimitNeo\2026-05-09-191624
```

Backup verification:

```text
Restored stash refs: 4 (source had 4)
Verification: OK (HEAD matches, commit count >= source, all stashes preserved)
```

Clean-substrate build before instrumentation:

```text
docker compose run --rm neo cargo k
PASS
`altimit-neo-kernel` (bin "kernel") generated 18 warnings
```

Applied instrumentation:

```text
git stash apply stash@{1}
```

Applied files:

```text
kernel/src/arch/x86_64/idt.rs
kernel/src/drivers/console/serial.rs
kernel/src/sched/scheduler.rs
```

NMI threshold chosen:

```text
10 seconds of serial silence after NEO_TEST BEGIN for smp_cross_cpu_rendezvous_stress, with no terminal PASS/FAIL frame.
```

Rationale:

```text
The dispatch defaulted to 10 seconds if uncertain. The runner only armed the silence trigger after the scenario BEGIN frame, reducing false positives during boot.
```

Patch adaptation:

```text
The stash called sched.inner.is_locked(), but current SwitchMutex has no is_locked() method.
Adapted scheduler.rs inside the NMI instrumentation footprint to infer inner_locked from a single try_lock() attempt:

let inner = sched.inner.try_lock();
let locked = inner.is_none();
...
if let Some(inner) = inner { ... }
```

No kernel source outside the NMI stash footprint was modified.

Patched-substrate build:

```text
docker compose run --rm neo cargo k
PASS
`altimit-neo-kernel` (bin "kernel") generated 18 warnings
```

## Run Summary

### Instrumented NMI loop

Run shape:

```text
50 boots of smp_cross_cpu_rendezvous_stress at NEO_SMP=4.
NMI monitor enabled with NEO_QEMU_MONITOR_PORT.
Host runner would send NMI sequence to CPUs 0..3 after 10s post-BEGIN serial silence.
Early stop threshold: 3 NMI captures or 50 runs.
```

Artifact directory:

```text
target/verify/sc3b-phase-b-nmi/neo-smp4/
```

Results:

```text
50 runs total
50 PASS
0 NMI-triggered runs
0 NMI lines observed
0 FAIL
0 PANIC
0 timeout
```

Timing:

```text
min elapsed: 7.797s
max elapsed: 67.326s
avg elapsed: 10.068s
```

Representative PASS frame:

```text
NEO_TEST/1 event=PASS name=smp_cross_cpu_rendezvous_stress iterations=1000 send_first=500 recv_first=500 cpus=4
```

Results index:

```text
target/verify/sc3b-phase-b-nmi/neo-smp4/results.jsonl
```

### Clean counterfactual loop

Trigger:

```text
The instrumented loop completed 50/50 with zero NMI fires, so the optional zero-fire counterfactual was run.
```

Counterfactual substrate:

```text
Clean a61e318.
NMI stash restored out of the working tree.
No NEO_QEMU_MONITOR_PORT.
No Fix 6.1 patch applied.
```

Clean-substrate restoration check before counterfactual:

```text
git status --short -uno
[no output]

git rev-parse HEAD
a61e318d93321c9e84732869f5de9da96abe3d5f

docker compose run --rm neo cargo k
PASS
`altimit-neo-kernel` (bin "kernel") generated 18 warnings
```

Artifact directory:

```text
target/verify/sc3b-phase-b-nmi/neo-smp4-counterfactual-clean/
```

Results:

```text
50 runs total
50 PASS
0 FAIL
0 PANIC
0 timeout
```

Timing:

```text
min elapsed: 7.594s
max elapsed: 66.545s
avg elapsed: 10.392s
```

Representative PASS frame:

```text
NEO_TEST/1 event=PASS name=smp_cross_cpu_rendezvous_stress iterations=1000 send_first=500 recv_first=500 cpus=4
```

Results index:

```text
target/verify/sc3b-phase-b-nmi/neo-smp4-counterfactual-clean/results.jsonl
```

## Per-trace Analysis

No NMI-captured traces were produced.

Therefore no per-CPU RIP/RSP/RFLAGS, scheduler snapshot, run-queue state, or backtrace data exists for Hn / H1-variant / H4 discrimination in this pass.

No state dump files were generated:

```text
target/verify/sc3b-phase-b-nmi/neo-smp4/run-*.nmi.serial.log
```

## Pattern Across Traces

No traces exist.

Pattern across successful instrumented runs:

```text
All 50 instrumented runs reached PASS.
All PASS frames reported iterations=1000 send_first=500 recv_first=500 cpus=4.
No watchdog trigger fired.
No NMI handler output appeared in serial.
```

Pattern across clean counterfactual runs:

```text
All 50 clean runs reached PASS.
All PASS frames reported iterations=1000 send_first=500 recv_first=500 cpus=4.
No silent-timeout signature reproduced.
```

## Counterfactual Data

The counterfactual did not confirm that the bug still reproduces on clean a61e318 during this observation window.

Observed combined data after Fix 6.1 failure:

```text
NMI-instrumented a61e318: 50/50 PASS, 0 timeouts.
Clean a61e318 counterfactual: 50/50 PASS, 0 timeouts.
```

Interpretation:

```text
This does not prove the SC-3B stress hang is fixed.
It does mean Phase B failed to capture the hang and failed to reproduce it in the clean counterfactual sample.
The zero-capture result cannot discriminate Hn, H1-variant, or H4.
```

Statistical note:

```text
At the earlier observed 1/36 failure point estimate, a 0/50 clean run is plausible: (35/36)^50 ~= 24.5%.
A 0/100 combined observation, if treated as the same rate and no instrumentation effect, is lower probability: (35/36)^100 ~= 6.0%.
The samples are not cleanly pooled because the first 50 used the NMI stash and monitor gate while the second 50 did not.
```

## Recommendation

Supported:

```text
The NMI stash plus monitor gate did not produce false-positive NMI triggers under normal PASS runs.
The 10s post-BEGIN silence threshold did not fire during successful stress runs.
The applied NMI instrumentation built at the 18-warning baseline after one local API-drift adaptation.
```

Refuted by this pass:

```text
No hypothesis is refuted. No stuck state was captured.
```

Not supported:

```text
No evidence supports Hn, H1-variant, or H4 from this pass because no NMI dump occurred.
The clean counterfactual did not reproduce the silent timeout, so this pass also cannot prove the NMI stash masked the bug.
```

Recommended next step:

```text
Do not design a kernel-level fix from this data.
Either run a larger clean baseline to re-establish the current failure rate, or move to lower-perturbation progress/cpu_id tracking only after the failure rate is re-observed.
```

Specific next-dispatch options:

```text
Option 1: Long clean baseline, no instrumentation, same host path, 200 runs at NEO_SMP=4. Stop on first silent timeout and preserve serial log.
Option 2: Re-apply NMI stash and run a larger sample only if the clean baseline first confirms the hang is still active.
Option 3: Add minimal PROGRESS frames to the stress orchestrator, but only after confirming the clean hang still reproduces; otherwise it risks chasing a currently non-reproducible failure.
```

## Exit State

Final tracked working tree:

```text
Clean on tracked files after restoring the NMI stash.
```

Final HEAD:

```text
a61e318d93321c9e84732869f5de9da96abe3d5f
```

Final stash list:

```text
stash@{0}: On main: sc-3b-fix-6-1-attempt
stash@{1}: On main: wedge-investigation-8-nmi-watchdog
stash@{2}: On main: wedge-investigation-2-instrumentation
stash@{3}: On main: h1-experiment-instrumentation
```

No commits landed.
