<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# CLOSE REPORT: Phase 5 BUILD-3 CPU Inference Executor

**Branch:** `codex/phase-5-build-3-cpu-inference-jarvis`
**Dispatch:** `Plans/DISPATCH-2026-06-01-phase-5-build-3-cpu-inference-executor-jarvis.md`
**Design contract:** `docs/ADR-029-phase-5-layer-2-cpu-inference-design.md`
**Status:** IMPLEMENTED - targeted BUILD-3 gate PASS; full-sweep serial evidence green with exit-code capture note

## Summary

BUILD-3 implements the Layer-2 CPU inference backend:

- `Model` now retains validated graph metadata (`nodes`, `ios`, `value_count`) and exposes read-only accessors for the executor.
- `Tensor` has writable allocation plus guarded mutation/read APIs for inference activations; immutable tensors reject mutation.
- `kernel/src/ai/exec.rs` walks graph nodes in file order, requires produced-before-use inputs, dispatches BUILD-3 ops by `match`, and returns frozen output tensors.
- `kernel/src/ai/ops/` implements Identity, Reshape, Transpose, Add, Mul, MatMul, and Relu on f32 bit patterns through software helpers.
- `SYS_INFER_RUN = 0x120` is registered in kernel and userland, with kernel-allocated output tensor handles.
- `infer_mlp_forward` stages `valid_mlp.neom`, runs `SYS_MODEL_LOAD` then `SYS_INFER_RUN`, and checks output bytes against a hand-computed oracle.
- The BUILD-2 absolute-offset compatibility gaps are closed for the kernel parser, `neomc`, model-logic tests, and `neofs-mkfs`'s embedded default `model.neom`.

## ISC Disposition

| Gate | Result | Evidence |
|---|---:|---|
| ISC-1 - Model graph retention | PASS | `Model` stores `nodes`, `ios`, `value_count`; accessors are used by `ai::exec`. `cargo check -p neoplutarch-kernel --features neo-test-infer-mlp-forward` compiles. |
| ISC-2 - Writable tensor API | PASS | `Tensor::allocate_writable`, `bytes`, guarded `bytes_mut`, and `freeze` added; immutable tensors reject mutation with `PermissionDenied`. New unsafe blocks have SAFETY comments. |
| ISC-3 - Soft-float ops | PASS | `kernel/src/ai/ops/` implements the BUILD-3 op set using `u32` f32 bit patterns and `soft_f32::{add,mul,relu}`. Disassembly of the linked kernel found no XMM/SSE scalar FP patterns in `ai::ops` arithmetic or `ai::ops::soft_f32`. |
| ISC-4 - Executor | PASS | Executor uses a per-value table, file-order node walk, produced-before-use guard, clean unsupported-op error, dtype/shape checks, and no panics on graph errors. |
| ISC-5 - `sys_infer_run` | PASS | `SYS_INFER_RUN = 0x120`; syscall dispatch validates model/input/output buffers, runs the executor, installs output tensor caps, and writes `u32` output handles to userspace. |
| ISC-6 - Errno | PASS | Mappings cover stale model `ESTALE`, bad handles `EBADF`, rights `EPERM`, flags/dtype/shape/graph errors `EINVAL`, short output buffer `E2BIG`, OOM `ENOMEM`, unsupported op `ENOSYS`. |
| ISC-7 - Host unit tests | PASS | `cargo test -p ai-ops-tests` passes 5 tests: soft-float known patterns, 2x2 MatMul, Add broadcast, Relu, and full MLP oracle. `Cargo.lock` was regenerated for the new workspace member. |
| ISC-8 - QEMU scenario | PASS WITH EXEC-STATUS NOTE | Targeted Docker QEMU `infer_mlp_forward` PASS at `NEO_SMP=4`, with output bits `[0x4330d1ea,0x43407f7d,0x43502d0e]`. `model_load_cascade` initially exposed a stale relative-offset `neofs-mkfs` default model and now PASSes after the absolute-offset fix. A Docker `neo-test all --no-meta` rerun progressed through the final listed scenario, `phase3_aarch64_virtio_barrier_lane`, whose serial log PASSes; serial scan found no unexpected `event=FAIL`, `PANIC`, or `BUG:` beyond the deliberate/expected meta-panic cases. The outer tool timeout means the final process exit code was not captured. |
| ISC-9 - Standards + lean | PASS | `cargo fmt`, focused tests/checks, host clippy for `ai-ops-tests`, Docker QEMU gates, no-XMM scan, and `git diff --check` completed. Kernel compile still reports the existing warning set. Anti-scope held: no hardware FP, no transcendentals, no accelerator path, no async/scheduler class. |

## MLP Oracle

Input tensor: `[1.0, 2.0, 3.0, 4.0]` as F32 little-endian bytes.

Expected output shape: `[1, 3]`

Expected output bits:

```text
[0x4330d1ea, 0x43407f7d, 0x43502d0e]
```

Expected output bytes:

```text
ea d1 30 43 7d 7f 40 43 0e 2d 50 43
```

QEMU evidence:

```text
target\neo-test\infer_mlp_forward.attempt1.serial.log:
NEO_TEST/1 event=PASS name=infer_mlp_forward model=valid_mlp input=[1,2,3,4] output_bits=[0x4330d1ea,0x43407f7d,0x43502d0e] sys_infer_run=ok storage_freed=ok
```

## No-XMM Evidence

Linked kernel scanned:

```text
target\x86_64-neoplutarch\debug\deps\kernel-335884a3d3f32313
```

Symbol ranges scanned with Docker `/usr/bin/objdump -d`:

```text
ai::ops::soft_f32       0xffffffff8009b790..0xffffffff8009c4c0
ai::ops arithmetic path 0xffffffff80116ba0..0xffffffff80118a00
```

Pattern set:

```text
xmm, movss, addss, mulss, divss, subss, ucomiss, comiss, cvtsi2ss, cvtss2si, movaps, movups, movdqa, xorps, pxor
```

Result:

```text
NO_XMM_CHECK=PASS
```

## Validation Commands

```text
cargo fmt
cargo test -p neomc
cargo test --manifest-path tools\model-logic-tests\Cargo.toml
cargo test -p ai-ops-tests
cargo clippy -p ai-ops-tests -- -D warnings
cargo check -p neofs-mkfs
cargo check -p xtask
cargo check -p neo-init --features infer-mlp-forward
cargo check -p neoplutarch-kernel --features neo-test-infer-mlp-forward
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test infer_mlp_forward --no-meta
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test model_load_cascade --no-meta
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test all --no-meta
git diff --check
```

`git diff --check` reports line-ending normalization warnings only; no whitespace errors.
