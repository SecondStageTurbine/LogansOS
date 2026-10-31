<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Codex cross-check of B1b (the 0.5B loaded from a pinned boot module) → Chad

**Type:** review only. **Deliverable:** one pushed RETURN.

**Why Chad:** you found C1–C4, and B1b is the model half of the answer: C1 (the load-time peak), C4 (the
failure bank), and the integrity items of your item 4. The author and the first AAG lens are Claude-family.
If you are busy, Jarvis or Janus takes it unchanged. If you are also doing B1a
(`Plans/DISPATCH-2026-10-19-b1a-memory-core-codex-crosscheck.md`), do that first: B1b stacks on it.

## What is under review

The branch is `claude/b1b-arthur-boot-module-model`, stacked on `claude/b1-arthur-boot-module` (B1a, #70).
The PM names the head SHA when relaying. Review only B1b's own commits:

```
git diff origin/claude/b1-arthur-boot-module...origin/claude/b1b-arthur-boot-module-model
```

Start from `docs/B1-boot-module-model.md`, which holds the run plan, the minimum-RAM terms, F1–F7 and
the trap-mining.

What B1b adds:
- **Pins** (`kernel/src/boot_model_logic.rs`, pure, host-tested):
  - `/boot/models/<file>` names a module `/models/<file>`.
  - A product kernel pins only the real 0.5B (`a32e4a1b…1ccafe`).
  - A test kernel also accepts the deterministic neomc fixture. A KAT regenerates the fixture and checks
    its BLAKE3.
- **The load** (`boot_modules::load_model`, reached from `sys_model_load`):
  1. an unpinned path is refused before the take;
  2. the module is taken once;
  3. every byte is hashed and judged;
  4. the model is built only on a match;
  5. the module is reclaimed whatever the verdict (B1a zone).

  Damaged returns `EBADMSG` (-74); absent returns `ENOENT`.
- **`/boot`:** a read-only VFS mount serving *ordinary* modules (the vocab). Consumable modules are never
  served.
- **Heap:** under `model-boot-module` the 0.5B tier drops its staging buffer, 768 → 305 MiB. A KAT proves
  the policy is unchanged when the feature is off.
- **Arthur** (`arthur-boot-module`):
  - the registry's 0.5B entry is the module path;
  - the vocab is read from `/boot/vocab`;
  - a failed load is a named state with one sentence (`model_state_logic.rs`), and Arthur degrades
    instead of exiting.
- **Scenarios:** `arthur_boot_module_answers`, `_damaged` (one byte flipped), `_absent` (vocab only).
  None has a NeoFS disk.
- **Licences:** `assets/licenses/Apache-2.0.txt` (raw from apache.org) under `/licenses/`, the `NOTICE`
  MODEL WEIGHTS section, Bible Article 26, and ADR-063 Amendment 1.

## Your job: refute it

1. **Integrity.**
   - Find a path where model bytes are used before every byte was hashed and matched.
   - Find a path where bytes change between the hash and the build.
   - Find a path where a product kernel accepts the fixture digest.
   - Find a path where an unpinned or attacker-named module is consumed.
   - Is skipping Limine's `#blake2b` suffix the right call, given that its mismatch path panics the
     bootloader?
2. **Lifetime.** Does anything keep a pointer into the module bytes after `reclaim`? Check:
   - `Model::from_neom_bytes`;
   - the dedup and `content_hash` path;
   - the arena;
   - `FileProvenance::boot_module`.

   Can `/boot` ever serve the model?
3. **C1, the peak.** Is 305 MiB enough for the 0.5B load and generate on the kernel heap? Look at:
   - the KV cache (`vec!` in `forward_logic.rs`, sized by `prompt + max_new`);
   - logits;
   - dequant scratch.

   Do the minimum-RAM terms in the doc match the code? What would make the OptiPlex peak exceed 1.5 GiB?
4. **The ABI.** Do `EBADMSG` in `kernel/src/errno.rs`, in libneo, and in Arthur's `model_state_logic.rs`
   agree? Does the `/boot/models` branch of `sys_model_load` skip a check the NeoFS branch performs?
5. **The kill-switch.**
   - With `model-boot-module` off, is every kernel path unchanged?
   - With `arthur-boot-module` off, is Arthur unchanged?
   - Is `arthur_real_brain_boot` still the NeoFS path?
6. **C4, the bank.**
   - Does each of F1–F7 have a discriminator the run actually captures?
   - Is F2 really timed externally?
   - Is F3 split correctly?
   - What would the OptiPlex run hit that QEMU cannot show?
7. **The product question.** The real-LM `sys_generate` is compiled only under the test feature, so B1b
   ships the bare-metal bundle, not a product `iso --with-model`. Is that reading of the code right?

A finding bounces the branch only if it survives your own re-verification. "No findings" must state what
you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\chad    — yours alone, never another coder's folder, never the repo root
BRANCH:   codex/b1b-crosscheck-chad
          (git fetch origin && git checkout -B codex/b1b-crosscheck-chad origin/claude/b1b-arthur-boot-module-model)
```

```
git fetch origin
git show origin/claude/b1b-arthur-boot-module-model:Plans/DISPATCH-2026-10-20-b1b-boot-module-model-codex-crosscheck.md
git show origin/claude/b1b-arthur-boot-module-model:docs/B1-boot-module-model.md
git diff origin/claude/b1-arthur-boot-module...origin/claude/b1b-arthur-boot-module-model
```

Optional runs:
- `cargo test -p module-reclaim-logic-tests -p heap-policy-logic-tests -p arthur-logic-tests -p xtask`
- `cargo xtask neo-test arthur_boot_module_answers arthur_boot_module_damaged arthur_boot_module_absent`

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?" questions
(faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='sys_model_load boot module pinned digest reclaim'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-20-b1b-boot-module-model-codex-crosscheck-chad.md` to your branch and **push
it**. It must contain:
- a verdict per item;
- each finding with its severity, file:line, and a repro or the reasoning;
- for each item, what you checked.

Paste the pushed commit SHA in your reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree afterwards; never
delete the worktree.
