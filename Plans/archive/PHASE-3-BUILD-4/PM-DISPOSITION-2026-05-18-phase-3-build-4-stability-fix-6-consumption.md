# PM Disposition - Phase 3 BUILD-4 Stability Fix 6 Consumption

Date: 2026-05-18

**SUPERSEDED:** This close-with-caveat disposition is superseded by
`Plans/PM-DISPOSITION-2026-05-18-phase-3-build-4-close-with-caveat-superseded.md`.
BUILD-4 does not close while the stale-fsync double-timeout tail remains known.

Authority consumed:

- `Plans/PHASE-3-BUILD-4-STABILITY-FIX-6-BLOCKER-2026-05-17.md`
- `Plans/DISPATCH-2026-05-17-phase-3-build-4-stability-fix-6-smaller-neofs-disk-cerebro.md`
- `Plans/RESEARCH-2026-05-16-phase-3-build-4-virtio-kvm-timing-prometheus.md`

## Disposition

PM accepts the Fix 6 blocker and accepts Cerebro's conclusion that the
test-infrastructure tuning lanes are exhausted.

The immediate path is option (d): accept the residual `phase3_neofs_fsync_filecap_stale_regression`
double-timeout tail as a documented substrate caveat, preserve the diagnostic
substrate, and prepare a close-with-caveat artifact for BUILD-4 stability work.

Option (c), the storage architecture lane, remains real but is not authorized
inside BUILD-4 stability close. Pipelined fsync drain or multi-in-flight block
I/O requires a separate design lane because it changes durability and
crash-recovery ordering assumptions.

## Rationale

Three sequential low-blast-radius interventions moved the tail but did not
remove it:

- Fix 9: serialized block-I/O waiters are woken to the front of the ready queue;
  failures still clustered around `seed_fsync`.
- Fix 14: I/O-heavy NeoFS scenarios received a targeted 360s per-attempt budget;
  failures migrated to pre-mount.
- Fix 15: I/O-heavy NeoFS scenarios received a smaller valid 72 MiB test disk;
  mount moved inside budget, but `seed_fsync` double-timeouts returned.

That pattern is now evidence, not noise. The remaining failure mode is broad
per-operation host/QEMU/virtio latency variance under SMP=4. More timeout,
backend, or disk-size tuning would train the wrong reflex and obscure the
architectural choice that is actually left.

## Accepted Substrate

The following uncommitted work remains accepted for the close-with-caveat lane:

- Fix 8: per-attempt serial logs.
- Fix 9: narrow virtio-only `wake_thread_token_front`.
- Fix 11: `NEO_QEMU_STORAGE_MODE` diagnostic env gate, default unchanged.
- Fix 12: `NEO_TEST_RUN_ID` log suffix.
- Fix 14: targeted 360s timeout for the I/O-heavy NeoFS allowlist.
- Fix 15: targeted 72 MiB NeoFS image for the same I/O-heavy allowlist.
- Stale-fsync `NEO_TEST/1 event=PROGRESS` frames.

The rejected AP idle-halt experiment remains rejected and must not be revived
or committed in this lane.

## Guardrails

No further test-infrastructure tuning is authorized in this lane:

- no larger timeout;
- no additional retry attempts;
- no new backend mode experiments;
- no further disk-size tuning;
- no scenario removal, ignore marker, or CI masking;
- no storage architecture changes;
- no BUILD-4B feature work.

The only authorized work now is packaging the accepted substrate, recording the
known caveat honestly, and producing close evidence.

## BUILD-4B Status

BUILD-4B remains blocked until PM consumes the close-with-caveat artifact.
This disposition authorizes preparation of that artifact; it does not authorize
merge, push, BUILD-4B, sys_mount, FAT32, ext4, or storage architecture work.
