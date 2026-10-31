<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #97): post-merge review of A9-B, the repetition guard for greedy decode (#156) → Jarvis

**MODEL:** GPT-6-Luna. **EFFORT:** high. One pure module and two guarded calls in the generation core. No SMP-core
change and no new syscall. Claude-authored (S1), so it gets a Codex-family review. **Do this after relays #89 and #92.**

**Type:** review only, post-merge. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `500606e9`, #156's squash on main
(PR head `0c158e16` plus a merge of main). Gate: batch 4, run `40446328993`, all jobs green.

**What it does.**
- **The penalty.** `kernel/src/ai/repetition_logic.rs` adds CTRL's repetition penalty over the distinct tokens in the
  last 64 generated: `l / p` if `l > 0`, else `l * p`.
- **The loop stop.** Once the last 16 generated tokens repeat an earlier run, the reply ends before the repeated copy.
- **Where.** Both sit in `generate_inventory_greedy` behind `decode-repetition-guard`, which is now in the product
  kernel.
- **The penalty is inert.** `DECODE_PENALTY = 1.0` until Qwen's `generation_config.json` is fetched byte-exact, so in
  the product only the loop stop is live.
- **The oracle.** `tools/real-lm-oracle/inventory_ref.py` writes an independent float64 oracle for the guard.

## Setup

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/a9-repetition-guard-review-jarvis
          git fetch origin && git checkout -B codex/a9-repetition-guard-review-jarvis 500606e9

git show origin/main:Plans/DISPATCH-2026-10-27-a9-repetition-guard-review-jarvis.md   # this file
git diff 500606e9^ 500606e9
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='repetition penalty loop stop generate_inventory_greedy'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For any FAIL, give a concrete input.

1. **The penalty rule.**
   - Each distinct token in the window is penalized once, not once per occurrence.
   - Only generated tokens are in the window, as the PR says. Say whether the prompt should count, and what
     transformers does.
   - At `p = 1.0`, no logit changes bit for bit, including −0.0 and NaN.
2. **The loop stop.**
   - It fires only on a true repeat of the last 16 generated tokens.
   - It cuts before the repeated copy, not inside the original.
   - It can't index out of range or underflow when fewer than 16 tokens exist or when runs overlap.
   - A legitimate answer that repeats a phrase shorter than 16 tokens is never cut.
3. **Wiring.**
   - The penalty applies before the greedy pick and the stop after it, on every decode step, in the one core that
     `sys_generate` and the KV-persist KAT share.
   - When the stop fires, the reply returned to Arthur ends cleanly: the right length, and no stale tokens past the
     cut.
4. **The oracle is independent.** `inventory_ref.py`'s guard shares no code with `repetition_logic.rs`, and the
   `real_lm_generate_guard` tokens `[11,0,3,4,10,3]` come from it, not from the kernel.
5. **Kill-switch and off ≡ today.**
   - With `decode-repetition-guard` off, the code is compiled out.
   - `real_lm_generate_boot` still gives `[11,0,3,3,3,3,3,3]`.
   - The `guard_at_the_product_penalty_picks_today_s_tokens` KAT would fail if the guard changed a pick at 1.0
     without stopping.
6. **Prior art.** The PR body's survey table follows CLAUDE.md's "Unfamiliar features" rule: llama.cpp, transformers
   and vLLM (or the equivalent), with license, last commit date and SHA, and how each one's rule compares. The PM asked
   S1 to add it on 2026-10-27; FAIL if it's still missing.

## Close condition

- Commit `Plans/RETURN-2026-10-27-a9-repetition-guard-review-jarvis.md` on your branch and **push it**, with PASS/FAIL
  per item and an overall verdict. Paste the SHA to the PL.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
