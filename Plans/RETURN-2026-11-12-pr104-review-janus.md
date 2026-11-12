<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Relay 393 — PR #104, OT-100 R2, Janus code review

**Reviewer:** Janus (Codex), cross-family review of Seat F's implementation.
**Dispatch:** `Plans/DISPATCH-2026-11-12-relay-393-pr104-usb-write-review.md`, Section 393.
**Reviewed head:** `2cf42002d5f6e8bf6cf829e32ae53a9afe5298ab`, read directly from an isolated worktree.
**RETURN branch:** `codex/pr104-review-janus`.
**Verdict: FAIL.** F1–F3 below need correction and the same reviewer's re-check. No new guard bypass was found.
All source coordinates below refer to the reviewed head, not to the RETURN branch's source tree.
This is Section 393 only; it does not stand in for Muse's separate security read.

## Findings and concrete inputs

### F1 — P1: the write path does not enforce the advertised time bounds

`Transport::run_dir` stores a duration and clears it when the command returns
(`kernel/src/drivers/usb/msc.rs:301`, `kernel/src/drivers/usb/msc.rs:303`). Every
`wait_for` invokes `wait_until` with that duration (`kernel/src/drivers/usb/msc.rs:212`),
which calculates a fresh start/end (`kernel/src/drivers/usb/msc.rs:179`). Consequently
CBW, data and CSW each receive another 10 seconds, rather than sharing a chunk deadline;
a no-data SYNC receives a fresh 30 seconds for each of CBW and CSW.

**Concrete timing input:** a WRITE's CBW completes after 9 seconds, its data after another
9 seconds, and its CSW after another 9 seconds. All three waits pass: the chunk succeeds
after 27 seconds. A SYNC with a CBW after 29 seconds and CSW another 29 seconds later
succeeds after 58 seconds. These are completion schedules, not retry accounting.
An extracted, unchanged `wait_until` with a mock endpoint and advancing `TICKS` reproduced
both outcomes (2700 and 5800 ticks at the driver's 100 Hz).

Some waits have **no tick deadline at all**:

- A failed WRITE reaches `MscDisk::sense` (`kernel/src/drivers/usb/msc.rs:526`), which
  delegates to `Transport::sense` (`kernel/src/drivers/usb/msc.rs:479`). That calls the
  old `run` (`kernel/src/drivers/usb/msc.rs:310`) after `deadline_ms` was cleared.
  Its transfers fall through to the poll-only wait (`kernel/src/drivers/usb/msc.rs:219`,
  `kernel/src/drivers/usb/msc.rs:228`). The same happens after a failed SYNC
  (`kernel/src/drivers/usb/msc.rs:592`).
- The post-MODE-SENSE TEST UNIT READY uses `t.run`, without a deadline
  (`kernel/src/drivers/usb/msc.rs:849`).
- Stall/reset recovery calls host endpoint recovery and EP0 control requests
  (`kernel/src/drivers/usb/msc.rs:264`, `kernel/src/drivers/usb/msc.rs:389`). Their waits
  use `wait_event` (`kernel/src/drivers/usb/xhci.rs:1043`,
  `kernel/src/drivers/usb/xhci.rs:2355`), whose bound is a spin budget
  (`kernel/src/drivers/usb/xhci.rs:939`, `kernel/src/drivers/usb/xhci.rs:955`).

**Concrete device input:** return a failed WRITE CSW, then accept REQUEST SENSE's CBW
but never complete its data stage. This new write-error path spends `WAIT_POLLS`,
including 20,000 spins per poll, without checking elapsed ticks. Alternatively, stall
WRITE's bulk OUT and stop completing CLEAR_FEATURE on EP0; that recovery wait also
has no tick deadline. A finite poll backstop is present, but it does not satisfy CP-80.

**Required correction:** carry an absolute tick deadline through the command's transfers,
sense and recovery, with an explicit bounded retry policy. Keep the poll backstop, and
preserve the old read-only `command` behavior. Exercise both slow-success schedules and
failed-command/recovery waits with a clock-controlled test.

### F2 — P2: DATA PROTECT on SYNCHRONIZE CACHE does not latch write protection

`write_action` recognizes sense key 7 (`kernel/src/drivers/usb/msc_logic.rs:617`),
but `sync_once` handles only `Unsupported`; every other sense becomes `Failed`
(`kernel/src/drivers/usb/msc.rs:592`). The protection latch is set only in the WRITE
handler (`kernel/src/drivers/usb/msc.rs:533`).

**Concrete input:** start with `protect=PROTECT_NO`, run SYNC, return a valid status-1
CSW with residue 0, then fixed-format REQUEST SENSE bytes
`70 00 07 00 00 00 00 0a 00 00 00 00 27 00 00 00 00 00`.
`sync_once` returns `Failed` and leaves `protect=PROTECT_NO`; `flush` retries and ends
with `Io` (`kernel/src/drivers/usb/msc.rs:777`, `kernel/src/drivers/usb/msc.rs:798`).
`write_protected()` still says `Some(false)`, and a subsequent WRITE passes its
protection check (`kernel/src/drivers/usb/msc.rs:725`). An extracted, unchanged
`sync_once` with mocked command/sense replies reproduced the unchanged latch.

**Required correction:** latch DATA PROTECT for the write path's failed SYNC as well as
failed WRITE, and test the resulting observable protection state. ILLEGAL REQUEST's
flush-disable behavior already works.

### F3 — P2: two trap-bank discriminators claimed by the PR do not exist

The PR body's `INITIAL_READ10`/first-command row promises a `sense=6/…` line. On the
first UNIT ATTENTION, the WRITE handler only increments `attentions`
(`kernel/src/drivers/usb/msc.rs:528`); it prints sense only if it ultimately gives up
(`kernel/src/drivers/usb/msc.rs:547`).

**Concrete input:** WRITE returns status 1; REQUEST SENSE returns key 6, ASC/ASCQ 28/00;
the retried WRITE succeeds. The call returns `Ok`, sends two WRITEs, and prints no
diagnostic line. An extracted, unchanged `write_chunk` with these replies and a log
collector reproduced exactly that result. The row's advertised discriminator is absent.

The `MAX_SECTORS_64`/`240` row promises a deadline line naming the chunk size. On a
128-block WRITE data timeout, `wait_until` merely returns `None`
(`kernel/src/drivers/usb/msc.rs:193`), and `classify` collapses it into `Xfer::Error`
(`kernel/src/drivers/usb/msc.rs:251`). The driver can print a generic reset line with
`blocks=128` and `reason=Data` (`kernel/src/drivers/usb/msc.rs:566`), but no deadline
line or elapsed-time discriminator distinguishes timeout from another data error.

**Required correction:** make those diagnostics real, or amend the table to name an
existing, accurate discriminator and explicitly state its limits. Keep the 64-block
fallback described as a later/configured cap, since R2 always selects its normal cap
from the bounce size (`kernel/src/drivers/usb/msc.rs:738`).

## Dispatch checklist

### 1. Allowlist

- **PASS — 1.1:** WRITE is Out, SYNC is None, MODE SENSE is In, with exactly nine pairs:
  `kernel/src/drivers/usb/msc_logic.rs:494`.
- **PASS — 1.2:** the opcode/direction check returns before `bulk_out`:
  `kernel/src/drivers/usb/msc_logic.rs:640`, `kernel/src/drivers/usb/msc_logic.rs:652`.
  Wrong-direction/empty-data/unknown-opcode KAT: `tools/usb-msc-logic-tests/src/write_tests.rs:298`.
- **PASS — 1.3:** the entire 15,756-byte baseline `msc_logic.rs` is an identical prefix
  at this head, including `ALLOWED_OPCODES` (`kernel/src/drivers/usb/msc_logic.rs:134`)
  and `command` (`kernel/src/drivers/usb/msc_logic.rs:379`). The feature-off driver
  still calls it (`kernel/src/drivers/usb/msc.rs:281`). Table and READ-CBW KATs:
  `tools/usb-msc-logic-tests/src/write_tests.rs:275`,
  `tools/usb-msc-logic-tests/src/write_tests.rs:251`.

### 2. CBW, CDB and CSW bytes

- **PASS — 2.1:** `command_dir` passes `dir == Dir::In` to the encoder
  (`kernel/src/drivers/usb/msc_logic.rs:646`); CBW byte 12 is set only for a nonempty
  In stage (`kernel/src/drivers/usb/msc_logic.rs:69`).
- **PASS — 2.2:** WRITE starts with zeroed bytes and fills only opcode, LBA and count;
  FUA stays clear (`kernel/src/drivers/usb/msc_logic.rs:521`).
- **PASS — 2.3:** residue above requested length is invalid
  (`kernel/src/drivers/usb/msc_logic.rs:118`); a device claiming more than the known
  sent count triggers reset (`kernel/src/drivers/usb/msc_logic.rs:718`); a passed
  write with nonzero residue becomes `Failed` (`kernel/src/drivers/usb/msc_logic.rs:727`).
- **PASS — 2.4:** expected CBWs are independent literal arrays
  (`tools/usb-msc-logic-tests/src/write_tests.rs:96`,
  `tools/usb-msc-logic-tests/src/write_tests.rs:113`,
  `tools/usb-msc-logic-tests/src/write_tests.rs:126`), CDB expectations are literals
  (`tools/usb-msc-logic-tests/src/write_tests.rs:186`), and CSW layout is independently
  assembled (`tools/usb-msc-logic-tests/src/write_tests.rs:78`). The short-write and
  overclaim cases are tested at `tools/usb-msc-logic-tests/src/write_tests.rs:368`
  and `tools/usb-msc-logic-tests/src/write_tests.rs:382`.

### 3. Data-out stage

- **PASS — 3.1:** legal block payloads are at least 512 bytes, so the 31-byte-wrapper
  branch selects `small` only for the CBW; payload selects `bounce`. The copy precedes
  fencing and submit (`kernel/src/drivers/usb/msc.rs:325`,
  `kernel/src/drivers/usb/msc.rs:336`, `kernel/src/drivers/usb/msc.rs:343`). Bind
  allocates both buffers in the DMA arena (`kernel/src/drivers/usb/msc.rs:920`,
  `kernel/src/drivers/usb/msc.rs:923`); the address is obtained through the arena
  fence (`kernel/src/drivers/usb/msc.rs:136`). Caller memory is not submitted.
- **PASS — 3.2:** Release precedes `EndpointPipe::submit`
  (`kernel/src/drivers/usb/msc.rs:342`); TRB publication writes control last with
  fences (`kernel/src/drivers/usb/xhci.rs:558`), then rings the doorbell
  (`kernel/src/drivers/usb/xhci.rs:1955`).
- **PASS — 3.3:** stalled OUT clears OUT, then enters CSW receive
  (`kernel/src/drivers/usb/msc_logic.rs:672`, `kernel/src/drivers/usb/msc_logic.rs:684`).
  The independent recording-pipe KAT checks the exact order at
  `tools/usb-msc-logic-tests/src/write_tests.rs:339`.

### 4. The guard

- **PASS — 4.1:** unarmed probe returns `None` before forwarding
  (`kernel/src/drivers/block/mod.rs:611`); `writes_allowed` requires a claim
  (`kernel/src/drivers/block/mod.rs:458`). Bind performs the existing read-only
  discovery (`kernel/src/drivers/usb/msc.rs:969`, `kernel/src/drivers/usb/msc.rs:1026`).
  `sync_ok` begins false (`kernel/src/drivers/usb/msc.rs:438`), so an unprobed stick's
  flush sends nothing (`kernel/src/drivers/usb/msc.rs:770`).
- **PASS — 4.2:** both write entry points check the range before forwarding:
  `kernel/src/drivers/block/mod.rs:585`, `kernel/src/drivers/block/mod.rs:627`.
- **PASS — 4.3:** registration exposes the guard, not the raw disk
  (`kernel/src/drivers/block/mod.rs:767`, `kernel/src/drivers/block/mod.rs:781`).
  The private `MscDisk` is registered there (`kernel/src/drivers/usb/msc.rs:1085`);
  its new retained references are used for counters, not writes
  (`kernel/src/drivers/usb/msc.rs:459`). Partition writes map and forward to their
  existing guarded parent (`kernel/src/drivers/block/partition.rs:156`), and the new
  probe forwarding does not change that (`kernel/src/drivers/block/partition.rs:147`).

### 5. Waits and locks

- **FAIL — 5.1:** F1. The main bulk waits have tick checks and poll backstops
  (`kernel/src/drivers/usb/msc.rs:180`, `kernel/src/drivers/usb/msc.rs:181`), and the
  NOT READY pause has both (`kernel/src/drivers/usb/msc.rs:485`,
  `kernel/src/drivers/usb/msc.rs:490`). Sense, TUR and recovery are exceptions, and
  durations restart for each transfer. Concrete inputs are in F1.
- **PASS — 5.2:** the driver holds `transport` across command execution
  (`kernel/src/drivers/usb/msc.rs:737`, `kernel/src/drivers/usb/msc.rs:776`,
  `kernel/src/drivers/usb/msc.rs:815`). Registry is dropped before probing
  (`kernel/src/drivers/block/mod.rs:866`); xHCI's controller lock is scoped before
  its command wait (`kernel/src/drivers/usb/xhci.rs:1026`,
  `kernel/src/drivers/usb/xhci.rs:1040`). Event/route/completion locks are transient
  per poll (`kernel/src/drivers/usb/xhci.rs:866`, `kernel/src/drivers/usb/xhci.rs:1962`).
- **PASS — 5.3:** no governance module or named governance static is referenced by
  either MSC file. The wait calls xHCI's bounded event drain
  (`kernel/src/drivers/usb/xhci.rs:981`), whose locks cover event routing
  (`kernel/src/drivers/usb/xhci.rs:869`, `kernel/src/drivers/usb/xhci.rs:901`), with
  no AI-governance call. I-14a is preserved in the reviewed path.
- **FAIL — 5.4:** the CP table does not hold in full; **CP-80 fails by F1**.
  Individual rows:
  - **PASS CP-1:** arming precedes driver transport; no new reverse callback while
    holding transport (`kernel/src/drivers/block/mod.rs:612`,
    `kernel/src/drivers/usb/msc.rs:815`).
  - **PASS CP-2/3, lock scope:** the held lock is a `SwitchMutex`, which does not
    disable interrupts (`kernel/src/sync/switch_mutex.rs:180`); xHCI's IRQ locks
    are released between polls. The missing elapsed-time guarantees are F1.
  - **PASS CP-18:** transport serializes complete read/write/probe/flush commands
    (`kernel/src/drivers/usb/msc.rs:636`, `kernel/src/drivers/usb/msc.rs:737`,
    `kernel/src/drivers/usb/msc.rs:776`, `kernel/src/drivers/usb/msc.rs:815`).
  - **PASS CP-77/78, R2's data-out addition:** no new bus-master grant; data uses
    the existing fenced arena and is filled before publication
    (`kernel/src/drivers/usb/msc.rs:336`, `kernel/src/drivers/usb/msc.rs:342`).
  - **FAIL CP-80:** sense/recovery still count spins, and no absolute per-command
    deadline is propagated (`kernel/src/drivers/usb/msc.rs:303`,
    `kernel/src/drivers/usb/msc.rs:479`, `kernel/src/drivers/usb/xhci.rs:939`).
  - **PASS CP-82:** only the judgement CAS winner enters the blocking `Once`
    initializer; other callers yield with interrupts enabled
    (`kernel/src/drivers/block/mod.rs:423`, `kernel/src/drivers/block/mod.rs:437`).
    New probe entry drops registry before forcing that judgement
    (`kernel/src/drivers/block/mod.rs:866`).

### 6. The 32-bit edge and sense mapping

- **PASS — 6.1:** `next_write` checks both endpoints before returning a chunk
  (`kernel/src/drivers/usb/msc_logic.rs:574`, `kernel/src/drivers/usb/msc_logic.rs:577`);
  the driver also preflights the whole request before acquiring transport or building
  a CBW (`kernel/src/drivers/usb/msc.rs:717`). Edge KAT:
  `tools/usb-msc-logic-tests/src/write_tests.rs:453`.
- **PASS — 6.2:** READ explicitly rejects the full range before issuing commands or
  copying into caller buffers (`kernel/src/drivers/usb/msc.rs:625`). This also fixes
  the prior short-buffer copy/panic edge. KAT:
  `tools/usb-msc-logic-tests/src/write_tests.rs:484`.
- **FAIL — 6.3:** DATA PROTECT latches on WRITE
  (`kernel/src/drivers/usb/msc.rs:534`), but not SYNC (F2,
  `kernel/src/drivers/usb/msc.rs:592`). **PASS for the other half:** ILLEGAL REQUEST
  on SYNC maps to Unsupported and flush stores `sync_ok=false`
  (`kernel/src/drivers/usb/msc.rs:593`, `kernel/src/drivers/usb/msc.rs:781`).

### 7. The scenario oracle

- **PASS — 7.1:** the oracle selects the last reset with `rposition`, then scans only
  that suffix (`tools/xtask/src/main.rs:17890`, `tools/xtask/src/main.rs:17896`).
- **PASS — 7.2:** missing reset is an error (`tools/xtask/src/main.rs:17893`), tested
  independently of an empty trace (`tools/xtask/src/main.rs:27523`).
- **PASS — 7.3:** any parsed post-reset opcode outside the six read-only plus three
  write-class opcodes is refused (`tools/xtask/src/main.rs:17816`,
  `tools/xtask/src/main.rs:17910`). Post-reset MODE SENSE(10) negative control:
  `tools/xtask/src/main.rs:27520`.
- **PASS — 7.4:** unmarked counts must be 0/0/0
  (`tools/xtask/src/main.rs:17883`), traced counts equal marked driver counts
  (`tools/xtask/src/main.rs:17915`), and the kernel requires marked=3/2/1 and
  unmarked=0/0/0 (`kernel/src/test_harness.rs:17620`). The oracle also requires a
  READ, proving a nonempty command trace (`tools/xtask/src/main.rs:17907`).
- **PASS — 7.5, by initialization order:** there is no pre-reset kernel-command
  detector in the oracle; it deliberately discards the prefix. At this head no MSC
  command can precede the kernel reset: `msc::init` only registers a class driver
  (`kernel/src/drivers/usb/msc.rs:79`), then `usb::init` calls xHCI initialization
  (`kernel/src/drivers/usb/mod.rs:339`, `kernel/src/drivers/usb/mod.rs:344`).
  Bring-up writes HCRST and waits for reset completion
  (`kernel/src/drivers/usb/xhci.rs:670`), programs rings, publishes the controller,
  and only then scans ports (`kernel/src/drivers/usb/xhci.rs:755`,
  `kernel/src/drivers/usb/xhci.rs:773`). MSC bind/discovery follows that scan.
  The reviewed code has no later HCRST write; BOT reset recovery is an interface
  class request (`kernel/src/drivers/usb/msc.rs:389`), not another controller reset.
  Thus the last-reset window does not hide a kernel command in this scenario.
  A future controller reinitialization would require revisiting this assumption.

### 8. Prior art

- **FAIL — 8.1:** the PR body's trap bank is not accurate for every row (F3).
  Row-by-row audit of its twelve rows:

| PR trap-bank row | Result | Actual handling and discriminator at the reviewed head |
|---|---|---|
| Large MODE SENSE / NO_WP_DETECT | PASS | Four-byte header, unknown on failure, probe prints `wp=unknown`: `kernel/src/drivers/usb/msc_logic.rs:542`, `kernel/src/drivers/usb/msc.rs:829`, `kernel/src/drivers/usb/msc.rs:861`. |
| Lying write-protect | PASS for WRITE | DATA PROTECT latch and its explicit line: `kernel/src/drivers/usb/msc.rs:533`. SYNC's omission is separately F2. |
| ALWAYS_SYNC / WRITE_CACHE | PASS within R2 scope | Successful probe enables real SYNC on flush: `kernel/src/drivers/usb/msc.rs:760`, `kernel/src/drivers/usb/msc.rs:778`. The table explicitly assigns commit integration to R3 and the boot-counter durability discriminator to R5; neither is evidence from R2. |
| Unsupported SYNC / SENSE_AFTER_SYNC | PASS for failed CSW | Failed SYNC requests sense, ILLEGAL REQUEST disables flush with a line: `kernel/src/drivers/usb/msc.rs:592`, `kernel/src/drivers/usb/msc.rs:781`. This handles failed status; it does not claim to detect devices that falsely return passed status. |
| BROKEN_FUA | PASS | Zero flags and literal-byte KAT: `kernel/src/drivers/usb/msc_logic.rs:521`, `tools/usb-msc-logic-tests/src/write_tests.rs:186`. |
| Zero-length CSW | PASS | One extra read and KAT: `kernel/src/drivers/usb/msc_logic.rs:699`, `tools/usb-msc-logic-tests/src/write_tests.rs:420`. |
| IGNORE_RESIDUE | PASS | Strict write residue failure, with `took=` log: `kernel/src/drivers/usb/msc_logic.rs:727`, `kernel/src/drivers/usb/msc.rs:548`. |
| MAX_SECTORS_64 / 240 | FAIL | Normal cap is 128 blocks (`kernel/src/drivers/usb/msc.rs:738`); smaller helper caps work, but the advertised timeout/deadline discriminator is absent. F3 gives the concrete 128-block timeout. |
| INITIAL_READ10 / first command after reset | FAIL | WRITE retries UNIT ATTENTION once (`kernel/src/drivers/usb/msc.rs:528`), but a successful retry has no advertised `sense=6/…` line. Concrete reply sequence in F3. Existing bind readiness retries remain at `kernel/src/drivers/usb/msc.rs:996`. |
| BULK_IGNORE_TAG | PASS | Tag validation, phase recovery, reason log: `kernel/src/drivers/usb/msc_logic.rs:114`, `kernel/src/drivers/usb/msc_logic.rs:709`, `kernel/src/drivers/usb/msc.rs:567`. |
| Phase error / stalled bulk OUT | PASS for protocol behavior | OUT halt clear before CSW; class-reset/IN-clear/OUT-clear recovery and one WRITE recovery retry: `kernel/src/drivers/usb/msc_logic.rs:672`, `kernel/src/drivers/usb/msc_logic.rs:339`, `kernel/src/drivers/usb/msc.rs:574`. KAT at `tools/usb-msc-logic-tests/src/write_tests.rs:339`. Recovery timing is F1. |
| Stick pulled | PASS | `gone` ends polling, WRITE reports NoDevice with a line: `kernel/src/drivers/usb/msc.rs:183`, `kernel/src/drivers/usb/msc.rs:561`. |

## Validation and limits

- `cargo test -p usb-msc-logic-tests` at the reviewed head: **36 passed**.
- `cargo test -p xtask usb_msc_write -- --nocapture`: **1 passed**, 212 filtered out.
- Byte comparison with the main baseline: all original `msc_logic.rs` bytes identical;
  only the new write-path section is appended.
- Three targeted scratch host probes used unchanged function bodies extracted from
  the reviewed `msc.rs`, with mock hardware/clock/replies: `wait_until` accepted a
  27-second WRITE schedule and 58-second SYNC schedule; `sync_once` left protection
  unset on DATA PROTECT; `write_chunk` silently succeeded after UNIT ATTENTION.
  These are deterministic function-level reproductions, not physical-device tests.
- QEMU and xorriso are unavailable in this worker environment, so I did not rerun the
  four-shard QEMU gate. Run 43158521142's green status is dispatch-provided evidence.
- The PR body was read from GitHub's API and checked against the code; source at
  `2cf42002` controls all findings. This RETURN changes documentation only.
