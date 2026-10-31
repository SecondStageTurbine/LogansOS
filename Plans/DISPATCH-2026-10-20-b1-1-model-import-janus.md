<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: B1-1 — prove the Arthur 0.5B `.neom` is reproducible from the raw upstream GGUF → Janus

**Type:** host-only verification. No kernel change and no QEMU gate. **Deliverable:** a pushed RETURN file.

**Why this is your task.** The PM's cloud container cannot reach `huggingface.co`, and your worktree on the PL's PC
can. B1 (`Plans/DECISION-2026-10-19-arthur-first-boot-model.md`, Amendment 1, item 5) requires two things:
- that the model Arthur ships is rebuilt from the pinned upstream file;
- that the rebuild comes out **byte-identical every time**.

The kernel already pins the model's BLAKE3 digest, `a32e4a1b…1ccafe`, in
`kernel/src/boot_model_logic.rs`, `QWEN25_05B`. That value came from the copy that was hand-staged on the OptiPlex.
This task proves the value is what upstream actually produces.

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/b1-1-model-import-janus      (git fetch origin && git checkout -B codex/b1-1-model-import-janus origin/main)
```

## Exact-bytes rule

Every digest and byte value in this task is an oracle. Get them **only** from raw files and raw API JSON, using
`curl`/`Invoke-WebRequest` and local hashing. **Never** take one from WebFetch or any summarizing or LLM-mediated read
(CLAUDE.md, "exact bytes come from raw source").

## Steps

1. **Download the pinned GGUF** (Qwen2.5-0.5B-Instruct, Q4_K_M, at the pinned upstream commit):
   ```powershell
   $rev = "9217f5db79a29953eb74d5343926648285ec7e67"
   $f   = "qwen2.5-0.5b-instruct-q4_k_m.gguf"
   New-Item -ItemType Directory -Force "E:\AI Automation\b1-1" | Out-Null
   Invoke-WebRequest "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/$rev/$f" -OutFile "E:\AI Automation\b1-1\$f"
   ```
   If that file name 404s, list the repo at that revision through the raw API (step 2) and use the Q4_K_M file it
   names. Record the name you used.
2. **Get the upstream SHA-256 from raw API JSON and match it locally.**
   ```powershell
   (Invoke-WebRequest "https://huggingface.co/api/models/Qwen/Qwen2.5-0.5B-Instruct-GGUF/tree/$rev").Content | Out-File "E:\AI Automation\b1-1\tree.json"
   Get-FileHash "E:\AI Automation\b1-1\$f" -Algorithm SHA256
   ```
   The file's `lfs.oid` (or `lfs.sha256`) entry in `tree.json` must equal the local SHA-256. Quote both in full.
3. **Import twice, in fresh output directories, with separate tool invocations.** Build `neomc` from the current
   `origin/main`.
   ```powershell
   cargo build -p neomc --release
   foreach ($n in 1,2) {
     $o = "E:\AI Automation\b1-1\run$n"; New-Item -ItemType Directory -Force $o | Out-Null
     .\target\release\neomc.exe import "E:\AI Automation\b1-1\$f" -o "$o\model-qwen2.5-0.5b.neom" --vocab-output "$o\model-qwen2.5-0.5b.vocab"
   }
   ```
4. **Compare byte-for-byte, then hash with BLAKE3.**
   ```powershell
   fc.exe /b "E:\AI Automation\b1-1\run1\model-qwen2.5-0.5b.neom"  "E:\AI Automation\b1-1\run2\model-qwen2.5-0.5b.neom"
   fc.exe /b "E:\AI Automation\b1-1\run1\model-qwen2.5-0.5b.vocab" "E:\AI Automation\b1-1\run2\model-qwen2.5-0.5b.vocab"
   cargo install b3sum --locked
   b3sum "E:\AI Automation\b1-1\run1\model-qwen2.5-0.5b.neom" "E:\AI Automation\b1-1\run1\model-qwen2.5-0.5b.vocab"
   (Get-Item "E:\AI Automation\b1-1\run1\model-qwen2.5-0.5b.neom").Length
   ```
   - **Expected `.neom`:** 485,489,152 bytes, BLAKE3
     `a32e4a1ba94a850724c15d840f3ae2c44902d9f1f34d8069bac5fcb37e1ccafe`.
   - The vocab sidecar has no pin yet; this run supplies one.
5. **The licence check.** Use `tree.json` (raw) to record whether the repo at that revision has `LICENSE` and/or
   `NOTICE` files. Download `LICENSE` raw and record its SHA-256.

## RETURN (the close condition)

Commit `Plans/RETURN-2026-10-20-b1-1-model-import-janus.md` to `codex/b1-1-model-import-janus` and **push it**. It
must contain:

- **The inputs:** the GGUF file name and size, the upstream `lfs` SHA-256 (quoted from `tree.json`), the local
  SHA-256, and whether the two match.
- **The tool:** the `neomc` source commit (`git rev-parse HEAD`).
- **Reproducibility:** both `fc /b` results.
- **The output:** the `.neom` size and BLAKE3, and the `.vocab` size and BLAKE3.
- **A verdict:**
  - **REPRODUCIBLE + PIN CONFIRMED** if the runs match and the `.neom` BLAKE3 equals the kernel pin;
  - **REPRODUCIBLE, PIN MISMATCH** if they match but the digest differs, with both digests quoted. That outcome
    blocks the 0.1 image until resolved;
  - **NOT REPRODUCIBLE** if the runs differ, with the first differing offset from `fc`.
- **Licence:** the `LICENSE`/`NOTICE` inventory and the `LICENSE` SHA-256.
- **Do not commit** the GGUF, `.neom` or `.vocab` files. They are large and are never stored in git.

**Disk reclaim:** after you push, delete `E:\AI Automation\b1-1` (about 1.5 GB). `cargo clean` is optional.

RAG (semantic code search) is available. Use it for "where is X done?" questions; grep wins for exact symbols. Check
its health with `Invoke-RestMethod http://127.0.0.1:8404/health`. If it is down, note that in your RETURN and use
grep/Read instead; the PL manages the RAG infrastructure, and it is never a blocker.
