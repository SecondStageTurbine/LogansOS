#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Full-precision (float32) reference for the quantization-drift KAT.

Loads the pinned `Qwen/Qwen2.5-0.5B-Instruct` safetensors in float32 through
transformers and consumes the SAME token IDs as the other two runners
(`prompts.tsv`, `corpus.u32`), so no tokenizer is involved here.

Usage:
  fp32_ref.py MODEL_DIR PROMPTS_TSV CORPUS_U32 OUT_TSV \
      [--eval-start 4096] [--corpus-tokens 8192] [--corpus-blake3 HASH]
"""

import argparse
import math
import sys

import torch
from transformers import AutoModelForCausalLM


def read_prompts(path):
    prompts = []
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            if not line.startswith("prompt_ids\t"):
                continue
            _, pid, ids = line.rstrip("\n").split("\t")
            prompts.append((pid, [int(part) for part in ids.split(",")]))
    return prompts


def read_u32(path):
    with open(path, "rb") as handle:
        data = handle.read()
    assert len(data) % 4 == 0
    return list(torch.frombuffer(data, dtype=torch.uint32).tolist())


def logsumexp(row):
    return float(torch.logsumexp(row, dim=0))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("model_dir")
    parser.add_argument("prompts")
    parser.add_argument("corpus")
    parser.add_argument("out")
    parser.add_argument("--label", default="llama_f32")
    parser.add_argument("--source", default="")
    parser.add_argument("--sha256", default="")
    parser.add_argument("--tool", default="transformers float32; ref/fp32_ref.py")
    parser.add_argument("--eval-start", type=int, default=4096)
    parser.add_argument("--corpus-tokens", type=int, default=8192)
    parser.add_argument("--corpus-blake3", default="")
    parser.add_argument("--no-generate", action="store_true", help="skip greedy decode (use another runner's generated IDs)")
    args = parser.parse_args()

    torch.set_num_threads(4)
    model = AutoModelForCausalLM.from_pretrained(
        args.model_dir, torch_dtype=torch.float32, low_cpu_mem_usage=True
    )
    model.eval()
    vocab = model.config.vocab_size

    prompts = read_prompts(args.prompts)
    corpus = read_u32(args.corpus)[: args.corpus_tokens]
    assert len(corpus) >= args.corpus_tokens, "corpus is too short"

    out = []
    out.append("schema\tquant-drift-ref-v1")
    out.append(f"model\t{args.label}")
    out.append(f"model_sha256\t{args.sha256}")
    out.append(f"model_source\t{args.source}")
    out.append(f"tool\t{args.tool}")
    out.append(f"corpus_tokens\t{args.corpus_tokens}")
    out.append(f"corpus_eval_start\t{args.eval_start}")
    out.append(f"corpus_slice_blake3\t{args.corpus_blake3}")
    out.append(f"prompts\t{len(prompts)}")

    with torch.no_grad():
        for pid, ids in prompts:
            input_ids = torch.tensor([ids], dtype=torch.long)
            result = model(input_ids=input_ids, use_cache=True)
            logits = result.logits[0]
            top1 = [int(torch.argmax(row)) for row in logits]
            generated = []
            if not args.no_generate:
                past = result.past_key_values
                next_logits = logits[-1]
                for _ in range(32):
                    token = int(torch.argmax(next_logits))
                    generated.append(token)
                    step = model(
                        input_ids=torch.tensor([[token]], dtype=torch.long),
                        past_key_values=past,
                        use_cache=True,
                    )
                    past = step.past_key_values
                    next_logits = step.logits[0, -1]
            out.append(f"prompt\t{pid}\t")
            out.append(f"prompt_ids\t{pid}\t{','.join(map(str, ids))}")
            out.append(f"generated\t{pid}\t{','.join(map(str, generated))}")
            out.append(f"top1\t{pid}\t{','.join(map(str, top1))}")
            first = generated[0] if generated else "-"
            print(f"prompt {pid}: {len(ids)} tokens, first={first}", file=sys.stderr)

        nll_sum = 0.0
        nll_count = 0
        past = None
        chunk = 512
        for begin in range(0, len(corpus), chunk):
            piece = corpus[begin : begin + chunk]
            result = model(
                input_ids=torch.tensor([piece], dtype=torch.long),
                past_key_values=past,
                use_cache=True,
            )
            past = result.past_key_values
            logits = result.logits[0]
            for offset in range(len(piece)):
                index = begin + offset
                if index < args.eval_start or index + 1 >= len(corpus):
                    continue
                row = logits[offset]
                nll_sum += logsumexp(row) - float(row[corpus[index + 1]])
                nll_count += 1
            print(f"corpus {min(begin + chunk, len(corpus))}/{len(corpus)}", file=sys.stderr)
    out.append(f"corpus_nll_sum\t{nll_sum:.17f}")
    out.append(f"corpus_nll_count\t{nll_count}")

    with open(args.out, "w", encoding="utf-8") as handle:
        handle.write("\n".join(out) + "\n")
    print(
        f"{args.label}: PPL {math.exp(nll_sum / nll_count):.6f} over {nll_count} targets",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
