# SPDX-License-Identifier: MPL-2.0
"""B1c (F2): an independent reference for the real-LM generation oracle.

Reads the committed tiny inventory model (`kernel/src/ai/fixtures/
kv_persist_kat.neom`) through the byte layout in ADR-028, and runs a textbook
Qwen2 decoder step in float64 numpy: RMSNorm, rotate-half RoPE, grouped-query
causal attention, SwiGLU, untied output head. Greedy argmax.

It shares no code with the kernel (or with `neomc`). Its common inputs are
the `.neom` bytes, the architecture's published definition, and two facts
ADR-028 does not carry: the head size (64, the fixture's documented geometry)
and the GGUF dims order (dims[0] contiguous).

Limits (B1c AAG): the fixture is all F32 with 2 query and 2 KV heads, so this
oracle does not exercise the quantized kernels or a GQA group above 1 (the
0.5B is 14:2); those are covered by the qwen35/llama.cpp golden and the
real-model runs, not here. The
kernel's `real_lm_generate_boot` scenario asserts its tokens equal the vector
this script commits (`expected_tokens.txt`), and that a one-token change to
the prompt changes the result.

Usage:
    python3 -I tools/real-lm-oracle/inventory_ref.py [--check]

`--check` recomputes and compares with the committed file instead of writing
it (exit 1 on any difference).

A9-B: it also writes `expected_guard_tokens.txt`, the same model's greedy
decode under the repetition guard (kernel `decode-repetition-guard`), written
here from the rule's definition and sharing no code with
`kernel/src/ai/repetition_logic.rs`. The penalty is CTRL's (Keskar et al.
2019, section 4.1): over the distinct tokens among the last WINDOW generated, a
logit l becomes l / p if l > 0 else l * p. The loop stop ends the reply before
its last NGRAM tokens once they repeat an earlier run in the reply. Both are
applied in float64 like the forward, at the kernel scenario's penalty and at
1.0 (the product value until Qwen's published one is read).
"""

import argparse
import struct
import sys
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[2]
MODEL = ROOT / "kernel/src/ai/fixtures/kv_persist_kat.neom"
EXPECTED = Path(__file__).resolve().parent / "expected_tokens.txt"
EXPECTED_GUARD = Path(__file__).resolve().parent / "expected_guard_tokens.txt"

PROMPT = [1, 5, 9, 2]
CONTROL_PROMPT = [1, 5, 9, 3]
MAX_NEW = 8
# The fixture's documented geometry (tools/neomc/src/fixtures.rs): two query
# heads and two KV heads of 64.
HEAD_DIM = 64


def read_neom(path):
    """Parse a v2 .neom per ADR-028: header, TensorEntry table, string heap."""
    raw = path.read_bytes()
    assert raw[0:4] == b"NEOM", "magic"
    version, header_size, flags, tensor_count = struct.unpack_from("<IIII", raw, 4)
    node_count, input_count, output_count = struct.unpack_from("<III", raw, 0x14)
    assert version == 2 and header_size == 108 and flags == 0, (version, header_size)
    assert node_count == 0 and input_count == 0 and output_count == 0, "inventory model"
    (tensor_table_off,) = struct.unpack_from("<Q", raw, 0x20)
    string_off, string_len = struct.unpack_from("<QQ", raw, 0x38)
    (total_size,) = struct.unpack_from("<Q", raw, 0x58)
    assert total_size == len(raw), "total_size"
    layer_count, rope_theta, rms_eps = struct.unpack_from("<Iff", raw, 0x60)

    tensors = {}
    for i in range(tensor_count):
        base = tensor_table_off + 96 * i
        dtype, rank = struct.unpack_from("<II", raw, base)
        dims = struct.unpack_from("<8Q", raw, base + 0x08)[:rank]
        weight_off, weight_len = struct.unpack_from("<QQ", raw, base + 0x48)
        name_off, name_len = struct.unpack_from("<II", raw, base + 0x58)
        assert dtype == 0, "the fixture is all F32"
        assert name_off + name_len <= string_len
        name = raw[string_off + name_off : string_off + name_off + name_len].decode()
        count = int(np.prod(dims))
        assert weight_len == 4 * count
        data = np.frombuffer(raw, dtype="<f4", count=count, offset=weight_off).astype(np.float64)
        # GGUF/ADR order: dims[0] is the contiguous (input) axis, so a rank-2
        # tensor is stored as dims[1] rows of dims[0]: a [out, in] matrix.
        tensors[name] = data.reshape(tuple(reversed(dims))) if rank == 2 else data
    return tensors, layer_count, float(rope_theta), float(rms_eps)


def rms_norm(x, weight, eps):
    return x / np.sqrt(np.mean(x * x) + eps) * weight


def rope(x, position, theta):
    """Rotate-half RoPE on one head vector of length HEAD_DIM."""
    half = HEAD_DIM // 2
    inv_freq = theta ** (-np.arange(half, dtype=np.float64) * 2.0 / HEAD_DIM)
    angle = position * inv_freq
    cos, sin = np.cos(angle), np.sin(angle)
    x1, x2 = x[:half], x[half:]
    return np.concatenate([x1 * cos - x2 * sin, x2 * cos + x1 * sin])


def silu(x):
    return x / (1.0 + np.exp(-x))


def forward_all(tensors, layers, theta, eps, tokens):
    """Logits for the last position of `tokens` (full causal recompute)."""
    hidden = [tensors["token_embd.weight"][t].copy() for t in tokens]
    for layer in range(layers):
        p = f"blk.{layer}."
        wq, wk, wv = (tensors[p + n] for n in ("attn_q.weight", "attn_k.weight", "attn_v.weight"))
        wo = tensors[p + "attn_output.weight"]
        n_q = wq.shape[0] // HEAD_DIM
        n_kv = wk.shape[0] // HEAD_DIM
        group = n_q // n_kv
        qs, ks, vs = [], [], []
        for pos, h in enumerate(hidden):
            x = rms_norm(h, tensors[p + "attn_norm.weight"], eps)
            q, k, v = wq @ x, wk @ x, wv @ x
            qs.append([rope(q[i * HEAD_DIM : (i + 1) * HEAD_DIM], pos, theta) for i in range(n_q)])
            ks.append([rope(k[i * HEAD_DIM : (i + 1) * HEAD_DIM], pos, theta) for i in range(n_kv)])
            vs.append([v[i * HEAD_DIM : (i + 1) * HEAD_DIM] for i in range(n_kv)])
        out = []
        for pos, h in enumerate(hidden):
            heads = []
            for qh in range(n_q):
                kh = qh // group
                scores = np.array(
                    [qs[pos][qh] @ ks[j][kh] for j in range(pos + 1)]
                ) / np.sqrt(HEAD_DIM)
                weights = np.exp(scores - scores.max())
                weights /= weights.sum()
                heads.append(sum(weights[j] * vs[j][kh] for j in range(pos + 1)))
            h = h + wo @ np.concatenate(heads)
            x = rms_norm(h, tensors[p + "ffn_norm.weight"], eps)
            gate = tensors[p + "ffn_gate.weight"] @ x
            up = tensors[p + "ffn_up.weight"] @ x
            h = h + tensors[p + "ffn_down.weight"] @ (silu(gate) * up)
            out.append(h)
        hidden = out
    x = rms_norm(hidden[-1], tensors["output_norm.weight"], eps)
    return tensors["output.weight"] @ x


def generate(tensors, layers, theta, eps, prompt):
    context = list(prompt)
    tokens, margins = [], []
    for _ in range(MAX_NEW):
        logits = forward_all(tensors, layers, theta, eps, context)
        order = np.argsort(-logits, kind="stable")
        tokens.append(int(order[0]))
        margins.append(float(logits[order[0]] - logits[order[1]]))
        context.append(int(order[0]))
    return tokens, margins


# A9-B: the guarded decode's parameters (kernel repetition_logic).
GUARD_MAX_NEW = 48
GUARD_WINDOW = 64
GUARD_NGRAM = 16
# The kernel scenario's test penalty, and the product's current one.
GUARD_PENALTIES = (1.3, 1.0)


def generate_guarded(tensors, layers, theta, eps, prompt, penalty):
    """Greedy decode with the penalty applied before each pick and the loop
    stop after it. Returns the kept tokens, whether the loop stop ended the
    reply, and the top1 - top2 margin of every pick (after the penalty)."""
    context = list(prompt)
    generated, margins = [], []
    for _ in range(GUARD_MAX_NEW):
        logits = forward_all(tensors, layers, theta, eps, context).copy()
        seen = set()
        for token in generated[-GUARD_WINDOW:]:
            if token in seen:
                continue
            seen.add(token)
            logits[token] = logits[token] / penalty if logits[token] > 0 else logits[token] * penalty
        order = np.argsort(-logits, kind="stable")
        pick = int(order[0])
        margins.append(float(logits[order[0]] - logits[order[1]]))
        generated.append(pick)
        context.append(pick)
        n = len(generated)
        if n > GUARD_NGRAM:
            tail = generated[n - GUARD_NGRAM :]
            if any(generated[s : s + GUARD_NGRAM] == tail for s in range(n - GUARD_NGRAM)):
                return generated[: n - GUARD_NGRAM], True, margins
    return generated, False, margins


def render_guard(runs):
    lines = [
        "# A9-B guarded-decode oracle: tools/real-lm-oracle/inventory_ref.py over",
        "# kernel/src/ai/fixtures/kv_persist_kat.neom (float64, greedy, repetition guard).",
        f"guard_prompt={','.join(map(str, PROMPT))} max_new={GUARD_MAX_NEW} "
        f"window={GUARD_WINDOW} ngram={GUARD_NGRAM}",
    ]
    for penalty, (tokens, stopped, margins) in runs:
        tag = f"p{penalty:.1f}".replace(".", "_")
        lines += [
            f"{tag}_tokens={','.join(map(str, tokens))}",
            f"{tag}_loop_stop={'yes' if stopped else 'no'} generated={len(margins)}",
            f"{tag}_min_margin={min(margins):.6f}",
        ]
    return "\n".join(lines) + "\n"


def render(tokens, control, margins):
    lines = [
        "# B1c real-LM oracle: tools/real-lm-oracle/inventory_ref.py over",
        "# kernel/src/ai/fixtures/kv_persist_kat.neom (float64, greedy).",
        f"prompt={','.join(map(str, PROMPT))} max_new={MAX_NEW}",
        f"tokens={','.join(map(str, tokens))}",
        f"control_prompt={','.join(map(str, CONTROL_PROMPT))}",
        f"control_tokens={','.join(map(str, control))}",
        "# top1 - top2 logit per step (a small margin could flip under f32 reordering):",
        "margins=" + ",".join(f"{m:.6f}" for m in margins),
        f"min_margin={min(margins):.6f}",
    ]
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    tensors, layers, theta, eps = read_neom(MODEL)
    tokens, margins = generate(tensors, layers, theta, eps, PROMPT)
    control, _ = generate(tensors, layers, theta, eps, CONTROL_PROMPT)
    if control == tokens:
        sys.exit("the control prompt must change the tokens (the oracle could not fail)")
    text = render(tokens, control, margins)
    guard = render_guard(
        [(p, generate_guarded(tensors, layers, theta, eps, PROMPT, p)) for p in GUARD_PENALTIES]
    )
    if args.check:
        if EXPECTED.read_text() != text:
            sys.exit("expected_tokens.txt differs from a fresh run")
        if EXPECTED_GUARD.read_text() != guard:
            sys.exit("expected_guard_tokens.txt differs from a fresh run")
        print("expected_tokens.txt and expected_guard_tokens.txt match")
        return
    EXPECTED.write_text(text)
    EXPECTED_GUARD.write_text(guard)
    sys.stdout.write(text + guard)


if __name__ == "__main__":
    main()
