# DISPATCH — Sub-commit 3 IrqSafeMutex finalize (Jarvis)

**Date filed:** 2026-05-11 (early morning)
**From:** PM
**To:** Jarvis (Codex, kernel coding lane per `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.1)
**Ferry:** PL ferries to Jarvis session. Working-tree state intact from prior dispatch stop; this dispatch picks up from that state.
**Status:** Continuation of `Plans/DISPATCH-2026-05-10-kernel-rename-and-adr-implementation-jarvis.md` sub-commit 3 work. Sub-commits 1 and 2 already landed on origin/main (commits `053a5bc` and `de8965c`, rebased over parallel README updates).

---

## 1. Context

Prior dispatch stopped at sub-commit 3 gate per `Plans/JARVIS-FAILURE-2026-05-10-kernel-bundle-sub-commit-3.md`. PM reviewed the failure and resolved the threshold issue per `Plans/PM-NOTE-2026-05-11-adr-019-bench-threshold-environment-clarification.md`.

**Resolution summary:** The 50 ns architectural threshold in ADR-019 Q4 is correct for native hardware but unmeasurable under QEMU TCG debug build, where IF-toggle alone costs ~1711 ns (derived from the IrqSafeMutex - SwitchMutex overhead delta in Jarvis's data). The bench reframes as a **regression detector** at a QEMU-debug-realistic ceiling of **6000 ns**. ADR-019 text stays unchanged (native target preserved as design intent); native-hardware absolute-performance characterization deferred to Phase 2+ bare-metal CI. See the PM-NOTE for full reasoning.

**The two implementation optimizations Jarvis already applied** (constant `lock_site!()`, forced inlining on lock/owner hot path) are real improvements and stay. They reduced overhead from 4377 ns → 3085 ns; that's the floor under QEMU debug, and the 6000 ns ceiling gives ~95% headroom for regression detection.

## 2. Mandate

Apply the threshold revision, re-run the verification gates, commit sub-commit 3, push to origin/main.

The working tree currently contains:
- New file: `kernel/src/sync/irqsafe_mutex.rs` (94 LOC, IrqSafeMutex implementation)
- Modified: `kernel/Cargo.toml`, `kernel/src/arch/mod.rs`, `kernel/src/arch/x86_64/mod.rs`, `kernel/src/sync/lock_owner.rs`, `kernel/src/sync/mod.rs`, `kernel/src/sync/switch_mutex.rs`, `kernel/src/test_harness.rs`, `tools/xtask/src/main.rs`

This is the partial sub-commit 3 work from the prior dispatch. Do not re-implement; pick up from this state.

## 3. Scope

### 3.1 Threshold revision (the only behavioral change in this dispatch)

Locate the `irqsafe_mutex_bench` threshold check in the bench scenario code (likely in `kernel/src/test_harness.rs` or a related scenario file). Find the comparison that produces `event=FAIL name=irqsafe_mutex_bench reason=overhead_threshold overhead_ns=N threshold_ns=50`.

Change the threshold from 50 to **6000**.

Update the surrounding comment (or add one if none exists) to document the reasoning:

```rust
// IrqSafeMutex bench: regression detector under QEMU TCG debug build.
//
// The architectural target per ADR-019 Q4 is 50 ns acquire/release
// overhead vs spin::Mutex on native hardware. That target is unmeasurable
// under QEMU TCG debug build, where IF-toggle alone (cli/sti/pushfq/popfq)
// costs ~1711 ns due to software emulation of privileged instructions.
//
// This bench reframes as a regression detector with a 6000 ns ceiling:
// observed floor with implementation optimizations applied is ~3085 ns;
// 6000 ns gives ~95% headroom for run-to-run variance while catching any
// catastrophic regression (10x slowdown).
//
// Native-hardware absolute-performance characterization is deferred to
// Phase 2+ when bare-metal CI exists. ADR-019 architectural target unchanged.
//
// See Plans/PM-NOTE-2026-05-11-adr-019-bench-threshold-environment-clarification.md
const IRQSAFE_MUTEX_BENCH_OVERHEAD_CEILING_NS: u64 = 6000;
```

Exact wording is at Jarvis's discretion; the substantive content (architectural target preserved, QEMU-debug regression-detector framing, native-hardware deferred, cite the PM-NOTE) should all be present.

### 3.2 No other behavioral changes

- IrqSafeMutex implementation: **unchanged**. Functional test `irqsafe_mutex_if_restore` already passes; no API changes.
- The two optimizations already applied (constant `lock_site!()`, forced inlining): **kept**.
- All other modified files (`kernel/Cargo.toml`, `kernel/src/arch/mod.rs`, `kernel/src/arch/x86_64/mod.rs`, `kernel/src/sync/*.rs`, `tools/xtask/src/main.rs`): **kept as-is**.

If during code review Jarvis notices anything that warrants a minor cleanup (e.g., dead code, unused imports, comment typos), small cleanups are welcome but not required. Do not introduce new behavior beyond the threshold revision.

## 4. Verification gates

All gates must PASS before commit:

1. `cargo k` — kernel build with default features.
2. `cargo k --no-default-features` — kernel build with minimal features.
3. `cargo neo-test irqsafe_mutex_if_restore` — functional scenario (already passing in prior dispatch; should continue to pass).
4. `cargo neo-test irqsafe_mutex_bench` — bench scenario; should now PASS with the 6000 ns ceiling. The bench should still emit `spin_mean_ns`, `irqsafe_mean_ns`, `switch_mean_ns`, `overhead_ns` for human review on pass (the regression detector is informational on pass, not just gate-on-fail).
5. `cargo neo-test all` — full scenario suite (must not have regressed any previously-passing scenarios).

If any gate fails, STOP and file a failure report per the standard pattern. PM expects no failures here; the only change is a threshold constant.

## 5. Commit

Single commit. Suggested title:

```text
kernel/sync: introduce IrqSafeMutex (ADR-019 sub-phase A)
```

Suggested body (Jarvis may revise wording; substance should match):

```text
Introduces IrqSafeMutex as a sibling type to SwitchMutex for lock sites
that require IRQ-off semantics. Implementation per ADR-019 sub-phase A:

- Disable local interrupts before acquiring the inner spin::Mutex.
- Restore the prior IF state on guard drop (ManuallyDrop ensures inner
  guard is dropped before IF is restored).
- ADR-020 LockOwner integrated.

Performance characterization (QEMU TCG debug build):
- spin_mean_ns       = 463  (baseline spin::Mutex)
- switch_mean_ns     = 1837 (SwitchMutex; LockOwner bookkeeping only)
- irqsafe_mean_ns    = 3548 (IrqSafeMutex; LockOwner + IF-toggle)
- IF-toggle alone    = ~1711 ns under QEMU TCG (irqsafe - switch delta)

The bench threshold is set as a 6000 ns regression-detector ceiling
under QEMU TCG debug build. The 50 ns architectural target per ADR-019
Q4 is preserved as the native-hardware design intent; absolute-
performance characterization is deferred to Phase 2+ bare-metal CI.

Two implementation optimizations applied to reduce overhead from
initial 4377 ns to final 3085 ns:
- Constant lock_site!() values eliminate per-acquire runtime
  Location::caller() file-path hashing in the ADR-020 path.
- Forced inlining on the lock/owner hot path.

See:
- Plans/PM-NOTE-2026-05-11-adr-019-bench-threshold-environment-clarification.md
- Plans/PM-NOTE-2026-05-11-jarvis-first-dispatch-calibration.md
- Plans/JARVIS-FAILURE-2026-05-10-kernel-bundle-sub-commit-3.md (prior stop)
```

## 6. Push

After commit, push to origin/main. Branch is currently `jarvis/kernel-rename-adr-2026-05-10`; HEAD is already on top of the rebased sub-commits 1+2 that landed earlier. Push command:

```bash
git push origin HEAD:main
```

Should be a clean fast-forward — no other commits have landed on origin/main since the rebase (verified by PM immediately before filing this dispatch). If a rejection occurs, STOP and surface to PL.

## 7. Verification report

File `Plans/IRQSAFE-MUTEX-SUB-COMMIT-3-VERIFICATION-2026-05-11.md` documenting:

- All five gates' PASS results with verbatim output where representative.
- Final bench numbers (mean/min/max ns for spin, irqsafe, switch; overhead_ns; threshold confirmed 6000).
- Commit SHA after push.
- Confirmation that the threshold revision matches the PM-NOTE's reasoning.

Format/style: match Jarvis's prior verification reports (`Plans/KERNEL-RENAME-SUB-COMMIT-1-VERIFICATION-2026-05-10.md` and `Plans/ADR-020-SUB-COMMIT-2-VERIFICATION-2026-05-10.md`); those are the voice anchors.

## 8. Closure protocol

After commit + push + verification report:

1. Jarvis reports back to PL with: (a) commit SHA, (b) bench numbers, (c) verification report filename, (d) any incidental observations during the work.
2. PL ferries the report back to PM.
3. PM consumes the report and ratifies the ADR-019 sub-phase A close. No follow-up PM-NOTE required unless something surfaced that wasn't anticipated.
4. ADR-019 sub-phase A is closed. ADR-019 sub-phase B (call-site migration to IrqSafeMutex where IRQ-off semantics required) becomes a future dispatch candidate.

## 9. Cross-references

**Prior dispatch and failure:**
- `Plans/DISPATCH-2026-05-10-kernel-rename-and-adr-implementation-jarvis.md` — original three-sub-commit bundle.
- `Plans/JARVIS-FAILURE-2026-05-10-kernel-bundle-sub-commit-3.md` — sub-commit 3 stop at gate.

**PM resolution:**
- `Plans/PM-NOTE-2026-05-11-adr-019-bench-threshold-environment-clarification.md` — bench threshold operational resolution.
- `Plans/PM-NOTE-2026-05-11-jarvis-first-dispatch-calibration.md` — Jarvis first-dispatch calibration record.

**Architectural anchors:**
- `docs/adr/ADR-019-switchmutex-irq-off-by-construction.md` — unchanged; native-hardware 50 ns target preserved as design intent.
- `docs/adr/ADR-020-owner-metadata-as-production-diagnostic.md` — LockOwner cost baseline included in sub-commit 3 measurements.

**Voice anchors:**
- `Plans/KERNEL-RENAME-SUB-COMMIT-1-VERIFICATION-2026-05-10.md` — Jarvis verification report format.
- `Plans/ADR-020-SUB-COMMIT-2-VERIFICATION-2026-05-10.md` — Jarvis verification report format.

**Already landed on origin/main:**
- `053a5bc kernel: rename AltimitNeo → NeoPlutarch; A3 boot identity` (sub-commit 1, rebased).
- `de8965c kernel/sync: add LockOwner field to SwitchMutex (ADR-020 foundation)` (sub-commit 2, rebased).

---

**End of dispatch.**

*Sub-commit 3 finalize: apply 50 ns → 6000 ns threshold revision per PM-NOTE; re-run gates; commit; push. Implementation unchanged; bench reframes as regression detector. ADR-019 architectural target preserved. Verification report follows.*
