<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: 0.1 product integration build — Chad

**Date:** 2026-08-10 · **From:** Chad (Codex) · **To:** PM (Claude)
**Dispatch:** `Plans/DISPATCH-2026-08-07-01-product-integration-build-chad.md`
**Closes:** OT-78
**Branch:** `codex/product-integration-build-chad`
**Implementation:** `c8b9cb9ed2fe24faf9961391938bd9b8fec77b02`
**Base:** `origin/main` @ `39cf5b18ae03d48f194e379bc040cba9328d2f40` (ff-clean)
**RAG:** healthy (`ollama=true`, `qdrant=true`); used for product, NeoFS boot-arm, xtask, and Arthur/Atrium discovery before direct reads.

## Verdict

OT-78 is implemented. CI now has a blocking `cargo build -Zjson-target-spec --bin kernel --features product` job. One product-feature QEMU scenario proves the complete 0.1 path on boot 1 and exact persisted-byte recovery on boot 2. Three additional QEMU scenarios execute every `boot-neofs` degradation arm, including the two-device discriminating probe path. No timeout was raised.

## What shipped

- `product-build` is a blocking prerequisite of host checks, which in turn remains the prerequisite of the four QEMU shards. The older OT-69 feature matrix remains explicitly non-blocking.
- `product_integration` selects `neo-test-product-integration = ["neo-test-arthur-compositor", "product"]`, builds the DW-1 worker role and interactive Arthur role, uses `-cpu max`, 4096 MiB, and a 768 MiB Arthur-memory NeoFS image.
- Boot 1 runs the shipping feature composition through framebuffer, DW-1, typed Arthur inference, clean no-dongle `eth0`, and a fsynced persistence write.
- Boot 2 reuses the same raw disk, mounts through the product boot path, byte-compares the prior record, and earns a distinct kernel PASS only after the user probe returns its readback-success code. It does not redundantly rerun the expensive desktop/inference proof.
- `boot_neofs_no_block_device`, `boot_neofs_no_valid_superblock`, and `boot_neofs_unexpected_error` cover all degrade arms. The catch-all scenario attaches invalid device 0 plus valid device 1, proves two probes, then deliberately reaches the ordinary mount-table error after selecting device 1.
- The hermetic QEMU Arthur fixture still performs a live tensor forward. Its product-only vocab maps the selected non-EOS token to `Arthur is ready.` so the coherent-answer oracle has teeth; an explicitly supplied real Stage-B artifact retains its real adjacent vocabulary.

## Per-rung failure conditions

| Rung | Runtime proof | What makes it fail |
|---|---|---|
| 1 — boot/framebuffer | The kernel reads framebuffer pixel `(0,0)` only after the product boot reaches the harness. | Missing framebuffer returns `None` and calls `fail`; a panic or failure to reach the harness cannot produce the single terminal PASS. |
| 2 — desktop | Real DW-1 creates two `ROLE_TEXT` clients, composites them, switches focus A then B, observes exactly two focused deliveries and zero unfocused deliveries, and requires both clients' direct present attempts to return `EPERM`. | Every protocol syscall/response has a nonzero return code; bad focus activation/injection, lost/duplicated delivery, unfocused delivery, bad commit, failed `EPERM`, bad composition, or bad child exit makes neo-init nonzero and the kernel emits FAIL. |
| 3 — Arthur | Twelve HID usage IDs spell `hello world\n` through the existing InputFocus router. Arthur performs the forward, requires a reply of at least four bytes that is non-echo, printable/whitespace, and alphabetic, logs the active model's runtime `/neofs/...` path, commits the answer serial, and the compositor observes/draws it. | Injection failure, missing key, inference/decode failure, empty/short/echo/non-text reply, missing dynamic path breadcrumb, missing answer commit, surface protocol error, or nonzero Arthur exit prevents the ordered runner oracle and terminal PASS. |
| 4 — `eth0` | The real boot bring-up publishes `BootEth0Status::NoDevice` under QEMU, where no AX88179 is attached; boot continues. | Any status other than the runtime `NoDevice` latch calls `fail`. The runner also requires the ordered production no-device boot line before the product rung latch. |
| 5 — persistence | Boot 1 writes exactly 39 bytes, calls `fsync`, closes, reopens, and byte-compares. Boot 2 mounts the same image, reads exactly 39 bytes, and byte-compares against the compiled pattern. | Short write/read, fsync failure, reopen failure, content mismatch, missing record, wrong second-boot exit code, newly seeded disk, or absent `stage=readback` prevents PASS. |

## NeoFS degradation evidence

| Scenario | Runtime discriminator | Local QEMU |
|---|---|---|
| `boot_neofs_no_block_device` | `NoBlockDevice`, exactly 0 probe attempts, harness reached | PASS, 5.2 s / 180 s |
| `boot_neofs_no_valid_superblock` | `NoValidSuperblock`, exactly 1 probe attempt | PASS, 5.8 s / 180 s |
| `boot_neofs_unexpected_error` | catch-all `UnexpectedError`, exactly 2 probes, invalid index 0 skipped before valid index 1 | PASS, 10.7 s / 180 s |

These are runtime outcome latches and counters compiled only for `neo-test`; the assertions do not infer an arm from a serial sentence.

## Product scenario evidence

- Boot 1: PASS, 116.7 s / 180 s.
- Boot 2: PASS, 41.5 s / 180 s.
- Persistence: one raw image reused across both boots; `stage=write bytes=39 fsync=ok same_boot_readback=byte_equal`, then `stage=readback bytes=39 source=prior_boot byte_equal=true fsync_prior_boot=true`.
- Arthur reply: `reply="Arthur is ready."`; coherence true; non-echo true; provenance `/neofs/model-qwen2.5-0.5b.neom`; answer serial composited.
- Shard coverage: 290 canonical scenarios partitioned across 4 shards, union equals full suite, each scenario exactly once.

## Kill-switch boundary, stated literally

The new outcome latch, probe counters, busy-mount setup, input injector exposure, eth0 status reader, scenario routing, and product user drivers are test-only. In non-test production builds, turning `product` off removes the product boot pair and restores the prior default boot path; the new observability code is absent. Existing standalone scenarios remain mapped to their prior features.

This is **not** a blanket historical bit-equality claim. Rung-5 F1/F2 already ride unconditionally in all builds and remain unconditional; this dispatch neither hides nor changes them. The equality claim covers the new OT-78 integration/test machinery and the pre-existing `product` feature switch, not those earlier unconditional fixes.

## Scope and gates

- No bare-metal run.
- No OptiPlex or physical-disk access; no `mkfs` outside disposable test images.
- No OT-73 model-verification change.
- No scenario budget increase; the product scenario retains 180 s per boot.
- Local product compile: PASS.
- `cargo test -p xtask`: 25/25 PASS.
- `cargo run -p xtask -- shard-coverage 4`: PASS.
- `scripts/worker-close-gate.sh` exact host set: PASS after final rebase (format, workspace tests, workspace clippy, blocking kernel clippy, USB-host kill-switch KAT, SPDX).
- Push CI (exact implementation SHA): `https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/31399685395`
- Four-shard `workflow_dispatch` (exact implementation SHA): `https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/31399835169`

Both URLs were checked at `headSha=c8b9cb9ed2fe24faf9961391938bd9b8fec77b02`.
The workflow-dispatch completed `success` on attempt 1: all four QEMU shards, Host Tests/Format/Lint,
Product Feature Build, SPDX, Monolithic Identity Guard, and all five feature-matrix legs were individually
`success`. On shard 4, product boot 1 passed in 142.7 s and boot 2 in 40.2 s against the unchanged 180 s
budget; no-block-device passed in 2.9 s. The other degrade scenarios passed on shard 3
(no-valid-superblock, 3.2 s) and shard 1 (catch-all, 3.4 s).

The push run completed `success` on attempt 2. Attempt 1 had one failure among 243 `ai-ops-tests`:
the pre-existing cycle-timing oracle sampled 8 devices anomalously fast and measured 128 devices at
6540 cycles against the derived 5840 ceiling. It is outside this diff. The failed job alone was rerun
on the identical SHA and passed; the local close-gate had already passed the same complete host-test set.
