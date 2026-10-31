<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #111): security review of K1a, the root-directory capability (#168) → Muse (Gwen)

**MODEL:** Muse (Gwen). **Why:** K1a turns "any process can open any path" into "only a holder of a root-directory
capability can". That's the base of the browser's file sandbox (OT-61 C2.1) and the login design's data protection
(OT-66 K1). Claude-authored (S4); security reviews of Claude-authored code go to Muse.

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL).
**Deliverable:** a RETURN. Muse can't push, so the PL pastes or uploads it and the PM saves it as
`Plans/RETURN-2026-10-27-k1a-rootdircap-review-muse.md`.

**Reviewed SHA:** `2922bf0c` on `main`, #168's squash (PR head `bfb8f37b` plus a merge of main). Gate: batch 10, run
`40500773798`, all jobs green. Read the files at that SHA from the repo, not a summary:
- `kernel/src/fs/mod.rs`, `kernel/src/arch/x86_64/syscall.rs` (the root-form open, `sys_model_load`, spawn
  inheritance, `SYS_HANDLE_INFO`, `SYS_PROCESS_SELF`) and `kernel/src/main.rs`;
- `kernel/src/sched/process.rs` and `handle_info_logic.rs`;
- `userland/neo-init/src/main.rs` (minting the cap and deriving one for Arthur and the shell) and `userland/libneo`;
- `kernel/Cargo.toml` (`cap-rooted-open`);
- `docs/THINK-2026-10-27-ot66-login-session.md` §3 and §8 K1 (on branch `claude/ot66-login-think`), for intent.

**What it does** (feature `cap-rooted-open`, default off ⇒ kernel byte-identical):
- `RootDirCap` is an ordinary directory file handle on "/", minted only at boot into init.
- The root-form `SYS_OPEN`, `READDIR`, `MOUNT`, `UMOUNT` and `sys_model_load` require it. Without it they return
  EPERM.
- An opened file's rights are at most the cap's, and the file is the cap's child, so revoking the cap reaches it (this
  relies on K1a-0, #166).
- Spawn inheritance is widened to directory caps only, never plain files.
- init derives a cap for Arthur and the shell.
- `SYS_HANDLE_INFO` returns the caller's own handles. `SYS_PROCESS_SELF` returns the caller's pid.

## Checklist

Answer each item PASS or FAIL, with the `file:line` that enforces it. For a FAIL, give a concrete sequence of calls.

1. **No root-form open without the cap.** With the feature on, every path that resolves from "/" (open, readdir, mount,
   umount, model load, and any other caller of the root resolver) requires the cap. Name any path that still resolves
   ambiently.
2. **Minting.** The root cap is created once, at boot, into init only. No syscall mints another. A forged handle index,
   or a handle of another kind, is refused.
3. **Rights never widen.** A file opened through the cap gets at most the cap's rights. A read-only derived cap can't
   open for write (the scenario's EACCES leg).
4. **Revocation reaches opened files.** Revoking the cap makes files opened through it EBADF (the scenario leg, and the
   second falsifier, which FAILs at 214 when the file isn't parented).
5. **Spawn inheritance.** The widened rule admits directory caps only, never plain files (the EPERM leg). A child can't
   gain more than its parent passed.
6. **`SYS_HANDLE_INFO` and `SYS_PROCESS_SELF`.** Each reads only the caller's own state and grants nothing. Out-of-range
   and empty indices give EBADF. Check the hand-laid byte oracle for the HANDLE_INFO layout (the flags byte with
   ROOT_DIR).
7. **Feature off.** `.text` and `.rodata` are identical to main with `NEO_BUILD_ID` pinned. Ambient open is unchanged.
8. **Coverage.** Name what the gate doesn't cover. For example: a third-party process spawned by Arthur, or the browser
   process (C2.1b), neither of which exists yet.

## Close condition

Give the RETURN to the PL with PASS/FAIL per item, `file:line`, and an overall verdict (PASS or FAIL). The PM saves it
under `Plans/`.
