<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Relay 396 — Chad's cross-family review of F0 (#106)

**Dispatch:** `Plans/DISPATCH-2026-11-12-relays-395-397.md`, Section 396.
**Reviewed head:** `e2b5daff318648ef8db76e8d73ae842d44bdc295`, `claude/f0-neofs-dir-ops`.
**RETURN branch:** `codex/pr106-review-chad` (documentation only, based on `origin/main`).
**Requested model / effort:** GPT-6.1-Sol / high; new kernel syscalls on the NeoFS commit path.
**Verdict: FAIL.** F1 fails the PR's CP-13 claim; F2 fails the explicit device-wait requirement and the
CP-63 claim. D1 is a checklist/design wording discrepancy: stat correctly makes no commit.
All source line numbers below refer to the reviewed head, not the RETURN branch's source tree.

## Findings and concrete inputs

### F1 — A shared directory handle can publish a new generation with an old inode record

**FAIL, concurrency / CP-13.** `kernel/src/fs/neofs/inode/dir_ops.rs:497` obtains a directory snapshot,
then separately stores its inode block (`:503`) and record (`:504`).
`kernel/src/fs/mod.rs:684` calls that reload and only afterwards stores the OpenFile generation (`:687`).
The namespace guard in a mutation ends before this reload; there is no common lock around those three stores.
`kernel/src/fs/mod.rs:699` validates the OpenFile generation, whereas
`kernel/src/fs/neofs/inode.rs:2058` reads the separately stored record for readdir.

Concrete input: two threads share one scope fd on a non-root empty directory `/neofs/d`, with
LOOKUP|READDIR|WRITE. T1 calls `mkdir_at(fd, "a")`; T2 calls `mkdir_at(fd, "b")`.
Let `(B1,R1,G1)` be d's version after a, and `(B2,R2,G2)` its version after b.
This interleaving is allowed by the individual lock scopes:

1. T1 commits a, snapshots `(B1,R1)`, stores `inode_block=B1`, then pauses before storing its record.
2. T2 commits b, snapshots `(B2,R2)`, stores `inode_block=B2` and `record=R2`, returns G2 from
   `reload_dir`, then pauses before `fs/mod.rs:687`.
3. T1 stores `record=R1`, returns G1, stores `OpenFile.generation=G1`, and returns success.
4. T2 stores `OpenFile.generation=G2` and returns success.
5. With no further mutation, readdir on fd passes the current-generation check at
   `kernel/src/fs/mod.rs:406`, but reads R1's directory data. It lists a and then EOF, omitting b.
   The superseded data need not have been drained or reused for this failure.

Final state is `(inode_block=B2, record=R1, OpenFile.generation=G2)`. Reading one snapshot initially does
not make its later publication atomic. The serial readdir scenario at `kernel/src/test_harness.rs:33805`
cannot distinguish this schedule, and the host disk model has no shared OpenFile reloads.
Use a common consistency protocol for publishing and consuming the handle snapshot, including its saved
generation. Required discriminator: force the two pauses above and require a complete current listing
or ESTALE, never a successful old listing. This finding is a source-level interleaving proof; I did not
run that new kernel discriminator.

### F2 — Namespace spin lock remains held during blocking device I/O

**FAIL, concurrency / CP-63.** `kernel/src/fs/neofs/inode.rs:124` declares `entries` as a `SwitchMutex`.
`kernel/src/sync/switch_mutex.rs:180` acquires it with a spin loop (`:187`); it is not a parking lock,
contrary to #106's CP-63 row.

Concrete input: on a mounted volume containing `/neofs/a/note`, evict note's clean inode block, then
delay its device read. T1 calls `stat_at(a_fd, "note")`. It holds `entries` from
`kernel/src/fs/neofs/inode/dir_ops.rs:232` through the inode read at `:240`.
The cache miss calls `read_fs_block` at `kernel/src/fs/neofs/cache.rs:247`; this reaches
`device.read_block` at `kernel/src/fs/neofs/inode.rs:2844`.
For virtio-blk the path is `kernel/src/drivers/block/virtio_blk.rs:715`, whose submit/wait sequence
can park (`:777`, `:901`). While that read is delayed, T2 calls `mkdir_at(root_fd, "z")` on the same
volume and spins acquiring `entries` at `kernel/src/fs/neofs/inode/dir_ops.rs:63`.

The three mutations likewise hold `entries` through chain/cache reads (`dir_ops.rs:64`, `:101`, `:162`),
and a deeper rename also walks the subtree under it (`:192`, `:328`). Dirty throttling can hold it
through an entire drain (`kernel/src/fs/neofs/inode.rs:975`). This is an inherited locking pattern,
newly used by F0; it still fails the dispatch's requirement. IF-on permits interrupts and preemption
but does not turn a contending spin acquisition into a parked wait. I found no reverse completion
acquire of `entries`, so this finding establishes held-lock I/O and spinning contention, not an ABBA
deadlock. Move blocking reads outside the namespace critical section with version revalidation, or
resolve the requirement explicitly with a tested alternative. Required discriminator: hold the device
completion and observe another writer's wait behavior and namespace-lock ownership.

### D1 — The request for stat to commit exactly once conflicts with its implementation

**FAIL as written, documentation discrepancy.** Concrete input: `stat_at(root_fd, "hello.txt")` on the
normal writable fixture succeeds via `kernel/src/fs/neofs/inode/dir_ops.rs:226` without calling any
commit/publish function. It makes zero commits, which is the appropriate behavior for a read-only stat.
The design's wording at `docs/THINK-2026-10-29-ot88-app-model.md:488` and dispatch item 1 should say
each **mutation** commits once; do not add a write to stat to satisfy that sentence.

## Section 396 checklist

### 1. One commit per operation

- **FAIL as literally written — D1.** Each successful production mutation has one `commit_dir_tree` call:
  `kernel/src/fs/neofs/inode/dir_ops.rs:79`, `:135`, `:222`. That helper calls
  `commit_root_entries` once at `:384`, which bumps one superblock generation at
  `kernel/src/fs/neofs/inode.rs:1640`. Stat (`dir_ops.rs:226`) commits zero times. The pre-publish
  early return at `dir_ops.rs:379` exists only under the test feature.
- **PASS — superblocks last.** `kernel/src/fs/neofs/commit_logic.rs:37` orders the roles;
  `:76` requires the barrier flush before a superblock. The real drain uses that decision at
  `kernel/src/fs/neofs/cache.rs:1031`, flushes at `:1061`, and writes blocks at `:1076`.
- **PASS — deferred reuse.** `kernel/src/fs/neofs/inode.rs:1642` frees the superseded versions in
  the publishing generation; `:2962` defers them. `kernel/src/fs/neofs/alloc_logic.rs:78` releases
  only durable frees and `:85` excludes remaining deferred blocks. The allocator's durability source
  is `kernel/src/fs/neofs/inode.rs:3176`; the drain advances it after a flush at `cache.rs:1066`.
- **PASS — crash-cut tree property in the tested disk model.** Kernel directory rewrites are
  bottom-up at `kernel/src/fs/neofs/inode/dir_ops.rs:356` and `:373`, followed by the single root
  publish at `:384`. The model follows the same tail/shared-chain structure at
  `tools/neofs-dir-ops-logic-tests/src/model.rs:347`, publishes at `:402`, and imports the real
  barrier/deferred-free decisions. `tools/neofs-dir-ops-logic-tests/src/cut_tests.rs:64` checks
  both the published tree and before/after generation; `:102` iterates event cuts. The seven
  scripts at `:132` cover mkdir, file/empty-dir unlink, and same/across/up/down rename. Subsets are
  exhaustive through 12 pending writes and sampled beyond (`:49`), not universally exhaustive.
  The no-barrier falsifier at `:200` fails as intended. Stat has no writes to cut. This is host
  model evidence, not a claim that these tests exercise concurrent kernel handle reloads.

### 2. Rights

- **PASS — no WRITE gives EACCES for mkdir/unlink/rename.**
  `kernel/src/fs/mod.rs:597`, `:605`, `:622` select mutation checks; `:674` requires handle WRITE,
  and `kernel/src/arch/x86_64/syscall.rs:8210` maps insufficient rights to EACCES.
- **PASS — stat requires LOOKUP.** `kernel/src/fs/mod.rs:643` selects the non-mutating check;
  `:670` requires LOOKUP in the OpenFile/handle intersection. The errno mapping is the same.
- **PASS — rename checks both handles.** `kernel/src/fs/mod.rs:622` and `:623` run the complete
  checks before invoking the filesystem at `:627`.
- **PASS — names cannot traverse outside the handle path.**
  `kernel/src/fs/neofs/dir_ops_logic.rs:43` rejects empty, >47-byte, slash, NUL, dot/dot-dot and
  invalid UTF-8 names. All four operations enter that check through `kernel/src/fs/mod.rs:678`.
  The exact-child lookup is under `entries`, e.g. `dir_ops.rs:233`. Host boundary cases are at
  `tools/neofs-dir-ops-logic-tests/src/logic_tests.rs:11`.

### 3. The rules

- **PASS — unlink refuses a non-empty directory.**
  `kernel/src/fs/neofs/inode/dir_ops.rs:118` returns NotEmpty; the syscall maps it to ENOTEMPTY at
  `kernel/src/arch/x86_64/syscall.rs:8218`. Kernel scenario: `test_harness.rs:33757`.
- **PASS — rename neither replaces nor crosses volumes.**
  `kernel/src/fs/neofs/inode/dir_ops.rs:184` refuses an occupied destination. Mount IDs are checked
  at `kernel/src/fs/mod.rs:624`, and the NeoFS volume identity at `dir_ops.rs:460`; CrossDevice
  maps to EXDEV at `kernel/src/arch/x86_64/syscall.rs:8220`.
- **PASS — no folder move into its descendant.**
  `kernel/src/fs/neofs/dir_ops_logic.rs:131` checks a component prefix, called before mutation at
  `kernel/src/fs/neofs/inode/dir_ops.rs:157`. Host cases: `logic_tests.rs:62`.
- **PASS — the stated limits return errors.** Capacity is checked before mkdir/cross-directory
  insertion (`kernel/src/fs/neofs/inode/dir_ops.rs:69`, `:210`), enforced at
  `kernel/src/fs/neofs/inode.rs:1837` (4096/64 = 64). Names use `dir_ops_logic.rs:47`.
  NoSpace/TooDeep become Invalid at `dir_ops.rs:519`, then EINVAL at `syscall.rs:8206`.
  Ordinary writes reject more than eight 4096-byte blocks at `kernel/src/fs/neofs/inode.rs:734`;
  `NeoInode::write_at` maps that failure to Invalid at `:2105`. The separate large-file replace
  path is outside this ordinary-write limit and F0's scope.

### 4. The format

- **PASS — empty/new directories have no data block.**
  `kernel/src/fs/neofs/inode/dir_ops.rs:290` uses the special empty version, whose size/count,
  direct/indirect pointers and empty-content hash are set at `:297`. Mkdir calls it at `:73`;
  an emptied source/parent uses it during chain rewriting at `:363` or `:389`.
  The root always retains the pinned mount-required entries (`dir_ops_logic.rs:85`).
- **PASS — directory byte layout is hand-laid independently.**
  `kernel/src/test_harness.rs:33559` builds expected bytes from literal offsets (name at 0,
  child u64 at 48, kind at 56, 64-byte stride, zero padding), without the writer/reader codecs,
  and compares the device block at `:33596`. Dynamic child block addresses are read separately;
  the layout and expected names/order are specified by the scenario. Empty inode assertions
  start at `:33573`. Reporting note: the `dir_blocks:9` text at `:33943` is hard-coded; this
  source has 11 oracle calls, three empty (no data block) and eight non-empty data-block
  comparisons. Correct/count that label; it is not an asserted coverage count.

### 5. User arguments

- **PASS — user pointers/lengths checked before dereference.**
  `kernel/src/arch/x86_64/syscall.rs:7903` bounds names before `validate_user_buf` (`:7906`)
  and fault-tolerant copy (`:7907`). Both rename names go through it at `:7978`, `:7982`.
  Stat enforces exactly 16 output bytes at `:8007` and checks the range at `:8010` before copy.
  Range overflow/kernel-half checks are at `:1637`; actual mapping/protection faults return
  EFAULT through `user_copy` at `:1718`, used by both copy helpers (`:1786`, `:1928`).
  Invalid name content returns EINVAL through `fs/mod.rs:678`. The kernel bad-args leg is at
  `kernel/src/test_harness.rs:33859`.
- **PASS — stat's 16-byte ABI entry and byte oracle agree.**
  `docs/SDK-ABI.md:191` and `:506` specify the call/reply length.
  `kernel/src/fs/neofs/dir_ops_logic.rs:179` writes LE u32 kind at 0, u32 flags at 4, u64 size
  at 8. `kernel/src/fs/neofs/inode/dir_ops.rs:477` maps directory size to entry count and sets
  the empty flag. `tools/neofs-dir-ops-logic-tests/src/logic_tests.rs:97` lays out three byte
  arrays by hand; `:153` checks the kernel encoder and independent libneo decoder against them.

### 6. Concurrency

- **FAIL — the CP table does not hold in full.** The PR has the required bank section, but CP-13
  fails by F1 and CP-63's parking-lock premise fails by F2. Concrete inputs and enforcing source
  lines are above. Per-row assessment follows.
- **FAIL — a lock shared with other writers is held across a blocking device wait.** F2's delayed
  stat/cache-miss input reaches device I/O with `entries` held. The cache's own inner lock is
  released before device I/O (`kernel/src/fs/neofs/cache.rs:233`, `:247`); that does not release
  the caller's namespace lock. The handle table is released before entering the filesystem
  (`kernel/src/arch/x86_64/syscall.rs:8175`, `:7947`).

| PR bank row | Verdict | Code and discriminator assessment |
|---|---|---|
| CP-1 lock order | PASS, leaves existing order | Mutations take namespace before metadata/root (`dir_ops.rs:63`, `inode.rs:1637`, `:1648`); cache inner sections are short and do not call back into namespace. Handle lookup ends before VFS (`syscall.rs:8175`, `:7947`). No new reverse nested acquisition found. |
| CP-37 / CP-56 IF discipline | PASS, guards | All four syscall bodies use `vfs_body_if_on` (`syscall.rs:7947`, `:7967`, `:7998`, `:8032`); handle lookup stays outside (`:7916`, `:8175`). This preserves the existing IF-on pattern. |
| CP-12 slot reuse | PASS for stated block-reuse guards | Frees are deferred (`inode.rs:2962`, `alloc_logic.rs:85`); absent handle paths map to Stale (`dir_ops.rs:259`). The unlinked-reader and moved-path legs are at `test_harness.rs:33722`, `:33787`. This does not establish the separate CP-13 snapshot claim. |
| CP-13 consistent reload | FAIL, introduced exposure | F1; `dir_ops.rs:503` / `:504` and `fs/mod.rs:687` publish separately. The serial listing leg does not test two shared-handle reloads. |
| CP-17 publish after ready | PASS for barrier model | `commit_logic.rs:76`, used at `cache.rs:1031`; cut tests and no-barrier falsifier at `cut_tests.rs:174`, `:200`. |
| CP-63 long-call lock | FAIL, inherited and extended | F2; namespace is a spin mutex (`inode.rs:124`, `switch_mutex.rs:180`), held across cache-miss I/O and the new deeper-subtree walk (`dir_ops.rs:192`). |
| CP-99 user copies | PASS, guards | `syscall.rs:7902`, `:8005`, `:1718`; bad-args discriminator at `test_harness.rs:33859`. |
| CP-96 rights cannot be widened | PASS for F0 use of handle rights | `fs/mod.rs:670` / `:674` check the supplied actual handle rights from `syscall.rs:8178`; no new mint/derive path is added. No-WRITE cases at `test_harness.rs:33884`. |
| CP-111 last reference under lock | PASS, leaves existing behavior | Handle lookup returns an owned Arc before VFS (`syscall.rs:8178`); operation-local directory frames are plain vectors (`inode.rs:68`). No new locked final OpenFile teardown found. |

**I-14: PASS for F0's additions.** The added paths use handle lookup, inode/namespace/metadata/root,
and cache locks (`syscall.rs:7915`, `fs/mod.rs:661`, `dir_ops.rs:46`, `:497`). They add no
AI-governance-lock acquisition, IRQ handler or DMA path.

### 7. Kill-switch

- **PASS — the four kernel calls disappear and existing syscall dispatch stays unchanged when off.**
  Constants and match arms are gated individually (`kernel/src/arch/x86_64/syscall.rs:125`, `:1249`),
  with the existing unknown-number ENOSYS fallback at `:1529`. The handlers (`:7901` onward),
  OpenFile/trait methods (`kernel/src/fs/mod.rs:175`, `:595`), and NeoFS modules
  (`kernel/src/fs/neofs/inode.rs:36`, `kernel/src/fs/neofs/mod.rs:16`) are gated as well.
  `kernel/Cargo.toml:1960` makes the feature opt-in and implies `cap-rooted-open`.
  This is a source/cfg assessment, not a newly run kernel binary-equivalence test. Libneo's
  unconditional number constants/wrappers remain available to callers, as for existing optional
  syscalls; an off kernel returns ENOSYS for them.

## Verification and handoff

- Ran at the reviewed head: `cargo test -p neofs-dir-ops-logic-tests --target x86_64-unknown-linux-gnu`.
  **13 passed, 0 failed**, including every-cut, no-barrier falsifier, free-space return, operation trees,
  name/depth rules and stat byte KATs. Unit tests completed in 43.56 seconds.
- Independently queried the Actions API: [run 43186671038](https://github.com/SecondStageTurbine/LogansOS/actions/runs/43186671038)
  reports completed/success for exactly `e2b5daff318648ef8db76e8d73ae842d44bdc295`.
  I did not launch a replacement four-shard run or claim the new concurrency discriminators ran.
- Read #106's actual PR body, including its CP rows and R3 notes. The required CP section is present;
  its two failed claims are evaluated above. This directory-ops review borrows no implementation code.
- RAG health at `http://127.0.0.1:8404/health` failed with connection refused. Used direct repository
  reads and exact-symbol searches; did not try to start the PL-managed service.
- R3 should retain the documented path-scoped-directory semantics and ancestor-readdir staleness,
  and account for F1's reload consistency and F2's namespace-wait behavior before handing out long-lived
  shared scopes. This RETURN changes documentation only; it does not repair #106.
