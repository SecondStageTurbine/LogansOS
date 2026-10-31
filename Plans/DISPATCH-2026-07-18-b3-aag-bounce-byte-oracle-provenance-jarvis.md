<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-16 B3 AAG bounce — anchor the brain's oracle + close the malformed-input guard → Jarvis

**Date:** 2026-07-18
**From:** PM (Claude)
**To:** Jarvis (Codex CLI; branch `codex/ot16-b3-qwen35-deltanet-bringup-jarvis`)
**Status:** BOUNCE — B3's blockers are CLEARED and the gate is GREEN, but the PM adversarial-audit gate (unblock-condition 5) surfaced two CONFIRMED ABI/byte-oracle findings + one completeness gap. This is NOT a code-safety bounce; it's the byte-oracle rule applied to the single most load-bearing correctness claim in the project. The fixes are cheap and specific.

## Where B3 stands (so this reads as "almost there," not "start over")

Your re-gate is verified. I independently confirmed on exact head `debc038c5a9b56f12e3b2530b6b6d14434409c90`:
- **Push CI GREEN** (run `29657914271`, event=push, on `debc038`).
- **workflow_dispatch QEMU gate GREEN** (run `29657922861`, event=workflow_dispatch, all 4 shards, ~53 min under the new 240-min ceiling, on `debc038`).
- **ff-safe** — `origin/main` (6a0c4bd4) is a direct ancestor; merges `--ff-only`, no cherry-pick.
- **My independent stat matches your Step-4 self-stat**: 32 files, 6513(+)/639(−). No phantom deletion.
- The two defects that parked B3 on 2026-07-12 were **OT-59** (swapgs/TLB-shootdown deadlock + the Arthur/NeoFS post-spawn virtio-blk hang) — **both fixed and merged** (`a6ed45e` → main). Your unblock conditions **1–4 are MET.**

**The AAG (unblock-condition 5) ran 5 refuting lenses. Three came back clean:**
- **Concurrency** — CLEAN. `InventoryDecodeState` is stack-local per-call; GDN recurrent/conv + KV state are owned by-value `Vec<f32>`; shared `Arc<Model>` is read-only immutable weights; no lock held across the decode loop.
- **Interrupt/EOI + I-14** — CLEAN. Zero governance-lock acquisition in the GDN/forward/decode path; heap change is a compile-time size-const (`HEAP_SIZE_REAL_BRAIN_QWEN35 = 1 GiB`) + pure selector param (kill-switch preserved bit-for-bit); no ISR/EOI/APIC code; timer access is read-only.
- **Memory-safety** — CLEAN. Zero `unsafe` added in any `.rs`; every file-derived length is validated (`checked_mul` + shape reconciliation) before it bounds a slice; the new Q5_K raw-byte readers are length-guarded and provably in-bounds (a malformed `.neom` returns `EINVAL`, never a panic); no DMA/uninit surface.

## The blocking findings (unblock-condition 5, must be clean)

### FINDING 1 (SEVERE, must-fix) — the 32-token oracle is self-referential; anchor it to an independent llama.cpp golden
`kernel/src/arch/x86_64/syscall.rs:74` defines `QWEN35_HELLO_ORACLE: [u32;32] = [11,353,1044,…]` with **no provenance comment**, and the merge-gating scenario asserts the kernel forward reproduces **that first-party array** (`syscall.rs:2350`). The array appears only in first-party files (the kernel oracle, `userland/neo-arthur/src/main.rs:297`, `tools/xtask/src/main.rs:8516`, the RETURN doc). The only genuinely-independent parity harness — `real_model_stage_a_teacher_forced_parity_when_available` (`tools/ai-ops-tests/src/lib.rs:6378`) — **does not anchor this**: its reference is supplied at runtime via `NEO_REAL_LLAMA_TOPK` (never committed), it compares **per-position top-k logprobs with a near-tie tolerance** (not the exact greedy 32-token sequence), and it `read_real_neom_or_skip`s in CI (`NEO_REAL_NEOM` unset ⇒ skip). So the llama.cpp parity for these exact 32 tokens lives **only in RETURN prose** — no committed golden, no CI-runnable exact check.

This is exactly the byte-oracle rule (CLAUDE.md, ADR-028): *"a writer and reader built by the same team from the same spec agree even when both implement it wrong… a KAT that equals your own output proves nothing."* If the shared host+kernel forward were subtly wrong in a way llama.cpp is not, the oracle would still pass green. And it matters **now**: the upcoming bare-metal Arthur run judges correctness against this same array, so it would NOT independently catch a host-lineage bug. Anchor the brain's oracle to a true reference **before** we bet the headline run on it.

**Fix (values are likely already correct — this is provenance + an auditable, CI-runnable anchor, per the exact-bytes-from-raw-source rule):**
1. Re-run **llama.cpp** greedy decode on the pinned GGUF (SHA `f5b14da98939b60bbe1019a964eba656407e1e0b64f1fe3003ff6d650e93bfec`) for prompt "Hello", **capture the raw stdout directly** (NOT via WebFetch / any LLM-summarizing read — that mangles exact tokens and poisons the oracle).
2. Commit that raw 32-token output as a **small in-tree golden fixture** (32 integers — tiny, no 725 MiB artifact needed) with a **provenance comment**: the GGUF SHA + the llama.cpp commit/version + the exact command. Model the provenance on the Qwen2.5 precedent already in-tree (`REAL_QWEN_LLAMA_CPP_HELLO_TOP1: u32 = 271`, `ai-ops-tests:13769`).
3. Make `QWEN35_HELLO_ORACLE` (and the userland/xtask duplicates) **derive from / assert-equal to that committed golden**, and add a **host KAT that runs in CI** (not env-gated) asserting the committed golden == the constant the kernel gates on. That gives an auditable, gate-enforced independent anchor even while the full model-present exact-parity still needs the runner artifact.

### FINDING 2 (MODERATE, must-fix, one line) — kernel reader accepts `rope_dimension_count = 0`
ADR-072's v4 amendment mandates `rope_dimension_count` be **non-zero**, even, and ≤ derived attention `head_dim`. The tool validator enforces it (`tools/neomc/src/validator.rs:62` rejects `0`), but the **kernel** load-time boundary does not: `kernel/src/ai/model_logic.rs:505-506` guards only `rope_dimension_count > head_dim || !is_multiple_of(2)` — `0 > head_dim` is false and `0.is_multiple_of(2)` is true, so `0` flows through to `ModelConfig` and runs degenerate RoPE (silently wrong output, no error). This inverts the "tool validator mirrors the kernel's checks" principle (ADR-066 F4 — here the tool is *stricter* than the kernel, which is backwards) and violates the ADR invariant that the kernel rejects malformed input before any forward executes.

**Fix:** add `|| rope_dimension_count == 0` to the `model_logic.rs:497` `InvalidConfig` guard. Add a load-validation unit/host test with a crafted `rope_dimension_count=0` v4 header asserting the kernel path now returns `InvalidConfig` (mirror the tool-validator test).

### FINDING 3 (COVERAGE/EVIDENCE, must-address) — the real-brain scenario silently skips in CI; the local proof was on `deaeda6`, not the head
`arthur_real_brain_boot_qwen35` short-circuits to `Ok(())` in every CI shard when `NEO_STAGE_B_NEOM`/`NEO_REAL_NEOM` is unset (`tools/xtask/src/main.rs:3510`), and **`ci.yml` sets neither** — so the green 4-shard gate never boots the brain. A regression to the real-brain path would NOT redden CI. The only real-brain proof ran on **pre-rebase head `deaeda6`** (your RETURN, "The local model-present QEMU proof completed successfully on code-equivalent pre-rebase head `deaeda6`") — not on `debc038`. `deaeda6` is not fetchable from origin, so the PM cannot even verify code-identity to the head being merged.

What IS proven-in-CI (the genuine anchors — keep these): the **kill-switch** `ot16_walker_forward_is_byte_identical_to_hardcoded_loop` (real, armed, with a reversed-layer-order negative control) and the synthetic op-KATs (Q5_K dequant vs llama.cpp reimpl, attention-gate, partial-RoPE, `forward_mixed_quant`).

**Fix — pick the strongest you can land, in order of preference:**
1. **Best:** provision the `.neom` on the runner + set `NEO_STAGE_B_NEOM` so `arthur_real_brain_boot_qwen35` **actually runs in the gate** (this closes the silent-skip AND proves the brain on the real head in one move). Coordinate with the PL on placing the artifact on the runner box.
2. **Minimum:** re-run the model-present QEMU proof **on the actual head `debc038` (or the re-spun head)**, archive the serial log under `Plans/evidence/` with the head SHA, AND make the CI skip **loud + tracked** (emit a distinct `SKIP` marker the PM can grep, so a green shard is never mistaken for real-brain coverage). Either way, the silent-skip-masks-regression property must be removed or made impossible to misread.

## Also fold in (non-blocking, do them in the same re-spin)
- **RETURN doc is stale.** `Plans/RETURN-2026-07-12-…-jarvis.md` still reads "BLOCKED RETURN / parked," cites the old red gate and `deaeda6`. Rewrite it as the CLOSE return: the now-green gate URLs on the actual head, the OT-59 blockers cleared, the AAG outcome, and the independent-oracle anchor.
- **Cosmetic doc nit (Lens 4).** A line in the RETURN doc claims "new unsafe is limited to the AVX2 target-feature entry points" — that describes a **rejected** alternative; the shipped code took the no-unsafe scalar-FMA path (`recurrent_state_dot` reproduces the FMA reduction in safe scalar; `fused_mul_add_f32` widens to f64). Reword to match the shipped implementation.

## Close-gate + merge
- Run `scripts/worker-close-gate.sh` (rebase-fresh onto current `origin/main` — main may have advanced; the full host-check set in CI order; SPDX; Step-4 self-stat). B3 is **kernel-touching** ⇒ the pre-merge `workflow_dispatch` QEMU gate is mandatory: paste the **green push-CI URL + the green 4-shard workflow_dispatch URL** on the exact re-spun head. "Green without a CI URL is not done."
- RETURN must state, per finding, what changed and the evidence: Finding 1 → the committed golden + its provenance + the new CI-runnable KAT (paste the token IDs and where they came from); Finding 2 → the guard + the new negative test; Finding 3 → which option you landed + the archived-evidence path or the now-running-in-CI proof.
- **Disk reclaim (after PM confirms merge):** once the PM has merged this branch to `main` and confirmed it on `origin/main`, delete this build's working tree to reclaim its `target/` (`git worktree remove <path>` / delete the clone). Never delete an unpushed/unmerged tree — verify the work is on `origin/main` first. `cargo clean` is the interim if you want to keep the checkout.

## Relay — what to tell Jarvis
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-18-b3-aag-bounce-byte-oracle-provenance-jarvis.md` (this file). PM artifacts live on `origin/main`; your tree is on a feature branch, so a local file-open won't find it — use the `git show` command. Also re-read your own `Plans/RETURN-2026-07-12-…-jarvis.md` (to rewrite it) and ADR-072/ADR-066.
- **One-line:** B3's gate is GREEN and OT-59 cleared it — the AAG bounces on **three specific items, all cheap**: (1) **anchor the 32-token oracle to a committed, provenance-commented, CI-runnable llama.cpp golden** (today it asserts against your own output — the byte-oracle rule; capture llama.cpp's raw stdout on GGUF `f5b14da9…`, don't fetch/summarize it); (2) **one-line guard** — reject `rope_dimension_count == 0` in `model_logic.rs:497` so the kernel matches the tool validator + ADR-072; (3) **the real-brain scenario silently skips in CI and your only proof was on `deaeda6`** — either provision the `.neom` on the runner so it runs in the gate, or re-prove on the actual head + archive the serial log + make the skip loud. Then rewrite the stale BLOCKED RETURN as a CLOSE, re-gate green (push-CI + 4-shard QEMU URLs on the exact head), and RETURN.
- **RAG:** RAG (semantic code search) is available — use it for conceptual "where is X done?" (faster/more semantic than grep; grep still wins for exact symbols). Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`. If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads (never a blocker). Optional self-serve start (PowerShell): `powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"` then `cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`. Query e.g.: `$body=@{query='llama.cpp parity oracle qwen35 greedy tokens'; top_k=5}|ConvertTo-Json; Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.
