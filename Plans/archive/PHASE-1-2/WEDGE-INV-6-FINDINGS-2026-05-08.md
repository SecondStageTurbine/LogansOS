# WEDGE-INV-6 findings: direct-QEMU baseline without GDB stub

Date: 2026-05-08

Dispatch: `Plans/DISPATCH-2026-05-08-wedge-investigation-6-direct-qemu-baseline-cerebro.md`

Substrate: `05e15c9 boot: skip Limine boot menu (timeout: 0) to eliminate H7 pre-kernel stall`

## 1. Summary

Ran 200 direct-QEMU boots of `smp_per_cpu_scheduler_smoke` at `NEO_SMP=4` with no GDB stub. Result: 199 PASS, 1 `TIMEOUT_IN_KERNEL_SMOKE`. The single timeout was run 151 and classified as `SMOKE_MID_PHASE`, not WEDGE-INV-4's `POST_SMOKE_SMP_SCENARIO`.

Verdict: Case II, marginal. Direct QEMU without GDB does reproduce an in-kernel timeout, so direct QEMU is not a complete masking factor. But the observed 1/200 rate is far below the WEDGE-INV-3+4 cargo-driven 4/150 rate, and the signature differs from the WEDGE-INV-4 failures, so this does not cleanly validate direct-QEMU as an observation path for the residual `POST_SMOKE_SMP_SCENARIO` bug.

## 2. Methodology

No kernel code, scenario code, userland code, or tracked `tools/xtask` code was modified. The pre-existing tracked `Plans/INDEX.md` PM updates were left untouched and remain out of scope.

Post-flight caveat: after the measurement completed, `HEAD` had advanced to `708223b docs: Plans/INDEX.md - sweep through WEDGE-INV-6 launch`. `git diff --name-only 05e15c9..HEAD` showed only `Plans/INDEX.md`, so the measured kernel/boot/tool/userland substrate remains equivalent to `05e15c9`.

Scratch tooling:

- `tools/wedge-investigation/wedge_inv6_baseline.py` (untracked)

The WEDGE-INV-6 runner is a fork of the WEDGE-INV-5 scratch runner with all GDB-stub and state-capture behavior removed. It:

- Built `neo-init`, `neo-worker`, and the kernel with `--features neo-test,neo-test-smp-per-cpu-scheduler-smoke`.
- Assembled `boot/altimit-neo.iso` once from built artifacts and `boot/limine.conf`.
- Launched QEMU directly per boot with the same `neo-test` QEMU arguments, except without `-gdb tcp::PORT`.
- Removed each prior WEDGE-INV-6 source serial log before booting.
- Classified with the canonical H7-aware classifier from `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md` §5.1.
- Distinguished `POST_SMOKE_SMP_SCENARIO` from the coarser `SMOKE_COMPLETION` label, per `Plans/WEDGE-INV-4-FINDINGS-2026-05-08.md` §4.1.

Artifacts:

- Serial logs: `target/verify/WEDGE-INV-6-run-*.serial.log`
- Results JSONL: `target/verify/WEDGE-INV-6-results.jsonl`

Timing:

- Average across all 200 runs: 3.401 seconds.
- Average across 199 PASS runs: 2.514 seconds.
- Median PASS runtime: 2.208 seconds.
- Max PASS runtime: 8.152 seconds.
- Timeout run 151 lasted 180.013 seconds.

The PASS-runtime average is below WEDGE-INV-5's 2.922s average, not above the dispatch's >10s/boot concern threshold.

## 3. Measurement

| Run | Classification | Signature | Notes |
|---:|---|---|---|
| 1 | PASS | | |
| 2 | PASS | | |
| 3 | PASS | | |
| 4 | PASS | | |
| 5 | PASS | | |
| 6 | PASS | | |
| 7 | PASS | | |
| 8 | PASS | | |
| 9 | PASS | | |
| 10 | PASS | | |
| 11 | PASS | | |
| 12 | PASS | | |
| 13 | PASS | | |
| 14 | PASS | | |
| 15 | PASS | | |
| 16 | PASS | | |
| 17 | PASS | | |
| 18 | PASS | | |
| 19 | PASS | | |
| 20 | PASS | | |
| 21 | PASS | | |
| 22 | PASS | | |
| 23 | PASS | | |
| 24 | PASS | | |
| 25 | PASS | | |
| 26 | PASS | | |
| 27 | PASS | | |
| 28 | PASS | | |
| 29 | PASS | | |
| 30 | PASS | | |
| 31 | PASS | | |
| 32 | PASS | | |
| 33 | PASS | | |
| 34 | PASS | | |
| 35 | PASS | | |
| 36 | PASS | | |
| 37 | PASS | | |
| 38 | PASS | | |
| 39 | PASS | | |
| 40 | PASS | | |
| 41 | PASS | | |
| 42 | PASS | | |
| 43 | PASS | | |
| 44 | PASS | | |
| 45 | PASS | | |
| 46 | PASS | | |
| 47 | PASS | | |
| 48 | PASS | | |
| 49 | PASS | | |
| 50 | PASS | | |
| 51 | PASS | | |
| 52 | PASS | | |
| 53 | PASS | | |
| 54 | PASS | | |
| 55 | PASS | | |
| 56 | PASS | | |
| 57 | PASS | | |
| 58 | PASS | | |
| 59 | PASS | | |
| 60 | PASS | | |
| 61 | PASS | | |
| 62 | PASS | | |
| 63 | PASS | | |
| 64 | PASS | | |
| 65 | PASS | | |
| 66 | PASS | | |
| 67 | PASS | | |
| 68 | PASS | | |
| 69 | PASS | | |
| 70 | PASS | | |
| 71 | PASS | | |
| 72 | PASS | | |
| 73 | PASS | | |
| 74 | PASS | | |
| 75 | PASS | | |
| 76 | PASS | | |
| 77 | PASS | | |
| 78 | PASS | | |
| 79 | PASS | | |
| 80 | PASS | | |
| 81 | PASS | | |
| 82 | PASS | | |
| 83 | PASS | | |
| 84 | PASS | | |
| 85 | PASS | | |
| 86 | PASS | | |
| 87 | PASS | | |
| 88 | PASS | | |
| 89 | PASS | | |
| 90 | PASS | | |
| 91 | PASS | | |
| 92 | PASS | | |
| 93 | PASS | | |
| 94 | PASS | | |
| 95 | PASS | | |
| 96 | PASS | | |
| 97 | PASS | | |
| 98 | PASS | | |
| 99 | PASS | | |
| 100 | PASS | | |
| 101 | PASS | | |
| 102 | PASS | | |
| 103 | PASS | | |
| 104 | PASS | | |
| 105 | PASS | | |
| 106 | PASS | | |
| 107 | PASS | | |
| 108 | PASS | | |
| 109 | PASS | | |
| 110 | PASS | | |
| 111 | PASS | | |
| 112 | PASS | | |
| 113 | PASS | | |
| 114 | PASS | | |
| 115 | PASS | | |
| 116 | PASS | | |
| 117 | PASS | | |
| 118 | PASS | | |
| 119 | PASS | | |
| 120 | PASS | | |
| 121 | PASS | | |
| 122 | PASS | | |
| 123 | PASS | | |
| 124 | PASS | | |
| 125 | PASS | | |
| 126 | PASS | | |
| 127 | PASS | | |
| 128 | PASS | | |
| 129 | PASS | | |
| 130 | PASS | | |
| 131 | PASS | | |
| 132 | PASS | | |
| 133 | PASS | | |
| 134 | PASS | | |
| 135 | PASS | | |
| 136 | PASS | | |
| 137 | PASS | | |
| 138 | PASS | | |
| 139 | PASS | | |
| 140 | PASS | | |
| 141 | PASS | | |
| 142 | PASS | | |
| 143 | PASS | | |
| 144 | PASS | | |
| 145 | PASS | | |
| 146 | PASS | | |
| 147 | PASS | | |
| 148 | PASS | | |
| 149 | PASS | | |
| 150 | PASS | | |
| 151 | TIMEOUT_IN_KERNEL_SMOKE | SMOKE_MID_PHASE | Terminal line: `[ kernel ] sched: reaped 'pong' (id=5)` |
| 152 | PASS | | |
| 153 | PASS | | |
| 154 | PASS | | |
| 155 | PASS | | |
| 156 | PASS | | |
| 157 | PASS | | |
| 158 | PASS | | |
| 159 | PASS | | |
| 160 | PASS | | |
| 161 | PASS | | |
| 162 | PASS | | |
| 163 | PASS | | |
| 164 | PASS | | |
| 165 | PASS | | |
| 166 | PASS | | |
| 167 | PASS | | |
| 168 | PASS | | |
| 169 | PASS | | |
| 170 | PASS | | |
| 171 | PASS | | |
| 172 | PASS | | |
| 173 | PASS | | |
| 174 | PASS | | |
| 175 | PASS | | |
| 176 | PASS | | |
| 177 | PASS | | |
| 178 | PASS | | |
| 179 | PASS | | |
| 180 | PASS | | |
| 181 | PASS | | |
| 182 | PASS | | |
| 183 | PASS | | |
| 184 | PASS | | |
| 185 | PASS | | |
| 186 | PASS | | |
| 187 | PASS | | |
| 188 | PASS | | |
| 189 | PASS | | |
| 190 | PASS | | |
| 191 | PASS | | |
| 192 | PASS | | |
| 193 | PASS | | |
| 194 | PASS | | |
| 195 | PASS | | |
| 196 | PASS | | |
| 197 | PASS | | |
| 198 | PASS | | |
| 199 | PASS | | |
| 200 | PASS | | |

Aggregate:

| Classification | Count | Rate |
|---|---:|---:|
| PASS | 199 | 99.5% |
| TIMEOUT_IN_KERNEL_SMOKE | 1 | 0.5% |
| TIMEOUT_PRE_KERNEL / H7 | 0 | 0.0% |
| TIMEOUT_BOOT_KERNEL_PRE_SMOKE | 0 | 0.0% |
| FAIL / PANIC / OTHER | 0 | 0.0% |

Signature breakdown:

| Signature | Count | Runs |
|---|---:|---|
| SMOKE_MID_PHASE | 1 | 151 |
| POST_SMOKE_SMP_SCENARIO | 0 | |
| SMOKE_COMPLETION | 0 | |
| POST_SMOKE_USER_PROCESS | 0 | |

Run 151 terminal signal lines:

```text
[ kernel ] sched: spawned 'ping' (id=4)
[ kernel ] sched: spawned 'pong' (id=5)
[ kernel ] sched: spawned 'hog' (id=6)
[ ping ] 1
[ pong ] 1
[ pong ] 2
[ ping ] 2
[ pong ] 3
[ ping ] 3
[ pong ] 4
[ ping ] 4
[ pong ] 5
[ ping ] 5
[ kernel ] sched: reaped 'pong' (id=5)
```

This is the known smoke-test partial-reap signature. It matches the `SMOKE_MID_PHASE` family from `Plans/PM-NOTE-2026-05-08-patch1-signature-analysis.md` and H1 run 4, not the WEDGE-INV-4 `POST_SMOKE_SMP_SCENARIO` signature.

## 4. Comparison table

| Configuration | Boots | Timeouts | Rate | Status |
|---|---:|---:|---:|---|
| `cargo neo-test`, no GDB stub (WEDGE-INV-3 + WEDGE-INV-4) | 150 | 4 | 2.67% | measured |
| Direct QEMU + GDB stub (WEDGE-INV-5 pre-flight) | 60 | 0 | 0.0% | measured |
| Direct QEMU, no GDB stub (WEDGE-INV-6) | 200 | 1 | 0.5% | measured |
| `cargo neo-test` + GDB stub | 0 | - | - | unmeasured |

Statistics:

- WEDGE-INV-6 observed rate: 1/200 = 0.5%.
- Wilson 95% CI for 1/200: approximately 0.09% to 2.78%.
- Under the WEDGE-INV-3+4 rate of 2.67%, observing 0/200 would have probability about 0.45%; observing at most 2/200 has probability about 9.6%.
- Under the WEDGE-INV-4 M2-only rate of 4.0%, observing at most 2/200 has probability about 1.25%.

## 5. Verdict and recommendation

Verdict: Case II, marginal. Direct QEMU without GDB produced 1 in-kernel timeout in 200 boots.

Implications:

- Direct QEMU is not a complete masking factor. The bug can reproduce without the `cargo neo-test` driver.
- Direct QEMU appears to reduce or alter the observed failure profile. The rate dropped to 0.5%, and the only reproduced signature was `SMOKE_MID_PHASE`, not WEDGE-INV-4's `POST_SMOKE_SMP_SCENARIO`.
- WEDGE-INV-5's 0/60 cannot be attributed cleanly to the GDB stub alone, because WEDGE-INV-6 shows the direct-QEMU path itself materially changes the rate/signature mix.

Recommendation: project lead should choose the next step. The dispatch's Case II options remain the right menu:

- Extend direct-QEMU/no-GDB to 400 boots if the priority is sharpening the direct-driver rate estimate.
- Treat this as sufficient evidence that direct QEMU is perturbing the bug and move to a cargo-driven GDB-stub gate (`NEO_QEMU_GDB_PORT`) so observation preserves the `cargo neo-test` driver path.
- Treat the reproduction as enough to justify a non-GDB out-of-band strategy, but target `SMOKE_MID_PHASE` rather than assuming `POST_SMOKE_SMP_SCENARIO`.

I would not claim GDB-stub masking as isolated. The data says direct-QEMU itself is a confound.

## 6. Cross-references

- `Plans/DISPATCH-2026-05-08-wedge-investigation-6-direct-qemu-baseline-cerebro.md`
- `Plans/WEDGE-INV-5-FINDINGS-2026-05-08.md`
- `Plans/WEDGE-INV-4-FINDINGS-2026-05-08.md`
- `Plans/WEDGE-INV-3-BASELINE-FINDINGS-2026-05-08.md`
- `Plans/PM-NOTE-2026-05-08-wedge-inv-4-case-iii-verdict.md`
- `Plans/PM-NOTE-2026-05-08-patch1-signature-analysis.md`
- `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md`
- `target/verify/WEDGE-INV-6-results.jsonl`
- `target/verify/WEDGE-INV-6-run-*.serial.log`
- `tools/wedge-investigation/wedge_inv6_baseline.py`
