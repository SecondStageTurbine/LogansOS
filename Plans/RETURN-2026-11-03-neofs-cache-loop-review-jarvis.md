<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Return 300: NeoFS `find_cached_block` review

Reviewed branch: `claude/neofs-cache-lookup-loop`
Reviewed head: `4e8e6aa400cff552edfc990f68d2e3a5d128a69e`
PR: none open when checked; reviewed the fetched branch directly.

- **PASS — 1.** `kernel/src/fs/neofs/cache.rs:757-765`: the loop checks index 0 upward, returns at the first match, and reaches `None` for missing and empty inputs. The last index is checked because the condition is `idx < blocks.len()` before access. The branch has no host KAT pinning these cases, so this is a code inspection result only.
- **PASS — 2.** `kernel/src/fs/neofs/cache.rs:757-765`: the branch changes only this helper and its comment. Its callers and surrounding cache code are unchanged from `origin/main`, so lock, borrow, and ordering behavior is unchanged.
- **FAIL — 3.** `kernel/src/fs/neofs/cache.rs:757-765`: the branch adds no host KAT. Add a host test with blocks `[10, 20, 30]` asserting `20 => Some(1)`, `99 => None`, and `30 => Some(2)`, plus an empty slice asserting `99 => None`; compare each result with the former `iter().position` behavior.
- **FAIL — 4.** `kernel/src/fs/neofs/cache.rs:752-756`: no PR is open, so there is no PR body or cited run set to verify. The source comment claims `install_fresh_4k` changed from about 145 s to 100 s locally under TCG, but gives no separate main/branch/fix results or run references. It also does not name the unbounded linear scan with no eviction as a follow-up. Provide the three comparable timing results and their run references, and state that the unbounded scan/no-eviction issue remains follow-up work.
