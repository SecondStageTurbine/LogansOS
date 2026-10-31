# BUILD-4A Verification Readiness Checklist

Date: 2026-05-16
Type: PM note — pre-implementation readiness check
Scope: Dispatch review only, no code changes

---

## 1. Seven Required Scenarios — Sufficiency Assessment

| # | Scenario | Covers | Gap? |
|---|----------|--------|------|
| 1 | `phase3_vfs_readdir_testfs_root` | Full userland readdir loop, 4 entries, EOF via neo-init | No |
| 2 | `phase3_vfs_readdir_requires_right` | Missing READDIR right returns EACCES | No |
| 3 | `phase3_vfs_readdir_regular_is_enotdir` | readdir on regular file returns ENOTDIR(-20) | No |
| 4 | `phase3_vfs_directory_read_is_eisdir` | sys_read on directory still returns EISDIR(-21) | **Is anti-regression in disguise** — see §2 |
| 5 | `phase3_neofs_readdir_root` | NeoFS root readdir, 3 entries, kernel-harness path | No |
| 6 | `phase3_neofs_readdir_stale_generation` | Stale cap on directory returns ESTALE, not ENOTDIR | No |
| 7 | `phase3_readdir_cursor_eof_stable` | Empty dir, cursor=0, repeated calls stay at EOF | No |

**Missing scenarios (recommend adding):**

- **`phase3_vfs_readdir_empty_dir_never_returns_entries`** — Scenario 7 only verifies `/dir` returns EOF immediately. Does not verify that a non-empty directory exhausted via cursor also stays at EOF on subsequent calls. The testfs root scenario iterates to EOF but doesn't make a post-EOF call beyond the one in `read_one`. Low risk, but worth an explicit assertion.

- **`phase3_readdir_neofs_non_root_dir`** — No scenario exercises readdir on a NeoFS subdirectory (e.g., `/neofs/subdir`). Only the root is tested. If non-root directory readdir has a different code path in `neofs/inode.rs::readdir_at`, it goes unverified. **Medium risk.**

- **`phase3_readdir_buffer_too_small`** — No scenario verifies that a dirent whose `record_len > out_len` is rejected with EINVAL rather than silently truncating. The kernel impl checks `n <= out_len` at `syscall.rs:761`, but nothing exercises this branch. **Medium risk.**

---

## 2. Anti-Regression Scenarios for Close Evidence

The dispatch lists seven anti-regressions. Assessment:

| Scenario | Necessary? | Notes |
|----------|-----------|-------|
| `phase3_vfs_testfs_open_read_close` | Yes | Baseline file I/O must not regress |
| `phase3_vfs_testfs_write_readback` | Yes | Write path untouched but verify |
| `phase3_vfs_type_vs_rights_errors` | Yes | Confirms type-first, rights-second dispatch unchanged |
| `phase3_block_vfs_smoke` | Yes | Block layer unaffected |
| `phase3_neofs_mount_readonly_root` | Yes | NeoFS mount path unchanged |
| `phase3_neofs_filecap_generation_stale` | Yes | File-cap stale behavior must not leak into dir caps differently |
| `phase3_neofs_fsync_dirty_persists_after_crash` | Yes | Fsync path untouched |

**Add to anti-regression list:**

- **Explicit `sys_read` on directory still returns EISDIR** — Scenario 4 (`phase3_vfs_directory_read_is_eisdir`) is listed as a *new* scenario, but it's fundamentally an anti-regression check: existing behavior that `sys_read` on a directory returns EISDIR must not break after adding `sys_readdir`. It should appear in BOTH the new-scenario PASS list AND the anti-regression section of the close artifact.

- **`phase3_vfs_testfs_open_read_close` with a directory path** — Verify that opening `/dir` as a regular file (with READ rights) still fails appropriately. The `open_root_temporary` refactor in Step 3 changes how directory opens are accepted; we need to confirm the rejection path for wrong-rights directory opens still works.

---

## 3. ABI Coverage: libneo/userland vs. Kernel-Harness

**Current split:**

| Layer | testfs root | rights check | ENOTDIR | EISDIR | cursor/EOF | NeoFS root | NeoFS stale |
|-------|-------------|--------------|---------|--------|------------|------------|-------------|
| neo-init (userland, full syscall path) | Yes | Yes | Yes | Yes | Yes | — | — |
| kernel harness (kernel buf, bypasses user copy) | — | — | — | — | — | Yes | Yes |

**Assessment:** The split is sound. The four testfs scenarios exercise the complete userspace-to-kernel syscall path, including `with_user_access`, buffer validation, and cursor round-trip through userspace. The two NeoFS scenarios use `debug_sys_readdir_kernel_buf_for_test`, which bypasses user-space memory copies — this is acceptable because:

1. The ABI encoding (`encode_dirent_v1`) is shared code; testfs scenarios already validate the wire format end-to-end.
2. NeoFS scenarios test filesystem-specific logic (generation anchors, mount state), not the syscall plumbing.

**No additional kernel-harness scenario is needed** for the ABI itself. However, one concern:

- The NeoFS stale scenario (`phase3_neofs_readdir_stale_generation`) creates a new file to advance generation, then calls readdir on the stale directory handle. This tests `ESTALE` on a *directory* cap specifically. The existing `phase3_neofs_filecap_generation_stale` only tests stale on a *file* cap. These are different code paths through `validate_current()`. **The new scenario is necessary and correct.**

---

## 4. False-Green Risks

### 4.1 NeoFS readdir entry order is implicit, not asserted against a contract

The `phase3_neofs_readdir_root` scenario expects entries in mkfs creation order: `hello.txt`, `model.neom`, `subdir`. The test will pass as long as the implementation returns them in that order. If the implementation uses a different iteration order (e.g., sorted by inode number, hash order), the test fails — which is correct behavior. **No false-green risk here.**

However, if the implementation returns entries in a *different but deterministic* order and someone adjusts the test expectations to match without realizing the order changed, that would be a silent regression. Mitigation: the PASS line includes `order=mkfs`, making the expected order visible in the close artifact.

### 4.2 Empty-directory cursor semantics

`phase3_readdir_cursor_eof_stable` asserts that on an empty directory, `cursor` remains `0` after EOF. The dispatch says "Cursor value is opaque to userspace." If the implementation sets cursor to some non-zero sentinel on EOF for empty dirs, the test catches it. But if a *non-empty* directory's final EOF call also leaves cursor non-zero, nothing asserts that cursor resets. The testfs root scenario's `read_one` helper checks `Ok(0)` return but doesn't assert the cursor value after EOF.

**Risk: Low.** The ABI spec says cursor is opaque; userspace shouldn't depend on its value at EOF. But the close artifact should record the cursor value at EOF for both empty and non-empty directories.

### 4.3 `readdir_requires_right` — EACCES vs rights check location

The scenario opens `/` with `FileRights::LOOKUP` only, then calls `sys_readdir`. The syscall's `file_lookup(fd, FileRights::empty())` succeeds (empty rights is a subset of everything). Rights enforcement happens inside `OpenFile::readdir_one()`, which returns `VfsError::InsufficientRights` → `EACCES(-13)`.

**Risk: Low, but worth flagging.** If someone refactors `sys_readdir_impl` to add rights checking in `file_lookup` (e.g., `file_lookup(fd, FileRights::READDIR)`), the test still passes with EACCES, but the error originates from a different code path. The dispatch explicitly says "resolve FileCap without premature rights gating" — this is a stop condition, not a test assertion. If the implementation violates this principle, the test won't catch it.

**Recommendation:** Add a kernel-harness assertion that `file_lookup` in `sys_readdir_impl` uses `FileRights::empty()`, or document this as a code-review checkpoint rather than a test gate.

### 4.4 NeoFS debug path diverges from production syscall path

The NeoFS scenarios call `debug_sys_readdir_kernel_buf_for_test`, which takes a kernel buffer (`&mut [u8]`) instead of a user pointer. The production `sys_readdir_impl` validates `out_ptr` through `validate_user_buf`, copies cursor in/out through `copy_user_u64` / `copy_kernel_u64_to_user`, and encodes into a user buffer. These two paths share `encode_dirent_v1` and `file.readdir_one()`, but the user-memory safety layer is only exercised by testfs scenarios.

**Risk: Low.** The testfs root scenario exercises the full userland path. If the encoding is correct there, it's correct for NeoFS. The NeoFS scenarios correctly focus on FS-specific logic (generation, mount state).

### 4.5 `phase3_vfs_directory_read_is_eisdir` opens `/dir`, not `/`

The scenario uses `DIR_PATH` (`/dir`) to verify that `sys_read` on a directory returns EISDIR. This is correct and covers the subdirectory case. No risk.

---

## 5. Summary Checklist

**Status:** Checklist accepted. BUILD-4A may proceed to close evidence once `NEO_SMP=4 --no-meta` exits 0.

### Close Narrative Requirement
- Do NOT allow a "perfectly clean" close narrative.
- The correct narrative is: **"BUILD-4A verified, residual substrate instability tracked separately."**
- Close evidence **must** cite: `Plans/CI-SMP-RACE-TRIAGE-FINDINGS-2026-05-16.md`
- Include the residual retry-pass caveat referencing that triage document.

### Do not demand speculative readdir scenarios
- Only add new readdir scenarios if the final close gate points back to readdir.

### Close evidence requirements:
- [ ] All 7 BUILD-4A scenario `event=PASS` lines captured
- [ ] Buffer-too-small EINVAL negative test for `sys_readdir`
- [ ] `phase3_vfs_directory_read_is_eisdir` recorded in **both** new-scenario AND anti-regression sections
- [ ] Code-review checkpoint: `sys_readdir_impl` uses `file_lookup(fd, FileRights::empty())` — no premature rights gating
- [ ] NeoDirentV1 ABI note in close artifact: `record_len == 16 + name_len` exactly, not a skip-forward field
- [ ] Explicit citation of `Plans/CI-SMP-RACE-TRIAGE-FINDINGS-2026-05-16.md`
- [ ] Residual retry-pass caveat referencing the triage document
- [ ] Close narrative reads: "BUILD-4A verified, residual substrate instability tracked separately"
- [ ] `NEO_SMP=4 cargo neo-test --no-meta` exits 0
- [ ] Close artifact explicitly states no mount/FAT32/ext4 code landed
