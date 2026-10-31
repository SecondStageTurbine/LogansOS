<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW BUNDLE (relays #141–#143) → Jarvis: app icons, WM client errors, C2.2 memory quotas

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. A bundle, so one model throughout (CLAUDE.md bundle rule). #143 is core
memory code, so Sol high is its own routing anyway.

All three merged after batch 14b (run `40817887458`, green at `67f023bc`). Review each squash on `main`:
`git show <sha> --stat` and `git show <sha>`. One pushed RETURN per relay, each a plain checklist (pass or fail, the
enforcing `file:line`, a concrete input for any fail): `Plans/RETURN-2026-10-29-<slug>-review-jarvis.md`.

## #141: #185 `40f60300`, the Atrium app icons (product-on). Slug `atrium-app-icons`

Design: `Plans/DISPATCH-2026-10-29-icons-build-s1.md` and `Plans/SYNTHESIS-2026-10-29-icons.md`.
1. **The rasteriser KAT** (`tools/icon-raster`). Does the tiny SVG rasterise to exactly the expected bytes? Are the
   BLAKE3 pins of the committed blobs checked, so a resvg bump can't change pixels silently?
2. **The kill-switch.** With `atrium-app-icons` off, the neo-init and neo-atrium `.text` are identical to before, and
   the chips draw their letters bit for bit.
3. **Chip compositing** (`userland/neo-atrium/src/{icons.rs,scene.rs}`). The 40 px icon is centred in the 56×56 chip.
   The focus glow, selected state, label and Reduce-Transparency opaque chip are unchanged. Is straight alpha
   composited correctly?
4. **The scenario.** `desktop_icons_boot` asserts the Arthur chip's pixels equal the pinned blob over the chip, and
   fails with the wrong blob. Its post-check reads its own capture (the `f90b7b7c` fix).
5. **The product mapping.** `atrium-app-icons` is in the product and image-A WM build, and
   `product_image_draws_the_app_icons` pins it.

## #142: #189 `46d6ba1e`, every WM client error refused, not fatal. Slug `wm-client-errors`

1. **The per-arm table** in the PR body. Each client-error arm of `arthur_session` replies with a refusal status and
   keeps the session running: Commit/CommitDamage, Configure, Destroy, Yield, the catch-all, and undecodable bytes. Is
   every arm in the table, and is any arm mislabelled WM-internal?
2. **Undecodable bytes.** A whole header gets a refusal; anything shorter is dropped and logged. Check libneo's
   `request_target` and its byte oracle.
3. **The discriminator.** Arthur's probe sends one refused request per arm. Does it fail on the old code (`neo-init
   exit code 25`)?
4. **The binary diff.** neo-init `.text` grows 2.1–2.9 KB, and product neo-arthur's functions are the same, only
   reordered.

## #143: #190 `130a7ef3`, OT-61 C2.2 memory quotas. Slug `ot61-c2.2`

Behind `mem-quota` (the `browser-runtime` lane, in no product image). Muse reviews the containment claim separately
(#138). The PR's checklist:
1. Is every frame a charged `Pmap` owns charged before it's allocated, and released exactly once (on unmap or `Drop`)?
   Check the PML4, the intermediate tables, `map_user`, `allocate_user_region` / `_guarded`, `unmap_user_range` and
   `map_borrowed_region`.
2. Can `charge` leave any level above its limit, or a level charged after a refusal, when two CPUs charge one chain?
   (`mem_account_logic.rs`)
3. Does a failed spawn (any error return in `spawn_user_body`, including `ParentGone`) release everything it charged?
4. Does any path release a charge while still holding a frame, or drop a charged `Pmap` under `CAP_GRAPH`? This
   includes the #183 overlap table in the PR body, and the charged-`Pmap` leg in `mem_quota`.
5. Does `SYS_MEM_INFO` match its byte oracle, and does `SYS_SPAWN_BUDGETED` refuse `budget_bytes == 0`?
6. With `mem-quota` off, is anything left: fields, calls or syscall numbers?
7. Does the `cap-graph-logic-tests` gate change (it now recognises `macro_rules!`) still catch a `lock_cap_graph()` call
   outside `spawn_user_body`?

```
MODEL: GPT-6.1-Sol. EFFORT: high. (A bundle: one model throughout, per the bundle rule.)
WORKTREE: E:\AI Automation\workers\jarvis   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/review-bundle-2026-10-29b-jarvis
          git fetch origin && git checkout -B codex/review-bundle-2026-10-29b-jarvis origin/main

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-review-bundle-b14b-jarvis.md
```

RAG health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that and read the files
directly. Push all three RETURNs. Before ending, run `git status --porcelain` and `git log @{u}..`; both must be empty.
