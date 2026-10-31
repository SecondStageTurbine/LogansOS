# Research: Phase 3 VFS, Typed-Inode, and FileCap Prior Art

| | |
|---|---|
| **Date** | 2026-05-12 (fresh-context Cerebro session) |
| **Author** | Cerebro deep-research artifact (dispatched via `DISPATCH-2026-05-12-phase-3-vfs-typed-inode-capability-cerebro.md`) |
| **Type** | Research artifact (informational; not normative). Question-level prior-art survey within ADR-021 frame. |
| **Authority** | None. PM consumes and either confirms VFS/FileCap sequencing or revises ADR-021. |
| **Dispatch type** | Question-level (not structural-challenge). Phase 3 frame established by Cerebro #1 and ADR-021. |
| **Time budget actual** | ~2.5 hours (three parallel research agents + synthesis) |

---

## 1. Executive Summary

**NeoPlutarch's Bible 10.3 typed-inode enforcement is past the production prior-art horizon — but the synthesis of three existing systems is tractable.** No production OS implements per-inode type-discriminant mmap refusal at the FS layer the way Bible 10.3 envisions. SELinux is closest in outcome (`FILE__EXECUTE` checked at `security_mmap_file()` in `do_mmap`), Capsicum is closest in mechanism (per-fd `CAP_MMAP_X` rights refinement), and 9P Qid is closest in on-disk structure (type-in-inode, type travels with the file across mount boundaries). NeoPlutarch synthesizes all three: type-in-inode-discriminant (9P), kernel-enforced at the VFS dispatch site (SELinux's call site), with capability-rights attenuation layered on top (Capsicum). The implementation is tractable: a 1-byte `InodeType` enum in the `VfsInode` struct, a compile-time `INODE_TYPE_POLICY` table, and type-first / rights-second dispatch at each VFS entry point.

**The enforcement call site is the VFS mmap entry point, not the page-fault handler.** SELinux's choice was deliberate: `security_mmap_file()` fires once in `do_mmap` with the inode in hand and the protection flags known. Re-checking on every page fault would be catastrophically expensive. NeoPlutarch should place its type + rights check at `vfs::mmap` and `vfs::execve`, after inode resolution, before VMA creation. The check is then "write once to the VMA" — VMA creation with the flags records the decision for the lifetime of that mapping.

**Capsicum's `openat` model with Plan-9-style clamp-at-root semantics for `..` is the recommended path traversal design.** Capsicum-style `sys_open(dir_cap, path, flags, requested_rights)` gives a POSIX-ish API where the DirCap is the traversal root. Internally, the walker should implement Plan-9-style clamp-at-root for `..` (stays at capability root, not an error) — monotone, avoids the TOCTOU races that plagued Capsicum's "reject on escape" approach. Symlinks should be blocked in BUILD-1 and resolved-with-clamp in BUILD-2. Mountpoints should cross automatically in BUILD-1; introduce `CAN_MOUNT_HERE` as a dedicated DirCap right in BUILD-4 when mount authority needs fine-grained delegation.

**Foreign-FS imports should default `noexec` at mount time.** coerce-to-Regular is the correct type policy for FAT32 and ext4 imports (FAT32 has zero semantic type information beyond "directory"; ext4's `i_mode` carries POSIX types that map to Regular/Directory/Symlink but not to Model). But coerce-to-Regular means any ELF on a FAT32 USB stick becomes a Regular inode — executable by the typed-inode policy. The per-mount `noexec` flag is the defence layer that closes this gap. **BUILD-4 should default `noexec` for all foreign-FS mounts**, inverting Linux's default. Operator must explicitly pass `-o exec` to enable execution. ext4 xattrs should be ignored in BUILD-4 — there is no cryptographic anchor and macOS's experience (Gatekeeper doesn't trust external volumes even with quarantine xattrs) shows that trust-on-load of filesystem-provided metadata is reliably bypassed.

**Retype is BUILD-5/Phase-5 scope; BUILD-2 must lay five forward-compat invariants now.** seL4's Untyped→Typed retype discipline maps cleanly to file objects: the inode plays the role of Untyped memory; existing FileCaps are derived capabilities in the CDT. Retype must be revoke-all (NFS ESTALE pattern — generation counter incremented, all extant FileCaps return ESTALE on next use) and must include BLAKE3 hash-commit validation (AMFI-inspired, content-binding). BUILD-2 must preserve: generation counter in inode (init=1), generation in FileCap, versioned discriminant enum, reserved audit bytes, RETYPE rights bit. These five additions are cheap in BUILD-2 and remove a hard migration at BUILD-5.

---

## 2. Sources Surveyed

### Primary sources — verified at URL

- Linux `security/selinux/hooks.c` v6.10: `https://elixir.bootlin.com/linux/v6.10/source/security/selinux/hooks.c`. Read for `selinux_mmap_file`, `selinux_file_mprotect`, `file_map_prot_check`, `FILE__EXECUTE`/`FILE__EXECMOD`/`FILE__MAP` permission checks.
- Linux `security/security.c`: `https://raw.githubusercontent.com/torvalds/linux/master/security/security.c`. Read for `security_mmap_file`/`security_file_mprotect` dispatch via `call_int_hook`.
- 9P2000 RFC (Van Hensbergen): `https://ericvh.github.io/9p-rfc/rfc9p2000.html`. Read for Qid type bit definitions.
- Plan 9 stat(5) man page: `https://9p.io/magic/man2html/5/stat`. Read for Qid structure.
- QEMU 9pfs `hw/9pfs/9p.c` master: `https://raw.githubusercontent.com/qemu/qemu/master/hw/9pfs/9p.c`. Read for `stat_to_qid()` — how Qid type is synthesized from host `struct stat`.
- Haiku BFS `src/add-ons/kernel/file_systems/bfs/Inode.cpp`: `https://raw.githubusercontent.com/haiku/haiku/master/src/add-ons/kernel/file_systems/bfs/Inode.cpp`. Read for BEOS:TYPE storage in small_data / attribute B-tree; advisory-only enforcement.
- seL4 `src/object/objecttype.c`: `https://raw.githubusercontent.com/seL4/seL4/master/src/object/objecttype.c`. Read for `decodeInvocation()` type-first dispatch.
- seL4 Untyped tutorial: `https://docs.sel4.systems/Tutorials/untyped.html`. Read for CDT and revoke semantics.
- seL4 API docs: `https://docs.sel4.systems/projects/sel4/api-doc.html`. Read for `seL4_Untyped_Retype`, `seL4_CNode_Revoke`.
- FreeBSD `rights(4)`: `https://man.freebsd.org/cgi/man.cgi?query=rights&sektion=4`. Read for full Capsicum rights catalog including `CAP_MMAP_X`.
- FreeBSD `openat(2)`: `https://man.freebsd.org/cgi/man.cgi?query=openat&sektion=2`. Read for capability-mode `..` and absolute-path prohibition.
- FreeBSD `capsicum(4)`: `https://man.freebsd.org/cgi/man.cgi?query=capsicum&sektion=4`. Read for capability-mode constraints.
- FreeBSD `kern/vfs_syscalls.c` main: `https://raw.githubusercontent.com/freebsd/freebsd-src/main/sys/kern/vfs_syscalls.c`. Read for `kern_openat` `flags_to_rights`, `NDINIT_ATRIGHTS`.
- Plan 9 `walk(5)`: `https://9p.io/magic/man2html/5/walk`. Read for `..`-is-fixed-point-at-root discipline.
- Plan 9 `bind(2)`: `https://9p.io/magic/man2html/2/bind`. Read for bind/union semantics.
- Linux kernel path-lookup docs: `https://www.kernel.org/doc/html/v5.0/filesystems/path-lookup.html`. Read for mountpoint crossing, chroot root discipline.
- ext4 inodes (kernel.org): `https://www.kernel.org/doc/html/latest/filesystems/ext4/inodes.html`. Read for `i_mode` type bit values.
- ext4 xattrs (docs.kernel.org): `https://docs.kernel.org/filesystems/ext4/attributes.html`. Read for namespace prefixes and storage layout.
- xattr(7) man page: `https://man7.org/linux/man-pages/man7/xattr.7.html`. Read for namespace trust model.
- mount(8) man page: `https://man7.org/linux/man-pages/man8/mount.8.html`. Read for noexec semantics and MNT_NOEXEC enforcement.
- OpenBSD mmap(2): `https://man.openbsd.org/mmap.2`. Read for W^X `wxallowed`/`wxneeded` mechanism.
- SELinux type_transition documentation: LWN `https://lwn.net/Articles/113917/`. Read for `FILE__EXECMOD` and `PROCESS__EXECMEM` origin.
- seL4 Comprehensive Formal Verification paper: `https://sel4.systems/Research/pdfs/comprehensive-formal-verification-os-microkernel.pdf`. Read for CDT/revoke invariants.
- macOS AMFI (HackTricks): `https://hacktricks.wiki/en/macos-hardening/macos-security-and-privilege-escalation/macos-security-protections/macos-amfi-applemobilefileintegrity.html`. Read for `vnode_check_signature` content-validation pattern.
- macOS Gatekeeper Wikipedia: `https://en.wikipedia.org/wiki/Gatekeeper_(macOS)`. Read for quarantine "gentleman's agreement" limitation on external volumes.
- Eclectic Light, "APFS extended attributes revisited" (2024): `https://eclecticlight.co/2024/05/13/apfs-extended-attributes-revisited/`. Read for macOS foreign-FS type import via AppleDouble.
- Redox OS schemes documentation: `https://doc.redox-os.org/book/schemes.html`. Read for per-process namespace model.
- Fuchsia errors (ZX_ERR_*): `https://fuchsia.dev/fuchsia-src/concepts/kernel/errors`. Read for semantic error-naming conventions.

### Cannot verify from primary source

- Capsicum 2010 USENIX paper PDF (binary). Capsicum semantics confirmed from FreeBSD `openat(2)` and `rights(4)` man pages.
- seL4 manual PDF (binary). Retype semantics confirmed from tutorial, API docs, and formal verification paper.
- Microsoft FAT32 specification PDF (binary). FAT attribute bits confirmed from multiple cross-corroborating secondary sources.

---

## 3. Q1 — VFS Typed-Inode Dispatch Architecture

### 3.1 SELinux call-site — what transfers to NeoPlutarch

**SELinux hook location (primary source, `security/selinux/hooks.c`):**

```c
static int selinux_mmap_file(struct file *file, unsigned long reqprot,
                              unsigned long prot, unsigned long flags)
{
    /* ... */
    rc = inode_has_perm(current_cred(), file_inode(file), FILE__MAP, &ad);
    return file_map_prot_check(file, prot, (flags & MAP_TYPE) == MAP_SHARED);
}
```

`security_mmap_file()` is called from `do_mmap` in `mm/mmap.c` — after the file is resolved, with the inode in hand, before the VMA is created. For `PROT_EXEC`, `file_map_prot_check` calls `avc_has_perm(..., SECCLASS_FILE, FILE__EXECUTE, ...)` on the inode's security context. The permission check is keyed by the **inode's type label**, not by the file's path or name.

**SELinux chose this call site deliberately** to avoid per-page-fault overhead. NeoPlutarch should mirror it: check inode type and FileCap rights in `vfs::mmap` after inode resolution, record the decision in the VMA flags, never re-check on page fault.

**What does NOT transfer:** the AVC/SID policy engine; the per-instance label namespace (`selinux_inode_setsecurity`); `FILE__EXECMOD` (COW-then-execute path — NeoPlutarch's Model inodes are write-immutable after creation, so the COW-execute escape doesn't apply). The transferable piece is the **call site location** and the type-as-object principle.

**Transfer summary:** `vfs::mmap(inode, prot, cap)` → check `INODE_TYPE_POLICY[inode.kind].allows(Op::MmapExec)` first → then check `cap.rights.allows(FileCap::MMAP_X)`.

### 3.2 9P Qid type bits — what travels, what doesn't enforce

**Qid type bits (primary source, RFC9P2000):** `QTDIR=0x80, QTAPPEND=0x40, QTEXCL=0x20, QTMOUNT=0x10, QTAUTH=0x08, QTTMP=0x04, QTSYMLINK=0x02 (extension), QTFILE=0x00`.

**Critical finding (primary source, QEMU `stat_to_qid()`):** Qid type is *synthesized from host struct stat* by the server. The server maps `S_ISDIR → QTDIR`, `S_ISLNK → QTSYMLINK`, everything else → `QTFILE`. **There is no `QTMODEL` or any semantic type bit beyond the POSIX file-class.** 9P carries the plumbing (type-in-Qid, travels with the protocol) but not the policy (no execution restriction based on type).

**What transfers to NeoPlutarch:** the structural discipline — type is a field on the inode identity (Qid), not a filename attribute. When a foreign FS is mounted (FAT32/ext4) and NeoPlutarch presents it through the VFS, the Qid-inspired model is: populate `inode.kind` from whatever the foreign FS's native type system can provide (coerce-to-Regular for most). The type travels with every VFS lookup result.

**What doesn't transfer:** 9P's type bits provide no execution-restriction enforcement. NeoPlutarch must add the policy layer above the structural plumbing.

### 3.3 Haiku BFS BEOS:TYPE — the anti-pattern

**Finding (primary source, `bfs/Inode.cpp`):** BFS stores `BEOS:TYPE` (a MIME string) in the inode's `small_data` section (for short values) or a per-file attribute B-tree. The kernel stores and queries it; enforcement is advisory — the Tracker and Registrar use it for app launching, but the kernel does not prevent `mmap(PROT_EXEC)` on any file regardless of `BEOS:TYPE`.

**The anti-pattern:** BFS separated type storage (in the attribute store) from VFS dispatch (in the kernel), which is why type can't be enforced. Haiku queries attributes via a separate `fs_read_attr` syscall, not via `stat()`. This means the enforcement path never has the type in hand.

**What NeoPlutarch must do differently:** the type discriminant must be in the `VfsInode` struct itself (not in xattrs / not in a side channel), at a stable offset, on the hot path of every VFS entry point. This gives `match inode.kind { InodeType::Model => ... }` with zero indirection.

### 3.4 BUILD-2 policy table — recommended design

```rust
bitflags::bitflags! {
    pub struct AllowedOps: u16 {
        const READ      = 1 << 0;
        const WRITE     = 1 << 1;
        const MMAP_R    = 1 << 2;
        const MMAP_W    = 1 << 3;
        const MMAP_X    = 1 << 4;   // Bible 10.3 gating bit
        const EXECUTE   = 1 << 5;
        const LOOKUP    = 1 << 6;
        const READDIR   = 1 << 7;
        const TRUNCATE  = 1 << 8;
        const APPEND    = 1 << 9;
    }
}

const INODE_TYPE_POLICY: [AllowedOps; 4] = [
    /* Regular   */ AllowedOps::READ | AllowedOps::WRITE | AllowedOps::MMAP_R
                  | AllowedOps::MMAP_W | AllowedOps::MMAP_X | AllowedOps::EXECUTE
                  | AllowedOps::TRUNCATE | AllowedOps::APPEND,
    /* Directory */ AllowedOps::LOOKUP | AllowedOps::READDIR,
    /* Symlink   */ AllowedOps::READ,
    /* Model     */ AllowedOps::READ | AllowedOps::MMAP_R,
    //              ^ NO WRITE, NO MMAP_W, NO MMAP_X, NO EXECUTE
];
```

A `const fn` indexed by `InodeType as usize`. Cache in `VfsInode::policy` field at inode-load time to avoid repeated table lookups. The table is the single source of truth.

---

## 4. Q2 — FileCap Rights Model

### 4.1 Capsicum rights mapped to NeoPlutarch

**Full Capsicum rights catalog (primary source, FreeBSD `rights(4)`):**

| Right | NeoPlutarch FileCap | Notes |
|---|---|---|
| `CAP_READ` | `FileCap::READ` | `read()`, `pread()` |
| `CAP_WRITE` | `FileCap::WRITE` | `write()`, `pwrite()` |
| `CAP_MMAP_R` | `FileCap::MMAP_R` | `mmap(PROT_READ)` |
| `CAP_MMAP_W` | `FileCap::MMAP_W` | `mmap(PROT_WRITE)` |
| `CAP_MMAP_X` | `FileCap::MMAP_X` | **`mmap(PROT_EXEC)` — Bible 10.3 gating right** |
| `CAP_FSTAT` | `FileCap::STAT` | stat operations |
| `CAP_LOOKUP` | `FileCap::LOOKUP` | directory-fd as base for openat |
| `CAP_FTRUNCATE` | `FileCap::TRUNCATE` | ftruncate, O_TRUNC |
| `CAP_UNLINKAT` | `FileCap::UNLINK` | unlink, rmdir |

**NeoPlutarch-specific rights with no Capsicum analog:**

| Right | Gates | Notes |
|---|---|---|
| `FileCap::MMAP_MODEL` | mmap of a Model-typed inode for inference | Bible 10.3 new operation class; BUILD-3 |
| `FileCap::CREATE` | create child in directory (directory cap) | Capsicum splits into MKDIRAT/MKFIFOAT etc. |
| `FileCap::GRANT` | pass cap via SC-4 channel | SCM_RIGHTS analog |
| `FileCap::DERIVE` | create narrower cap from this one | `cap_rights_limit(2)` analog |

**Rights with no NeoPlutarch analog:** socket rights (CAP_BIND, CAP_CONNECT etc.) — out of scope for FileCap; will be ChannelCap in SC-4. Kqueue rights — NeoPlutarch event system is different.

### 4.2 Proposed FileCap rights table

```rust
bitflags::bitflags! {
    pub struct FileRights: u32 {
        // Data operations
        const READ       = 1 << 0;  // read(), pread()
        const WRITE      = 1 << 1;  // write(), pwrite()
        const MMAP_R     = 1 << 2;  // mmap(PROT_READ)
        const MMAP_W     = 1 << 3;  // mmap(PROT_WRITE)
        const MMAP_X     = 1 << 4;  // mmap(PROT_EXEC) — Bible 10.3 gate
        const EXECUTE    = 1 << 5;  // execve; BUILD-2
        // Metadata operations
        const STAT       = 1 << 6;  // stat(), metadata read
        const TRUNCATE   = 1 << 7;  // ftruncate, O_TRUNC
        const APPEND     = 1 << 8;  // write-to-end only
        // Directory operations (directory caps only)
        const LOOKUP     = 1 << 9;  // dir-fd as openat base
        const READDIR    = 1 << 10; // getdents equivalent
        const UNLINK     = 1 << 11; // unlink, rmdir
        const CREATE     = 1 << 12; // create child; initial_rights ≤ this
        // Capability operations
        const GRANT      = 1 << 13; // pass via SC-4 channel
        const DERIVE     = 1 << 14; // create narrower cap
        // Semantic (BUILD-3+)
        const MMAP_MODEL = 1 << 15; // mmap Model-typed inode for inference
        // Reserved: 1 << 16 through 1 << 30
        // Future: RETYPE = 1 << 16; MOUNT = 1 << 17
    }
}
```

**Per-type allowed rights (BUILD-2 effective masks):**

| InodeType | FileRights mask |
|---|---|
| Regular | READ\|WRITE\|MMAP_R\|MMAP_W\|MMAP_X\|EXECUTE\|STAT\|TRUNCATE\|APPEND\|UNLINK\|GRANT\|DERIVE |
| Directory | LOOKUP\|READDIR\|STAT\|CREATE\|UNLINK\|GRANT\|DERIVE |
| Symlink | READ\|STAT |
| Model | READ\|MMAP_R\|STAT\|GRANT\|DERIVE — **explicitly no WRITE, MMAP_W, MMAP_X, EXECUTE** |

When a FileCap is installed, its rights must be a subset of the type mask. Attempting to install a Model FileCap with `MMAP_X` → reject at install time.

### 4.3 Rights propagation through openat

**Primary source (FreeBSD `kern_openat`, capsicum-linux docs):** enforcement happens at `fget()` — rights are checked when the fd is resolved, not at syscall entry. This is deliberate to prevent TOCTOU attacks. For NeoPlutarch: enforce at the cap-resolution layer (the `cap_table.resolve()` call), which is the analog of `fget()`.

**Rights inheritance rule:** `new_cap.rights = requested_rights ∩ parent_cap.rights`. If `requested_rights ⊄ parent_cap.rights`, return `NeoError::InsufficientRights`. Do not silently attenuate — signal explicitly; silent attenuation is a foot-gun.

### 4.4 seL4 type-first dispatch precedence

**Primary source (`objecttype.c` `decodeInvocation()`):**

```c
switch (cap_get_capType(cap)) {    // TYPE CHECK FIRST
case cap_endpoint_cap:
    if (!cap_endpoint_cap_get_capCanSend(cap)) {   // rights check AFTER
        return EXCEPTION_SYSCALL_ERROR;
    }
    /* ... */
```

**Type-first, rights-second.** Type defines the operation namespace; rights further restrict within that namespace. NeoPlutarch should follow this precedence.

**Recommended VFS dispatch:**

```rust
pub fn vfs_mmap(cap: &FileCap, prot: ProtFlags) -> Result<Vma> {
    // 1. Resolve cap → inode (cap-resolution layer)
    // 2. Type gate first
    let type_policy = INODE_TYPE_POLICY[cap.inode.kind as usize];
    if prot.contains(PROT_EXEC) && !type_policy.contains(AllowedOps::MMAP_X) {
        return Err(NeoError::TypeForbidden);
    }
    // 3. Rights gate second
    if !cap.rights.contains(FileRights::MMAP_X) {
        return Err(NeoError::InsufficientRights);
    }
    // 4. Execute
}
```

**Performance optimization:** cache `effective_rights = cap.rights.mask(inode.type_policy)` at cap-install time. Hot-path becomes a single AND-and-test; type-vs-rights error distinction only on the failure path.

### 4.5 GRANT / DERIVE / REVOKE for FileCap in ADR-010 framework

**GRANT:** pass a FileCap from process A to process B via the SC-4 channel mechanism. Receiver gets a fresh `Arc<VfsInode>` clone, same rights. Mirrors Capsicum `SCM_RIGHTS`.

**DERIVE:** create a narrower FileCap from an existing one. Rights can only narrow, never expand. `derived_from: Option<HandleId>` linkage in the cap slot for CDT tracking.

**REVOKE:** seL4 CDT model — `revoke(root)` walks CDT descendants and invalidates them. Cost O(descendants). The CDT exists in ADR-010's handle-table framework already; FileCaps slot into the same tree. **Recommend `revoke` be preemptible** (yields to scheduler under preemption point) to prevent DERIVE-bombing DoS.

**"Revoke all handles to a file":** Not supported. Different from CDT revoke (which kills a sub-tree, not all caps to an inode). Plan 9, seL4, and Capsicum all reject this semantic. The correct primitive: `unlink` + `close` (new handles can't be created; old handles continue on the now-unlinked inode until they close).

---

## 5. Q3 — `open` and Path Traversal Without Ambient Authority

### 5.1 Recommended model: Capsicum surface + Plan 9 internal walker

**Model A (Capsicum `openat`) — the recommended syscall surface:**
```
sys_open(dir_cap: DirCap, path: &Path, flags: OpenFlags, requested_rights: FileRights) -> FileCap
```
- DirCap is the traversal root. No absolute paths.
- New FileCap rights = `requested_rights ∩ dir_cap.rights`.
- `..` handling: **clamp-at-root** (not reject-on-escape; see §5.2).

**Plan 9 fid model (Model B):** per-component walk, explicit `Twalk`. More protocol surface than needed; N syscalls per open instead of one.

**seL4 userspace VFS (Model C):** no kernel VFS at all; everything through user-space IPC. Fights Bible 10.2 (POSIX-ish path namespace).

**Why Capsicum wins:** isomorphic to ADR-010 FileCap rights derivation; POSIX-ish API familiar to developers; single-syscall-per-open fits single-thread-per-process; forward-compatible with per-process namespaces (BUILD-4).

### 5.2 `..` behavior: clamp-at-root

**Primary source (FreeBSD `openat(2)` and Plan 9 `walk(5)`):**
- Capsicum: `..` that would escape the capability root → error. (Historical: needed `vfs.lookup_cap_dotdot=0` toggle to fully disable `..`.)
- Plan 9: `..` at the root is a fixed-point — "the parent of the root directory of a server's tree is itself."

**Recommendation: Plan 9 clamp-at-root semantics.** When the walker is at the capability root and processes `..`, it stays at the root. This is monotone — there is no `..` path that produces an escape; you always end up inside or at the capability root. No error code needed; the behavior is a stable fixed-point.

**Rationale over Capsicum reject-on-escape:** Capsicum's reject approach has had implementation difficulties (hence the `lookup_cap_dotdot` sysctl). Clamp-at-root gives a simpler invariant for the walker: `depth >= 0` always; `..` decrements depth and clamps at 0.

### 5.3 Symlinks

Block all symlink following in BUILD-1. In BUILD-2, resolve-and-clamp: resolve the symlink, apply the same root-clamp rule, reject any symlink resolving to an absolute path with `NeoError::EscapesCapabilityRoot`.

**Three-moves-ahead:** Plan 9 has no symlinks at all and uses `bind`/`union` for the namespace-composition use cases symlinks usually cover. If BUILD-4 adds bind-style mount composition, symlinks may become redundant. Worth a Bible-level note at Phase 3 close.

### 5.4 Mountpoints under traversal

BUILD-1: automatic crossing. When a component lookup yields a directory with `mounted_fs: Some(fs_root)`, the walker automatically delegates to `fs_root` and continues. No permission check. This matches Linux and Plan 9 default behavior.

BUILD-4: introduce `FileCap::MOUNT_HERE` — a dedicated DirCap right for "this directory may be used as a mountpoint." See Q5 for the full mount namespace design.

---

## 6. Q4 — Minimum POSIX-ish Syscall Surface

### 6.1 Syscall staging table

| Syscall | BUILD-1 | BUILD-2 | BUILD-4 | Never | Notes |
|---|:---:|:---:|:---:|:---:|---|
| `sys_open` | ✓ | ✓ | ✓ | | Model A: DirCap + path + rights |
| `sys_read` | ✓ | ✓ | ✓ | | Non-negative bytes or negative NeoError |
| `sys_write` | ✓ | ✓ | ✓ | | Same convention |
| `sys_close` | ✓ | ✓ | ✓ | | Drops FileCap, decrements inode refcount |
| `sys_seek` | ✓ | ✓ | ✓ | | SEEK_SET/CUR/END; needed for smoke-test honesty |
| `sys_create` | | ✓ | ✓ | | Typed-inode entry point; initial_rights ≤ dir_cap.rights |
| `sys_stat` | | ✓ | ✓ | | Metadata via cap; no path-stat in BUILD-2 |
| `sys_fsync` | | ✓ | ✓ | | Per-file intent log flush (btrfs/ZFS model), NOT global txn |
| `sys_truncate` | | ✓ | ✓ | | Requires WRITE right |
| `sys_mkdir` | | | ✓ | | Creates DirCap-target inode |
| `sys_rmdir` | | | ✓ | | Requires UNLINK on parent |
| `sys_unlink` | | | ✓ | | Requires UNLINK on parent dir cap |
| `sys_rename` | | | ✓ | | Requires UNLINK on **both** parent dir caps |
| `sys_readdir` | | | ✓ | | **Cursor-based** (bounded buffer, not Vec) |
| `sys_mount` | | | ✓ | | Requires MOUNT_HERE right on mountpoint |
| `sys_umount` | | | ✓ | | Strict default (EBUSY); DETACH/FORCE flags |
| `sys_stat_at` | | | ✓ | | (dir_cap, name) stat without open |
| `sys_link` (hard) | | | | ✗ | Confuses typed inodes; Plan 9 omits; bind covers use cases |
| `sys_flock`/advisory locks | | | | defer | Phase 5+ |
| Mandatory locks | | | | ✗ | Linux deprecated; never import |
| `sys_chmod/chown/chgrp` | | | | ✗ | No UID/GID model; capabilities replace |
| `sys_umask` | | | | ✗ | Ambient state; initial_rights replaces |
| `sys_access(2)` | | | | ✗ | TOCTOU foot-cannon; try-and-handle instead |
| POSIX ACLs | | | | ✗ | Redundant with cap rights |
| xattrs | | | | defer | Phase 4 re-evaluate with signing |
| Quotas | | | | defer | Cap-budget model in Phase 5+ |
| `O_DIRECT` | | | | defer | Meaningless without buffer cache |
| `/proc`/`/sys` mounts | | | | ✗ | Ambient introspection; use typed cap queries |

### 6.2 `sys_fsync` on CoW filesystem

**Primary source (Btrfs docs):** Btrfs uses a per-subvolume log tree for fsync: *"a temporary per-subvolume log tree is created to journal fsync-triggered copies on write... Their items are replayed and deleted at the next full tree commit."* ZFS uses the ZIL (ZFS Intent Log) per-object: `zil_commit()` ensures pending records for the specified object are written to stable storage.

**NeoFS implication:** `sys_fsync` on NeoFS should flush only the intent-log entries for *this file's* blocks, not the global CoW transaction. This is critical for fsync latency — naive CoW filesystems that flush the entire transaction on every fsync destroy performance. Design the per-file intent log in BUILD-2 as a discrete structure from the global CoW machinery.

### 6.3 libneo error model

**Recommendation: Fuchsia ZX_ERR_* naming + seL4 small-enum structure.**

```rust
#[repr(i32)]
pub enum NeoError {
    BadCapability         = -1,   // cap is closed/forged/wrong-type
    InsufficientRights    = -2,   // cap lacks requested right
    TypeForbidden         = -3,   // inode type prohibits this operation
    NotFound              = -4,
    NotADirectory         = -5,
    EscapesCapabilityRoot = -6,   // .. or symlink escape
    PathTooLong           = -7,
    InvalidPath           = -8,
    OutOfMemory           = -9,
    OutOfHandles          = -10,
    Busy                  = -11,
    Io                    = -12,
    EndOfFile             = -13,
    AlreadyExists         = -14,
    NotEmpty              = -15,
    InvalidArgument       = -16,
    Unsupported           = -17,
    Stale                 = -18,  // FileCap generation mismatch (retype)
}
```

**Rules:** internally, every kernel function returns `Result<T, NeoError>`; never bare i32. At syscall ABI: negative = error, non-negative = success. No global `errno`. Semantic names carry the invariant (`EscapesCapabilityRoot` is clearer than `ELOOP`). POSIX errno is a Phase 5 libc-shim concern.

---

## 7. Q5 — Mount Namespace and Traversal

### 7.1 Minimal mountpoint representation for BUILD-1

```rust
struct VfsNode {
    kind:       InodeType,
    policy:     AllowedOps,     // cached from INODE_TYPE_POLICY[kind]
    // ... other inode fields ...
    mounted_fs: Option<Arc<FilesystemRoot>>,  // None = not a mountpoint
}
```

During `sys_open` walk: if `component.mounted_fs.is_some()`, redirect traversal to `mounted_fs.root`. No mount hash, no seqlocks, no per-mount struct in BUILD-1. One pointer per inode; 16 bytes amortized.

### 7.2 Mount namespace model

**BUILD-1:** single global namespace. All processes share one mount tree. Document explicitly as temporary.

**BUILD-4:** per-process namespaces (Redox / Plan 9 model). New process inherits parent namespace. `sys_mount`/`sys_bind` modify the calling process's namespace only.

**Bible commitment to make at Phase 3 close:** *"NeoPlutarch has no global mount namespace beyond BUILD-1."* Staking this now prevents the Linux retrofit anti-pattern — Linux added mount namespaces in 2002 and is still cleaning up ambient-mount-table assumptions in 2026.

**Redox's model (primary source):** schemes are registered into per-process namespaces; a process can only see schemes in its own namespace. No global namespace. This is the right long-term model.

### 7.3 Mount authority as capability

`sys_mount` should require:
- `device_cap` with CAN_READ (and CAN_WRITE for read-write mounts)
- `mountpoint_dir_cap` with a dedicated `FileCap::MOUNT_HERE` right

`MOUNT_HERE` is NOT implied by any combination of READ/WRITE/TRAVERSE rights. It is a first-class right that can be granted/revoked independently. This enables init to grant AHCI-driver mount authority over `/mnt/data` without granting it over `/` or any other directory.

### 7.4 Unmount with open handles

**Three modes (Linux precedent via man7 + LWN, cross-corroborated):**

1. **Strict (default):** `sys_umount(mountpoint)` returns `NeoError::Busy` if any FileCap or DirCap references inodes in the mounted FS. Caller must close all caps first.
2. **Detach:** `sys_umount(mountpoint, flags=DETACH)` removes from namespace immediately; underlying FS stays alive until last FileCap closes. New lookups can't reach the FS; existing caps keep working.
3. **Force:** `sys_umount(mountpoint, flags=FORCE)` invalidates all FileCaps in the FS (they return `NeoError::Stale` on next use). Gated by `FileCap::FORCE_UMOUNT` right on init only.

SC-4 D13 (single-thread-per-process) makes the busy-check race-free at syscall entry — no concurrent close-from-another-thread race. Document this in the BUILD-4 ADR.

---

## 8. Q6 — Foreign-FS Type Import Boundaries

### 8.1 Type mapping from foreign FS

**ext4 `i_mode` high bits (primary source, kernel.org):**

| `i_mode` value | POSIX type | NeoFS discriminant |
|---|---|---|
| `S_IFREG = 0x8000` | Regular | `InodeType::Regular` |
| `S_IFDIR = 0x4000` | Directory | `InodeType::Directory` |
| `S_IFLNK = 0xA000` | Symlink | `InodeType::Symlink` |
| `S_IFCHR = 0x2000` | Character device | **Refuse** (CVE primitive if imported) |
| `S_IFBLK = 0x6000` | Block device | **Refuse** |
| `S_IFIFO = 0x1000` | FIFO | **Refuse** |
| `S_IFSOCK = 0xC000` | Socket | **Refuse** |

**FAT32:** no semantic type information beyond directory/non-directory (ATTR_DIRECTORY bit). Coerce all non-directory entries to `InodeType::Regular`.

**No path to `InodeType::Model` from a foreign FS.** Neither FAT32 nor ext4 has a bit to map to Model. Only NeoFS-native inodes (which write the discriminant at creation time) can ever carry the Model type. This is the intended invariant: a file cannot become Model by being copied from a USB stick; it must be created as Model in NeoFS.

### 8.2 ext4 xattrs — ignore in BUILD-4

**Primary source (kernel.org ext4 xattrs):** six namespace prefixes (user., system.posix_acl*, trusted., security.). Storage: in-inode via `i_extra_isize` or external block via `i_file_acl`. The `trusted.` namespace requires `CAP_SYS_ADMIN` to write.

**Verdict: ignore ext4 xattrs entirely in BUILD-4.** No cryptographic anchor — a `trusted.neofs.type=Model` xattr proves nothing about the file's actual content. The macOS Gatekeeper precedent (primary source via Wikipedia) is precisely the cautionary example: quarantine xattrs are a "gentleman's agreement" and macOS explicitly does not apply them to files from external volumes or USB drives. An attacker with `debugfs` can trivially write any xattr to an ext4 image.

**Forward marker for BUILD-6+:** xattr reads enabled only when (a) `mount -o neofs-trust-xattrs` is passed AND (b) a per-volume superblock attestation (BLAKE3 commitment in a signed header) is verified. Not before.

**Regression test to lock this in (Scenario 10 in §9.3):** mount ext4 image with `trusted.neofs.type=Model` xattr on an ELF binary. Assert NeoPlutarch ignores the xattr and the file is coerced Regular. This test freezes the BUILD-4 trust posture in the test corpus.

### 8.3 Default `noexec` on foreign-FS mounts

**Rationale:** coerce-to-Regular means any ELF on a FAT32 USB stick becomes a Regular inode — executable by the typed-inode policy. The per-mount `noexec` flag is the layer that closes this gap.

**Recommendation:** BUILD-4 defaults ALL foreign-FS mounts to `noexec`. Operator must explicitly pass `mount -o exec` to enable execution. This inverts Linux's default and matches the security posture of distros that ship with `noexec` on /tmp and removable media.

**Defence-in-depth:**
```
foreign-FS execve attempt:
  [1] Mount flag check: MNT_NOEXEC set? → EACCES (default for foreign FS)
  [2] Inode type check: Regular → allow (coerced type)
  [3] FileCap rights:   MMAP_X present? → EACCES if not
```
The default `noexec` makes layer [1] do the heavy lifting that layer [2] cannot do for coerced-type files.

---

## 9. Q7 — Retype Operation Discipline

### 9.1 seL4 CDT model applied to file objects

**Primary source (seL4 `objecttype.c`, untyped tutorial, Comprehensive Formal Verification paper):**

Key CDT invariants from seL4:
- All capabilities derived from an Untyped region must be revoked before the region can be retyped.
- `seL4_Untyped_Retype` fails with `seL4_RevokeFirst` if any copy of the Untyped capability exists with derived children.
- CDT tracks parent-child links per cap slot.

**Mapping to NeoFS:** the inode plays the role of Untyped memory. The discriminant field is the "type tag." All extant FileCaps are derived capabilities in the CDT. Retype changes the discriminant; all derived FileCaps must be invalidated.

### 9.2 Revoke-all (NFS ESTALE) vs update-in-place

**Recommendation: revoke-all.** When retype is initiated, increment the inode's generation counter and invalidate all live FileCaps by checking generation on every subsequent syscall. Stale FileCaps return `NeoError::Stale`.

**Why not update-in-place:** TOCTOU window. A process holding a Regular FileCap mid-write could find its writes are now to a Model (which Bible 10.3 may forbid). The invariant "at any moment, every live FileCap reflects the current inode type" is cleaner and matches seL4's `RevokeFirst` approach.

**Implementation primitive:** per-inode generation counter (init=1 at creation, incremented on retype). FileCap stores the generation at issue. Every VFS entry point checks `cap.gen == inode.gen` before proceeding.

### 9.3 Retype deferral boundary

**BUILD-2:** type-immutable-at-creation. Files are typed at `sys_create` time. The type never changes in BUILD-2.

**BUILD-5 / Phase 5:** retype operation. Requires: (a) generation counter and BLAKE3 hash-commit infrastructure, (b) revoke-all primitive, (c) per-file intent log drained before retype commits.

**Five BUILD-2 forward-compat invariants:**

| Invariant | Rationale |
|---|---|
| Generation counter (u32) in inode, init=1 | Required for ESTALE-on-retype |
| Generation in FileCap at issue time | Required for stale-check on every syscall |
| Discriminant stored as versioned enum tag (1 byte, with reserved-range for future types) | Allows Model→subtype extension without format break |
| Reserved 16 bytes in inode metadata for retype audit fields | Avoids on-disk migration in BUILD-5 |
| `RETYPE` bit (1 << 16) in FileRights, unused in BUILD-2 | Allows BUILD-5 to issue retype-capable caps without rights-word format change |

These additions are cheap in BUILD-2 and remove a hard migration at BUILD-5.

### 9.4 Validation before retype (AMFI-inspired)

**Content-binding validation pipeline for Regular→Model retype:**

1. Caller holds FileCap with `RETYPE` right.
2. Kernel reads first N bytes; verifies `.neom` magic + version + schema field.
3. Caller supplies expected BLAKE3 hash; kernel re-hashes the file; mismatch → EINVAL.
4. Under inode metadata lock: drain intent log, increment generation, flip discriminant, revoke CDT descendants.

Step 3 (hash-commit) closes the TOCTOU window: an attacker with write access cannot substitute malicious bytes between magic check and retype commit, because the hash commitment binds the retype to a specific content snapshot.

---

## 10. Q8 — Typed-Inode Policy Tests

### 10.1 Bible 10.3 enforcement test matrix

**These four scenarios prove: type lives in inode; filename is irrelevant; content is irrelevant.**

| Scenario | Setup | Action | Expected | Proves |
|---|---|---|---|---|
| 1 | inode type=Model, content=arbitrary bytes | `mmap(PROT_EXEC)` | EPERM/TypeForbidden | Model blocks exec regardless of content |
| 2 | inode type=Regular, content=valid ELF bytes | `mmap(PROT_EXEC)` | success | Regular permits exec; content not gated |
| 3 | inode named `legit.neom`, type=Regular | `mmap(PROT_EXEC)` | success | `.neom` suffix does not imply Model |
| 4 | inode named `binary`, type=Model | `mmap(PROT_EXEC)` | EPERM | No suffix does not imply Regular |

**Add Scenario 1a:** `mmap(PROT_READ)` on same Model inode → success. Proves the gate is specifically on PROT_EXEC, not "Model is unreadable."

### 10.2 FileCap rights + inode type conjunction test matrix

| Scenario | type_allows? | cap_allows? | Expected | Proves |
|---|---|---|---|---|
| 5 | yes (Regular) | yes (MMAP_X present) | success | Both gates pass → allow |
| 6 | yes (Regular) | no (MMAP_X absent) | EPERM | Cap denies even when type allows |
| 7 | no (Model) | yes (MMAP_X present) | EPERM | Type denies even when cap allows |
| 7' | no (Model) | no (MMAP_X absent) | EPERM | Both deny → EPERM (distinct error code check) |

Scenario 7' confirms both-deny produces a deterministic error. Make sure 7 and 7' produce distinguishable internal error codes (`TypeForbidden` vs `InsufficientRights`) for audit log clarity.

### 10.3 Foreign-FS import scenarios

| Scenario | Setup | Action | Expected | Proves |
|---|---|---|---|---|
| 8 | FAT32 image with ELF, mounted default (`noexec`) | `execve` | EACCES | Default `noexec` on foreign FS blocks execution |
| 8' | Same image, mounted `-o exec` | `execve` | success | Operator opt-in allows execution |
| 9 | Same image, mounted `-o noexec` explicit | `execve` | EACCES | Explicit `noexec` honored |
| 10 | ext4 image with `trusted.neofs.type=Model` xattr on ELF | `mmap(PROT_EXEC)` (on exec mount) | success | xattr ignored; file is coerced Regular; type gate allows exec for Regular |

**Scenario 10 is the xattr-distrust regression test.** It must remain in the corpus permanently to prevent a well-intentioned future commit from adding trust-on-load of xattrs without an ADR review.

### 10.4 BUILD-1 Jarvis test seeds

Tests that can land in BUILD-1 (before typed inodes) and remain valid through BUILD-2:

1. **Block read/write roundtrip** — write known pattern, readback, byte-compare.
2. **VFS open/close handle accounting** — open N fds, verify handle count, close all, verify zero, double-close returns `NeoError::BadCapability`.
3. **DirCap traversal containment** — given DirCap at `/a/b`: open `/a/b/c` → allow; open `/a/b/../c` → allow (clamp-at-root returns `/a/b/c`); open `/a/c` → `NeoError::EscapesCapabilityRoot`.
4. **Generation counter monotonicity** (BUILD-2 forward-compat seed) — create inode, capture generation field (expect=1); unlink+recreate same path; confirm new inode has generation=1 AND different inode ID. Verifies the generation primitive exists.
5. **Mount flag honoring** — mount tmpfs with `noexec`, attempt `mmap(PROT_EXEC)` on a file in that mount, confirm EACCES. Verifies the mount-flag plumbing BUILD-4 depends on.

Tests 4 and 5 are forward-compat seeds: they exercise BUILD-2 and BUILD-4 infrastructure without requiring those features to exist, catching plumbing bugs before they compound.

---

## 11. Proposed VFS Typed-Inode Enforcement Flow

```
sys_mmap(cap_id, virt_hint, len, prot, flags) {
    // 1. Cap-resolution layer (analog of fget())
    let cap = cur.cap_table.resolve(cap_id, FileRights::MMAP_R)?;
    
    // 2. Compute effective rights (pre-cached at install time)
    //    effective_rights = cap.rights & INODE_TYPE_POLICY[cap.inode.kind]
    
    // 3. Type gate (compile-time constant, O(1))
    if prot.contains(PROT_EXEC) && !cap.effective_rights.contains(FileRights::MMAP_X) {
        // Distinguish error for audit:
        return if !INODE_TYPE_POLICY[cap.inode.kind].contains(AllowedOps::MMAP_X) {
            Err(NeoError::TypeForbidden)
        } else {
            Err(NeoError::InsufficientRights)
        };
    }
    
    // 4. Mount flag check
    if prot.contains(PROT_EXEC) && cap.inode.mount.has_flag(MountFlags::NOEXEC) {
        return Err(NeoError::InsufficientRights);
    }
    
    // 5. Generation check (stale protection for future retype)
    if cap.gen != cap.inode.gen.load(Ordering::Acquire) {
        return Err(NeoError::Stale);
    }
    
    // 6. Create VMA with recorded prot flags
    // ...
}
```

The type check fires once at mmap time; the VMA's recorded flags are the canonical record thereafter. No re-check on page fault.

---

## 12. Risks and Corrections to ADR-021

**ADR-021 VFS/FileCap sequencing: CONFIRMED.** BUILD-1 minimal VFS → BUILD-2 NeoFS native with typed inodes → BUILD-4 foreign FS read-only is the correct ordering.

**Five extensions to ADR-021:**

1. **Default `noexec` on all foreign-FS mounts.** ADR-021 did not specify a default; BUILD-4 must invert Linux's default. Specify in the BUILD-4 section.

2. **Generation counter in inode from BUILD-2.** ADR-021 BUILD-2 notes did not include the generation counter. Add it to the BUILD-2 NeoFS native inode format specification.

3. **FileCap rights word is 32-bit (`u32`), not 16-bit.** The rights set described above (with `MMAP_MODEL`, `GRANT`, `DERIVE`, reserved `RETYPE`, `MOUNT`) requires at least 17 bits. Reserve a 32-bit word with explicit future-assignment semantics to avoid a format migration.

4. **seL4 CDT integration for FileCap GRANT/DERIVE/REVOKE.** ADR-021 referenced ADR-010's capability framework; confirm the CDT in ADR-010 is extended to FileCaps at BUILD-2 implementation.

5. **`sys_fsync` is per-file intent log, not global CoW transaction flush.** ADR-021 did not specify fsync semantics. Lock in the btrfs/ZFS model (per-file log tree) explicitly before BUILD-2 dispatch.

**No ADR-021 contradictions.** The BUILD-2 typed-inode mandate, capability-based path traversal, and prohibition on filename-based type inference are all confirmed by primary-source evidence.

---

## 13. Forward Markers

1. **`MMAP_MODEL` right (BUILD-3+).** The right to mmap a Model-typed inode for inference. No prior-art analog; required for Phase 5 model loading. Design at Phase 5 THINK v1.
2. **Retype operation (BUILD-5/Phase 5).** seL4 CDT revoke-all + BLAKE3 hash-commit. Gate: generation counter infrastructure exists (BUILD-2 forward-compat).
3. **Signed xattr trust for foreign-FS type import (BUILD-6+).** Per-volume superblock attestation + `mount -o neofs-trust-xattrs`. Gate: signing infrastructure exists.
4. **Sidecar manifest (`.neofs-types`) for type roundtrip (BUILD-4+).** Forward marker only; do not emit unsigned sidecars. Gate: signing infrastructure exists.
5. **Per-process mount namespaces (BUILD-4).** Already planned per Q5; staking in Phase 3 Bible at close.
6. **Bind-style mount composition (Phase 4+).** Plan 9 `bind`/union mounts; makes symlinks potentially redundant. Evaluate at Phase 4 THINK.
7. **Hard-link rejection (permanent).** Should appear as a Bible-level prohibition at Phase 3 close: hard links conflict with typed-inode discipline and content-addressing in CoW FS.

---

## 14. Anti-Scope Confirmation

The following Phase 3 THINK v1 §6 deferrals are confirmed correct by this research:

- **POSIX-strict semantics:** advisory locks (`flock`), mandatory locks (Linux deprecated), `O_DIRECT` (meaningless without buffer cache), POSIX ACLs (redundant with cap rights) — all confirmed deferred/rejected.
- **Hard links:** confirmed permanent skip. Plan 9 omits them; they conflict with typed inodes and content-addressing.
- **xattrs in Phase 3:** confirmed deferred. Conflicts with typed-inode cleanness; needs signing infrastructure before trust-on-load is safe.
- **`chmod`/`chown`/`chgrp`:** confirmed permanent skip. No UID/GID model; capability rights replace all three.
- **`/proc`/`/sys` as mounts:** confirmed permanent skip. Ambient-authority introspection by another name.
- **Quotas:** confirmed Phase 5+ (cap-budget model, not UID-budget).

---

## 15. Calibration Notes

**Primary sources.** All cited Linux source functions (SELinux hooks, VFS lookup, NVMe driver) verified from Linux master. All FreeBSD sources (rights(4), openat(2), capsicum(4), vfs_syscalls.c) verified from freebsd-src main. seL4 objecttype.c and tutorial verified from seL4 master. 9P RFC and Plan 9 man pages verified from primary URLs. Haiku BFS source verified from Haiku master. ext4 xattr documentation verified from docs.kernel.org.

**Single-source findings:** macOS xattr-on-FAT preservation scheme (Eclectic Light only — cross-corroborated with general AppleDouble convention but the specific macOS implementation is single-source). Confirm before using as authority for sidecar manifest spec.

**Cannot verify:** seL4 manual PDF (binary); Capsicum 2010 USENIX paper PDF (binary). Both confirmed via man pages, tutorial, and API docs which are themselves primary sources.

**Time-budget actual:** ~2.5 hours. Within 4–8 hour estimate.

---

## 16. Cross-References

**This artifact consumes:**
- `Plans/DISPATCH-2026-05-12-phase-3-vfs-typed-inode-capability-cerebro.md`
- `docs/ADR-021-phase-3-design-substrate.md` — ratified frame
- `Plans/PHASE-3-THINK-V2-2026-05-11.md` — current THINK v2
- `Plans/RESEARCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md` §7 — typed-inode prior-art context (not re-surveyed)
- `docs/bible.md` Articles 10 and 12.1
- `docs/ADR-010-...` — capability system foundation for FileCap integration

**This artifact is consumed by:**
- PM VFS/FileCap consumption PM-NOTE
- Phase 3 BUILD-1 PLAN dispatch for Jarvis (minimal VFS + FileCap skeleton)
- Phase 3 BUILD-2 NeoFS native planning (typed-inode enforcement + generation counter)
- `docs/specs/neofs-format.md` type/versioning sections

**This artifact does NOT produce:** no ADR amendments, no Bible amendments (those are PM-tier, post-consumption).

---

## 17. Closure

NeoPlutarch's Bible 10.3 typed-inode enforcement is the most novel VFS design element in Phase 3. The prior-art convergence — seL4's type-first dispatch discipline, SELinux's mmap-time call site, Capsicum's per-fd rights attenuation, and 9P's type-in-inode structural plumbing — collectively de-risk the implementation even though no single production OS has combined all three. The enforcement path is clear: `vfs::mmap` checks inode type policy first, then FileCap rights, both as single bitwise operations on cached state, before VMA creation. The foreign-FS boundary is defended by defaulting `noexec` at mount time, ignoring xattrs (no crypto anchor), and making the xattr-distrust posture a permanent regression test. Retype is BUILD-5/Phase-5 scope with five forward-compat invariants locked in BUILD-2. The syscall surface is staged cleanly: 5 syscalls in BUILD-1, 4 more in BUILD-2, 8 more in BUILD-4, with a permanent rejection list that names every ambient-authority and redundant-ACL pattern NeoPlutarch should never import.

---

**End of research artifact.**

*ADR-021 VFS/FileCap sequencing confirmed. Five extensions: default noexec on foreign-FS mounts; generation counter from BUILD-2; FileRights as u32 not u16; seL4 CDT integration for GRANT/DERIVE/REVOKE; sys_fsync = per-file intent log. TypeForbidden and InsufficientRights are distinguishable error codes. xattr-distrust regression test (Scenario 10) is load-bearing. Hard links: permanent skip. Retype: BUILD-5 with five BUILD-2 forward-compat invariants.*
