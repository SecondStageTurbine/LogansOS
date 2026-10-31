#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Rebuild the pinned Qwen2.5-0.5B vocab sidecar (NEOM_TOKENIZER 1) without
the model GGUF, from llama.cpp's vocab-only GGUF.

Source: the `llama-cpp-python` 0.3.36 sdist on PyPI
(llama_cpp_python-0.3.36.tar.gz), member
`vendor/llama.cpp/models/ggml-vocab-qwen2.gguf`. Its 151,936 tokens and
151,387 merges are Qwen2's; Qwen2.5 adds 19 special tokens at 151646-151664
(that GGUF has `[PADnnnnnn]` there) and sets bos/eos/pad = 151643/151645/151643.

The output is checked against the kernel's pin (`QWEN25_05B_VOCAB`,
kernel/src/boot_model_logic.rs): 7,990,862 bytes, BLAKE3 e8974121...c05265.
The digest check is required: without the `blake3` module (pip install
blake3) the script fails and writes nothing. The output file is written only
after the digest matches, and every run first proves the check refuses a
same-size body with one byte changed.

    python3 -I scripts/rebuild-qwen25-vocab.py ggml-vocab-qwen2.gguf out.vocab
    python3 -I scripts/rebuild-qwen25-vocab.py --self-test   # the check alone
    cargo xtask vocab-bin out.vocab out.vocb   # the NEOVOCB1 form and its pin
"""
import struct
import sys

PIN_LEN = 7_990_862
PIN_BLAKE3 = "e89741214838ae7af6277122c1093e899138884ea3bea4f83ff5671e76c05265"
QWEN25_SPECIALS = [
    "<|object_ref_start|>", "<|object_ref_end|>", "<|box_start|>", "<|box_end|>",
    "<|quad_start|>", "<|quad_end|>", "<|vision_start|>", "<|vision_end|>",
    "<|vision_pad|>", "<|image_pad|>", "<|video_pad|>", "<tool_call>", "</tool_call>",
    "<|fim_prefix|>", "<|fim_middle|>", "<|fim_suffix|>", "<|fim_pad|>",
    "<|repo_name|>", "<|file_sep|>",
]
FIRST_SPECIAL = 151646


def read_gguf_metadata(data):
    """The GGUF header's key/value metadata (no tensors are read)."""
    pos = 0

    def rd(fmt):
        nonlocal pos
        (v,) = struct.unpack_from("<" + fmt, data, pos)
        pos += struct.calcsize("<" + fmt)
        return v

    def rstr():
        nonlocal pos
        n = rd("Q")
        s = data[pos:pos + n]
        pos += n
        return s

    scalars = {0: "B", 1: "b", 2: "H", 3: "h", 4: "I", 5: "i", 6: "f", 7: "?",
               10: "Q", 11: "q", 12: "d"}

    def rval(t):
        if t in scalars:
            return rd(scalars[t])
        if t == 8:
            return rstr()
        if t == 9:
            et = rd("I")
            return [rval(et) for _ in range(rd("Q"))]
        raise ValueError(f"unknown GGUF value type {t}")

    if data[:4] != b"GGUF":
        raise ValueError("not a GGUF file")
    pos = 4
    rd("I")  # version
    rd("Q")  # tensor count
    kv = {}
    for _ in range(rd("Q")):
        key = rstr().decode()
        kv[key] = rval(rd("I"))
    return kv


def load_blake3():
    """The `blake3` module, or exit non-zero: the pin is a digest, and a size
    match alone accepts a same-size sidecar with any token changed."""
    try:
        import blake3
    except ImportError:
        sys.exit(
            "error: the `blake3` Python module is required to check the output "
            "against its pin (install it: pip install blake3); nothing was written"
        )
    return blake3


def check(body, blake3):
    """None if `body` is the pinned sidecar, else why not."""
    if len(body) != PIN_LEN:
        return f"{len(body)} bytes, the pin is {PIN_LEN}"
    got = blake3.blake3(body).hexdigest()
    if got != PIN_BLAKE3:
        return f"blake3 {got}, the pin is {PIN_BLAKE3}"
    return None


def self_test(blake3):
    """The check must refuse a same-size body with one byte changed, and a
    body one byte short. Exits non-zero if it accepts either."""
    probe = bytearray(PIN_LEN)
    if check(bytes(probe), blake3) is None:
        sys.exit("self-test: an all-zero body passed the pin check")
    probe[PIN_LEN // 2] ^= 1
    if check(bytes(probe), blake3) is None:
        sys.exit("self-test: a same-size altered body passed the pin check")
    if check(bytes(PIN_LEN - 1), blake3) is None:
        sys.exit("self-test: a short body passed the pin check")


def main():
    blake3 = load_blake3()
    self_test(blake3)
    if sys.argv[1:] == ["--self-test"]:
        print("self-test: the pin check refuses altered and short bodies")
        return
    if len(sys.argv) != 3:
        sys.exit("usage: rebuild-qwen25-vocab.py <ggml-vocab-qwen2.gguf> <out.vocab> | --self-test")
    src, dst = sys.argv[1], sys.argv[2]
    kv = read_gguf_metadata(open(src, "rb").read())
    tokens = list(kv["tokenizer.ggml.tokens"])
    merges = kv["tokenizer.ggml.merges"]
    for i, name in enumerate(QWEN25_SPECIALS):
        tokens[FIRST_SPECIAL + i] = name.encode()
    # The layout of neomc's write_tokenizer_export (tools/neomc/src/gguf_import.rs).
    out = ["NEOM_TOKENIZER\t1\n", "bos\t151643\n", "eos\t151645\n", "pad\t151643\n"]
    out.append(f"tokens\t{len(tokens)}\n")
    out += [f"{i}\t{t.hex()}\n" for i, t in enumerate(tokens)]
    out.append(f"merges\t{len(merges)}\n")
    out += [f"{i}\t{m.hex()}\n" for i, m in enumerate(merges)]
    body = "".join(out).encode()
    # Same size, one token changed: the check must refuse it too.
    altered = bytearray(body)
    altered[len(altered) // 2] ^= 1
    if check(bytes(altered), blake3) is None:
        sys.exit("self-check: an altered rebuild passed the pin check; nothing was written")
    why = check(body, blake3)
    if why is not None:
        sys.exit(f"{src}: the rebuilt sidecar is not the pinned one ({why}); nothing was written")
    open(dst, "wb").write(body)
    print(f"{dst}: {len(body)} bytes, blake3 {PIN_BLAKE3} = the QWEN25_05B_VOCAB pin")


if __name__ == "__main__":
    main()
