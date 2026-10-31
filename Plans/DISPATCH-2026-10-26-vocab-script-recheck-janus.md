<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #77): narrow re-check of your relay #73 item 7, after #139 → Janus

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. This is a narrow re-check of your own finding. Relay #73 item 7 showed
that `scripts/rebuild-qwen25-vocab.py` accepted a different same-sized sidecar when the Python `blake3` module was
missing (it fell back to a size-only check). #139 (S1) makes the digest check required. It's scripts only.
**Do this now.** The A9 image-build relay follows once batch 2 merges; if it arrives first, build A9 first.

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `11fbd161` (main; the squash of PR #139, PR CI run `40351847332` green, host + SPDX;
no kernel/userland/tools change, so no QEMU shards).

## Setup

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/vocab-script-recheck-janus
          git fetch origin && git checkout -B codex/vocab-script-recheck-janus 11fbd161b9d76d74bb5c977751720e1d186f8482

git show origin/main:Plans/DISPATCH-2026-10-26-vocab-script-recheck-janus.md   # this file
git show origin/main:Plans/RETURN-2026-10-26-binary-vocab-review-janus.md      # your relay #73
git diff 11fbd161~1 11fbd161 -- scripts/rebuild-qwen25-vocab.py
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give the command and its output.

1. **Your probe.** Re-run your #73 harness (`relay73_missing_blake3_probe.py`, the token-1000 same-size change) with
   `python -S`, so `blake3` is missing. The script must exit non-zero and say the digest couldn't be checked. It must
   not print "size matches".
2. **With `blake3` present.** The altered sidecar fails on the digest; the real sidecar passes against
   `QWEN25_05B_VOCAB` (size and BLAKE3).
3. **No other way to skip.** No flag, environment variable or exception path turns the digest check off or reports
   success without it. A failure leaves no output file that looks finished (or the script says clearly that the file
   is unverified).
4. **The pins match the kernel.** The size and BLAKE3 in the script are the ones in `kernel/src/boot_model_logic.rs`.
5. **SPDX header** is still the first line after the shebang.

## Close condition

- Commit `Plans/RETURN-2026-10-26-vocab-script-recheck-janus.md` on your branch and **push it**, with PASS/FAIL per item
  and an overall verdict. Paste the SHA to the PL.
- A FAIL is reported only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
