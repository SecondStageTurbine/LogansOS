<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW (relay #157) → Muse: sockets close when their last handle leaves (#202)

**Reviewed SHA:** `34c6c132` (#202's squash on `main`; batch 17 run `40855721787`, green at `e29f21d0`). Read the files
at that SHA from the repo, not a summary. **Deliverable:** a RETURN the PL pastes back, banked as
`Plans/RETURN-2026-10-29-socket-teardown-review-muse.md`. Write it as a plain checklist: pass or fail, the enforcing
`file:line`, and a concrete input for any fail.

**The defect.** A socket handle leaving a table any way other than `SYS_SOCKET_CLOSE` (exit, kill, revoke) never closed
the socket, because the registries' own `Arc` kept `Drop` from running. A bound port stayed bound forever, and
bind-and-exit leaked kernel heap. The socket syscalls are ungated and ambient in the product image.

1. **The handle count** (`net::HandleCount` on `Socket`, `TcpSocket`, `TcpListener`). It's retained at
   `HandleTable::install` / `install_excluding` and released in `Detached::release`. Can a handle copy (grant, derive,
   spawn inheritance) enter a table without a retain, or leave without a release?
2. **Matching.** The close matches the table entry by `Arc::ptr_eq`. Can an old handle's close remove a newer socket
   bound to the same address?
3. **The listener.** Its close also closes the connections no process has a handle to: those queued for accept, and
   half-open ones. Can a connection that a process has accepted be closed by its listener's close?
4. **Revoke.** Revoking a socket cap closes the socket when it was the last handle. Can revoke leave an endpoint live
   for nobody, or close one another process still holds?
5. **Remote peers.** The PR notes `passive_open` doesn't bound half-open connections (OT-94, a separate lane). Is
   anything else in this PR reachable by a remote peer to grow kernel state?
