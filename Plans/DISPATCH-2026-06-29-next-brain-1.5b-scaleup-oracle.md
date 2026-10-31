<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Next-brain scale-up — run the 1.5B Qwen2.5 brain (the memory budget is the build) → Oracle

**Date:** 2026-06-29
**From:** PM (Claude)
**To:** Oracle (main-tier; just landed CB-3 identity + the SMP AP-ready barrier — the mm/SMP surface this build needs). New branch off `origin/main` HEAD: `codex/next-brain-1.5b-oracle`.
**Status:** DISPATCHED — **the PL-headline next capability step: a bigger brain for Arthur (0.5B → 1.5B).** F2 (`f25138b`) made `ModelConfig` data-driven from the `.neom` v2 header, so the forward math is already config-driven and "1.5B-runnable" on paper — **but nobody has run one, and it almost certainly does not fit memory.** **Non-conflicting** with Cerebro's WiFi/connectivity surface (this is `mm` + the forward/model path, not mt7601u/xHCI/DHCP). **Kernel-touching ⇒ 4-shard QEMU gate + AAG.**

## 0. The decision this build makes
Make the 1.5B Qwen2.5 brain actually load and emit correct tokens in-kernel. **The crux is memory, not math:** `HEAP_SIZE = 768 MiB` (`kernel/src/mm/heap.rs:31`) was sized for the **0.5B** (the 485 MB `.neom` that already strains it — the `byte_len` OOM fix `4279683` exists precisely because a 485 MB load on a 768 MiB heap doubles a growing Vec and OOMs). A 1.5B model is **~3× larger** (≈1.3–1.5 GB at the shipped quant) and **will not fit the current heap.** So the build is: provision a real 1.5B `.neom`, **give it a place to live**, and prove correctness against the parity oracle.

## 1. References
```
git fetch origin
git show origin/main:kernel/src/mm/heap.rs                         # HEAP_SIZE 768 MiB (line 31) — the budget to fix
git show origin/main:kernel/src/arch/x86_64/smp.rs                 # :31 note — "768 MiB Stage-B heap + MAX_ORDER 16" SMP/buddy interaction (mind this when widening)
git show origin/main:kernel/src/fs/mod.rs                          # Inode::byte_len() (4279683) — pre-size the single load allocation, no doubling
git show origin/main:docs/OPEN-THREADS.md                          # OT-28 (brain last-mile, F2 f25138b — config data-driven; 1.5B runnable) + OT-38 (Qwen2.5 1.5B/3B is the chosen scale-up; Gemma-4 parked)
git show origin/main:docs/ADR-064-neofs-large-files.md            # NeoFS to 2 GiB (the 1.5B + sidecar storage path)
git show origin/main:tools/neomc/                                  # neomc import: GGUF → real .neom v2 + vocab sidecar (the host provisioning tool)
# Stage-A parity harness (OT-30 methodology, e517a52) — the llama.cpp top-1 oracle to re-run at 1.5B
```

## 2. ISCs
- **ISC-1 — OBSERVE: measure the 1.5B footprint vs the budget (decide the memory approach before writing).** Import a real Qwen2.5-1.5B GGUF → `.neom` via `neomc` (host), read its resident size, and measure: does `with_capacity(byte_len)` single-shot load fit if `HEAP_SIZE` grows, or does the 1.5B need a **dedicated model arena** outside the general heap? Report the number; the answer picks the approach (don't guess). **State the chosen approach as the build's load-bearing decision.**
- **ISC-2 — The memory budget.** Implement the ISC-1 decision: either **bump `HEAP_SIZE`** to hold the 1.5B (mind the `smp.rs:31` MAX_ORDER-16 buddy interaction + the per-CPU AP-bringup the barrier guards — a larger heap changes the map-in cost) **or** a **dedicated `.neom` model arena** (a separate region the model loads into via the `byte_len`-presized single allocation, leaving the 768 MiB general heap intact). Whichever: the load path must **not** double-buffer (the `byte_len` pre-size is the discipline). Kill-switch: the new budget is config/const — the default build (0.5B) must be **bit-for-bit unchanged** when the 1.5B path is off.
- **ISC-3 — Run it + parity.** Boot the 1.5B `.neom` and prove correctness against the **Stage-A llama.cpp top-1 parity oracle** (OT-30 methodology — `first_divergence=None` over the token window). The config is data-driven (F2) so no forward-math change should be needed; if a 1.5B-specific assumption surfaces (head-dim/GQA-group/rope-theta), it is config, not a kernel constant (OT-16 frozen — values only). Capture the per-token latency (the honest 1.5B number).
- **ISC-4 — Provisioning record + bundle path.** Document the `neomc` 1.5B import recipe (no blob in git — staged from disk via `NEO_STAGE_B_NEOM`, the existing pattern) so the 1.5B ISO is reproducible, and confirm NeoFS serves it (ADR-064, files to 2 GiB).

## 3. Standing-rule sections
- **Kill-switch (Rule 1):** the 1.5B path is off-by-default (config/feature); default build ≡ the proven 0.5B, bit-for-bit (the memory-budget change is inert when the larger model isn't staged). Ship the equality check.
- **Invariant budget (Rule 2):** preserve the ledger; **no new invariant** (this is sizing + provisioning, not a new cross-cutting property). I-1 (the byte-oracle / result-invariance) is *upheld* by the Stage-A parity at 1.5B.
- **Byte-oracle:** the `.neom` v2 header + per-quant-type dequant already carry byte-oracles (F1/F2); re-assert they hold for the 1.5B's tensor set. The parity oracle (ISC-3) is the end-to-end check.
- **I-14:** N/A by construction (no driver/ISR/DMA — `mm` + forward only); no AI-governance lock added.
- **Blind-hardware note:** if a bare-metal 1.5B run is in scope later, it inherits the Stage-B/gold-standard discriminator ladder — but **this build's gate is QEMU + the Stage-A host parity**, not silicon (the 1.5B bare-metal run is a follow-on once it fits + parities).

## 4. Gate + close
- **Kernel-touching ⇒ mandatory 4-shard QEMU `NEO_SMP=4` workflow_dispatch + AAG** (the heap/arena change is core, always-compiled — non-waivable). Push-CI green (host parity harness + the kill-switch equality). **Green without the CI URLs is not done.**
- **Close-gate `scripts/worker-close-gate.sh`** (rebase onto `origin/main`, re-run, push ff-clean; host set; SPDX; self-stat).
- **RETURN** with the QEMU shard URLs + SHA, the 1.5B footprint number + the chosen memory approach, the Stage-A parity result (`first_divergence`) + per-token latency, and the kill-switch equality proof. **Disk reclaim** after merge.

---
## Relay — what to tell Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-29-next-brain-1.5b-scaleup-oracle.md` (+ `mm/heap.rs`, the `smp.rs:31` note, OT-28/OT-38, the Stage-A parity harness, `neomc`). **Branch:** `codex/next-brain-1.5b-oracle` off `origin/main` HEAD.
- **One-line:** **Make the 1.5B Qwen brain actually run — and the build is the memory budget, not the math.** F2 made the forward config-driven, but `HEAP_SIZE` is 768 MiB sized for the 0.5B (485 MB already strains it — that's why `byte_len` exists); a 1.5B is ~3× and won't fit. **OBSERVE first** (`neomc`-import a real 1.5B, measure its resident size), then **decide + implement: bump the heap OR give the `.neom` a dedicated model arena** (single `byte_len`-presized load, no double-buffer), then **prove correctness via the Stage-A llama.cpp top-1 parity oracle at 1.5B** (`first_divergence=None`) + capture the per-token latency. **Kill-switch:** 1.5B off ≡ the proven 0.5B bit-for-bit. **No connectivity, no forward-math change** (1.5B specifics are config — OT-16 frozen). **Gate:** 4-shard QEMU + AAG (the heap/arena is core ⇒ non-waivable) + the host parity. Close-gate; RETURN shard URLs + SHA + the footprint number + parity result + kill-switch equality.
