<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW BUNDLE (relays #167–#169) → Muse: input-route authority, TCP listen bounds, the lock screen

Three security reviews of Claude-authored code. All merged after batch 19 (run `40883250031`, green at `2a313a39`).
**Read the files at each squash SHA from the repo, not a summary.** The PL pastes your RETURNs back, and the PM banks
them. Write each as a plain checklist: pass or fail, the enforcing `file:line`, and a concrete input for any fail.

## #167: #210 `ff4bdd51`, input route activation needs the route's own authority. Banked as `Plans/RETURN-2026-10-29-input-activate-review-muse.md`

Product security (CP-96). Before this, `mint_activator` gave ACTIVATE to any process holding RECEIVE on an input
route, so any such process could take the keyboard. Files: `kernel/src/arch/x86_64/syscall.rs`, `kernel/src/main.rs`,
`kernel/src/sched/process.rs`, `userland/neo-arthur/src/showcase_surface.rs`.
1. Can a process that isn't the route's owner (the WM or the session broker) get ACTIVATE on a route by any path:
   minting, deriving, a handle transfer over a channel, or a spawn grant?
2. Is a process holding only RECEIVE refused ACTIVATE, with a defined errno, and is the route left unchanged?
3. Does revoking the owner's capability also end every ACTIVATE derived from it?
4. Does the scenario's negative leg fail with the old rule?

## #168: #211 `cb5d633a`, OT-94, a TCP listener's queues stay bounded. Banked as `Plans/RETURN-2026-10-29-tcp-listen-bounds-review-muse.md`

Network resource exhaustion. Before this, a SYN flood grew the half-open set without bound. File:
`kernel/src/net/tcp.rs`; scenario `tcp_listen_bounds`.
1. Can a remote peer make a listener hold more than `backlog` half-open connections, or more than `backlog`
   connections waiting for accept, by any sequence of SYN, ACK, RST, or data?
2. Is every refused or reaped connection removed from `CONNECTIONS`, so no entry is left with no handle? Check the
   timeout reap, the peer's RST, and a handshake completing into a full accept queue.
3. Can a forged RST or ACK (wrong sequence number) free a half-open slot that belongs to another peer?
4. Is the SYN timeout (300 ticks) applied from the connection's own reservation, so a flood can't keep a slot alive?

## #169: #208 `ed56c60d`, OT-66 L2, the lock screen (Super+L). Banked as `Plans/RETURN-2026-10-29-l2-lock-review-muse.md`

Input isolation. Super+L locks the session; the WM shows the lock view and takes the lock focus. A pre-merge Claude
AAG found and fixed three problems:
- F1: a route takeover while locked;
- F2: a channel flood;
- F3: waits on the control channel.

Files: `userland/neo-init/src/desktop_session/lock.rs`, `desktop_session.rs`, `login.rs`; `kernel/src/atrium/focus.rs`,
`sak_logic.rs`.
1. While locked, can any key or pointer event reach a client other than the lock view? Check every route, the
   Super+Space and Super+Escape chords, and a client that was activated just before the lock.
2. Can a client unlock the session, or skip the password check, without the user typing the password into the lock
   view?
3. Is the password compared in constant time, and kept out of logs and out of any buffer a client can read?
4. Can a client stall the lock (F2, F3): flood its channel, or never answer the WM, so the lock view never appears
   or never takes input?
5. Does the session stay locked across an Arthur fault or a client crash while locked?

```
WORKTREE: Muse reads the repo at the SHAs above; it has no tree and doesn't push.
The PL pastes each RETURN back to the PM, who banks it at the path given.

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-review-bundle-b19-muse.md
git show ff4bdd51 ; git show cb5d633a ; git show ed56c60d
```
