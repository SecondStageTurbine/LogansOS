<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 366 (#72 code review) and Seat C's next lane (2026-11-09)

Read with `git fetch origin && git show origin/main:<path>`; PR heads with `git fetch origin +pull/<N>/head:refs/pr/<N>`.

## Section 366: Janus, #72 (L1a.1 step 2b: stdio chosen at spawn). **MODEL** GPT-6.1-Sol, high: kernel spawn path, new syscalls, cross-process handles.
Head `576cd862`; PR run 42493229380 is green. The design is THINK 2b revision 2, which Dexter passed in relays 322/325.
§7 of the THINK lists every place the build departs from that revision. Answer each item pass/fail plus `file:line`,
with a concrete input for any fail.
- [ ] The stdio selection is stored on the child before `register`/`try_push_ready`
  (`process.rs` `spawn_user_body!`, `Stdio::for_spawn`): no window where the child runs without it.
- [ ] `sys_spawn_impl`'s stdio checks (step 2 and step 4, `stdio_roles_ok`) refuse every bad combination. Check the
  wrong pipe end, a closed handle, a handle of another kind, and a slot out of range; name the refusal for each.
- [ ] `pipe_lookup`'s `expect` and `buffer_handle_lookup_at` check the `CapId` before using the handle.
- [ ] The fd 0/1 arms in `sys_read_impl`/`sys_write_impl` (`selected_stdio`) copy the selection out before resolving
  it, so a concurrent close gives `EBADF`, not a use-after-free. This is the CP-112 case; check it against the CP table.
- [ ] `SYS_STDIO_INFO` and `SYS_STDIO_TERM_MAP`: the error, all-none and term-identity cases match the THINK. The
  SDK-ABI rows match the kernel's numbers.
- [ ] Endpoint counting: the child's copy is retained, and its exit releases it, with no leak and no early EOF.
- [ ] Kill-switch: with `pipe` off, spawn and console stdio are today's, unchanged.
- [ ] The late-control's witness would fail on the bug it guards. It has only been clippy-checked so far: say whether
  that's acceptable for a control, or name the run it needs.
- [ ] The CP table (CP-14/33/61/67/112): each class is introduced, guarded or N/A, with its discriminator.
RETURN: `Plans/RETURN-2026-11-09-pr72-review-janus.md` on `codex/pr72-review-janus`.

## Seat C: Arthur's drift check on Q8_0 (host-only)
Arthur moves to Q8_0 in #90. The drift harness in #84 measured the old Q4_K_M. Once #84 is ready (Jarvis's re-check,
relay 364), stack a branch on `dexter/quant-drift`, `claude/quant-drift-q8`, that runs the same bars on the shipped
Q8_0 `.neom` from #90:
- Bar A: ours vs llama.cpp on the same Q8_0 file.
- Bar B: Q8_0 vs the same-checkpoint F16.
- Control C, unchanged.
Use the same pinned window and IDs. Report the numbers in a short THINK addendum and re-pin a Q8 KAT next to the Q4 one,
keeping the Q4 KAT as the historical record. Don't change the bars.

Gate: host only. Run the close-gate, scan, and open a PR stacked on #84. The PM routes its review.
