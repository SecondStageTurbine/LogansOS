<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-056: Phase 8 Rule-4 consolidation

**Status:** Accepted
**Date:** 2026-06-17
**Authority:** `docs/complexity-governance.md` Rule 4, after the Phase 8 inference arc landed BUILD-8.1 through BUILD-8.5.
**Dispatch:** `Plans/DISPATCH-2026-06-17-phase-8-consolidation-jarvis.md`
**Scope:** no features, no behavior change, no new invariant, no new `unsafe`.

## Decision

Consolidate the Phase 8 inference arc without changing its runtime behavior:

- Make the existing clippy debt a blocking gate instead of a recurring advisory.
- Add an explicit no_std userland workspace-hygiene guard for the CI #243 class:
  std-only tool dependencies must not feature-unify into the no_std userland
  binaries.
- Collapse duplicated Qwen2 KV-cache storage-layout code between the host KAT
  cache and the governed arena cache.
- Reconcile the live ADR/ANSPEC/Bible/ledger/open-thread pointers.

## CI hygiene

The branch makes the full workspace clippy command blocking:

```text
cargo clippy --workspace --all-targets -- -D warnings
```

That exact command is the cross-member feature-unification tripwire. It caught
the CI #243 class when a std-only benchmark harness lived in the workspace and
pulled std-requiring dependency features into the no_std userland binaries.

The CI job also runs a named userland target-spec check:

```text
cargo check \
  -Zjson-target-spec \
  -Zbuild-std=core,compiler_builtins,alloc \
  -Zbuild-std-features=compiler-builtins-mem \
  --target userland/x86_64-neoplutarch-user.json \
  -p neo-init -p neo-worker -p neo-shell
```

The full-workspace clippy step catches cross-member feature leaks; the userland
check localizes no_std failures to the binaries that own the panic handler.
Kernel clippy is also promoted from run-and-report to blocking:

```text
cd kernel && cargo clippy -Zjson-target-spec -- -D warnings
```

## AI code consolidation

BUILD-8.5 created two consumers of the same Qwen2 KV layout:

- `forward_logic::Qwen2KvCacheLayer`, used by host KATs and the default-off
  forward scaffold.
- `kv_cache::GovernedKvCache`, which stores pre-reserved arena tensors.

Both used the same `[head, pos, dim]` storage convention and active-prefix read
order. This ADR factors that convention into `kernel/src/ai/kv_layout.rs` and
wires both consumers through it. The refactor does not change:

- RoPE-at-cache-write behavior.
- Cache-read behavior.
- The cached==uncached KAT surface.
- The real Qwen top-1 oracle.
- The `kv_cache_enabled=false` kill switch.

## Ledger re-audit

Rule-4 audit result: 14/14 existing invariant rows still stand; zero rows added.
BUILD-8.5's governed KV cache remains covered by the existing I-9/I-13 and
reservation rules because the cache tensors are fixed-footprint tensors
registered at pre-reserve time, with `used_len` as controller-local state.

Current structural measurements from this checkout:

| Measurement | Value |
|---|---:|
| Production lock statics | 60 |
| NEO_TEST/test-harness lock statics | 19 |
| Grand total lock statics | 79 |
| `SwitchMutex::new_ordered` call sites | 13 |
| Max documented lockdep level | 31 (`USB_CORE`) |
| Top-level `kernel/src/ai` pure-tier share | 45.3% |
| Pure tier plus `ai/ops` host-tested kernels | 54.7% |

The lock census uses the ADR-048 method: `static` items typed as
`SwitchMutex`, `IrqSafeMutex`, or `spin::Mutex`, including those behind
`Lazy<...>`, with `test_harness.rs` counted separately. The ordered band still
fits the documented `1..=31` domain; a future lock that must order above USB
still requires a design decision before implementation.

## Consequences

- Kernel-wide clippy warnings become merge-blocking debt, not report-only debt.
- The no_std userland panic-handler class is guarded by CI at the workspace
  boundary and at the target-spec boundary.
- The Phase 8 KV layout convention has one implementation point.
- No forward-pass, importer, sampler, or decode-loop behavior changes.
