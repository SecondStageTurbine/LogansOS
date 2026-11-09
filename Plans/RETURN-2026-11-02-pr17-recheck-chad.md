<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: relay #275, PR #17 item 1 re-check (MBR overlap)

Reviewed head: `96cf462cd2eeb69905348f6a8e149ff658612ee6`.

- **PASS — previously reported MBR overlap FAIL is fixed.** `kernel/src/drivers/block/mbr_logic.rs:216-237` builds claimed ranges from every nonempty primary entry and checks each FAT candidate against all other primary ranges plus discovered logical ranges; `:261-262` rejects an intersection. This is symmetric, so a rejected or non-FAT entry still blocks a FAT entry that overlaps it. The exact earlier input (FAT32-LBA `[2048,6144)` overlapping Linux `[4096,8192)`) is recorded at `tools/block-guard-logic-tests/src/mbr_tests.rs:191-211`; the test asserts the FAT entry is rejected and cannot be mounted. It also covers overlap with a past-end entry, an extended container, and a logical range. Production collects extended-chain ranges before classification in `kernel/src/drivers/block/mod.rs:794-826`; the bounded walk and its range/loop checks are in `kernel/src/drivers/block/mbr_logic.rs:155-193`.
- **Verification:** `cargo test -p block-guard-logic-tests mbr_tests -- --nocapture` against the reviewed head passed all 12 MBR tests.
- **New findings:** None for item 1.
