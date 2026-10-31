<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Codex cross-check of the Arthur first-boot model THINK → Chad

**Type:** design review only; no code. **Deliverable:** one pushed RETURN.

**Why Chad:** the THINK's author is Claude-family. Its central claims are judgement calls, and a same-family reader
would share the author's priors on them:
- "the kernel can't read the USB, so ship the model as a boot module";
- "building from a boot module and then reclaiming it costs no more RAM than today".

A Codex-family read is the independent check (CLAUDE.md §Cross-model diversity).

## What is under review

`docs/THINK-2026-10-19-arthur-first-boot-model.md` on branch `claude/arthur-first-boot-model-think`, cut from `main`
@ `19d476a4`. The PM names the head SHA when relaying.

## Your job: refute it

1. **The USB claim.** Is it true that the kernel cannot read a USB stick after boot? The THINK says the block drivers
   are AHCI, NVMe and virtio only (`kernel/src/drivers/block/`), and that no USB mass-storage driver exists. If any
   path reads the boot USB (a Limine-provided buffer, an ACPI/firmware service, or something else), the THINK's
   main recommendation changes.
2. **Option A1, build the model from a Limine boot module and then reclaim the module.**
   - Is the load-time peak really no worse than today's? Today the file buffer is in heap and the weights are copied
     into the AI arena (`kernel/src/ai/model.rs:123-150`).
   - May the kernel free Limine's module region? Check the memory-map type and the raw Limine spec.
   - Is any live reference left into the reclaimed frames?
   - Is deferring A2 (in-place adoption, a change to the ARENA governance object) the right call?
3. **The tier rule (Q2).**
   - Is heap(T) + R ≤ U a sound rule?
   - Is R = 1024 MiB defensible? Name a machine profile where it is wrong.
   - The heap is sized before NeoFS mounts. Does the THINK's ordering (size by U, then pick the largest present tier
     at or below that size) have a hole?
4. **Integrity.**
   - Is the pinned `.neom` blake3 reproducible from upstream? That needs a deterministic `neomc import`. Check
     `tools/neomc` for nondeterminism such as map iteration order, timestamps or padding.
   - Is hashing in the load pass (the OT-73 fix) enough on its own, without Limine's `#hash` suffix?
5. **Licences.** Is anything missing from Q3? Check:
   - Apache-2.0 §4(d) and upstream `NOTICE` files;
   - the Qwen License's attribution and use-policy terms for 3B;
   - whether `/licenses/` in the image is the right place.
6. **The failure bank.** For the B1 OptiPlex run, which likely failure is missing? Check that each listed mode has
   a discriminator that is captured *before* the run.
7. **The PL decisions.** Is any decision in "For the PL" mis-framed, or missing? Is any recommendation not supported
   by the evidence cited?

A finding counts only if it survives your own re-verification. Put uncertain items in the RETURN as notes. A "no
findings" verdict must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\chad    — yours alone, never another coder's folder, never the repo root
BRANCH:   codex/arthur-first-boot-model-crosscheck-chad
          (git fetch origin && git checkout -B codex/arthur-first-boot-model-crosscheck-chad origin/claude/arthur-first-boot-model-think)
```

```
git fetch origin
git show origin/claude/arthur-first-boot-model-think:Plans/DISPATCH-2026-10-19-arthur-first-boot-model-codex-crosscheck.md
git show origin/claude/arthur-first-boot-model-think:docs/THINK-2026-10-19-arthur-first-boot-model.md
```

Exact bytes, such as Limine's `CONFIG.md`, xorriso's man page or the upstream model licence and `NOTICE` files, come
from raw sources (`curl`/`git`), never a summarizing fetch.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?" questions
(faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='model load sys_model_load model object bytes ownership'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-19-arthur-first-boot-model-crosscheck-chad.md` to your branch and **push it**. It must
contain:
- a verdict per item;
- each finding with file:line and its reasoning;
- for each item, what you checked.

Paste the pushed commit SHA in your reply to the PL.

**Disk reclaim:** this is a review lane. If you built anything, run `cargo clean` in your worktree; never delete the
worktree.
