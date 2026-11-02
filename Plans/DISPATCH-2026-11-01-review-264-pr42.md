<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 264: Jarvis, review of #42 (L1a.2: neo-term-core, the terminal's core)

**MODEL** GPT-6-Luna, **EFFORT** high: one subsystem (userland), but it is the terminal's input/output boundary.

Same rules as `Plans/DISPATCH-2026-11-01-m910q-reviews-and-l2b.md`. `git fetch origin +pull/42/head:pr42 && git diff
origin/main...pr42` (head `861a140a`). The design you approved is `docs/THINK-2026-10-31-l1a-terminal-window.md` on main.

1. The escape-sequence parser implements exactly the THINK's subset; anything else (OSC 52, title set, device
   queries beyond the fixed answerbacks) is consumed and ignored, never echoed back or acted on.
2. Malformed or endless sequences (no terminator, huge parameters, deep nesting) are bounded: no unbounded buffer,
   no panic, no integer overflow on parameters.
3. The key encoder emits the byte table the THINK specifies (arrows normal/DECCKM, function and editing keys,
   modifiers, Ctrl+letter), checked against its hand-laid byte oracle.
4. The grid: resize, scroll regions, wide characters and cursor bounds can't index out of range.
5. Answerbacks are fixed strings only; nothing from the child's output can make the terminal send arbitrary bytes
   back to the child.
RETURN: `Plans/RETURN-2026-11-01-pr42-term-core-review-jarvis.md`, branch `codex/pr42-review-jarvis`.
