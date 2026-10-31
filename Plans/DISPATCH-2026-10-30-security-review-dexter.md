<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RELAYS #185–#186 → Dexter: two security reviews of Claude-authored code (standing in for Muse)

**Why Dexter.** Security reviews of Claude-authored code go to a different model family; the Codex models refuse
them. Muse is unavailable, and the DeepSeek family is cross-family against Claude (CLAUDE.md §Cross-model diversity).
Read the files at the SHAs below from the repo, not from a summary. Each item: PASS or FAIL, the enforcing
`file:line`, and for a FAIL a concrete sequence of syscalls or inputs.

## #185: input-route ACTIVATE vs revoke (#210, merged as `ff4bdd51`). Slug `input-activate-revoke`

Muse's #167 found item 3 FAILing: a minted ACTIVATE outlives the revoke of the owner's capability. Her text wasn't
captured. Product security (CP-96): ACTIVATE decides which process gets the keyboard. Files: `kernel/src/arch/x86_64/syscall.rs`
(`mint_activator` and the route syscalls), `kernel/src/sched/process.rs` (capability revoke), `kernel/src/main.rs`.
Review current `origin/main`.
1. Can a process that isn't the route's owner (the WM or the session broker) get ACTIVATE on a route by any path:
   minting, deriving, a handle transfer over a channel, or a spawn grant?
2. Is a process holding only RECEIVE refused ACTIVATE, with a defined errno, and the route left unchanged?
3. **Does revoking the owner's capability end every ACTIVATE minted or derived from it?** If not, give the exact
   sequence (who mints, who revokes, what the holder can still do afterwards) and say what the fix should key on:
   the minted handle's parent in the capability graph, a route epoch, or something else.
4. Does the scenario's negative leg fail with the old rule?

## #186: login in the product image (#224 `45798eb3`, gating now). Slug `product-login`

The product now boots through the WM's login (`kernel-rng`, `sak-lock`, `login`). Files:
`userland/neo-init/src/desktop_session/login.rs`, `userland/neo-init/src/desktop_session.rs`,
`kernel/src/rng/mod.rs`, the `sak-lock` route in `kernel/src/input/focus.rs`, and `tools/xtask/src/main.rs` (the
pairing test). Read #224's head: `git fetch origin +pull/224/head:pr224`.
1. Is the password verifier stored only in the derived form (no plaintext, no reversible encoding), with the salt
   from `kernel-rng`?
2. Is the comparison constant-time, and does the wrong-password backoff hold across a reboot?
3. Is the secure-attention key (Super+L / the SAK chord) unforgeable by any userland process: can a client inject it,
   or read keystrokes typed while the lock or login screen is up?
4. **J.c.'s decision, so not a FAIL:** sign-in is skipped when `.system/` is missing or the record file can't be
   opened for writing (`login.rs:118–145`). Check only that the skip can't be **triggered** by an unprivileged process
   on a writable volume (for example by deleting or locking a file it shouldn't be able to touch).
5. Without `kernel-rng`, does first boot fail closed?

## Delivery

WORKTREE `E:\AI Automation\workers\dexter`. One pushed RETURN per relay:
`Plans/RETURN-2026-10-30-<slug>-review-dexter.md`, on a `dexter/` branch. Paste both SHAs.
