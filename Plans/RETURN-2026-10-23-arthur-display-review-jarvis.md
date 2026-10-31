<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #46): review of PR #108, Arthur display fixes

**Reviewed SHA:** `821dd6349662111f94f383f9e304f8acb82abfc3` (base `436674dd`)

**Verdict:** **PASS.** All 18 checklist items pass at the pinned revision. In particular, the prior item 5 defect at `cfb995ed` is corrected: the same effective reply cap now drives both generation and cut detection.

RAG health was healthy (`ollama=true`, `qdrant=true`). I used direct searches and source reads for the findings below. `cargo test -p arthur-logic-tests` passed all 81 tests. Host `cargo check` passed for `arthur-real-vocab`, `desktop-session`, and the combined `arthur-real-brain-boot-qwen35,desktop-session` feature sets. `cargo clean` then removed all build artifacts.

## Checklist (1–18)

1. **PASS.** `is_control_spelling` requires `<|`, a non-empty interior, and `|>`; construction includes every matching vocab entry plus sidecar EOS, then sorts and deduplicates before binary search (`userland/neo-arthur/src/vocab_sidecar.rs:78`, `userland/neo-arthur/src/vocab_sidecar.rs:120`).

2. **PASS.** `reply_tokens` finds the first control ID and returns the preceding slice, falling back to the full slice (`userland/neo-arthur/src/vocab_sidecar.rs:56`). The only-control and no-control cases are tested explicitly (`userland/neo-arthur/src/vocab_sidecar.rs:460`).

3. **PASS.** The real-vocab decode passes only `reply_tokens(tokens)` to the tokenizer; the resulting output is what reaches the display and `reply="` log (`userland/neo-arthur/src/main.rs:3421`, `userland/neo-arthur/src/conversation_logic.rs:1576`).

4. **PASS.** `reply_was_cut` requires both `tokens.len() >= max_new` and an unchanged reply slice, so a control token—including one at the cap—prevents the marker (`userland/neo-arthur/src/vocab_sidecar.rs:64`). The at-cap stop-token case is tested (`userland/neo-arthur/src/vocab_sidecar.rs:474`).

5. **PASS.** The mutually exclusive definitions select 32 for Qwen 3.5 and otherwise select `STAGE_B_MAX_NEW_TOKENS`, which is 256 for desktop and 32 for non-desktop real vocab (`userland/neo-arthur/src/main.rs:327`, `userland/neo-arthur/src/main.rs:335`, `userland/neo-arthur/src/main.rs:341`, `userland/neo-arthur/src/main.rs:344`). Both the `sys_generate` request and `reply_was_cut` consume `REPLY_MAX_NEW_TOKENS` (`userland/neo-arthur/src/main.rs:2843`, `userland/neo-arthur/src/main.rs:3433`). The three representative feature classes compile, including Qwen 3.5 + desktop; thus 32 ordinary tokens without control are marked cut there, while a shorter reply or any control-ended reply is not.

6. **PASS.** The backing array is sized by `STAGE_B_MAX_NEW_TOKENS`; the effective cap is either equal to it or the Qwen 3.5 value 32. Therefore `&mut buf[..max_new]` is in bounds in every buildable configuration (`userland/neo-arthur/src/main.rs:2843`).

7. **PASS.** The kernel admits `max_new <= 256` before copying or allocation (`kernel/src/ai/generate_logic.rs:22`, `kernel/src/ai/generate_logic.rs:105`, `kernel/src/arch/x86_64/syscall.rs:3620`). The inventory path uses checked `prompt.len() + max_new` as its cache length (`kernel/src/arch/x86_64/syscall.rs:3409`), and `InventoryDecodeState::new` refuses a length above model context before allocating caches (`kernel/src/ai/qwen_inventory.rs:113`). Arthur's context window is 512 (`userland/neo-arthur/src/conversation_logic.rs:32`), so the sum is either allocated exactly or cleanly rejected.

8. **PASS.** The 4096-byte limit exists only with `desktop-session`; all other builds retain 256 (`userland/neo-arthur/src/conversation_logic.rs:34`). Desktop returns through `run_desktop_session` and writes replies to its surface (`userland/neo-arthur/src/main.rs:558`, `userland/neo-arthur/src/main.rs:1024`) before the generic channel loop. Console/channel consumers are in the mutually exclusive non-desktop route (`userland/neo-arthur/src/main.rs:613`, `userland/neo-arthur/src/main.rs:637`).

9. **PASS.** After decode and marker append, `generate_model_reply` rejects output above `MAX_REPLY_BYTES` (`userland/neo-arthur/src/conversation_logic.rs:1568`). Incremental construction uses `saturating_add` and returns `ReplyTooLarge`, including `push_memory_reply` (`userland/neo-arthur/src/main.rs:2568`) and the common helper (`userland/neo-arthur/src/conversation_logic.rs:1618`), so there is no panic or arithmetic overflow.

10. **PASS.** `footer_model` returns `NoModel` unless ready and holding a model, and derives a name only from `model_path` (`userland/neo-arthur/src/main.rs:2777`). That path is stored only after `sys_model_load` succeeds and the handle is installed (`userland/neo-arthur/src/main.rs:2762`).

11. **PASS.** The required path becomes `Qwen2.5 0.5B`; path splitting and word iteration are safe for non-ASCII, empty, `/`, `model-.neom`, and `-` (`userland/neo-arthur/src/showcase_logic.rs:318`). The only byte-indexed slice is guarded by `is_size_word`, which requires an ASCII unit and all-ASCII numeric prefix (`userland/neo-arthur/src/showcase_logic.rs:348`). Exact naming cases are tested (`tools/arthur-logic-tests/src/showcase_s2_tests.rs:526`).

12. **PASS.** Name shortening walks valid `char_indices`, strictly decreases `cut`, and falls back to `Esc to go back` (`userland/neo-arthur/src/showcase_logic.rs:359`). Multibyte shortening is tested (`tools/arthur-logic-tests/src/showcase_s2_tests.rs:612`), as is real-font fitting at the 320-pixel minimum (`tools/arthur-logic-tests/src/showcase_s2_tests.rs:620`; `userland/neo-arthur/src/showcase_logic.rs:25`).

13. **PASS.** `set_footer` clears/redraws precisely `footer_rule_y + 1` through the bottom and unions only that rectangle into `footer_damage`; it does not touch transcript history or transcript damage (`userland/neo-arthur/src/showcase_text.rs:224`). The pixel and damage boundary is tested (`tools/arthur-logic-tests/src/showcase_s2_tests.rs:653`).

14. **PASS.** `commit_footer` consumes only footer damage, advances from `client_serial` with checked addition, and commits that rectangle (`userland/neo-arthur/src/showcase_surface.rs:421`). The serial changes only after a successful request/reply (`userland/neo-arthur/src/showcase_surface.rs:439`). It runs after the handshake and returns 63 on failure (`userland/neo-arthur/src/main.rs:952`); returning exits Arthur rather than stranding it in the session loop. Separate footer consumption leaves transcript damage pending, and a second footer take is empty in the host test (`tools/arthur-logic-tests/src/showcase_s2_tests.rs:680`).

15. **PASS.** `is_footer_band` requires non-empty damage, `damage.y > client.bottom() - 36`, and full client containment (`userland/neo-init/src/desktop_session.rs:1627`). Footer commits take a separate branch, leaving `damage_logged` for the first keystroke (`userland/neo-init/src/desktop_session.rs:1393`). Both WM and renderer constants are 36 (`userland/neo-init/src/desktop_session.rs:80`, `userland/neo-arthur/src/showcase_text.rs:59`).

16. **PASS.** Xtask requires a footer line and `band=yes` (`tools/xtask/src/main.rs:12177`), requires the no-model line in `desktop_session_boot` (`tools/xtask/src/main.rs:12211`), requires `ARTHUR footer model="Qwen2.5 0.5B"` in the real compositor scenario (`tools/xtask/src/main.rs:12903`), and rejects any real-brain `reply="` line containing `<|` (`tools/xtask/src/main.rs:13096`). Each missing or bad witness returns `Err`.

17. **PASS.** Vocab tests cover EOS and arbitrary-control cuts, only-control/no-control behavior, cap marking, at-cap EOS, and the literal marker (`userland/neo-arthur/src/vocab_sidecar.rs:440`). Showcase tests cover exact names, character-count fitting, multibyte shortening, real-font fitting, and footer damage (`tools/arthur-logic-tests/src/showcase_s2_tests.rs:524`). Spot-check: expected strings, cap `4`, marker literal, and the production 320-pixel minimum are concrete values, not values recomputed by the functions under test. All 81 host tests passed.

18. **PASS.** `git diff 436674dd 821dd634 --stat` changes the dispatch plus files under `userland/` and `tools/`; it touches no `kernel/` file, syscall definition, or ABI structure.

## Worktree note

Pre-existing unrelated changes to `.gitignore`, `.ignore`, and `tools/acpi-aml-grade/tests/jarvis_review_probe.rs` were preserved and excluded from this commit.
