<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #31): cross-family review of the Zenbook model-load freeze fix (PR #99) → Jarvis

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. The change moves a syscall's work from IF=0 to IF=1 and takes spin
locks while preemptible, which is core syscall/IF/memory code.

**Type:** review only. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `52148e22` (pinned; the author
may merge `main` into the branch while you review, and that merge only touches `ci.yml`).

## Setup

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/zenbook-model-load-freeze-review-jarvis
          git fetch origin && git checkout -B codex/zenbook-model-load-freeze-review-jarvis 52148e227f4162b8f344e2aa522c0faf5e9eb29e

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-23-zenbook-model-load-freeze-review-jarvis.md   # this file
git show 52148e22:Plans/NOTE-2026-10-23-zenbook-model-load-freeze-s1.md                     # the author's analysis
git show origin/claude/zenbook-ask-freeze:Plans/NOTE-2026-10-22-zenbook-ask-freeze.md       # the freeze diagnosis
git diff 11f72f94 52148e22                                                                   # the change under review
```

Also read CP-74 (new), CP-62, CP-64 and CP-59 in `docs/concurrency-pitfalls.md` at `52148e22`.

## What changed

- **Preload** (`kernel/src/boot_modules.rs`: `preload`, `preloaded`, `Preloaded`; feature `boot-model-preload`).
  Called once from `main.rs` before userland. For each pinned, present model it runs `load_model` inside
  `inference_stack::run_checked` (256 KiB, guard page, IF=1) and stores `Arc<Model>`. A model whose pinned
  vocab fails is skipped.
- **Syscall** (`kernel/src/arch/x86_64/syscall.rs`, `sys_model_load_impl`). A preloaded path returns
  `install_shared_model_handle(proc, Arc)`; anything else falls back to `load_model`.
- **Product image.** `boot-model-preload` is in `xtask iso --with-model`, `product-elf-check` and the CI clippy
  arm `product-real-lm`.
- **WM** (`userland/neo-init/src/desktop_session.rs`, `poll_control`). Arthur's `'M'` reply is polled while
  Home's pointer keeps ticking (with `desktop-pointer`; otherwise the blocking receive, as before).
- **`Cargo.toml`:** `[profile.dev.package.blake3] opt-level = 3`.
- **New scenarios:** `boot_model_load_stack`, `boot_model_load_ticks` + `_control`,
  `arthur_boot_module_preload`, `arthur_boot_module_preload_vocab_damaged`.

## Checklist

Answer each item PASS or FAIL with the enforcing `file:line`. For a FAIL, give a concrete input or
interleaving that produces the wrong result.

1. **Locks taken at IF=1.** The build takes `ARENA`, `DEDUP` and `REGISTRY` (`SwitchMutex`: plain spin, no
   interrupt masking) while preemptible.
   - 1a. Check that no thread or ISR can spin on any of these locks at IF=0 on CPU 0 while `preload` holds it
     (kernel threads, ISRs, `boot-neofs`, the USB port service, AP paths).
   - 1b. Check that every other lock the build takes is safe to hold with interrupts on.
2. **Preemption on the boot thread.** `preload` runs on the boot thread with the scheduler live.
   - 2a. Check that per-CPU state, `KERNEL_PID` attribution and the inference stack's `Held` owner (tid/pid)
     stay correct if the boot thread is preempted mid-load.
   - 2b. Check whether another thread can take the inference stack's `BUSY` during the load, and what it sees.
3. **One shared model.** Every Arthur gets the same `Arc<Model>`.
   - 3a. Check that nothing in `Model` is per-process or mutated per caller (KV state, dedup ownership,
     residency accounting, `release` on handle drop).
   - 3b. Check what happens when an Arthur exits normally, and when it is force-killed holding the handle.
   - 3c. The dedup owner is now `KERNEL_PID`, not Arthur's pid. Check that no accounting or teardown depends
     on the owner.
4. **Vocab ordering (B1c F8).**
   - 4a. Check that the `skipped=vocab` decision matches Arthur's `compose` in every state: vocab absent,
     vocab damaged, vocab fine with the model damaged, vocab fine with the model absent.
   - 4b. Check whether a vocab verdict cached at boot (`VERIFY`) can differ from what Arthur reads later.
5. **Error values.**
   - 5a. A damaged model at preload stores `Err(EBADMSG)`, and every later `sys_model_load` returns it (on
     `main` a second call returned `ENOENT`). Check every caller handles that.
   - 5b. Check an absent model still returns `ENOENT` (no preload entry; the fallback finds no module).
6. **Stack measurement.** `boot_model_load_stack` reports BLAKE3 at 485 MB as 39,536 B (opt 0) and 29,808 B
   (opt 3), against 32,768 B thread stacks.
   - 6a. Check the measurement method is sound (what it measures, from where).
   - 6b. With the feature off the in-syscall path is still exposed. Check that this is exactly the previous
     behaviour (the kill-switch), and say whether that is acceptable.
7. **Discriminator.**
   - 7a. Check that `boot_model_load_ticks` cannot pass with the load at IF=0.
   - 7b. Check the park placement (between the hash and the build) is representative of the real load.
   - 7c. Check that the control's 0 ticks / 0 co-runner runs is specific to IF=0.
8. **`poll_control`.**
   - 8a. Check it cannot drop or reorder a byte of Arthur's reply.
   - 8b. Check it cannot starve Arthur on CPU 0 (the WM's other polls spin the same way; CP-59).
   - 8c. Check whether any path relied on the old blocking receive.
9. **Kill-switch.** Check that with `boot-model-preload` off, `main.rs`, `syscall.rs` and `boot_modules.rs`
   behave exactly as on `main` (`11f72f94`).

A FAIL is reported only after you have re-checked it yourself. For each PASS, say what you checked.

## RAG

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?" questions
(faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='SwitchMutex ARENA DEDUP REGISTRY taken from a syscall or kernel thread'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition

- Commit `Plans/RETURN-2026-10-23-zenbook-model-load-freeze-review-jarvis.md` on your branch and **push it**.
  It contains: the verdict (MERGE or BOUNCE), each FAIL with severity, `file:line` and input, and what you
  checked for every item.
- Paste the pushed commit SHA to the PL.
- **Disk reclaim:** review lane. If you built anything, `cargo clean` in your worktree; never delete it.
- **Never write the Zenbook's disk.**
