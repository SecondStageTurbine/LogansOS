<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 336: #70 item 5 re-check (the hidden Language/Keyboard pages)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:** `Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md` §336, running §315 of `Plans/DISPATCH-2026-11-08-relays-315-321.md`.
**Reviewed:** PR #70 head `1c036bf25b331576a28d75d30ef765c123fa776c` (fix `c2be34bf6c09db5b8265a0524b42ab97f7932545`), against `origin/main` `93dee06723262afb50cfc4229b01877b8a42b138`. Earlier FAIL: `Plans/RETURN-2026-11-03-pr70-setup-s1-review-janus.md` item 5. Newest PM note: PR #70 comment, 2026-11-08T07:40:08Z.

**Overall: PASS.** Both scenarios now reject `page=language` and `page=keyboard` over the whole serial log, and the default `setup-menu-boot-test` build cannot emit either page. The fix is test-only; the five prior PASS items are untouched.

## Item 5 — PASS

| Check | Verdict | Enforcing lines |
|---|---|---|
| The walk scenario rejects both hidden pages | **PASS** | `tools/xtask/src/main.rs:20081-20092` |
| The skip scenario rejects both hidden pages | **PASS** | `tools/xtask/src/main.rs:20142-20153` |
| The rejection is whole-log, not only the ordered subsequence | **PASS** | `tools/xtask/src/main.rs:21800-21816` |
| The default build hides both pages | **PASS** | `userland/neo-init/src/desktop_session/setup.rs:24`; `userland/setup-core/src/lib.rs:149-160`; `userland/neo-init/Cargo.toml:194-198` |

The original gap was that the skip scenario checked an ordered subsequence (Welcome → Skip → done → intro) and only four page names; a `page=language` line before Welcome could pass. `require_serial_absent` reads the serial file and fails on a plain `serial.contains(needle)` (`tools/xtask/src/main.rs:21806-21816`), so the two additions close it wherever the line occurs. The walk scenario gets the same two checks, as the PM note requested.

The default build is right: `setup-menu-boot-test = ["setup-menu", "desktop-intro-boot-test"]` (`userland/neo-init/Cargo.toml:198`) does not enable `setup-stub-pages`; `run` constructs `Wizard::new(cfg!(feature = "setup-stub-pages"))` (`userland/neo-init/src/desktop_session/setup.rs:24`); and `Wizard::pages` pushes Language/Keyboard only under `stub_pages` (`userland/setup-core/src/lib.rs:149-160`). xtask passes the scenario feature to neo-init through the default `Some(f) => vec![f]` arm (`tools/xtask/src/main.rs:7688`), so the scenarios are not silently building without `setup-menu`.

## Anything new

`c2be34bf6` is one file, +26 lines: the four `require_serial_absent` calls and their comment (`git show --stat c2be34bf6`). No product code changes; the prior PASSes (S1 flow, independent oracle, every-boot/no-persistence, kill-switch, prior art) are unaffected. The merge head `1c036bf2` also brings in main's F8 clock work; its only xtask conflict was Arthur's feature arm, where the F8 `desktop-clock-boot-test` split is kept (`tools/xtask/src/main.rs:8120-8126`) and `setup-menu-boot-test` stays on the intro arm (`:8111-8118`). Nothing in the setup path changed.

## Verification

- `cargo test -p setup-core` (at `1c036bf2`): **8 passed, 0 failed**.
- `cargo test -p xtask` (at `1c036bf2`): **195 passed, 0 failed, 1 ignored**.
- PR run on the exact head: https://github.com/SecondStageTurbine/LogansOS/actions/runs/42452669297 — `completed/success`, all 4 QEMU shards plus Host and Build Checks. (QEMU was not rerun locally; the review tree has no QEMU executable.)
