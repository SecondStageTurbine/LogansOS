# Dispatch: WEDGE-INV-7 — cargo-driven GDB-stub capture of `POST_SMOKE_SMP_SCENARIO`

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-08
**Type:** Investigation. Out-of-band observation under the original `cargo neo-test` driver path with QEMU GDB stub enabled. **One small tracked code change required** — a minimal `NEO_QEMU_GDB_PORT` env-var gate in `xtask` that appends `-gdb tcp::<port>` to the QEMU launch line when set. Default is OFF so all existing test invocations are unaffected. Beyond that one commit, substrate is read-only: no kernel code, no scenario code, no userland code, no further `xtask` changes.
**Substrate:** Project HEAD `708223b` (`docs: Plans/INDEX.md — sweep through WEDGE-INV-6 launch`). The only diff from `05e15c9` is `Plans/INDEX.md`; kernel/boot/tool/userland substrate is functionally equivalent to the substrate WEDGE-INV-3 through WEDGE-INV-6 measured against. Both prior instrumentation stashes remain preserved and are NOT used:

- `stash@{0}: On main: wedge-investigation-2-instrumentation`
- `stash@{1}: On main: h1-experiment-instrumentation`

---

## 0. Standing instructions

1. **Substrate is read-only beyond the one xtask commit.** No kernel code, no scenario code, no userland code, no further `xtask` modifications, no other tracked changes. The xtask change is the §3.1.3 path from WEDGE-INV-5's dispatch — small, gated, off-by-default.
2. **Stashes are not popped or dropped.** Both remain reference state.
3. **Backups are run after the xtask commit lands.** `Plans\backup-to-synology.ps1`. The script preserves stashes via `refs/stashes/N` per the 2026-05-08 stash-preservation refinement; both your stashes will survive the backup verification correctly.
4. **Findings reports are honest.** Inconclusive captures are reported as inconclusive. The §6 case verdict accepts "no convergent mechanism" as a valid honest outcome.
5. **Container hygiene as before.** Persistent build container is fine; remove transients on completion.

---

## 1. Goal

Capture kernel state at the moment `POST_SMOKE_SMP_SCENARIO` fires, by running the same `cargo neo-test` driver path that produced 4/150 (2.67%) reproductions in WEDGE-INV-3+4 but with QEMU's GDB stub enabled so the host-side observer can dump state on timeout.

This dispatch tests the configuration WEDGE-INV-5 didn't reach: cargo-driven driver path AND GDB stub on. WEDGE-INV-5 chose §3.1.2 (direct QEMU + stub) and produced 0/60, conflating two factors. WEDGE-INV-6 isolated direct-QEMU-no-stub at 1/200 with a different signature. This dispatch isolates the GDB stub effect under the original driver, which is the configuration that historically reproduces.

The end state is either (a) a corpus of state dumps that converge on a mechanism per the §6 case matrix, or (b) a clean negative result that names the GDB stub itself as the masking factor under cargo, freeing the next dispatch to pivot to a different observation strategy.

---

## 2. Background — the four-signature picture

| Configuration | Boots | Timeouts | Rate | Signatures observed |
|---|---:|---:|---:|---|
| Cargo, no stub (WEDGE-INV-3+4) | 150 | 4 | 2.67% | `POST_SMOKE_SMP_SCENARIO` ×4 |
| Direct QEMU, stub (WEDGE-INV-5) | 60 | 0 | 0.0% | none |
| Direct QEMU, no stub (WEDGE-INV-6) | 200 | 1 | 0.5% | `SMOKE_MID_PHASE` ×1 |
| Cargo, stub (this dispatch) | 30+200 expected | TBD | TBD | TBD |

The driver-path-vs-direct comparison (rows 1 vs 3) shows host-side cadence not only changes the rate but **changes which latent race fires**. Same kernel, same scenario, same `NEO_SMP=4`. Different timing → different bug surfaces. Combined with PATCH1's pre-substrate-shift signatures (`SMOKE_MID_PHASE`, `SMOKE_COMPLETION` literal, `POST_SMOKE_USER_PROCESS`) all going quiet on the post-ADR-015 substrate, we now have evidence of at least four distinct latent partial-reap-class races, each surfacing under particular timing profiles.

This dispatch's goal is mechanism localization on the ONE we have a strong cargo-driven reproducer for: `POST_SMOKE_SMP_SCENARIO`. Mechanism on this one signature does not address the other three — but it advances project understanding, and future signatures may share root cause.

---

## 3. Tooling

### 3.1 The `xtask` env-var gate (the one tracked code change)

In `tools/xtask/src/main.rs` (or wherever QEMU args are constructed for `cargo neo-test`), add a minimal env-var gate:

```rust
// (location: where qemu_args / cmd.args() / similar is built)
if let Ok(port) = std::env::var("NEO_QEMU_GDB_PORT") {
    if !port.is_empty() {
        qemu_args.push("-gdb".to_string());
        qemu_args.push(format!("tcp::{}", port));
    }
}
```

Constraints:

- Default behavior unchanged when `NEO_QEMU_GDB_PORT` is unset or empty.
- No `-S` (which would pause the guest at boot — we want the stub running but not pausing).
- The change is minimal — one block, one env var, no refactor.
- Single focused commit: `xtask: optional QEMU GDB stub for investigation use`.
- The commit lands BEFORE the §4 loops run.
- Backup runs after the commit lands.

### 3.2 GDB state-dump script

Reuse the GDB script from WEDGE-INV-5's `tools/wedge-investigation/` (you wrote it; the WEDGE-INV-5 findings document it). If it needs adjustments for symbol layout changes since `05e15c9`, adjust it. Otherwise it's drop-in.

### 3.3 Wrapper that invokes GDB on timeout

Reuse the wrapper from WEDGE-INV-5 (or WEDGE-INV-6's no-GDB variant). Adapt for the cargo-driven invocation: instead of launching QEMU directly, launch via `cargo neo-test` (or whichever the standard command is) with `NEO_QEMU_GDB_PORT=1234` set in the environment. The wrapper still:

1. Removes prior source serial logs.
2. Launches the test via the cargo path with GDB-stub env var set.
3. On PASS or kernel-panic-with-output: records, kills, moves on.
4. On TIMEOUT (no completion within window): invokes `gdb <kernel-elf> -batch -x <state-dump-script>` against the running QEMU stub, writes to `target/verify/WEDGE-INV-7-state-dump-run-N.txt`, then kills QEMU.

Investigation tooling stays in `tools/wedge-investigation/` as scratch (untracked).

---

## 4. Methodology

### 4.1 Pre-flight: 30 boots, GDB stub enabled, no GDB attached

Run 30 cargo-driven boots of `smp_per_cpu_scheduler_smoke` at `NEO_SMP=4` with `NEO_QEMU_GDB_PORT=1234` set. Do NOT attach GDB; we're testing whether the stub-enabled-but-unattended substrate still reproduces. Classify per the WEDGE-INV-4 §4.2 + `POST_SMOKE_SMP_SCENARIO` distinction.

Expected: at least 1 `POST_SMOKE_SMP_SCENARIO` timeout. (At p=0.0267, P[≥1 in 30] ≈ 56%; P[0 in 30] ≈ 44%.)

- **If pre-flight produces ≥1 in-kernel timeout:** proceed to §4.2.
- **If pre-flight produces 0 in-kernel timeouts in 30:** extend to 60. If 0/60: GDB stub appears to mask under cargo too. **Stop.** Document the result, do not run the main loop, ping for direction. The project lead's stated requirement is to find the source of the error; the next dispatch will pivot to a different observation strategy (likely a kernel-side scenario watchdog in stash, or a different external observer mechanism). Do not silently push through the stop condition.

### 4.2 Main loop: 200 boots, GDB stub enabled, GDB capture on timeout

200 cargo-driven boots of `smp_per_cpu_scheduler_smoke` at `NEO_SMP=4`, GDB stub enabled, GDB capture on timeout per §3.3.

Expected: ~5 in-kernel timeouts at the historical 2.67% rate.

Pre-flight timeouts also produce state dumps via the same wrapper; include them in the cross-dump synthesis. Classify pre-flight (M1) and main loop (M2) separately in the findings table.

### 4.3 Runtime budget

Cargo-driven boots include build/ISO-assembly time; expect roughly 30s/boot at steady state (depends on cache state). 230 boots at 30s ≈ 2 hours. If runtime is materially different (>3× expected), flag it before continuing.

---

## 5. State dump contents

Reuse WEDGE-INV-5 §5 verbatim. The required-and-best-effort lists do not change. Each captured dump aims to answer: *what is every CPU doing at this instant, and is there a runnable thread that no CPU is running?*

---

## 6. Decision matrix

After the main loop completes, classify the cross-dump pattern into one of these and name the runs that diverge:

### Case A — Lock spin
One or more CPUs spinning at the same address inside a known lock acquire path. **Next move:** PM-NOTE or ADR naming the lock site, then a Cerebro dispatch fixing the offending lock-order or removing the spin loop.

### Case B — Missing wakeup / parked CPU
One or more CPUs halted while a runnable thread exists. **Next move:** dispatch to instrument the wakeup paths or audit them.

### Case C — Reap inconsistency
The zombie list, `zombie_count`, and per-CPU current-thread state are mutually inconsistent. **Next move:** PM convergence note re-opening the H1-broader family, dispatch for targeted reap-side instrumentation or audit.

### Case D — Unexpected fault / silent panic
A CPU is in a fault path that didn't reach the serial console. **Next move:** dispatch to fix serial-during-fault, then re-run.

### Case E — All CPUs idle, work outstanding
No CPU is doing useful work but `smp-hog-{1..3}` (or whichever didn't reap) are unaccounted for. **Next move:** dispatch to instrument scenario worker spawn + scheduler enqueue.

### Case F — No convergent pattern
Captures don't agree across runs. **Next move:** project lead decides whether to widen the dump, increase sample size, or pivot. Do not fabricate convergence.

### Case G — Pre-flight stop condition
0/60 in-kernel timeouts in pre-flight. GDB stub masks under cargo. **Next move:** pivot to a different observation strategy (project lead's call). The investigation continues per the standing directive.

---

## 7. Output artifacts

1. **State dumps:** `target/verify/WEDGE-INV-7-state-dump-run-N.txt` (single sequence across pre-flight and main loop).
2. **Per-run classification table:** in the findings report, format `run | classification | signature | dump-file (if applicable)`.
3. **Findings report:** `Plans/WEDGE-INV-7-FINDINGS-2026-05-08.md`. Structure parallel to WEDGE-INV-5 (summary, methodology, M1 pre-flight, M2 main loop, signature breakdown, cross-dump synthesis, case verdict from §6, recommendation, cross-references).
4. **Tooling artifacts:** GDB script, wrapper, helpers in `tools/wedge-investigation/` as scratch (untracked).

The findings report must NOT inline the full state dumps. Synthesize across them; cross-reference the file paths.

---

## 8. Commit policy

**One permitted commit only:** the `xtask` env-var gate per §3.1.

- Single, focused message: `xtask: optional QEMU GDB stub for investigation use`.
- The change must default to OFF; absent the env var, behavior is identical to today.
- Minimal scope — the env-var-gated argument-append and nothing else.
- Lands BEFORE the §4 loops run.
- Backup runs after it lands. The 2026-05-08 backup script preserves stashes correctly; verify both stashes are still reachable in `git stash list` after the commit + backup.

Beyond this one commit, zero new commits from this dispatch. Investigation tooling (`tools/wedge-investigation/`) stays untracked. If during the run something tempts you toward a second tracked change, stop and ping.

---

## 9. Stop conditions

Stop and ping for direction in any of these cases:

- **§4.1 pre-flight produces 0/60 in-kernel timeouts.** Don't run the main loop. Project lead decides next observation strategy.
- **The GDB capture itself hangs or crashes the guest** during attach. Tooling problem; surface, don't power through.
- **A novel signature appears** (anything outside the canonical buckets and `POST_SMOKE_SMP_SCENARIO`). Capture, classify, ping; don't bury under the loop.
- **A kernel panic appears in any boot's serial log** with content we haven't seen before. Higher-priority signal than timeouts. Capture, classify, ping.
- **Build or ISO assembly fails** post-xtask-commit. Should not happen for a minimal env-var gate, but if it does, ping; don't paper over.
- **Per-boot runtime materially diverges** from expected (>3× the WEDGE-INV-3+4 baseline). Investigate before continuing.

In any stop condition, write what you have to the findings report so it survives the chat boundary.

---

## 10. Cross-references

- `Plans/DISPATCH-2026-05-08-wedge-investigation-5-gdb-capture-cerebro.md` — §3.1 named the three GDB-stub-enabling paths; this dispatch is §3.1.3.
- `Plans/DISPATCH-2026-05-08-wedge-investigation-6-direct-qemu-baseline-cerebro.md` — the disambiguation that informed the four-signature picture in §2 above.
- `Plans/WEDGE-INV-5-FINDINGS-2026-05-08.md` — the 0/60 pre-flight result and the confound that drove the WEDGE-INV-6 disambiguation.
- `Plans/WEDGE-INV-6-FINDINGS-2026-05-08.md` — the 1/200 `SMOKE_MID_PHASE` direct-QEMU result.
- `Plans/WEDGE-INV-4-FINDINGS-2026-05-08.md` — the cargo-driven 4/100 result and `POST_SMOKE_SMP_SCENARIO` signature definition.
- `Plans/PM-NOTE-2026-05-08-wedge-inv-4-case-iii-verdict.md` — the four-signature project-state framing.
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — the active commitment whose discharge this investigation continues to inform. Project lead's directive: find the source; the commitment is not amended around an unfixed mechanism.
- `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md` §5.1 — canonical classifier.

---

**End of dispatch.**

*One small tracked change to xtask, then cargo-driven cargo-driven 30 + 200 boots with GDB capture. Either a mechanism converges from the captures or the stop condition tells us the GDB stub masks under cargo too and the next dispatch pivots. The project lead's standing directive is that we keep investigating until the mechanism is found.*
