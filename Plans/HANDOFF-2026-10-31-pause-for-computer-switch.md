<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Pause for a computer switch (2026-10-31)

J.c. is moving his work to another computer. Everything pauses once it's in a safe state. Nothing below needs a
decision; it is the checklist for leaving the old PC and the list of what to set up on the new one.

## What lives where

- **Cloud (unaffected by the switch):** the PM, and seats A, B, C and D. They run in claude.ai cloud sessions,
  push to GitHub, and keep their history. Pausing them means they've finished their current step, pushed it and
  posted a status comment on each PR. Nothing of theirs is on the PC.
- **The PC (has to move):**
  - the relay runner (`scripts/relay-runner.sh`) and its three engines: Codex (Chad, Jarvis, Janus), DeepSeek
    (`dsh`, Dexter) and OpenCode with the local Qwen model (Prometheus);
  - the worker folders `E:\AI Automation\workers\<coder>` and the runner state in
    `E:\AI Automation\workers\.relay-runner\`;
  - the RAG (`E:\AI Automation\RAG\`: Ollama, Qdrant, the API on port 8404);
  - the PL reference tree `E:\AI Automation\AltimitNeo`;
  - the bare-metal tools (PuTTY, the image stick).

## Before leaving the old PC

1. **Let the runner finish relays 222 (Jarvis, #254) and 223 (Dexter, #247/#253).** Batches 40 and 41 are
   gate-green and merge on those two reviews. If the PM has confirmed both are back, stop the runner.
2. **Stop the runner** (all three `ONLY=` processes).
3. **Run the tree-state audit** (CLAUDE.md, "Work is only real when it is on `origin`"):

   ```powershell
   $git = "C:\Program Files\Git\bin\git.exe"
   Get-ChildItem "E:\AI Automation\workers" -Directory | ForEach-Object {
     $p=$_.FullName
     [PSCustomObject]@{ Worker=$_.Name; Branch=(& $git -C $p branch --show-current 2>$null);
       Dirty=((& $git -C $p status --porcelain 2>$null)|Measure-Object).Count;
       Unpushed=((& $git -C $p log --oneline "@{u}.." 2>$null)|Measure-Object).Count }
   } | Format-Table -AutoSize
   ```

   Every row must show `Dirty = 0` and `Unpushed = 0`. Anything else is potentially the only copy: commit and push
   it first. Check `git cherry origin/main HEAD` before believing a folder is a stale duplicate, and never
   force-push to fix a rejected push.
4. **Copy the things that aren't in git:**
   - the relay runner's local settings (its environment and any paths it uses);
   - the Codex, `dsh` and OpenCode logins and configs;
   - the Qwen model file (`strata/qwen3.8-flash-next-iq3_s`) if the new PC will run it locally;
   - the RAG's index (or plan to rebuild it with `python -m src.ingest --full`).

## Setting up the new computer

1. Git, and clone `https://github.com/SecondStageTurbine/LogansOS.git` fresh for each worker:
   `E:\AI Automation\workers\<coder>` (siblings, outside the repo; one per coder). Check that every `origin`
   points at GitHub, not a local path (the stale-dispatch trap).
2. The three CLIs and their logins: Codex (`codex exec`, with `daybreak=false`, full access, `approval_policy=never`,
   as on the old PC), DeepSeek `dsh`, and OpenCode with the Qwen model.
3. The relay runner from `scripts/relay-runner.sh` on main (`e6cc61cc` and later), started as three processes
   (`ONLY=codex`, `ONLY=deepseek`, `ONLY=qwen`). The queue is `Plans/RELAY-QUEUE.md` on main. Relays already
   answered have RETURN files on main; tell the PM the highest relay number the old runner finished, so it can
   re-queue anything lost (relays 215-218 were lost once already).
4. The RAG (optional; coders fall back to grep).
5. Tell the PM when the runner is up. The PM resumes the merge loop from the ledger
   (`Plans/LEDGER-2026-07-29-logansos-release-ledger.md`), the last entries of which list every open PR and its
   state.

## State at the pause (also in the ledger)

- **Merged overnight:** #259, #252, #240, #243, #265, #248, #245.
- **Gate-green, waiting on one review:** batch 40 (#267, #241, #250, #254: Jarvis 222 and seat C's PR-text fixes);
  batch 41 (#247, #253: Dexter 223).
- **Gating:** batch 42 (#249 alone; stays draft until seat D's paperwork lands).
- **With seats:** seat A, the kernel-wide user-copy fault fix (a bad user pointer halts the kernel); seat B, the
  unsaved-Always follow-up; seat C, the OT-90 stack (#255-#258) after #267 merges; seat D, #249 paperwork and
  #261's clang-in-CI blocker.
- **Decided by J.c. today:** Unicode-3.0 allowed; build-time Python fine; the Lens keeps a browse-only `/`.
