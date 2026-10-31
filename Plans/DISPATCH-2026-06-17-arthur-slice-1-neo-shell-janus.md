<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Arthur arc-2 Slice 1 — `neo-shell` (the minimal capability-aware REPL) → Janus

**Date:** 2026-06-17
**From:** PM (Claude)
**To:** Janus (parked for the Arthur arc since 8.5; clean fit). New branch off `origin/main` HEAD: `codex/phase-8-arthur-slice-1-neo-shell-janus`.
**Status:** DISPATCHED. **The first build of Arthur's user-facing surface.** With 8.5 landed (the model generates), arc-2 (the serving/userland surface) is unblocked, and the PL set the order: **Slice 1 → Benchmark → Consolidation.** This is Slice 1. It carries the **one and only kernel change in the entire Arthur arc** — the serial-stdin path — and is otherwise pure userland.

## 0. The decision this build makes
Build the **invocation surface**: a minimal userland REPL (`neo-shell`) you can type into, that spawns capability-bounded child processes and relays IPC — the thing a user (later, Arthur) is invoked *through*. This satisfies the Article 28.2 floor (*"boot, log in, type, reach a working userspace shell"*) at its **smallest sufficient** form (Rule-3), and lays the surface Slices 2–5 build Arthur on. **It is not Arthur, not the action surface, not inference** — it is the REPL + the one syscall that lets a human type into the machine.

## 1. Authoritative design — RATIFIED (read first, build to it)
```
git fetch origin
git show origin/main:Plans/THINK-2026-06-16-arthur-userland-arc-and-architecture.md   # §10 = the ratified decisions; §2 = neo-shell design; §6 Slice 1
git show origin/main:docs/VISION-arthur.md                                              # the destination this serves
```
The design is **decided** — build it, don't re-decide it. Ratified calls that bind this slice (THINK §10.2/§10.3):
- **OD-1 = A — serial stdin.** `neo-shell` reads user input from the 16550 UART RX FIFO via a new `SYS_READ` path on **fd=0**, symmetric to the existing fd=1-is-serial-stdout namespace rule (`syscall.rs` `sys_write_impl`). **This ~50-LOC kernel addition is the arc's only kernel-touching change** — keep it that minimal.
- **Slice 1 scope (THINK §6, as amended by §10.3):** reads lines from fd=0, echoes to fd=1, spawns children with scoped capabilities, relays IPC, reports exit. **No builtins beyond the minimal set** (`spawn`, `exit`). Reuse the existing `EcmNetDevice`/`neo-init`/`neo-worker` userland patterns and `libneo`.

## 2. ISCs
- **ISC-1 — The serial-stdin kernel path (the one kernel change).** Add a `SYS_READ`/fd=0 path: a read on fd=0 pulls bytes from the 16550 RX FIFO (blocking-or-EAGAIN per the existing console idiom; document the choice). fd=0 is **namespace-reserved** (like fd=1 = serial stdout), not a capability handle — symmetric, minimal. The MMIO RX read carries a `// SAFETY:` note (ADR-025 typed MMIO where applicable). **No new `unsafe` beyond that documented read; no new invariant; no AI-governance/lock touch.**
- **ISC-2 — `neo-shell` (the REPL).** New crate `userland/neo-shell/` (~400 LOC) linking `libneo`: read a line from fd=0, echo to fd=1, parse `command [args...]`, dispatch the minimal builtins (`spawn <name>` → `sys_spawn`; `exit` → terminate), wait on a spawned child and report its exit status. A clean prompt + line discipline (backspace handling at minimum); **no** readline/history/pipes/redirection/job control (that's the POSIX-shell trap §2B rejects).
- **ISC-3 — Capability-bounded spawn + IPC relay.** `spawn` launches a child via `sys_spawn` with a **scoped capability bundle** (derive/grant only what the child needs — demonstrate the "shell grants scoped caps" spine, even with a trivial demo child), and the shell relays a message to/from the child over an IPC channel (the relay primitive Slices 3–5 reuse for Arthur). The child inherits **nothing** it wasn't granted.
- **ISC-4 — Standards + kill-switch.** **Kill-switch (Rule-1):** the fd=0 path + `neo-shell` are additive and default-off — with the feature/config disabled, fd=0 behaves as today (no serial read) and `neo-shell` is not spawned, so the kernel is **bit-for-bit today** (the disabled≡prior regression). `fmt`/`clippy -D warnings` on the touched crates; host-testable parsing/dispatch logic in a `*_logic` split where it factors cleanly; ANSPEC §11.x or a short note; ADR if the fd=0 ABI warrants one (likely a paragraph in an existing console/syscall ADR).

## 3. Gate
- **Host:** the line-parse/builtin-dispatch logic unit-tested; builds on+off the feature; `fmt`/`clippy` clean.
- **QEMU scenario `neo_shell_repl`:** boot → `neo-init` spawns `neo-shell` → shell prompt appears on serial → a scripted serial input types a command → it echoes → `spawn` launches a child that runs and exits → shell reports the exit → `exit` terminates the shell → `neo-init` continues. Serial I/O round-trip + capability-scoped spawn + IPC relay all exercised. NEO_SMP=4.
- **Kernel-touching ⇒ run the pre-merge `workflow_dispatch` QEMU gate green (all 4 shards) before declaring close** (the fd=0 syscall makes this kernel-touching; the gate that just validated 8.5 — run flow in `Plans/RUNBOOK-second-self-hosted-runner.md`).

## 4. Scope guard
The minimal REPL + the fd=0 serial-stdin syscall + capability-scoped spawn + IPC relay + `exit` **only**. **Do NOT** build: the capability-bounded **action surface** (Slice 2), the **Arthur process** (Slice 3), intent parsing (Slice 4), on-demand cap escalation (Slice 5), inference wiring, or a full POSIX shell (no pipes/redirection/job control/history/env). One kernel change only (fd=0); no new `unsafe`/invariant; reuse `libneo` + existing userland patterns.

## 5. Merge (PM)
Close-gate: rebase onto current `main`; re-run the host logic tests + the `neo_shell_repl` QEMU scenario; **run the pre-merge `workflow_dispatch` QEMU gate green** (kernel-touching); push (CI). PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (Article 13 — **Arthur arc-2 Slice 1: the invocation surface** — + ANSPEC §11.x). This is the build where LogansOS first has **a shell a human types into** — the surface Arthur is invoked through. Next: Slice 2 (the capability-bounded action surface — pure userland, OD-2 is moot so no kernel change).

**Disk reclaim (after PM confirms merge):** once this branch is on `origin/main` and the PM confirms it, delete this build's working tree to reclaim its `target/` — `git worktree remove <path>` (then `git worktree prune` if needed) if a worktree, or delete the folder if a standalone clone. **Never delete a tree whose branch is unpushed/unmerged** — verify the work is on `origin/main` first. (`cargo clean` is the interim if you want to keep the checkout.) See CLAUDE.md "Disk hygiene — reclaim the build tree after merge."

---
## Relay — what to tell Janus
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-17-arthur-slice-1-neo-shell-janus.md` (`git show origin/main:<path>`).
- **Branch:** `codex/phase-8-arthur-slice-1-neo-shell-janus` off `origin/main` (fresh; has 8.5).
- **One-line:** Build **`neo-shell`** — the minimal capability-aware REPL, **Slice 1 of the Arthur arc** (read the RATIFIED design: `git show origin/main:Plans/THINK-2026-06-16-arthur-userland-arc-and-architecture.md` §10 + §6, and `docs/VISION-arthur.md` for the why). One kernel change only — **OD-1=A serial stdin**: a `SYS_READ`/fd=0 path pulling from the 16550 RX FIFO, symmetric to the fd=1-serial-stdout rule (~50 LOC, documented `// SAFETY:` MMIO read, no other new `unsafe`/invariant). Then a userland `userland/neo-shell/` crate (~400 LOC, links `libneo`): read line from fd=0 → echo to fd=1 → parse `command [args]` → builtins `spawn` (capability-scoped `sys_spawn` + IPC relay to the child) and `exit`. **Minimal only** — NO action surface (Slice 2), NO Arthur (Slice 3), NO intent parsing/inference, NO POSIX shell (pipes/redirection/history). Kill-switch: feature default-off ⇒ kernel bit-for-bit today. Gate: the `neo_shell_repl` QEMU scenario (boot→prompt→type→echo→spawn-child→exit→init continues) at NEO_SMP=4 + **kernel-touching ⇒ run the pre-merge QEMU gate green before close**. fmt/clippy clean.
