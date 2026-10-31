<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# CLOSE REPORT: Phase 5 BUILD-2 Kernel Model Object

**Branch:** `codex/phase-5-build-2-kernel-model-object-jarvis`
**Dispatch:** `Plans/DISPATCH-2026-05-31-phase-5-build-2-kernel-model-object-jarvis.md`
**Format contract:** `docs/ADR-028-neom-v1-format-and-model-object.md`
**Status:** PASS - ready for Cerebro review

## Summary

BUILD-2 track 2-kernel lands the Layer-1 kernel model object:

- `.neom` v1 parse/validate path in `kernel/src/ai/model_logic.rs`.
- `Model` object in `kernel/src/ai/model.rs` with immutable snapshotted weight tensors.
- `Handle::Model`, model rights, and syscalls `0x110..0x112`.
- §D8 staleness by `{ mount_id, inode generation }` provenance: model operations return `ESTALE` after unmount/stale source.
- No op execution and no `SYS_MODEL_INFER`; BUILD-2 validates graph metadata only.

## ISC Disposition

| Gate | Result | Evidence |
|---|---:|---|
| ISC-1 - Modules | PASS | `ai/model.rs` + `ai/model_logic.rs` added and wired through `ai/mod.rs`, both with SPDX and module docs. |
| ISC-2 - Parser | PASS | `.neom` v1 parser uses checked arithmetic and bounds-before-read parsing; no raw struct casts required. |
| ISC-3 - Validation | PASS | Header, table bounds, disjoint table ranges, tensor sizes/alignment, op set, refs, IO entries, and DAG/topological order validated. |
| ISC-4 - Model object | PASS | `Model` stores graph metadata, immutable `Arc<Tensor>` weights, and source provenance; `OpenFile` is dropped after snapshot. |
| ISC-5 - Kernel load path | PASS | `SYS_MODEL_LOAD` opens via root-temporary policy, reads a bounded whole-file buffer, validates, snapshots, and installs a model cap. |
| ISC-6 - `Handle::Model` | PASS | `Handle::Model`, `model_rights::USE`, `full_model_rights`, cap lookup/drop/revoke arms added. |
| ISC-7 - Syscalls | PASS | `SYS_MODEL_LOAD=0x110`, `SYS_MODEL_QUERY=0x111`, `SYS_MODEL_DROP=0x112`; no `SYS_MODEL_INFER`. |
| ISC-8 - §D8 staleness | PASS | Loaded model does not block unmount; later query returns `ESTALE` after mount disappearance. |
| ISC-9 - Errno | PASS | Validation failures map to `EINVAL`, oversized reads to `E2BIG`, allocation to `ENOMEM`, bad caps to `EBADF`, stale provenance to `ESTALE`. |
| ISC-10 - Host unit tests | PASS | `tools/model-logic-tests`: valid header/graph plus bad magic/version, offset overflow, weight mismatch, unknown op, and cyclic/future ref rejects. |
| ISC-11 - QEMU scenario | PASS | `model_load_cascade` passes at `NEO_SMP=4`; full `neo-test all --no-meta` at `NEO_SMP=4` exited 0. |
| ISC-12 - Standards + lean | PASS WITH ENV NOTE | `cargo fmt` and `git diff --check` clean. `cargo clippy --version` confirms `cargo-clippy` is not installed in the minimal `nightly-2026-04-16` container; same environment limitation as the sibling BUILD-2 tool close. No BUILD-2 op execution, no kernel importers, no accel path, no global unmount rewrite. |

## Layer-1 Scope

ADR-028 D2.7 is implemented as the scoped Layer-1 reading: BUILD-2 validates structural graph integrity, weight integrity, op validity, ref validity, IO dtype validity, and acyclicity. Per-op output-shape derivation remains deferred to Layer 2 because it depends on execution semantics.

## §D8 Evidence

The ship scenario loads `/neofs/model.neom`, queries the model, unmounts NeoFS while the model cap remains live, verifies unmount is not `EBUSY`, verifies model query returns `ESTALE`, then drops the model and confirms tensor storage frees.

Serial excerpt:

```text
NEO_TEST/1 event=PASS name=model_load_cascade model=ok neom_v1=validated graph=validated_no_execute weights=snapshot_immutable staleness=ESTALE live_file_handle=none umount_not_busy=ok storage_freed=ok
```

## Validation Commands

```text
docker compose run --rm neo cargo fmt
docker compose run --rm neo cargo test --manifest-path tools/model-logic-tests/Cargo.toml
docker compose run --rm neo cargo test --manifest-path tools/tensor-logic-tests/Cargo.toml
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test model_load_cascade --no-meta
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test q8c_blocked_kill_foreign_parked --no-meta
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test all --no-meta
git diff --check
docker compose run --rm neo cargo clippy --version
```

`cargo clippy --version` result: `cargo-clippy` is not installed for `nightly-2026-04-16-x86_64-unknown-linux-gnu`; install would require a toolchain component change outside this build.
