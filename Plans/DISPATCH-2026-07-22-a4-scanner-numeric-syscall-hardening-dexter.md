<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: A4 manifest-gate scanner hardening — catch numeric-literal syscalls → Dexter

**Date:** 2026-07-22
**From:** PM (Claude)
**To:** Dexter (DeepSeek, OpenCode — second cell)
**Status:** DISPATCHED — **host-only / build-script cell (NO hardware, NO heavy QEMU gate — push-CI + a falsification proof).** A real security-hardening the C0.5 AAG surfaced. Bounded, self-contained, and un-gameable (the falsification test is the objective judge — same shape as your first cell).

## The gap (real, flagged by the C0.5 audit)
The **A4 anti-compat-island gate** (`userland/neo-std-pal/build.rs`) is the structural guard that fails the build if the PAL or the `sys/*` overlay references a syscall not in `tools/ot61-pal-manifest.toml` `[gate].allowed_syscalls`. It works by tokenizing **named `SYS_*` identifiers** (`scan_sys_tokens`). But a raw `syscall` with a **numeric-literal `rax`** — e.g. `in("rax") 6u64` instead of `in("rax") SYS_MAP_REGION` — **is invisible to the scanner and evades the gate.** (The overlay's `apply-overlay.sh` exit arm already uses a numeric `0u64` for `SYS_EXIT`.) So an off-manifest syscall added via a numeric literal would NOT fail the build — a hole in the guard the whole browser-confinement story leans on.

## Scope — close the numeric-literal hole
Harden `scan_sys_tokens` / the scan in `userland/neo-std-pal/build.rs` so a **numeric `rax` syscall literal is also caught and checked against the manifest.** Two acceptable approaches (pick the cleaner):
- **(A) Number → name mapping:** detect the `in("rax") <N>u64` (and `mov rax, <N>` if present) pattern in the scanned `.rs` files, map the number to its syscall name via the known syscall-number table, and apply the same `allowed_syscalls` check. (You'll need the number→name table — pull the authoritative syscall numbers from the kernel's syscall definitions; don't hardcode a guess.)
- **(B) Lint against numeric `rax`:** require every syscall site in the PAL + overlay to use the *named* `SYS_*` constant, and **fail the build** if a numeric `rax` syscall literal appears (forcing the named form the scanner already checks). Simpler + arguably safer (no number table to drift), but touches the overlay's exit arm — if you pick (B), convert `apply-overlay.sh`'s numeric `0u64` exit to the named `SYS_EXIT` so it complies.
- Keep `build.rs` **dependency-free** (hand-parse, no new crates — the existing discipline).
- The scan must cover **both** the PAL `src` AND the overlay (`../neoplutarch-std-overlay/std`) — the C0.5 fix already chains both trees; your change extends the *detection*, not the *coverage*.

## The proof (un-gameable — this is the judge)
- **Falsification (the key deliverable):** add a **numeric off-manifest** syscall to a scanned file (e.g. `in("rax") 99u64` in `stdio_neoplutarch.rs`) → the build **FAILS** the A4 gate → revert it. Paste the failing build output in your RETURN.
- **Clean pass:** the current tree (whose real numeric syscall is `0`=`SYS_EXIT`, which IS allowed) still builds green — no false positive on the legitimate `SYS_EXIT`.
- If you take approach (A), a unit test for the number→name mapping (a KAT: number 6 → `SYS_MAP_REGION`, etc.).

## Standing rules
- Behavior-preserving build tooling — no kernel/userland runtime change. SPDX on any new file. No new invariant.
- **Never `[skip ci]`** — `build.rs` runs during compile, so let push-CI run.

## Close-gate + merge (your second cell — the ritual matters)
- **Rebase-fresh FIRST:** `git fetch origin && git rebase origin/main` before you build (main moves; a stale base bounces at merge — this is the close-gate's first requirement).
- Run `scripts/worker-close-gate.sh` in full. **This is NOT kernel-touching** (only `build.rs`/the overlay + maybe a host KAT), so a green **push-CI** URL is the gate (no 4-shard QEMU needed unless you touch a kernel path). **"Green" without a CI URL is not done** — paste the push-CI URL + the falsification-fails output.
- Branch `dexter/a4-scanner-numeric-hardening`; PM merges after CI + a light review. `Workers/dexter`.

## Relay — what to tell Dexter
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-22-a4-scanner-numeric-syscall-hardening-dexter.md`. Files: `userland/neo-std-pal/build.rs` (the scanner), `userland/neoplutarch-std-overlay/std/{stdio_neoplutarch.rs,alloc_neoplutarch.rs}` + `apply-overlay.sh` (the overlay syscalls), `tools/ot61-pal-manifest.toml` (allowed list). The kernel syscall-number table (for the number→name map if you pick A): grep the kernel's syscall dispatch for the `SYS_*` numeric values. Work in `Workers\dexter`; branch `dexter/a4-scanner-numeric-hardening`.
- **One-line:** The A4 build-gate catches named `SYS_*` references but a **numeric-literal `rax` syscall evades it** (the C0.5 audit flagged this real hole). Harden `build.rs`'s scanner to also catch/reject numeric `rax` syscall literals in the PAL + overlay — either map number→name→allowed-list check (pull the real syscall-number table, don't guess) OR lint against numeric `rax` and require the named `SYS_*` form (converting the overlay's exit arm to `SYS_EXIT`). Keep `build.rs` dependency-free. **Proof (un-gameable): inject a numeric off-manifest syscall → the build must FAIL the A4 gate** (paste the output), and the current tree (numeric `0`=`SYS_EXIT`, allowed) still builds green. **Rebase-fresh on `origin/main` FIRST**, run the full close-gate, and **paste the green push-CI URL** (this is host-only tooling — push-CI is the gate, no 4-shard QEMU; but never `[skip ci]` a build.rs change).
- **RAG** (PowerShell): `neo-std-pal build.rs scan_sys_tokens A4 gate`, `mt7601u syscall number SYS_ dispatch`, `ot61 pal manifest allowed_syscalls`. Health-check `Invoke-RestMethod http://127.0.0.1:8404/health` first; if down, note it + grep/Read (PL owns RAG startup; never a blocker).
