# Dispatch: WEDGE-INV-6 — direct-QEMU baseline without GDB stub, to disambiguate WEDGE-INV-5's confound

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-08
**Type:** Diagnostic. Pure reproduction test. **No state capture, no GDB stub, no kernel code changes, no tracked tooling changes.** Single loop of 200 direct-QEMU boots without `-gdb tcp::PORT`, classified the same way WEDGE-INV-4 and WEDGE-INV-5 classified theirs. Goal: determine whether the direct-QEMU invocation path (independent of the GDB stub) is the masking factor in WEDGE-INV-5's 0/60 pre-flight. Default zero new commits; substrate stays at `05e15c9`.
**Substrate:** Project HEAD `05e15c9`. Working tree should be clean on tracked files apart from the pre-existing `Plans/INDEX.md` doc updates from PM, which you correctly noted in WEDGE-INV-5 §2 and which you should leave alone here too. Both prior instrumentation stashes remain preserved and are NOT used:

- `stash@{0}: On main: wedge-investigation-2-instrumentation`
- `stash@{1}: On main: h1-experiment-instrumentation`

---

## 0. Standing instructions

1. **Substrate is read-only.** No kernel code, no scenario code, no userland code, no `tools/xtask/` modifications, no GDB stub. The investigation tooling from WEDGE-INV-5 (`tools/wedge-investigation/wedge_inv5_capture.py`) is reusable for this dispatch — either invoke it without the `-gdb` flag, or copy/fork it as `wedge_inv6_baseline.py`. Whichever is cleaner. Both stay untracked.
2. **Stashes are not popped or dropped.**
3. **No backup is required for this dispatch** unless something unexpected lands a tracked change (which §6 says shouldn't happen).
4. **`Plans/INDEX.md`'s tracked PM updates are out of scope.** Do not touch that file. They land in a separate small commit when project lead decides; they don't affect this measurement.
5. **Container hygiene as before.** Persistent build container is fine. Remove transients and temp runners on completion.

---

## 1. Goal

Resolve the confound identified in WEDGE-INV-5 §2: the 0/60 pre-flight result was generated via direct QEMU invocation **with** the GDB stub enabled, so it cannot distinguish "the GDB stub is masking" from "the direct-QEMU invocation path itself (vs `cargo neo-test`) is masking."

This dispatch tests "direct QEMU **without** the GDB stub" against the WEDGE-INV-3+4 baseline of 4/150 (2.67%) under `cargo neo-test`. The single comparison isolates the cargo-vs-direct factor.

---

## 2. Background — what we know and don't know after WEDGE-INV-5

| Configuration | Boots | Timeouts | Rate |
|---|---:|---:|---:|
| `cargo neo-test`, no GDB stub (WEDGE-INV-3 + WEDGE-INV-4) | 150 | 4 | 2.67% |
| Direct QEMU + GDB stub (WEDGE-INV-5 pre-flight) | 60 | 0 | 0.0% |
| Direct QEMU, no GDB stub (this dispatch) | 200 | TBD | TBD |
| `cargo neo-test` + GDB stub (not yet measured) | 0 | — | — |

Two factors changed simultaneously between WEDGE-INV-4 and WEDGE-INV-5: the driver path (`cargo neo-test` → direct QEMU) and the GDB stub (off → on). This dispatch holds the GDB stub off and changes only the driver path. The result names which factor caused the masking.

---

## 3. Methodology

### 3.1 Tooling

Reuse `tools/wedge-investigation/wedge_inv5_capture.py` minus the `-gdb tcp::1234` argument. If that requires a flag/edit to the script, edit it. The file is untracked scratch; shape it as needed. Acceptable to either:

- Add a `--no-gdb` flag (or remove the `-gdb` arg unconditionally) and reuse the file, OR
- Copy to `wedge_inv6_baseline.py` and remove the `-gdb` line.

Either way, document the choice in a short methodology note in the findings report.

The build/ISO/QEMU launch sequence is otherwise identical to WEDGE-INV-5 §2:

- Build `neo-init`, `neo-worker`, kernel with `--features neo-test,neo-test-smp-per-cpu-scheduler-smoke`.
- Assemble `boot/altimit-neo.iso` once from built artifacts and `boot/limine.conf`.
- Launch QEMU directly per boot, NEO_SMP=4, same args as WEDGE-INV-5 minus `-gdb tcp::1234`.
- Remove each prior source serial log before booting.
- Classify with the canonical H7-aware classifier from `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md` §5.1.
- Distinguish the `POST_SMOKE_SMP_SCENARIO` signature from `SMOKE_COMPLETION` qualitatively, per `Plans/WEDGE-INV-4-FINDINGS-2026-05-08.md` §4.1. Any timeout that occurs gets a signature.

### 3.2 Sample size

200 boots. At ~3 seconds per boot direct (per WEDGE-INV-5's 2.922s observed average), that's ~10 minutes raw + classification time. Cheap enough to run unconditionally.

Why 200 specifically:

- At p=0.0267 (the WEDGE-INV-3+4 combined rate), `P[0/200] ≈ 0.45%`. Decisive if we observe 0.
- At p=0.04 (the WEDGE-INV-4 M2-only rate), `P[0/200] ≈ 0.03%`. Decisive if we observe 0.
- 100 alone would not be decisive: `P[0/100 | p=0.0267] ≈ 6.7%`, marginal.

### 3.3 What to record per run

Same per-run table as WEDGE-INV-5 §3:

| Run | Classification | Signature | Notes |

Logs to `target/verify/WEDGE-INV-6-run-*.serial.log`; results JSONL to `target/verify/WEDGE-INV-6-results.jsonl`.

---

## 4. Decision matrix

After 200 boots, classify the in-kernel timeout count and produce one of these verdicts. Pick exactly one.

### Case I — Reproduces (≥3 in-kernel timeouts)

The direct-QEMU path on its own does NOT mask the bug. The GDB stub is the masking factor in WEDGE-INV-5. **Implication:** GDB-stub-based observation is not viable for this bug; pivot to a non-GDB observation strategy (QEMU monitor + `pmemsave` on timeout, or kernel-side watchdog). Project lead picks the pivot in the next dispatch.

If the timeouts that did occur match `POST_SMOKE_SMP_SCENARIO` (expected based on WEDGE-INV-4), that's confirmation the bug is still the same. If they're a different signature, that's a new finding worth flagging.

### Case II — Marginal (1–2 in-kernel timeouts)

`P[≤2/200 | p=0.0267] ≈ 6.7%`. Statistically marginal — neither cleanly "direct path is fine" nor "direct path masks." **Implication:** project lead's call. Options would be to (a) extend to 400 boots for a sharper estimate, (b) treat as Case I and pivot, or (c) treat as Case III and run option (1) from the prior status update. Don't pre-decide; report and ping.

### Case III — Doesn't reproduce (0 in-kernel timeouts)

Direct QEMU itself is masking the bug independent of any GDB stub. **Implication:** the cargo-vs-direct difference matters; the next observation strategy must preserve `cargo neo-test`'s per-run rebuild/ISO timing. The natural follow-up is the §3.1.3 path of WEDGE-INV-5 (xtask `NEO_QEMU_GDB_PORT` env-var gate, cargo-driven, GDB stub on top). Project lead approves the follow-up dispatch in response to this finding.

### Case IV — Unexpected signal

A panic, a never-before-seen signature, a serial-log corruption pattern that suggests something other than the wedge — anything outside `PASS / TIMEOUT_IN_KERNEL_SMOKE / TIMEOUT_PRE_KERNEL` should be noted prominently. **Implication:** report and ping; don't continue past a novel signal without direction.

---

## 5. Output artifacts

1. **Per-run logs:** `target/verify/WEDGE-INV-6-run-*.serial.log`.
2. **Results JSONL:** `target/verify/WEDGE-INV-6-results.jsonl`.
3. **Findings report:** `Plans/WEDGE-INV-6-FINDINGS-2026-05-08.md`. Structure parallel to `Plans/WEDGE-INV-5-FINDINGS-2026-05-08.md` (summary, methodology, single measurement table, comparison to prior baselines, case verdict from §4, recommendation, cross-references).

The findings report should include a clean four-row comparison table (see §2 above) showing all four configurations and which are now measured vs. which remain unmeasured.

---

## 6. Commit policy

Zero new commits from this dispatch. The kernel substrate is `05e15c9` going in and `05e15c9` going out. The investigation tooling is scratch.

If during the run something forces a tracked code change to come up (which it shouldn't — this dispatch is read-only), stop and ping before committing anything. There is no permitted-exception path in this dispatch.

---

## 7. Stop conditions

- **A panic appears in any boot's serial log.** Capture, classify, ping. Don't bury under the loop.
- **A novel signature appears** (anything that doesn't fit `PASS / POST_SMOKE_SMP_SCENARIO / SMOKE_COMPLETION (literal) / SMOKE_MID_PHASE / POST_SMOKE_USER_PROCESS / TIMEOUT_PRE_KERNEL / TIMEOUT_BOOT_KERNEL_PRE_SMOKE`). Capture, classify, ping.
- **Build or ISO assembly fails.** This shouldn't happen on a clean tree at `05e15c9`, but if it does, ping; don't paper over.
- **Average per-run runtime materially diverges** from WEDGE-INV-5's 2.922s baseline (e.g., >10s/boot would suggest something has changed in the host environment). Investigate the cause before continuing.

The 200-boot loop runs to completion if none of these fire — there's no stop condition based on timeout count, because 200 is the decisive sample regardless of what we see.

---

## 8. Cross-references

- `Plans/DISPATCH-2026-05-08-wedge-investigation-5-gdb-capture-cerebro.md` — the prior dispatch; §3.1 framed the three GDB-stub-enabling paths.
- `Plans/WEDGE-INV-5-FINDINGS-2026-05-08.md` — the 0/60 pre-flight result and the confound identification that drove this dispatch.
- `Plans/WEDGE-INV-4-FINDINGS-2026-05-08.md` — the 4/100 + 4/150 baseline this measurement is compared against.
- `Plans/WEDGE-INV-3-BASELINE-FINDINGS-2026-05-08.md` — contributes to the 4/150 baseline.
- `Plans/PM-NOTE-2026-05-08-wedge-inv-4-case-iii-verdict.md` — the project-state context for why we're observing at all.
- `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md` §5.1 — canonical classifier.

---

**End of dispatch.**

*One factor changes between WEDGE-INV-5's pre-flight and this dispatch: the GDB stub is off. Whichever way the result lands, it cleanly localizes the masking factor between "GDB stub" and "direct QEMU driver path." That localization picks the next move.*
