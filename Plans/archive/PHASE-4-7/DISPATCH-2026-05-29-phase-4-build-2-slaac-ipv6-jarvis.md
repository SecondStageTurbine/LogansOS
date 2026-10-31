# DISPATCH: Phase 4 BUILD-2 — IPv6 SLAAC + UDP transport stack

**Date:** 2026-05-29
**From:** PM (Cerebro)
**To:** Jarvis
**Repo:** E:\AI Automation\AltimitNeo (GitHub: SecondStageTurbine/NeoPlutarch)
**Branch:** `codex/phase-4-build-2-jarvis` (create from origin/main at e4963c3)
**Author:** SecondStageTurbine

---

## Your assignment

Implement Phase 4 BUILD-2: IPv6 address autoconfig via SLAAC + full UDP send/recv with kernel-managed packet dispatch.

**Goal milestone:** A QEMU userspace process acquires an IPv6 address automatically (via SLAAC), opens a UDP socket, binds to the acquired address, and exchanges datagrams with a simulated remote host.

---

## Before you start

1. **Fetch latest origin/main:**
   ```
   git fetch origin
   ```

2. **Create and check out the feature branch:**
   ```
   git checkout -b codex/phase-4-build-2-jarvis origin/main
   ```

3. **Read the PLAN (your specification):**
   ```
   Plans/PLAN-2026-05-29-phase-4-build-2-slaac-ipv6.md
   ```

4. **Understand BUILD-1 baseline** (already on main):
   - `kernel/src/drivers/net/virtio_net.rs` — virtio-net driver
   - `kernel/src/net/smoltcp_wrap.rs` — smoltcp integration layer
   - `kernel/src/net/socket.rs` — socket handle management (from BUILD-1b)
   - `kernel/src/arch/x86_64/syscall.rs` — syscall dispatch (0x180–0x184)

---

## Sub-commits (4 total)

Follow the sequencing in the PLAN exactly. Each sub-commit should compile and pass its ISCs.

### BUILD-2a: SLAAC state machine + RA processing
**Files to create/amend:**
- `kernel/src/net/ipv6.rs` (NEW) — Address table, SLAAC state machine, DAD coroutine
- `kernel/src/net/icmpv6.rs` (NEW or extend) — RA handler, option parsing
- `kernel/src/sched/process.rs` (AMEND) — per-process IPv6 address list

**ISC target:** ISC-2-1, ISC-2-2

**Commit message:** `build-2a: ipv6 address state machine + ra parsing (slaac, rfc4862, rfc8981)`

### BUILD-2b: UDP send/recv + loopback short-circuit
**Files to amend:**
- `kernel/src/net/socket.rs` — UDP send path, RX queue per socket
- `kernel/src/net/udp.rs` (NEW or extend) — RX queue, notification on arrival
- `kernel/src/net/device.rs` — loopback short-circuit trigger

**ISC target:** ISC-2-3, ISC-2-6

**Commit message:** `build-2b: udp send/recv + loopback short-circuit + per-socket accounting`

### BUILD-2c: QEMU SLAAC integration + sys_net_stats syscall
**Files to amend:**
- `kernel/src/arch/x86_64/syscall.rs` — Add syscall 0x185 (sys_net_stats)
- `userland/libneo/src/lib.rs` — Add wrapper for sys_net_stats
- `userland/neo-init/src/main.rs` — phase4_slaac_udp_ipv6_loopback test scenario
- `tools/xtask/src/main.rs` — Integrate scenario into harness

**ISC target:** ISC-2-4, ISC-2-5, ISC-2-6

**Commit message:** `build-2c: sys_net_stats syscall + qemu slaac integration test`

### BUILD-2d: Full sweep + code quality
- Run `cargo neo-test all` under NEO_SMP=4
- No panics, unwraps in new code
- Code review + polish

**ISC target:** ISC-2-7

**Commit message:** `build-2d: full sweep smp=4 + code quality pass`

---

## ISC checklist (from PLAN)

- [ ] **ISC-2-1:** SLAAC state machine compiles (RFC 4862 + Privacy Extensions RFC 8981)
- [ ] **ISC-2-2:** RA parsing works (RFC 4861: PIO, RIO extraction)
- [ ] **ISC-2-3:** UDP loopback send/recv (process-to-process on ::1:port)
- [ ] **ISC-2-4:** UDP on SLAAC-acquired address (simulated remote in QEMU)
- [ ] **ISC-2-5:** QEMU smoke test (phase4_slaac_udp_ipv6_loopback) passes
- [ ] **ISC-2-6:** sys_net_stats syscall returns packet counts
- [ ] **ISC-2-7:** No unwrap/panic in new code

---

## Key design points (from PLAN)

1. **SLAAC-only at BUILD-2:** IPv4 + DHCPv4 are BUILD-3+. No IPv4 code needed.
2. **RFC 8981 Privacy Extensions:** Generate temporary addresses per RFC 8981. Default rotation, no userspace control yet.
3. **Loopback short-circuit:** `::1` traffic never goes to the driver. Direct kernel RX queue delivery.
4. **Notification-on-activity:** Kernel notifies process when socket has RX data available or send capacity available.
5. **QEMU RA setup:** Use dnsmasq or radvd on host to simulate router. RA frequency should be 3–10 sec (default 30 sec).

---

## Risks to monitor (from PLAN)

**R1: SLAAC + DAD edge cases.** Decision: reject UDP on tentative addresses (ISC-2-3 requires bind on preferred address only).

**R2: Temp-address expiration while socket is bound.** Decision: close socket when temp address expires (ISC tests use preferred-lifetime addresses to avoid this).

**R3: QEMU RA simulation quirks.** Use real radvd on host or QEMU's native IPv6 support. Test with actual RA traffic capture.

---

## Failure modes to avoid

1. **Forgetting to initialize address-lifetime timers.** Addresses will never transition from preferred → deprecated. Test with explicit waits on 1-day timers.
2. **UDP send path still returns ENOTSUP.** Make sure BUILD-2b syscall is fully wired (currently a stub from BUILD-1b).
3. **Loopback interception checked before capability validation.** Loopback packets should still respect socket-handle rights; only the delivery path short-circuits.

---

## Testing expectations

- **Host:** `cargo check` must pass
- **Docker:** `docker compose run --rm neo cargo neo-test` must pass NEO_TEST=SMOKE variant first, then NEO_SMP=4 variant
- **Test harness:** New scenario `phase4_slaac_udp_ipv6_loopback` must boot QEMU, acquire address, send/recv, exit cleanly

---

## Return artifact

When done, file:
```
Plans/RETURN-2026-05-29-phase-4-build-2-jarvis.md
```

Include:
- ISC validation results (PASS/FAIL for each ISC-2-1 through ISC-2-7)
- Cargo check output (no warnings escalations)
- QEMU smoke test output (both NEO_TEST=SMOKE and NEO_SMP=4 runs)
- Code review notes (any areas of concern)
- Sub-commit hashes (BUILD-2a, 2b, 2c, 2d)

---

## Dispatch assumptions

- Phase 2 capability infrastructure (handle creation, IPC passing, per-handle rights) is complete and tested
- smoltcp's `Ipv6Socket` or equivalent is available and works in BUILD-1a's integrated layer
- QEMU virtio-net is stable and supports IPv6
- Kernel network subsystem layout (kernel/src/net/) follows BUILD-1a conventions

---

## Escalation path

If you hit a blocker, file BLOCKER-YYYY-MM-DD-description.md in Plans/ and include:
- What you tried
- The error message or unexpected behavior
- Your hypothesis
- What information you need from PM to unblock

Do NOT work around blockers; surface them immediately.

---

**Fetch origin/main, create the feature branch, read the PLAN, and begin BUILD-2a. Target completion: 2026-06-08 (10 days from now).**