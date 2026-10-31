<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# PRE-REGISTER: LogansOS vs Linux+llama.cpp CPU benchmark

**Date:** 2026-06-17
**Branch:** `codex/phase-8-benchmark-vs-llamacpp-jarvis`
**Dispatch:** `Plans/DISPATCH-2026-06-17-benchmark-vs-llamacpp-jarvis.md`
**Status:** RATIFIED 2026-06-17 by user/PL instruction. Methodology, §6
thresholds, §7 dispositions, and §8 KC-1 trigger are frozen. ISC-2 harness work
may proceed; ISC-3 measurement must still record the §2 rig fields before any
number.

## 0. Integrity Boundary

This file freezes the benchmark question before any result exists. No latency,
throughput, footprint, or fairness numbers have been collected on this branch.

After PM/PL ratification:

- Pre-registered metrics and win/loss thresholds may not be removed or relaxed.
- Additional exploratory metrics may be added only if they are labelled
  exploratory and cannot replace a pre-registered metric.
- A run that loses is reported as prominently as a run that wins.
- Any rig or sample-count change after ratification requires a dated amendment
  before measurement starts; changes after numbers exist are invalid for the
  headline claims.

## 1. Scope

The fair, answerable benchmark is:

```text
LogansOS CPU inference vs Linux + llama.cpp CPU inference
same Qwen2.5-0.5B-Q4_K_M model, same quantization, same host, same CPU budget
```

GPU/vLLM is out of scope for this run because LogansOS does not run the 8.5
forward pass on an accelerator today. A CPU LogansOS vs GPU vLLM comparison is
not an allowed headline or ledger disposition.

## 2. Ratification Fields

PM/PL ratified the gaming PC Ryzen host for the near-term
`CONTROLLED_SAME_HOST_VM` run: LogansOS-in-QEMU vs Linux+llama.cpp in matched
same-host virtualization, pinned cores, controlled load, and PL idle during
runs. The bare-metal dual-boot run remains the gold-standard
`valid-for-ledger` run when a dedicated box is free.

These fields must be filled in the run manifest before any benchmark number:

| Field | Ratified value |
| --- | --- |
| Rig mode | Near-term: `CONTROLLED_SAME_HOST_VM`, LogansOS-in-QEMU vs Linux+llama.cpp in matched same-host virtualization. Gold standard: `BARE_METAL_DUAL_BOOT`, valid-for-ledger when a dedicated box is free. |
| Hardware ID | Gaming PC Ryzen host for ISC-3 controlled-same-host. Before measurement, record exact CPU model, core/thread count, RAM, firmware settings, storage, and thermal/power mode. |
| CPU pinning | Pin both systems to the same physical cores; record exact pin mask and affinity commands before measurement. PL must be idle and unrelated workloads disabled during runs. |
| LogansOS commit | Branch commit containing the ratified harness. |
| llama.cpp source | Local source tree, commit SHA recorded before build. Preferred pin: `5f04dc7ac3f40271f60105574be0f617887195bb` unless PM/PL ratify a different local commit before measurement. |
| Model artifact | Qwen2.5-0.5B-Q4_K_M GGUF. Path and SHA-256 recorded before measurement. |
| Context length | 4096 unless PM/PL ratify a lower equal context for both systems due to memory constraints. |
| Threads | Primary single-stream run uses one CPU worker thread/core on both systems. Multi-tenant runs use one worker thread/core per tenant unless PM/PL ratify a different equal allocation. |

If the near-term controlled-same-host rig cannot satisfy same-host, same-model,
same-quant, same-thread-count, and controlled-load requirements, the run is
reported as "not valid for ledger promotion/demotion." Even when it satisfies
those controls, it is an ISC-3 scoping run unless PM/PL explicitly promote it;
the bare-metal dual-boot run remains the ledger-grade hardware run.

## 3. Workloads

### 3.1 Single-Stream Decode

- Prompt: raw token `[9707]` (`"Hello"` for Qwen2.5, no BOS).
- Generation: greedy/top-1, `temperature = 0`, max new tokens = 128.
- Warmup: one full 128-token generation discarded per binary/OS after model
  load.
- Samples: minimum 10 measured generations per system, producing at least 1280
  per-token decode samples per system. If runtime makes this infeasible, PM/PL
  must ratify a smaller sample count before measurement starts; the p99 is then
  marked low-confidence.
- Timing window: decode only. Model load, tokenizer setup, and process/boot
  startup are excluded from p50/p99 and tokens/sec. Time-to-first-token may be
  recorded as exploratory.

### 3.2 Governance-Overhead Delta

Run the same single-stream workload in two LogansOS modes:

- `governance_backstop_only`: minimum correctness/governance required to execute
  the model path.
- `governance_daemon_on`: residency/admission/accounting/daemon path enabled as
  the shipping governed path for this branch.

The latency oracle is extended from ADR-052 to record the same run metadata and
to report per-token orchestration overhead alongside the existing hot-path
cycle probes.

### 3.3 Multi-Tenant Contention

Run concurrent tenants with the same prompt/generation settings:

- Primary tenant counts: N = 2 and N = 4.
- Each tenant receives one equal CPU worker budget.
- Samples: minimum 5 measured generations per tenant per N after one discarded
  warmup generation.
- Metrics: per-tenant p50/p99 token latency, per-tenant tokens/sec, aggregate
  tokens/sec, Jain fairness index over per-tenant tokens/sec, and max/min tenant
  throughput ratio.

### 3.4 Noisy Neighbor

Run a victim tenant plus a noisy co-tenant:

- Victim: same prompt/generation as single-stream.
- Noisy neighbor: same model and prompt, continuously generating in a loop with
  the same CPU worker budget.
- Samples: minimum 10 victim generations.
- Metric: victim p99 token latency degradation and tokens/sec degradation
  relative to the victim-alone single-stream baseline.

## 4. Baseline Configuration

The Linux baseline is `llama.cpp` CPU, not GPU, not vLLM:

```text
model: Qwen2.5-0.5B-Q4_K_M GGUF
backend: CPU only
threads: same as the LogansOS scenario
batch/context: same context limit as LogansOS
sampling: greedy / temperature 0
prompt: raw token [9707] or exactly equivalent tokenizer path
```

Baseline command lines are emitted into the raw log before execution. If
`llama.cpp` cannot accept raw token input in the selected tool mode, the harness
must record the tokenizer output and assert that `"Hello"` maps to `[9707]`
before timing starts.

## 5. Metrics

### 5.1 Headline Single-Stream Metrics

- p50 token latency, decode-only.
- p99 token latency, decode-only.
- tokens/sec, decode-only.
- Whole-serving footprint:
  - absolute resident footprint of the serving environment;
  - incremental resident footprint over idle baseline on the same rig.
- Governance overhead:
  - daemon-on vs backstop-only p50/p99 delta;
  - daemon-on vs backstop-only tokens/sec delta;
  - ADR-052 latency-oracle per-token orchestration cost.

### 5.2 Headline Multi-Tenant Metrics

- per-tenant p99 token latency at N = 2 and N = 4;
- Jain fairness index over per-tenant tokens/sec;
- max/min tenant throughput ratio;
- aggregate tokens/sec;
- victim p99 and tokens/sec degradation under noisy-neighbor load.

### 5.3 Variance Reporting

For each metric, report:

- raw samples retained as CSV or JSON;
- median/p50, p95 where available, p99, mean, min, max;
- bootstrap 95% confidence interval for p50, p99, and tokens/sec;
- hardware/thermal/load notes for every run.

## 6. Win/Loss Criteria

All criteria use the same rule:

- A win/loss requires at least a 5% relative difference and non-overlapping
  bootstrap 95% confidence intervals unless a stricter threshold is listed.
- Within +/-5%, or overlapping confidence intervals, is "inconclusive" and the
  ledger row stays Unproven.
- Any invalid apples-to-apples condition also yields "not valid for ledger
  movement."

### 6.1 Single-Stream Latency

LogansOS wins this metric if its p99 token latency is at least 5% lower than
Linux+llama.cpp CPU. LogansOS loses if its p99 is at least 5% higher. Otherwise
the result is inconclusive.

p50 is reported and may explain mechanisms, but p99 is the ledger-bearing
latency criterion.

### 6.2 Throughput

LogansOS wins this metric if decode tokens/sec is at least 5% higher than
Linux+llama.cpp CPU. LogansOS loses if tokens/sec is at least 5% lower.
Otherwise the result is inconclusive.

### 6.3 Footprint

LogansOS wins this metric if incremental resident footprint is at least 10%
lower than Linux+llama.cpp CPU and the absolute footprint is not higher by more
than 10%. LogansOS loses if incremental resident footprint is at least 10%
higher. Otherwise the result is inconclusive.

Absolute whole-OS footprint is reported in the headline table, but incremental
footprint is the primary criterion because the two systems have different idle
OS baselines.

### 6.4 Governance Overhead

The "governance overhead is bounded/affordable" row is promoted only if:

- daemon-on p99 token latency is no more than 5% worse than backstop-only;
- daemon-on tokens/sec is no more than 5% worse than backstop-only;
- latency-oracle orchestration cost remains shape-stable and below 5% of
  measured per-token compute time.

It loses if daemon-on p99 or tokens/sec is worse by more than 10%, or if the
oracle shows size-dependent or tenant-count-superlinear hot-path behavior.
Between those bands, the row stays Unproven.

### 6.5 Multi-Tenant Fairness

LogansOS wins the fairness metric if, at N = 2 and N = 4:

- Jain fairness index is at least 0.05 higher than Linux+llama.cpp CPU, or
  LogansOS is >= 0.98 while Linux is < 0.95; and
- LogansOS max/min tenant throughput ratio is lower by at least 15%; and
- aggregate tokens/sec is not worse by more than 10%.

LogansOS loses if the inverse holds, or if aggregate throughput is worse by more
than 20% without a fairness win. Otherwise the result is inconclusive.

### 6.6 Noisy-Neighbor Resistance

LogansOS wins noisy-neighbor resistance if victim p99 degradation is at least
15% lower than Linux+llama.cpp CPU and victim tokens/sec degradation is not
worse. LogansOS loses if victim p99 degradation is at least 15% higher.
Otherwise the result is inconclusive.

## 7. Strategic Honesty Ledger Dispositions

The current ledger rows in `docs/STRATEGIC-HONESTY-LEDGER.md` §3 mention some
GPU/vLLM claims. This CPU benchmark cannot honestly prove or disprove a GPU
claim. The FINDINGS must therefore separate "CPU fair baseline result" from
"GPU/vLLM deferred."

| Ledger row | This benchmark can do | Disposition rule |
| --- | --- | --- |
| Lower p99 token latency vs Linux+vLLM | Cannot prove the vLLM/GPU row. Can evaluate the fair CPU subclaim vs Linux+llama.cpp. | If LogansOS wins §6.1, propose adding a CPU-specific Proven row while keeping vLLM Unproven/deferred. If it loses §6.1, report the CPU latency loss prominently and do not promote the vLLM row. |
| Higher GPU utilization | Not measured. | Remains Unproven/deferred. CPU utilization may be reported as exploratory but cannot move this row. |
| Better fairness under contention | Measured under CPU multi-tenant load. | Promote only if §6.5 wins at both N values. Demote/report loss if §6.5 loses. Otherwise stays Unproven. |
| Better noisy-neighbor resistance | Measured under CPU noisy-neighbor load. | Promote only if §6.6 wins. Demote/report loss if §6.6 loses. Otherwise stays Unproven. |
| Smaller serving footprint | Measured against Linux+llama.cpp CPU. | Promote CPU-specific footprint claim only if §6.3 wins. If §6.3 loses, report loss. Original K8s+Triton row remains Unproven unless PM/PL ratify that baseline before measurement. |
| Governance overhead is bounded/affordable | Measured by daemon-on vs backstop-only plus ADR-052 oracle. | Promote only if §6.4 wins. If §6.4 loses, mark the overhead claim contradicted for this rig. Otherwise stays Unproven. |

## 8. KC-1 Evaluation

This benchmark evaluates the CPU perf clause of KC-1; it cannot alone fire the
full KC-1 because KC-1 also depends on GPU/vLLM utilization and on whether a
target customer finds the security/sovereignty value compelling enough to pay a
performance gap.

The CPU perf clause is marked **fired for this benchmark** if all are true:

- LogansOS loses single-stream p99 latency by §6.1.
- LogansOS loses tokens/sec by §6.2.
- LogansOS does not win either multi-tenant fairness (§6.5) or noisy-neighbor
  resistance (§6.6).
- Governance overhead fails §6.4 or footprint loses §6.3.

If the CPU perf clause fires, the FINDINGS headline must say so and must propose
ledger edits that drop any CPU serving-performance advantage claim. If the CPU
perf clause does not fire, the FINDINGS must still leave GPU/vLLM claims
Unproven.

## 9. Required Raw Artifacts

The harness phase must write, at minimum:

- `Plans/benchmarks/2026-06-17-llamacpp-cpu/raw/*.jsonl` or `.csv` sample files;
- run manifest with commit SHAs, model SHA-256, command lines, CPU pinning, and
  thermal/power notes;
- summary table used by
  `Plans/RESEARCH-2026-06-17-benchmark-vs-llamacpp-FINDINGS.md`;
- explicit "valid for ledger movement: yes/no" field per scenario.

Raw artifacts are append-only once a measured run starts. If a run is invalid,
keep it and label it invalid; do not delete unfavorable samples.

## 10. Ratification Checklist

PM/PL ratification should answer:

1. Is the fair baseline Linux+llama.cpp CPU, not vLLM/GPU, ratified for this
   build?
2. Which exact rig mode and hardware ID are ratified?
3. Are the sample counts and thresholds in §3 and §6 accepted?
4. Is the KC-1 CPU perf-clause trigger in §8 accepted?
5. Are the §7 ledger-disposition rules accepted, including keeping GPU/vLLM
   rows Unproven after a CPU-only run?

Only after those answers are ratified may ISC-2 harness implementation and ISC-3
measurement proceed.
