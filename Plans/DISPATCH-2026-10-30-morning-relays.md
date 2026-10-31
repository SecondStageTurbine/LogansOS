<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Morning queue for J.c. (2026-10-30)

The PM fills this file overnight as reviews come due. Each entry is a ready-to-paste relay. Ask the PM "what do you
need?" and it hands them over in one message, along with the overnight summary.

## To do with J.c. this morning: try the WSL route for the Codex workers

Goal: let the cloud PM reach the worker folders directly, instead of J.c. pasting every relay.
1. In WSL (Ubuntu), clone `https://github.com/SecondStageTurbine/LogansOS.git` into a Linux path (not `/mnt/e/...`).
2. In that folder, run `claude remote-control`, or add the folder under Remote Control in the Claude desktop app.
3. The PM calls `list_computer_folders`, then `request_computer_folder`, and J.c. approves the connection.
4. Test: from that folder, can the PM run `codex` (installed in WSL, or the Windows `codex.exe` through WSL interop)
   with the right model and effort, in the right worker tree?

Caveats to settle first:
- this gives the cloud session command access on the PC;
- it works only while the PC is on;
- each connection needs J.c.'s approval;
- the per-coder worktree rule still applies.

## Relays queued overnight

Paste each block as-is. All the dispatch files are on `main`.

### 1. Dexter: re-check of #224's security fix (relay #187)
```
MODEL: (Dexter's usual) · security re-check of Claude-authored credential code (your #186 items 2 and 4)
WORKTREE: E:\AI Automation\workers\dexter
BRANCH:   dexter/login-recheck-2026-10-30   (git fetch origin && git checkout -B dexter/login-recheck-2026-10-30 origin/main)
git fetch origin +pull/224/head:pr224      (head f82d2b3d)
Your review: git show origin/main:Plans/RETURN-2026-10-30-product-login-review-dexter.md
Say CLOSED / STILL OPEN with file:line:
 1. Item 4: every path to a file under .system/ is EPERM without the SystemStore cap: root and relative opens, opening
    .system as a directory and then a child, SYS_MODEL_LOAD, SYS_FILE_REPLACE, readdir, and path tricks (./, //, ..).
    Can the cap be granted, derived, inherited, or sent over a channel?
 2. Item 2: the failure count (NEOFAIL1, two copies) is written before each check, survives a reboot, can't be reset
    by a torn or corrupt copy (it fails closed to the higher count), and resets only on success, unlock or recovery.
 3. The PM AAG's notes, for your view: (a) the product has no cap-rooted-open, so any process can unmount neofs (when
    no handles are open, fs/mod.rs:772) and mount a different writable NeoFS volume (a USB stick) carrying its own
    .system/ record, which the lock then checks (lock.rs:363). Is that reachable, and what should close it? (b) The
    lock's show() sets wait_until = 0 (lock.rs:300), so the first try after a lock skips a saved delay.
 4. Anything new in the diff since 45798eb3.
Commit Plans/RETURN-2026-10-30-login-recheck-dexter.md, push it, and paste the SHA.
RAG: Invoke-RestMethod http://127.0.0.1:8404/health; if down, note it and use grep/Read.
```

### 2. Chad: #183, post-merge re-checks of #227, #229 and #230 (now merged)
```
MODEL: GPT-6.1-Sol · EFFORT: high (bundle)
WORKTREE: E:\AI Automation\workers\chad
BRANCH:   codex/b24-recheck-chad   (git fetch origin && git checkout -B codex/b24-recheck-chad origin/main)
git fetch origin && git show origin/main:Plans/DISPATCH-2026-10-29-b24-rechecks.md   (section #183)
Merged as: #227 df55d40b, #229 6d01cb1d, #230 af164506. Update Plans/RETURN-2026-10-29-b24-recheck-chad.md, push it, and paste the SHA.
RAG: Invoke-RestMethod http://127.0.0.1:8404/health; if down, note it and use grep/Read.
```

### 3. Jarvis: #184, post-merge re-check of #228 (now merged as 0769274c)
```
MODEL: GPT-6.1-Sol · EFFORT: medium (narrow re-check of your own finding)
WORKTREE: E:\AI Automation\workers\jarvis
BRANCH:   codex/mem-quota-leaf-recheck-jarvis   (git fetch origin && git checkout -B codex/mem-quota-leaf-recheck-jarvis origin/main)
git fetch origin && git show origin/main:Plans/DISPATCH-2026-10-29-b24-rechecks.md   (section #184)
Update Plans/RETURN-2026-10-29-mem-quota-leaf-recheck-jarvis.md, push it, and paste the SHA.
RAG: Invoke-RestMethod http://127.0.0.1:8404/health; if down, note it and use grep/Read.
```

### 4. Janus: pre-merge bundle, #188 (#226), #190 (#236), #191 (#238) and #193 (#237). All are in batch 26.
**Overnight update:** the PM AAG bounced **#226** (`scheduler::reap` still drains, via `finish_kill`'s early return
and `on_thread_reap`) and **#238** (the exit claim is held across the `sys_wait` park; a killed waiter leaks it). Both
are back with seats B and A. **Both fixed heads are in (05:50 UTC); send the whole bundle.** (#226 gets one more one-line follow-up for its `on_thread_reap_threaded` drop; review the latest #226 head when you fetch.) Also check each PM bounce comment on #226 and #238.
```
MODEL: GPT-6.1-Sol · EFFORT: high (a bundle; SMP-core kill and exit code; pre-merge)
WORKTREE: E:\AI Automation\workers\janus
BRANCH:   codex/b26-recheck-janus   (git fetch origin && git checkout -B codex/b26-recheck-janus origin/main)
git fetch origin +pull/226/head:pr226 +pull/236/head:pr236 +pull/237/head:pr237 +pull/238/head:pr238   (heads: #226 3e8e3239, #236 894ce31e, #237 aa6ee599, #238 fa152231 — batch 28 green; batch 30 gates the #226 follow-up)
Your reviews: git show origin/main:Plans/RETURN-2026-10-29-k1a0-grant-pin-recheck-janus.md
              git show origin/main:Plans/RETURN-2026-10-29-ot98-recheck-janus.md   (item 7)
Say CLOSED / STILL OPEN with file:line:
 1. #182 item 2: does the deferred drain run only from the reaper's top level (sched::reap_and_drain from main.rs's
    loops), never from a reap nested in NeoFS under `entries` or the cache `drain` guard? Is IF=1 asserted there?
 2. #182 item 4 and #180 item 7: does every push onto DEFERRED_PROCESS_DROPS go through defer_process_drop, which
    always publishes DEFERRED_PENDING, including #219's release_scan_pin (#219 is on main as ad1150f3)?
 3. Does the F1d leg fail with the publish removed (neo-test-deferred-pending-control)?
 3b. #193, #237 (install_cap drops a refused handle after CAP_GRAPH, not under it): does every refusal path in
    install_minted_locked, install_cap_excluding and complete_grant drop the handle outside the lock? Does the control trip?
 4. #190, #236 (OT-93/OT-98 follow-ups): does the spawn-failure path (a kill landing between set_process_anchor and a
    failed try_push_ready) now complete the kill, exactly once?
 5. #191, #238 (#171: an in-flight wait keeps its claim; a failed inherited table releases its handles' claims): can an
    exit code be freed while a waiter is between finding it and reading it? Are claimant counts released exactly once?
    Does it compose with #219's DYING/finish_kill and #226's deferred drops?
One RETURN per relay: Plans/RETURN-2026-10-30-{k1a0-recheck2,ot98-followups,exit-claims,install-cap-drop}-janus.md. Push them and paste the SHAs.
RAG: Invoke-RestMethod http://127.0.0.1:8404/health; if down, note it and use grep/Read.
```

### 5. Chad: pre-merge bundle, #189 (#218 wire-RX) and #192 (#222 settings WM)
**Overnight update:** the PM AAG bounced both. In **#218**, `identity::snapshot_links` can spin forever with IF off
in the product build (a `reserve_exact` bug). In **#222**, any app can turn off the idle lock through OP_SETTING
(security). They're back with seats C and B, and batch 27 was cancelled. **Both fixed heads are in (about 06:00 UTC); send it.** Also check the PM bounce comments
on #218 (the snapshot_links spin) and #222 (the idle-lock disable).
```
MODEL: GPT-6.1-Sol · EFFORT: high (a bundle)
WORKTREE: E:\AI Automation\workers\chad
BRANCH:   codex/b27-recheck-chad   (git fetch origin && git checkout -B codex/b27-recheck-chad origin/main)
git fetch origin +pull/218/head:pr218 +pull/222/head:pr222   (heads: #218 fe02d28f, #222 ac23143a (rebased; code = e25734bd) — batch 29 green)
#189, a narrow re-check of your #173 (Plans/RETURN-2026-10-29-ot96-aag-chad.md) and #175 (Plans/RETURN-2026-10-29-net-tick-review-chad.md):
 CLOSED / STILL OPEN with file:line for #173 F1 (IF-on takers), F2 (bounded flush and virtio used-index), F3 (listener
 retirement via #230, merged), the CP-56 row; and for #175 items 1, 2, 4, 5 and 7. See seat C's two RETURN comments
 on PR #218.
#192, #222 (OT-88 S1, the WM side of the NEOSET1 settings store, plus OP_SETTING). A plain checklist:
 1. Is the record parsed fail-closed (defaults, never a panic)?
 2. Can a torn write across the two copies apply a half-written setting?
 3. Which settings can a client change through OP_SETTING? Can an app turn off the idle lock?
 4. Is the byte oracle hand-laid?
 5. Is "off is bit-identical" true?
One RETURN per relay: Plans/RETURN-2026-10-30-{ot96-recheck2,settings-wm-review}-chad.md. Push them and paste the SHAs.
RAG: Invoke-RestMethod http://127.0.0.1:8404/health; if down, note it and use grep/Read.
```

(More may be added below during the night: Janus on #226 once #219 merges, and Chad on #235 / #218.)
