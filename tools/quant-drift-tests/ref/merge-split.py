# SPDX-License-Identifier: MPL-2.0
"""Merge a split quant-drift `run` into the one TSV a single run writes.

Usage: merge-split.py SPLIT_DIR OUT.tsv

SPLIT_DIR holds `part*.tsv` (prompts-only runs that together cover every
prompt once) and `window.tsv` (the run with the corpus window; it carries one
prompt too). The output has window.tsv's header and NLL fields and every
prompt's rows, prompts sorted by id as a single `run` writes them.

It refuses a merge unless:
- no prompt appears in two parts, and the parts cover the pinned prompt set
  (the Q4_K_M KAT's ids);
- every header field other than the corpus fields and the prompt count is the
  same in every file (model, sha256, source, tool, window settings);
- the prompt in window.tsv, computed in a separate process, is identical in
  every row to the same prompt in the parts (the determinism check).
"""
import glob
import os
import sys

PER_PROMPT = {
    "prompt", "prompt_ids", "generated", "generated_logit", "generated_2nd",
    "generated_2nd_logit", "top1", "top1_logit", "top1_2nd", "top1_2nd_logit",
}
CRATE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PINNED = os.path.join(CRATE, "fixtures", "quant-drift-kat-2026-11-08-control.ours.tsv")


def parse(path):
    head, body = [], {}
    for line in open(path).read().split("\n"):
        if not line:
            continue
        fields = line.split("\t")
        if fields[0] in PER_PROMPT:
            body.setdefault(fields[1], []).append(line)
        else:
            head.append(line)
    return head, body


def keyed(head):
    return dict(line.split("\t", 1) for line in head)


def main(split_dir, out_path):
    parts = [parse(p) for p in sorted(glob.glob(os.path.join(split_dir, "part*.tsv")))]
    window = parse(os.path.join(split_dir, "window.tsv"))
    merged = {}
    for _, body in parts:
        twice = set(body) & set(merged)
        if twice:
            sys.exit(f"refusing: prompts in two parts: {sorted(twice)}")
        merged.update(body)
    pinned = set(parse(PINNED)[1])
    if set(merged) != pinned:
        sys.exit(f"refusing: parts cover {len(merged)} prompts, not the pinned {len(pinned)}")
    win_head = keyed(window[0])
    for head, _ in parts:
        for key, value in keyed(head).items():
            if key.startswith("corpus_") or key == "prompts":
                continue
            if win_head.get(key) != value:
                sys.exit(f"refusing: header {key} differs between files")
    for prompt_id, rows in window[1].items():
        if rows != merged.get(prompt_id):
            sys.exit(f"refusing: {prompt_id} differs between the window run and the parts")
    out = []
    for line in window[0]:
        key = line.split("\t", 1)[0]
        out.append(f"prompts\t{len(merged)}" if key == "prompts" else line)
    for prompt_id in sorted(merged):
        out.extend(merged[prompt_id])
    open(out_path, "w").write("\n".join(out) + "\n")
    print(f"merged {len(merged)} prompts; {sorted(window[1])} identical across processes")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit("usage: merge-split.py SPLIT_DIR OUT.tsv")
    main(sys.argv[1], sys.argv[2])
