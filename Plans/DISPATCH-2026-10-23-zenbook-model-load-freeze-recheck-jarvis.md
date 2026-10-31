<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #32): narrow re-check of PR #99 item 6 (F1) → Jarvis

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. This is a narrow re-check of your own earlier finding (F1, relay #31
RETURN `bf85b939`).

**Type:** review only. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `18f8f4a2` (pinned).

## Setup

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/zenbook-model-load-freeze-recheck-jarvis
          git fetch origin && git checkout -B codex/zenbook-model-load-freeze-recheck-jarvis 18f8f4a28cbe74106ab16eeeb1fffa9930b489a7

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-23-zenbook-model-load-freeze-recheck-jarvis.md   # this file
git show bf85b939:Plans/RETURN-2026-10-23-zenbook-model-load-freeze-review-jarvis.md         # your RETURN
git diff 5255cb0c 18f8f4a2 -- kernel/ docs/concurrency-pitfalls.md Plans/NOTE-2026-10-23-zenbook-model-load-freeze-s1.md
```

What changed since your review: `kernel/src/test_harness.rs` (`run_boot_model_load_stack`), `kernel/Cargo.toml` (the
scenario feature now implies `real-lm`), and the NOTE / CP-74 text. The other commits since `52148e22` are a
`main` merge (#93's code and docs) and do not touch this lane's code.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete input.

1. **F1 closed.** Check that the probe hashes an allocation this function owns exclusively; that every byte is
   initialised before the slice is formed; and that no interrupt handler or other CPU can write it while
   `blake3::hash` borrows it.
2. **Same measured path.** Check that each size is still ONE `blake3::hash` call over the full prefix, inside
   `inference_stack::run_checked`, with the same depth reporting.
3. **Allocation.** Check that adding `real-lm` to `neo-test-boot-model-load-stack` gives a heap large enough for
   the 485 MB buffer, that a failed reservation fails the scenario cleanly (no panic, no hang), and that the feature
   change affects only this scenario.
4. **Wording.** Check that the NOTE and CP-74 now say (a) the watermark measures the deepest written bytes, not
   total syscall stack use, and (b) with preload off the full-size load is still exposed, a rollback, not a fix
   (your 6a/6b notes).
5. **Nothing else moved.** Check that `git diff 5255cb0c 18f8f4a2 -- kernel/` touches only the two files above.

A FAIL is reported only after you have re-checked it yourself.

## RAG

The RAG index on the PL's machine is being rebuilt tonight (it had been indexing a July checkout). Results may be
incomplete until the rebuild finishes. Use grep and direct reads for this check; treat RAG as a convenience only.
Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`.

## Close condition

- Commit `Plans/RETURN-2026-10-23-zenbook-model-load-freeze-recheck-jarvis.md` on your branch and **push it**,
  with the verdict (CLOSED or STILL OPEN) and each item's result.
- Paste the pushed SHA to the PL.
- No build is needed; if you build, `cargo clean`. **Never write the Zenbook's disk.**
