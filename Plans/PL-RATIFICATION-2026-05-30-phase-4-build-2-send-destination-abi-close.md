# PL-RATIFICATION: Phase 4 BUILD-2 send-destination ABI (0x186/0x187) — ratified as-is

**Date:** 2026-05-30
**From:** PL (decision recorded by PM/Cerebro)
**Status:** RATIFIED — closes BUILD-2 carried-forward open item (a)
**References:**
- Plans/PM-DISPOSITION-2026-05-29-phase-4-build-2-send-destination-abi.md (PM recommendation)
- Plans/CLOSE-REPORT-2026-05-29-phase-4-build-2.md §"Open items carried forward" item 1
- kernel/src/arch/x86_64/syscall.rs:69–76 (syscall ledger as shipped)

---

## Decision

The send-destination ABI shipped in BUILD-2 is **ratified as-is**. No rework.

| Syscall | Number | Ratified semantics |
|---------|--------|--------------------|
| `sys_socket_send` | 0x182 | Connected-send. Returns `ENOTCONN` until `connect` lands. |
| `sys_net_stats` | 0x185 | Per-socket packet/byte counters. |
| `sys_socket_sendto` | 0x186 | Destination-bearing UDP send (sockaddr in args). |
| `sys_socket_recvfrom` | 0x187 | Recv reporting source sockaddr. |
| *(reserved)* | 0x188 | Reserved for future `sys_socket_connect`. Unchanged. |

The BSD-conventional `sendto`/`recvfrom` shape was the PM-recommended,
least-surprising choice. PL accepts it. The alternative (connect-first
shape, which would have reworked BUILD-2b) is **declined**.

## Effect

- BUILD-2 carried-forward open item (a) — "PL ratification: sendto/recvfrom
  ABI (0x186/0x187)" — is **CLOSED**.
- The next free socket-syscall number for BUILD-3 and beyond is **0x189**
  (0x188 remains reserved for `connect`).
- No code change results from this ratification; it confirms what is already
  on `main` (BUILD-2 merge `05cff8d`).

## Standing

This is a project-lead ratification of an already-shipped ABI. It does not
re-open BUILD-2 and does not gate BUILD-3; it discharges a governance flag
that was left open at BUILD-2 close.
