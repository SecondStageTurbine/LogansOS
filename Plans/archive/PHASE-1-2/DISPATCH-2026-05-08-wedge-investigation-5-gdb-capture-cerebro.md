# Dispatch: WEDGE-INV-5 — out-of-band observation of `POST_SMOKE_SMP_SCENARIO` via QEMU GDB stub

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-08
**Type:** Investigation. Out-of-band observation of the residual wedge identified in WEDGE-INV-4. **No kernel code changes.** No test scenario changes. The substrate under observation is the kernel exactly as built from HEAD `05e15c9`. Tooling required to run the GDB capture loop lives outside the kernel and outside the test scenario; whether any of it warrants a tracked commit is a §8 decision based on what survives the investigation.
**Substrate:** Project HEAD `05e15c9` (`boot: skip Limine boot menu (timeout: 0) to eliminate H7 pre-kernel stall`). Working tree should be clean on tracked files. The two prior instrumentation stashes remain preserved and are NOT used here:

- `stash@{0}: On main: wedge-investigation-2-instrumentation`
- `stash@{1}: On main: h1-experiment-instrumentation`

---

## 0. Standing instructions

1. **Substrate is read-only for this dispatch.** No kernel code, no scenario code, no userland code, no `tools/xtask/` modifications that change kernel/scenario behavior. The only tracked changes that may land are §8-policy-permitted tooling additions, and even those default to "untracked scratch unless the investigation produces something worth keeping."
2. **Stashes are not popped or dropped.** They remain reference state.
3. **Backups are run after any tracked commit lands.** `Plans\backup-to-synology.ps1`. If no commit lands, no new backup is required for this dispatch.
4. **Findings reports are honest.** A state dump that does not localize the bug is reported as "did not localize the bug," not as "additional investigation needed in direction X" if X isn't supported by the data. Inconclusive is a permitted outcome and §6 names what to do with it.
5. **Container hygiene.** Same Docker-based build flow as WEDGE-INV-4. Persistent build container is fine. Per-run containers are fine. Whatever you used for WEDGE-INV-4 is acceptable here. Remove transient containers and temp runners on completion.

---

## 1. Goal

Localize the `POST_SMOKE_SMP_SCENARIO` wedge from WEDGE-INV-4 by capturing kernel state at the moment the wedge fires, using QEMU's GDB stub to inspect the running guest from outside the kernel itself. The substrate is not modified; observation is purely external.

The end state of this dispatch is a findings report that, for each captured timeout, names what every CPU was doing at the moment of capture, what scheduler/reap/zombie state was present, and across the captures, what the common mechanism is — or, if the captures don't converge, an honest "no convergent mechanism, here is the variance" report that informs the next move.

---

## 2. Background — why out-of-band observation, and what we're targeting

WEDGE-INV-4 produced 4 in-kernel timeouts in 100 uninstrumented `smp_per_cpu_scheduler_smoke` boots at `NEO_SMP=4` (combined with WEDGE-INV-3: 4/150, 2.67%, Wilson 95% CI ~1.04%–6.66%). All four shared a qualitative signature you correctly distinguished from the literal `SMOKE_COMPLETION` label:

- Initial ping/pong/hog smoke completes.
- `NEO_TEST/1 event=BEGIN name=smp_per_cpu_scheduler_smoke phase=dispatch` fires.
- All four `smp-hog-{0..3}` workers spawn.
- A subset (1 in three runs, 2 in one run) of those workers reap.
- The boot then goes silent.

Three of four timeouts wedged after exactly `smp-hog-0` reaped; one wedged after `smp-hog-0` and `smp-hog-1` reaped. The phase boundary is tight.

WEDGE-INV-2 demonstrated that in-kernel instrumentation can mask this bug: instrumented smoke-test/thread/scheduler paths produced 0/30 in-kernel timeouts on a substrate that historically produced ~14% (post-H7-correction). That is the heisenbug risk we're avoiding here. Out-of-band observation via the QEMU GDB stub leaves the kernel binary bit-identical to what produces the bug; only the host-side observer changes.

The expected captured rate at 2.67% over 200 boots is ~5 timeouts (variance: roughly 2–9 across realistic luck). That is the operating sample size assumption for §4 and §6.

---

## 3. Tooling

You will need three pieces:

### 3.1 QEMU GDB stub enabled

QEMU exposes a GDB remote serial protocol stub via `-gdb tcp::PORT` or the shorthand `-s` (which is `-gdb tcp::1234`). The stub does NOT pause the guest unless `-S` is also passed; we want the stub running but not pausing, so the guest boots normally and only is broken-in by the host-side capture script when a timeout is detected.

Inspect `tools/xtask/src/main.rs` (or wherever QEMU is invoked) and identify whether the GDB stub is currently enabled, gated behind an env var, or absent. Three permissible paths in order of preference:

1. **Existing env-var gate.** If there's already a `NEO_QEMU_GDB=1`-style switch, use it. No code change.
2. **Direct QEMU invocation.** Bypass `cargo neo-test` for this investigation; replicate the QEMU command line it generates and add `-gdb tcp::1234` (or a port of your choice). This avoids modifying `xtask` at all. Acceptable if it's straightforward to replicate the command line; the WEDGE-INV-4 logs and the xtask source should be enough to reconstruct it exactly.
3. **Minimal `xtask` change.** If neither (1) nor (2) is reasonable, add a thin `NEO_QEMU_GDB_PORT` env-var gate to `xtask` that appends `-gdb tcp::PORT` when set. This is a tracked-code change. If you take this path, it's a separate small commit with a focused message ("xtask: optional QEMU GDB stub for investigation use"), it gates on env-var so the default behavior is unchanged, and it lands BEFORE the investigation loop runs (so the investigation can use it). See §8.

Pick one. Document the choice in the findings report.

### 3.2 GDB state-dump script

A `gdb` script that, when GDB is invoked with `-x <script>`, connects to the stub, dumps the state defined in §5, and detaches without crashing or perturbing the guest beyond the read access required to inspect memory and registers.

Suggested structure (you adjust to match the symbols actually in the build):

```
target remote localhost:1234
set logging file <state-dump-path>
set logging on
set pagination off

# Per-CPU state. Iterate APIC-ID / per-CPU pointer.
info threads
thread apply all bt 30
thread apply all info registers

# Scheduler state.
print sched::PER_CPU
print -array-indexes -- sched::PER_CPU
# (whatever the actual symbol is — adjust based on `info variables sched::`)

# Zombie state.
print sched::zombie_count
print sched::zombie_list
# (or the equivalent — adjust based on actual symbol layout)

# Per-CPU current thread.
# Walk PER_CPU and dump current_thread for each CPU.

# Heap / spinlock indicators.
# (any spinlock instrumentation, lock-state queries, etc. — what's available)

set logging off
detach
quit
```

Symbol resolution requires the kernel ELF with debug info. After build, that's at `target/.../kernel` — you know the actual path. Pass it to gdb with `gdb <kernel-elf> -x <script>` so symbol lookups work.

The script should be defensive: if a symbol doesn't resolve or a memory read faults, log it and continue. We do not want one missing field to abort the entire dump.

### 3.3 Wrapper that invokes GDB on timeout

A host-side script (PowerShell or Python — whichever you prefer; PowerShell aligns with the existing `Plans\backup-to-synology.ps1` pattern) that, for each iteration of the boot loop:

1. Removes any prior source serial log (per WEDGE-INV-4 hygiene).
2. Launches QEMU with GDB stub enabled (per §3.1).
3. Monitors the test for completion within the existing timeout window.
4. **On PASS:** records PASS, kills the process tree, moves to next iteration.
5. **On TIMEOUT (no completion within window):** before killing QEMU, invokes `gdb <kernel-elf> -batch -x <state-dump-script>` with a per-iteration `<state-dump-path>` of `target/verify/WEDGE-INV-5-state-dump-run-N.txt`. After GDB completes (or after a 30-second GDB-side timeout, whichever comes first), kill QEMU, record TIMEOUT_IN_KERNEL_SMOKE, move to next iteration.
6. **On FAIL/PANIC** (kernel produced a panic line): record it, no GDB attach (the kernel told us what's wrong), kill, move on.

Place this wrapper somewhere it's clear it's investigation scaffolding rather than permanent infrastructure. Suggested: `tools/wedge-investigation/` as a new directory, untracked. If at the end of the investigation any of it is worth keeping, that's a §8 follow-up dispatch, not part of this one.

---

## 4. Methodology

### 4.1 Pre-flight: confirm the bug still reproduces with the GDB stub enabled but unattended

The GDB stub is normally inert when no client connects — but "normally" is doing work in that sentence. Before the full 200-boot loop, run a confirmation pass.

**Pre-flight:** 30 boots of `smp_per_cpu_scheduler_smoke` at `NEO_SMP=4` with the GDB stub enabled and no GDB client attached. Classify by the WEDGE-INV-4 §4.2 scheme plus the `POST_SMOKE_SMP_SCENARIO` distinction.

Expectation: at least 1 `POST_SMOKE_SMP_SCENARIO` timeout. (At 2.67%, P[≥1 in 30] ≈ 56%; P[0 in 30] ≈ 44%, so 0 is plausible by luck alone.)

- **If pre-flight produces ≥1 `POST_SMOKE_SMP_SCENARIO` timeout:** proceed to §4.2.
- **If pre-flight produces 0 in-kernel timeouts:** that's not yet evidence the stub is masking the bug — sample size is too small. Run 30 more (60 total). If 0/60 in-kernel timeouts appear, the stub-enabled substrate is showing a meaningfully lower rate than the stub-disabled substrate (P[0/60] under p=0.0267 ≈ 19.5%, marginal), and we should treat this as a soft heisenbug indicator. **Stop.** Document the 60-boot result, do not run the full 200, and ping for direction. We may need to capture without the stub via a different mechanism (e.g., post-mortem core dump from QEMU's monitor on timeout).

### 4.2 Main loop: 200 boots with GDB capture on timeout

200 boots of `smp_per_cpu_scheduler_smoke` at `NEO_SMP=4`, GDB stub enabled, GDB capture on timeout per §3.3. Record per-run classification in the same format as WEDGE-INV-4 §4.

Expected: ~5 `POST_SMOKE_SMP_SCENARIO` timeouts (variance roughly 2–9). Each produces a state dump at `target/verify/WEDGE-INV-5-state-dump-run-N.txt`.

Pre-flight's timeouts also produce state dumps using the same wrapper; include them in the corpus you analyze (so you may end up with 6–10 dumps total counting pre-flight). The classification tables in the findings report should keep pre-flight (M1) and main loop (M2) separated for honest reporting, but the qualitative cross-dump synthesis in §6 covers all dumps.

### 4.3 If 0 timeouts in 200

Possible at 2.67% but unlikely (P[0/200] ≈ 0.4%). If it happens, stop, report the 0/200, do not invent additional measurements unprompted. Combined with WEDGE-INV-3's 50/50 and WEDGE-INV-4's 96/100, that's 0/200 + 4/150 = 4/350 = 1.14% with the stub on a smaller-fraction-of-runs basis. The honest framing is "we could not reproduce the wedge during this dispatch; the bug rate may be inflated by chance in WEDGE-INV-4 OR the stub may mask it; the next dispatch picks one of those hypotheses to test." Project lead decides the next move.

---

## 5. State dump contents

Each captured dump aims to answer: *what is every CPU doing at this instant, and is there a runnable thread that no CPU is running?*

**Required (must be present in every successful dump):**

1. **Per-CPU registers** for all 4 CPUs (NEO_SMP=4): RIP, RSP, RBP, RFLAGS, CR3, CR2.
2. **Per-CPU backtrace** to at least 20 frames (or until backtrace terminates), with symbol resolution.
3. **The current thread (or task / process / TCB — whatever the abstraction is) on each CPU**: thread ID, name, state, and which run queue it's associated with.
4. **All scheduler run queues**: per-CPU run queue contents, runnable thread count, presence of any zombie/exiting threads.
5. **Zombie state**: `zombie_count`, contents of the zombie list, and any in-flight reap state.
6. **`smp-hog-{0..3}` thread states specifically**: which are alive, which are zombie, which are reaped, which are running, which CPU each is associated with.

**Best-effort (include if reachable; don't fail the dump if not):**

7. **Spinlock states** anywhere we can identify a held lock. The Rust `spin::Mutex` and similar likely have a held-bit field; dump it for any lock you can identify by symbol.
8. **Timer / IPI state**: are there pending IPIs? Is the LAPIC timer fired and unhandled on any CPU?
9. **A small heap memory dump (~4KB)** around the current thread and any zombie thread that's the prime suspect.
10. **The interrupt mask state** (RFLAGS.IF, IDT entries for any "exotic" vectors).

The dump format is plain text (per the suggested GDB script in §3.2). It does NOT need to be machine-parseable. The findings report's job is to synthesize across dumps; the dumps themselves can be human-readable.

---

## 6. Decision matrix

The findings report classifies the cross-dump pattern into one of these. **Pick exactly one** and name the runs that diverge from it (if any) explicitly.

### Case A — Lock spin

One or more CPUs are spinning at the same address inside a known lock acquire path across multiple captures. The held-side and waiting-side are both identifiable from the dumps. **Next move:** ADR or PM-NOTE naming the lock site, then a Cerebro dispatch fixing the offending lock-order or removing the spin loop. Mechanism is identified.

### Case B — Missing wakeup / parked CPU

One or more CPUs are halted (`hlt` in the backtrace, or in the scheduler's idle loop) while a runnable thread exists in some run queue. The bug is a missing IPI / missing wakeup. **Next move:** dispatch to instrument the wakeup paths (this can be in-stash instrumentation since we now know which paths to look at), or audit them for missing notify calls. Mechanism is partially identified — we know it's a wakeup-class bug but not the specific call site.

### Case C — Reap inconsistency

The zombie list, `zombie_count`, and per-CPU current-thread state are mutually inconsistent across captures (e.g., zombie_count == 0 but a zombie is present, or a zombie is present and reap is not making progress). H1 was refuted as written but a related reap-side issue may still exist; this would be that. **Next move:** PM convergence note re-opening H1's broader family, dispatch for targeted reap-side instrumentation OR audit. Mechanism is partially identified.

### Case D — Unexpected fault / silent panic

A CPU is in a fault handler, double-fault path, or panic path that didn't reach the serial console. **Next move:** dispatch to fix serial-output-during-fault so the panic surfaces, then re-run. The current bug is "we lose the panic line"; the wedge is downstream of that.

### Case E — All CPUs idle, work outstanding

No CPU is doing useful work. All run queues empty. No zombies. But `smp-hog-{1..3}` (or whichever didn't reap) are unaccounted-for. They should be running, exiting, or zombie; if they're none of those, the scheduler dropped them. **Next move:** dispatch to instrument the scenario worker spawn path and the scheduler enqueue path. Mechanism is partially identified.

### Case F — No convergent pattern

The captures don't agree. Each dump implicates a different mechanism. **Next move:** the dumps may not be granular enough OR the bug may be more variable than expected. Project lead decides whether to widen the dump (more state, more runs) or pivot to a different observation strategy. Honest "we don't know yet" reporting is required; do not fabricate convergence.

---

## 7. Output artifacts

1. **State dumps:** `target/verify/WEDGE-INV-5-state-dump-run-N.txt` for each timeout captured. Pre-flight and main loop both write here; runs are numbered in a single sequence (so pre-flight runs are 1–30 or 1–60, main loop runs are 31–230 or 61–260, etc., picked to be unambiguous).
2. **Per-run classification table:** plain markdown table in the findings report, one row per boot, `run | classification | signature | dump-file (if applicable)`.
3. **Findings report:** `Plans/WEDGE-INV-5-FINDINGS-2026-05-08.md`. Structure parallel to `Plans/WEDGE-INV-4-FINDINGS-2026-05-08.md` (summary, methodology, M1 pre-flight, M2 main loop, signature breakdown, cross-dump synthesis, case verdict from §6, recommendation, cross-references).
4. **Tooling artifacts:** the GDB script, the wrapper, and any helper code go in `tools/wedge-investigation/` as scratch (untracked) unless §8 says otherwise. If you put them somewhere else, document the location in the findings report so the next investigator can find them.

The findings report must NOT inline the full state dumps — they're large. It synthesizes across them and cross-references the file paths.

---

## 8. Commit policy

**Default expectation:** zero new commits from this dispatch. The kernel substrate is `05e15c9` going in and `05e15c9` going out. The investigation tooling is scratch.

**The one permitted exception** is the §3.1.3 path: if you add a `NEO_QEMU_GDB_PORT` env-var gate to `xtask`, that's a tracked code change because it's substrate-affecting (it changes how QEMU is launched in CI/local-dev when the env var is set, even if the default is unchanged). Commit policy in that case:

- One small commit, focused message: `xtask: optional QEMU GDB stub for investigation use`.
- The change must default to OFF; absent the env var, behavior is identical to today.
- The commit lands BEFORE the §4 loops run.
- Backup runs after it lands.

If §3.1.1 (existing env var) or §3.1.2 (direct QEMU invocation) work, no `xtask` commit is needed and no new tracked changes land. Default to those.

If, during the investigation, you discover the wrapper script or GDB script is genuinely valuable as ongoing infrastructure (rather than scratch for this specific investigation), **do not** commit them as part of this dispatch. Note the observation in the findings report and we'll handle the integration in a follow-up dispatch with proper review.

---

## 9. Stop conditions

Stop and ping for direction in any of these cases:

- **§4.1 pre-flight produces 0/60 in-kernel timeouts.** Stub-masking is plausible; project lead picks the next observation strategy.
- **§4.2 main loop produces 0/200 in-kernel timeouts.** Combined rate drops; project lead decides whether the bug is now plausibly absent or whether the stub is masking.
- **The GDB capture itself hangs or crashes the guest** during attach. Out-of-band observation is supposed to be non-perturbing; if the act of attaching hangs the guest, that's a tooling problem to surface, not power through.
- **A kernel panic appears in any boot's serial log** with content we haven't seen before. Panics are higher-priority signal than timeouts; don't bury them in the loop. Capture, classify, ping.
- **§4.3** — if 0 timeouts in 200, that's a stop condition by §4.3 itself.

In any stop condition, write what you have to the findings report so it survives the chat boundary.

---

## 10. Cross-references

- `Plans/WEDGE-INV-4-FINDINGS-2026-05-08.md` — the previous dispatch's findings; basis for the `POST_SMOKE_SMP_SCENARIO` signature.
- `Plans/PM-NOTE-2026-05-08-patch1-signature-analysis.md` — the four-signature picture.
- `Plans/WEDGE-INV-3-BASELINE-FINDINGS-2026-05-08.md` — the prior 50-boot baseline that contributes to the combined 4/150.
- `Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md` — H1 refutation; relevant to Case C above.
- `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md` §5.1 — canonical classifier; basis for §4 classification.
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — the active commitment whose disposition this investigation continues to inform.
- `Plans/HANDOFF-2026-05-08-wedge-investigation-mid-campaign.md` — the chat-boundary handoff that bridges WEDGE-INV-4 to this dispatch; §3.1 names the case verdict that selected Option B.

---

**End of dispatch.**

*The substrate is read-only. The observer is external. The captures, taken together, either name a mechanism or honestly report that they did not. Either outcome is acceptable; the project lead picks the next move from the §6 case verdict.*
