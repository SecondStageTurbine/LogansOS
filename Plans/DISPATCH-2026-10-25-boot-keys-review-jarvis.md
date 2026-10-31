<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #55): post-merge review of the boot-keys cut and startup timeline (#118) → Jarvis

**MODEL:** GPT-6-Luna. **EFFORT:** high. This is contained kernel code: the i8042 driver, one hook in
`focus::activate`, and watchdog-only hooks. There's no SMP core and no new syscall. It's Claude-authored, so it
gets a Codex-family review.

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `911a75bc` (main; the squash of PR #118, gated as `7ff0dd54`, run `40169275618`,
23/23).

## Setup

```
WORKTREE: E:\AI Automation\workers\jarvis    (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/boot-keys-review-jarvis
          git fetch origin && git checkout -B codex/boot-keys-review-jarvis 911a75bc61601a8ffbe73476c789c929240739f8

git show origin/main:Plans/DISPATCH-2026-10-25-boot-keys-review-jarvis.md   # this file
git diff 5f31dbf2 911a75bc -- kernel tools
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's
down, note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='i8042 pump boot cut tail_after_cut activate'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete byte or call sequence.

1. **The cut is taken once.** Only the first keyboard-route activation records the cut point. A later activation
   (Arthur's route, a handoff back to Home) never records a new one, so keys typed at the desktop are never dropped.
2. **The cut arithmetic.** `tail_after_cut` (`i8042_logic.rs`) is correct across the ring's u32 index wrap. It never
   skips a byte received after the cut point. When the cut is older than the ring's capacity, it never reads past
   HEAD.
3. **The reset is complete.** At the cut, the decoder's held keys and prefix state, the modifiers, the SAK filter, the
   stage and the autorepeat are all reset, so no key or modifier stays held. The A5-replay host KAT covers a held key
   whose break was lost.
4. **Lock and IRQ context.** The cut runs under the pump lock. The `dropped N` log line is written after the lock is
   released, in a stage-only log section (CP-71). The ISR path is unchanged and takes no new lock.
5. **Kill-switch.** With `ps2-boot-keys-legacy` on, no cut is recorded and the backlog is delivered exactly as before.
   Name the check that proves it (.text identity or a test).
6. **The timeline hooks.** Each one is a few relaxed atomics, with no lock, no allocation and no wake, and is compiled
   only under `freeze-watchdog`. The line fits the Zenbook's overlay width.
7. **Gate coverage.** Name the QEMU scenarios that exercise this path (`ps2_keyboard`, `ps2_key_repeat`,
   `desktop_home_boot` …) and say what they don't cover, e.g. real i8042 timing on silicon.

## Close condition

- Commit `Plans/RETURN-2026-10-25-boot-keys-review-jarvis.md` on your branch and **push it**, with PASS/FAIL per
  item and an overall verdict. Paste the SHA to the PL.
- No build is needed. A FAIL is reported only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
