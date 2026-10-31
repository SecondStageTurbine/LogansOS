<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE (design, not built): a prebuilt binary vocab, so Arthur maps its tokenizer instead of parsing it (S1, 2026-10-26)

**Where this comes from.** A7 on the Zenbook (main `745dd314`, HWP on, 3.3 GHz):
`arthur=10790 vocab=10792..10792 model=10986`. The vocab read finishes inside one tick; the remaining
**1.94 s from Arthur's spawn to its model handle is the vocab parse**. #126 made that parse about 60×
faster (`build-std` opt-levels). What is left is the work itself:
- hex-decoding 151,936 tokens (and, on the real model, about 151k merges) from a 5.4 MB text sidecar;
- then hundreds of thousands of small heap allocations into three `BTreeMap`s (`tokenizer_logic.rs`
  `BpeTokenizer`).

This NOTE designs removing the parse rather than speeding it up further. **Nothing here is built yet.**

## The idea

At image build, `xtask` converts the pinned text sidecar (`model-qwen2.5-0.5b.vocab`, format
`NEOM_TOKENIZER 1`) once into a binary file, `model-qwen2.5-0.5b.vocb`. It is laid out exactly as the
tokenizer reads it: id → bytes, bytes → id, and merges by token-id pair, all in sorted arrays. Arthur
reads it into one buffer, validates it in one linear pass, and uses it in place: no hex, no per-token
allocation, no map building. Encode and decode keep the same algorithm (byte-level BPE, lowest-rank pair
first); only the lookups change from `BTreeMap` to binary search over the arrays.

**Expected:** validation is one pass over about 4 MB, a few milliseconds at opt 3. That brings
spawn → model from about 2 s to the read time plus a few ms.

## Format `NEOVOCB1` (version 1)

All integers little-endian. Sections start 8-byte aligned (padding bytes are zero) and appear in this
order. Every offset is from the start of the file.

**Header** (64 bytes):

| Offset | Size | Field |
|---|---|---|
| 0x00 | 8 | magic `NEOVOCB1` |
| 0x08 | 4 | version = 1 |
| 0x0C | 4 | header_size = 64 |
| 0x10 | 4 | token_count N (ids are dense, `0..N`) |
| 0x14 | 4 | merge_count M |
| 0x18 | 4 | bos id, or `0xFFFF_FFFF` for none |
| 0x1C | 4 | eos id, or none |
| 0x20 | 4 | pad id, or none |
| 0x24 | 4 | flags = 0 (reserved; must be 0) |
| 0x28 | 8 | total_size (== file length) |
| 0x30 | 4 | offsets_off |
| 0x34 | 4 | blob_off |
| 0x38 | 4 | sorted_off |
| 0x3C | 4 | merges_off |

**Sections:**
- **offsets**: `N + 1` × u32. Token `i` is `blob[offsets[i] .. offsets[i+1]]`. `offsets[0] = 0`,
  non-decreasing, and `offsets[N]` is the blob's length.
- **blob**: the token bytes, concatenated by id. Tokens are stored in the byte-level-mapped form the
  encoder looks up (as in the text sidecar today).
- **sorted**: `N` × u32, the ids ordered by their token bytes (unsigned lexicographic). Encode's
  bytes → id is a binary search here.
- **merges**: `M` × `{left: u32, right: u32, result: u32, rank: u32}`, sorted by `(left, right)`.
  `best_pair` turns each adjacent symbol pair into ids and binary-searches here for the rank.
  `result` is the merged token's id, so a merge never builds a new byte string.

**What the reader validates** (any failure ⇒ `VocabLoad::Unparseable`, as for a bad text sidecar today):
- magic, version, header_size, flags = 0, total_size = file length;
- every section in bounds, aligned, in order, not overlapping;
- offsets non-decreasing with `offsets[N]` = blob length;
- `sorted` is a permutation of `0..N`, strictly increasing by bytes (so tokens are unique);
- merges strictly increasing by `(left, right)`, every id `< N`, ranks unique and `< M`, and
  `bytes(left) ++ bytes(right) == bytes(result)`;
- bos/eos/pad `< N` or none.

All of it is linear (merges also need a rank bitmap), and the reader never trusts an offset it has not
bounded.

**What the converter requires of the source.** In byte-level BPE, every merge part and every merge
result is a vocab token. `xtask` checks that for each merge and **refuses to convert** otherwise. It
also refuses non-dense ids. The QEMU fixture vocab is dense (0..151,935) with 0 merges. The real Qwen
sidecar's merges must be checked at conversion: this container has no real sidecar to check.

## Byte oracle (the ABI rule)

A tiny vocab, laid out by hand and independent of any writer: the five tokens of `vocab_sidecar.rs`'s
`build_test_sidecar`, plus eos = 4.

| id | token | bytes |
|---|---|---|
| 0 | `\n` | `0A` |
| 1 | `Ġ` (mapped space) | `C4 A0` |
| 2 | `h` | `68` |
| 3 | `i` | `69` |
| 4 | `hi` | `68 69` |

One merge, rank 0: `h` + `i` → `hi`, i.e. `{left 2, right 3, result 4, rank 0}`.

```
0x00  4E 45 4F 56 4F 43 42 31  01 00 00 00  40 00 00 00   NEOVOCB1, v1, header 64
0x10  05 00 00 00  01 00 00 00  FF FF FF FF  04 00 00 00   N=5, M=1, bos none, eos 4
0x20  FF FF FF FF  00 00 00 00  88 00 00 00 00 00 00 00   pad none, flags 0, total 136
0x30  40 00 00 00  58 00 00 00  60 00 00 00  78 00 00 00   offsets@64 blob@88 sorted@96 merges@120
0x40  00 00 00 00  01 00 00 00  03 00 00 00  04 00 00 00   offsets 0,1,3,4,
0x50  05 00 00 00  07 00 00 00                             5,7
0x58  0A C4 A0 68 69 68 69 00                              blob (7 bytes + 1 pad)
0x60  00 00 00 00  02 00 00 00  04 00 00 00  03 00 00 00   sorted: \n(0) h(2) hi(4) i(3)
0x70  01 00 00 00  00 00 00 00                             Ġ(1), pad
0x78  02 00 00 00  03 00 00 00  04 00 00 00  00 00 00 00   merge {2,3,4,0}
                                                           total 0x88 = 136 bytes
```

**Required tests** (in the build lane):
1. The converter's output for this sidecar equals these 136 bytes exactly (a literal in the test, not
   computed by the writer).
2. The reader on these bytes yields encode(`"hi"`) = `[4]`, encode(`"h i"`) = `[2, 1, 3]`, and
   decode(`[4]`) = `"hi"`, matching the text tokenizer's own KAT.
3. Equivalence: for the fixture vocab, and the real sidecar where available, text-built and binary-built
   tokenizers give the same encode and decode over a fixed corpus, the same control-token set and
   `reply_tokens`.
4. Rejection: every validation rule above has a mutated-byte case that the reader refuses.

## Shipping it

- **Boot module.** `xtask iso --with-model` stages `/vocab/model-qwen2.5-0.5b.vocb` beside the text
  sidecar, with its own pinned digest (B1c's module pins). The converter must be deterministic (sorted
  output, no hash-map iteration order), so the pin is stable across builds.
- **Kill-switch.** An Arthur feature, `arthur-vocab-binary`. Off: Arthur loads the text sidecar exactly
  as today, with no change in behaviour or bytes. On: Arthur opens the `.vocb` file.
  - A missing or invalid `.vocb` is reported as the vocab's state (absent/damaged/unparseable, the B1c
    states). It does **not** silently fall back to the text file, so a broken conversion can't hide
    behind a slow path.
- **Code shape.** `BpeTokenizer`'s lookups go behind a small trait, or an enum over two backings: the
  `BTreeMap`s built from text, and a borrowed view of the binary file. Encode and decode stay one
  implementation, so equivalence is structural, not just tested.
- **Memory.** One buffer of about 4 MB (offsets 0.6 MB, sorted 0.6 MB, merges 1.8 MB at the real
  size, blob about 1 MB), against today's ~600k small allocations.

## Scope (in gates, not time)

- **Host-only:** the converter, the reader and all four test kinds.
- **Kernel/QEMU:** none, beyond staging one more boot module.
- **Review:** GPT-6-Luna high (a new on-disk format with a byte oracle).
- **Acceptance:** an image's overlay shows `vocab=O..C` with `model` within a few ticks of C.

## As built (S1, `claude/arthur-binary-vocab`)

The format is as above, byte for byte (the 136-byte oracle is a literal in `tools/ai-ops-tests`). Where the
build differs from or adds to the design:

- **The reader is stricter than the list above.** Each section must start exactly where the previous one ends,
  rounded up to 8 (the writer's only layout); padding bytes must be zero; and offsets must *strictly*
  increase, because the text tokenizer refuses an empty token too.
- **Encode looks merges up by bytes, as the text tables do.** `BpeTables::merge_rank(left, right)` maps both
  parts to ids (two binary searches) and then searches the merges. `result` is validated
  (`left ++ right == result`) but encode does not use it yet. This keeps one algorithm over both backings;
  an id-based merge loop is a later speed-up, not needed for parse_ms.
- **The converter also refuses ranks other than exactly `0..M`, and a pair merged twice.**
- **Real sidecar, checked in this container.** The proxy blocks Hugging Face, but llama.cpp's vocab-only
  `models/ggml-vocab-qwen2.gguf` ships in the `llama-cpp-python` 0.3.36 sdist on PyPI. Its tokens and
  151,387 merges, with Qwen2.5's 19 added specials at 151646–151664 (`<|object_ref_start|>` …
  `<|file_sep|>`) and `bos 151643 / eos 151645 / pad 151643`, written in the `NEOM_TOKENIZER 1` format,
  give **exactly** the pinned sidecar: 7,990,862 bytes, BLAKE3 `e8974121…c05265`. Against it:
  - the converter's preconditions hold (every merge part and result is a token);
  - text and binary agree over the corpus, on every id's decode and control status, and on the chat
    template; the binary reproduces the kernel's raw-captured 21-token boot prompt;
  - its NEOVOCB1 is **5,010,512 bytes, BLAKE3 `7973d739…e6cee015`**, now the product pin
    `QWEN25_05B_VOCB`.
  Rebuild: `scripts/rebuild-qwen25-vocab.py <ggml-vocab-qwen2.gguf> <out.vocab>` (it checks the size and
  BLAKE3 against the pin), then `cargo xtask vocab-bin <out.vocab> <out.vocb>`. Re-run the equivalence:
  `NEO_REAL_VOCAB=<out.vocab> cargo test --release -p ai-ops-tests binary_equals_text_on_the_real`.
- **Boot.** The product image (`iso --with-model`) and the `arthur_boot_module_preload` pair stage only
  `/vocab/model-qwen2.5-0.5b.vocb`. The kernel's preload accepts a model when every staged form of its vocab
  verifies and at least one is staged. The other `arthur_boot_module_*` scenarios keep the text sidecar
  (the kill-switch-off path).
