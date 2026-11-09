<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 313 — Chad — PR #56 item-5 re-check

Reviewed head: `71bc2dcacab6753403d3a4c6ef9d6b2af69a3c62`
(`claude/l1a1-pipe-wiring`; squash `6a5652708` plus `71bc2dcac`).
Fetched `pull/56/head:pr56`, read `git diff origin/main...pr56`, the newest
`PM:` comment, and relay 303's RETURN at `origin/codex/pr56-review-chad`.
RETURN branch: `codex/pr56-recheck-chad`, from `origin/main` at `5a0a3efed`.
All source references below name the reviewed head. Verdict: **PASS**.

- **PASS — item 5(a), capacity floor and unconditional writer atomicity.**
  `kernel/src/arch/x86_64/syscall.rs:2217` rejects a capacity outside
  `create_capacity_ok` with EINVAL before allocation or handle installation.
  `kernel/src/ipc/pipe_logic.rs:38` sets the floor to `PIPE_ATOMIC` (512),
  and `:41` bounds it through 65536. The KAT at
  `tools/pipe-logic-tests/src/lib.rs:347` refuses 0, 1, 511 and 65537,
  accepts 512, 4096 and 65536, and stores a 512-byte write whole at the floor.
  `syscall.rs:2351` makes a write of at most 512 bytes one chunk;
  `pipe_logic.rs:293` requires space for the entire small write, under the
  guard at `kernel/src/ipc/pipe.rs:270`. Thus two writers cannot interleave
  such writes on any user-created pipe. The syscall scenario checks
  `SYS_PIPE_CREATE(511) == -22` (`kernel/src/test_harness_pipe.rs:232`).
  Readers still consume a byte stream; the PM explicitly withdrew the old
  requirement that a write never be split across readers.

- **PASS — item 5(b), fault after consumption reports copied bytes.**
  `kernel/src/arch/x86_64/user_copy.rs:34` uses `rep movsb`; its fixup at
  `:39` returns RCX, the remaining byte count. `try_copy_count` at `:105`
  returns `len - left` after closing the SMAP window. The pipe read at
  `kernel/src/arch/x86_64/syscall.rs:2301` uses that count, adds preceding
  chunks at `:2309`, and returns EFAULT only when the sum is zero (`:2310`).
  For example, a fault after 17 bytes of a 512-byte first chunk returns 17;
  after a completed 512-byte chunk plus 17 bytes it returns 529; a fault
  before any byte returns -14. The destination precheck remains at `:2288`.
  `docs/SDK-ABI.md:378` also explicitly documents the remaining loss of the
  consumed but uncopied suffix when a sibling unmaps between check and copy.
  This race was checked in source, not reproduced by a timed sibling-unmap test.

- **PASS — squash preserves items 1–4 (park/wake, lock order, lifetime,
  bounded waiters).** Compared the old contribution
  `6fbbb48d7..e60cfa1b1` with the squashed contribution
  `8a6de7429..71bc2dcac`, excluding hunk coordinates and base-only changes,
  and compared against `origin/backup/pipe-wiring-pre-squash` (`2087cc5`).
  The pipe object and scheduler/process/thread contributions are unchanged.
  The same guard reaches token installation (`kernel/src/ipc/pipe.rs:257`,
  `:319`); wakes run after unlock (`:185`, `:186`); full-list purge takes
  pipe then scheduler and returns EAGAIN without evicting live tokens (`:302`,
  `:317`). Thread-owned anchoring and cleanup remain at
  `kernel/src/sched/scheduler.rs:1716`, `:1723` and
  `kernel/src/sched/thread.rs:273`. The prior PASS findings stand.

- **PASS — squash preserves items 6–8 (readiness/identity, kill-switch,
  controls and CP table).** Read/write readiness is unchanged at
  `kernel/src/ipc/pipe_logic.rs:170`, `:180`. The scanner records the key
  and CapId at `kernel/src/arch/x86_64/syscall.rs:6667`, checks identity
  again at `:6788`, and removal fires the old key at
  `kernel/src/sched/process.rs:2085`. Feature gating remains at
  `kernel/Cargo.toml:551`, `syscall.rs:1366`, `:6673`, `:1429`.
  The three guard-removal controls and their discriminators are preserved
  (`kernel/src/ipc/pipe.rs:251`, `syscall.rs:6788`,
  `kernel/src/sched/scheduler.rs:1731`); relay 303's successful control
  discrimination remains prior-head evidence, not a new control run.
  The committed CP table remains at
  `docs/THINK-2026-11-02-l1a1-pipe-wiring.md:113`; the CP-99 documentation
  now adds the explicit partial-copy contract. No unrelated pipe behavior
  changed in the squash; feature/wrapper conflict resolutions preserve the
  original pipe additions alongside main's #15 additions.

- **PASS — new close path, including `71bc2dcac`'s slots 0 and 1.**
  `kernel/src/arch/x86_64/syscall.rs:7280` tries a pipe-only close before
  the stdin/stdout refusals; the normal-slot predicate also accepts pipes
  at `:7323`. `kernel/src/sched/process.rs:3270` checks and removes under
  CAP_GRAPH and the table lock, retires the capability, then releases the
  detached handle outside both locks (`:3290`). `Detached::release` calls
  `release_pipe` once (`:1845`, `:2064`); the returned temporary handle is
  merely an Arc, so dropping it does not decrement the endpoint twice.
  `kernel/src/ipc/pipe_logic.rs:242` decrements one endpoint; a non-last
  close produces no endpoint wakes, while the last close wakes both sides
  (`:263`). `kernel/src/ipc/pipe.rs:174` collects under the pipe lock and
  fires after unlock (`:186`): readers drain then EOF after the last writer,
  writers get EPIPE after the last reader, and a parked sibling on the
  closed end gets EBADF. Failed pipe predicates leave other handles intact
  and fall through to the existing stdio/NetDial rules. The create leg
  closes the write end in slot 1 and reads EOF (`test_harness_pipe.rs:244`);
  slot 0 uses the same predicate and release path. The harness prints a note
  if the ends unexpectedly occupy other slots (`:241`).

Validation: `cargo test -p pipe-logic-tests` at the reviewed head: **23 passed**.
At that exact head, local Docker/QEMU with `NEO_SMP=4`,
`cargo neo-test pipe_core pipe_park_kill --no-meta`: **2 passed, 0 failed**,
both on attempt 1 (QEMU 28.8 s and 5.2 s). The create witness was
`bad_capacity=-22 small_capacity=-22 handles_ok=true write=3 read=3 bytes_ok=true read_end_write=-1 close_write=0 read_after_close=0`;
no unexpected-slot note appeared. CP-14 reported
`rounds=8 woken=8 window_hits=8`, both close-under-parked-sibling legs
returned -9 with `arc=1`, and the full-list witness was
`stale_before=8 extra=-11 live_total=8 arc=1`. No implementation or harness
source was changed. The full PR gate was still in progress when inspected;
this PASS is the scoped review and local validation, not a claim that the
full exact-head gate has completed. No new blocking finding.
