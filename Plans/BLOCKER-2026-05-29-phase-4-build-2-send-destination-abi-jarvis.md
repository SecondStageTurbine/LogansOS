# BLOCKER: Phase 4 BUILD-2 requires UDP destination addressing but BUILD-1b ABI has none

Date: 2026-05-29
Branch: codex/phase-4-build-2-jarvis
From: Jarvis
To: PM / PL

## Summary

BUILD-2 cannot proceed without a PM/PL ABI decision for UDP destination
addressing.

The PLAN requires datagram sends to a destination address:

- ISC-2-3: `sys_socket_send(handle_A, packet, [::1]:5556)`
- ISC-2-4: bind to a SLAAC-acquired global address and send UDP to a remote
  host on the same prefix

The merged BUILD-1b syscall ABI has no destination-address argument and no
connect syscall:

```rust
SYS_SOCKET_SEND => sys_socket_send_impl(a0 as u32, a1 as *const u8, a2 as usize, a3 as u32)
```

This handler accepts only:

- socket handle
- payload buffer
- payload length
- flags

The libneo wrapper mirrors that shape:

```rust
pub fn sys_socket_send(handle: SocketHandle, buf: &[u8]) -> i64
```

Current kernel behavior therefore can only enqueue to the sender socket's own
loopback queue. It cannot express "send this datagram to another bound socket"
or "send this datagram to a remote IPv6 endpoint."

## What I tried

1. Fetched `origin/main`.
2. Created `codex/phase-4-build-2-jarvis` from `origin/main`.
3. Read:
   - `Plans/PLAN-2026-05-29-phase-4-build-2-slaac-ipv6.md`
   - `Plans/DISPATCH-2026-05-29-phase-4-build-2-slaac-ipv6-jarvis.md`
4. Inspected the BUILD-1 baseline:
   - `kernel/src/net/socket.rs`
   - `kernel/src/arch/x86_64/syscall.rs`
   - `userland/libneo/src/lib.rs`
   - `kernel/src/net/smoltcp_wrap.rs`
   - `tools/xtask/src/main.rs`

## Blocking Details

The blocker is not a local compile or implementation issue. It is an ABI/spec
gap:

- BUILD-2 asks for UDP datagram destination selection.
- BUILD-1b exposed no `sendto`-style address parameter.
- BUILD-1b exposed no `connect`/default-peer syscall.
- BUILD-2 dispatch only adds `sys_net_stats` at 0x185; it does not authorize a
  new destination-address syscall or a changed 0x182 ABI.

Changing 0x182 ad hoc would alter the already-merged syscall ABI and libneo
surface. Adding a new syscall, for example `sys_socket_sendto`, would exceed
the dispatch unless PM/PL explicitly ratifies it.

## Additional Harness Gap

The current QEMU harness path only adds virtio-net devices via `hubport`
netdevs for the Phase 4 network scenarios. I found no RA generator, tap setup,
host-side IPv6 router, radvd/dnsmasq integration, or remote UDP peer wiring in
the existing xtask path. That may become a second blocker for ISC-2-4 after the
socket ABI is resolved, but the send-destination ABI gap blocks earlier at
ISC-2-3.

## Hypothesis

BUILD-2 likely needs one of these PM/PL decisions:

1. Revise `SYS_SOCKET_SEND` 0x182 to a sendto ABI that includes a destination
   sockaddr pointer and length.
2. Keep 0x182 as-is and add a new `SYS_SOCKET_SENDTO` syscall.
3. Add `SYS_SOCKET_CONNECT` and define `SYS_SOCKET_SEND` as connected-datagram
   send.

Option 2 is the least disruptive to the merged BUILD-1b ABI.

## Information Needed

Please ratify the UDP destination-address ABI before implementation resumes:

- syscall number, if adding a new syscall
- argument layout
- libneo wrapper shape
- whether the existing `sys_socket_send` self-loopback behavior remains for
  BUILD-1b compatibility
- whether ISC-2-4 requires real host-side RA/UDP peer setup in xtask or permits
  a kernel/test-harness RA injection path for BUILD-2

No workaround has been applied.

