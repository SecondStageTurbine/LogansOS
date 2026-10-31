# Research: Phase 3 BUILD-2 2E typed-inode policy and generation review

| | |
|---|---|
| **Date** | 2026-05-13 |
| **Author** | Cerebro review artifact |
| **Type** | Review only. Not normative. Does not claim 2E or BUILD-2 closure. |
| **Dispatch** | `DISPATCH-2026-05-13-phase-3-build-2-2e-policy-review-cerebro.md` |
| **Base commit** | `a2f1e49` (`kernel/fs: harden NeoFS CoW recovery`) |

---

## 1. Verdict

**2E plan confirmed.** No blocker found. One ordering trap in the existing `OpenFile` read/write methods (rights-before-type) must not be propagated to the 2E exec policy helper. Six questions answered below with specific guidance for Jarvis. Three top risks enumerated in §8.

---

## 2. Q1 — Typed policy helper: where and what

**Where:** `kernel/src/fs/mod.rs` as a free function (not a method on `OpenFile`). Not syscall-facing; not test-harness-only. The helper belongs at the VFS layer and is called both from test scenarios and from any future mmap/exec dispatch hook. Keeping it as a pure function makes it auditable independent of the open-file lifecycle.

**Minimal correct shape:**

```rust
/// Check whether an inode type permits an executable mapping/execution
/// operation, and whether the FileCap carries the required rights.
///
/// Type is evaluated first; a `Model` inode returns `TypeForbidden`
/// regardless of rights. A `Regular` inode returns `InsufficientRights`
/// if the cap lacks `EXECUTE_RESERVED`.
///
/// This helper does not implement actual mmap or exec. It only enforces
/// the BUILD-2 typed-inode policy at the VFS level.
pub fn check_exec_policy(kind: InodeKind, rights: FileRights) -> Result<(), VfsError> {
    match kind {
        InodeKind::Regular => {
            if !rights.contains(FileRights::EXECUTE_RESERVED) {
                Err(VfsError::InsufficientRights)
            } else {
                Ok(())
            }
        }
        // Model, Directory, Symlink, and any future/reserved kind:
        // all return TypeForbidden regardless of rights.
        _ => Err(VfsError::TypeForbidden),
    }
}
```

**How to prove Model is TypeForbidden without implementing real exec:** the 2E test harness calls `check_exec_policy(InodeKind::Model, rights_with_execute_bit)` and asserts the result is `Err(VfsError::TypeForbidden)`. No mmap system call, no ELF loader, no process spawn. The policy function is the unit under test; the exec operation it guards does not need to exist in BUILD-2.

---

## 3. Q2 — Four-corner matrix and the ordering trap

**The trap:** the existing `OpenFile.read()` and `OpenFile.write()` methods in `fs/mod.rs` check rights BEFORE type:

```rust
pub fn read(&self, out: &mut [u8]) -> Result<usize, VfsError> {
    if !self.rights.contains(FileRights::READ) {
        return Err(VfsError::InsufficientRights);  // rights check first
    }
    if self.inode.kind() != InodeKind::Regular {
        return Err(VfsError::TypeForbidden);        // type check second
    }
    ...
}
```

If Jarvis follows this same pattern for the exec policy helper, the second corner of the matrix fails:

| Corner | Expected | Would get if rights-first |
|---|---|---|
| Model + execute right | TypeForbidden | TypeForbidden ✓ (type check fires after rights pass) |
| **Model + no execute right** | **TypeForbidden** | **InsufficientRights ✗** (rights check fires first, Model fails on rights) |
| Regular + no execute right | InsufficientRights | InsufficientRights ✓ |
| Regular + execute right | Ok | Ok ✓ |

The second corner (Model with no execute right returning `InsufficientRights`) makes the test lie: the test would pass, but the reason is wrong — a `Model` file with no execute rights would be returning "you don't have the right" rather than "this type is forbidden regardless."

**The fix is explicit in `check_exec_policy` above:** the `match kind` arm fires on all non-Regular kinds and returns `TypeForbidden` unconditionally, before any rights check. This is the correct type-first/rights-second ordering that ADR-021 and `RESEARCH-2026-05-12-phase-3-build-2-neofs-format-cerebro.md` §5.3 (Invariant T3) require.

**The existing read/write methods in `OpenFile` are wrong-ordered** relative to the ADR-021 spec but the tests pass because the practical effect is limited: a Directory opened for read hits TypeForbidden after InsufficientRights only when READ is not in the cap, but the path `/dir` with READ rights hits TypeForbidden correctly. The ordering error doesn't manifest in the existing tests because the existing tests don't have a Model-type inode to exercise the trap. Jarvis should NOT fix the existing read/write ordering in 2E (out of 2E scope); but 2E's exec helper MUST be type-first from the start.

---

## 4. Q3 — FileCap generation capture and why in-memory snapshots are insufficient

**Why an in-memory `NeoInode` snapshot is insufficient:**

Consider this sequence:
1. Both handle A and handle B are opened; each captures `generation = 5` from the on-disk inode.
2. Handle A writes, triggering the CoW path: new data block written, inode block updated on disk (generation bumped to 6, new direct block pointers), superblock committed.
3. In a naive implementation, the NeoFS module might hold a `Arc<NeoInode>` struct in memory for performance. After A's write, a new `NeoInode` struct with `generation = 6` is created for A's refreshed view, but the existing `Arc<NeoInode>` that B holds still has `generation = 5`. B's stale check compares `cap.generation (5)` against `its Arc<NeoInode>.generation (5)` — they match. B is not detected as stale. B reads orphaned pre-CoW content.

The in-memory snapshot is an old view of the inode that was valid at open time. After a CoW commit, the in-memory snapshot no longer reflects the current on-disk state. The stale check must compare against the on-disk committed generation, not the in-memory cached value.

**The correct check:** re-read the inode block from disk (via the block layer) and compare the on-disk generation with the FileCap's captured generation. In the synchronous write-through model (no buffer cache in BUILD-2), "read from disk" always returns the committed state.

```rust
pub fn check_stale(&self, dev: &dyn BlockDevice) -> Result<(), VfsError> {
    // Re-read the inode from disk to get committed generation.
    let current_inode = read_inode_from_disk(dev, self.inode_block)?;
    if current_inode.generation != self.captured_generation {
        return Err(VfsError::Stale);
    }
    Ok(())
}
```

**Two-handle scenario correctness:**
- A's CoW write path increments the on-disk inode generation to 6 and commits the new superblock.
- A's handle refreshes its captured generation to 6 after the successful write.
- B's handle still has captured generation = 5. On B's next read/write, `check_stale` re-reads the inode from disk, sees generation = 6, and returns `Err(VfsError::Stale)`.

**What "current reachable inode" means in BUILD-2:** because the only open-by-path mechanism in BUILD-2 resolves `inode_block` from the directory entry at open time, and because CoW write updates the inode at its existing block address (in-place generation bump; the inode block address does not change in the BUILD-2 bump-allocator model), re-reading `inode_block` from disk is sufficient to get the current generation.

Note for Jarvis: this is only safe because BUILD-2 updates inodes in-place (bumps generation, rewrites block pointers) rather than allocating a new inode block. If BUILD-3+ ever allocates new inode blocks on CoW, the stale check must instead re-traverse the directory path to find the current inode block address. This is a forward-compat design note, not a BUILD-2 blocker.

---

## 5. Q4 — Stale errno: ESTALE = -116

**Confirmed correct.** `ESTALE = -116` is the right distinct errno for stale FileCaps. The existing codebase uses `VfsError` variants (NotFound, Invalid, TypeForbidden, InsufficientRights) that map to specific errno values in the syscall layer. 2E adds `VfsError::Stale` and maps it to `-116` in `kernel/src/errno.rs`.

Do not collapse stale to any existing error. The four-corner matrix requires `TypeForbidden` and `InsufficientRights` to remain distinct; stale adds a third distinct class for handle lifetime violations. `EBADF` is wrong (the descriptor is structurally valid; the underlying object changed). `EACCES` is wrong (it's not a permission issue). `EINVAL` is wrong (the argument was valid when issued). `ESTALE` is the POSIX NFS-precedent for exactly this case: a handle was valid at issue time and the underlying object has since changed incompatibly.

**Local convention check:** the existing errno definitions are at `kernel/src/errno.rs`. The `VfsError` → errno mapping is in `kernel/src/arch/x86_64/syscall.rs`. Jarvis should verify no other codebase constant already uses -116 before finalizing. If a conflict exists, file a deviation in the 2E artifact naming the alternative; otherwise PM's ESTALE = -116 ratification stands.

---

## 6. Q5 — Reserved/future inode type behavior

**Option (b) — mount inert and refuse policy/open — best fits BUILD-2.**

Rationale: refusing the whole mount when a reachable reserved type appears (option a) is overly disruptive. A test image with one reserved-kind directory entry should not prevent the rest of the filesystem from being accessible. Option (a) would also complicate the regression suite (any test image that accidentally contains an unusual kind value would block mount).

Option (b) is the correct behavior: the mount succeeds; reachable directory entries with unknown `kind` values are present in the directory listing (their `child_block` and kind byte are readable from the directory data block); but any `sys_open` of the inode returns `TypeForbidden` because `InodeKind`'s `available_rights()` method (or its BUILD-2 equivalent) returns `FileRights::empty()` for any `kind` value that doesn't match the known variants.

Jarvis implementation: in `check_exec_policy`'s `match kind` wildcard arm (`_`), and in `available_rights()` for unknown kinds, return `FileRights::empty()`. The directory entry remains visible but inert.

**Evidence via option (c):** the scenario `phase3_neofs_reserved_type_refusal` uses a deterministic mkfs variant that writes a directory entry with `kind = 4` (first reserved value beyond Model). The test verifies:
- Mount succeeds
- The reserved-kind file is unreachable via open (TypeForbidden)
- No executable access is obtainable regardless of filename (e.g., naming it `model.neom` and trying exec policy → TypeForbidden via the `_` arm)

This is the smallest honest BUILD-2 evidence. No format migration required; the mount validator already accepts unknown kinds in directory entries (it doesn't validate directory entry kind bytes against the inode kind).

---

## 7. Q6 — Forward-compat invariants: all preserved by 2E plan

Checking each of the seven invariants against the 2E dispatch and PM decision:

| Invariant | Status | Evidence |
|---|---|---|
| `FileRights` as `u32` | ✓ Preserved | `fs/mod.rs` already `pub struct FileRights(u32)`. 2E dispatch §2.2 explicitly states "Keep `FileRights` as a `u32`." |
| On-disk type discriminant is source of truth | ✓ Preserved | `check_exec_policy(kind, rights)` takes `kind` from the inode, not from filename. The 2E dispatch explicitly forbids checking filename. |
| Inode generation sourced from `superblock.next_generation` | ✓ Preserved **if** Jarvis reads this field at inode creation and at mount — verify explicitly | The Cerebro format spec (§3.3) and PM decision (§4) both specify this. Jarvis must not use `rand()`, `clock()`, or a per-inode counter independent of the superblock. The generation value comes from the superblock's `next_generation` field, advanced atomically with the superblock commit. |
| `reserved_retype_audit[16]` untouched | ✓ Preserved | 2E dispatch §7 explicitly lists "retype" as a non-scope item. The 16-byte retype audit field must remain zero-initialized and unmodified by 2E. |
| No retype implementation | ✓ Preserved | Explicit non-scope in both PM decision and Jarvis dispatch. |
| No FileCap grant/derive/revoke semantics | ✓ Preserved | Explicit non-scope. 2E adds generation capture to FileCap but not derivation graph semantics. |
| No BUILD-3 CAS/dedup/GC/write-back/FLUSH | ✓ Preserved | Explicit non-scope. The bump allocator and write-through path from 2A-2D are unchanged. |

**One invariant requiring Jarvis verification:** the generation-from-superblock source (row 3). Jarvis must confirm at the 2E commit that the code path for new inode creation reads `superblock.next_generation`, uses it as the inode's `generation` field, and increments `next_generation` in the same superblock commit transaction. An inode whose generation is not sourced from the superblock is a format violation.

---

## 8. Top Risks for Jarvis Before Commit

**Risk 1 — Exec helper ordering trap (Q2).** If Jarvis models the exec policy helper on the existing `OpenFile.read()` implementation, it will check rights before type. The second corner of the four-corner matrix (Model + no execute right → must be TypeForbidden) will return InsufficientRights instead. The test passes but the semantics are wrong. Mitigation: implement `check_exec_policy` with `match kind` as the outer check, rights only entered if kind is Regular.

**Risk 2 — In-memory snapshot stale check (Q3).** If the stale check compares the FileCap's captured generation against an in-memory `NeoInode` struct rather than re-reading the inode block from disk, handle B will not be detected as stale after handle A's CoW write. Mitigation: the stale check must issue a real `BlockDevice::read_blocks()` call to get the committed inode generation.

**Risk 3 — Generation not sourced from superblock (Q6).** If Jarvis derives the generation value from anything other than `superblock.next_generation` — a per-struct counter, a timestamp, a random value — the generation invariant is violated and forward-compat for retype is broken. Mitigation: explicitly trace the code path from `sys_open` to inode creation to the superblock's `next_generation` field and document it in the 2E verification artifact.

---

## 9. Blocker Language (if needed)

If any of the Q2, Q3, or Q3/Q6 risks materialize into a genuine implementation impossibility, PM's blocker language should be:

> **2E BLOCKER — [Policy ordering / Stale detection / Generation source]**
>
> [One of:]
> - Exec policy helper cannot be implemented with type-before-rights ordering without a structural refactor that is outside 2E scope.
> - Stale FileCap detection cannot read the committed on-disk inode generation without infrastructure (inode cache, path traversal) not present in BUILD-2.
> - Inode generation cannot be sourced from `superblock.next_generation` without a format migration that is outside 2E scope.
>
> BUILD-2 sub-commit 2E BLOCKED. 2D remains the last accepted commit.
> `Plans/BLOCKER-2026-05-13-phase-3-build-2-2e-[reason]-jarvis.md` filed.
> BUILD-2 close-candidate is 2D (`a2f1e49`).

None of these blockers are anticipated based on the current substrate. The existing code has the required extension points; 2E is adding named hooks into already-reserved fields and enum variants.

---

## 10. Time-Budget Actual

~25 minutes (source read: fs/mod.rs, block/mod.rs, virtio_barrier.rs, prior Cerebro format spec; PM decision and Jarvis dispatch read; six-question synthesis; no external research agents needed).

---

**End of review artifact.**

*Verdict: 2E plan confirmed. Primary trap: exec policy helper must be type-first (match kind outer, rights inner) — do not copy the existing rights-before-type ordering from OpenFile.read(). Stale check must re-read inode from disk, not compare against in-memory NeoInode snapshot. Generation must come from superblock.next_generation. ESTALE = -116 confirmed. Reserved types: mount inert, refuse open via wildcard arm. All seven forward-compat invariants preserved.*
