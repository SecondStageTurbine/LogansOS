<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-062: Rule-4 consolidation after OT-23, I-15, and Arthur E1/E2/E3

**Status:** Accepted
**Date:** 2026-06-20
**Dispatch:** `Plans/DISPATCH-2026-06-20-rule4-consolidation-jarvis.md`
**Scope:** no-feature consolidation since ADR-059; behavior-preserving only.
No new invariant, no new unsafe, no scenario change.

## Decision

Run the overdue Rule-4 consolidation over the post-ADR-059 feature run:

1. CSPRNG 10.3 (`SYS_GETRANDOM`) and the later M2 crypto-design dependency.
2. OT-18 E1a/E2/E3 Arthur grant-store slices.
3. OT-24 per-CPU frame allocator guard.
4. I-15 / ADR-061 file-capability unification.
5. q8_K and Q4_K AVX2 host-tier kernels.
6. W2 discriminator-bank THINK.
7. OT-23 XSAVE/YMM preservation.
8. Arthur memory M1 and the M2 build dispatch.

The consolidation changes no runtime behavior. It removes one avoidable
`grant_store.rs` helper fork, reconciles stale pointers, re-measures the ledger,
and prunes the open-thread register.

## Code Consolidation

`userland/libneo/src/grant_store.rs` was the only code edited. The E2 writer and
E3-audit renderer had parallel no-alloc output helpers:

- E2: `push` + `push_u32_decimal`.
- E3: `push_bytes` + `push_u64_decimal` + `push_u32_decimal_audit`.

Those are now one private helper pair, `push_bytes` and `push_decimal`, gated by
`any(feature = "arthur-grant-rerequest", feature = "arthur-grant-audit")`. The
feature discriminators remain on the consuming surfaces:

- `serialize_grants` still exists only under `arthur-grant-rerequest`.
- grant listing and the action log still exist only under `arthur-grant-audit`.
- the `grants.json` byte oracle and human-readable audit output are unchanged.

The stale E3 revoke seam comment was also corrected: E3 did not build revoke;
ADR-061 now unblocks a later revoke slice, but this module still only names the
seam.

The scalar and AVX2 kernels were re-inspected and left cohesive:

- `qk_matmul.rs` keeps scalar and AVX2 Q4_K x Q8_K as separate execution forms.
- `ai/ops/mod.rs` keeps scalar and AVX2 Q4_K dequantize as separate kernels.
- The shared byte/layout logic stays in the Q4_K helpers and KATs; abstracting
  lane-local SIMD code would add indirection without deleting a second source of
  truth.

No kernel source was edited by this ADR.

## Document Reconciliation

- ADR-057 and ADR-058 now describe Arthur M2 as an AEAD/ChaCha20-Poly1305
  consumer of `sys_getrandom`, matching the ratified M2 THINK instead of the
  older AES-256-GCM wording.
- ADR-060 now states E3-revoke is post-ADR-061 unblocked but still deferred.
- ADR-061 remains the I-15 authority record.
- `Plans/THINK-2026-06-20-arthur-memory-m2-encryption-design.md` is the M2
  build contract: ChaCha20-Poly1305, write-ahead nonce reservation, and the
  full-snapshot-rollback residual named for the build.
- `Plans/THINK-2026-06-20-w2-mt7601u-discriminator-bank-and-fault-containment.md`
  is the W2 discriminator contract: channel triple first, every blind W2 run
  conclusive on failure, and I-14 containment already satisfied for the USB
  path.
- `README.md`, `CLAUDE.md`, `Plans/INDEX.md`, `docs/OPEN-THREADS.md`, and
  `docs/complexity-governance.md` were refreshed as navigation pointers.

The ADR number sequence is intentionally not renumbered. There are two shipped
ADR-058 records; this consolidation leaves them stable and adds ADR-062 as the
successor to ADR-059.

## Ledger Audit

No row is added. No unnamed invariant crept in.

| Row | Verdict |
|---|---|
| I-1 result-invariance | Preserved. This branch only factors userland formatting helpers and updates docs; the grant writer byte oracle remains the equality proof. |
| I-2 kill-switch/strict superset | Preserved. `arthur-grant-rerequest` and `arthur-grant-audit` remain default-off, and the helper consolidation is private to those feature-gated surfaces. |
| I-3 single-mutator relax-and-reprove | Untouched. |
| I-4 refcount/unique-owner-at-rest | Untouched. |
| I-5 lock order | Preserved. No lock code changed; 13 ordered call sites, max lockdep level 31. |
| I-6 accounting exactness | Untouched. |
| I-7 residency capacity | Untouched. |
| I-8 anti-thrash | Untouched. |
| I-9 pin protection | Untouched. |
| I-10 temporal isolation | Untouched. |
| I-11 no lost wakeup | Untouched. |
| I-12 byte-layout oracle | Preserved. The E2 writer still matches the hand-laid `grants.json` oracle; CSPRNG, file-cap, q8/Q4_K, and M2/W2 exact-byte disciplines remain named. |
| I-13 stale handles fail safe | Untouched. |
| I-14 subsystem fault containment | Preserved. W2's USB path is xHCI-poll-driven and DMA-fenced/IOMMU-contained per the W2 THINK; this branch does not touch driver code. |
| I-15 file handles are first-class caps | Preserved. ADR-061 remains the authority; this branch updates dependent docs/comments only. |

**Page-fit flag:** the ledger remains one enumerated I-1..I-15 table, but the
literal Markdown section is over a printed page because I-14/I-15 carry long
guard prose. No new row was added here. Before admitting I-16, compress the row
prose or split evidence details out so the "one page" alarm keeps its bite.

## Re-Measured Ledger

Method matches ADR-059: count Rust `static` items under `kernel/src` whose type
is `SwitchMutex`, `IrqSafeMutex`, `spin::Mutex`, or `Lazy<...Mutex...>`,
excluding `kernel/src/test_harness.rs` for the production count. Code LOC counts
exclude blank lines, `//`/`//!`/`///` lines, and block comments.

| Measure | Current |
|---|---:|
| Production lock statics | 67 |
| Test-harness lock statics | 17 |
| Total lock statics | 84 |
| `SwitchMutex::new_ordered` call sites | 13 |
| Max lockdep level | 31 (`USB_CORE`) |
| AI top-level pure LOC / total LOC | 3,461 / 7,010 = 49.4% |
| AI pure plus `ai/ops` LOC / total with `ai/ops` | 5,724 / 9,273 = 61.7% |

The ordered band still fits inside the existing `1..=31` lockdep domain. The
ceiling remains `USB_CORE=31`; a future lock that must order above USB still
requires an explicit domain-widening slice.

## Open-Thread Register

OT-23, OT-24, and OT-25 are pruned from active status:

- OT-23 closed 2026-06-20 at `0e1917e`: XSAVE/XRSTOR preserves YMM state at the
  scheduler/syscall/timer boundaries; 4-shard QEMU run `27878169059` green.
- OT-24 closed 2026-06-19 at `3724ad1`: the debug frame-allocation reentry guard
  is per-CPU; push CI `27810936999` and 4-shard QEMU `27813496137` green.
- OT-25 closed 2026-06-20 at `2eb28b9`: file caps are first-class capability
  graph nodes; the `file_cap_derive_grant` QEMU scenario is green.

Remaining active/register items keep their boundaries:

- OT-2 remains open for the W2 SNonce swap; 10.3 unblocks Arthur M2 but does not
  close W2.
- OT-17 M2 is build-ready after M1 and 10.3; the rollback residual must be named
  in the build.
- OT-18 E1b/E3-revoke are unblocked by I-15 but not silently folded into this
  consolidation.

## Gate

This branch touches docs and userland only. It does not edit `kernel/`, `boot/`,
or `tools/xtask/`, so the manual 4-shard QEMU dispatch is not required by the
kernel-touch rule. Equality is proven by the same host gate:

- `cargo fmt --all --check`
- `cargo test --workspace --exclude neoplutarch-kernel`
- `cargo clippy --workspace --all-targets -- -D warnings`
- blocking kernel clippy
- changed-file SPDX check

Feature-on Arthur grant tests are also run explicitly for the consolidated
helper paths.
