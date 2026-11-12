<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Relay 411, Chad, #110 OT-100 R3 code review

Reviewed **`ac93965401c8ecb11120d294350f1f51649f297d`**, on
`claude/ot100-r3-area-mount`, against its merge-base with `origin/main`.
Return branch: `codex/pr110-review-chad`.
Dispatch: Section 411 of `Plans/DISPATCH-2026-11-12-relays-410-411.md`.
All code citations below refer to the reviewed head, not a later revision.

**Verdict: FAIL.** The non-write guard gates, USB mount decision, command-line
source and image checks pass. The loss latch does not prevent subsequent I/O
through every drain or the direct large-write path. CP-113 overstates what the
code enforces. There is also an explicit difference between the dispatch's
absolute boot-stick-only requirement and the design's installed-disk exception.

## Findings and concrete inputs

### F1 — P1: the loss latch does not serialize with drains; a superblock can be written after the flip

`kernel/src/fs/neofs/cache.rs:475` checks loss **before** taking `drain` at
`:479`. `drain_pass` does not receive or inspect the latch (`:1081`), and writes
blocks, including superblocks, at `:1141`. The worker's `drain_worker_cycle`
does not check loss at all (`:564-581`). `lose_by_user` drains and then flips
(`:758-760`); it neither stops the worker nor holds `drain` across the flip.

Concrete execution on a healthy, armed boot stick:

1. CPU A calls `create_regular("late")`, passes `inode.rs:363`, and is
   descheduled before taking its writeback pause.
2. CPU B calls `persist_switch_read_only`. Its final drain sees a clean cache,
   returns, and stores `Lost::User` at `cache.rs:766`.
3. A resumes, caches its new inode, directory, map and superblocks. Cache writes
   have no loss check (`cache.rs:315-333`); publishing queues superblock copies
   (`inode.rs:1772`, `:1789`). Dropping the pause can wake the worker
   (`cache.rs:693-703`).
4. The still-running worker drains those blocks and superblocks after the flip.
   Its one-step cycle limit (`cache.rs:36`) does not stop subsequent cycles.

There is a separate foreground execution after a **failure** flip: a worker is
inside a failing drain; a foreground `drain_all` passes `:475` and waits at
`:479`; the worker releases `drain`, sets loss at `:729`, and exits; the
foreground thread then acquires `drain` and retries dirty blocks without
rechecking. If the device answers again, the pass can reach a superblock.
An already-entered foreground pass is unlimited (`max_blocks=None`, `:484`).

Repair needs a shared I/O/flip serialization rule, with the loss decision
checked while holding it, covering the worker, foreground and shutdown drains.
The user switch also needs to exclude or settle admitted mutations before its
final drain and flip. Acquire/Release publication alone cannot provide this.
Test with a healthy device and controlled scheduling; physical removal hides
successful writes that the software still attempts.

### F2 — P1: an admitted large-file mutation sends new writes after the flip

With `usb-persistence,neofs-large-write`, `create_file_from_bytes` checks
`is_writable` once at `kernel/src/fs/neofs/inode.rs:440`. Its `write_large_run`
writes directly to the device at `:595` and `:607`, then flushes at `:610`.
Neither that loop nor `write_fs_blocks` (`:2793-2822`) checks loss. This bypasses
the cache drain's new check entirely.

Concrete input: a healthy armed stick, and a 128 KiB file. Suspend the creator
immediately after `:440`, run the user read-only switch to completion, then
resume the creator. It can issue both 64 KiB data writes, pointer writes if
needed, and the flush while `is_writable()` is already false. These are new
submissions after the flip, not merely completion of a command already on USB.
The same absence of cancellation permits remaining chunks after a failure flip
if an admitted operation and the worker overlap.

The bound is the admitted file's remaining data and pointer blocks, available
volume space and file-size limit, with `LARGE_WRITE_RUN=64 KiB` (`:2740`). MSC
adds geometry, guard-extent, gone and write-protect checks (`msc.rs:834-859`)
and per-command retries/deadlines (`msc_logic.rs:744-782`). None consults the
mount's loss latch. There is no bound of "one already-issued command" or
"zero bytes after the flip". Include this direct path in the serialization
and deterministic race test from F1.

### F3 — P2: a fatal foreground error need not flip the mount

`drain_all` propagates `drain_pass` errors at `cache.rs:486` without calling
`lose_to_failure`. That call is only on the worker's error path (`:726-730`).
`PartitionDevice::note` records the failure (`partition.rs:99-112`), but a
recorded failure is not itself the loss latch. Direct large writes likewise
propagate an I/O failure without setting loss (`inode.rs:595-599`).

Concrete input: a mounted RW stick whose foreground fsync receives an exhausted
driver `Io` error on a data write; subsequent device operations succeed. Fsync
returns an error, but loss stays zero. The worker can retry the dirty block
successfully, so it never takes its error branch and later mutations remain
writable. THINK §4.2 classifies an error surviving driver retries as fatal
(`persist_logic.rs:278-287`). Latch that failure in the common I/O/error path,
including foreground/direct writers, and test an error followed by recovery.

### F4 — dispatch scope mismatch: non-USB volumes are explicitly exempted

The literal Section 411 item 2 requirement (no write/flush/query to any device
other than the identified boot stick) is **not enforced**. `rw_decision`
immediately accepts every non-USB volume (`persist_logic.rs:155-158`). The
mount considers other devices (`fs/mod.rs:1282-1288`), mounts an accepted
partition RW at `:1349`, and has a non-USB whole-device RW fallback at `:1392`.

Concrete input: boot USB A without a usable area, plus internal NVMe B with a
valid, sealed, guard-armed LogansOS NeoFS partition and durable flush support.
B is selected and mounted RW even with `persist=off`; a create and sync writes
and flushes B. The host KAT explicitly accepts the non-USB bypass
(`tools/persist-logic-tests/src/lib.rs:206-218`). I also ran the actual pure
decision on non-boot NVMe facts with `Persist::Off`, mismatched arming and
`sync_ok=false`: it returns `Ok(())`.

This is **consistent with the approved THINK §4.1 item 3**
(`docs/THINK-2026-11-10-ot100-usb-persistence.md:314-317`), which preserves
installed internal LogansOS volumes. It is not a new write-guard bypass and
does not show a write to the Windows-shaped Zenbook fixture. Nevertheless the
dispatch's unrestricted assertion cannot pass. Preserve the Windows-only
qualification in the safety claim, or enforce boot-stick-only selection for
this boot mode if the absolute dispatch rule is intended.

## Section 411 checklist

### 1. Non-write guard operations

- **PASS — three guarded paths.** `gate_non_write` calls the closure only when
  allowed (`kernel/src/drivers/block/write_guard_logic.rs:632`). The answer is
  installer-claimed or guard-armed (`block/mod.rs:475-482`). `submit(Flush)`,
  `flush` and `write_protected` use it at `:610`, `:630`, `:642` respectively.
- **PASS — zero calls unarmed.** The mock counts all three operations:
  `tools/block-guard-logic-tests/src/gate_tests.rs:47-62`; armed calls each once
  (`:66-69`). These test the shared gate; inspection above establishes the
  actual wrapper wiring.
- **PASS — feature off retains R2 behavior.** Each added wrapper branch is
  `cfg(usb-persistence)`; old forwarding expressions remain under its inverse
  (`block/mod.rs:609`, `:635-646`). The added helper is also feature-gated.
  Source comparison finds no changed R2 guard behavior with this feature off.

### 2. Never internal

- **PASS for unchanged arming; FAIL for an unrestricted boot-only selection
  claim.** The R3 diff to `write_guard_logic.rs` only appends `gate_non_write`;
  `judge_with_partitions` is unchanged. The USB RW path requires exactly one
  NeoFS claim equal to the partition (`fs/mod.rs:1318-1322`, `:1336-1343`).
  Non-USB selection has the explicit exception described in F4.
- **FAIL as written — no other device can receive write/flush/query.** F4 is
  the concrete non-USB write/flush counterexample. The **USB probe** itself
  correctly requires identified boot device, On, and exact arming
  (`fs/mod.rs:1336-1340`); no non-boot device gets that probe.
- **PASS — Windows-shaped NVMe scenario.** The fixture contains ESP, MSR,
  Microsoft basic-data and recovery GPT types, both GPT copies and sealed
  superblocks planted at the data start and absolute 64 MiB
  (`tools/xtask/src/persist_image.rs:398-472`). The scenario takes its planted
  bytes from the real formatted area and checks no host guard claims
  (`persist_scenarios.rs:244-250`). Its oracle requires an NVMe census with
  `armed=none table=gpt`, a refused NVMe raw write, and the unchanged entire
  NVMe hash (`:366-395`). This passes that Windows-shaped case, not F4's case.

### 3. Read-write rule and identification

- **PASS for the specified USB rule.** Order is boot unidentified, wrong boot
  device, Off, arming mismatch, WP, no durable flush
  (`kernel/src/persist_logic.rs:159-176`). Each alone is tested from otherwise
  good facts (`tools/persist-logic-tests/src/lib.rs:148-191`); precedence is
  additionally tested at `:193-200`. Non-USB exemption is F4.
- **PASS — zero/duplicate GUID never identifies.** Nonzero filter at
  `persist_logic.rs:80`; exactly-one match at `:94-99`. KATs cover zero, absent,
  no match and duplicate (`tools/persist-logic-tests/src/lib.rs:85-118`).
- **PASS — exact, unique `persist=on`.** Byte tokenization, duplicate refusal
  and exact value comparison (`persist_logic.rs:31-50`); KATs at
  `tools/persist-logic-tests/src/lib.rs:20-61` cover defaults, malformed values,
  duplicates and invalid UTF-8 elsewhere.

### 4. Command-line source

- **PASS — file offset 32 and byte KAT.**
  `kernel/src/limine_file_logic.rs:37-45` reads the LE pointer at 32;
  `tools/limine-file-logic-tests/src/lib.rs:71-76` plants hand-laid pointer
  bytes. Independently read the pinned upstream
  [Limine v8.6.0 header](https://raw.githubusercontent.com/limine-bootloader/limine/v8.6.0/limine.h).
  Its sequence of four 8-byte fields before `cmdline` puts that field at 32.
  Compiled C static assertions against that header passed for `cmdline=32`,
  `gpt_disk_uuid=64` and `sizeof(limine_file)=112`.
- **PASS — bounded bytes, no UTF-8 trust.** Request then file fallback at
  `kernel/src/boot_medium.rs:91-119`; `limine_string` reads bytewise only for
  offsets 0..4095 and requires a NUL (`:67-82`). It returns a byte slice.
  Parser bound is `persist_logic.rs:23`; boundary KAT at
  `tools/persist-logic-tests/src/lib.rs:65-73`. This relies on the normal
  trusted Limine pointer/mapping contract; it is not arbitrary-pointer validation.

### 5. Read-only flip / CP-113

- **PASS — once-only publication and Acquire readers.** Initial zero at
  `kernel/src/fs/neofs/cache.rs:209`; sole transition is CAS from zero with
  `AcqRel` success (includes Release), Acquire failure at `:766`; the read is
  Acquire at `:741`. There is no reset. `is_writable` consults it at
  `inode.rs:231-236`.
- **PASS — all 13 old conditional field checks replaced.** The R3 diff replaces
  the 13 `if !self.writable` tests with `is_writable`, including create
  (`inode.rs:363`), large create (`:440`), write (`:733`), truncate (`:812`),
  replace (`:915`) and fsync (`:993`). Remaining raw field reads describe the
  original mount mode or initiate the switch (`:236`, `:248`, `:275`);
  they do not admit ordinary mutations. This passes the entry-check audit;
  it does not close F1/F2's check-to-I/O window.
- **FAIL — no block/superblock after flip, including drains.** F1 and F2 give
  concrete schedules and enforcing omissions. F3 also leaves some fatal
  failures unlatched. A new `drain_all` invocation after loss refuses at
  `cache.rs:475`; an already-admitted drain and the worker are not covered.
- **PASS for calls starting after the flip — fsync semantics.**
  `inode.rs:993-1001` returns `ReadOnly` when loss is present and Ok when the
  mount was always RO with no loss. An admitted fsync's drain race is F1.
- **FAIL — the check-to-write window is not restricted to cache staging.**
  It spans admission, waits/locks, allocation and direct block submission
  (`inode.rs:440-595`). Yes, a write admitted before the flip still reaches
  the device; F2 describes its remaining-file/chunk bounds. F1 additionally
  allows a post-flip superblock. The guard still bounds device extents, but
  does not enforce mount writability.

### 6. Image

- **PASS — independent CRC and GPT reader.** Own bitwise IEEE CRC-32
  (`tools/xtask/src/persist_image.rs:40-52`), header decoder (`:84-111`) and
  array decoder (`:114-129`). `gpt_logic` is used separately for the guard KAT
  adapter (`:267`), not this oracle. Standard CRC check-value KAT at `:490`.
- **PASS — listed image invariants.** Both header CRCs/LBA pairs/GUID and
  usable range (`:141-159`), both array CRCs and equality (`:162-164`), exactly
  one type/zero attributes/alignment/no overlap (`:180-213`), hand-laid
  superblock fields and recomputed seal/fit (`:215-239`). APM exclusion at
  `:241-255`. Guard arms exactly the span as built and repaired (`:327-345`).
  The build invokes both checks (`tools/xtask/src/main.rs:11443-11444`).
- **PASS for arguments/source; byte identity not independently rerun.** Without
  an area, the old xorriso list gains only file-date pinning
  (`main.rs:11557-11601`), with epoch supplied in the environment (`:11420`)
  and sourced deterministically (`:11532-11548`). The full list is pinned
  by `persist_image.rs:534-584`. No other no-area builder change found.
  This environment has no xorriso, so I do not claim a local ISO hash comparison.

### 7. No network secrets

- **PASS — KAT covers every current settings key.** Both KATs iterate `KEYS`
  (`userland/settings-core/src/tests.rs:400-415`,
  `src/setup_keys_tests.rs:185-200`). The lists contain all five ordinary or
  seven setup keys (`src/lib.rs:96-113`); exhaustive `kind` and `valid` matches
  at `:146-169` restrict them to current numeric choices/flags/minutes.
  Ran both default and all-feature configurations. This is evidence for the
  current setup/settings schema, not a claim that arbitrary files cannot
  encode secrets. The KAT's finite scan alone would not prove all future u32
  values are bounded; the current `valid` implementation does.

### 8. Prior art, trap bank and concurrency bank

- **FAIL overall — CP-113 and the loss trap overclaim; other examined rows pass.**
  The PR's F11 handling and CP-113 say no write/superblock after loss. F1/F2
  contradict that; the same overclaim is in
  `docs/concurrency-pitfalls.md:154`, `:4878-4882`. Concrete inputs are F1's
  admitted cached mutation/foreground drain and F2's 128 KiB direct create.
  The unplug harness checks a later create/sync (`test_harness.rs:17793-17798`),
  not successful I/O across the flip or a healthy-device user switch. The
  classifier KAT (`tools/persist-logic-tests/src/lib.rs:224`) tests reason
  mapping, not races. Add controlled write-count/superblock-count tests.
  CP-1's registry/arming lock release is supported by `block/mod.rs:755-759`;
  the user-switch path clones `PERSIST_MOUNT` before I/O (`fs/mod.rs:1170`).
  CP-82's census is forced before selection and worker creation
  (`fs/mod.rs:1249`, `:1349`; `inode.rs:199`). Other trap discriminators are
  present: repaired-image KAT (`persist_image.rs:332-334`), duplicate sticks
  (`persist_scenarios.rs:349-364`), corrupt area (`:221-225`, `:328-331`),
  WP/durable-flush refusals (`persist_logic.rs:172-176`) and command-line
  source fallback (`boot_medium.rs:106-117`). Prior-art statement is accurate
  as a scope statement: THINK §7 still supplies the design/traps; this diff
  adds no vendored third-party driver code. Internal-disk claims need F4's
  qualification rather than an unconditional guarantee.
- **PASS — I-14 section, subject to the functional loss failures above.**
  No new IRQ handler or governance lock import in these changes. Census and
  mount are boot-thread work; the loss operation is CAS, dirty-count under
  the cache lock, and printing (`cache.rs:763-780`). Existing USB transport
  serialization and fenced bounce transfers remain (`msc.rs:867-888`). R2's
  write/flush retry deadlines remain (`msc_logic.rs:744-782`); there is no new
  DMA route. This does not establish a whole-operation 30-second bound:
  R2 exposes conservative per-chunk and flush bounds at `:774` and `:781`.

## Verification and limits

- Ran `cargo test -p persist-logic-tests -p block-guard-logic-tests -p
  limine-file-logic-tests -p settings-core -p xtask`: **70 + 5 + 11 + 13 + 218
  passed**, zero failures; one xtask test ignored.
- Ran `cargo test -p settings-core --all-features`: **6 passed**.
- Compiled the independent C layout assertions against the pinned Limine
  header and ran the production `rw_decision` non-USB counterexample.
- Reviewed the PR body and the approved design, including the installed-volume
  exception. Did not rerun the reported four-shard CI or QEMU scenarios;
  QEMU and xorriso are unavailable here. F1/F2/F3 are source-derived execution
  counterexamples, not claimed local kernel reproductions. The supplied green
  run does not exercise those controlled schedules.
- No kernel or test implementation changes in this RETURN.
