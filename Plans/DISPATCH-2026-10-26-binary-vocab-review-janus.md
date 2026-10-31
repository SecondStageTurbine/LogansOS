<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #73): post-merge review of the binary vocab, NEOVOCB1 (#135) → Janus

**MODEL:** GPT-6-Luna. **EFFORT:** high. This is a new on-disk format with a converter at image build and a loader in
Arthur. Contained code, Claude-authored, so it gets a Codex-family review. Under the format/ABI rule, the main evidence
is the independent byte oracle. **Do this after relay #72 (the A8 image build).**

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `87730200` (main; the squash of PR #135, gated as `a50296be`, run `40270293866`,
green).

## Setup

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/binary-vocab-review-janus
          git fetch origin && git checkout -B codex/binary-vocab-review-janus 8773020065f6909de01f6d800ce645d87bf0e8d2

git show origin/main:Plans/DISPATCH-2026-10-26-binary-vocab-review-janus.md   # this file
git show origin/main:Plans/NOTE-2026-10-26-arthur-binary-vocab-design-s1.md  # the format and its "As built" section
git diff 039e9e70 87730200 -- kernel userland tools scripts
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='NEOVOCB1 vocab_bin_logic validate convert_vocab_sidecar QWEN25_05B_VOCB'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give concrete bytes or input and the wrong result.

1. **Byte oracle.** The 136-byte hand-laid `NEOVOCB1` example is checked against both the writer and an independent
   construction, not only a round trip. Compare its bytes against the NOTE's format description field by field
   (magic, version, counts, offsets, endianness, alignment, padding).
2. **Validation before use.** Arthur's loader validates the whole file in one pass before it uses any of it:
   - every count and offset in bounds;
   - arrays sorted where the lookups assume they are;
   - ids dense;
   - merge parts and results are tokens;
   - no overlapping sections;
   - no integer overflow in size arithmetic.
   Give a malformed input that passes validation and then reads out of bounds or misbehaves, if you find one.
3. **Converter preconditions.** The xtask converter refuses a source that breaks the preconditions (dense ids; merge
   parts and results are tokens) and writes no image. It ran against the real Qwen sidecar, all 151,387 merges. The
   `.vocb` pin (5,010,512 bytes, `QWEN25_05B_VOCB`) is enforced at image build and at boot.
4. **No silent fallback.** If the `.vocb` is damaged, missing, or fails its pin, Arthur doesn't quietly parse the
   text vocab instead. Check what the user sees in that case (`arthur_boot_module_preload_vocab_damaged`).
5. **Equivalence.** The binary path tokenizes and detokenizes the same as the text parse over the corpus: the chat
   template plus a few hundred varied strings, including non-ASCII, specials and byte-fallback cases. It reproduces
   the kernel's raw 21-token boot prompt.
6. **Kill-switch.** With `arthur-vocab-binary` off, Arthur uses the text sidecar exactly as before #135 (the
   text-path controls pass).
7. **Reproducibility.** `scripts/rebuild-qwen25-vocab.py` rebuilds the sidecar from the named sdist and checks it
   against the `QWEN25_05B_VOCAB` pin itself. The script has an SPDX header.

## Close condition

- Commit `Plans/RETURN-2026-10-26-binary-vocab-review-janus.md` on your branch and **push it**, with PASS/FAIL per
  item and an overall verdict. Paste the SHA to the PL.
- You may run the host tests (`cargo test -p ai-ops-tests`, `cargo xtask vocab-bin`) if you want. A FAIL is reported
  only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
