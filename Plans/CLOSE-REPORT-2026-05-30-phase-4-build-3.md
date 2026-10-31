# CLOSE-REPORT: Phase 4 BUILD-3 — IPv4 + ARP + DHCPv4 + dual-stack (RFC 6724)

**Date:** 2026-05-30
**From:** PM (Cerebro)
**Status:** ACCEPTED — ready for ff-only merge to main
**References:**
- Plans/PM-DECISION-2026-05-30-phase-4-build-3-opening.md (Q1–Q5, §4.5, ISCs)
- Plans/PLAN-2026-05-30-phase-4-build-3-ipv4-dhcpv4-dual-stack.md
- Plans/DISPATCH-2026-05-30-phase-4-build-3-ipv4-dhcpv4-jarvis.md
- Plans/RETURN-2026-05-30-phase-4-build-3-jarvis.md (Jarvis)
- Plans/BUILD-3f-BARE-METAL-DEFERRAL-2026-05-30.md (Jarvis)

---

## Summary

Phase 4 BUILD-3 (IPv4 + ARP + DHCPv4 + dual-stack source-address selection) is
**accepted on all functional ISCs**. Reviewed by PM against the branch
`origin/codex/phase-4-build-3-ipv4-dhcpv4-jarvis` @ `5d1edcd` (six commits
`4a40e43`…`5d1edcd` atop main `d1ba0e8`).

**Goal milestone met:** UDP over IPv4 to a remote IPv4 host with a
DHCPv4-acquired source and ARP-resolved next-hop MAC; correctly-framed
Ethernet+IPv4+UDP datagram egresses the virtio-net TX ring.

## Sub-commits

| Commit | Sub-commit |
|--------|-----------|
| `4a40e43` | build-3a: ipv4 header + arp cache (rfc 826) |
| `f60d239` | build-3b: dhcpv4 state machine + lease timers (rfc 2131/2132) |
| `3365eed` | build-3c: dual-family interface + rfc 6724 source selection |
| `218421e` | build-3d: send_to source-selection hook + ipv4 loopback |
| `19e466d` | build-3e: virtio-net ipv4/udp wire tx + arp resolution |
| `5d1edcd` | build-3f: build-3 sweep + bare-metal deferral marker |

## ISC results (21/21)

All 21 ISCs (3a-1…3f-3) reported PASS in the RETURN. 3a–3d independently
re-verified by PM via source read of the branch; 3e–3f accepted on Jarvis's
QEMU sweep (PM environment has no QEMU).

## PM independent verification (against the constraints I dispatched)

| Check | Result | Evidence |
|-------|--------|----------|
| **Q1/Q2 — no smoltcp iface adoption** | ✓ PASS | `kernel/Cargo.toml:47` smoltcp features unchanged (`medium-ethernet, proto-ipv6, socket-raw`); no `smoltcp::iface`/`Interface` anywhere in `net/`. IPv4 fully hand-rolled in `ipv4.rs`/`arp.rs`/`dhcpv4.rs`. |
| **§4.5 — bounded restructuring** | ✓ PASS (under-shot) | Came out *more* additive than predicted: `ipv6.rs` `INTERFACES`/`tick`/`preferred_global_address` unchanged; a parallel `ipv4::INTERFACES` + thin `interface.rs` + a `net::tick()` fan-out (`mod.rs:15–19`). No `ipv6.rs` churn beyond mod wiring. |
| **InterfaceAddress not re-typed** | ✓ PASS | `ipv6.rs:39` still `pub struct InterfaceAddress`; per-family tables separate. |
| **No new syscall ABI** | ✓ PASS | `syscall.rs` not in the diff; existing `send_to` surface reused. |
| **Panic/unwrap/expect scan over 4 new modules** | ✓ PASS | PM-run `git grep` over `ipv4.rs`/`arp.rs`/`dhcpv4.rs`/`interface.rs`: no matches. |
| **IPv4 header checksum** | ✓ PASS | computed `ipv4.rs:116`, verified-on-parse `:137` (ones-complement). |
| **send_to hook correctness** | ✓ PASS | wildcard→`select_source`, explicit-bind bypass, cross-family reject preserved (`socket.rs:178`), real `transmit_ipv4_udp` for non-loopback IPv4. |
| **DHCPv4 state machine** | ✓ PASS | `Init→Selecting→Requesting→Bound→Renewing→Rebinding` + ACD stub + T1/T2 + NAK→Init + RFC 2132 option parse. |

## Findings carried forward (non-blocking forward-markers)

1. **FM-3-1 (RFC 6724 depth):** `select_source` is **family-directed** —
   IPv4 dst → IPv4 source, IPv6 dst → IPv6 source — relying on the per-family
   tables' state filtering. This is correct and sufficient for the BUILD-3
   goal milestone. RFC 6724 Rule 1/2 longest-prefix tie-breaking among
   multiple same-family globals, and the **prefer-IPv6 _destination_ default**
   (which belongs to destination selection, not source selection), are
   deferred to **BUILD-5** (DNS + Happy Eyeballs). ISC-3c-2 is satisfied in the
   degenerate sense (dst family is fixed at `send_to`, so there is no
   cross-family arbitration to perform at this layer).
2. **FM-3-2 (zero UDP checksum):** IPv4 UDP egress emits a zero UDP checksum
   (`ipv4.rs:205`) — valid per RFC 768 for IPv4. Acceptable at the egress
   floor. Real receivers that require a checksum, and IPv6 (where the UDP
   checksum is mandatory), are a **BUILD-3f bare-metal** / future concern.
3. **FM-3-3 (tick driver):** `net::tick()` is driven by the test harness
   (`test_harness.rs:11889`), not yet wired into the periodic timer ISR
   (`sched::on_tick`). Automatic DHCP lease renewal and ARP aging therefore do
   not fire autonomously in production yet — identical posture to BUILD-2's
   harness-driven `ipv6::tick`. Wiring `net::tick` into the timer is tracked
   forward (bare-metal lane).

None of these gate BUILD-3; all are recorded for BUILD-3f/BUILD-5.

## Separate-lane note

Unlike BUILD-1b and BUILD-2, the full `cargo neo-test all` NEO_SMP=4 sweep is
**fully green at ATTEMPTS=1** with no perf-proxy block — the BUILD-2-era
perf-proxy SMP-gate (`a4d37f2`) and the syswait race fix (`c0a3b04`) held
through BUILD-3. No separate-lane carve-out was needed this build.

## Phase 4 progress

| BUILD | Content | Status |
|-------|---------|--------|
| BUILD-1 | virtio-net + UDP socket syscalls + loopback | ✓ CLOSED |
| BUILD-2 | IPv6 SLAAC + sendto/recvfrom + net stats | ✓ CLOSED |
| BUILD-3 | IPv4 + ARP + DHCPv4 + dual-stack (RFC 6724) | ✓ **CLOSED (this report)** |
| BUILD-3f | bare-metal live DHCP/ARP/UDP-reply/RFC 5227 (deferred) | 📋 TRACKED |
| BUILD-4 | TCP + CUBIC + RFC 5961 | 📋 NEXT |
| BUILD-5 | DNS + Happy Eyeballs + kTLS (= Phase 4 ship) | 📋 PLANNED |

## Open items carried forward

1. **FM-3-1/3-2/3-3** — forward-markers above (BUILD-3f / BUILD-5).
2. **BUILD-3f bare-metal** — real lease/ARP/reply/RFC 5227 on wire (tracked,
   `BUILD-3f-BARE-METAL-DEFERRAL-2026-05-30.md`).
3. **Perf-proxy baseline re-calibration study** — verdict-A follow-up,
   pre-Phase-5 prerequisite (unchanged; not a BUILD-4 blocker).
4. **ANSPEC-002 §17/§18 canonical integration** — pending (unchanged).

## Merge

ff-only merge of `codex/phase-4-build-3-ipv4-dhcpv4-jarvis` (`5d1edcd`) to
main authorized. Merge is performed from the PM/main seat (this PM session is a
remote seat without main-push rights). See the merge recipe handed to the PL.

---

**BUILD-3 is CLOSED. Next: BUILD-4 (TCP + CUBIC + RFC 5961).**
