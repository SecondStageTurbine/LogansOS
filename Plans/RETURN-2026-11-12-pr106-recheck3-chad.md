<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Relay 405 — Chad, #106 superblock-pin re-check

**Dispatch:** `Plans/DISPATCH-2026-11-12-relays-405-406.md`, Section 405.
**Reviewed head:** `fbd75e50305e3bce1ca5d00963cf0d85a94a296b`, including `101d958f2bb6e2f303168adaf4bebd2f834dc5bb`.
**Previous RETURN:** `Plans/RETURN-2026-11-12-pr106-recheck2-chad.md`.
**RETURN branch:** `codex/pr106-recheck3-chad`, based on `origin/main`.

**Verdict: FAIL.** The ordinary cold-cache superblock-merge bug is fixed for devices containing the full superblock blocks. Two gaps remain: the pin and publish disagree about a copy in a device's final partial 4 KiB block; and the new “held completion” discriminator runs its nested writer before submitting the read, rather than holding a device completion. The CP-63 row overclaims both guarantees. No additional actual device read under `entries` was found in the four operations.

All code references are at the reviewed head, not at the RETURN branch's main base. In the checklist below, `cache.rs`, `inode.rs`, `mount.rs`, and `format.rs` mean `kernel/src/fs/neofs/`; `dir_ops.rs` means `kernel/src/fs/neofs/inode/dir_ops.rs`.

## Section 405 checklist

### 1. FAIL — mount pins all complete copy blocks, but not every copy publication selects

The ordinary path is correct: `mount.rs:83` calls pinning before returning the mounted filesystem at `:86`; `inode.rs:309` restricts it to writable mounts. `cache.rs:423` visits the three offsets from `format.rs:26`, reads a full block at `cache.rs:429`, and inserts it at `:432`. Read-only mounts cannot mutate and need no pin.

However, **pinning tests `offset + 4096 <= device_bytes` at `cache.rs:425`, while publication tests `offset + 512 <= device_bytes` at `inode.rs:3573`**. Those predicates are not equivalent.

**Concrete input:** a flush-capable device with 512-byte logical sectors and 131073 sectors (64 MiB + 512 bytes). Put a valid writable NeoFS volume of 16384 filesystem blocks in its first 64 MiB, with the required root children, valid free map, available space, and valid copies at 0 and 1 MiB. The last sector may be zero; mount can reject that copy and choose either valid earlier copy. `mount.rs:390` reads available 512-byte copies, and `:401` requires only that the declared filesystem fit the device, not that the device length be a multiple of 4096. `inode.rs:3324` reserves only copy blocks inside the declared filesystem. This geometry is not refused by those checks.

Mount pins blocks 0 and 256, but skips block 16384. A later root mkdir selects the third copy for publication anyway. This is a coverage defect in the new pin invariant; it is not an eviction of an already pinned block. A consistent writable-mount geometry/copy eligibility rule is needed.

### 2. PASS — clean-victim eviction excludes pinned blocks

`cache.rs:963` requires both `!block.dirty` and `!pinned_block(block.block)` before choosing the LRU victim. The predicate at `:1148` uses the exact copy-block membership at `:1158`. Draining a pin to clean does not make it eligible. Host extraction exercised this filter with three clean pinned blocks and an additional clean ordinary block; only the ordinary block was removed.

### 3. PASS — debug cold-cache clearing preserves pins

`inode.rs:883` drains then calls `forget_range(0, total_blocks)` at `:886`. Without cache eviction, `cache.rs:450` retains pinned blocks. With eviction enabled, `:470` excludes them before removal. These are the only range-removal branches; the other block removal is the protected clean-victim path above. Both feature configurations passed the extracted-method test. The cold-cache helper's “none in the cache” comment at `inode.rs:880` is now stale: the superblock blocks remain.

### 4. FAIL — normal cold-root mkdir merge is fixed, but “always a cache hit” is false

For a device with all three complete blocks, source replay of `debug_neofs_cold_cache(); mkdir_at(root_fd, "x")` reaches `dir_ops.rs:155`, root publication at `:482`, then `inode.rs:3576`. All three blocks remain present. `cache.rs:366` takes the hit branch, and the 512-byte merge at `:398` performs no device read. The extracted exact cache methods corroborate this for all three offsets, including preservation of the trailing 3584 bytes after cold clearing and eviction pressure.

**Failing input:** use item 1's 64 MiB + 512-byte device with `x` absent and a LOOKUP|WRITE root handle. After cold clearing, root mkdir reaches the third copy while holding `entries`. `inode.rs:3573` selects it, `cache.rs:366` misses, and `:378` attempts the full-block read. Crucially, `inode.rs:2707` rejects its out-of-device range **before issuing device I/O**. Thus this input disproves “always a cache hit” and causes `InvalidDeviceGeometry`; it is not evidence of an actual held device read. The test probe at `cache.rs:376` nevertheless counts that attempted read before the range check.

The host harness reproduced that exact pin/write mismatch. The complete kernel `mkdir_at` input was source-replayed, not executed: this host has no `qemu-system-x86_64`. No kernel runtime pass is claimed.

### 5. PASS — no other actual device read under entries found in F0

Re-audited `commit_dir_tree` and its callees:

- `dir_ops.rs:100` clones entries with a temporary guard; `:101` runs reads before acquiring the mutation guard at `:106`. Mkdir's chain is read at `:137`; unlink's chain, child and regular indirect-tree collection at `:179`, `:187`, `:193`; rename's chains and optional subtree walk at `:245`, `:246`, `:273` / `:426`; stat's chain and inode at `:328`, `:335`. Stat only releases the validated guard and returns at `:345`.
- `commit_dir_tree` collects directory blocks at `:449` using direct pointers already in memory (`inode.rs:2946`), changes pointers at `dir_ops.rs:458`, `:467`, `:473`, and writes directory versions at `:461`, `:487`. Empty versions use `:406`; nonempty versions use `inode.rs:1700` and `:1722`.
- Allocation takes in-memory metadata at `inode.rs:1512`, `:1517`, `:3227`, `:3231`. Freeing and rollback edit the map and deferred list at `:2971`, `:2998`. Neither reads the device. `write_inode_to_cache` expands the inode into a full 4096-byte buffer at `:2284`; cache full-block writes at `cache.rs:312` never read the device.
- The root publish at `dir_ops.rs:482` calls `inode.rs:1654`; directory data, inode and free-map writes are full blocks (`inode.rs:1700`, `:2286`, `:3523`). The only partial writes are the superblock copies at `:3576`. Complete copy blocks stay cached; item 4's incomplete tail is rejected before actual I/O.
- Handle reload follows operation return at `kernel/src/fs/mod.rs:700`; path resolution clones entries before its device reads (`inode.rs:331`). It does not retain the operation's guard.
- Inherited dirty throttling still drains under entries (`inode.rs:997`), but drain performs full-block writes and flushes (`cache.rs:1097`, `:1112`), not reads. OT-101's write/flush latency remains outside this narrow no-device-read conclusion.

### 6. PASS — cold mutation leg asserts zero held reads through commit

`kernel/src/test_harness.rs:34153` cold-clears before root `mkdir_at(root_fd, b"x")` at `:34155`; `:34159` cold-clears again before unlink at `:34161`. Both must succeed. The counter is checked only after both operations at `:34165`, and any held read fails at `:34166`; contention also fails at `:34172`. The `DirOpLock` probe lives through commit (`dir_ops.rs:55`, `:65`, `:155`, `:216`), and partial-write misses are instrumented at `cache.rs:376`. This is a substantive source-level assertion, not an executed kernel result on this host.

### 7. FAIL — nested writer precedes submission; no device completion is held

The new leg at `kernel/src/test_harness.rs:34184` arms one hook, performs a nested-directory mkdir at `:34186`, and requires two completed nested commits at `:34192`. Those are real root mkdir/unlink operations (`dir_ops.rs:760`, `:762`), with recursion suppressed at `:751`. It is useful evidence that the caller has released entries **at the read boundary**.

But `note_device_read` invokes the writer synchronously at `dir_ops.rs:737`. For `read_block`, that entire callback returns at `cache.rs:248` **before** the device-read call at `:249`; the prefix path likewise calls the hook at `:281` before reading at `:283`. The partial-write path does the same at `:376` / `:378`. No request has been submitted and no completion is pending while the nested writer runs. Neither the hook nor the harness holds/releases a driver completion or runs a concurrent writer during one.

**Concrete missing input:** cold-clear a valid mount, submit T1's first nested-directory inode read for `mkdir_at(f1_fd, "held")`, hold that request's completion after submission, then run T2's root mkdir/unlink and require both to finish before releasing T1. Finally release the completion and require T1 to finish/retry correctly. The present test instead finishes T2 before submitting T1's read. This is an evidence gap, not a demonstrated production deadlock.

### 8. FAIL — live CP-63 row includes pins but overstates the guarantee and discriminator

Read the actual [PR #106 body](https://github.com/SecondStageTurbine/LogansOS/pull/106) through GitHub REST; its head matched the reviewed SHA. The CP-63 row now names all three offsets, their 4 KiB pins, writable mount, eviction exclusion, and `forget_range` protection. That correctly describes full-block devices. However, its unconditional cache-hit claim fails on item 1's concrete geometry (`cache.rs:425` versus `inode.rs:3573`). Its fourth discriminator describes an outstanding device read, whereas the nested commits precede submission (`cache.rs:248`, `:249`; `dir_ops.rs:737`). Item 7's submitted-and-held input is still missing. The row needs both corrected eligibility and evidence wording (or an actual held-completion test).

### 9. PASS — bounded pin and normal teardown ownership

The predicate can identify only three blocks (`format.rs:26`, `cache.rs:1158`), each with 4096 payload bytes (`format.rs:11`): at most 12288 payload bytes plus ordinary cache metadata. Pinning adds no global registry, leaked reference, separate allocation owner or permanent reference count. A fresh state owns a fresh cache at `inode.rs:205`; the mount owns the state at `mount.rs:73`. `NeoFsState::drop` stops the writeback worker at `inode.rs:1873`; `cache.rs:767` requests stop, wakes it and joins at `:769`, and the worker exits at `:742`. The worker's cache reference and then the state's cache reference can be released normally. Remount creates a new cache; outstanding inode references retaining an old state follow existing mount lifetime semantics. This is an ownership audit, not a measured unmount leak test.

### 10. PASS — F1, bounded EAGAIN refusal, rights, one commit and kill-switch remain

- **F1:** `dir_ops.rs:614` holds the record lock while comparing generations, replacing inode block/record at `:616` / `:617`, and returning the held generation at `:619`. `kernel/src/fs/mod.rs:713` publishes only a newer handle generation. The shared host F1 test passed.
- **EAGAIN:** eight attempts (`dir_ops.rs:50`), read before lock (`:101`, `:106`), no locked fallback, `Contended` at `:113`, mapping to `Again` at `:796` and EAGAIN at `kernel/src/arch/x86_64/syscall.rs:8258`. Mutation edits follow successful `after_reads` returns at `dir_ops.rs:144`, `:207`, `:287`; refusal publishes no operation change or handle reload (`kernel/src/fs/mod.rs:699`).
- **Rights before reads:** `kernel/src/fs/mod.rs:609`, `:617`, `:634`, `:635`, `:657` call the common check before invoking NeoFS. LOOKUP intersection and mutation WRITE are enforced at `:684` through `:690`, including both rename handles.
- **One commit per successful mutation:** `dir_ops.rs:155`, `:216`, `:313` each converge on the one root publish at `:482`, returning immediately at `:484`. Its generation bump is `inode.rs:1664`; stat publishes nothing.
- **Kill-switch:** opt-in feature at `kernel/Cargo.toml:1981`, test implication at `:1984`; syscall numbers and dispatch gated at `kernel/src/arch/x86_64/syscall.rs:125` and `:1254`; OpenFile methods gated at `kernel/src/fs/mod.rs:607`, `:615`, `:625`, `:651`. New mount pinning is gated at `mount.rs:82`; with F0 off, `cache.rs:1152` makes the pin predicate false. This is source/cfg inspection, not a feature-off boot.

## Executed verification and limits

- Read the dispatch, prior RETURN, repository AGENTS guidance, fix diff, and relevant source at detached worktree `/tmp/pr106-fbd75e50`. Known paths and exact symbols sufficed; no semantic discovery was needed.
- `cargo test -p neofs-dir-ops-logic-tests --target x86_64-unknown-linux-gnu`: **14 passed, 0 failed**, unit tests 43.44 seconds. Includes F1, crash cuts and the no-barrier falsifier; does not execute kernel block-cache or driver code.
- Generated `/tmp/pr106-pin-harness.rs` with `/tmp/pr106-pin-harness-gen.py`, extracting the reviewed `write_bytes_at`, `pin_superblock_blocks`, `forget_range`, `evict_locked`, `pinned_block`, and `superblock_copy_of_block` bodies. Compiled and ran with `rustc --edition=2024 --cfg 'feature="neofs-dir-ops"'`, then again adding `--cfg 'feature="neofs-cache-evict"'`. Both passed the full-device assertions and reproduced the tail-device miss/error. Device, mutex, cache bookkeeping and scheduler plumbing were stubbed; range rejection matches `inode.rs:2707`. This is a callee test, not a full mount/syscall test.
- Exact harness inputs: 128 MiB device; pin three blocks; add an ordinary clean block and evict to capacity three; forget the full range; merge 512 bytes at each copy offset; require zero additional device reads and unchanged trailing bytes. Then a 67109376-byte device; require only two pins and the third 512-byte merge to attempt a 4096-byte read and return `InvalidDeviceGeometry` without actual device I/O. Both forget-range cfg branches were exercised.
- Public REST PR snapshot saved at `/tmp/pr106-recheck3-pr.json`. No PR-body edit, message, or implementation change was made.
- QEMU is unavailable on this host. No full cold-root mkdir run, held-device-completion run, unmount runtime test, or new CI success is claimed. The remaining failures above follow from source ordering and the reproduced cache predicate mismatch.
