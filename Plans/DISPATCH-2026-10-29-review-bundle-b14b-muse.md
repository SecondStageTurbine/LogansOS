<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW BUNDLE (relays #137–#138) → Muse: the boot-disk GUID fix and C2.2's containment claim

Two security reviews of Claude-authored code. Both merged after batch 14b (run `40817887458`, green at `67f023bc`).
**Read the files at each squash SHA from the repo, not a summary.** The PL pastes your RETURNs back, and the PM banks
them. Write each as a plain checklist: pass or fail, the enforcing `file:line`, and a concrete input for any fail.

## #137: #186 `e51d8a00`, the boot medium's GPT GUID at Limine's protocol offset. Banked as `Plans/RETURN-2026-10-29-ot87-boot-disk-guid-review-muse.md`

The installer's boot-disk refusal (OT-87 THINK §2, layer 3) never fired, because the `limine` crate 0.6.3 read
`gpt_disk_uuid` at offset 56 instead of 64. Behind `installer`, in no product image. Files:
`kernel/src/limine_file_logic.rs`, `kernel/src/installer.rs` (`boot_disk_guid`), `tools/limine-file-logic-tests/src/lib.rs`,
`kernel/src/test_harness.rs` (`run_install`, the `refusals` plan).
1. `gpt_disk_guid` reads 16 bytes at offset 64 of Limine's `struct limine_file`, and the response is read as raw bytes
   with its length checked. Check the offsets against Limine's raw `PROTOCOL.md` at v8.6.0.
2. A medium with no GPT (zero at offset 64) gives `None`. Can `None` ever let a claim on the boot medium through?
3. The `refusals` scenario requires the real boot medium to be refused `Claim(BootDisk)` before any other check.
4. Does anything else read a crate `File` or `Uuid` field past the gap (`gpt_part_uuid`, `part_uuid`, `media_type`)?
5. The byte-oracle fixture comes from the kernel's own serial dump. Does `the_limine_crate_misplaces_the_field` fail if
   the crate is fixed?

## #138: #190 `130a7ef3`, OT-61 C2.2 memory quotas: the containment claim. Banked as `Plans/RETURN-2026-10-29-ot61-c2.2-review-muse.md`

Per-process memory quotas under `mem-quota` (the `browser-runtime` lane, in no product image). A Codex seat checks the
memory-core correctness separately (#143). Yours is the containment claim: a process can't use more memory than its
budget, and can't use its budget to harm another process. Files: `kernel/src/mm/mem_account_logic.rs`,
`kernel/src/mm/pmap.rs`, `kernel/src/sched/process.rs` (`spawn_user_body`, teardown), and the `SYS_MEM_INFO` and
`SYS_SPAWN_BUDGETED` handlers.
1. Can a process allocate past its budget by any path: mapping, page tables, borrowed regions, or a child?
2. Can a child's budget exceed what its parent holds? Can a budget be created out of nothing?
3. `SYS_SPAWN_BUDGETED` refuses `budget_bytes == 0`. Does it also refuse an overflow, or more than the parent holds?
4. Can one process read or change another's usage through `SYS_MEM_INFO`?
5. Is every charge released exactly once on exit, kill and failed spawn, so a crashing child can't leak its parent's
   budget?
