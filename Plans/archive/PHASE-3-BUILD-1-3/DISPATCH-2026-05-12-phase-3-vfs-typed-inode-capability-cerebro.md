# DISPATCH - Phase 3 VFS, typed-inode, and FileCap prior-art research (Cerebro #3)

**Date filed:** 2026-05-12
**From:** PM (NeoPlutarch Phase 3 cadence)
**To:** Cerebro (deep-research lane)
**Ferry:** PL ferries this dispatch to a fresh Cerebro session. Status: ready to ferry.
**Type:** Question-level deep-research dispatch within the Phase 3 frame established by Cerebro #1 and ADR-021.
**Estimated budget:** 4-8 hours.
**Expected output:** `Plans/RESEARCH-2026-05-12-phase-3-vfs-typed-inode-capability-cerebro.md`

---

## 1. Context

ADR-021 ratifies the Phase 3 substrate:

- BUILD-1 introduces minimal VFS, file capabilities, and initial file syscalls.
- BUILD-2 introduces NeoFS native v1 with typed inode discriminants and executable-mapping refusal.
- BUILD-4 expands the POSIX-ish syscall surface and adds read-only FAT32/ext4.

Cerebro #1 already surveyed typed-inode storage prior art and found that NeoPlutarch's Bible Article 10.3 design is past the production prior-art horizon. This dispatch should **not** re-survey that same ground. It should focus on the VFS-layer mechanism: how typed inodes, file capabilities, mount traversal, retype operations, and foreign-FS imports should be structured.

This is a question-level dispatch inside the established Phase 3 frame. If prior art contradicts ADR-021, surface the contradiction explicitly, but do not reopen the whole Phase 3 substrate by default.

## 2. What this dispatch is NOT asking

Do not re-survey:

- CAS vs single-tier NeoFS. Cerebro #1 settled the Phase 3 storage substrate.
- Storage-driver mechanics. That is Cerebro #2.
- FAT32/ext4 low-level parser details.
- AI model format internals beyond what typed inodes need for `.neom` refusal.
- General Linux POSIX compatibility as a goal. NeoPlutarch is POSIX-ish, not POSIX-compatible.

## 3. Research questions

### Question 1 - VFS typed-inode dispatch architecture

Bible Article 10.3 requires file type to live in the inode, not filename extensions.

Research:

- Where should typed-inode policy be enforced: VFS dispatch table, filesystem driver, syscall layer, or capability lookup?
- How do SELinux LSM hooks route file type to mmap/exec decisions, and what pattern transfers cleanly without importing LSM complexity?
- How do Plan 9 9P Qid type bits travel across mount boundaries, even though they do not enforce execution policy?
- How do BFS/Haiku file type attributes inform the UX or metadata side, while remaining advisory only?
- What is the minimal BUILD-2 policy table for `Regular`, `Directory`, `Symlink`, and `Model`?

Expected output: a VFS enforcement architecture for typed inode operations.

### Question 2 - FileCap rights model

ADR-010 established capability handles and a two-tier rights model. Files now need a semantic rights byte.

Research:

- What rights should a Phase 3 FileCap carry: READ, WRITE, EXEC, DELETE, MMAP_EXEC, STAT, DIRECTORY_TRAVERSE, MOUNT?
- How does Capsicum structure per-fd rights attenuation, especially `CAP_MMAP_X` and rights propagation through `openat`?
- How does seL4's typed-object discipline inform "object type determines valid operations; capability rights further restrict them"?
- How should rights attenuation work when opening a child path through a directory capability?
- What should `GRANT` / `DERIVE` / `REVOKE` mean for FileCap in the ADR-010 framework?

Expected output: proposed FileCap rights table and attenuation rules for BUILD-1/BUILD-2.

### Question 3 - `open` and path traversal without ambient authority

NeoPlutarch has no root user and no ambient filesystem authority.

Research:

- What is the capability-based equivalent of Unix path traversal?
- Should `sys_open` take a directory capability plus relative path, instead of a process-global absolute path?
- Should absolute paths be resolved only relative to a process root capability?
- How should `..`, symlinks, mountpoints, and chroot-like roots behave under capabilities?
- What patterns from Capsicum capability mode, Plan 9 fids, seL4 userspace VFSes, or capability filesystems are portable?

Expected output: recommended Phase 3 path-resolution API shape and anti-scope.

### Question 4 - minimum POSIX-ish syscall surface

ADR-021 leaves syscall detail open beyond the initial set.

Research:

- What is the minimum syscall set for BUILD-1 smoke: `open`, `read`, `write`, `close`, maybe `stat`?
- What must BUILD-2 add for native NeoFS create/read/write semantics?
- What should BUILD-4 add: `mkdir`, `rmdir`, `unlink`, `rename`, `readdir`, `mount`, `umount`, fuller `stat`?
- Which POSIX semantics should be explicitly rejected or deferred: hard links, advisory locks, mandatory locks, `O_DIRECT`, POSIX ACLs, xattrs, quotas?
- What return/error model should libneo expose without copying Linux errno wholesale?

Expected output: syscall staging table for BUILD-1, BUILD-2, and BUILD-4.

### Question 5 - mount namespace and mount traversal

NeoPlutarch needs a POSIX-ish path namespace, not a flat namespace.

Research:

- How do Linux/BSD VFS systems represent mountpoints, and what minimal form is enough for NeoPlutarch?
- How does Plan 9 represent mounted file trees and per-fid traversal?
- Should NeoPlutarch start with one global namespace, per-process roots, or capability-rooted namespaces?
- How should mount authority be represented as a capability?
- How should unmount interact with open FileCaps and background FS work?

Expected output: mount namespace model for Phase 3, including what to defer.

### Question 6 - foreign-FS type import boundaries

FAT32 and ext4 do not natively carry NeoFS typed-inode discriminants.

Research:

- Is "coerce-to-Regular" the right BUILD-4 default?
- What prior art exists for sidecar metadata files: AppleDouble, Haiku MIME typing, desktop metadata stores, or extended attributes?
- Should ext4 xattrs be read if present, or ignored for security simplicity?
- How can foreign-FS mounts prevent accidental model-as-code confusion without filename-extension reliance?
- What does a `.neofs-types` manifest need if it lands later?

Expected output: foreign-FS type import policy, with sidecar manifest forward marker if supported.

### Question 7 - retype operation discipline

NeoPlutarch will eventually need to convert a `Regular` file into a `Model` file after validation.

Research:

- How does seL4 Untyped-to-Typed retype discipline transfer to file objects?
- What capabilities must be revoked before an inode changes type?
- Should retype be a VFS operation, NeoFS-only operation, or separate syscall?
- What validation must occur before `Regular` becomes `Model`?
- How does retype interact with open handles, mmap, cached pages, and in-progress I/O?

Expected output: design constraints and recommended deferral boundary for retype.

### Question 8 - typed-inode policy tests

BUILD-2 needs tests that prove Bible Article 10.3.

Research:

- What scenarios prove model-typed inode executable mapping refusal?
- What scenarios prove filename extension is irrelevant?
- What scenarios prove FileCap rights and inode type both gate access?
- What scenarios prove foreign-FS imports default safely?
- What negative tests should Jarvis implement in BUILD-1/BUILD-2?

Expected output: test matrix for Jarvis dispatch.

## 4. Deliverable structure

Output file: `Plans/RESEARCH-2026-05-12-phase-3-vfs-typed-inode-capability-cerebro.md`

Required sections:

1. Header: date, author, type, authority none, consumed dispatch path.
2. Executive summary: 4-8 paragraphs with the main VFS/FileCap conclusions.
3. Sources surveyed: primary sources separated from secondary/community sources.
4. Findings by question: one section per question above.
5. Proposed FileCap rights table.
6. Syscall staging table for BUILD-1, BUILD-2, BUILD-4.
7. VFS typed-inode enforcement flow.
8. BUILD mapping and test matrix.
9. Risks and corrections to ADR-021.
10. Forward markers and anti-scope confirmation.
11. Calibration notes.
12. Cross-references and closure.

## 5. Citation rigor

Use primary sources where possible:

- Capsicum paper and FreeBSD `rights(4)` for per-fd rights.
- seL4 reference manual for typed-object and retype discipline.
- Linux LSM/SELinux documentation or source for mmap/exec hooks.
- Plan 9 9P RFC and stat man page for Qid and fid behavior.
- Linux/BSD VFS documentation/source for mount traversal.
- Haiku/BFS docs only for type metadata comparison, not enforcement.

Mark single-source claims honestly. Do not treat secondary explainers as production authority when primary docs/source are available.

## 6. Closure protocol

When complete, Cerebro reports back to PL with:

- artifact path;
- time-budget actual;
- one-line verdict on whether ADR-021 VFS/FileCap sequencing is confirmed;
- any hard contradictions or BUILD-1/BUILD-2 blockers;
- recommended next action for PM.

PM consumes the artifact and either:

- files a PM-NOTE confirming VFS/FileCap sequencing and updating BUILD-1 dispatch inputs; or
- revises ADR-021 / THINK v2 if Cerebro surfaces a real contradiction.

## 7. Cross-references

Authoritative inputs:

- `docs/ADR-021-phase-3-design-substrate.md`
- `Plans/PHASE-3-THINK-V2-2026-05-11.md`
- `Plans/PM-NOTE-2026-05-11-phase-3-cerebro-1-consumption-and-q5-reframed.md`
- `Plans/RESEARCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md` section 7 only as typed-inode prior-art context; do not re-survey storage substrate
- `docs/bible.md` Articles 10 and 12.1
- ADR-010 for capability system foundation
- ANSPEC-001 sections 4.1, 4.3, 10.3, 10.5 for `.neom` and model-security consumers

Output consumed by:

- PM VFS/FileCap consumption PM-NOTE
- Phase 3 BUILD-1 PLAN dispatch for Jarvis
- Phase 3 BUILD-2 NeoFS native v1 planning
- `docs/specs/neofs-format.md` type/versioning sections

---

**End of dispatch.**

*Phase 3 Cerebro #3. Question-level VFS, typed-inode, and FileCap prior-art survey inside ADR-021 frame. Focus: VFS enforcement architecture, FileCap rights, capability-rooted path traversal, syscall staging, mount namespace, foreign-FS type import, retype discipline, and tests. Ready to ferry.*
