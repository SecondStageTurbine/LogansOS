<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Phase 8 benchmark harness

`bench-harness` is the ISC-2 harness for the ratified LogansOS vs
Linux+llama.cpp CPU benchmark. It is deliberately rig-agnostic: LogansOS and
llama.cpp are external commands, so the same tool can run controlled same-host
VM/QEMU shakedowns and the later bare-metal dual-boot run.

The harness does not know how to produce a benchmark by itself. It records the
pre-number manifest, launches configured subject commands, preserves raw output,
and summarizes only the pre-registered metrics.

## Required sequence

1. Create a manifest before any sample:

   ```powershell
   cargo run -p bench-harness -- manifest `
     --out bench-runs/2026-06-17-isc3-ryzen-vm `
     --rig-mode CONTROLLED_SAME_HOST_VM `
     --hardware-id "Ryzen gaming PC; exact CPU/RAM/firmware/power filled before run" `
     --cpu-pinning "fill exact pin mask and commands before run" `
     --load-state "PL idle; no unrelated workloads" `
     --model E:\models\Qwen2.5-0.5B-Q4_K_M.gguf `
     --llamacpp-src "E:\AI Automation\llama.cpp-5f04dc7"
   ```

2. Check the manifest:

   ```powershell
   cargo run -p bench-harness -- check-manifest `
     --manifest bench-runs/2026-06-17-isc3-ryzen-vm/manifest.json
   ```

3. Run subject commands. The command must emit per-token lines:

   ```text
   NEO_BENCH/1 event=token index=0 latency_ns=1234567
   NEO_BENCH/1 event=done tokens=128 decode_ns=158000000 rss_bytes=123456789
   ```

   Serial sample:

   ```powershell
   cargo run -p bench-harness -- run-command `
     --manifest bench-runs/2026-06-17-isc3-ryzen-vm/manifest.json `
     --subject llamacpp `
     --scenario single_stream `
     --samples 10 `
     --warmups 1 `
     --tokens 128 `
     -- path\to\subject.exe --subject-specific-args
   ```

   Multi-tenant sample:

   ```powershell
   cargo run -p bench-harness -- run-parallel `
     --manifest bench-runs/2026-06-17-isc3-ryzen-vm/manifest.json `
     --subject logansos `
     --scenario tenants_4 `
     --tenants 4 `
     --samples 10 `
     --warmups 1 `
     --tokens 128 `
     -- path\to\subject.exe --subject-specific-args
   ```

4. Summarize the raw CSV:

   ```powershell
   cargo run -p bench-harness -- summarize `
     --raw bench-runs/2026-06-17-isc3-ryzen-vm/raw/samples.csv `
     --out bench-runs/2026-06-17-isc3-ryzen-vm/summary.md
   ```

## Integrity notes

- Rows without per-token `NEO_BENCH/1 event=token` lines are
  `valid_for_latency=false` and cannot satisfy p50/p99 claims.
- `--allow-wall-clock-only` is for harness shakedown only.
- The controlled same-host Ryzen run is an ISC-3 scoping run unless PM/PL
  explicitly promote it. The bare-metal dual-boot rig remains the ledger-grade
  hardware run.
- No metric, threshold, or disposition may be changed after numbers exist.
