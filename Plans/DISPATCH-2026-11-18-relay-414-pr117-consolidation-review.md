<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 414: Jarvis, a code review of the 2026-11-18 consolidation (#117)

**MODEL** GPT-6-Luna, **EFFORT** high.
Why: it's contained code (one kernel boot file, xtask, CI workflows), with no SMP core and no new syscall.

- **Head:** `474c19e7` on `claude/consolidation-2026-11-18`, on `main` `fb08219d`.
- **The PR body** lists each change and what it closes (OT-76, OT-119 a/e/f, OT-113).

Answer each item pass or fail with the enforcing `file:line`. Give a concrete input for any fail.

1. **The boot-frame fold** (`kernel/src/init/boot_frame.rs`, `init/mod.rs`; `boot_frame_eth0.rs` deleted):
   - [ ] With `boot-eth0` off, the compiled `boot_frame` is the same as before. The only additions are `cfg`'d
     out.
   - [ ] With it on, the status line is the deleted fork's: the same helper and the same call.
   - [ ] Nothing else referenced the fork's path.
2. **The tier contract** (`tools/xtask/src/main.rs`, `parse_neo_test_args`, `no_tier_is_the_full_tier`):
   - [ ] The extracted parser is the old loop verbatim, apart from returning a struct, and help returns `Ok(None)`.
   - [ ] `cmd_neo_test`'s behaviour is unchanged: every flag reaches the same variable, and the validation after the
     loop runs as before.
   - [ ] The test really pins "no `--tier` equals `--tier full`", including flag order.
3. **The label changes:**
   - [ ] The emitter (`userland/neo-arthur/src/main.rs`) and the needle (`tools/xtask/src/main.rs`) still match.
   - [ ] No other scenario or tool matches the old string.
4. **The rustup pin** (`.github/workflows/ci.yml`, `ot61-closure.yml`):
   - [ ] All nine fallbacks changed the same way, and none remain.
   - [ ] A checksum mismatch fails the step under the job's shell.
   - [ ] The arguments to `rustup-init` are the same as the old script's.
5. **The ledger re-measure** (`docs/complexity-governance.md`):
   - [ ] The counts are reproducible by the stated method.
   - [ ] Spot-check three rows' cited files and symbols.

**RETURN:** `Plans/RETURN-2026-11-18-pr117-review-jarvis.md` on `codex/pr117-review-jarvis`. Push it.
