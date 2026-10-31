<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Arthur arc-2 Slice 2 — the capability-bounded action surface → Janus

**Date:** 2026-06-17
**From:** PM (Claude)
**To:** Janus (built Slice 1; full context). New branch off `origin/main` HEAD: `codex/phase-8-arthur-slice-2-action-surface-janus`.
**Status:** DISPATCHED. **Pure userland — NO kernel change** (the ratified §10.1 finding: file-write already exists, so OD-2 is moot). Builds the **security spine** of Arthur: the capability-checked action surface that turns an intent into a *bounded* effect.

## 0. The decision this build makes
Build the **"model proposes, capability disposes" disposition layer** as a typed userland surface: a `libneo` action catalog where every action maps NL-intent → a **capability-checked** syscall sequence, refusing anything the caller wasn't granted. Slice 1 gave the REPL; Slice 2 gives the *bounded hands*. This is Article 29.3's security keystone made concrete — **Arthur reaches only what it was granted, enforced, not promised** — and it's built **before** Arthur exists (Slice 3) so the bound precedes the actor.

## 1. Authoritative design — RATIFIED (read first, build to it)
```
git fetch origin
git show origin/main:Plans/THINK-2026-06-16-arthur-userland-arc-and-architecture.md   # §3 (action surface), §6 Slice 2, §10 (ratified — esp. §10.1 OD-2 MOOT)
git show origin/main:docs/VISION-arthur.md                                              # "model proposes, capability disposes"
```
Binding ratified calls:
- **OD-2 is MOOT — NO kernel change.** §10.1: `sys_write_impl` (`syscall.rs:2168`) already dispatches non-stdout fds to `sys_file_write_impl` (`syscall.rs:2406`), a capability-checked `FileRights::WRITE` VFS write. The `write_file` action uses the **existing** ABI. Do **not** add or extend a syscall.
- **Slice 2 is pure userland** — `libneo` action wrappers over existing syscalls (§10.3). The kill-switch is the per-action **deny path**, not a syscall fallback.

## 2. ISCs
- **ISC-1 — The action catalog (`userland/libneo/src/action.rs`, ~300 LOC).** A typed `Action` enum + `execute_action()` dispatcher covering the v1 catalog (§3C): `read_file`, `write_file`, `list_dir`, `spawn_process`, `kill_process`, `inspect_process`, `wait_process`, `net_dial`, `net_send`, `net_recv`, `infer`, `load_model`. Each arm maps to its existing syscall sequence + the **required capability/rights** (e.g. `read_file` → `SYS_OPEN`+`SYS_READ`+`SYS_CLOSE` gated on `FileRights::READ`; `write_file` → the existing FileHandle write gated on `FileRights::WRITE`).
- **ISC-2 — "Model proposes, capability disposes" at the wrapper.** `execute_action` checks the granted capability/rights **before** the syscall; on insufficient authority it returns a structured **`Denied`** (never silently nothing, never a panic) so the caller can report *"Denied: no WRITE cap for /path."* The wrapper is the disposition layer; the kernel capability check is the enforcement (the wrapper never bypasses it — a missing cap fails at the syscall too, so the wrapper's check is *defense-in-depth + a clean error*, not the security boundary itself).
- **ISC-3 — The grant/deny gate.** Unit/scenario tests: spawn a child granted a **read-only** file cap; assert it **can** `read_file` and is **Denied** on `write_file`; exercise each action's grant *and* deny path. The deny path is the load-bearing test — it proves the bound holds.
- **ISC-4 — Standards.** Host-testable dispatcher logic in a `*_logic` split; **kill-switch:** the action surface is additive — not invoked unless a caller (Slice 3's Arthur) uses it, so default behavior is unchanged (≡ today). `fmt`/`clippy -D warnings` on touched crates; **no new `unsafe`/invariant; NO kernel change**; reuse Slice 1's spawn/IPC + existing syscalls. ANSPEC §11.x note.

## 3. Gate
- **Host:** the `execute_action` dispatcher logic unit-tested incl. every deny path; builds clean; `fmt`/`clippy`.
- **QEMU scenario `arthur_action_surface`:** in the running OS, a process with a scoped cap bundle reads a granted file, is **denied** a write it wasn't granted, lists a granted dir, spawns a capability-bounded child — the grant/deny spine exercised end-to-end at NEO_SMP=4.
- **No kernel change**, but the new QEMU scenario needs QEMU validation that branch-push CI skips ⇒ **run the pre-merge `workflow_dispatch` QEMU gate green before declaring close** (same coverage-gap guard).

## 4. Scope guard
The `libneo` action catalog + the capability-checked dispatcher + the grant/deny tests **only**. **Do NOT** build the Arthur process (Slice 3), intent parsing (Slice 4), on-demand cap escalation (Slice 5), inference wiring, or any kernel change (OD-2 moot). Reuse Slice 1 + existing syscalls. No new `unsafe`/invariant.

## 5. Merge (PM)
Close-gate: rebase onto current `main`; re-run host dispatcher tests + the `arthur_action_surface` scenario; **run the pre-merge QEMU gate green**; push (CI — `userland/` triggers it). PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (Article 13 — **Arthur arc-2 Slice 2: the capability-bounded action surface** — + ANSPEC §11.x). Next: Slice 3 (the Arthur process — depends on 8.5, now landed).

**Disk reclaim (after PM confirms merge):** once on `origin/main` and PM-confirmed, delete this build's tree (`git worktree remove` / folder delete; never an unmerged tree). See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Janus
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-17-arthur-slice-2-action-surface-janus.md` (`git show origin/main:<path>`).
- **Branch:** `codex/phase-8-arthur-slice-2-action-surface-janus` off `origin/main` (fresh; has Slice 1).
- **One-line:** Build **the capability-bounded action surface** — Arthur's security spine, **Slice 2** (read the RATIFIED design: `git show origin/main:Plans/THINK-2026-06-16-arthur-userland-arc-and-architecture.md` §3/§6/§10). **PURE USERLAND — no kernel change** (§10.1: OD-2 is moot, file-write already exists via `sys_file_write_impl`). Add `userland/libneo/src/action.rs` (~300 LOC): a typed `Action` enum + `execute_action()` mapping each v1 action (`read_file`/`write_file`/`list_dir`/`spawn`/`kill`/`inspect`/`wait`/`net_*`/`infer`/`load_model`) to its existing syscall sequence + required capability, checking the grant **before** the syscall and returning a structured **`Denied`** on insufficient authority ("model proposes, capability disposes"). Gate: host dispatcher tests incl. **every deny path** + the `arthur_action_surface` QEMU scenario (granted read OK, ungranted write Denied, scoped spawn) at NEO_SMP=4 + **run the pre-merge QEMU gate green before close** (the new scenario needs QEMU). Kill-switch: additive surface, unused ≡ today. **NO Arthur (Slice 3), NO intent (Slice 4), NO escalation (Slice 5), NO kernel change.** fmt/clippy clean, no new `unsafe`.
