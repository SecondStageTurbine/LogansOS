<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #47): narrow re-check of PR #105 items 8, 10, 18, 19 → Janus

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. This is a narrow re-check of your own round-1 findings (relay #44,
RETURN `5466f68d`: items 8, 10, 18 and 19 FAILED). Items 8 and 10 now touch per-CPU boot state (an AVX2 census
across CPUs), so this is also the cross-family lens for that SMP seam.

**Type:** review only. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `6b9df951` (pinned; PR #105, branch
`claude/rtl8125-ethernet`). Since your round, the author made no rebase. The commits are:
- `c20c91d3`: items 8, 10 and 19;
- `c1ee8d53`: item 18;
- `a59c79c6`: merge of `main` `436674dd`, which brings in #104 (acpi-loganfy) and shares no files with this PR;
- `6b9df951`: merge of `main` `9b6fe712`, Plans only.

## Setup

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/arthur-latency-recheck-janus
          git fetch origin && git checkout -B codex/arthur-latency-recheck-janus 6b9df951

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-23-arthur-latency-recheck-janus.md      # this file
git show 6b9df951:Plans/DISPATCH-2026-10-23-arthur-latency-review.md                # "Round 2" section at the end: R8, R10, R18, R19
git show 5466f68d:Plans/RETURN-2026-10-23-arthur-latency-review-janus.md            # your round-1 RETURN
git diff b619e769 c1ee8d53 -- kernel tools docs/concurrency-pitfalls.md
```

RAG (semantic code search) is available. Use it for "where is X done?" questions; grep is better for exact symbols.
Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it is down, note that in the RETURN and use
grep/Read; the PL manages the RAG. Example query: `$body=@{query='AVX2 census report_cpu_avx2 enable_sse ap_entry'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer R8, R10, R18 and R19 as written in the "Round 2" section of the review dispatch, each PASS or FAIL with
`file:line`. For a FAIL, give a concrete input or sequence. Also answer these four PM items:

- **P1. Fewer CPUs up than counted.** `publish_avx2_decision` uses `online_cpu_count()` (Limine's count). If an AP
  never reaches `enable_sse`, check that the decision stays Undecided, so every call runs its own local check, and
  is never Yes.
- **P2. Publish order.** In `kernel/src/main.rs`, the publish runs after `bring_up_aps` has returned with every AP
  past its `enable_sse`. Cite the wait. Check that no AP can poll for or run a matmul partition before its own
  `report_cpu_avx2`.
- **P3. The census word.** In `Avx2Census::update`, `report` and `publish`, check that:
  - every change is one read-modify-write;
  - a veto that lands after a Yes publish leaves No;
  - the reported count can't overflow into the low bits.
- **P4. CP-79.** The new pitfall entry in `docs/concurrency-pitfalls.md` states the trap and the guard correctly,
  and names a discriminator that exists in the tree: `threaded_matmul_parity` with `avx2=No` on `qemu64` and
  `avx2=Yes` on `-cpu max`.

A FAIL is reported only after you have re-checked it yourself.

## Close condition

- Commit `Plans/RETURN-2026-10-23-arthur-latency-recheck-janus.md` on your branch and **push it**, with the verdict
  (CLOSED or STILL OPEN, by item). Paste the pushed SHA to the PL.
- No build is needed, though `cargo test -p ai-ops-tests` with your round-1 mutation applied to
  `kernel/src/ai/decode_logic.rs` is the R18 evidence. **Never write the Zenbook's disk.**
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
