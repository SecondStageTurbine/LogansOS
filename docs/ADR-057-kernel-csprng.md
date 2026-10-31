<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-057: Kernel CSPRNG 10.1 entropy and DRBG core

**Status:** Accepted
**Date:** 2026-06-17
**Dispatch:** `Plans/DISPATCH-2026-06-17-kernel-csprng-10.1-entropy-drbg-prometheus.md`
**Design:** `Plans/THINK-2026-06-17-kernel-csprng-entropy-design.md`
**Scope:** `kernel-rng` default-off entropy source, ChaCha20 DRBG, and KATs only; no `fill_random` consumer API, no W2 SNonce swap, no `SYS_GETRANDOM`, no RDSEED.

## Decision

Add the first kernel CSPRNG slice behind the default-off `kernel-rng`
feature:

- Entropy: RDRAND with the ratified ten-retry fail-closed protocol, mixed
  with timer jitter. Jitter is never a trusted sole seed.
- DRBG: ChaCha20 keyed from a SHA-256 seed mix, using RustCrypto primitives
  rather than hand-rolled crypto.
- Reseed guards: reseed by call count and counter-wrap guard, with reseed
  material mixed through SHA-256 before rekeying.
- KATs: published-anchor ChaCha20 and SHA-256 component tests plus a golden
  composition self-test for the DRBG wiring.

With `kernel-rng` off, the RNG module and crypto dependencies are absent from
the kernel build. The build therefore preserves the Rule-1 kill-switch shape:
the default kernel remains the pre-CSPRNG kernel.

## Safety And Unsafe

The only new unsafe in this slice is the x86 entropy/timing intrinsic surface:
RDRAND and the timing reads used for jitter. The code requires RDRAND success
before seeding the real DRBG path and panics if the hardware source fails the
retry policy. The QEMU `neo-test` path uses a fixed seed because TCG does not
provide the real entropy surface.

No AI-governance lock is touched. No new invariant is added to the complexity
ledger. The CSPRNG is security infrastructure for later consumers, not a new
AI execution path.

## KAT Discipline

The test contract is I-12:

- ChaCha20 component behavior is checked against the RFC 8439 stream vector.
- SHA-256 component behavior is checked against the FIPS 180-4 empty-message
  digest.
- The DRBG composition uses a fixed seed and records deterministic output so
  future refactors cannot change the wiring silently.

The entropy source itself is random and is not KAT-able. Its correctness is the
fail-closed hardware protocol plus the deterministic fixed-seed path used by
the QEMU self-test.

## Follow-Ons

This ADR does not close OT-2. The CSPRNG exists, but the security boundary is
not live until later slices consume it:

- 10.2: add the kernel-internal `fill_random` API and swap W2's counter SNonce
  to CSPRNG output.
- **10.3: COMPLETED** — userland `SYS_GETRANDOM` surface + libneo wrapper landed
  (`prometheus/csprng-10.3-sys-getrandom`). See ADR-058.

Arthur userland crypto depends on the userland surface, not merely the 10.1
DRBG core. **OT-17 M2 is now unblocked** — Arthur memory encryption can generate
per-category AEAD keys via `sys_getrandom` (the later M2 design ratifies
ChaCha20-Poly1305 for the record format).
