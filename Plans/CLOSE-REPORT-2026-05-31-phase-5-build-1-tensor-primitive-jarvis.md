<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# CLOSE REPORT: Phase 5 BUILD-1 - Tensor Primitive

**Date:** 2026-05-31
**Worker:** Jarvis
**Branch:** `codex/phase-5-build-1-tensor-primitive-jarvis`
**Base:** `origin/main` at `79ed760`
**Dispatch:** `Plans/DISPATCH-2026-05-31-phase-5-build-1-tensor-primitive-jarvis.md`
**Status:** PASS - ready for PM review/merge

## Scope Landed

- Added `kernel/src/ai/` with the BUILD-1 tensor metadata, validation logic, and buddy-frame-backed tensor storage.
- Added `BuddyAllocator::deallocate_blocks(order)` and tensor storage `Drop` so the last `Arc<Tensor>` release frees the contiguous frame block.
- Added `Handle::Tensor` with normal capability-table integration, `cap_id`, rights propagation, and derivation tracking.
- Added AI syscalls in the required range:
  - `SYS_TENSOR_CREATE = 0x100`
  - `SYS_TENSOR_QUERY = 0x101`
  - `SYS_TENSOR_SHARE = 0x102`
  - `SYS_TENSOR_DROP = 0x103`
- Added `libneo` tensor ABI wrappers and the `tensor_create_query_drop` userspace scenario.
- Added host-testable tensor logic coverage in `tools/tensor-logic-tests`.

## Contract Notes

- Tensor data bytes are stored in contiguous buddy frames, not slab/heap. Tensor metadata remains an `Arc<Tensor>`, as allowed by the dispatch.
- `mutable` is stored and queryable. Immutability enforcement is intentionally deferred because BUILD-1 introduces no tensor mutator and does not invent one just to test `EPERM`.
- `Device::Accel(u32)` exists only as a reserved ABI shape. No accelerator driver or device path was added.
- The tensor QEMU scenario emits its terminal PASS from a test-only user-exit hook after userspace drops both tensor handles and the storage alloc/free counters match. Production `sys_exit` remains on the normal scheduler path outside `neo-test-tensor-create-query-drop`.
- No cryptographic primitive, handshake, cipher, hash, or key-exchange code was added in this build.

## Verification

| Command | Result |
| --- | --- |
| `docker compose run --rm neo rustfmt --config edition=2024,skip_children=true --check ...` | PASS |
| `docker compose run --rm neo cargo test --manifest-path tools/tensor-logic-tests/Cargo.toml` | PASS - 5 tests |
| `docker compose run --rm neo cargo check -p neoplutarch-kernel --target x86_64-unknown-none` | PASS |
| `docker compose run --rm neo cargo check -p neoplutarch-kernel --target x86_64-unknown-none --features neo-test-tensor-create-query-drop` | PASS |
| `docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test tensor_create_query_drop --no-meta` | PASS |
| `docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test q8c_blocked_kill_foreign_parked --no-meta` | PASS |
| `docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test all --no-meta` | PASS |
| `git diff --check` | PASS - CRLF warnings only |
| anti-criteria diff grep for `0x1A0`, `.neom`, `virtio-gpu`, tensor mutators | PASS - no new matches |
| new tensor code grep for `unwrap(`, `expect(`, `panic!` | PASS - no matches |

## Serial Excerpt

`target/neo-test/tensor_create_query_drop.attempt1.serial.log`:

```text
NEO_TEST/1 event=BEGIN name=tensor_create_query_drop phase=dispatch
[ kernel ] proc: user thread exited with code 0
NEO_TEST/1 event=PASS name=tensor_create_query_drop tensor=ok refcount=ok stale_handle=EBADF storage_freed=ok immutable_mutation_deferred=no_mutator
```

Adjacent integration fallout from adding `Handle::Tensor` was also closed:

```text
NEO_TEST/1 event=PASS name=q8c_blocked_kill_foreign_parked mechanism=q8c_blocked_kill_foreign_parked kill_rc=0 foreign_wake=EPIPE foreign_zombified=false target_reaped=true frame_audit_clean=true
```

## Return

BUILD-1 lands the Phase 5 tensor primitive with buddy-frame data storage, capability-gated handles, refcount-preserving share/drop semantics, host logic tests, and SMP=4 QEMU coverage. The full `neo-test all` gate passed at `NEO_SMP=4`.
