<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay #126: K1a fixes re-check (#174)

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. **Seat:** Janus.
**Reviewed SHA:** `57b7960e96b55688bb1e89d975c7f183d2e20806` (#174 squash).
**Branch base:** `origin/main` at `b69b5ee2fa151974e4831b6b3a24cbd250c30611`.
**Overall: PASS for the dispatched #112 F1/F2 re-check.** No remaining failure found in this scope. All source references below are at the reviewed SHA unless explicitly labeled pre-fix. This does not reopen the other findings in RETURN #112.

1. [x] **PASS — F1: effective handle rights reach every affected consumer; type errors retain precedence.**

   `file_lookup_with_rights` snapshots the OpenFile and the same handle's rights together under the handles lock (`kernel/src/arch/x86_64/syscall.rs:6498-6513`). The backing object can remain shared by derives; the consumer now checks its rights intersected with the handle's rights.

   | Former empty-rights lookup | Enforcement at #174 |
   |---|---|
   | `sys_read` | Lookup at `kernel/src/arch/x86_64/syscall.rs:6099`; `read_as(..., rights)` at `:6110`; regular/model type check then READ intersection at `kernel/src/fs/mod.rs:378-385` |
   | `sys_readdir` | Lookup and rights forwarding at `kernel/src/arch/x86_64/syscall.rs:6179-6183`; directory type check then READDIR intersection at `kernel/src/fs/mod.rs:438-449` |
   | `do_mount` | Lookup and rights forwarding at `kernel/src/arch/x86_64/syscall.rs:6264-6274`; directory type check then MOUNT intersection before mount construction at `kernel/src/fs/mod.rs:466-473` |
   | `do_umount` | Lookup and rights forwarding at `kernel/src/arch/x86_64/syscall.rs:6288-6292`; directory type check then MOUNT intersection before mount-table mutation at `kernel/src/fs/mod.rs:479-487` |
   | `debug_sys_readdir_kernel_buf_for_test` | Same rights forwarding at `kernel/src/arch/x86_64/syscall.rs:6470-6474`; same `readdir_one` enforcement |
   | `sys_fsync` | Deliberately remains rights-free at `kernel/src/arch/x86_64/syscall.rs:6381-6389`; rationale below |

   The old concrete inputs now return EACCES: a LOOKUP-only derived directory cannot enumerate; a client-policy root without MOUNT cannot mount or unmount even an existing idle child; and a WRITE-only derived ordinary-file handle cannot read a nonempty buffer. The guards precede the inode read, readdir, filesystem construction, and unmount mutation. Wrong-type reads return EISDIR and wrong-type directory operations return ENOTDIR before the new rights error (`kernel/src/arch/x86_64/syscall.rs:6540-6542`); existing argument-validation and stale-handle errors retain their earlier positions.

   An exhaustive comparison found all six empty-required sites listed in #174's PR body. Only fsync remains empty-required. `sys_write` and `sys_ftruncate` already require handle WRITE at lookup (`kernel/src/arch/x86_64/syscall.rs:6337`, `:6394`).

   **Rights-free fsync is safe for this authority boundary.** It accepts a live File handle, validates its generation, and delegates synchronization (`kernel/src/fs/mod.rs:350-352`); it exposes no file bytes and takes no caller-selected write data, path, or new length. NeoFS permits only regular-file fsync (`kernel/src/fs/neofs/inode.rs:1432-1436`) and drains already-pending writes under the mount's entries lock (`:584-595`). It can force mount-wide I/O and persistence, including pending writes made through other authorized handles, but it grants no READ, WRITE, READDIR, or MOUNT operation. No distinct fsync right is defined by this change. This is an assessment of the handle-authority boundary, not a review of NeoFS's commit protocol.

2. [x] **PASS — F2: no inode lock remains reachable inside spawn's IF-off inheritance section.**

   OpenFile construction records `inode.kind()` in `is_dir` (`kernel/src/fs/mod.rs:264-286`); the inheritance accessor is now a plain field read (`:296-301`). Both the exact LOOKUP|READDIR own-directory case and the GRANT-bearing directory case require that cached flag (`kernel/src/sched/process.rs:1206-1211`). A plain file with own-directory-shaped rights therefore cannot use the old short circuit.

   I traced the entire masked section at `kernel/src/arch/x86_64/syscall.rs:1743-1785`. `HandleTable::get` clones an entry (`kernel/src/sched/process.rs:1474-1475`), File holds an `Arc<OpenFile>` (`:816-819`), `cap_id`/`rights` inspect fields (`:943-995`), allocation of a CapId is atomic (`:102-105`), and the File arm of `with_cap` only transforms rights bits while retaining the Arc (`:1063-1081`). None calls an Inode method or takes OpenFile's generation/offset locks. The source entry remains pinned by the locked handle table while temporary File clones are dropped, so those drops cannot be the final OpenFile/inode release. The inherited vector outlives the masked closure. Child construction starts after it at `kernel/src/arch/x86_64/syscall.rs:1790`.

   The original preempted NeoFS-record-holder interleaving consequently has no inode-lock acquisition to wait at in this section. This conclusion is source-established; the new steps 7–9 exercise rights, not a mixed-IF scheduler interleaving.

3. [x] **PASS — steps 7, 8, and 9 discriminate the pre-fix rights bugs.**

   Compared with parent `d72d4470e7c6b4521f4c2480da7e4bac7c39df45`:

   | Step and source at #174 | Pre-fix behavior, evaluated independently | Fixed behavior required |
   |---|---|---|
   | 7: `userland/neo-init/src/main.rs:7203-7221` | LOOKUP-only derive shares the root's READDIR-bearing OpenFile. Old `syscall.rs:6179-6183` discards handle rights and old `fs/mod.rs:411-419` checks only backing READDIR. Enumeration or end-of-directory is not EACCES, so step returns **16**. | Derived handle gets EACCES; original root still enumerates. |
   | 8: `userland/neo-init/src/main.rs:7222-7247` | Client-policy derive lacks MOUNT but shares the root's backing MOUNT. Old `syscall.rs:6287-6291` and `fs/mod.rs:446-454` reach the mount table; the absent child gives ENOENT rather than EACCES, so step returns **19**. Independently, the mount subprobe also passes backing MOUNT and would fail its EACCES expectation at exit **20**. | Both client operations get EACCES; original root passes the rights checks. Its mount need not succeed for this permission discriminator. |
   | 9: `userland/neo-init/src/main.rs:7248-7264` | WRITE-only derive shares a READ|WRITE OpenFile. Old `syscall.rs:6099-6110` discards handle rights and old `fs/mod.rs:359-373` reads with backing READ. Even EOF returns a nonnegative value, so step returns **25**. | Derived handle gets EACCES; original file still reads. |

   Step 8's copied policy matches `userland/libneo/src/lib.rs:1096-1102`. In one pre-fix scenario run, step 7 aborts first; exits 19 and 25 describe their respective later probes if reached or checked separately. The red outcomes above were source-traced, not guest-executed locally.

Validation: RAG health was healthy (`ollama=true`, `qdrant=true`); graft discovery and pinned direct reads were used. Read the dispatch, RETURN #112, #174's PR body, the complete five-file fix, and the affected callers. GitHub reports gate [40787440872](https://github.com/SecondStageTurbine/LogansOS/actions/runs/40787440872) completed successfully at `679fd739fe73777537198f6a3d6a76607b8f13ee`. The four code files changed by #174 match that gated head; other source files differ, so whole-tree identity is not claimed. No build, local QEMU run, mutation run, or new gate was launched for this documentation deliverable.
