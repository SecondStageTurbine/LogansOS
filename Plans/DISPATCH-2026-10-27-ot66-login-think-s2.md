<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-66, login, session and a human user: design THINK → S2

**Why.** The MVP tier (`Plans/DECISION-2026-10-27-release-tiers-mvp.md`, PL decision 2026-10-27) includes **login**.
`docs/OPEN-THREADS.md` OT-66 records the gap. The kernel principal is `ProcessId`, and there's no human-user object:
- no credential store for OS sign-in;
- no login or lock screen;
- no session broker beyond DS-1's advisory one;
- no logout.

`kernel/src/init/boot_frame.rs` carries the standing TODO that the boot status line stands in for a login prompt. This
lane is a design THINK only. No code yet.

**Deliverable:** `docs/THINK-2026-10-27-ot66-login-session.md` (CC-BY-4.0 SPDX header), on branch
`claude/ot66-login-think` with a PR. It's docs only, so it needs no QEMU gate. Send the PM the PR number. A
cross-family read of the THINK follows before rung 1 is dispatched.

## Ground rules

- **Survey first** (CLAUDE.md, "Unfamiliar features: survey open source before writing from scratch"). Include a
  comparison table: candidate, what it does, license, last commit date and SHA (read from the repo itself), and what
  you'd take. Candidates to consider:
  - Fuchsia's account and session managers (BSD-3);
  - seL4/CAmkES or Genode's session and capability model (Genode is AGPL: principle only);
  - systemd-logind and PAM (LGPL/BSD: principle only for LGPL);
  - Redox's login and user model (MIT);
  - a password-hash crate such as `argon2` from RustCrypto (MIT/Apache-2.0).
  Only MPL-compatible code can be vendored, under `third_party/`. GPL, LGPL and AGPL are read for principle only.
- **Don't conflate.** `userland/vault-core/` (ADR-075 Phase A) is a password vault for the user's secrets.
  `kernel/src/net/identity.rs` (ADR-068) is NIC identity. Neither is OS sign-in, though the THINK may reuse their
  crypto.
- **Interaction with OT-87.** The installer (`docs/THINK-2026-10-27-ot87-installer.md`) creates the system disk. Say
  where first-boot user creation fits: in the installer, or a first-boot screen. Say what Try mode (read-only) does
  about users.

## Questions the THINK answers

1. **What is a user, in a capability OS?** The kernel's model is capabilities, not uid/gid. Propose the object: what
   it holds, who mints it, and how a session's processes inherit or are granted caps from it. Say how this closes the
   ambient `sys_spawn` gap the DS-1 THINKs left to OT-66 (`docs/THINK-2026-10-18-ds1-home-spine-shell.md`, the
   kernel `SpawnCap` rung).
2. **Credentials.** The password hash (algorithm, parameters, and where the cost is tuned on the Zenbook and
   OptiPlex), where it's stored on NeoFS, and how a wrong password, lockout and a forgotten password behave. For the
   MVP, a single local user is acceptable. Say what multi-user would need later, without building it.
3. **The screens.** The login screen, the lock screen (when it locks: idle, lid, a key), and logout, in the existing
   desktop (Atrium WM, `OP_*` protocol ADR-073). Say which process owns input while locked, so no other client can
   read keystrokes.
4. **Arthur and the user.** Arthur's memory file and model are per user or per machine. What Arthur sees before
   sign-in.
5. **A rung ladder** with a close gate per rung, scoped in dependencies, cycles and gates (no calendar estimates).
   Name the kill-switch for each policy rung (login off ≡ today's boot straight to the desktop). Mark which rungs
   change the kernel's cap model: those need the concurrency pitfall-bank pass and a cross-family AAG.
6. **Security review routing.** Which rungs need Muse (security review of Claude-authored code, per CLAUDE.md's
   cross-family routing).

## Setup

```
WORKTREE: cloud session S2 (your own clone)
BRANCH:   claude/ot66-login-think
          git fetch origin && git checkout -B claude/ot66-login-think origin/main

git show origin/main:Plans/DISPATCH-2026-10-27-ot66-login-think-s2.md       # this file
git show origin/main:Plans/DECISION-2026-10-27-release-tiers-mvp.md
git show origin/main:docs/THINK-2026-10-27-ot87-installer.md
```

RAG isn't available in the cloud session. Cite code by `file:line`. Before ending, `git status --porcelain` and
`git log @{u}..` are both empty.
