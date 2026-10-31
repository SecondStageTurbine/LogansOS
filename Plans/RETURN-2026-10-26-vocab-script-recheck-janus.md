<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #77): vocab rebuild digest re-check — Janus

**Reviewed SHA:** `11fbd161b9d76d74bb5c977751720e1d186f8482` (PR #139).
**Reviewer model / effort:** GPT-6.1-Sol / medium.
**Overall verdict:** PASS. Relay #73 item 7 is resolved. Review only; no product changes.
**RAG:** healthy (`ollama=true`, `qdrant=true`, 2026-10-26 19:13 UTC).

## Checklist

1. **Original missing-module probe — PASS.** Re-ran `python -S $env:TEMP/relay73_missing_blake3_probe.py`; its child also uses `-S` and the original token-1000 same-length mutation. Child exit 1, empty stdout, stderr: `error: the `blake3` Python module is required to check the output against its pin (install it: pip install blake3); nothing was written`. No `size matches` text and no output file. Mandatory import precedes parsing and writing (`scripts/rebuild-qwen25-vocab.py:80`, `scripts/rebuild-qwen25-vocab.py:117`).

2. **BLAKE3 present — PASS.** Using temporary CPython 3.12.14 with `blake3==1.0.11`, ran the actual CLI with `-I` on the same synthetic GGUF. Exit 1; it rejected the 7,990,862-byte altered body with BLAKE3 `0caecd35398d5e07aa07f9ee7285c8a6dfcf384d2b27198d25c80be6a2531eb8`, versus required `e89741214838ae7af6277122c1093e899138884ea3bea4f83ff5671e76c05265`; no destination created. Restoring only the token-1000 mutation and rerunning the CLI exited 0 and emitted 7,990,862 bytes with the required digest. The result is byte-for-byte equal to `target/b1-model/model-qwen2.5-0.5b.vocab`. The helper also accepts that real sidecar directly. Size/digest checks and write ordering: `scripts/rebuild-qwen25-vocab.py:93`, `scripts/rebuild-qwen25-vocab.py:142`, `scripts/rebuild-qwen25-vocab.py:145`.

3. **No digest bypass / unfinished output — PASS.** Read the entire script. There are no environment-variable branches or skip flags. `--self-test` still loads BLAKE3 and runs negative checks, produces no sidecar, and reports only the self-test result (`scripts/rebuild-qwen25-vocab.py:117`, `scripts/rebuild-qwen25-vocab.py:119`). Ordinary CLI success follows both required checks and the write (`scripts/rebuild-qwen25-vocab.py:140`, `scripts/rebuild-qwen25-vocab.py:142`, `scripts/rebuild-qwen25-vocab.py:145`). The only caught exception is ImportError, which exits nonzero (`scripts/rebuild-qwen25-vocab.py:85`). Missing-module, wrong-digest, and malformed-GGUF probes all left fresh destinations absent. Repeating the wrong-digest probe with an existing sentinel destination preserved its bytes and said `nothing was written`; it did not label the existing file verified. A write exception would propagate after the body has already passed the digest; there is no exception-to-success path. `--self-test` exited 0; direct short-body rejection also passed.

4. **Kernel pins match — PASS.** `scripts/rebuild-qwen25-vocab.py:26` and `scripts/rebuild-qwen25-vocab.py:27` specify 7,990,862 bytes and `e89741214838ae7af6277122c1093e899138884ea3bea4f83ff5671e76c05265`, matching the size documentation at `kernel/src/boot_model_logic.rs:62` and `QWEN25_05B_VOCAB` digest at `kernel/src/boot_model_logic.rs:66`. The kernel's `ModelPin` has no numeric length field; the live sidecar measurement independently confirms the documented size.

5. **SPDX — PASS.** `scripts/rebuild-qwen25-vocab.py:2` remains `# SPDX-License-Identifier: MPL-2.0`, immediately after the shebang at line 1.

## Reproduction and scope

- Original harness: `C:\Users\<user>\AppData\Local\Temp\relay73_missing_blake3_probe.py`.
- Expanded asserting harness: `C:\Users\<user>\AppData\Local\Temp\relay77_digest_recheck.py`, run with `& $env:TEMP/relay77-janus-venv/Scripts/python.exe -B $env:TEMP/relay77_digest_recheck.py`; exit 0, `ALL_ASSERTIONS_PASSED`.
- Artifacts: `C:\Users\<user>\AppData\Local\Temp\relay73-pin-probe-2ag9h_9n` (altered/restored synthetic GGUFs and verified output).
- The positive source was reconstructed from the available real pinned sidecar, as in #73; the upstream llama.cpp sdist source was not fetched. This is the requested narrow digest re-check, not a new upstream provenance review. No kernel/userland/tools edits or QEMU runs.
- Existing local graft configuration and generated cache were preserved in the stash named `Janus pre-existing graft configuration and cache before relay 77 RETURN`, outside this review commit.
