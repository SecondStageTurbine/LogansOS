# Phase 3 BUILD-2 Close SMP Sweep Addendum

Date: 2026-05-14
Owner: Jarvis
Branch: `codex/phase-3-build-2-2e-typed-inode-filecap`
HEAD under test: `f6532cb7dabb5f94adc1134b5cfd2fdd6b2ee6be`
Dispatch: `Plans/DISPATCH-2026-05-14-phase-3-build-2-close-smp-sweep-jarvis.md`

## Verdict

PASS. The full non-explicit `NEO_SMP=4` sweep completed with exit status 0:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

This addendum is evidence only. It does not claim BUILD-2 closure by itself.
No code changes, close report, BUILD-3 work, merge, or push were performed.

## Run Record

- Command: `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta`
- Exit status: `0`
- Wrapper exit file: `target/neo-test/smp-sweep-2026-05-14/exit-code.txt`
- Docker/xtask log: `target/neo-test/smp-sweep-2026-05-14/stderr.log`
- Wrapper stdout log: `target/neo-test/smp-sweep-2026-05-14/stdout.log`

The pre-run Docker check found no leftover `altimitneo-neo-run` test
container. An unrelated `altimitneo-qdrant` orphan container was present and
was not modified.

## Prior Abandoned-I/O Failure

The prior terminal failure in `phase3_virtio_blk_abandoned_io_no_leak` did not
reproduce in this full rerun. In this sweep, xtask treated the first QEMU
timeout as a transient retry candidate and the second attempt passed:

```text
[xtask] neo-test: `phase3_virtio_blk_abandoned_io_no_leak` attempt 1/2 failed: QEMU timed out after 180s for
`phase3_virtio_blk_abandoned_io_no_leak`
[xtask] neo-test: retrying `phase3_virtio_blk_abandoned_io_no_leak` after transient QEMU failure candidate
[xtask] neo-test: `phase3_virtio_blk_abandoned_io_no_leak` PASS
[xtask] neo-test: `phase3_virtio_blk_abandoned_io_no_leak` PASS after retry attempt 2/2
```

The final serial PASS frame for that scenario was:

```text
NEO_TEST/1 event=PASS name=phase3_virtio_blk_abandoned_io_no_leak bdf=0x000300 retained_dma=3 live_before=3 live_after=3 late_wake=2 completions_delta=1 F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false
```

No `virtio_blk_abandoned_io_dma_live_leak` terminal failure was observed in
the completed sweep.

## Retry Notes

The full sweep completed with exit 0 after xtask retried these transient QEMU
timeout candidates:

```text
[xtask] neo-test: `phase3_neofs_mount_selects_highest_generation` attempt 1/2 failed: QEMU timed out after 180s for
[xtask] neo-test: `phase3_neofs_mount_selects_highest_generation` PASS after retry attempt 2/2
[xtask] neo-test: `phase3_neofs_fsync_noop_write_through` attempt 1/2 failed: QEMU timed out after 180s for
[xtask] neo-test: `phase3_neofs_fsync_noop_write_through` PASS after retry attempt 2/2
[xtask] neo-test: `phase3_neofs_crash_mid_create_recovery` two-boot attempt 1/2 failed: QEMU timed out after 180s for
[xtask] neo-test: `phase3_neofs_crash_mid_create_recovery` two-boot PASS after retry attempt 2/2
```

These were not terminal failures; the command returned exit status 0.

## 2E Scenario Evidence

The full sweep included the accepted 2E scenarios. Current serial logs contain
these PASS frames:

```text
NEO_TEST/1 event=PASS name=phase3_neofs_typed_inode_exec_refusal model_read=ok read_bytes=54 forced_execute_cap=true exec_policy=TypeForbidden type_first=true selected_superblock=0 generation=1 no_real_mmap_exec=true
NEO_TEST/1 event=PASS name=phase3_neofs_regular_exec_policy_stub path=/neofs/hello.txt regular_execute_policy=ok execute_right=0x00000010 no_actual_mmap=true no_actual_exec=true selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_neofs_type_vs_rights_policy_errors matrix_model_exec=TypeForbidden matrix_model_no_exec=TypeForbidden matrix_regular_no_exec=InsufficientRights matrix_regular_exec=ok type_first=true rights_second=true selected_superblock=0 generation=1 no_real_mmap_exec=true
NEO_TEST/1 event=PASS name=phase3_neofs_filecap_generation_stale path=/neofs/stale-2e.txt stale_cap=ESTALE stale_errno=-116 old_cap_generation=7 refreshed_generation=9 current_inode_generation=11 handle_a_continued=true reachable_generation_checked=true selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_neofs_reserved_type_refusal reserved_type_mount_rejected=NoValidSuperblock executable_access=NotFound inert_before_rights=true no_real_mmap_exec=true
NEO_TEST/1 event=PASS name=phase3_neofs_filerights_u32_forward_bits kernel_filerights_size=4 libneo_filerights_size=4 requested_bits=0x0c000010 observed_bits=0x0c000010 retype_bit=0x04000000 open_error=InsufficientRights policy_preserved_high_bits=true selected_superblock=0 generation=1
```

## AArch64 Barrier Lane

The full sweep also executed the runtime aarch64 barrier lane and recorded a
runtime QEMU PASS, not compile-only evidence:

```text
NEO_TEST/1 event=PASS name=phase3_aarch64_virtio_barrier_lane runtime=aarch64 qemu_machine=virt transport=virtio_mmio event_idx_negotiated=true feature_low=0x30006e54 feature_high=0x00000101 sector=8 block_size=512 write_ok=true read_ok=true data_integrity=ok publish_descriptors=2 before_notification_check=2 notification_decisions=2 notifications_sent=2 compile_only=false sentinel=runtime_qemu_aarch64
```

## Stop-State Confirmation

This dispatch was executed as verification-only. The only repository artifact
created for this run is this addendum. No blocker artifact was filed because
the command passed. No BUILD-2 close report, BUILD-3 work, merge, or push was
performed.
