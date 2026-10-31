<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay #112: K1a RootDirCap review

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. **Seat:** Janus.
**Reviewed SHA:** `2922bf0cdb440759890074e4b8781171b23df052` (PR #168 squash).
**Branch:** `codex/k1a-rootdircap-review-janus`.
**Overall: FAIL. Request revert-first of #168 by the PM under the dispatch's post-merge rule.**
F1/F2 are newly exposed behaviors of the new RootDirCap/client integration. The inherited #166 findings
are recorded separately and are not the basis for this verdict. No implementation or main changes made.
All source references below are at the reviewed SHA.

## Finding

- [ ] **FAIL F1 — P1: directory operations use backing OpenFile rights instead of the attenuated RootDirCap handle rights.**
  The new root constructor opens its backing OpenFile with LOOKUP|READDIR|MOUNT
  (`kernel/src/sched/process.rs:1158`–`:1161`). A derive keeps that same OpenFile and attenuates only
  the Handle's rights (`process.rs:1071`–`:1081`). The new spawn policy admits that directory when
  its handle has GRANT (`:1209`). `CLIENT_ROOT_CAP_RIGHTS` explicitly omits MOUNT
  (`userland/libneo/src/lib.rs:1093`–`:1102`), but the backing object still has it.

  `sys_readdir` passes `FileRights::empty()` to file_lookup (`syscall.rs:6179`); mount and umount
  do likewise (`:6264`, `:6287`). file_lookup accepts that empty requirement and returns only the
  OpenFile (`:6503`–`:6506`), discarding the attenuated rights. `OpenFile::readdir_one` checks its
  own READDIR (`fs/mod.rs:416`), and mount/umount check its own MOUNT (`:437`, `:450`).

  Concrete input, no race required:
  1. With cap-rooted-open on, init derives its minted root with meta USE|DERIVE|GRANT|REVOKE and
     semantic LOOKUP only, then spawns a worker with two control handles followed by that cap.
     The new inheritance branch (`syscall.rs:1763`) accepts it and places the root at fd 2, so
     neither stdin nor stdout special handling is involved.
  2. In the worker, HANDLE_INFO(2) reports LOOKUP and no READDIR/MOUNT. Call
     `sys_readdir(FileHandle(2), valid_buffer_256, cursor=0)`. The current code enumerates the
     VFS root through the original backing object's READDIR; the handle should have refused it.
  3. For the actual client policy, take the root given by sys_spawn_client, derive a copy with the
     same client rights so it occupies a normal file fd, and call `sys_umount(copy, "k1a-check", 0)`.
     If privileged init previously mounted a child named k1a-check and that child has zero open
     handles, this succeeds through backing MOUNT although the client's handle lacks MOUNT.
     `sys_mount` similarly reaches filesystem construction without checking that handle right.

  The empty-required lookups predate #168, as does the generic file-cap attenuation mechanism.
  **The new exposure is minting and passing this shared, MOUNT-bearing root object to clients whose
  handle policy expressly removes MOUNT**, plus admitting further attenuated directory inheritance.
  This is distinct from #109's CAP_GRAPH lifetime/parking findings. A follow-up discriminator should
  check READDIR and MOUNT on derived/inherited root handles, including the normal client policy,
  against the handle's advertised rights. This case was source-traced, not guest-executed here.

- [ ] **FAIL F2 — P1: the new spawn directory-type check takes a preemptible NeoFS inode lock with IF=0.**
  Spawn holds the caller's handles lock inside without_interrupts (`syscall.rs:1743`–`:1763`) and
  calls the new `is_spawn_inheritable_dir_cap`. Its non-legacy branch evaluates `file.is_directory()`
  (`process.rs:1209`), which calls the inode's kind method (`fs/mod.rs:288`). For NeoInode this
  acquires plain `record: SwitchMutex<InodeRecord>` (`fs/neofs/inode.rs:1258`, `:1307`).
  Concrete interleaving with cap-rooted-open and browser-runtime enabled: two threads share a
  NeoFS directory opened LOOKUP|READDIR, whose handle receives GRANT/DERIVE meta rights. On CPU 0,
  thread A executes readdir IF-on (`syscall.rs:6183`), reaches the same inode's kind/record lock
  (`fs/mod.rs:413`, `inode.rs:1307`), and is timer-preempted while that guard lives. Thread B runs
  on CPU 0, calls spawn inheriting that directory, and spins at IF=0 in the new directory check
  waiting for A's record. A cannot run again to release it. No other CPU can unlock A's guard.
  The old exact LOOKUP|READDIR exception short-circuits this check; the new GRANT-bearing directory
  branch does not. This is a new CP-37/56 acquisition path, independent of #166. Cache/check directory
  type without nesting this preemptible inode lock under the IF-off handles section, and add a
  mixed-IF inheritance discriminator. Source interleaving only; no guest hang claimed reproduced.

## Dispatch checklist

1. [x] **PASS — new graph installs use the K1a-0 paths.**
   Root-form open records `Some(root_cap)` (`syscall.rs:6048`) and installs via
   `install_cap_excluding` (`:6056`), the reserved-fd variant of install_cap. It takes CAP_GRAPH
   (`process.rs:2004`), validates the parent (`:2012`–`:2016`), and records the edge (`:2024`).
   Client derivation uses SYS_DERIVE (`libneo/src/lib.rs:1115`), with graph lock and parent-linked
   insertion in `process.rs:2037`–`:2061`; inheritance is covered below. Init's initial root is
   deliberately root-minted, parent=None (`kernel/src/main.rs:433`–`:435`). All eight existing
   cap-graph KATs, including both source edge gates, passed on this exact tree. New call sites use
   existing guarded writers; the source gate does not independently prove every supplied parent value.

2. [x] **PASS — open versus revoke, for installed file reachability.**
   `root_dir_cap_for` snapshots identity/rights under handles (`process.rs:1175`–`:1188`) before
   the IF-on VFS operation (`syscall.rs:6017`). Install revalidates under CAP_GRAPH and maps
   ParentGone to EPERM (`:6059`–`:6060`). CPU A can resolve while CPU B revokes: if B finishes
   first, A's parent check refuses; if A installs first, B's guarded walk includes A's edge.
   No parent lock is retained across VFS work. Closing the parent before install likewise refuses.
   These guarantees are about the file capability returned; an already-started CREATE may still
   have a filesystem side effect before install is refused. No claim of rollback is made.

3. [x] **PASS — directory inheritance publication and ancestry; effective directory rights are F1.**
   The file branch clones to a fresh CapId with parent=source (`syscall.rs:1764`–`:1768`).
   `spawn_user` takes CAP_GRAPH and checks inherited parents (`process.rs:2762`–`:2768`), records
   their edges (`:2780`), registers (`:2790`), then constructs the thread (`:2793`). If revoke wins,
   spawn is refused; if spawn wins, the child is visible to revoke before it runs. Rights are copied
   through with_cap, not widened at the stored-bit level. Plain-file refusal remains at
   `syscall.rs:1771`; the old exact LOOKUP|READDIR own-directory exception remains at
   `process.rs:1208`. F1 concerns the consumers of these attenuated directory rights.

4. [ ] **FAIL — root resolver coverage passes; directory-cap consumers do not enforce the effective rights (F1).**
   Exact searches found two syscall paths into `fs::open_root_temporary`: open
   (`syscall.rs:6017`, checked at `:6012`) and model-load (`:2667`, checked at `:2624` before all
   boot-module/preloaded/NeoFS branches). Remaining production root opens are the kernel root mint
   (`process.rs:1161`) and boot/preload operations (`boot_modules.rs:613`, `:679`,
   `neofs_model.rs:107`, whose caller is boot preload at `boot_modules.rs:631`); test-harness calls
   are fixtures. No omitted user-facing absolute-path resolver was found.
   Readdir/mount/umount are **handle-based** APIs, not additional root-form resolvers:
   `syscall.rs:6167`, `:6297`, `:6308`. They need effective rights on that directory, rather than
   requiring a separate RootDirCap merely to use an inherited own-directory capability. Their
   empty-required lookups fail that obligation for the new root handles, as F1 shows.

5. [x] **PASS — SYS_HANDLE_INFO locking and the byte oracle.**
   Syscall entry supplies IF=0; the new body performs no IF-on operation before cloning the
   caller's occupant under handles (`syscall.rs:6936`). A concurrent close/reuse can occur before
   or after that clone, yielding one complete occupant's immutable kind/rights, not mixed fields.
   Encoding/copy occurs after guard release (`:6939`, `:6943`); no graph mutation is needed.
   Wrong length is EINVAL (`:6923`), oversized index EBADF (`:6929`), and an empty slot EBADF
   (`:6937`). The encoder writes LE version/kind/file-rights at offsets 0/4/8, meta/semantic/flags
   at 12/13/14, reserved zero at 15 (`handle_info_logic.rs:58`–`:66`). The independent decoder
   reads the same layout (`userland/libneo/src/handle_info.rs:68`–`:83`). Four existing literal
   oracle tests passed, including the RootDirCap record and all 15 kind codes. The oracle tests
   encoder/decoder layout; it does not test syscall close contention or operation-right enforcement.

6. [x] **PASS — stated falsifiers discriminate the tested open/parent mechanisms, with a wording qualification.**
   A write open succeeding for the read-only worker reports 13 (`neo-worker/src/main.rs:1627`–`:1631`),
   and init returns 200+13=213 (`neo-init/src/main.rs:7170`). A file surviving revoke reports 14
   (`neo-worker:1638`–`:1640`) and init returns 214 (`neo-init:7182`). A missing parent edge
   therefore fails even if the surviving file is at EOF: the probe requires exactly EBADF.
   The worker first proves a read open/read works, so general I/O failure cannot substitute for
   correct attenuation. Init completes revoke before sending the worker's go message (`:7174`,
   `:7177`); no timing lottery is needed. No worker operation reuses the file's fd during that leg.
   Removing **all** presence/rights gating coherently can fail earlier in the no-cap worker:
   its unexpected successful open reports 2 (`neo-worker:1606`), giving init exit 102. Thus 213
   specifically describes the read-only/write gate falsifier; any complete gate removal need
   not reach 213. PR #168 reports those two mutation runs; their exact temporary patches and
   executions were not reproduced here. Neither falsifier covers F1's directory operations.

7. [ ] **FAIL — required pitfall section exists, but misses the new mixed-IF inode acquisition (F2).**
   PR #168's CP table states CP-87/88 dependence and names cap_graph_revoke/cap_rooted_open,
   CP-37 for IF-off handle lookup, and CP-56/82 for IF-on VFS bodies. The new lookup's masking is
   explicit (`process.rs:1175`); open's VFS wrapper remains outside that lookup (`syscall.rs:6017`).
   The syscall-only handle-info lookup is also IF=0; no new IRQ acquisition was found. However,
   the table's "no new lock" rationale overlooks a new acquisition of an existing NeoFS record lock
   under the spawn handles guard (F2). No discriminator covers that mixed-IF inheritance case.
   #109's existing final-Arc shootdown and
   tensor-removal park findings remain dependencies requiring repair; this review does not reclassify
   their authorship or treat their mere presence as a newly introduced #168 failure.

8. [ ] **FAIL as an unqualified feature-off identity claim; the reported three-set comparison and seven scenarios are narrower.**
   With cap-rooted-open off **and browser-runtime on**, #168 still adds SYS_PROCESS_SELF and
   SYS_HANDLE_INFO constants, dispatch arms and bodies (`syscall.rs:171`, `:177`, `:1022`,
   `:1024`, `:6909`, `:6920`). The syscall module and kind encoder are compiled under the same
   OR condition (`sched/mod.rs:22`), so that supported configuration is not byte-identical to
   pre-#168 and exposes two new ABI operations. Concrete configuration: the existing std_threads
   browser-runtime scenario with cap-rooted-open absent. This is a qualification of the identity
   claim, not a separate unintended runtime defect: the PR explicitly intends the two browser APIs.
   PR #168 reports pinned-build-id .text/.rodata equality for default, desktop and usb-host versus
   #166, and seven feature-on passes: neo_shell_builtins, neo_shell_repl, arthur_console, arthur_act,
   arthur_memory, desktop_session_boot and arthur_compositor. The new spawn helper is wired into
   those callers (`libneo/src/lib.rs:1112`; `neo-shell/src/main.rs:205`, `:295`;
   `neo-init/src/desktop_session.rs:1770`; `arthur_compositor.rs:69`). These binary comparisons and
   seven extra-feature executions are author-reported evidence, not local reruns. The unqualified
   claim should be restricted to the measured feature sets.

## Validation and close

- [x] PASS: exact branch/SHA and clean start confirmed; full 24-file squash diff read. RAG healthy
  and queried, plus graft discovery; main-checkout hints were checked against exact local sources.
- [x] PASS: independently ran existing handle-info tests **4/4** and cap-graph tests **8/8** by direct
  pinned rustc (nightly-2026-04-16). No workspace Cargo success is claimed; this avoids the known
  unrelated uncached `gpt` dependency. No new tests or kernel changes were added.
- [x] PASS: independently queried [gate 40500773798](https://github.com/SecondStageTurbine/LogansOS/actions/runs/40500773798):
  success, all 22 jobs success including four QEMU shards. Its head is
  `4fb2502033d0ac0fdcfdb3864842af4de4336bd4`. GitHub's root-tree API and local git ls-tree agree
  exactly for kernel, userland, tools, Cargo.toml and Cargo.lock; thus those gated source trees
  match this squash despite distinct commit IDs. No new heavy QEMU gate was launched.
- [ ] FAIL: the existing gates do not check F1/F2's directory-rights/mixed-IF inputs. Request **revert-first**,
  then a repair and deterministic directory attenuation test before re-landing. The finding is
  source-established, with the guest execution limitation stated above.
