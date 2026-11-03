<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Relay 307: #66 SWGL re-check (Jarvis)

Reviewed PR head: `5804028b68f0a6c881fb442e3243bce91a52437d` (fix `a15ba7e400601f81a56e7f628dddc9521cfd7f58`), fetched with `git fetch origin +pull/66/head:pr66`; reviewed `git diff origin/main...pr66`.

- **PASS — earlier FAIL, item 2: zero width/height.** `userland/swgl-smoke/src/fb.rs:39` refuses either zero dimension with `Empty`. Smoke inputs at `userland/swgl-smoke/src/main.rs:73` and `:74` cover 0x0/stride 0 and nonzero width/zero height.
- **PASS — item 2: stride below width × 4.** `userland/swgl-smoke/src/fb.rs:46` computes the row size with checked multiplication; `:47` returns `StrideTooSmall`. Smoke cases at `userland/swgl-smoke/src/main.rs:75` and `:76` use stride 0 and 27 for width 7.
- **PASS — item 2: buffer shorter than stride × height.** `userland/swgl-smoke/src/fb.rs:50` checks multiplication and `:51` returns `TooShort`. The smoke at `userland/swgl-smoke/src/main.rs:85` supplies 179 bytes for a 36 × 5 = 180-byte surface.
- **PASS — item 2: misaligned buffer.** `userland/swgl-smoke/src/fb.rs:54` returns `Misaligned` for an address not divisible by four. The smoke at `userland/swgl-smoke/src/main.rs:94` offsets a u32-backed allocation by one byte while keeping enough space for the surface.
- **PASS — item 2: refusal precedes SWGL, with smoke coverage.** `userland/swgl-smoke/src/fb.rs:74` propagates validation errors before the only SWGL framebuffer initialization at `:75`. `userland/swgl-smoke/src/main.rs:114` checks every case's exact error, `:121` checks the original framebuffer is still attached, and `:213` makes that result check 5. `tools/xtask/src/main.rs:18181` and `:18182` require both valid-attach and rejection-check serial markers.
- **PASS — local validation.** Compiled the reviewed `fb.rs` and unchanged `bad_framebuffers_refused` function in a temporary host harness with a recording SWGL boundary stub. All seven smoke cases returned their expected errors; an additional isolated zero-width/nonzero-height case returned `Empty`. The valid attach made one boundary call and all rejected attaches made zero additional calls. This checks validation/control flow, not SWGL rendering. The newest [PM comment](https://github.com/SecondStageTurbine/LogansOS/pull/66#issuecomment-6598591055) reports on-target `swgl_smoke` PASS and green closure build at this head ([run 41590374898](https://github.com/SecondStageTurbine/LogansOS/actions/runs/41590374898)); those target checks were not rerun locally.
- **PASS — anything new:** no additional finding in this narrow framebuffer-validation re-check.

Result: **PASS** (relay 298's item 2 is resolved at the caller-side attachment boundary).
