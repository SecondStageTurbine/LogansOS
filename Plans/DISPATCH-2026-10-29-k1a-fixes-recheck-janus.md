<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #126): narrow re-check of #174, the K1a fixes for your #112 F1/F2 → Janus

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. This re-checks your own earlier finding.

**Reviewed SHA:** `57b7960e` (#174's squash on `main`; batch 12b run `40787440872`, green). **Deliverable:**
`Plans/RETURN-2026-10-29-k1a-fixes-recheck-janus.md`, a plain checklist (pass or fail, `file:line`, a concrete input
for any fail).

1. **F1.** readdir, mount and umount now pass the handle's rights through `file_lookup_with_rights`, and the
   `OpenFile` method checks OpenFile ∩ handle after its type check, so ENOTDIR/EISDIR come before EACCES.
   - Do your concrete inputs now fail? A LOOKUP-only derived root on readdir; a client root without MOUNT on umount.
   - S5 found that `sys_read` had the same defect (a WRITE-only copy could read) and fixed it too. Check that, and
     every other `file_lookup(…, empty())` S5 listed. `fsync` stays rights-free on purpose; is that safe?
2. **F2.** The directory kind is cached in the `OpenFile` at open, so `is_directory()` takes no lock under spawn's
   IF-off section, and the own-dir branch checks it too. Is any other inode lock still reachable under that section?
3. **Discriminators.** `cap_rooted_open` steps 7, 8 and 9 (readdir, mount/umount, read). S5 couldn't run QEMU, and
   red on the old code was source-traced only. The PM's gate ran them green on the fix. Does each step fail on the
   pre-fix code by your reading?

```
WORKTREE: E:\AI Automation\workers\janus   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/k1a-fixes-recheck-janus
          git fetch origin && git checkout -B codex/k1a-fixes-recheck-janus origin/main

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-k1a-fixes-recheck-janus.md
git show origin/main:Plans/RETURN-2026-10-27-k1a-rootdircap-review-janus.md
```

RAG health check: `Invoke-RestMethod http://127.0.0.1:8404/health`; note it if it's down. Push the RETURN. Before
ending, run `git status --porcelain` and `git log @{u}..`; both must be empty.
