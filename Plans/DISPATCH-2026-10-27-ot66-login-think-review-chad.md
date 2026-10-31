<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #98): cross-family read of the OT-66 login / session THINK (#157) → Chad

**MODEL:** GPT-6-Luna. **EFFORT:** high. It's a design THINK with no code, but it sets the cap model for users and
two kernel rungs (K1 cap-rooted open, K2 SpawnCap). Claude-authored (S2), so it gets a Codex-family read before rung
L1 is dispatched. **Do this after relay #95.**

**Type:** design review. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `d49acd0f` (PR #157, branch
`claude/ot66-login-think`). If the PR has merged by the time you start, review the same file on main.

**What it proposes.**
- **The user.** A user is a userland account record; the kernel keeps no uid. A session is the set of caps the WM
  broker hands out at spawn.
- **Two kernel rungs** close today's ambient gaps: K1 makes open cap-rooted (`open_at` plus `RootDirCap`), and K2 adds
  SpawnCap. Until they land, the password protects the screen, not the data, and §2 says so.
- **Credentials.** Argon2id (vault-core's `argon2` crate, floor 64 MiB, t=3, p=1). A two-slot `NEOCRED1` record with
  a generation and a checksum, plus a byte oracle. Wrong passwords get a growing delay, never a lockout.
- **Screens.** Login, lock and logout are WM chrome. While locked, the WM holds the only focus and composites no
  client surface.
- **Ladder.** L1, L2, K1, K2 and L3 are the MVP; L4 and L5 come later.

**PM answers to §10 (recorded here; the THINK should adopt them).**
- **Q1, the verifier:** in the WM for the MVP. The WM already sees every keystroke, so a separate process adds no
  protection until input routing changes.
- **Q3, K1's scope:** port the product call sites only. Test drivers keep the root form through a test-only grant,
  which must be impossible to build into the product or image-A kernels.
- **Q2 (recovery code) and Q4 (idle-lock default) go to J.c.** Review the THINK's handling of both options; don't
  pick one.

## Setup

```
WORKTREE: E:\AI Automation\workers\chad     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot66-login-think-review-chad
          git fetch origin && git checkout -B codex/ot66-login-think-review-chad d49acd0f

git show origin/main:Plans/DISPATCH-2026-10-27-ot66-login-think-review-chad.md   # this file
git show origin/main:Plans/DISPATCH-2026-10-27-ot66-login-think-s2.md            # the dispatch
git show d49acd0f:docs/THINK-2026-10-27-ot66-login-session.md                     # the THINK
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='sys_spawn root open fs mod temporary SAK_KEYS focus'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line` (code or THINK line). For any FAIL, say what's missing or wrong.

1. **The "what exists today" claims are true.**
   - `sys_spawn` is ambient.
   - The root open at `kernel/src/fs/mod.rs:1186` is the temporary ambient path the THINK says it is.
   - There's no uid in the kernel.
   - `vault-core` is a password vault, not OS sign-in.
2. **§2's threat table is honest.** For each threat listed, the "protected before K1/K2" and "protected after"
   columns match what the code would actually enforce. Name any threat that's missing (for example, a process
   spawned before login, Arthur's tools, a boot from Try reading the installed disk).
3. **K1 and K2 close the gaps they claim to.**
   - After K1, a process without a `RootDirCap` can't open any path.
   - After K2, a process can't spawn without a SpawnCap.
   - The test-only grant in Q3 can't reach a product build: check §10.1's three layers (the feature, the
     `compile_error!` combinations, and the xtask refusal in every Cargo spelling) against the write-guard-control
     precedent at `kernel/src/drivers/block/mod.rs:505-516`.
   - Say which existing syscalls would still be ambient after both rungs.
4. **`NEOCRED1`.**
   - The two-slot layout survives a torn write: the generation and checksum pick the newer valid slot, and a power
     loss mid-write never leaves both invalid.
   - The THINK commits to a hand-laid byte oracle (CLAUDE.md's format rule).
   - Argon2id parameters are never below the stated floor, and the tuning can't lower them.
5. **The lock screen.** While locked, no client receives a key or pointer event and no client surface is
   composited. Say whether any existing path (Arthur's busy emblem `OP_BUSY` from #155, the SAK keys, a client
   created during lock) could draw or receive input anyway.
6. **Wrong passwords.** The growing delay can't be skipped by restarting the WM or the login screen, and it can't be
   turned into a denial of the real user's login. Say where the delay state is kept.
7. **Fit with OT-87.** First-boot account creation and Try having no account match the installer THINK
   (`docs/THINK-2026-10-27-ot87-installer.md`, R5: Try mounts the installed disk read-only).
8. **The ladder.** Each rung has dependencies, a close gate and a kill-switch (`login` off ≡ today's boot), with no
   calendar estimates. The cap-model rungs are marked for the pitfall bank and a cross-family AAG, and Muse is named
   for the security rungs.
9. **Prior art.** §11's survey follows CLAUDE.md's "Unfamiliar features" rule: licenses, last commit dates and SHAs,
   nothing GPL vendored, and the unverified Fuchsia row is clearly marked.

## Close condition

- Commit `Plans/RETURN-2026-10-27-ot66-login-think-review-chad.md` on your branch and **push it**, with PASS/FAIL per
  item and an overall verdict. Paste the SHA to the PL.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
