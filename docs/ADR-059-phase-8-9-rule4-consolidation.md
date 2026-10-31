<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-059: Phase 8/9 Rule-4 consolidation

**Status:** Accepted
**Date:** 2026-06-18
**Dispatch:** `Plans/DISPATCH-2026-06-18-consolidation-phase-8-9-jarvis.md`
**Scope:** seven-build consolidation since `645fd4b`; no features, no new
invariants, no new unsafe, behavior-preserving only.

## Decision

Run the Rule-4 consolidation over the seven post-`645fd4b` builds:

1. Arthur Slice 4.
2. Phase 9.1 scalar Q4_K x Q8_K matmul.
3. Phase 9.2 quantized forward wiring.
4. CSPRNG 10.1.
5. Phase 8b framebuffer/keyboard input path.
6. Phase 9.3 AVX2 host op.
7. Arthur physical-console integration.

The consolidation does not change runtime behavior. It removes avoidable lint
masking, reconciles the governing docs, and records the fresh ledger numbers
for the next feature run.

## Code Consolidation

The scalar and AVX2 quantized-matmul paths were inspected first because they
looked like the highest-value duplication candidate. The duplication did not
survive inspection:

- The fragile Q4_K scale/min depack is already centralized in
  `q4_k::get_scale_min_k4`.
- The scalar loop and AVX2 loop share the same block layout, scale/min lookup,
  q8 `bsums` min-correction, and final dequant formula.
- The remaining difference is the lane representation itself: scalar nibble
  extraction vs `_mm256_maddubs_epi16`/widen-accumulate/horizontal reduce.

Per the BUILD-K rule, this leaves the code cohesive but not worth an
abstraction. A helper would add indirection around two intentionally different
execution forms without removing a second source of truth.

The code edits are lint-debt consolidation only:

| File | Consolidation |
|---|---|
| `kernel/src/main.rs` | Removed the crate-root clippy allow batch inherited from Arthur Slice 3. |
| `kernel/src/ai/ops/qk_matmul.rs` | Removed the module-level `dead_code` allow; 9.2/9.3 now provide real consumers and host gates. |
| `kernel/src/rng/mod.rs` | Replaced the unused KAT re-export allow with `#[cfg(feature = "neo-test")]`. |
| `kernel/src/sync/mod.rs` | Removed an unnecessary unused-import allow on `IrqSafeMutex`. |
| `kernel/src/drivers/usb/mod.rs` | Removed an unused `EcmDescriptor` re-export and its allow. |

The diff against `645fd4b` now has no added `#[allow(...)]` in `kernel/src`.
Older item-level/test-harness allows remain scoped and documented; no blanket
crate-root allow is introduced.

## Document Reconciliation

ADR numbering is reconciled without renumbering shipped Phase 9 records:

- ADR-057 is the CSPRNG 10.1 record.
- ADR-058 remains the Phase 9 quantized-matmul record and now also records the
  9.3 AVX2 note.
- ADR-059 is this consolidation record.

The ANSPEC header and §11.2.4 now include Phase 9.2/9.3 and the YMM-state
blocker. ANSPEC also records ADR-057 as an external security prerequisite for
future Arthur/W2 randomness consumers. The Plans index, Bible Article 13
addendum, open-thread register, and complexity ledger were reconciled to those
records. The resolved clippy-debt open-thread row is removed from the active
register.

## Ledger Audit

No invariant row is added. The post-consolidation audit is:

| Row | Verdict |
|---|---|
| I-1 result-invariance | Preserved by the full family bit-identical gate; Phase 9.3 AVX2 is bit-identical to scalar. |
| I-2 kill-switch/strict superset | Preserved: all seven builds remain feature-gated or host/doc-only, and close-gate reruns the family. |
| I-3 single-mutator relax-and-reprove | Untouched; no tensor migration/reservation path changed. |
| I-4 refcount/unique-owner-at-rest | Untouched; Qwen dual raw+f32 storage does not change slot ownership. |
| I-5 lock order | Preserved; 13 ordered call sites, max lockdep level 31. |
| I-6 accounting exactness | Untouched; no residency/accounting path changed. |
| I-7 residency capacity | Untouched; no device capacity policy changed. |
| I-8 anti-thrash | Untouched; no eviction/retain policy changed. |
| I-9 pin protection | Preserved; governed KV cache remains the same arena/reservation consumer. |
| I-10 temporal isolation | Untouched; no admission or device run-to-completion path changed. |
| I-11 no lost wakeup | Untouched; no park/wake primitive changed. |
| I-12 byte-layout oracle | Preserved and extended by q8_K bytes, Q4_K/Q8_K scalar source oracle, AVX2 equality, CSPRNG KATs, and input-path host KATs. |
| I-13 stale handles fail safe | Untouched; no slot generation behavior changed. |
| I-14 subsystem fault containment | Preserved; HID inherits xHCI arena/VT-d containment and the AVX2 host op never enters the kernel forward path. |

## Re-Measured Ledger

Method: count Rust `static` items under `kernel/src` whose type is
`SwitchMutex`, `IrqSafeMutex`, `spin::Mutex`, or `Lazy<...Mutex...>`, excluding
`kernel/src/test_harness.rs` for the production count. Code LOC counts exclude
blank lines, `//`/`//!`/`///` lines, and block comments.

| Measure | Current |
|---|---:|
| Production lock statics | 64 |
| Test-harness lock statics | 17 |
| Total lock statics | 81 |
| `SwitchMutex::new_ordered` call sites | 13 |
| Max lockdep level | 31 (`USB_CORE`) |
| AI top-level pure LOC / total LOC | 3,461 / 7,010 = 49.4% |
| AI pure plus `ai/ops` LOC / total with `ai/ops` | 5,518 / 9,067 = 60.9% |

The ordered band remains inside the existing `1..=31` lockdep domain. A future
lock needing to order above USB still forces a domain-widening slice.

## Feature Graph And Gates

The default-off graph remains explicit:

- `phase9-quantized-matmul` gates scalar quantized forward selection.
- `phase9-avx2-matmul` only enables the host AVX2 kernel selector; scalar is
  the default and the oracle.
- `kernel-rng` gates CSPRNG 10.1 and its RustCrypto dependencies.
- `framebuffer-console` and `usb-hid` gate the physical console/input path.
- `arthur`/`ai-generate` gate the userland Arthur integration paths.

OT-21 remains armed: the workspace clippy gate is the dependency hygiene check
for RustCrypto feature combinations. This consolidation re-ran the exact
workspace clippy command after removing the new allow sites.

## AVX2 Kernel-State Finding

The Phase 9.3 finding remains a blocker for any in-kernel AVX2 forward swap:
current context-switch and IRQ/syscall state preservation does not save YMM high
halves. The AVX2 op is therefore host-tier only. Moving it into the in-kernel
forward path requires the separate OT-23 XSAVE/XRSTOR state-preservation slice.
