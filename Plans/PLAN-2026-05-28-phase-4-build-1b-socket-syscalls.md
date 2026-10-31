# PLAN: Phase 4 BUILD-1b — sys_socket_* Syscall Surface

**Date filed:** 2026-05-28
**Phase:** 4 (Network Stack)
**Build:** 1b (Syscall Surface)
**Status:** PENDING PL RATIFICATION
**Complexity:** Medium (kernel-internal network complete; exposing via syscalls + capability gates)
**Estimated effort:** ~3–5 days (design 1 day, implementation 2–3 days, validation 1–2 days)

---

## Objective

Expose kernel-resident network socket operations via the syscall interface. Phase 4 BUILD-1a provides virtio-net device driver + smoltcp wrapper (kernel-internal). BUILD-1b exposes socket operations to userspace via sys_socket_* family (0x180–0x19F), enabling userspace processes to create, bind, send, and receive UDP datagrams.

**Goal milestone:** A userspace test process can create a UDP socket, send a datagram to itself, and receive it. Validation on QEMU + NEO_SMP=4 (same as BUILD-1a).

---

## Scope

### Syscalls to implement (5 core operations)

**0x180: `sys_socket_create`**
```
sys_socket_create(family: u32, socktype: u32) -> Result<SocketCap>
```
- Args: `family` (AF_INET=2, AF_INET6=10, only these two supported)
- Args: `socktype` (SOCK_DGRAM=2 for UDP, only SOCK_DGRAM supported in Phase 4)
- Returns: capability handle to socket (per-process socket ID, capability-gated)
- Behavior: Allocates a socket object in the kernel, initializes smoltcp UDP socket, returns handle

**0x181: `sys_socket_bind`**
```
sys_socket_bind(cap: SocketCap, addr: *const u8, addr_len: usize) -> Result<()>
```
- Args: socket capability (must be WRITE permission)
- Args: sockaddr pointer (in/out; kernel reads addr, port)
  - For IPv4: `struct sockaddr_in { sin_family: u16, sin_port: u16, sin_addr: u32 }` (4 bytes addr)
  - For IPv6: `struct sockaddr_in6 { sin6_family: u16, sin6_port: u16, sin6_addr: [u8; 16] }` (16 bytes addr)
- Returns: Ok if bind succeeds, error if port in use or invalid addr
- Behavior: Binds socket to local addr/port; subsequent sends originate from this addr

**0x182: `sys_socket_send`**
```
sys_socket_send(cap: SocketCap, data: *const u8, len: usize, flags: u32) -> Result<usize>
```
- Args: socket capability (WRITE permission)
- Args: data buffer (userspace → kernel)
- Args: flags (0 = blocking; future: MSG_DONTWAIT for non-blocking)
- Returns: bytes sent (or error)
- Behavior: Queue datagram for TX; if addr not yet set, returns ENOTCONN; dispatches to virtio-net TX ring

**0x183: `sys_socket_recv`**
```
sys_socket_recv(cap: SocketCap, buf: *mut u8, len: usize, flags: u32) -> Result<usize>
```
- Args: socket capability (READ permission)
- Args: buf pointer (kernel → userspace)
- Args: flags (0 = blocking; future: MSG_DONTWAIT)
- Returns: bytes received (or error)
- Behavior: Wait for RX datagram; copy to buf; return length. If no data, block until arrival (or timeout)

**0x184: `sys_socket_close`**
```
sys_socket_close(cap: SocketCap) -> Result<()>
```
- Args: socket capability
- Returns: Ok
- Behavior: Closes socket; kernel deallocates smoltcp socket; capability becomes invalid

### Non-scope

- ❌ TCP (SOCK_STREAM) — Phase 5+
- ❌ Multicast — Phase 6+
- ❌ Socket options (SO_RCVTIMEO, SO_SNDBUF, etc.) — Phase 5+
- ❌ Control messages (ancillary data, IP_RECVTOS, etc.) — Phase 5+
- ❌ Vectored I/O (sendmsg, recvmsg) — Phase 5+
- ❌ select/poll/epoll — Phase 5+
- ❌ Non-blocking (MSG_DONTWAIT) — Phase 5+

---

## Design: Syscall Binding Layer

### Syscall handler dispatch (kernel/src/syscall.rs amendment)

Add branch in the syscall dispatcher (around line 200–250):

```rust
0x180..=0x184 => {
    match num {
        0x180 => net::socket_create(arg0 as u32, arg1 as u32),
        0x181 => net::socket_bind(arg0, arg1, arg2),
        0x182 => net::socket_send(arg0, arg1, arg2, arg3 as u32),
        0x183 => net::socket_recv(arg0, arg1, arg2, arg3 as u32),
        0x184 => net::socket_close(arg0),
        _ => Err(SyscallError::EAINVAL),
    }
}
```

### Kernel-internal socket abstraction (kernel/src/net/socket.rs, new file)

```rust
pub struct Socket {
    id: SocketId,
    family: AddressFamily,
    state: SocketState,
    local_addr: SocketAddr,
    smoltcp_socket: smoltcp::socket::UdpSocket,
    rx_queue: VecDeque<Vec<u8>>,    // Buffered RX datagrams
}

enum SocketState {
    Unbound,
    Bound,
    Closed,
}

pub fn socket_create(family: u32, socktype: u32) -> Result<SocketHandle> {
    // Validate family, socktype
    // Allocate Socket struct
    // Initialize smoltcp UDP socket
    // Register in kernel socket table
    // Return capability handle
}

pub fn socket_bind(handle: SocketHandle, addr: *const u8, len: usize) -> Result<()> {
    // Validate capability, state
    // Copy sockaddr from userspace
    // Bind smoltcp socket
    // Update socket state
}

pub fn socket_send(handle: SocketHandle, data: *const u8, len: usize, flags: u32) -> Result<usize> {
    // Validate capability
    // Copy data from userspace
    // Dispatch to TX via smoltcp
    // Return bytes sent
}

pub fn socket_recv(handle: SocketHandle, buf: *mut u8, len: usize, flags: u32) -> Result<usize> {
    // Validate capability
    // Block until RX available (or timeout)
    // Copy from RX queue to userspace buf
    // Return bytes received
}

pub fn socket_close(handle: SocketHandle) -> Result<()> {
    // Validate capability
    // Deallocate smoltcp socket
    // Invalidate capability
}
```

### Ring integration

The kernel RX interrupt handler (virtio-net ring completion) should:

1. Dequeue completed RX descriptors
2. Parse Ethernet frame → IP packet → UDP datagram
3. Lookup receiving socket by destination port
4. Enqueue datagram in socket's rx_queue
5. Wake any blocked thread waiting on socket_recv

---

## Validation

### Host cargo check

```bash
cargo check -p neoplutarch-kernel --features neo-test-phase4-virtio-net-socket-udp
```

Verify: No clippy warnings, no unwrap() in steady-state code, capability semantics correct.

### QEMU smoke test

**Test scenario: `phase4_socket_udp_loopback`**

1. Userspace test process (via sys_socket_*):
   - Create UDP socket (AF_INET, SOCK_DGRAM) → cap_socket_1
   - Bind to 127.0.0.1:5555
   - Send datagram to 127.0.0.1:5555 ("hello")
   - Recv on same socket → should receive "hello"
2. Kernel:
   - TX: virtio-net ring dispatch to loopback (127.0.0.1 → virtio_net_tx_queue)
   - RX: Frame arrival → kernel reassembles UDP → lookup socket 5555 → queue datagram
   - RX: socket_recv wakes; returns datagram
3. Validation: Exit 0, datagram integrity verified

### Full-system (NEO_SMP=4)

Run all 31 scenarios (same as BUILD-1a) + socket UDP test as scenario 32.

---

## ISCs (Implementation Success Criteria)

1. **ISC-1b-1: Syscall handler compiles and dispatches 0x180–0x184** without undefined behavior
2. **ISC-1b-2: Socket creation, binding, send, recv work on host (cargo check)** + clippy clean
3. **ISC-1b-3: QEMU smoke test (phase4_socket_udp_loopback) passes** — datagram loopback works
4. **ISC-1b-4: No panics / unwraps in steady-state socket operations** (validated by cargo check + QEMU logs)
5. **ISC-1b-5: Full NEO_SMP=4 sweep passes** — all 32 scenarios (31 existing + UDP socket test)

---

## Risk Assessment

**Medium-low risk:**

✓ Kernel-internal network subsystem (BUILD-1a) is solid (all gates passed, ~71 min validation)
✓ smoltcp wrapper is stable (78 lines, well-tested in Phase 4 BUILD-1a)
✓ Syscall dispatcher pattern is established (follows §6 of neoplutarch-ai-subsystem-spec.md)

⚠ **Potential blockers:**

1. **RX → socket lookup:** How does the kernel route an arriving datagram to the correct userspace process? Need per-process socket namespace (e.g., per-PID socket table, or global table with PID tagging). Recommend: global kernel socket table, keyed by (PID, port).
2. **Blocking semantics:** socket_recv blocks on an empty rx_queue. Requires futex-like wait primitive. Recommend: kernel event notification or simple sleep-spin (acceptable for Phase 4).
3. **Capability lifetime:** Socket capability should become invalid on close(). Need to detect invalid caps in the syscall handler. Recommend: socket table maintains cap → (Socket, valid) mapping; invalid caps fail with EBADF.

---

## Phasing: Implementation Sequence

### Sub-1: Socket abstraction + syscall dispatch (Day 1)
- Implement `struct Socket` and socket table management
- Implement syscall handler dispatch (0x180–0x184)
- Implement socket_create, socket_close, socket_bind
- Host: cargo check passes

### Sub-2: RX/TX queue integration (Day 2)
- Integrate socket_send with virtio-net TX queue
- Implement socket_recv with blocking (placeholder: sleep-spin)
- RX interrupt handler → socket rx_queue enqueue
- QEMU smoke test passes

### Sub-3: Full-system validation (Day 1–2)
- NEO_SMP=4 sweep + socket UDP test scenario
- Barrier discipline verification (same as BUILD-1a)
- Finalize, document, close

---

## Artifacts

**New files:**
- `kernel/src/net/socket.rs` — Socket abstraction + syscall binding

**Modified files:**
- `kernel/src/syscall.rs` — Add 0x180–0x184 dispatch
- `kernel/src/drivers/net/virtio_net.rs` — Amend RX handler to enqueue socket rx_queue
- `kernel/src/net/mod.rs` — Export socket_* functions
- `kernel/src/main.rs` — Register socket subsystem
- `kernel/Cargo.toml` — (no changes, smoltcp already present)

**Test infrastructure:**
- `kernel/src/tests/phase4_socket_udp_loopback.rs` — UDP loopback smoke test

---

## Next Build (Phase 4 BUILD-2)

**Scope:** UDP messaging (userspace process-to-process)

- Multiple processes binding different UDP ports
- Processes exchange datagrams across process boundaries (capability-gated socket sharing)
- Broadcast / multicast (Phase 6+, deferred)

---

**PLAN end.**

*Awaiting PL ratification. On approval, dispatch to worker (Jarvis or available) for implementation.*