# PM-DISPOSITION: Phase 4 BUILD-2 acceptance + perf-proxy recurrence escalation

**Date:** 2026-05-29
**From:** PM (Cerebro)
**To:** Project record / PL (flagged)
**References:**
- Plans/RETURN-2026-05-29-phase-4-build-2-jarvis.md
- Plans/BLOCKER-2026-05-29-phase-4-build-2d-smp4-full-sweep-phase2-perf-proxy.md
- Plans/PL-RATIFICATION-2026-05-29-phase-4-build-1b-close-isc-revision.md (separate-lane precedent)

---

## Decision: BUILD-2 ACCEPTED

All seven BUILD-2 ISCs are satisfied. Merge to origin/main authorized.

| ISC | Result | Note |
|-----|--------|------|
| 2-1 | ✓ PASS | SLAAC state machine compiles (RFC 4862 + 8981) |
| 2-2 | ✓ PASS | RA parsing: PIO/RIO extraction |
| 2-3 | ✓ PASS | UDP loopback send/recv (sendto/recvfrom) |
| 2-4 | ✓ PASS | SLAAC-acquired IPv6 source via injected RA + virtio-net egress capture |
| 2-5 | ✓ PASS | phase4_slaac_udp_ipv6_loopback passes SMOKE and NEO_SMP=4 |
| 2-6 | ✓ PASS | sys_net_stats (0x185) per-socket counters |
| 2-7 | ✓ PASS | New-code panic/unwrap scan clean |

**On ISC-2-7:** Jarvis conservatively marked this BLOCKED because the BUILD-2d dispatch step (`cargo neo-test all`) did not pass. But ISC-2-7 as written is "no unwrap/panic in new IPv6/UDP code," and the new-code scan is clean. ISC-2-7 is satisfied. The full-sweep instruction in the BUILD-2d sub-commit was over-specified relative to the ratified ISC gate — see separate-lane ruling below.

**ABI conformance:** Implementation matches PM-DISPOSITION-2026-05-29 (send-destination ABI):
- 0x182 SEND = connected-send, returns ENOTCONN (correct)
- 0x186 SENDTO / 0x187 RECVFROM = destination-aware (correct)
- 0x185 sys_net_stats (correct)
- RA injection path kernel-test-only, scoped to authorized ISC path (correct)

**In-scope correctness fix accepted:** Jarvis added `Handle::Socket` to the Q8C "not channel" match arm, repairing q8c_blocked_kill_foreign_parked and q8c_close_peer_my_side_extract. This is a direct and correct consequence of socket handles existing in the handle table (since BUILD-1b); without it those Q8C tests mis-handle socket handles. Legitimate, in-scope, accepted.

---

## Separate-lane ruling (applies BUILD-1b precedent)

The full `cargo neo-test all --no-meta` NEO_SMP=4 sweep fails at `phase2_perf_proxy_baseline`. Per the PL-ratified BUILD-1b precedent (Option 2: explicit benchmarks/perf-proxies are a separate stability lane, not feature-BUILD gates), this does NOT block BUILD-2 closure. Functional scenario validation (ISC-2-1 through 2-7) is the gate, and it is green.

`phase2_perf_proxy_baseline` is a scheduler-behavior proxy (counts context switches, scheduling decisions, CR3 reloads for a synthetic worker), not a network-functional test. The BUILD-2 network code does not touch the scheduler's switch/CR3 accounting.

---

## ESCALATION: perf-proxy recurrence is now a tracked root-cause item

This is the SECOND consecutive BUILD where the full sweep is blocked by a Phase-2 perf-proxy:
- BUILD-1b: `irqsafe_mutex_bench` (overhead_ns drift)
- BUILD-2: `phase2_perf_proxy_baseline` (context_switches 3->5, scheduling_decisions 3->5, cr3_performed 2->4)

Two recurrences with structured drift change the posture from "defer" to "investigate." The separate-lane ruling unblocks BUILD closes; it does not license ignoring a real signal indefinitely.

### Leading hypothesis (PM)

The drift is a near-uniform +2 offset (5/5/4 vs expected 3/3/2; cr3_skipped, alloc, dealloc all still match). That signature is consistent with ONE extra runnable kernel entity participating in the boot/early-scheduler path a bounded number of times — NOT a runaway or busy-spin (which would manifest as timeouts/hangs, not a clean bounded offset).

`drivers::net::init()` runs unconditionally at boot (kernel/src/main.rs:411). The Phase-2 perf-proxy expected constants (PERF_PROXY_EXPECTED_CONTEXT_SWITCHES=3, etc. in test_harness.rs) were calibrated at Phase 2 — before any network subsystem existed. The most probable root cause is therefore:

**(A) Baseline staleness (likely):** the network stack's boot-time init legitimately adds a small, bounded amount of scheduler activity, and the Phase-2 proxy baselines simply need recalibration to a post-network reference. This is a test-fixture fix, not a kernel fix.

**(B) Busy net entity (less likely):** net::init() leaves a kernel task runnable that participates in scheduling more than it should. This would be a real kernel fix.

(A) is far more probable given the clean bounded offset and the passing functional/SMP scenarios. But the investigation must confirm (A) vs (B) empirically before recalibrating — recalibrating over a real (B) bug would mask it.

### Action: open investigation before Phase 5

Open a dedicated stability investigation (Cerebro diagnostic or Jarvis dispatch) with these steps:
1. Bisect: does the drift appear at BUILD-1a (net substrate landing), BUILD-1b, or BUILD-2? Run phase2_perf_proxy_baseline at each commit. Pinpoints whether net::init() is the cause.
2. Characterize: with net::init() stubbed out, does the proxy return to 3/3/2? Confirms hypothesis (A)/(B).
3. If (A): recalibrate the perf-proxy baselines to a post-network reference, document the new expected values + rationale in test_harness.rs, and note the network-init cost in the perf ledger. The separate-lane proxies should then pass again.
4. If (B): file a kernel fix dispatch for the busy net entity.

This investigation is a prerequisite for re-enabling the full-sweep gate, and should close before Phase 5 (AI subsystem) opens, so we enter Phase 5 with a green full sweep.

---

## Status

- **BUILD-2:** CLOSED (functional ISCs green; separate-lane perf-proxy does not gate)
- **Merge:** authorized to origin/main
- **PL flag 1:** sendto/recvfrom ABI (0x186/0x187) ratification — still pending PL sign-off at this BUILD-2 close (carried from prior disposition)
- **PL flag 2:** perf-proxy recurrence — PM is opening a root-cause investigation; PL informed. No PL action required unless PL wants to reprioritize.
- **New tracked item:** PERF-PROXY-BASELINE investigation (pre-Phase-5 prerequisite)

---

**BUILD-2 accepted. Merging to main.**