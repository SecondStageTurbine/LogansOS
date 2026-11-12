<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Muse security read, F0 / PR #106 (NeoFS folder operations)

> **Banked by the PM (2026-11-12).** Muse has read-only access, so J.c. relayed this text. **Head note:** Muse
> reviewed `e2b5daff`. #106's current head is `e7a50407`: Seat E's fix for Chad's F1/F2 (`b6ee94b4`) plus a merge of
> main. The PM checked `e2b5daff..e7a50407` against the code this read relies on, and none of it changed:
> - `OpenFile::dir_op_check` (`kernel/src/fs/mod.rs`: kind, then LOOKUP on the intersection, WRITE on the handle,
>   then `check_name`);
> - `dir_ops_logic::check_name`, which only gained the new `newer_generation` helper;
> - `handle_chain`'s `NotFound` → `Stale` mapping in `dir_ops.rs`;
> - the `neofs-dir-ops` gating.
>
> The `syscall.rs` changes in that range come from main's #100 merge (`SYS_NET_IF_STATUS`), not from F0. The
> `fs/mod.rs` changes are the newest-first generation publish (#106 F1) and test-only debug hooks under
> `neo-test-neofs-dir-ops`. So this PASS carries over to `e7a50407`.

- **Dispatch:** `Plans/DISPATCH-2026-11-12-relays-395-397.md` (on `main`), final section "Muse security read of F0 (#106), relayed by J.c.; Muse can't push". Relay numbering note: the file's numbered sections are 395 (Janus, #104), 396 (Chad, #106) and 397 (Jarvis, #107); the Muse security read is a separate, unnumbered section in the same file covering the same PR as Section 396. This RETURN grades only that Muse section's 4 items. Per the dispatch, the PM banks it as `Plans/RETURN-2026-11-12-pr106-security-muse.md`.
- **PR / branch:** #106, `claude/f0-neofs-dir-ops`.
- **Reviewed commit:** `e2b5daff` — matches the head named in the dispatch for #106; no discrepancy.
- **Reviewer:** Muse (Gwen) — a different model from the writer (Claude), so this review counts under the model-difference rule.
- **Overall verdict: PASS — all 4 items.** No FAIL. Two observations below, neither inflated into a fail.

## Item 1 — Every one of the four syscalls checks the handle's rights before touching the volume (`syscall.rs`, `fs/mod.rs`)

**PASS.**

- The single gate is `OpenFile::dir_op_check` (`kernel/src/fs/mod.rs:661`): kind first (NotDirectory), then rights — LOOKUP must be present in `self.rights ∩ handle`, and a mutation additionally requires WRITE on the handle — then the name (item 2). Every `OpenFile` dir-op method calls it before any inode/volume call: `mkdir_at` (`kernel/src/fs/mod.rs:596`), `unlink_at` (`kernel/src/fs/mod.rs:604`), `rename_at` (`kernel/src/fs/mod.rs:614` — `dir_op_check` on **both** `self` and `to`, plus the `mount_id` CrossDevice check, all before `inode.rename_at`), `stat_at` (`kernel/src/fs/mod.rs:638`, `mutate = false`, so LOOKUP only, no WRITE).
- The syscall layer resolves the fd to `(OpenFile, handle rights)` first — `dir_op_handle` (`kernel/src/arch/x86_64/syscall.rs:7915`) via `file_lookup_with_rights` (`kernel/src/arch/x86_64/syscall.rs:8166`) — and only then enters the VFS body: `mkdir_at_body` (:7942), `unlink_at_body` (:7962), `rename_at_body` (:7989, resolves both fds before the call), `stat_at_body` (:8027). Nothing between fd lookup and `dir_op_check` touches the volume; the only earlier work is user-memory name validation/copy (item 2) and, under the `system-store` feature, the `.system/` path refusal inside `dir_op_handle`, which only adds a refusal.
- Below the VFS, the only entries are the `NeoInode` trait impls (`kernel/src/fs/neofs/inode.rs:1972-1999`) calling `pub(super)` functions in `kernel/src/fs/neofs/inode/dir_ops.rs:432` (`mkdir`), `:450` (`rename`), `:497` (`reload`) — no path reaches `NeoFsState::{mkdir_at,unlink_at,rename_at,stat_at}` (`dir_ops.rs:46,84,142,226`) except through the checked `OpenFile` methods. The kernel-test shim `dir_ops_for_test` (`syscall.rs:8040`) calls the same `*_body` functions, so it passes the same checks.
- Exact shape of the WRITE check, for the record: mutations test `handle.contains(WRITE)` (the fd's own handle rights), not the intersection — sound here because LOOKUP *is* tested on the intersection, a derive can only narrow a handle's rights, and per the comment at `fs/mod.rs:667-671` a directory's OpenFile rights are its opening handle's (scope opens are capped by the authorizing cap in `root_dir_cap_for`). A handle without WRITE gets `InsufficientRights` → `EACCES` before any volume access; without LOOKUP, `stat_at` is likewise refused.

## Item 2 — No name can escape the handle's directory: `..`, `/`, NUL, an empty name, an over-long name

**PASS.**

- `check_name` (`kernel/src/fs/neofs/dir_ops_logic.rs:43`): empty → `Empty`; `len > NAME_MAX (47)` (`dir_ops_logic.rs:22`) → `TooLong`; contains `/` → `Slash`; contains NUL → `Nul`; exactly `.` or `..` → `Dot`; non-UTF-8 → `NotUtf8`. It runs inside `dir_op_check` (`fs/mod.rs:678`), i.e. for all four syscalls and for **both** names of a rename (source via `self`, destination via `to`).
- Defense in depth at the syscall boundary: `dir_op_user_name` (`kernel/src/arch/x86_64/syscall.rs:7902`) refuses `len == 0` or `len > 47` with `EINVAL` before validating/copying the user buffer, and names are copied by explicit length (`copy_user_bytes`), never as C strings — so an embedded NUL survives the copy and is caught by `check_name`, not silently truncated into a different name.
- Since a name can contain no separator and cannot be a dot component, it is always a single entry inside the handle's directory; there is no path arithmetic on the user name anywhere in `dir_ops.rs` — the name is only compared against, pushed into, or removed from the resolved directory's entry list.

## Item 3 — A stale handle (`ESTALE`) can't be used to act on a folder that has since moved

**PASS.**

- A directory handle names a path, and every operation re-resolves that path under the namespace lock before editing: `handle_chain` (`kernel/src/fs/neofs/inode/dir_ops.rs:254`) → `load_directory_chain` (`kernel/src/fs/neofs/inode.rs:1017`), where a missing component returns `NotFound`, which `handle_chain` maps to `NeoFsError::Stale` (`dir_ops.rs:260-263`) → `VfsError::Stale` → `ESTALE` (`kernel/src/arch/x86_64/syscall.rs:8211`). All four operations load the chain before any edit or commit (`dir_ops.rs:46,84,142,226`); rename loads both chains before either is edited.
- Concrete check of the checklist case: open a handle on folder `d`; rename `d` → `d_moved` through the parent; any `mkdir_at`/`unlink_at`/`rename_at`/`stat_at` through the old handle now fails `ESTALE` and writes nothing — the handle holds only the path string (`NeoInode.name`), never the moved folder's block, so it cannot follow the folder to its new location. The same holds when an ancestor is renamed (the handle's path stops resolving). After its *own* successful operation a handle reloads the new version at its path (`dir_ops::reload`, `dir_ops.rs:497`; `OpenFile::after_dir_op`, `fs/mod.rs:684`), and a failed reload also surfaces as `Stale`.

## Item 4 — With `neofs-dir-ops` off, none of this is compiled

**PASS.**

- Syscall numbers exist only under the feature: `SYS_MKDIR_AT..SYS_STAT_AT = 26..29` (`kernel/src/arch/x86_64/syscall.rs:124-135`), the four dispatch arms are each `#[cfg(feature = "neofs-dir-ops")]` (:1249-1265), and every `sys_*_at_impl` / `*_body` / `dir_op_user_name` / `dir_op_handle` is individually gated (:7901-8038). With the feature off, numbers 26-29 fall to the dispatch default `_ => ENOSYS` (`syscall.rs:1529`); the table is otherwise unchanged.
- VFS layer: the `Inode` trait methods, the `OpenFile` methods, `dir_op_check`/`after_dir_op`, and the F0 `VfsError` variants (`NotEmpty`, `CrossDevice`) are all `#[cfg(feature = "neofs-dir-ops")]` in `kernel/src/fs/mod.rs` (e.g. :138-206, :595-692).
- NeoFS layer: `pub mod dir_ops_logic` is gated (`kernel/src/fs/neofs/mod.rs:16-17`), as are the F0 `NeoFsError` variants in the same file; `mod dir_ops` is gated (`kernel/src/fs/neofs/inode.rs:37-38`) and so are the `NeoInode` trait impls that call it (:1972-1999).
- The feature itself, `neofs-dir-ops = ["cap-rooted-open"]` (`kernel/Cargo.toml:1960`), is not in `default` (`kernel/Cargo.toml:81`); the only feature that pulls it in is the test-only `neo-test-neofs-dir-ops` (:1963).

## Observations (not fails)

1. **Path-replacement boundary of item 3, stated so the PM banks it knowingly.** The handle binds the *path*, not the folder's identity — deliberate and documented (`dir_ops.rs:16-22`; `fs/mod.rs:656-659`: "No generation check: a directory handle names a path (K1b scopes authorize paths)"). So after the sequence *rename `d` → `d_moved`; `mkdir_at(parent, "d")`*, the old handle acts on the **new** folder now at `d`. It can never act on the moved folder (the item-3 case, which passes), and its authority never exceeds the path its K1b scope authorized — but a reader expecting Unix-fd inode semantics should note the difference.
2. **rename reload order.** In `OpenFile::rename_at` (`fs/mod.rs:614`), if the source handle's post-commit reload fails, the destination handle is handed the error and is not reloaded (`to.after_dir_op(reloaded)`). The rename itself has already committed; the caller sees `ESTALE`. Cosmetic/consistency only — no authority or on-disk effect.

## Limitations

- Reviewed from the files below at `e2b5daff`, fetched serially via the GitHub connector; no 429 was hit. The PR body was not readable with the read-only tools and was not needed: the checklist text came from the dispatch on `main`.
- `kernel/src/test_harness.rs` (~1.5 MB, over the MCP read limit) was not read; the four Muse items are all decided in the syscall/VFS/NeoFS code paths above, and no scenario leg was needed to grade them. Host KATs for the wider (Chad) checklist were out of scope for this read.
- The `.system/` refusal inside `dir_op_handle` is compiled only under `system-store`; I verified it can only add a refusal before the rights check, not bypass one. Whether that feature is on in the product build was not checked — it does not affect any verdict above.
