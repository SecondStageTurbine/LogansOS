# RETURN: Phase 4 BUILD-2 - SLAAC IPv6 + UDP Transport Stack

**Date:** 2026-05-29
**From:** Jarvis
**Branch:** `codex/phase-4-build-2-jarvis`
**Status:** BLOCKED on BUILD-2d broad SMP=4 sweep; BUILD-2 feature path passes targeted gates.

## Sub-Commits

- BUILD-2a: `6215d5a` - `build-2a: ipv6 address state machine + ra parsing`
- BUILD-2b: `b29b615` - `build-2b: udp sendto recvfrom and loopback accounting`
- BUILD-2c: `a448036` - `build-2c: net stats and slaac udp smoke`
- BUILD-2d: pending; broad `NEO_SMP=4` sweep blocked by `phase2_perf_proxy_baseline`

## ISC Results

- ISC-2-1: PASS - SLAAC state machine compiles.
- ISC-2-2: PASS - RA parsing handles PIO/RIO extraction.
- ISC-2-3: PASS - UDP loopback send/recv covered by `phase4_socket_udp_loopback`.
- ISC-2-4: PASS - SLAAC-acquired IPv6 source verified by injected RA + UDP egress capture.
- ISC-2-5: PASS - `phase4_slaac_udp_ipv6_loopback` QEMU smoke passes.
- ISC-2-6: PASS - `sys_net_stats` returns per-socket packet/byte counts in the SLAAC smoke.
- ISC-2-7: BLOCKED - new-code panic/unwrap scan is clean, but full `NEO_SMP=4` sweep fails in `phase2_perf_proxy_baseline`.

## Validation

Host checks:

```text
cargo check -p neoplutarch-kernel --features neo-test-phase4-slaac-udp-ipv6-loopback
cargo check -p neo-init --features phase4-slaac-udp-ipv6-loopback
cargo check -p neo-worker --features phase4-slaac-udp-ipv6-loopback
```

Result: PASS. Existing warning noise remains; no warning escalation observed.

QEMU smoke:

```text
docker compose run --rm -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test phase4_slaac_udp_ipv6_loopback --no-meta
```

Result: PASS.

SMP=4 QEMU smoke:

```text
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test phase4_slaac_udp_ipv6_loopback --no-meta
```

Result: PASS.

Adjacent broad-sweep repair checks:

```text
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test q8c_blocked_kill_foreign_parked --no-meta
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test q8c_close_peer_my_side_extract --no-meta
```

Result: PASS after adding `Handle::Socket` to the Q8C "not channel" match arm.

Full sweep:

```text
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test all --no-meta
```

Result: FAIL in `phase2_perf_proxy_baseline`.

Failure evidence:

```text
NEO_TEST/1 event=FAIL name=phase2_perf_proxy_baseline reason=perf_proxy_baseline_drift context_switches=5 expected_context_switches=3 scheduling_decisions=5 expected_scheduling_decisions=3 cr3_performed=4 expected_cr3_performed=2 cr3_skipped=1 expected_cr3_skipped=1 alloc_calls=1 expected_alloc_calls=1 dealloc_calls=1
```

See blocker artifact:

```text
Plans/BLOCKER-2026-05-29-phase-4-build-2d-smp4-full-sweep-phase2-perf-proxy.md
```

## Code Review Notes

- ABI follows PM disposition: `0x182` remains connected send and returns `ENOTCONN`; `0x186`/`0x187` implement destination-aware send/recv.
- `0x185` exposes `sys_net_stats`.
- RA injection path is kernel-test-only and scoped to the authorized BUILD-2 ISC path.
- New-code panic/unwrap scan did not find new `unwrap`, `expect`, or `panic!` usage in the SLAAC/socket/syscall paths. Existing hits remain in syscall setup and unrelated slab harness code.
