<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: fix the two FAILs in Janus's review of K1a RootDirCap (#168) → S5

**Why.** Janus (GPT-6.1-Sol high) FAILs #168: `Plans/RETURN-2026-10-27-k1a-rootdircap-review-janus.md`. The PM
re-checked F1 at `2922bf0c`, and it holds. That contradicts Muse's PASS on #111 item 1, which assumed readdir checks
the handle's rights.

The code is behind `cap-rooted-open` and `file-caps`, which no product image builds. So this is fix-forward with no
deadline, but it must land before `cap-rooted-open` goes into any image. S4 wrote #168 but is on the #166 fix, so
this contained fix comes to you.

## F1: readdir, mount and umount check the backing `OpenFile`'s rights, not the handle's

- **The finding.** A derive keeps the same `Arc<OpenFile>` and narrows only the handle's rights
  (`kernel/src/sched/process.rs:1068–1081`). But `sys_readdir` (`syscall.rs:6179`) and mount/umount (`:6264`,
  `:6287`) call `file_lookup(fd, FileRights::empty())`, which drops the handle's rights (`:6503–6506`). Then
  `OpenFile::readdir_one` (`fs/mod.rs:416`) and `mount_child` / `umount_child` (`:437`, `:450`) check the
  `OpenFile`'s own rights. So a handle with LOOKUP only can readdir the root, and a client root without MOUNT can
  umount.
- **Fix.** Pass the right each operation needs to `file_lookup` (READDIR; MOUNT for mount and umount), so the handle's
  rights are checked. Keep the `OpenFile` check as a second layer. Grep for every other `file_lookup(…, empty())` and
  say for each whether the operation needs a right.
- **Discriminator (QEMU, in `cap_rooted_tests`):**
  - A derived root with LOOKUP only gets EACCES on readdir.
  - A client-policy root copy (`CLIENT_ROOT_CAP_RIGHTS`, no MOUNT) gets EACCES on mount and umount.
  - The full root still succeeds.
  - Show the leg fails on today's code first.

## F2: the spawn directory check takes a preemptible inode lock with IF=0

- **The finding.** Spawn holds the caller's handles lock inside `without_interrupts` (`syscall.rs:1743–1763`) and
  calls `is_spawn_inheritable_dir_cap`. That calls `file.is_directory()` (`process.rs:1209`), which takes
  NeoInode's `record: SwitchMutex` (`fs/neofs/inode.rs:1307`).
- **How it hangs.** A thread preempted while holding that record lock (in readdir, with IF on) never runs again on
  that CPU while spawn spins at IF=0.
- **Fix.** Don't take an inode lock under the IF-off handles section. Record "is a directory" in the `OpenFile` when
  it's opened, since an inode's kind never changes, and read that field instead. Muse's #111 item 5 nit
  (`is_directory()` in the own-dir branch) then comes for free from the same cached field.
- **Pitfall-bank table:** CP-37 and CP-56, plus a new entry if this class isn't in the bank.

## Rules

- One PR, branch `claude/k1a-rootdircap-fixes`, rebased onto `origin/main`. Run `scripts/worker-close-gate.sh`.
- Kill-switch: with `cap-rooted-open` off, `.text` is identical to `main` (`NEO_BUILD_ID` pinned). The F1 change sits
  on shared syscall paths, so if `.text` moves, give both hashes and explain.
- Send the PM the head SHA from `git ls-remote`. Review after merge: Janus narrow re-check (GPT-6.1-Sol medium).

```
WORKTREE: cloud session S5 (your own clone)
BRANCH:   claude/k1a-rootdircap-fixes
          git fetch origin && git checkout -B claude/k1a-rootdircap-fixes origin/main

git show origin/main:Plans/DISPATCH-2026-10-27-k1a-rootdircap-fix-s5.md
git show origin/main:Plans/RETURN-2026-10-27-k1a-rootdircap-review-janus.md
git show origin/main:Plans/RETURN-2026-10-27-k1a-rootdircap-review-muse.md
```

RAG isn't available in the cloud session. Cite code by `file:line`. Before ending, `git status --porcelain` and
`git log @{u}..` are both empty.
