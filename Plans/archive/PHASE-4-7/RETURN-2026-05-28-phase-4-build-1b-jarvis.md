# RETURN: Phase 4 BUILD-1b - sys_socket_* syscall surface (Jarvis)

Date: 2026-05-29
Branch: codex/phase-4-build-1b-jarvis
Status: BLOCKED ON ISC-1b-5 FULL SWEEP

## Branch / Plan Context

The local branch was on 43fc2da and had diverged from origin/main. The controlling
PLAN was read from origin/main at 09f5a46:

- Plans/PLAN-2026-05-28-phase-4-build-1b-socket-syscalls.md

The stale local BLOCKER and DISPATCH files from the previous missing-PLAN stop were
removed before implementation resumed.

## Implementation Summary

Implemented the BUILD-1b kernel-internal UDP socket syscall foundation:

- Added `kernel/src/net/socket.rs` with a small UDP datagram socket abstraction,
  AF_INET/AF_INET6 validation, bind state, bounded receive queue, and loopback
  datagram enqueue/dequeue.
- Added socket handles and rights to `kernel/src/sched/process.rs`.
- Added syscall numbers 0x180-0x184 and handlers in
  `kernel/src/arch/x86_64/syscall.rs`:
  - `sys_socket_create`
  - `sys_socket_bind`
  - `sys_socket_send`
  - `sys_socket_recv`
  - `sys_socket_close`
- Added userland wrappers and IPv4 sockaddr helper in `userland/libneo/src/lib.rs`.
- Added the `phase4-socket-udp-loopback` user feature and smoke path in
  `userland/neo-init`.
- Added the `phase4_socket_udp_loopback` neo-test scenario and corresponding
  kernel/user feature wiring.

No syscall ABI beyond the planned 0x180-0x184 BUILD-1b surface was added.
No Phase 3 code was modified.

## Validation Evidence

Passing targeted checks:

- `cargo check -p neoplutarch-kernel --features neo-test-phase4-socket-udp-loopback`
  passed on the Windows host toolchain.
- `cargo check -p neo-init --features phase4-socket-udp-loopback` passed.
- `cargo check -p neo-worker --features phase4-socket-udp-loopback` passed.
- `docker compose run --rm -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test phase4_socket_udp_loopback --no-meta`
  passed.
- `docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test phase4_socket_udp_loopback --no-meta`
  passed.

The direct Windows-host `cargo xtask neo-test phase4_socket_udp_loopback` path could
not run QEMU because Limine was not available on the host:

```text
xtask: Limine directory /home/neo/limine does not exist. Set $LIMINE_DIR or build the dev container.
```

Docker validation was used for the real QEMU smoke path.

## ISC Status

- ISC-1b-1: PASS. Syscall numbers and kernel dispatch were added.
- ISC-1b-2: PASS. Kernel/user cargo checks passed; existing warnings remain.
- ISC-1b-3: PASS. `phase4_socket_udp_loopback` QEMU smoke passed.
- ISC-1b-4: PASS. No `unwrap`, `expect`, or `panic!` was introduced in the new
  socket module. The only matched `expect` in the checked files is a pre-existing
  syscall STAR initialization assert.
- ISC-1b-5: BLOCKED. Full `NEO_SMP=4` all-scenario sweep failed in an unrelated
  explicit benchmark before completing the suite.

Blocking command:

```powershell
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test all --no-meta
```

Failure:

```text
xtask: 'irqsafe_mutex_bench' did not exit via debug success; status=exit status: 35 serial=/workspace/target/neo-test/irqsafe_mutex_bench.attempt1.serial.log
```

Serial evidence:

```text
NEO_TEST/1 event=FAIL name=irqsafe_mutex_bench reason=overhead_threshold overhead_ns=6514 threshold_ns=6000 spin_mean_ns=410 irqsafe_mean_ns=6924 switch_mean_ns=1419
```

This failure is not in BUILD-1b socket code and is not a Phase 3 publication-fence
issue. No workaround was applied.

## PM / PL Disposition Needed

BUILD-1b implementation and targeted smoke validation are complete, but close is
blocked on ISC-1b-5 because the current full `all` sweep includes the explicit
`irqsafe_mutex_bench` benchmark and that benchmark exceeded its overhead threshold
under `NEO_SMP=4`.

PM/PL should decide whether to:

1. Treat the explicit benchmark threshold failure as a separate validation-lane
   blocker and re-run BUILD-1b close after it is cleared, or
2. Adjust the ISC-1b-5 sweep definition so explicit benchmarks are excluded from
   the BUILD-1b close gate.

