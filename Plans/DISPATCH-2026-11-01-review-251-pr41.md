<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 251: Chad, design read of #41 (THINK L2d: images in the desktop)

**MODEL** GPT-6-Luna, **EFFORT** medium: a design read from a different family than the author (Dexter, DeepSeek).

Same rules as `Plans/DISPATCH-2026-11-01-m910q-reviews-and-l2b.md`. `git fetch origin +pull/41/head:pr41 && git show
pr41:docs/THINK-2026-11-01-l2d-images-in-the-desktop.md` (name the head you read).

1. Decoding stays out of the WM and the kernel; the file reaches the decoder only through a capability the user
   granted. Name any path in the design where it doesn't.
2. The memory budgets (per image, per session) and the cache eviction are bounded; a hostile image can't exceed them.
3. The recommended first build (`neo-view` with a `no_std` PNG backend) is the smallest useful step, and its gate
   (frame-hash oracle) proves the decode reached the screen.
4. Its `file:line` citations into the WM and Atrium on `main` are accurate.

RETURN: `Plans/RETURN-2026-11-01-pr41-review-chad.md`, branch `codex/pr41-review-chad`.
