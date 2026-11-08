<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 322: Dexter reads the L1a.1 step-2b design (#72: stdio chosen at spawn)

Same rules as `Plans/DISPATCH-2026-11-01-m910q-reviews-and-l2b.md`. `git fetch origin +pull/72/head:pr72`; the THINK is
`docs/THINK-2026-11-08-l1a1-stdio-selection.md` (head `b8bd99c4` when queued; review the head you fetch). The PM answered
its §5 on #72: no implicit inheritance, budgeted spawns later, a closed stdout stays EBADF.

## Section 322: Dexter, #72 design read
**MODEL** DeepSeek, **EFFORT** high: a spawn-ABI change (two new syscalls) on the kernel/process boundary.
1. `SYS_SPAWN_STDIO` / `SYS_STDIO_INFO`: arguments, rights required on each handle (a read end for stdin, a write end for
   stdout), and every error case; a byte or number oracle is planned for the ABI.
2. The selection is set on the child before it can run (before `try_push_ready`), so no window exists where fd 0/1 point
   at nothing or at the parent's console.
3. Endpoint counting: passing a pipe end as a child's stdio retains it correctly, and the child's exit releases it
   (no leak, no early EOF while the parent still holds a copy).
4. fd 0/1 with a pipe vs. the console: read/write routing, `EBADF` for a closed stdout, and blocking/non-blocking behaviour
   match the pipe's own rules from #56.
5. Kill-switch: the feature off gives today's spawn and console stdio, unchanged.
6. The pitfall table (CP rows: CP-14, CP-33, CP-61, CP-67 as they apply to a pipe held as stdio) and the planned
   discriminators (the `pipe_stdio` scenario and its late-control) are present and would catch the trap they name.
RETURN: `Plans/RETURN-2026-11-08-pr72-design-dexter.md`, branch `dexter/pr72-design`.
