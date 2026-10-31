# PM-DISPOSITION: Phase 4 BUILD-2 UDP destination ABI + ISC-2-4 wiring (unblock)

**Date:** 2026-05-29
**From:** PM (Cerebro)
**To:** Jarvis
**Reference:** Plans/BLOCKER-2026-05-29-phase-4-build-2-send-destination-abi-jarvis.md
**Status:** BLOCKER RESOLVED — implementation may resume

---

## Acknowledgment

Blocker correctly raised. The BUILD-2 PLAN's ISC-2-3 wrote `sys_socket_send(handle, packet, [::1]:5556)` as if 0x182 already carried a destination — it does not. This is a PLAN defect (under-specified ABI), not a new scope item: the ratified PLAN already requires destination-bearing UDP sends. PM owns correcting this. No PL block; flagged for PL ratification at BUILD-2 close.

You followed dispatch discipline exactly — no workaround applied. Good.

---

## Decision 1: UDP destination ABI

**Chosen: Jarvis Option 2 (add new syscalls), extended with recvfrom.** Do NOT mutate the 0x182/0x183 signatures.

### New syscalls

**0x186 — `SYS_SOCKET_SENDTO`**
```
sys_socket_sendto(h: u32, buf: *const u8, len: usize, flags: u32, addr: *const u8, addr_len: usize) -> i64
```
- Sends a datagram to the explicit destination sockaddr.
- `addr` points to a sockaddr (the same IPv4/IPv6 sockaddr layout that `sys_socket_bind` already accepts at 0x181 — reuse that struct, do not invent a new one).
- This is the PRIMARY UDP send path for BUILD-2 (ISC-2-3, ISC-2-4).

**0x187 — `SYS_SOCKET_RECVFROM`**
```
sys_socket_recvfrom(h: u32, buf: *mut u8, len: usize, flags: u32, addr_out: *mut u8, addr_len_io: *mut usize) -> i64
```
- Receives a datagram and writes the source sockaddr into `addr_out`.
- `addr_len_io` is in/out: caller sets buffer capacity, kernel writes actual sockaddr length.
- If `addr_out` is null, behaves like recv (no source reported) — lets a caller opt out cheaply.
- Needed for request/response: the receiver must learn the sender's address to reply.

### Existing syscalls — signatures UNCHANGED

**0x182 `SYS_SOCKET_SEND(h, buf, len, flags)`** — signature frozen. Semantics REFINED:
- Now means "send to the socket's connected peer."
- Since `connect` is not implemented yet, returns **ENOTCONN** until a future build adds connect/default-peer.
- The BUILD-1b self-loopback stub behavior is REMOVED. Rationale: self-loopback was never correct UDP semantics, it was a placeholder, and no consumer depends on it (BUILD-1b merged days ago, all consumers are in-tree and under our control). Removing a stub behavior while preserving the signature is a semantic refinement, not an ABI break.

**0x183 `SYS_SOCKET_RECV(h, buf, len, flags)`** — unchanged signature and behavior (receive without source info; valid for connected-mode or don't-care-source).

### Deferred: `connect`/default-peer

Not in BUILD-2. UDP `connect` is a convenience (sets a default peer so `send` works), not essential — `sendto` covers all BUILD-2 needs. `connect` lands when TCP needs it (BUILD-4) or earlier if a concrete UDP use case demands it. Reserve **0x188** for `SYS_SOCKET_CONNECT` (do not implement; just leave the number unused per the 0x180-0x1FF reservation discipline).

### libneo wrappers

```rust
pub fn sys_socket_sendto(handle: SocketHandle, buf: &[u8], addr: &SockAddr) -> i64
pub fn sys_socket_recvfrom(handle: SocketHandle, buf: &mut [u8], addr_out: &mut SockAddr) -> i64
```
Reuse the existing `SockAddr` / IPv4 sockaddr helper introduced in BUILD-1b for bind. Add the IPv6 sockaddr variant in BUILD-2a (you need it for SLAAC global addresses anyway).

### Updated ISC-2-3 wording (supersedes PLAN text)

ISC-2-3 datagram loopback now reads:
- Process A: `sys_socket_sendto(handle_A, packet, [::1]:5556)`
- Process B: `sys_socket_recvfrom(handle_B, buf, &mut src)` → packet matches sent, `src` == `[::1]:<A's bound port>`

---

## Decision 2: ISC-2-4 RA + remote peer wiring

**Chosen: injected-RA path in QEMU is authorized. Real-network RA defers to bare-metal lane.**

Per THINK R3, the exact RA-simulation mechanism was explicitly deferred to PLAN. Decision now:

1. **RA acquisition (ISC-2-1/2-2/2-4):** Use a **test-harness RA injection path**, not host-side radvd/tap/dnsmasq. Feed a synthetic, well-formed RA frame into the IPv6 stack at a defined kernel test-hook entry point (e.g., a `#[cfg(test_harness)]` injection function on the net device, or a crafted frame delivered through the virtio-net RX path by the harness). This validates the SLAAC state machine + PIO processing + address derivation deterministically, with no flaky external-router dependency in CI.

2. **The packet path for ISC-2-4 MUST be real.** The distinction from ISC-2-3 (loopback) is preserved: ISC-2-4 binds to a SLAAC-derived **global** address and the UDP datagram must traverse the **actual virtio-net TX/RX path** (NOT the `::1` loopback short-circuit). The "remote peer" is the QEMU host side (slirp/hubport) or a second NeoPlutarch instance on a non-loopback address — whichever is simpler to wire in xtask. If neither is readily wireable, a harness-side packet capture confirming a correctly-formed UDP datagram egressed virtio-net with the SLAAC global source address satisfies ISC-2-4 for BUILD-2. Receiving a real response is a stretch goal; egress with correct source address is the floor.

3. **Real RA-on-the-wire + real remote peer** (against PL's home router / a real LAN) is **deferred to a bare-metal verification sub-commit (BUILD-2e)**, mirroring the Phase 3 BUILD-Nb pattern. Not part of the QEMU CI gate. This is where THINK R4 (router quirks) gets exercised.

**Net effect:** the harness gap Jarvis flagged (no radvd/tap/remote peer) is NOT a blocker. The injection path is the sanctioned BUILD-2 mechanism. Add the RA-injection test hook as part of BUILD-2a; add the egress-capture check as part of BUILD-2c.

---

## Amended sub-commit map

- **BUILD-2a:** SLAAC state machine + RA parsing + **RA injection test hook** + IPv6 sockaddr variant. (ISC-2-1, 2-2)
- **BUILD-2b:** `sendto`/`recvfrom` (0x186/0x187) + UDP RX queue + loopback short-circuit + 0x182 ENOTCONN refinement + per-socket accounting. (ISC-2-3, 2-6)
- **BUILD-2c:** `sys_net_stats` (0x185) + QEMU integration test: injected-RA SLAAC acquisition + virtio-net egress capture. (ISC-2-4, 2-5, 2-6)
- **BUILD-2d:** Full sweep NEO_SMP=4 + code quality. (ISC-2-7)
- **BUILD-2e (NEW, deferred):** bare-metal RA + real remote peer verification. Tracked, not blocking BUILD-2 close. Mirrors Phase 3 BUILD-Nb.

---

## Syscall ledger after BUILD-2

| Number | Name | Status |
|--------|------|--------|
| 0x180 | SYS_SOCKET_CREATE | BUILD-1b (shipped) |
| 0x181 | SYS_SOCKET_BIND | BUILD-1b (shipped) |
| 0x182 | SYS_SOCKET_SEND | BUILD-1b sig frozen; BUILD-2 semantics = connected-send (ENOTCONN until connect) |
| 0x183 | SYS_SOCKET_RECV | BUILD-1b (shipped) |
| 0x184 | SYS_SOCKET_CLOSE | BUILD-1b (shipped) |
| 0x185 | SYS_NET_STATS | BUILD-2c (new) |
| 0x186 | SYS_SOCKET_SENDTO | BUILD-2b (new) |
| 0x187 | SYS_SOCKET_RECVFROM | BUILD-2b (new) |
| 0x188 | SYS_SOCKET_CONNECT | RESERVED (not implemented) |

---

## PL ratification flag

This disposition extends the syscall ABI surface (0x186, 0x187) beyond what the BUILD-2 PLAN literally enumerated (it only named 0x185). Because the ratified PLAN already required destination-bearing UDP sends, PM is treating this as a PLAN-defect correction and proceeding. **PL: this is flagged for your ratification at BUILD-2 close.** If PL prefers a different ABI shape (e.g., connect-first), say so and BUILD-2b reworks — but the sendto/recvfrom shape is the BSD-conventional, least-surprising choice and is recommended.

---

## Resume instruction

Jarvis: proceed with BUILD-2a immediately under the amended sub-commit map. The ABI is ratified (PM-tier, PL-flagged). The RA injection path is authorized. Begin.