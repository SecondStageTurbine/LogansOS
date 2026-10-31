# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## LogansOS RAG — Semantic Code Search

A local semantic-search index of this repo is available at `E:\AI Automation\RAG\AltimitNeo`. Use it to find code by meaning instead of grep when exploring unfamiliar areas, locating related symbols, or answering "where is X done?" questions.

### Endpoints

- **API base:** `http://127.0.0.1:8404`
- **Docs:** `http://127.0.0.1:8404/docs`
- **Health:** `http://127.0.0.1:8404/health`
- **Stats:** `http://127.0.0.1:8404/stats`
- **Qdrant (raw):** `http://127.0.0.1:6373`  — collection `altimitneo-code`

### Prerequisites (must be running before queries)

```powershell
# 0. Dedicated Ollama on T4 (shared by all RAGs — start once per machine)
Start-Process powershell -ArgumentList '-NoProfile','-File','"E:\AI Automation\RAG\start-rag-ollama.ps1"' -WindowStyle Hidden
# 1. Qdrant container (already auto-restarts via Docker, but if not):
cd "E:\AI Automation\RAG\AltimitNeo"
.\scripts\start-qdrant.ps1
# 2. API server (detached — survives terminal close):
.\start-rag.ps1
```

Verify with:
```powershell
Invoke-RestMethod http://127.0.0.1:8404/health
Invoke-RestMethod http://127.0.0.1:8404/stats
```

A healthy response looks like `{"status":"healthy","ollama":true,"qdrant":true,...}`. If `ollama` is false, the embedding model is not loaded — the RAG can't embed queries even though Qdrant has the index. Pull the model:
```powershell
Invoke-WebRequest -Uri "http://127.0.0.1:11435/api/pull" -Method POST `
  -Body (@{name='nomic-embed-text'} | ConvertTo-Json) -ContentType "application/json"
```

### Querying (use this from Codex)

**Truncated snippets (default — fast overview):**
```powershell
$body = @{query='per-CPU TSS allocation'; top_k=5} | ConvertTo-Json
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST `
  -Body $body -ContentType "application/json"
```

**Full chunk text (when you need the complete code block):**
```powershell
$body = @{query='IPC rendezvous handoff invariants'; top_k=3} | ConvertTo-Json
Invoke-RestMethod -Uri http://127.0.0.1:8404/search/full -Method POST `
  -Body $body -ContentType "application/json"
```

**Filter by language or path prefix:**
```powershell
# Restrict to Rust source
$body = @{query='capability derivation walk'; top_k=5; language_filter='rs'} | ConvertTo-Json
Invoke-RestMethod -Uri http://127.0.0.1:8404/search/full -Method POST `
  -Body $body -ContentType "application/json"

# Restrict to markdown (ADRs and Bible) under docs/
$body = @{query='Phase 2 SMP retrofit'; top_k=5; language_filter='markdown'; path_prefix='docs'} | ConvertTo-Json
Invoke-RestMethod -Uri http://127.0.0.1:8404/search/full -Method POST `
  -Body $body -ContentType "application/json"

# Restrict to kernel source
$body = @{query='page table walk'; top_k=5; language_filter='rs'; path_prefix='kernel/src'} | ConvertTo-Json
Invoke-RestMethod -Uri http://127.0.0.1:8404/search/full -Method POST `
  -Body $body -ContentType "application/json"
```

Valid `language_filter` values for this repo: `rs`, `markdown`, `toml`, `ld`, `yaml`, `json`, `conf`. Confirm the live set against `/stats` — the `by_language` field lists exactly what's currently indexed.

### When to use the RAG

- **Use it for:** "where is the IPC rendezvous done?", "find the capability revoke walk", "show me where TSS.rsp0 is mutated", "what's the boot order in `mm::init`?" — any semantic/concept question where you don't already know the file.
- **Skip it for:** exact string lookups (use `Select-String` / grep), exact symbol-name searches across the codebase (grep is faster and more precise), or when you already know the file path (use Read directly).

The RAG is a *discovery* tool. For OBSERVE-pass work where you need to read a specific file in detail to understand its structure, direct file reads are still the right approach; RAG's top-N results don't replace reading a 200+ line file end-to-end.

### Troubleshooting

- **Connection refused on :8404** → API server not running; run `.\start-rag.ps1` from the RAG root.
- **`{"status":"degraded","ollama":false}`** → Ollama on `:11435` is up but the embedding model is not loaded. Pull `nomic-embed-text` (see Prerequisites).
- **Empty results / "collection does not exist"** → run `python -m src.ingest --full -v` from the RAG root to build the index.
- **Index looks stale (`last_indexed` doesn't match recent commits)** → the watcher auto-reindexes on file save (2s debounce). To force a rebuild: `python -m src.ingest --full -v`.
- **`by_language` shows far fewer Rust chunks than expected** → reindex. The watcher can fall behind on bulk changes.
- **Full end-to-end diagnostic:** `cd "E:\AI Automation\RAG\AltimitNeo" && python diagnose_rag.py`
- **Embeddings hang:** the dedicated Ollama on `:11435` is pinned to the T4 (GPU2). If that GPU is lost (`nvidia-smi` reports it missing), the host needs a reboot — embeddings will hang indefinitely until then.

### What's indexed

The RAG indexes everything under `E:\AI Automation\AltimitNeo` that isn't in a skip list:

- **Rust source:** `kernel/src/**/*.rs`, `userland/**/*.rs`, `tools/xtask/src/*.rs`. Indexed as language `rs`.
- **Documentation:** `docs/**/*.md` (Bible, ADRs, ANSPECs, phase plans, research). Indexed as `markdown`.
- **Build config:** `Cargo.toml` files, `rust-toolchain.toml`. Indexed as `toml`.
- **Linker scripts:** `kernel/src/arch/x86_64/linker.ld` and friends. Indexed as `ld`.
- **CI / docker:** `.github/workflows/*.yml`, `docker-compose.yml`. Indexed as `yaml`.
- **JSON config:** target spec JSONs. Indexed as `json`.
- **Limine config:** `boot/limine.conf`. Indexed as `conf`.

Excluded: `.git`, `target`, `.cargo-home`, `__pycache__`, `.venv`, lock files, env files, images, archives, binaries, and other non-source artifacts. Test files and lock files have their own skip patterns.

The chunker uses tree-sitter where available; for Rust it falls back to generic chunking (paragraph-style splits at double-newlines), which is good enough for semantic search but doesn't perfectly align chunks with function boundaries the way the TypeScript chunker does.
