# PM-NOTE — Phase 4 BUILD-1 — Speculative TCP Shape Sketch

**Date:** 2026-05-28
**From:** PM (Cerebro)
**Status:** Hypothesis-validation artifact (not a commitment). No kernel code emitted.
**Discharges:** THINK v0.2 §4.1 v0.2-refinement / R16 mitigation — *"BUILD-1 PLAN sketches `sys_net_socket_tcp` and `sys_net_listen_tcp` speculatively (without committing them) to validate the layered shape composes with TCP's connection-state model."*
**Companion to:** `Plans/PM-DECISION-2026-05-28-phase-4-build-1-opening.md` §4 Q3.

## 1. Purpose

R16 names the load-bearing claim: THINK v0.1 §3.1 asserted the layered-handle shape forward-accommodates UDP and TCP without API-shape revision; the pressure-test demonstrated that Hubris (the cited precedent) has not actually staged from UDP to TCP, leaving the claim unevidenced.

This note validates the claim by structured walk-through rather than by precedent. The walk-through cannot prove the claim — only an actual BUILD-3 implementation can — but it can *surface obstacles* that would otherwise be discovered at BUILD-3 implementation time, at which point the BUILD-1c-shipped UDP syscall surface would already be committed.

If the walk-through surfaces a blocker, BUILD-1 PLAN re-opens. If it surfaces tractable design points, those are recorded as forward markers for BUILD-3 PLAN. If it surfaces nothing, the claim is supported on stronger (not absolute) grounds.

**Outcome of this walk-through:** the layered shape composes with TCP. Three tractable design points are surfaced as forward markers, none rising to "API-shape revision required."

## 2. UDP Shape Recap (What BUILD-1c Ships)

For the comparison:

```
sys_net_socket_udp(NetworkAccess: cap) -> Result<UdpSocket: cap>
sys_net_send_udp  (UdpSocket: cap, dst: SocketAddr, buf: &[u8]) -> Result<usize>
sys_net_recv_udp  (UdpSocket: cap, buf: &mut [u8])              -> Result<(usize, SocketAddr)>
sys_net_close     (Socket: cap)                                  -> Result<()>
sys_net_wait      (Socket: cap, timeout: Option<Duration>)      -> Result<EventMask>
```

Capabilities:
- `NetworkAccess` (outer): granted at process spawn; bears rights for socket-family creation, port-bind range, etc.
- `UdpSocket` (inner): derived from `NetworkAccess` via `sys_net_socket_udp`. Passable between processes via Phase 2 cap-passing IPC. Bears rights for send / recv (per the BUILD-1c Q4 rights bitmap, deferred to dispatch).

Invariants:
- One outer cap per process; inner caps derive from outer.
- Datagrams are atomic: `send_udp` either delivers a whole datagram or fails.
- Notification-on-activity: `wait` returns when any associated socket has activity.

## 3. Speculative TCP Syscall Surface

The minimum TCP shape, sketched in the layered idiom:

```
// Active open (client side)
sys_net_connect_tcp(
    NetworkAccess:   cap,
    candidates:      &[NetCandidate],   // HEv3-compatible candidate list
) -> Result<TcpSocket: cap>

// Passive open (server side)
sys_net_listen_tcp(
    NetworkAccess:   cap,
    bind_spec:       BindSpec,
    backlog:         usize,
) -> Result<TcpListener: cap>

sys_net_accept_tcp(
    TcpListener:     cap,
) -> Result<TcpSocket: cap>             // *** new derived inner cap ***

// I/O
sys_net_send_tcp(TcpSocket: cap, buf: &[u8])     -> Result<usize>   // partial-write: returns bytes accepted
sys_net_recv_tcp(TcpSocket: cap, buf: &mut [u8]) -> Result<usize>   // partial-read: returns bytes available

// Lifecycle
sys_net_shutdown_tcp(TcpSocket: cap, dir: ShutdownDir) -> Result<()>  // dir = Read | Write | Both
sys_net_close (Socket: cap) -> Result<()>                              // shared with UDP

// Per-connection configuration
sys_net_setopt_tcp(TcpSocket: cap, opt: TcpOption, val: TcpOptionValue) -> Result<()>
sys_net_getopt_tcp(TcpSocket: cap, opt: TcpOption) -> Result<TcpOptionValue>

// Wait (shared with UDP)
sys_net_wait(Socket: cap, timeout: Option<Duration>) -> Result<EventMask>
```

This surface is structurally parallel to the UDP one: `NetworkAccess` → inner cap, inner cap is the I/O subject, `wait` is shared, `close` is shared. The novelty is `TcpListener` as a *third* cap type (separate from `TcpSocket`) and `accept_tcp` as a cap-deriving syscall.

## 4. TCP State-Machine Composition Walk-Through (RFC 9293)

Stepping the layered shape through TCP's states and identifying where new shape elements are required:

| TCP state | Cap state | Required shape primitive | UDP analog? |
|-----------|-----------|--------------------------|-------------|
| CLOSED | (no cap) | `NetworkAccess` exists; no inner cap yet | yes |
| LISTEN | `TcpListener` cap held | `sys_net_listen_tcp` (creates) | no — UDP has no listening cap |
| SYN-SENT | `TcpSocket` cap held; conn pending | `sys_net_connect_tcp` (creates; blocks or returns pending) | no — UDP is stateless |
| SYN-RECEIVED | half-open in kernel; no userspace cap | kernel-internal, not exposed | no |
| ESTABLISHED | `TcpSocket` cap held; full I/O | `sys_net_send_tcp` / `sys_net_recv_tcp` | yes (parallel to UDP I/O) |
| ESTABLISHED, half-closed (local FIN sent) | `TcpSocket` cap, WRITE-revoked | `sys_net_shutdown_tcp(Write)` | **no — needs read/write rights independence** |
| ESTABLISHED, half-closed (remote FIN recv) | `TcpSocket` cap, recv returns 0 | EOF signaling on `recv_tcp` | no — UDP has no EOF |
| FIN-WAIT-1/2, CLOSING, LAST-ACK | `TcpSocket` cap held during close handshake | `sys_net_close` initiates; kernel completes async | no |
| TIME-WAIT | **cap dropped, kernel state lingers** | **cap-lifecycle decoupled from protocol-state** | no |
| CLOSED (after TIME-WAIT) | no cap, no kernel state | normal completion | yes |

The `accept` path is the cap-derivation novelty:

```
TcpListener cap  (ready, queue non-empty)
        |
        |  sys_net_accept_tcp(TcpListener)
        v
TcpSocket cap    (derived, child of TcpListener and NetworkAccess)
```

This is *cap derivation from an inner cap*, not from `NetworkAccess` directly. Phase 2 cap-passing IPC supports passing existing caps between processes; whether it supports *deriving a new inner cap from a non-root cap* via a syscall is a primitive question for BUILD-3 PLAN — see §5.

## 5. Forward Markers For BUILD-3 PLAN

Three tractable design points surfaced; none requires BUILD-1c API revision but all should be recorded so BUILD-3 PLAN doesn't rediscover them.

### FM-1. Cap derivation primitive for `accept`

`sys_net_accept_tcp` derives a new `TcpSocket` from an existing `TcpListener`. Phase 2 capability infrastructure supports cap *passing* and cap-from-root creation; it may or may not support *cap derivation* as a first-class kernel primitive (i.e., "kernel mints a new inner cap and hands it to userspace as a syscall return value, parented to a non-`NetworkAccess` cap").

**Action at BUILD-3 PLAN:** Confirm Phase 2 cap-table mechanics allow a syscall to mint a derived inner cap with rights subset of an existing inner cap. If not, either extend the cap subsystem (preferred; small primitive) or re-shape `accept` (rejected; would distort the layered idiom).

**Action at BUILD-1c:** None required. UDP's syscall surface does not need this primitive. But the `NetworkAccess` rights bitmap should *not* preclude later cap-derivation — i.e., reserve a "child caps may be minted" semantic without committing the wire-up.

### FM-2. Read/write rights independence in the inner-cap rights bitmap

TCP half-close (`sys_net_shutdown_tcp(Write)`) requires the WRITE right to be revocable on a `TcpSocket` cap while READ remains held. The BUILD-1c `NetworkAccess` rights bitmap (Q4, deferred to dispatch) ships only UDP, where send and recv share a single I/O right.

**Action at BUILD-1c dispatch:** The rights bitmap *reserves* separate READ and WRITE bits on inner caps, even though UDP at BUILD-1 will require both bits set together. This is the same "reserve-then-activate" pattern used for `LOOKUP_RESERVED` / `READDIR_RESERVED` in the Phase 3 BUILD-4 file-rights work (PM-DECISION-2026-05-16-phase-3-build-4-opening.md §4 Q1). Cost is one bitmap layout decision now; benefit is BUILD-3 does not need to evolve the inner-cap rights ABI.

### FM-3. Cap-lifecycle decoupled from protocol-state (TIME-WAIT class)

TCP's TIME-WAIT state outlives the userspace cap drop. The kernel must hold TIME-WAIT state for 2MSL after the local close, independent of whether the userspace process retains the `TcpSocket` cap. This is a fundamentally different invariant from UDP, where dropping a `UdpSocket` cap returns all per-socket state immediately.

**Action at BUILD-3 PLAN:** Document the invariant explicitly — "kernel may retain protocol-state structures after associated userspace cap is dropped, for protocols requiring graceful close." Accounting (per-process resource quotas) attributes lingering state to the process that initiated the connection, even if the cap has been dropped, until the protocol's lingering state expires.

**Action at BUILD-1c:** None required. UDP does not exhibit the invariant. But the per-process socket counter (BUILD-1c-5: limit 64) should be designed to not require a 1:1 cap-to-state correspondence — i.e., the counter is "kernel-side socket structures attributable to this process," which already accommodates a future TIME-WAIT state.

## 6. What Did NOT Surface

The walk-through did not surface:

- API-shape revision of the layered idiom itself. `NetworkAccess` → inner cap, inner cap as I/O subject, shared `wait` and `close`, cap-passing IPC — all hold for TCP as for UDP.
- A new outer-cap type. `NetworkAccess` covers both UDP and TCP socket creation; no `TcpAccess` distinct cap is required.
- A re-shape of `sys_net_wait`. The shared notification primitive handles ESTABLISHED-receivable, ESTABLISHED-sendable (when window opens), accept-pending on listener, and close-completion. The event-mask grows; the syscall does not.
- Any obstacle that would force BUILD-1c's UDP syscall surface to change. The three forward markers are additive at BUILD-3, not revisionary at BUILD-1.

## 7. Disposition

R16 is **partially discharged** at BUILD-1 PLAN time: the layered shape composes with TCP under structured walk-through, with three specific forward markers handed forward to BUILD-3 PLAN. The remaining R16 residual — that walk-through is not implementation — is discharged only by BUILD-3 implementation.

This artifact does not authorize any code, dispatch, or commitment. It records design substrate for BUILD-3 PLAN consumption.

---

**End of speculative TCP shape sketch.**

*Layered shape composes with TCP. Three forward markers (cap derivation for `accept`, read/write rights independence in inner-cap bitmap, cap-lifecycle decoupling for TIME-WAIT) recorded for BUILD-3 PLAN. BUILD-1c rights-bitmap dispatch should reserve READ/WRITE bits per FM-2 without wiring TCP semantics.*
