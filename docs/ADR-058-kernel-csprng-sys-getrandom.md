<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-058: Kernel CSPRNG 10.3 — SYS_GETRANDOM userland surface

**Status:** Accepted
**Date:** 2026-06-18
**Dispatch:** `Plans/DISPATCH-2026-06-18-csprng-10.3-sys-getrandom-prometheus.md`
**Prerequisite:** ADR-057 (10.1 DRBG core)
**Scope:** Global RNG singleton, `fill_random` helper, `SYS_GETRANDOM` syscall with rigorous userland-buffer validation, libneo wrapper, and neo-test KAT scenario. Extends the existing `kernel-rng` feature gate.

## Decision

Expose 10.1's ChaCha20 DRBG to userland via a native `SYS_GETRANDOM(buf, len)` syscall:

- **Global singleton:** `static KERNEL_RNG: Mutex<Option<Csprng>>` in `rng/mod.rs`. Initialized once at boot via `rng::init()`. Replaces the local `_rng` variable that was dropped at end of `_kernel_start`.
- **Fill helper:** `rng::fill_random(&mut [u8]) -> Result<(), i64>` locks the singleton, delegates to `Csprng::fill_bytes()`. Fail-closed: returns `ENOMEM` if the DRBG is uninitialized.
- **Syscall ABI:** `SYS_GETRANDOM = 0x190`, `getrandom(buf_ptr, len) -> isize`. Returns bytes filled on success, negative errno on failure. Native Neo ABI (Article 5.1), not POSIX — `buf, len` only, no flag argument.
- **Buffer validation truth table:**
  - `buf == NULL` -> `EFAULT`
  - `buf` in kernel half -> `EFAULT` (via `validate_user_buf`)
  - `buf + len` overflows u64 -> `EFAULT` (via `validate_user_buf`)
  - `buf + len` crosses `USER_HALF_LIMIT` -> `EFAULT` (via `validate_user_buf`)
  - `len == 0` -> `0` (POSIX-compatible no-op)
  - `len > 65536` -> `EINVAL` (`MAX_GETRANDOM_LEN` cap)
  - DRBG uninitialized -> `ENOMEM` (fail-closed, never zero/weak bytes)
- **Write pattern:** Fill a kernel-side `Vec<u8>` first, then `copy_nonoverlapping` to userland inside `with_user_access`. This ensures we never write partial or weak bytes to userland — the fill either completes fully or the syscall returns an error before any user-write.
- **libneo wrapper:** `sys_getrandom(&mut [u8]) -> i64`. Also updates `neo_getrandom` (the `getrandom` crate custom backend) to route through the syscall instead of bare RDRAND, now that the kernel surface exists.
- **Kill-switch:** When `kernel-rng` is OFF (default), the syscall dispatch falls through to `ENOSYS`. The kernel binary is bit-for-bit identical with pre-10.3 for the syscall number range (0x190 was unassigned).
- **New unsafe:** The validated userland-buffer write (`copy_nonoverlapping` inside `with_user_access`). Documented `/// SAFETY` — the precondition is that `validate_user_buf` confirmed `[buf, buf + len)` lies entirely in the user half. SMAP protects kernel memory outside the `with_user_access` window.

## KAT Discipline (I-12)

The `neo-test-sys-getrandom` QEMU scenario verifies:
1. **Deterministic output:** First 32 bytes from `fill_random` match the known golden sequence produced by `ChaCha20(key=[0x42;32], nonce=[0;12])` at counter=0.
2. **Non-triviality:** A second call produces distinct bytes.
3. **Length-exactness:** A 64-byte fill returns non-zero content.
4. **Buffer-validation truth table:** NULL ptr -> EFAULT, kernel-half ptr -> EFAULT, overflow len -> EFAULT, oversized len -> EINVAL, zero len -> 0.

## Safety And Unsafe

One new `unsafe` block: the userland-buffer write in `sys_getrandom_impl`. The validation (`validate_user_buf` + NULL check + len cap) is the documented precondition. No new invariant. No AI-governance touch.

## Follow-Ons

- **OT-17 M2 unblocked:** Arthur memory encryption can now generate per-category
  ChaCha20-Poly1305 keys via `sys_getrandom` (per the later M2 encryption THINK
  ratification).
- **OT-2 remains open:** The W2 SNonce swap (Cerebro's slice) is independent and still gated on W2 silicon.
