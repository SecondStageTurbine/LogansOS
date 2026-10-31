<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: The Benchmark — LogansOS vs Linux+llama.cpp (the §3 falsifiability run) → Jarvis

**Date:** 2026-06-17
**From:** PM (Claude)
**To:** Jarvis (built the latency oracle / BUILD-LO, the self-hosted CI, the QEMU gate — the measurement-infra fit). New branch off `origin/main` HEAD: `codex/phase-8-benchmark-vs-llamacpp-jarvis`.
**Status:** DISPATCHED. **The build that tests the performance thesis — and is designed to be able to KILL it.** This is the run that moves Strategic Honesty Ledger §3 rows from *Unproven* to *Proven*, leaves them *Unproven*, or **fires KC-1**. A benchmark you can only pass is not a benchmark; this one must be able to falsify.

## 0. The decision this build makes
Run the **fair, answerable-now** head-to-head and report it honestly, whichever way it falls. Per the Strategic Honesty Ledger (`docs/STRATEGIC-HONESTY-LEDGER.md` §3/§5), the perf-superiority claims are *hypotheses with a harness, not results*; this build produces the results. **If LogansOS loses on single-stream latency/throughput — which is plausible — that is reported as prominently as any win, and KC-1's perf clause is evaluated honestly.** The thesis's real differentiator is sovereignty/enforcement (the §2 structural claims), which does **not** depend on winning this benchmark — so there is zero incentive to rig it, and rigging it would corrupt the one discipline (the ledger) that makes the whole project trustworthy.

## 1. The honest scoping — read before designing
- **LogansOS runs the LLM on CPU today.** The 8.5 forward pass is CPU (f32 dequant→matmul); the accel-driver class has op-stubs but the full forward pass is **not** on GPU yet. Therefore:
- **The fair baseline is Linux + `llama.cpp` (CPU), same model, same quantization** — Qwen2.5-0.5B-Q4_K_M on both, CPU on both. This isolates the *actual* §3 question that is answerable now: **does putting inference in a governed kernel cost more or less than userspace `llama.cpp` on Linux?** — not the confounded CPU-vs-GPU question.
- **vLLM-on-GPU is explicitly DEFERRED** to when LogansOS runs the LLM on the accel (Phase 9+ / a later accel-forward-pass build). Do **not** benchmark CPU-LogansOS against GPU-vLLM; a rigged CPU-vs-GPU loss is as dishonest as a rigged win. State this deferral in the FINDINGS.
- **Same hardware, real metal (or controlled identical QEMU on one host).** Perf numbers from QEMU-on-a-busy-laptop are noise. Specify the rig in ISC-1 (bare-metal on a dedicated box for both, or both as same-host controlled VMs) — this is PL-coordinated, like the silicon runs.

## 2. ISCs
- **ISC-1 — PRE-REGISTER the methodology (ratify before any number exists).** A short `Plans/` doc fixing, *in advance*: (a) the exact metrics — single-stream **p50/p99 token latency**, tokens/sec, **resident footprint** (whole-OS RSS for the serving path), and **governance overhead** (daemon-on vs backstop-only via the latency oracle); plus the multi-tenant set — **tail-latency + fairness under N concurrent tenants** and **noisy-neighbor degradation**; (b) the baseline (`llama.cpp` build flags, threads, same quant); (c) the **rig** (hardware, pinning, warmup, sample count, how variance is reported); (d) **the win/loss criterion per §3 row + the KC-1 trigger** — what number = Proven, what = stays-Unproven, what = thesis-loss. **PM/PL ratify ISC-1 before ISC-2/3.** This is the anti-rigging backbone (the byte-oracle "agree on the answer before you measure" rule, applied to perf).
- **ISC-2 — The harness + scenarios.** Extend the latency oracle (ADR-052) to capture p50/p99/footprint; build the LogansOS serving scenario (the 8.5 generate path under load) and the matched `llama.cpp` baseline harness; the **multi-tenant** scenario (≥2 concurrent inference tenants) — the most thesis-relevant test, where the admission scheduler + governed residency *might* beat naive Linux process contention on fairness/tail-latency (or might not). Reproducible: one command, logged config, raw samples retained.
- **ISC-3 — Run on the ratified rig + FINDINGS.** `Plans/RESEARCH-2026-06-17-benchmark-vs-llamacpp-FINDINGS.md`: the raw numbers (with variance), **each §3 row's disposition** (Proven-with-evidence / stays-Unproven / KC-1-fired), and the proposed Strategic-Honesty-Ledger edits (the PM applies them). Losses reported as prominently as wins. Where LogansOS wins, name *why* (which mechanism); where it loses, name *why* and whether it's fixable or inherent.

## 3. Integrity rules (non-negotiable)
- **No metric changes after numbers exist.** The ratified ISC-1 criteria are frozen; you may *add* metrics but never retire a pre-registered one because it came out unfavorable.
- **Report the loss.** If single-stream latency is worse than `llama.cpp` (plausible — `llama.cpp` is heavily optimized and LogansOS adds governance), the FINDINGS says so in the headline, and the ledger's §3 row stays Unproven or KC-1's first clause is marked fired pending the security-value half (KC-1 is *and*-gated: perf-loss AND security-not-compelling).
- **Apples-to-apples or it doesn't count.** Same model file, same quantization, same CPU, same thread count, same context length, warmup excluded, ≥N samples with reported p99 (not just mean).

## 4. Scope guard
The fair CPU comparison (LogansOS vs Linux+`llama.cpp`) **only**. **Do NOT** benchmark against vLLM/GPU (deferred), tune LogansOS specially for the benchmark (measure the shipping path), or change the ratified metrics post-hoc. This is a measurement + a FINDINGS, plus the harness code; no kernel feature work beyond the latency-oracle capture extension. `fmt`/`clippy` on any harness code; kernel-touching harness bits ⇒ the pre-merge QEMU gate.

## 5. Merge / close (PM)
ISC-1 ratified before the run. On ISC-3: PM reviews the FINDINGS, applies the Strategic-Honesty-Ledger edits (promote / hold / fire), and updates the bible. The harness branch merges via `pm-merge-check` → `--ff-only`/cherry-pick. **This is the build that turns "the model generates" into a *measured* claim — in whichever direction the data points.**

**Disk reclaim (after PM confirms merge):** once on `origin/main` and PM-confirmed, delete this build's tree (`git worktree remove` / folder delete; never an unmerged tree). See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Jarvis
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-17-benchmark-vs-llamacpp-jarvis.md` (`git show origin/main:<path>`).
- **Branch:** `codex/phase-8-benchmark-vs-llamacpp-jarvis` off `origin/main`.
- **One-line:** Run **the §3 falsifiability benchmark** — and design it to be able to **kill** the perf thesis, not just confirm it. **Honest scoping:** LogansOS runs the LLM on **CPU** today, so the fair baseline is **Linux + `llama.cpp` (CPU)**, same Qwen2.5-0.5B-Q4_K_M, same hardware — **NOT vLLM-GPU** (that's deferred to when LogansOS runs on the accel; a CPU-vs-GPU loss would be rigged). **PRE-REGISTER first (ISC-1, PM/PL ratify before any number):** the metrics (single-stream p50/p99 latency, tok/s, whole-OS footprint, governance overhead via the latency oracle; multi-tenant tail-latency + fairness + noisy-neighbor — the thesis-relevant one), the `llama.cpp` baseline config, the rig (real metal or controlled same-host, PL-coordinated), and the **win/loss criterion per §3 row + the KC-1 trigger**. Then build the harness (extend ADR-052's oracle), run reproducibly, and deliver `Plans/RESEARCH-2026-06-17-benchmark-vs-llamacpp-FINDINGS.md` with the raw numbers + each §3 row's disposition + proposed ledger edits. **Integrity: no metric changes after numbers exist; report losses as prominently as wins; same model/quant/CPU/threads or it doesn't count.** The thesis stands on sovereignty regardless of this result — so there's no reason to rig it and every reason not to. fmt/clippy clean; kernel-touching harness bits ⇒ pre-merge QEMU gate.
