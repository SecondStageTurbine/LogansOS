<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — #240 mount authority: security review (Section 195)

**Seat:** Dexter · **Reviewed at:** `#240` head `6f2ef641` (`pr240`)
**Branch:** `dexter/mount-authority-review-dexter` · **Base:** `origin/main` = `188fe40e`
**Answers:** `Plans/DISPATCH-2026-10-30-mount-authority.md`; finding `Plans/RETURN-2026-10-30-login-recheck-dexter.md` item 3a
**RAG:** healthy (`{"status":"healthy","ollama":true,"qdrant":true}`). All sources read from the `pr240` tree.
**Scope:** code review of `git diff origin/main...pr240` (13 files, +327/-3) at `6f2ef641`; I ran no gate.

## Verdict

| # | Item | Verdict |
|---|---|---|
| 1 | Only the authority holder can mount or unmount; no `MOUNT` right survives a plain open | **PASS** |
| 2 | The authority cannot be derived, granted, spawn-inherited, or sent over IPC | **PASS** |
| 3 | In image A (feature + pairing assert); boot and installer mounts still work | **PASS** |
| 4 | Scenario: the refused unmount leaves the volume mounted; its control fails | **PASS** |

---

## 1. Only the authority holder can mount or unmount — PASS

**There is one process-reachable path, and it checks the authority before it touches a handle.**

- `SYS_MOUNT`/`SYS_UMOUNT` dispatch to `do_mount`/`do_umount` (`kernel/src/arch/x86_64/syscall.rs:1135-1136`).
- `do_mount` refuses at the top, before parsing or `file_lookup_with_rights` (`syscall.rs:6662-6665`); `do_umount` likewise (`:6690-6693`). Both call `caller_holds_mount_authority` (`:8835-8838`), which asks `may_mount` (`:8844-8847`), which scans the caller's table for a `MountAuthority` with `USE|MOUNT` (`kernel/src/sched/process.rs:2263-2273`).
- No second entry: `mount_child`/`umount_child` (`kernel/src/fs/mod.rs:536-566`) are called only from those two sites (`syscall.rs:6674`, `:6702`). The test-only shims `debug_sys_mount_for_test`/`debug_sys_umount_for_test` (`syscall.rs:6736-6744`) route through the same functions, so they inherit the check too. `debug_vfs_mount_at_parent` (`fs/mod.rs:1283`) bypasses it, but is unreachable from a process — it has no callers in the tree and is compiled only under `neo-test-phase3-mount-table`.
- The kernel's own mount-table mutations are not process operations: boot (`kernel/src/main.rs:943-944` → `fs/mod.rs:1024-1048`, `:1051-1075`) and the installer (`kernel/src/installer.rs:830`, `:844`, `:873`, `:883`) call `install_root_child`/`NeoFs::mount_*` directly. They are gated by the boot path / the `Installer` cap, not by this authority; they do not open a bypass.

**A plain open no longer yields `MOUNT`.**

- `open_path_and_install` masks `MOUNT` out of the requested rights unless `may_mount` (`syscall.rs:6400-6409`) — before the OpenFile is resolved (`:6418-6419`) and before the handle rights are computed (`:6441-6456`). `MOUNT_RESERVED` is the same bit (`fs/mod.rs:59-60`), so one mask covers both spellings.
- `OpenFile::new_with_mount` stores that (masked) requested set as the file's rights (`fs/mod.rs:1473`, `:1478`), so the independent right check inside `mount_child`/`umount_child` (`fs/mod.rs:547`, `:561`) would also refuse a non-holder if the authority check were removed. Both gates are closed.
- The mask is not the load-bearing gate (the authority check is first), but it is correct: delete it and the scenario still passes, which is only a falsifiability gap (note 2 below), not a hole.

## 2. The authority cannot spread — PASS

- Rights are `USE` + `MOUNT`, with no `DERIVE` and no `GRANT` (`kernel/src/sched/process.rs:211-220`, `:365-373`; minted with those rights at `:1376-1381`).
- **Derive:** `derive_handle` requires `DERIVE` on the parent (`process.rs:3306-3312`) → `EPERM`.
- **Grant:** `sys_grant_impl` refuses a source without `GRANT` (`syscall.rs:7920-7923`) and names `MountAuthority` explicitly, failing the pending receive rather than wedging it (`:7937-7942`).
- **Spawn:** `sys_spawn_impl` refuses it in the inherited set (`syscall.rs:1903-1905`), as it does `SystemStore` (`:1901-1902`). Only handles the parent explicitly passes are inspected (`:1887-1941`), so neo-init merely holding the cap does not block its other spawns.
- **IPC:** all three resolvers refuse it: `ipc_lookup` (`:5906-5907`), `channel_lookup` (`:5955-5956`), `process_handle_lookup_pinned` (`:6020-6021`) → `EBADF`. It can be neither a channel nor a process handle.
- **Sole mint:** `root_mount_authority_cap` (`process.rs:1376-1381`), called once at kernel spawn into neo-init (`kernel/src/main.rs:501-507`). There is no create syscall; `SYS_HANDLE_INFO` only reports its kind (`kernel/src/sched/handle_info_logic.rs:45`; `process.rs:1133`) and grants nothing.
- Threads of the holder share the process's table, so a neo-init thread can mount. That is the same principal, not delegation. There is no fork-like handle copy.

## 3. In image A; the legitimate mount paths still work — PASS

- **Image A:** `"mount-authority"` is in `PRODUCT_IMAGE_KERNEL_FEATURES` (`tools/xtask/src/main.rs:6051-6054`) and asserted by the product pairing test (`:20696-20700`). Image A's `desktop` feature pulls `product` (`kernel/Cargo.toml:225` → `:143`), which pulls `boot-neofs` (`:82`), so the boot mount is in the shipped image.
- **Boot:** kernel-side at `kernel/src/main.rs:943-944`, through `fs::mount_neofs_from_device0_readonly` / `..._readwrite` (`fs/mod.rs:1024-1048`, `:1051-1075`). No `SYS_MOUNT`, no capability.
- **Installer:** kernel-side at `kernel/src/installer.rs:830`, `:844`, `:873`, `:883` (`NeoFs::mount_readwrite`/`mount_readonly`), reached through `SYS_INSTALL_RUN` under the `Installer` cap, not the VFS mount table. Unaffected.
- **Every `SYS_MOUNT`/`SYS_UMOUNT` caller at this head** (`git grep sys_mount|sys_umount|FileRights::MOUNT|MOUNT_RESERVED`): userland has **no production caller**. The callers are test modules only — neo-init `model_tests` (`userland/neo-init/src/main.rs:1007`, `:1037`; `model-load-cascade` builds), `cap_rooted_tests` (`:7585-7599`; `cap-rooted-open-test` builds), `mount_authority_tests` (`:10021-10034`; this scenario), and neo-worker `mount_authority_worker` (`userland/neo-worker/src/main.rs:2693-2720`; this scenario). Kernel-side: the `test_harness` mount-table shims (`kernel/src/test_harness.rs:35953` onward), compiled only under `neo-test-phase3-mount-table`, which does not enable `mount-authority` (`kernel/Cargo.toml:1696-1697`).
- Image A's WM is the `desktop-with-model` feature set (`xtask:6567-6576`), so none of those test modules is compiled into the shipped userland. The authority is minted and its holder path is exercised by the `mount_authority` scenario (item 4), not by a production mount — which is why nothing legitimate can break.
- **Kill-switch:** `mount-authority` off compiles out the variant, the mask and both checks (`kernel/Cargo.toml:104-111`; every use is `#[cfg(feature = "mount-authority")]`), i.e. today's behavior bit for bit. The unchanged `phase3_mount_*` scenarios, which do not enable it, are the ambient-behavior regression.

## 4. Scenario — PASS

- **Wiring:** `TestScenario { name: "mount_authority", feature: "neo-test-mount-authority", user_feature: Some("mount-authority-test") }` (`tools/xtask/src/main.rs:4212-4218`), plus the storage-disk list (`:12939`) and the NeoFS image variant `"normal"` (`:13715`). Kernel feature `neo-test-mount-authority` (`kernel/Cargo.toml:672`); user features `userland/neo-init/Cargo.toml:229` and `userland/neo-worker/Cargo.toml:85`. The harness mounts NeoFS readonly then runs neo-init (`kernel/src/test_harness.rs:4357-4374`).
- **Refusals:** the worker opens `/` asking for `LOOKUP|MOUNT` (masked), then expects `EPERM` from `SYS_UMOUNT("neofs")` and from `SYS_MOUNT(device0)` (`userland/neo-worker/src/main.rs:2699-2719`). neo-init maps any non-zero worker exit to `100+code` (`userland/neo-init/src/main.rs:10006-10014`).
- **Volume still mounted:** after the worker exits, neo-init reopens `/neofs` (`neo-init:10016-10018`). The root testfs has no `neofs` entry (`fs/mod.rs:1663-1671`), so a successful unmount makes that open `ENOENT` and the check fails. `neofs_mounted()` is a real `SYS_OPEN`/close, independent of the mount table, so it is a real witness.
- **Holder:** neo-init opens `/` with `LOOKUP|MOUNT`, unmounts `neofs` (expects 0), asserts it is gone, mounts device0 again (expects 0), asserts it is back (`neo-init:10021-10040`).
- **Control fails:** `neo-test-mount-authority-control` (`kernel/Cargo.toml:674`) makes `may_mount` answer yes for every process (`syscall.rs:8844-8846`). The worker's unmount then returns 0 (or `EBUSY` if a handle lingers), so the worker exits 10 (or 11) and neo-init fails at `10011-10013` with 110/111. The control cannot pass while the check exists, and the scenario only passes when the refusals are real and the volume survives — the right direction. (The control run is a gate action: `NEO_TEST_EXTRA_FEATURES=neo-test-mount-authority-control`; I did not run it.)

---

## Outside the four (notes, no blocking finding)

1. **Branch is behind main.** Merge-base `72bc9067`; `origin/main` at review time is `188fe40e`, 15 commits / 65 files / +5816/-254 ahead, including real userland code (login, lock, settings). Rebase + fresh gate required before merge; these four conclusions are stated for `6f2ef641`. I checked `origin/main`'s added userland for `SYS_MOUNT`/`SYS_UMOUNT`/`FileRights::MOUNT` and found none, so the conclusions should carry — but re-verify at the rebase, per the standing rule.
2. **The mask is not independently witnessed.** Delete the mask and the scenario still passes because the authority check fires first. If the dispatch's "mask it out" is meant to be enforced, add one probe to the worker: `SYS_HANDLE_INFO` on the root fd, asserting `file_rights & (1 << 27) == 0`. The code is correct today; this is about making it falsifiable.
3. **A `MOUNT`-only open is now `EINVAL`.** Masking leaves an empty right set, which `open_root_temporary` rejects (`fs/mod.rs:1442-1444`). No production caller does this; the only `MOUNT`-only open is `model_tests` (`neo-init:1007`), never built with `mount-authority`. Worth a comment, not a fix.
4. **Dispatch item 5 (prior-art/CP rows, ABI note) is not in the diff.** No `Plans/` or `docs/` change. The syscall ABI does not change (no new syscall number, request struct or flag), so the byte/ABI note is "none"; if item 5 required a committed CP-row write-up, it is absent from the branch (it may live in the PR text, which I did not read).

## Disposition

**PASS on all four.** No bypass found on `6f2ef641`: mount/unmount is gated before any handle lookup on every process-reachable path; the cap cannot leave neo-init through derive, grant, spawn or IPC; it is in image A with the pairing assert; and the boot and installer mounts are kernel-side. Merge remains gated on the rebase and the 4-shard gate (including the `mount_authority` scenario, with the control run recorded).

This was a code review. I ran no gate and claim no CI or QEMU result.
