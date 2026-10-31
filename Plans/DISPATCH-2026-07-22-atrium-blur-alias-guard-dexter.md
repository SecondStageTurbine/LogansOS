<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Atrium — enforce `blur_backdrop` dst≠self alias guard (Slice-6 hardening carry) → Dexter

**Date:** 2026-07-22
**From:** PM (Claude)
**To:** Dexter (DeepSeek, OpenCode CLI — first cell)
**Status:** DISPATCHED — **kernel-touching build (small, self-contained).** `SharedPixelBuffer::blur_backdrop` creates an immutable `&[u8]` view of `self` and a mutable `&mut [u8]` view of `dst`, and its `unsafe` SAFETY comment **assumes** `dst` is a distinct buffer from `self` — but nothing enforces it. If a caller ever passes the same buffer as both, the two slices alias the same memory → **undefined behavior** (an in-place blur reading while writing). Latent today (no live caller aliases), but the guard belongs in the code, not in a comment. This is the **Slice-6 hardening carry** recorded in the OT-63 Slice-1 ledger row. Build to the spec below exactly.

## Scope — what to build (and what NOT to)
1. **Add the alias guard.** In `kernel/src/atrium/buffer.rs`, `SharedPixelBuffer::blur_backdrop` (the fn at ~L480), **before** the two `unsafe` slice creations (`src_backing.as_slice()` / `dst_backing.as_mut_slice()`, ~L511-512), reject the aliasing case:
   ```rust
   if Arc::ptr_eq(self, dst) {
       return Err(EINVAL); // in-place blur would alias &[u8]/&mut [u8] -> UB
   }
   ```
   Place it alongside the existing geometry check (the early-return `Err(EINVAL)` block at ~L488). Both `self: &Arc<Self>` and `dst: &Arc<SharedPixelBuffer>`, so `Arc::ptr_eq(self, dst)` compiles directly (import `alloc::sync::Arc` if not already in scope in that file).
2. **Justify sufficiency in your RETURN.** State *why* `Arc::ptr_eq` is the complete guard here: each `SharedPixelBuffer` owns its own `arena_backing` allocation (a distinct arena span per buffer), so two **distinct** `Arc`s cannot share a backing span — therefore Arc-identity is sufficient to rule out aliasing (you do NOT also need a backing-pointer-overlap check). If your reading of the code contradicts that (e.g. you find a path where two distinct buffers can share a backing), SAY SO — that would be a real finding, not a deviation.
3. **Do NOT change the live path.** The only live callers pass two distinct buffers; their behavior must stay **bit-for-bit identical**. The guard is a pure addition on a case no live caller hits. Do not refactor the blur, the damage-commit, or the geometry check. Smallest possible diff.

## The test (this is the objective proof — make it genuine)
Add a test (a kernel unit test `#[cfg(test)]` in `buffer.rs` if the type is constructible there, else extend the existing `atrium_glass_blur` QEMU scenario / harness — whichever the surrounding code already uses for `SharedPixelBuffer`; check first):
- **guard-fires:** construct one arena-backed `SharedPixelBuffer`, call `buf.blur_backdrop(&buf, radius, weights, changed)` (same `Arc` as self AND dst) → assert it returns `Err(EINVAL)` **and** that the buffer's bytes are unchanged (the guard returns *before* any write).
- **negative control (no regression):** two distinct arena-backed buffers → assert `Ok(())` and the output bytes are **identical to the pre-change result** (the live path is untouched). If an `atrium_glass_blur` oracle/expected-bytes fixture already exists, assert against it — do not hand-wave "it still works."

A guard with no guard-fires test, or a test that never actually aliases, is not done.

## Standing-rule discharge (state each in the RETURN)
- **Kill-switch / no-regression (Rule-1):** the guard is behavior-preserving for every live (distinct-buffer) call — bit-for-bit. The negative-control test IS the equality proof. (There is no feature flag to add; the "disabled ≡ prior" property is that distinct-buffer calls are unchanged.)
- **Byte-oracle:** N/A — no new on-wire/on-disk byte layout is defined. (Say so explicitly.)
- **Concurrency-pitfall consult (`docs/concurrency-pitfalls.md`):** N/A — `blur_backdrop` is a leaf operation taking no new lock and no cross-CPU/interrupt/atomic path; `Arc::ptr_eq` is a pointer compare. State "no pitfall class touched."
- **I-14 fault-containment:** N/A — CPU-only, no DMA, no governance lock. State so.
- **SPDX:** only if you add a new source file (first line `// SPDX-License-Identifier: MPL-2.0`). If you edit `buffer.rs` in place, it already has its header.

## Close-gate + merge (the ritual — run it fully, it is one command)
- Run `scripts/worker-close-gate.sh` — it rebases you fresh on current `main`, runs the exact CI host-check set via Docker (`cargo fmt --all --check`, workspace tests, workspace clippy `-D warnings`, the blocking kernel clippy `cd kernel && cargo clippy -Zjson-target-spec -- -D warnings`), checks SPDX, prints your own `git diff --stat` vs `main`, and prints the RETURN-evidence block. **Do not run it partially** — the failures it prevents all come from running only part of it.
- **This is kernel-touching**, so a green **4-shard `workflow_dispatch` QEMU run** URL is mandatory in your RETURN (all 4 shards + host + SPDX), in addition to the push-CI URL. **"Green" without a CI URL is not done** — paste the URLs.
- Inspect your own `git diff --stat` before declaring done (no phantom deletions — this should be a ~single-digit-line change plus a test).
- Branch `dexter/atrium-blur-alias-guard`; PM merges after gate + a small AAG. **Disk reclaim (after PM confirms merge):** once the branch is on `origin/main`, reclaim the build tree's `target/` (`cargo clean` keeps the checkout). Never delete a tree whose branch is unpushed/unmerged.

## Relay — what to tell Dexter
- **Read (all on `main`; your tree sits on a feature branch, NOT `main`, so a local file-open of a `Plans/` path finds nothing — use `git show`):**
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-07-22-atrium-blur-alias-guard-dexter.md
  ```
  The code is already in your working tree: `kernel/src/atrium/buffer.rs` (`blur_backdrop` ~L480, the two `unsafe` slice lines ~L511-512, the existing geometry `Err(EINVAL)` guard ~L488) and `kernel/src/atrium/blur.rs` (the blur logic, for the negative-control oracle). Work in `Workers\dexter`; branch `dexter/atrium-blur-alias-guard`.
- **One-line:** `SharedPixelBuffer::blur_backdrop` makes a `&[u8]` of `self` + a `&mut [u8]` of `dst` under an `unsafe` whose SAFETY only *assumes* `dst≠self` — **enforce it**: add `if Arc::ptr_eq(self, dst) { return Err(EINVAL); }` before the slice creations, plus a **guard-fires test** (same buffer as self+dst → `Err(EINVAL)`, bytes unchanged) and a **negative-control test** (two distinct buffers → `Ok`, output byte-identical to the current result). Smallest diff; do NOT touch the live path. In the RETURN: justify why `Arc::ptr_eq` is sufficient (each buffer owns its own arena backing → distinct Arcs can't alias), and discharge the standing rules (kill-switch = distinct-buffer path bit-for-bit unchanged; byte-oracle/concurrency/I-14 all N/A, stated). Run `scripts/worker-close-gate.sh` in full + paste the green push-CI **and** 4-shard `workflow_dispatch` QEMU URLs (kernel-touching).
- **RAG** (semantic code search — you run PowerShell): use it for "where is `SharedPixelBuffer` constructed / where is `blur_backdrop` called / the `atrium_glass_blur` scenario" lookups. Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` (expect `{"status":"healthy",...}`). If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with grep/Read (never a blocker). Optional self-serve start: `powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"` then `cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`. Query e.g.: `$body=@{query='SharedPixelBuffer blur_backdrop arena backing'; top_k=5}|ConvertTo-Json; Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.
