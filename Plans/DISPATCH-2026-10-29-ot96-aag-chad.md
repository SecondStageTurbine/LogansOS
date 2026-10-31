<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW (relay #173) → Chad: pre-merge cross-family AAG of OT-96 R1+R2 (#218), wire receive

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. A new kernel thread on the net and driver spin locks that the syscalls on
other CPUs also take, with interrupt-discipline rules (CP-37) found the hard way twice today. The cross-family lens is
required **before** the merge. Paste this one before the rest of your list.

**What it is.** PR #218, branch `claude/ot96-wire-rx`, head `b2c8fe40`. It's not merged; review the branch:
`git fetch origin && git diff origin/main...origin/claude/ot96-wire-rx`. The design is
`docs/DESIGN-2026-10-29-ot96-wire-rx.md` in the PR. Until now nothing received from the wire after boot. This adds:
- **R1:** `kernel/src/net/rx_logic.rs`. `classify(frame, local)` bounds every header field, then routes the frame
  to ARP, NDP/RA, DHCP, a UDP socket, TCP, ICMP echo, or a counted drop.
- **R2:** `kernel/src/net/rx.rs`, the `net-rx` thread (feature `wire-rx`). Each pass, per device, it calls
  `service()`, takes up to 24 frames (copy out, recycle the buffer, route the copy), runs `tcp::flush_wire_egress`,
  then naps one tick.
  - Every net and driver lock step runs under `without_interrupts`.
  - ARP requests for our address are answered.
  - ICMP echo replies are capped at 16 a second.
  - It carries #215's `IrqSafeMutex` registry commit.

One pushed RETURN, a plain checklist (pass or fail, the enforcing `file:line`, and a concrete input or interleaving for
any fail): `Plans/RETURN-2026-10-29-ot96-aag-chad.md`.

1. **IF discipline.** Is every net or driver lock the `net-rx` thread takes, directly or through what it calls
   (`service`, `rx_take`, `rx_recycle`, ARP, `deliver_inbound`, `inject_segment`, `flush_wire_egress`, the drivers'
   TX), taken with interrupts off? Is any of those locks also taken with interrupts on by a thread-context path
   elsewhere (USB hotplug, VFS bodies, other kthreads)?
2. **Lock order and drops.** Is the driver lock released before any net lock? Is the socket table dropped before the
   socket's own lock? Does any `Arc`'s last drop run under a lock (CP-56)? Is any governance lock reachable (I-14(a))?
3. **DMA.** Is every byte parsed from the private copy, never from device-writable memory, and never read twice? Is
   each buffer recycled exactly once, on every path?
4. **Liveness.** Is each IF-off section bounded (one device service or one frame)? Can the thread block on anything
   unbounded? Under a flood, does the 24-frame budget plus the nap keep the CPU available?
5. **Two CPUs.** The thread on one CPU, a syscall on the same socket or listener on another: can a delivery race a
   close, so data lands in a freed or reused socket? Do OT-94's listener bounds hold for wire SYNs?
6. **The parser** (Muse reviews it in depth after the merge; spot-check here). Is any length or offset field used
   before it's bounded by the bytes present? With overflow checks on in the shipped kernel, can any arithmetic
   overflow?
7. **The kill-switch and the guard.** With `wire-rx` off, is nothing compiled? Does the `compile_error!` refuse the
   three IF-on harness scenarios built with `wire-rx`?
8. **Pitfall rows** in the PR body (I-14(a), CP-37, CP-1, CP-56, I-14(b)): is each claim right? Is any class
   missing?

```
MODEL: GPT-6.1-Sol. EFFORT: high. (A new kernel thread on shared spin locks: the required pre-merge cross-family lens.)
WORKTREE: E:\AI Automation\workers\chad   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot96-aag-chad
          git fetch origin && git checkout -B codex/ot96-aag-chad origin/main

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-ot96-aag-chad.md
git diff origin/main...origin/claude/ot96-wire-rx
```

RAG health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that and read the files
directly. Push the RETURN. Before ending, run `git status --porcelain` and `git log @{u}..`; both must be empty.
