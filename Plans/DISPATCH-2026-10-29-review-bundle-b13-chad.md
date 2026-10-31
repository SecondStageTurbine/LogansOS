<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW BUNDLE (relays #135–#136) → Chad: #180's cut KAT and #181's NVMe spec rows

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. This is a bundle, so one model throughout (CLAUDE.md bundle rule). On its
own each would be GPT-6-Luna high: contained code, no SMP core.

Both merged after batch 13 (run `40799093977`, green at `0395982f`). Review each squash on `main`:
`git show <sha> --stat` and `git show <sha>`. Muse does the security reviews of both; yours is the independent
Codex read. One pushed RETURN per relay, each a plain checklist (pass or fail, the enforcing `file:line`, a concrete
input for any fail): `Plans/RETURN-2026-10-29-<slug>-review-chad.md`.

## #135: #180 `c805f2b2`, the NeoFS cut KAT. Slug `neofs-cut-kat`

`tools/neofs-commit-logic-tests` mounts the real `kernel/src/fs/neofs/commit_logic.rs` by `#[path]`.
1. **The disk model.** A cut lands any subset of unflushed writes, and a flush can fail after landing a subset. Does
   it model a real write-back cache, or does it miss an ordering a real disk allows?
2. **The volume model.** Does its mount check what `mount.rs` checks: the root, the directory, every file's inode and
   data hash, and the free map against the live set?
3. **Coverage.** Power is cut before every write and flush of the four scripts. Every landing subset is tried up to
   12 pending writes, then sampled. Is the sampling enough to catch the failures `main`'s order had (2,044 of 24,574)?
4. **The falsifiers.** `main`'s single-flush order and "barrier, no deferred reuse" both fail. Do they fail for the
   reason the PR says, "no superblock copy mounts"?
5. **Model vs kernel.** Does the KAT drive the same `next_drain_step` decisions `cache.rs` uses, or a copy?

## #136: #181 `ee629498`, the NVMe spec rows. Slug `nvme-spec-rows`

The PR body has a table, S1–S13, of NVMe facts the driver relies on, each marked "to confirm". The author checked
the byte layouts against SPDK `include/spdk/nvme_spec.h` (BSD-3-Clause) but couldn't reach the spec itself.
1. **For each row S1–S13:** does the NVMe Base Specification (2.0 or later) say what the row says? Give the section
   or figure. Mark it pass, fail, or not checkable.
2. **The code uses each row as stated.** Check `nvme_event_logic.rs` (`namespace_ids`, `AerResult::decode`,
   `get_log_page_cdw10`, `register_verdict`) and `nvme.rs` (`arm_async_events`, `service_async_events`).
3. **Exact bytes come from the raw source.** If you check a byte offset, read it from the spec PDF or SPDK's header
   with `git`/`curl`, never a summarizing fetch.

```
MODEL: GPT-6.1-Sol. EFFORT: high. (A bundle: one model throughout, per the bundle rule.)
WORKTREE: E:\AI Automation\workers\chad   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/review-bundle-2026-10-29b-chad
          git fetch origin && git checkout -B codex/review-bundle-2026-10-29b-chad origin/main

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-review-bundle-b13-chad.md
```

RAG health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that and read the files
directly. Push both RETURNs. Before ending, run `git status --porcelain` and `git log @{u}..`; both must be empty.
