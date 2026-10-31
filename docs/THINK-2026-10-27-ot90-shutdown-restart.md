<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: OT-90, clean shutdown and restart

**Lane:** S5 (Claude), design only. Dispatch: `Plans/DISPATCH-2026-10-27-ot90-power-think-s5.md`. MVP tier:
`Plans/DECISION-2026-10-27-release-tiers-mvp.md`. Base: `main` `49087f1c`. A cross-family read of this THINK comes
before the first build rung.

**Revision 2 (2026-10-28), after Janus's review** (`Plans/RETURN-2026-10-27-ot90-power-think-review-janus.md`, FAIL on
items 2, 4 and 8):
- §3.2 now proves the accepted `Name` is exactly the root `\_S5`, and fails closed on a `Method (\_S5)` and on an `_S5`
  in any other scope (item 2).
- K2's durability rests on the NeoFS commit barrier (#180). A failed or timed-out flush is reported as an
  unclean shutdown, and the freeze-then-drain order is spelled out (item 4).
- §7 dates are UTC; Redox's is corrected (item 8).

**Revision 3 (2026-10-28), after Janus's re-check** (`Plans/RETURN-2026-10-28-ot90-think-recheck-janus.md`, FAIL on
item 2 only): revision 2's byte scan could accept `_S5`-shaped bytes inside a `Buffer` payload. §3.2 now locates the
`Name` by a parse-only namespace walk of the definition blocks. The planned KATs gain the Buffer-payload and
SSDT-root cases.

**Revision 4 (2026-10-29), after Janus's second re-check** (`Plans/RETURN-2026-10-29-ot90-think-recheck2-janus.md`,
FAIL on items 1 and 3):
- The prototype walks every table of every set to its end, instead of returning at the first `\_S5`, and prints a
  per-table result with the set name and the totals. That output is Appendix A (item 3).
- A forward call can no longer misalign the walk silently. A fixed-point pre-pass collects every declaration's
  arity first, and a table-level name that resolves to nothing fails the decode closed (item 1). Conditional bodies
  are now walked rather than skipped, because the Zenbook PEP SSDT declares `ADBG` inside one and calls it at table
  level: revision 3 was guessing that arity on a real table.
- The planned KATs gain the forward-call, unresolved-name and later-SSDT-parse cases.

**Revision 5 (2026-10-29), after Janus's third re-check** (`Plans/RETURN-2026-10-29-ot90-think-recheck3-janus.md`,
one narrow FAIL): a definition inside an `If`, `Else` or `While` body leaked into the unconditional declaration set,
so a name defined only under `If (Zero)` resolved for a table-level use. Such a definition is now conditional,
bound to its body: a use outside that body fails closed, and its arity never feeds an unconditional call. `External`
is still collected from conditional bodies as unconditional. No corpus table changes result (Appendix A is
unchanged, 93/93 and 10/10).

**Revision 6 (2026-10-29), with P1 (#188):** the THINK now matches the P1 code.
- §3.1 step 7: the 10 s retry repeats the full second write. ACPICA's SLP_EN-only retry clears SCI_EN and writes
  SLP_TYP 0.
- §3.1 hardware-reduced: `SLEEP_STATUS_REG` is required as well as `SLEEP_CONTROL_REG`, as in ACPICA.
- §3.1 step 6: PM1b's base value is its own read, and a GAS width of 0 takes the FADT length byte.
- §3.2: the decode runs once at ACPI init and the S5 path uses only the cached plan; the measured decode cost.

**The short answer.** Every machine we own powers off through the legacy PM1a control register. None is
hardware-reduced, the Zenbook included. Every `\_S5` in the corpus is a constant `Name (_S5, Package {…})`, so the
MVP can decode it from the DSDT bytes without running AML in the kernel. Restart uses the FADT reset register.
Three of the seven real sets don't use 0xCF9 there: they write an SMI command to port 0xB2. The fallbacks are the
keyboard controller, then 0xCF9, then a triple fault. "Clean" means:

1. userland is asked to save, waited for, then stopped;
2. NeoFS is drained and its disk flushed;
3. the APs are stopped;
4. DMA and the IOMMU are quiesced.

Each step has a bound. The one thing the MVP leaves out is `_PTS(5)`. On every real machine it calls SMI and EC
code, so it needs the mediated region seam and EC support first (OT-86 L6, L9). It comes after the MVP, and a
bare-metal check decides whether its absence matters.

## 1. What exists today

| Piece | State | Where |
|---|---|---|
| Power-off, reboot, S5, reset register | **none** anywhere in kernel or userland | the FADT is read only for timers, the RTC and i8042 flags: `kernel/src/acpi/mod.rs:289`, `:312-318`; `acpi/rtc_fadt.rs` |
| End of a session | neo-init exits → splash → `arch::halt_forever()` (`cli; hlt`) | `kernel/src/main.rs:957-983`; `arch/x86_64/mod.rs:108` |
| AML in the kernel | **none**. The interpreter (`third_party/acpi-loganfy`) is a dependency of the host tool `tools/acpi-aml-grade` only | `Cargo.toml:60-63` |
| DSDT bytes in the kernel | `acpi::dsdt()`, checksum-checked, under `i2c-hid-touchpad` | `kernel/src/acpi/mod.rs:325-357` |
| NeoFS durability | copy-on-write, three superblock copies (0, 1 MiB, 64 MiB), the highest valid generation wins at mount. No journal, **no clean/dirty flag**: superblock bytes 13..16 must be zero | `fs/neofs/format.rs:26`, `:131-133`; `fs/neofs/mount.rs:236-318` |
| NeoFS write-back | write-behind cache plus a `neofs-writeback` thread. `fsync` drains the whole mount and flushes the device | `fs/neofs/cache.rs:99-112`, `:301-318`, `:421-439`; `fs/neofs/inode.rs:352-364` |
| A global sync | **none**, and no `sync` syscall. `SYS_FSYNC = 15`, `SYS_UMOUNT = 18` exist | `arch/x86_64/syscall.rs:107`, `:110` |
| Unmount | drains only when the last `Arc<NeoFsState>` drops, and the drain's result is discarded (`let _ =`). Returns `EBUSY` while handles are live | `fs/mod.rs:615-625`; `fs/neofs/inode.rs:1070-1076` |
| Device flush | virtio-blk `VIRTIO_BLK_T_FLUSH`, NVMe Flush `0x00`, AHCI `FLUSH CACHE EXT 0xEA` | `drivers/block/virtio_blk.rs:644-650`; `nvme.rs:410-419`; `ahci.rs:332-339` |
| Write guard | `GuardedDevice` judges each disk once. `flush()` passes through **unguarded** | `drivers/block/mod.rs:102-118`, `:379-385`; `writes_allowed` `:402-407` |
| Stopping other CPUs | **none**. Only the reschedule (0xfd) and shootdown (0xfc) IPIs exist. Panic halts only the panicking CPU | `arch/x86_64/apic.rs:27-28`; `panic.rs:28-82` |
| Device quiesce | **no public stop on any driver**. The resets exist but are private and run at probe only (virtio `reset_device`, NVMe `disable_controller`, AHCI `stop_port`, RTL8125 `reset_chip`, xHCI halt) | e.g. `drivers/block/nvme.rs:644`, `virtio_blk.rs:1488` |
| IOMMU | `iommu::disable()` clears `GCMD_TE` on every unit; only tests call it | `drivers/iommu/mod.rs:704` |
| Killing processes | `process::terminate(pid)`. There is no kill-all | `sched/process.rs:3025`; `user_process_count` `:2348` |
| A "deadline, then force" primitive | **none** in the product image (`wait-multiplex` isn't built). `TICKS` at 100 Hz is | `arch/x86_64/timer.rs:47` |
| QEMU test exit | isa-debug-exit `0xF4`, host exit `(code << 1) \| 1`. Tests run `-no-reboot`, where a reset or triple fault also exits QEMU with 0. `run_qemu` adds `-no-shutdown`, so S5 there pauses QEMU | `kernel/src/test_harness.rs:100-102`; `tools/xtask/src/main.rs:8787`, `:8791`, `:15631`, `:15907-15908` |
| A privileged, non-delegable capability | `InputSystem`: `USE` + `SYSTEM`, no `DERIVE`/`GRANT`, minted only into neo-init | `sched/process.rs:335-340`, `:1132`; `kernel/src/main.rs:415-420` |
| A power menu | **none**. The craft bar holds a status light and a label; its right end is unused | `userland/neo-atrium/src/layout.rs:72`, `:229-265`; `scene.rs:821-833` |

## 2. The corpus: what every machine asks for

All ten sets in `fixtures/acpi/`, read for this THINK. The FADT fields come from the raw tables at the offsets in
ACPICA `actbl.h:228-287`. `\_S5` and `_PTS(5)` were evaluated by the patched interpreter (`acpi+fix1`) through the
grade's recording handler, and cross-checked with the `iasl -d` disassembly.

| Set | HW-reduced | LOW_POWER_S0 | PM1a_CNT | PM1b | `\_S5` SLP_TYP a/b | Reset register | Reset value | PWR_BUTTON as a control method | SMI_CMD / ACPI_ENABLE |
|---|---|---|---|---|---|---|---|---|---|
| QEMU q35 (all three) | no | no | IO 0x604 | none | 0 / 0 | IO 0xCF9 | 0x0F | no | 0xB2 / 0x02 |
| Zenbook S14 | **no** | **yes** | IO 0x1804 | none | 7 / 0 | IO 0xCF9 | 0x06 | **yes** | 0xB2 / 0xA0 |
| OptiPlex 7080 (both) | no | no | IO 0x1804 | none | 7 / 0 | **IO 0xB2** | **0x73** | no | 0xB2 / 0xD1 |
| HP 800 G6 | no | no | IO 0x1804 | none | 7 / 0 | IO 0xCF9 | 0x06 | no | 0xB2 / 0xF0 |
| M910q | no | no | IO 0x1804 | none | 7 / 0 | IO 0xCF9 | 0x06 | no | 0xB2 / 0xA0 |
| M75q (AMD) | no | no | IO 0x804 | none | 5 / 0 | **IO 0xB2** | **0xBE** | no | 0xB2 / 0xA0 |
| MSI MS-7E57 (AMD) | no | no | IO 0x804 | none | 5 / 0 | **IO 0xB2** | **0xBE** | no | 0xB2 / 0xA0 |

What this settles:

- **No hardware-reduced path is needed for any machine we own.** The Zenbook's flags are `0x20c4f5`: bit 20 (HW_REDUCED) is clear
  and bit 21 (LOW_POWER_S0) is set. The `SLEEP_CONTROL_REG` path (ACPI 5.0+) is still designed, so a future
  hardware-reduced machine fails closed rather than doing something wrong (§3.1).
- **SLP_TYP differs per machine (0, 5, 7)**, so it can't be a constant; it must come from `\_S5`.
- **`\_S5` is a constant `Name` on every set.** It occurs exactly once across each set's DSDT and SSDTs, always
  preceded by `NameOp` and followed by `PackageOp`, at DSDT root scope. QEMU's sits inside `Scope (\)`. Each has four
  elements in `ZeroOp`/`BytePrefix` form (e.g. Zenbook DSDT `0x9a47`: `12 07 04 0a 07 00 00 00`).
- **Three of seven real sets reset through the SMI port, not 0xCF9.** The OptiPlex writes `0x73` to `0xB2`; both AMD boxes
  write `0xBE`. A reset path that assumes 0xCF9 is wrong for the OptiPlex, our main bare-metal box.
- **`_PTS(5)` exists on every real machine and touches firmware.** Under the grade's zero model it makes 17 to 32 writes,
  including SMI port `0xB2` on all seven real sets, GPE/SMI-enable I/O (`0x1830`, `0x1834`) and NVS SystemMemory. On the
  Zenbook it calls `\_SB.PC00.LPCB.EC0.EC0S` (the embedded controller) and `\_SB.TPM.TPTS` (Zenbook DSDT
  `_PTS`, disassembly). The Zenbook run also writes physical `0x0`-`0xC`. That is a zero-model artifact (a base
  read from NVS came back 0), but it is exactly the class of address the region seam must range-check.
- **The power button.** Every real set except the MSI has a PNP0C0C `\_SB.PWRB` (`_HID` = `0x0C0CD041`); the QEMU sets have none. Only the Zenbook marks the
  button as a control-method device in the FADT, so its button events arrive as an AML `Notify`, not a fixed event.

## 3. Power-off (ACPI S5)

### 3.1 The sequence

This follows ACPICA `AcpiEnterSleepStatePrep` (`hwxfsleep.c:312`) and `AcpiHwLegacySleep` (`hwsleep.c:37-167`);
Linux does the same (`drivers/acpi/sleep.c:1084-1098`, read for principle). Each item is marked **MVP** or
**later**.

1. **SLP_TYP.** MVP: the static `\_S5` decode (§3.2). Later: evaluate `\_S5` through the in-kernel interpreter, and
   keep the static decode as a cross-check that must agree.
2. **`_PTS(5)`, then `_SST(0)`.** Later only (§3.3). The MVP skips both and logs that it did.
3. **Ensure ACPI mode.** Read `PM1a_CNT.SCI_EN` (bit 0, ACPICA `aclocal.h:1229`). If it is clear and `SMI_CMD` and
   `ACPI_ENABLE` are non-zero, write `ACPI_ENABLE` to `SMI_CMD` and poll `SCI_EN` for up to 3 s (ACPICA
   `AcpiHwSetMode`, `hwacpi.c:36`, poll at `:117-127`). If it never sets, log it and continue to step 6: some
   firmware traps the PM1 write in SMM anyway.
4. **Clear wake status.** Write 1 to `PM1_STS.WAK_STS` (bit 15). Also clear every PM1 status bit and every GPE0
   status bit (write-1-to-clear), as `AcpiHwClearAcpiStatus` does (`hwregs.c:422`).
5. **Disable all GPEs.** Write 0 to every byte of the GPE0 enable half (`GPE0_BLK + GPE0_BLK_LEN/2`), as
   `AcpiHwDisableAllGpes` does (`hwgpe.c:565`). For S5, OSPM may leave wake GPEs armed (FADT flag, rust-osdev
   `fadt.rs:447`). The MVP arms none, so nothing wakes the machine on its own.
6. **Two writes, as ACPICA does:**
   - write `PM1a_CNT` with the read value, SLP_TYP and SLP_EN cleared, then SLP_TYPa at bits 10-12;
   - write it again with SLP_EN (bit 13) set.

   Each write keeps the register's other bits (SCI_EN among them). If `PM1b_CNT` exists, give it SLP_TYP**b** and
   write it too. Never write SLP_TYPa to both registers: the rust-osdev `set_sleep_typ`
   (`third_party/acpi-loganfy/src/registers.rs:163-174`) does, and that is a trap (§9). No set has a PM1b today.
   - **PM1b's base value is read from PM1b itself** (P1 `pm1_sleep_values` on each register's own read). ACPICA
     instead reads both registers OR'd together and gives PM1b the same base as PM1a (`AcpiHwLegacySleep`). Ours
     keeps each register's own other bits.

   Use the X_ GAS fields when non-zero (FADT 172 and 184), else the 32-bit blocks (64 and 68). The access width
   comes from `PM1_CNT_LEN` (2 on every set). The MVP writes PM1 only as a 16-bit SystemIO register; any other
   space or width fails closed to the "safe to turn off" screen.
   - **A GAS bit width of 0 takes the FADT length byte** (`PM1_CNT_LEN × 8`). The HP's and the Zenbook's `X_GPE0`
     carry width 0, so a firmware X_ block with width 0 is real, not hypothetical.
7. **If still running after the write:** wait 10 s, as ACPICA does (`hwsleep.c:150-167`), then **repeat the second
   write** (SLP_TYP and SLP_EN, the other bits kept), logging each step. If the machine is still on, draw "It's safe
   to turn off the computer now" and halt with `cli; hlt`. **Never restart instead** of a failed power-off.
   - **This differs from ACPICA on purpose.** ACPICA's retry writes SLP_EN alone through `AcpiHwRegisterWrite`,
     which keeps only `ACPI_PM1_CONTROL_PRESERVED_BITS` (0xC3F8, `aclocal.h`). So it clears SCI_EN (bit 0) and
     writes SLP_TYP 0. That is S5 only where `\_S5` is 0 (QEMU); on the seven real sets (7 or 5) it asks for a
     different state. Repeating the full value retries the transition that was asked for.

**Hardware-reduced (designed, fails closed).** When FADT flag bit 20 is set, PM1 doesn't exist. OSPM writes
`(SLP_TYP << 2) | SLP_EN (bit 5)` to `SLEEP_CONTROL_REG` (FADT 244, a GAS). On those machines the button and
wake events come through GPIO and the Generic Event Device, so the full path needs AML. The MVP's rule: if
`HW_REDUCED` is set, **both** `SLEEP_CONTROL_REG` and `SLEEP_STATUS_REG` (FADT 256) are present in SystemIO or
SystemMemory, and the static `\_S5` decode succeeded, then clear `SLEEP_STATUS.WAK_STS` (bit 7) and do the single
write. Otherwise fail closed to the "safe to turn off" screen. Requiring `SLEEP_STATUS` follows ACPICA
(`AcpiHwExtendedSleep` refuses with `AE_NOT_EXIST` when either address is 0); revisions 1-5 named only
`SLEEP_CONTROL_REG`. It is tested by a host KAT only, since no machine we own is hardware-reduced.

### 3.2 The static `\_S5` decode (MVP)

A pure function over the DSDT and SSDT bytes (the kernel already maps the DSDT: `acpi::dsdt()`). It must prove
that what it reads is exactly the root `\_S5`. Two counterexamples from Janus's reviews show what a byte scan gets
wrong:
- **#93:** a `Method (\_S5, 0) { … }` beside `Scope (\_SB) { Name (_S5, Package { Zero, Zero }) }`. A scan that
  accepts the one static `Name _S5` programs SLP_TYP 0 from `\_SB._S5`.
- **#118:** `Name (DUMY, Buffer (0x0A) { 0x08, '_', 'S', '5', '_', 0x12, 0x04, 0x02, 0x00, 0x00 })`, with no real
  `\_S5`. The bytes *inside* the buffer look like `Name (_S5, Package …)`.

Revision 2 tried to prove the scope by scanning for byte patterns that could be scope headers covering the
occurrence. Counting every data container as covering (so the #118 case fails closed) made it fail closed on six of
the ten real sets: random bytes in those tables happen to look like a covering `Buffer` or `Package`. A byte scan
can't be both sound and complete, so revision 3 walks the namespace.

**The decode is a parse-only namespace walk** of every definition block: the DSDT, then the SSDTs in XSDT order.
It executes nothing, and it reads no region. Every table is walked to its end, even once a `\_S5` has been found,
so a table the walk can't parse fails the decode wherever it sits.

1. **Scopes are entered.** `Scope`, `Device`, `Processor`, `PowerResource` and `ThermalZone` push their resolved
   path, and their body is walked.
2. **Conditional bodies are walked, but marked conditional.** `If` and `While` parse their predicate, then walk
   their body. `Else` walks its body. A `Name (\_S5)` found inside any of them fails the decode closed. They are
   walked, not skipped, because firmware declares things there: iASL wraps its `External` block in `If (Zero)`.
   - An `External` inside a conditional body is recorded as unconditional. It creates nothing; it only states a
     type and an arity.
   - Every other definition inside one (`Name`, `Method`, a scope object, a field unit, and the rest of step 4) is
     **conditional**, bound to the body it sits in. It resolves only for a use inside that body or a body nested
     in it. A use anywhere else fails the decode closed, since the walk can't know whether the object will exist.
3. **Some bodies are skipped by PkgLength, never scanned.** These are:
   - `Method`, which records its path and argument count; method bodies are never walked;
   - the data objects `Buffer`, `Package` and `VarPackage`.

   `Field`, `IndexField` and `BankField` have their field lists parsed, to declare the field units. A `String` is
   skipped to its NUL. So the #118 payload is never seen as a term.
4. **Everything else at table level is parsed to its end:**
   - `Name`, with its `DataRefObject` parsed as a term argument;
   - `External`, which records a method's argument count;
   - `Alias`, `OperationRegion`, `Mutex`, `Event`, `DataRegion` and the `CreateXField` family, which declare names;
   - any other Type 1 or Type 2 statement, through an opcode-shape table (each opcode's argument kinds, from the
     ACPI 6.5 §20 grammar).
5. **Names used as terms must resolve.** A name in an expression may be a method invocation, and the walk can only
   stay aligned if it knows the argument count. So, before the decode pass, the walk collects every declaration in
   every table: `Method` and `External (…, MethodObj)` arities, and every other declared name as a non-method.
   This pre-pass repeats until the declaration set stops changing (at most eight passes, else fail closed), so a
   forward reference sees a method declared later in the table, or in a later SSDT. Then the decode pass runs with
   that set:
   - a name resolves by the ACPI 6.5 §5.3 search rules (a lone NameSeg searches up toward the root), against the
     declarations and the predefined root objects (`\_SB`, `\_GPE`, `\_PR`, `\_TZ`, `\_SI`, `\_GL`, `\_OS`,
     `\_REV`, and `\_OSI` with one argument);
   - **a name that resolves to nothing fails the whole decode closed**;
   - a name whose first match in the search order is a conditional definition from a body the use isn't inside
     fails the whole decode closed, so a conditional `Method`'s arity never reaches an unconditional call;
   - a method declared twice with different arities fails closed;
   - the decode pass must produce the same declaration set as the pre-pass, or it fails closed.
6. **Fail closed** on an unknown opcode, a malformed NameSeg or PkgLength, or a body or field list that doesn't end
   exactly at its PkgLength. The `External` record is the one tolerated exception: iASL emits relative names such as
   `External (^GFX0.CLID)` at the root (the HP DSDT), so an `External` whose name can't resolve declares nothing,
   as ACPICA's loader ignores it.
7. **Accept** only when all of these hold:
   - every table walked to its end;
   - the walk reached exactly one unconditional `Name` whose resolved path is `\_S5`;
   - that `Name`'s object is a `Package` with a one-byte PkgLength and NumElements ≥ 2;
   - its first two elements are `ZeroOp`/`OneOp`/`BytePrefix n`, with n ≤ 7 (`ACPI_SLEEP_TYPE_MAX`,
     `actypes.h:622`).

   As a second guard, the bytes `_S5_` must also occur exactly once across all the tables. Any second occurrence
   fails closed: a `Method (\_S5)`, a nested `Name (_S5)`, or a reference to `\_S5`.

**Why this is sound.** A `Name` is accepted only at a term boundary the walk reached, unconditionally, at the root
scope. Data payloads and method bodies are skipped by their own length and never searched. The walk's alignment
depends on one thing it can't read from the bytes, a called method's argument count, and step 5 removes the guess:
every table-level call uses an arity some declaration states, or the decode fails closed. Revision 3 guessed zero
for an unknown name, and that guess was live on a real table: the Zenbook's PEP SSDT calls `ADBG ("…")` at table
level, and its `External (\ADBG, MethodObj, 1)` sits inside the `If (Zero)` block revision 3 skipped. It only
stayed aligned because a string argument also parses as a statement.

**The remaining assumption** is that a declared arity is the true one. A table that declares one count and calls
with another, or calls a method that only exists after another table's method runs, is outside what a static walk
can know. Such a table may misparse, and that is not guaranteed to fail closed: it does when the misparse hits an
invalid byte or leaves a body not ending at its PkgLength. The backstop is the `_S5_` single-occurrence guard: to
accept, a misaligned walk would have to land on that one occurrence, at a `NameOp`, at the root, unconditionally,
followed by a well-formed package.

**Prototype evidence.** `docs/THINK-2026-10-27-ot90-s5-walk.py` is a Python model of these rules, rerun for
revisions 4 and 5 (`python3 -I docs/THINK-2026-10-27-ot90-s5-walk.py fixtures/acpi`); P1 ports it to Rust. The full
output is in Appendix A.
- **Tables walked to end: 93/93**, in the strict pass, every one of them with every table-level name resolved.
- **Sets decoded: 10/10**, to the interpreter's `\_S5` values: 7/0, 5/0 and 0/0.
- **Conditional definitions are common, and none is used outside its body.** Six sets hold 9,731 definitions
  inside table-level conditional bodies (the Zenbook 2,479, the MSI 3,136). Uses inside the same body resolve. No
  corpus table uses one outside its body, so the revision-5 rule fails no real table closed.
- **The hand-laid probes** are runnable: `python3 -I docs/THINK-2026-10-27-ot90-s5-probes.py` prints each case
  with its result.
- **Table-level calls with arguments are common.** The strict pass resolved 240 of them in six sets, for example
  the Zenbook's `GMIO` (3 arguments, 45 calls), `PC2M` (2 arguments) and `ADBG` (1 argument), and `_OSI` on the
  two AMD machines. A walk that guesses arities is not a corner case on this corpus.
- **The negatives.** Cases built with `iasl -f` are marked (iasl); the rest are hand-laid bytes, because iasl
  refuses a method call in a `Name` or a forward call:

  | Case | Result |
  |---|---|
  | Janus #93: `Method (\_S5)` plus `\_SB._S5` (iasl) | fails closed: `Method \_S5` |
  | Janus #118: `_S5` bytes in a `Buffer` payload (iasl) | fails closed: no root `\_S5` |
  | the same bytes in a `String`, and in a `Buffer` inside a `Package` (iasl) | fail closed: no root `\_S5` |
  | `Scope (\_SB) { Name (_S5, …) }` (iasl) | fails closed: no root `\_S5` |
  | `If (One) { Name (_S5, …) }` (iasl) | fails closed: conditional `\_S5` |
  | `Device (DEV0) { Name (_S5, …) }` (iasl) | fails closed: no root `\_S5` |
  | `Name (_S5, Package (2) { 8, 0 })` (iasl) | fails closed: SLP_TYP 8 |
  | Janus #119 probe: a valid root `_S5` in the DSDT, then an SSDT with an unknown opcode | fails closed: the SSDT doesn't parse |
  | `Store (UNKN (Zero), VAL0)`, with `UNKN` declared nowhere | fails closed: unresolved name |
  | a forward `Method (CALL, 2)` called with one argument, ahead of the root `Name (_S5)` | fails closed: the second argument would be the `Name`; revision 3 accepted it |
  | the same method declared with two different arities | fails closed |
  | Janus #120: `If (Zero) { Name (VAL0, One) }`, then `Store (VAL0, Zero)`, then a root `_S5` | fails closed: a conditional-only name used outside its body; revision 4 accepted it |
  | the same without the conditional `Name` | fails closed: unresolved name |
  | `If (Zero) { Method (CALL, 1) }`, then `Store (CALL (Zero), VAL1)` at table level | fails closed: a conditional-only name used outside its body |
  | `Scope (\_SB) { Name (\_S5, …) }` (iasl) | accepted (7/0): an absolute path, bound at the root |
  | a DSDT with no `_S5`, and an SSDT with `Name (_S5, Package { 5, … })` at its root (iasl) | accepted (5/0) from the SSDT |
  | Janus #119: `Name (VAL0, CALL (Zero))`, then `Method (CALL, 1)`, then a root `_S5` | accepted (5/0), with `CALL` taking its one argument |
  | the Zenbook shape: `If (Zero) { External (\ADBG, MethodObj, 1) }`, then `ADBG ("x")` | accepted (5/0), with `ADBG` taking its one argument |
  | `If (One) { Method (CALL, 1); Store (CALL (Zero), VAL1) }`: the call inside the same body | accepted (5/0), with `CALL` taking its one argument |

  ACPICA's own compiler refuses an `_S5` outside the root ("declaration of this named object outside root scope is
  illegal"), so such tables come from broken or hand-built firmware. The walk refuses them all the same.

This is a design limit, stated: a `\_S5` built by a method, inside an `If`, or referenced anywhere else can't be
decoded statically. All of those fail closed to the "safe to turn off" screen, and the interpreter rung (P7)
removes the limit.

**When the decode runs (a rule for P2).** The walk allocates: its declaration set is a map, and six sets hold
9,731 conditional definitions, too many for a fixed-capacity table. P1 measured it at **at most 15 ms per set** on
the host in a release build (the Zenbook's 28 tables, 840 KB of AML; QEMU's under 0.3 ms). So:
- P2 decodes `\_S5` **once, at ACPI init**, in process context with the allocator live, and builds the
  `PowerOffPlan` from it and the FADT;
- it caches that plan, and the decode's outcome is logged at boot;
- the S5 path uses **only the cached plan**. It never decodes, parses or allocates, and it never maps: `init` maps
  the page of any SystemMemory register the plans name (a hardware-reduced SLEEP_CONTROL/STATUS, a memory reset
  register), so the power paths only write.

A decode that failed at init is cached too, as the "safe to turn off" plan with its reason.

- **Byte oracle:** the decoded (a, b) per corpus set must equal the interpreter's `\_S5` result on that set: the
  table in §2 (7/0, 5/0, 0/0). This is a host test over the ten committed sets, so a decoder bug can't pass by
  agreeing with itself. The exact bytes come from the committed tables.

### 3.3 `_PTS(5)`: why the MVP skips it, and the risk

Skipping `_PTS` is a known divergence from the spec and from ACPICA and Linux. The reasons:

- **There is no in-kernel interpreter.** Moving one in needs OT-86's two standing constraints met first: (i) region access
  capability-mediated and range-checked at the handler seam (L6), and (ii) AML step-budgeted or preemptible (L7).
  §2 shows why: `_PTS` writes SMI ports and firmware-chosen memory.
- **On the Zenbook, `_PTS` calls into the EC** (`EC0.EC0S`), so it needs EC region support (L9) too.
- **Prior art shows S5 usually works without it.** Hermit and Redox power off with no `_PTS` (§7).

The risk is real, so it's in the failure bank (§8, F2, F7). The EC may not learn that the system is shutting
down; the TPM may not see its shutdown hook. The bare-metal checks on the OptiPlex and Zenbook decide whether the
MVP needs it. If either fails because `_PTS` is missing, the decision goes to J.c.: hold the MVP for the L6/L7/L9
interpreter rung, or ship a narrow per-machine quirk. The design doesn't pick that in advance.

## 4. Restart

**Order (decided here), each step logged on serial, the NVMe sink and the framebuffer:**

1. **The FADT reset register.**
   - Use it if FADT flag bit 10 (RESET_REG_SUP) is set and the address is non-zero.
   - The address space must be SystemIO, SystemMemory or PCI config.
   - For SystemIO, write an 8-bit value whatever the GAS width says (ACPICA `AcpiReset`, `hwxface.c:39-83`).
   - On QEMU, the Zenbook, the HP and the M910q this is `0xCF9 ← 0x0F/0x06`. On the OptiPlex and AMD boxes it is
     an SMI command on `0xB2`.
   - Then wait ~100 ms, measured with TSC ticks.
2. **The keyboard controller.**
   - Wait for the i8042 input buffer to empty (bounded), write `0xFE` to port `0x64`, and wait ~100 ms.
   - Linux does this twice, retrying ACPI once in between (`arch/x86/kernel/reboot.c:580` onward, read for
     principle).
   - Our i8042 driver already owns ports 0x60/0x64 (`drivers/ps2/i8042.rs:148-153`).
3. **0xCF9 directly.**
   - Only if step 1 wasn't already 0xCF9. Write `(cf9 & ~0x0E) | 0x02`, then `| 0x0E` for a cold hard reset.
   - Linux uses CF9 only when forced or quirked. We put it after the keyboard controller for the same caution:
     on the OptiPlex the FADT says 0xB2, not CF9.
4. **Triple fault.** Load a zero-limit IDT and execute `int3`. Hermit (`processor.rs:1065`) and Redox (`stop.rs`
   `emergency_reset`) do the same.
5. **If still running:** "Restart failed. It's safe to turn off the computer now," then `cli; hlt`.

EFI `ResetSystem` is left out of the MVP. Limine doesn't set the virtual address map, and the kernel has no runtime
services mapped. It is the one option a future machine might need, so it is named here.

Restart runs the same clean sequence as power-off (§5) up to the S5 write, then this chain instead.

## 5. What "clean" means

Shutdown runs in a dedicated kernel thread, not in the caller's syscall frame (the SerenityOS pattern,
`Kernel/Tasks/PowerStateSwitchTask.cpp:60-134`). Each step has a bound. A step that overruns is logged with its
number and skipped. The last two steps (§3.1, §4) depend on nothing that came before them.

| # | Step | Owner | Bound | On overrun |
|---|---|---|---|---|
| U1 | The user picks Shut down or Restart; the desktop asks to confirm | neo-init | none (the user) | – |
| U2 | neo-init sends `PREPARE_POWER_OFF` on every app's session channel. Apps save, then reply `READY` | neo-init + apps | 10 s total, a constant | stop waiting; the stragglers are killed in K1 |
| U3 | neo-init `fsync`s its own open NeoFS files, then makes the power syscall (§6) | neo-init | – | – |
| K1 | Terminate every user process except the caller, then reap | kernel | 2 s | log the survivors' PIDs and continue |
| K2 | NeoFS, per writable mount: freeze writes, then a shutdown-only drain, then stop the writeback thread (below) | kernel | progress-based: give up when no block has drained for 5 s | report **unclean** with the dirty count. The last durable generation still mounts, because the commit barrier (#180) holds even when a drain stops part-way |
| K3 | Flush the log sink (`log::flush()`, `log/block_sink.rs:698`) | kernel | 1 s | continue |
| K4 | Stop the APs: a new STOP IPI (vector 0xfb). The handler masks interrupts, sets a per-CPU `parked` flag and loops on `hlt`. Wait for every flag | kernel (BSP) | 100 ms, then an NMI-IPI to the holdouts (the NMI handler, `idt.rs:1163`, already halts; with a "shutdown in progress" flag it halts quietly) | log the CPU ids; continue on the BSP |
| K5 | Mask interrupt sources: LAPIC timer (the `disarm` logic, `lapic_timer.rs:302`), every IO-APIC entry, MSI/MSI-X per device | kernel (BSP) | – | – |
| K6 | Quiesce DMA, then turn the IOMMU off: stop each bus master (§5.1), clear Bus Master Enable, then `iommu::disable()` last | kernel (BSP) | per device: its own spin budget | log the device; continue |
| K7 | S5 (§3.1) or reset (§4) | kernel (BSP) | §3.1 step 7 | the "safe to turn off" screen |

**K2 in detail.** Its durability claim rests on the NeoFS commit barrier and deferred reuse (#180):
- a superblock copy is written only after a flush has made every block it names durable;
- a flush error stops the commit before any superblock is written;
- a block a commit frees isn't reused until that commit is durable on every copy;
- a device that can't flush is never mounted writable.

So any stop inside K2 leaves the last durable generation mountable. Data the cache held past that point is lost.

The order matters, because `fsync_regular` returns early on a mount that isn't writable (`kernel/src/fs/neofs/inode.rs`,
`fsync_regular`). "Mark read-only, then fsync" would therefore drain nothing. K2 instead:
1. **Freezes writes.** A per-mount shutdown flag makes every mutating VFS call return `EROFS`-equivalent, while the
   mount stays writable underneath.
2. **Runs a shutdown-only drain.** It drains the cache through the same barrier order (`drain_all`), with no early
   return for the frozen mount.
3. **Stops the writeback thread.** It reports the drain's result and the thread's exit code, which `stop_writeback_worker`
   returns today but `Drop` discards.

K2's outcome is **clean** only when the drain returned, nothing is left dirty, and the last flush returned success.
A flush error, a 5 s no-progress timeout, or a write error is **unclean**. Unclean is logged with the dirty count,
shown on the shutdown screen, and the power-off proceeds. It is never retried, and never reported as clean.

**A drain that never returns** (a device that accepts a write and never completes it) is bounded from outside, not by
K2's own checks (#267, Chad #199 item 2). K1-K3 run in their own thread, and the shutdown thread waits for it for at
most 20 s; past that it abandons it where it stands. Either way, the block log sink is then closed: K3 was its last
flush, so the power path's own prints and `log::flush()` never wait on that disk. The proof is two legs:
`ot90_stuck_disk` (the disk stops answering during K2) and `ot90_stuck_sink_off`/`_restart` (the sink's disk stops
right after K3). Each one's control, with the deadline or the close removed, hangs.

**Lock discipline (pitfall bank, `docs/concurrency-pitfalls.md`):**

- **Stop the APs (K4) only after every step that takes a shared lock** (K1-K3). An AP stopped while holding a lock
  is a deadlock for the BSP's next acquisition. This is the CP-29 class, and CP-2/CP-3 if the BSP spins with
  IF=0 waiting.
- **After K4, the BSP must not issue a TLB shootdown, allocate from a shared allocator, or take the serial or log
  lock.** A shootdown waits for acks from parked CPUs (the CP-8 and CP-64 class). It writes breadcrumbs through a
  lock-free raw path, as panic does. K5-K7 are written to need none of these.
- **The STOP IPI handler EOIs before it parks** (CP-7). Otherwise the LAPIC stays in-service for that vector.
- **K1 relies on `terminate` being complete across wait queues** (CP-33, CP-36). Its 2 s bound turns a teardown
  bug into a log line instead of a hang.
- **Leave no level-triggered line asserted** when IO-APIC entries are masked (CP-72, CP-75). Masking at the
  IO-APIC first and quiescing the device second is the order that can't storm.

**If something hangs inside a step** with interrupts off, no software bound can fire. The honest backstop is the
firmware's power-button override (hold 4 s), which always works. The design keeps every in-step wait bounded by
a spin budget, as each driver already does (e.g. `drivers/usb/xhci.rs:575`).

### 5.1 Per-device quiesce (K6)

Each driver gets a public `quiesce()`. It stops DMA and leaves the device re-initialisable, the shape sleep will
need (§10).

- **NVMe:** NVM Flush, then CC.SHN = 01b (normal shutdown), wait for CSTS.SHST = 10b, then CC.EN = 0
  (`disable_controller`, `nvme.rs:644`, already does the last step).
- **AHCI:** FLUSH CACHE EXT, STANDBY IMMEDIATE (so a spinning disk parks its heads), then stop each port
  (`stop_port`, `ahci.rs:511`).
- **virtio-blk, virtio-net, virtio-accel:** status = 0 (`reset_device`).
- **xHCI:** halt (USBCMD.R/S = 0), wait HCH. The init code already does this (`xhci.rs:662-680`).
- **RTL8125:** `reset_chip` (`rtl8125.rs:915`), which also stops it waking the machine (wake-on-LAN, F2).
- **Intel GPU:** clear Bus Master Enable; its rings are idle once userland is gone.

**Which disks are touched.** NeoFS's own device is flushed and shut down. A disk the write guard hasn't armed is
not flushed and not shut down: the Zenbook's Windows NVMe, for instance (`writes_allowed`, `block/mod.rs:402-407`).
`flush()` passes the guard unguarded today (`:379-385`), so K2 must check `writes_allowed` itself. This is
**decision D1 for J.c.** (§12). The firmware shuts disks down on S5 anyway, and a flush to the Windows disk writes
no data, but the project's rule is that the kernel leaves that disk alone.

### 5.2 Proving it was clean

NeoFS has no clean flag, and adding one would break every existing kernel (bytes 13..16 must be zero,
`format.rs:131-133`). Clean is proved with two checks:

1. **Durability.** Data written before shutdown, with no `fsync`, is present on the next boot. That is the complement
   of the existing crash scenario, where it is absent (`test_harness.rs:30705-30760`).
2. **Agreement.** All three superblock copies are valid and carry the same generation (`publish_superblock` writes
   all three, `inode.rs:2549-2571`). A shutdown cut mid-publish leaves them disagreeing.

The mount logs both facts at every boot, so a bare-metal run reads them from the screen or the serial log. The
format doesn't change.

Agreement on the next boot is evidence, not proof. A shutdown K2 reported as unclean can still leave agreeing copies
at an older generation, so the shutdown's own clean/unclean line is the authority for what was lost.

## 6. Who may ask: the capability

The design follows ADR-010 (`docs/ADR-010-capability-system-foundation.md`) and copies the `InputSystem` precedent.

- **`Handle::PowerControl`**, objectless. Rights: `USE` plus semantic bits `POWER_OFF`, `RESTART`, and `SLEEP`
  reserved for 1.0. **No `DERIVE`, no `GRANT`**, so it can't be passed on and holding it is exact.
- **It is minted by a kernel-only constructor**, `root_power_control_cap()`, with no syscall twin. It is installed
  only into neo-init, next to `root_input_system_cap` (`main.rs:415-420`), in the documented stable order
  (`main.rs:340-353`). Note CP-34 (feature-exclusive root-cap assembly): the composed product image must be
  tested, not only the single-feature build.
- **The syscall: `SYS_POWER = 24`**, the first free base slot (`docs/SDK-ABI.md:220-230`).
  - Arguments are in registers, so there is no struct: `rdi = action` (1 = power off, 2 = restart),
    `rsi = flags` (must be 0).
  - On success it doesn't return.
  - Errors: `EPERM` when the caller has no `PowerControl` with that bit (the `holds_input_system` pattern,
    `process.rs:1405`); `EINVAL` for an unknown action or a non-zero flag; `EBUSY` if a shutdown is already
    running.
  - The constants are mirrored in `libneo`; a host KAT pins the numbers on both sides.
- **The desktop holds it; apps and Arthur don't.** Apps are spawned by neo-init and only get handles with `GRANT`
  (`syscall.rs:1602-1676`), and this cap has none.
  - If Arthur asks to shut down, that request goes to neo-init's confirm dialog. It's an L-tier action (typed
    confirm in every posture, `userland/neo-arthur/src/conversation_logic.rs:722-742`), never a capability Arthur
    holds.
  - A future `neo-shell` `shutdown` command works the same way, as a request to the desktop.
- **With login (OT-88):** the session broker becomes the holder. The lock and login screens still offer Shut down
  and Restart, as desktop OSes do. That is OT-88's call; this design doesn't block it.

## 7. Prior-art survey

Per the 2026-10-27 standing rule (CLAUDE.md, "Unfamiliar features"). Each repository was cloned (`git clone
--depth 1`) and its code read for this THINK on 2026-10-27. The SHA is the clone's last commit, and **every date is
that commit's committer date in UTC** (`TZ=UTC git log -1 --date=iso-strict-local --format=%cd`).

| Project | Approach to power-off and restart | Licence | Last commit | Fit, and what we take |
|---|---|---|---|---|
| **rust-osdev/acpi** (our vendored base) | `FixedRegisters`/`Pm1ControlRegisterBlock`: `set_sleep_typ`, `SleepEnable`, SCI_EN; `Fadt::reset_register()` (`src/sdt/fadt.rs:331`). No sleep sequence, no `\_S5` lookup | MIT / Apache-2.0 | `4841ccb7` 2026-10-25, active | **Design, and already vendored code** (`third_party/acpi-loganfy/src/registers.rs`). Trap: `set_sleep_typ` writes SLP_TYPa to PM1b as well (`registers.rs:163-174`). The kernel can't use the crate's GAS mapping without a kernel `Handler`, so the MVP's PM1 and reset logic is a small pure module of our own |
| **Hermit** (`hermit-os/kernel`) | `src/acpi/shutdown.rs`: **static `_S5_` byte scan** of the DSDT then SSDTs (first match), a single PM1a write with SLP_EN, no PM1b, no `_PTS`. Then a triple fault (`processor.rs:1065-1100`). Tests use isa-debug-exit | MIT / Apache-2.0 | `373d7a01` 2026-10-26, active | **Design** for §3.2. We add: exactly-one rule, the SLP_TYP ≤ 7 check, PM1b, the two-write split, SCI_EN, and the corpus oracle. About 60 lines, so we write ours rather than vendor |
| **Redox** (`redox-os/kernel`, `redox-os/drivers`) | Kernel `kstop` hands off to a **userspace `acpid`** that evaluates `\_S5` with the `aml` interpreter (`drivers/acpid/src/acpi.rs:548-630`: PM1a only, "TODO: Handle SLP_TYPb"). Then QEMU and Bochs magic ports (0x8900 "Shutdown", 0x604 ← 0x2000), then `cli; hlt` (`kernel/src/arch/x86_shared/stop.rs:95-130`). Reset: the 8042 `0xFE`, then a triple fault (`stop.rs:9-60`); **no FADT reset register** | MIT | kernel `b68957d4` 2026-09-26 (19:58:20Z); drivers `20ffe4d7` 2025-11-29 (the GitHub mirror; the canonical GitLab repo wasn't checked) | **Design reference only.** The userspace power daemon doesn't fit our kernel-owned quiesce. The magic ports aren't needed: QEMU's ACPI S5 works (§9) |
| **Theseus** | Parses the FADT, including `reset_reg` (`kernel/acpi/fadt/src/lib.rs:68-69`). **No power-off or restart path**; `nano_core` "shutdown" is a halt | MIT | `1fbfe567` 2024-09-22, **past the two-year mark** | Nothing |
| **Asterinas** | A **prioritized handler chain** (`kernel/core/src/power.rs`): restart and power-off handlers registered at link time, tried in descending priority, and each must return (not panic) if it can't act. x86 restart is the FADT reset port (SystemIO only, `kernel/core/src/arch/x86/power.rs:12-20`, `ostd/src/arch/x86/kernel/acpi/mod.rs:150-154`), then the i8042 at low priority (`kernel/comps/i8042/src/lib.rs:43`). x86 power-off is **QEMU isa-debug-exit only** (`ostd/src/arch/x86/power.rs`) | **MPL-2.0** (our licence) | `6e7f97a1` 2026-10-27, active | **Design: the ordered, must-return chain** for §4. It is small enough to write in our own style. Its power-off doesn't help on real hardware |
| **SerenityOS** | A dedicated kernel task (`Kernel/Tasks/PowerStateSwitchTask.cpp:60-134`): kill all user processes, sync and unmount every filesystem, take the scheduler lock, then reboot via the FADT reset register (`Kernel/Firmware/ACPI/Parser.cpp:265-288`) then the i8042. Power-off is QEMU and VirtualBox ports only; **ACPI power-off is "not supported"** (`Parser.cpp:290-293`). Falls back to "safe to turn off" | BSD-2-Clause | `1f27b390` 2026-10-26, active | **Design: the kernel-task shape and the order of §5's K-steps.** C++, so no code |
| **Linux** (`torvalds/linux`, read for principle only, **GPL-2.0: no code copied**) | Power-off: `acpi_power_off_prepare` runs `_PTS` through `acpi_sleep_prepare` (`drivers/acpi/sleep.c:67-83`, `:1084-1091`), disables GPEs, waits for pending ACPI events, then `acpi_enter_sleep_state(S5)` (`:1098`). Restart (`arch/x86/kernel/reboot.c:580` onward): ACPI reset register → 8042 (twice, with one ACPI retry) → EFI → BIOS → 0xCF9 (only when forced or quirked) → triple fault | GPL-2.0 | `d135ab45` 2026-10-26, active | **Principle only:** the `_PTS` placement, the GPE disable, and the restart order and its caution about 0xCF9 |
| **ACPICA** (`acpica/acpica`, local checkout `e85aa3be`) | The reference sequence quoted throughout §3-§4: `hwsleep.c`, `hwxfsleep.c`, `hwacpi.c`, `hwxface.c` | dual BSD/GPL (Intel) | `e85aa3be` release 20260930 | **Principle and the exact register rules.** Out of scope in any form in the kernel (OT-86) |

**Conclusion.** No candidate does what we need on real hardware with the clean sequence: the Rust OSes skip `_PTS`
and most skip the reset register. We **write our own**, borrowing three designs:

- Hermit's static decode, made strict;
- Asterinas's must-return handler chain;
- SerenityOS's kernel-task order.

We also reuse the already-vendored rust-osdev register definitions as a cross-check. **No new code is vendored.**

## 8. Testing

### 8.1 QEMU scenarios

The xtask runner gets one capability: it reads QEMU's own report of *why* it stopped. It does this through a QMP
socket (`-qmp unix:…`): the `SHUTDOWN` event carries `reason: guest-shutdown` for ACPI S5 and `guest-reset` for a
reset or triple fault. Today the two can't be told apart, because under `-no-reboot` both exit QEMU with 0
(§1). If QMP isn't usable on a runner, the fallback discriminator is `-d cpu_reset -D <file>`: a reset logs a CPU
reset, and S5 logs none. Every scenario is hermetic: no network, no CPU-feature dependence.

| Scenario | What it shows | Pass condition |
|---|---|---|
| `ot90_s5_exits_qemu` | Power-off works and ends QEMU | QMP `SHUTDOWN` reason `guest-shutdown`; the serial log shows K1-K7 in order with no "overrun"; no `RESET` event |
| `ot90_s5_durability` | Clean means durable | Boot 1 writes a marker file **without fsync** and powers off. xtask relaunches QEMU on the same disk. Boot 2 finds the marker, logs that all three superblock copies agree, and passes through isa-debug-exit |
| `ot90_restart_second_boot` | Restart reboots into a second boot | Run without `-no-reboot`. Boot 1 writes a marker and restarts; QMP shows `RESET` reason `guest-reset`. Boot 2 (same QEMU process) sees the marker and boot counter = 2, then powers off (`guest-shutdown`) |
| `ot90_restart_fallback_i8042` | The chain moves on when a step does nothing | A test feature makes step 1 (FADT reset) a no-op. QEMU resets through the i8042; the log shows step 1 then step 2 |
| `ot90_cap_denied` | Only the cap holder can ask | A process without `PowerControl` calls `SYS_POWER` and gets `EPERM`; the system keeps running and the scenario passes through isa-debug-exit |
| `ot90_straggler_forced` | The bounds hold | A test app ignores `PREPARE_POWER_OFF`. After the U2 bound the shutdown proceeds, K1 kills the app, and S5 happens |
| `ot90_s5_smp4_viommu` | The quiesce order holds on SMP with VT-d | `NEO_SMP=4` and the viommu machine: every AP parks within 100 ms, devices quiesce before `iommu::disable()`, and S5 happens |
| `ot90_kill_switch_off` | Disabled ≡ today | With `power-control` off, `SYS_POWER` returns `ENOSYS`; the scenario set is otherwise unchanged |

Host KATs (push CI):

- the static `\_S5` decode on all ten corpus sets equals the interpreter's result (§3.2);
- the §3.2 walk parses every corpus DSDT and SSDT (93 tables) to its end in the strict pass, with every
  table-level name resolved; the KAT asserts the per-table result, not only the decoded value;
- the §3.2 negatives fail closed, committed as fixtures (compiled with `iasl -f` where iasl accepts them,
  hand-laid bytes otherwise):
  - Janus #93 (`Method (\_S5)` plus `\_SB._S5`);
  - Janus #118 (`_S5` bytes in a `Buffer` payload), and the same bytes in a `String` and in a `Package`;
  - `\_SB._S5` alone;
  - `_S5` in an `If` and in a `Device`;
  - a valid root `_S5` in the DSDT followed by an SSDT that doesn't parse (Janus #119);
  - a call to a name declared nowhere;
  - a forward `Method (CALL, 2)` called with one argument ahead of the root `Name (_S5)`;
  - one method declared with two arities;
  - Janus #120, a `Name` defined only under `If (Zero)` and used at table level;
  - a `Method` defined only under `If (Zero)` and called at table level.

  Five cases are accepted: the absolute `Name (\_S5)` inside `Scope (\_SB)`; a root `Name (_S5)` in an SSDT
  when the DSDT has none; Janus #119's forward call `Name (VAL0, CALL (Zero))` before `Method (CALL, 1)`; and the
  Zenbook shape, an `External (\ADBG, MethodObj, 1)` inside `If (Zero)` with `ADBG ("x")` called at table level; and a `Method` defined and called inside the same `If` body;
- malformed `_S5` packages fail closed (a PkgLength with a multi-byte lead, NumElements 1, a `DWordPrefix` element,
  SLP_TYP 8);
- K2's outcome: clean only on a returned drain with nothing dirty and a successful last flush; every other end is
  unclean;
- the PM1 value composition (bits 10-13, other bits preserved);
- the reset-chain order for each corpus FADT;
- the hardware-reduced branch;
- the `SYS_POWER` constants on both sides.

### 8.2 Bare metal

**The OptiPlex**, per `docs/runbook-baremetal-arthur-optiplex.md`. Evidence: serial log plus the NVMe log sink, no
photos. A joint run.

- **Power-off:** the box turns off by itself; the serial log ends with K7 and the S5 write. Then the PL powers it
  on and boots Ubuntu, and the sink is read.
- **Restart:** the box POSTs again. BootNext is spent by then, so it falls back to BootOrder (Windows). Success
  is the reset itself, seen as a new POST; the serial log ends with the restart chain's step 1 and the `0xB2`
  write. To see NeoPlutarch boot a second time, the PL picks it with F12, or temporarily puts it first in
  BootOrder. That is a runbook step, since the kernel can't set BootNext.

**The Zenbook**, per `docs/runbook-baremetal-zenbook-s14.md`. Photos of the fb progress console are evidence; there
is no serial. The image is built with the write guard on.

- **Power-off:** a photo of the K-step screen, then the lid LEDs go off and the fan stops.
- **Restart:** a photo of the restart step, then the ASUS logo appears; Esc then selects the stick again.
- Then power on normally: Windows boots unchanged, and its disk was never written. That is the standing rule,
  checked again here.

### 8.3 Failure-mode and discriminator bank (bare metal)

Pre-seeded from the corpus (§2) and prior art (§7). **Capture-before-you-need-it:** every boot logs `PM1_STS` and
`GPE0_STS` before clearing anything, and every shutdown logs the values below *before* the write that could end
the run.

| # | Failure | Likely causes | Discriminator (what is logged, and what maps to what) |
|---|---|---|---|
| F1 | The S5 write is accepted, the machine stays on | SCI_EN clear (legacy mode); wrong SLP_TYP; a PM1b that needed its own write; firmware wants `_PTS` first | Before the write: `PM1a_CNT` as read (bit 0 = SCI_EN), whether step 3 wrote `ACPI_ENABLE` and whether SCI_EN set, the SLP_TYP chosen, and the static-decode offset. After 10 s: the step-7 retry line. SCI_EN=0 that never set points to SMI mode; SCI_EN=1 with correct SLP_TYP points to `_PTS` (escalate, §3.3) |
| F2 | It powers off, then powers straight back on (looks like a restart) | A wake source left armed: GPE wake, PCIe PME, RTC alarm, USB wake, **wake-on-LAN (RTL8125)**; or `_PTS` missing on a board that arms wake in it | Before the write: `GPE0_STS`/`GPE0_EN` dump and `PM1_STS`. On the next boot: the boot-time `PM1_STS.WAK_STS` and `GPE0_STS`, which name the wake source. RTL8125 is reset in K6; a BIOS WoL setting is the remaining suspect |
| F3 | It hangs during shutdown | An AP that won't park; a device whose quiesce spins forever; a NeoFS drain stuck on a write error | Every K-step prints `[power] K<n> start` / `K<n> done <ms>` on serial, the sink and the framebuffer. The last line names the step. K4 names the CPU ids that never acked |
| F4 | Restart: the reset write does nothing | The OptiPlex's `0xB2 ← 0x73` SMI not honoured in our state (e.g. SMI disabled after our GPE writes); i8042 absent | Each chain step logs `reset: step <n> <space>:<addr> <- <value>` and then `no reset after 100 ms`. The last line shows which mechanism finally worked, or that none did |
| F5 | Restart boots Windows, not NeoPlutarch | BootNext is spent (the OptiPlex BootOrder has Windows first) | Expected, not a defect: success is the POST. The runbook's F12 / BootOrder step confirms a second NeoPlutarch boot |
| F6 | The next boot is missing data written before shutdown | K2 overran (the drain bound) or a device flush error | K2's line: blocks drained, the flush result, or `overrun: N dirty`. The next boot's superblock-agreement line: generations disagree means it was cut mid-publish |
| F7 | Zenbook: it powers off, but the EC state is odd (keyboard backlight stays lit, a charge LED, the next power-on needs a long press) | `_PTS(5)` → `EC0.EC0S(5)` never ran | A photo of the LEDs after S5, compared with a Windows shutdown on the same machine. A difference is F7, so escalate to D2 (§12) |
| F8 | Anything writes the Windows NVMe | A K2/K6 path that skips the `writes_allowed` check | Must never happen. K2 and K6 log every device they touch with its guard state. The guard itself also refuses the write (`block/mod.rs:372-376`) |
| F9 | An interrupt storm after masking (the machine stalls in K5-K6) | A level-triggered line still asserted from a device not yet quiesced | K5 logs each IO-APIC entry it masks. A stall between K5 and K6 with an IRQ count still rising points to the line named in the log |
| F10 | QEMU passes, bare metal triple-faults in K4-K7 | An AP stopped while holding a lock the BSP then takes; a shootdown issued after K4 | The BSP's raw-path breadcrumbs after K4. A "lock held by parked CPU n" assertion in debug builds |

## 9. Traps found while designing (go into the dispatch banks)

- **SLP_TYPa vs SLP_TYPb.** Each PM1 register gets its own value. The vendored `set_sleep_typ` writes the same value
  to both (`registers.rs:163-174`); Redox and Hermit ignore PM1b.
- **The reset register isn't always 0xCF9.** It's an SMI port on three of our seven real sets. Hard-coding CF9 is wrong
  for the OptiPlex.
- **ACPICA writes the SystemIO reset register 8 bits wide whatever the GAS width says** (`hwxface.c:66-73`), because
  some FADTs declare a wrong width. Ours vary: QEMU's, the Zenbook's, the M910q's and the AMD boxes' give access size 0
  (undefined); the HP's and the OptiPlex's give byte access. Bit width is 8 on every set.
- **Two PM1 writes** (SLP_TYP first, then SLP_EN), "to work around poorly implemented hardware"
  (`hwsleep.c:107-110`).
- **SCI_EN must be preserved** in every PM1_CNT write, and set (through SMI_CMD) if firmware left the machine in
  legacy mode.
- **QEMU under `run_qemu` has `-no-shutdown`**, so S5 pauses it instead of exiting (`xtask/src/main.rs:15907-15908`).
  `HOWTO-run-desktop.md` gets a line when P2 lands.
- **Under `-no-reboot`, a triple fault and S5 both exit QEMU with 0.** A test can't pass on the exit code alone (§8.1).
- **SeaBIOS answers a warm reset with a hard reboot of its own** ("Attempting a hard reboot" in its debug log), so
  one guest reset shows as **two** QMP `RESET` events (`guest-reset`). Found by P2's real-QEMU test of the
  discriminator (QEMU 8.2.2, SeaBIOS 1.16.3). A restart test that expects exactly one `RESET` fails on every BIOS
  boot; P2's judge accepts one or more and catches a boot loop by its serial boot markers instead.
- **On QEMU, SLP_TYP 1 is suspend, not power-off** (`hw/acpi/core.c` `acpi_pm1_cnt_write`: 0 powers off, 1
  suspends, the S4 value hibernates). A wrong `\_S5` decode on QEMU sleeps instead of failing loudly.
- **QEMU acts on SLP_EN only on a 16-bit or high-byte PM1 write** (SLP_EN is bit 13). A low-byte write is ignored,
  so PM1 is always written 16 bits wide.
- **NeoFS's reserved superblock bytes must be zero**, so a clean flag would break existing kernels (§5.2).
- **`flush()` is unguarded** (`block/mod.rs:379-385`). A flush-everything loop would touch the Windows disk (§5.1).

## 10. Sleep and wake (1.0): what OT-90 must not block

- **The Zenbook sets LOW_POWER_S0** (FADT bit 21), the modern-standby signal. Its 1.0 sleep is likely s2idle (S0ix),
  not S3, even though its DSDT also has an `_S3` package. So the power path must not assume "sleep = S3 via PM1".
  The state enum is `{S0idle, S3, S5, Reset}` from the start, even though only S5 and Reset are built.
- **Quiesce must be resumable.** Every driver's `quiesce()` stops DMA and leaves the device re-initialisable. The
  matching `resume()` is 1.0 work. No step frees driver state on the assumption that power is going away (the
  power-off path may free memory, but the per-driver quiesce doesn't).
- **APs are parked, not discarded.** Today's bring-up uses Limine MP (`smp.rs:197`), which exists only at boot. S3 needs
  an INIT-SIPI trampoline of our own. OT-90's K4 park loop is written so a 1.0 resume can restart the same CPUs.
  It doesn't touch per-CPU state, and nothing assumes a parked CPU never runs again.
- **The userland protocol is shared.** `PREPARE_POWER_OFF` / `READY` gets a sibling `PREPARE_SLEEP` / `RESUMED` on
  the same channel and with the same bound. The capability reserves the `SLEEP` bit.
- **Wake sources.** The MVP disables every GPE (§3.1 step 5). Sleep will arm chosen ones, so the GPE code is written per
  bit, not as "write zeros everywhere".
- **The FACS waking vector isn't touched.** Neither are `_WAK` or `_TTS`, which no set has.

## 11. The rung ladder

Dependencies, cycles and gates only, per the no-calendar rule. **Kill-switch on every policy rung:** feature
`power-control` (default off) gates P2-P5, and a nested feature per rung (`power-clean`, `power-cap`,
`power-menu`). Each one ships a regression showing that disabled ≡ the previous layer: today, `SYS_POWER` is
`ENOSYS` and the end-of-session path still reaches `halt_forever`.

| Rung | What | Depends on | Iteration cycles (expected) | Gate |
|---|---|---|---|---|
| **P0** | This THINK; a cross-family read (GPT-6-Luna high, plain checklist) | – | 1-2 | doc review |
| **P1** | Host-only pure logic: the FADT power-field parse, the static `\_S5` decode, PM1 value composition, the reset chain plan. A `kernel/src/acpi/power_logic.rs` module with host KATs, including the 10-set corpus oracle | P0 | 1-2 (mechanical, fully oracled) | push CI |
| **P2** | The mechanism: S5 write and reset chain, test-entry only. Scenarios `ot90_s5_exits_qemu`, `ot90_restart_second_boot`, `ot90_restart_fallback_i8042`, `ot90_kill_switch_off`; the xtask QMP discriminator | P1 | 1-3 (QEMU is forgiving; the QMP plumbing is new) | 4-shard QEMU + AAG (port I/O and interrupt lenses) |
| **P3** | Clean: K1-K6, with the STOP IPI, interrupt masking, per-driver `quiesce()`, the K2 freeze and shutdown-only drain with its clean/unclean outcome, guarded flush, and the bounds. Needs #180 merged. Scenarios `ot90_s5_durability`, `ot90_straggler_forced`, `ot90_s5_smp4_viommu` | P2 | 2-4 (new SMP stop path, six drivers) | 4-shard QEMU + AAG **with a cross-family lens** (SMP core: CPU stop, IPI); pitfall-bank consult (CP-2, -3, -7, -8, -29, -33, -36, -64, -72, -75) |
| **P4** | The capability and `SYS_POWER`, root-minted into neo-init. Scenario `ot90_cap_denied`; the constants KAT; the CP-34 product-composition check | P3 | 1-2 | 4-shard QEMU + AAG (ABI lens) |
| **P5** | Userland: the Power item at the right end of the craft bar, the confirm dialog, `PREPARE_POWER_OFF` / `READY` to apps, Arthur's save path, the U2 bound | P4; parallel with OT-89 | 1-2 | QEMU desktop scenario with synthetic input; cross-family review |
| **P6** | Bare-metal acceptance: OptiPlex (power-off, restart), then the Zenbook (photos) | P5 (P3 is enough for a scenario-driven run) | **unknown depth** (blind hardware: the next run may reveal the next layer, F1-F10) | acceptance milestone in the ledger, not a merge gate |
| **P7** (post-MVP) | `_PTS(5)` / `_SST`, `\_S5` from the interpreter (the static decode stays as a cross-check), the fixed-event power button (PM1 `PWRBTN_STS`, SCI on GSI 9 for every set), then the control-method button (Zenbook PWRB `Notify` via GPE/EC) | OT-86 L6, L7, L9 and an in-kernel interpreter rung | many; genuinely open | as for each OT-86 rung |
| **1.0** | Sleep and wake (§10) | P7 | many | its own THINK |

P1 and P2 can be one dispatch if the PM prefers. P5 may run alongside P3/P4 against a stub syscall, but it merges
only after P4.

## 12. Decisions for J.c.

- **D1, disks at shutdown. Decided (J.c., 2026-10-29): the recommendation.** Recommended: flush and shut down only disks the write guard has armed (NeoFS's device);
  leave every other disk untouched, including the Zenbook's Windows NVMe. The firmware's S5 handles them. The
  alternative is an NVMe shutdown notification (no data written) to every controller we enabled. It's tidier
  for the hardware but touches the disk the project treats as "not ours".
- **D2, `_PTS(5)` in the MVP. Decided (J.c., 2026-10-29): the recommendation.** Recommended: ship the MVP without it (§3.3), and let the bare-metal checks (F1, F2,
  F7) decide. If one fails because of it, the choice between holding the MVP for OT-86 L6/L7/L9 and a narrow
  per-machine quirk goes back to J.c.

Everything else in this THINK is a design choice the cross-family read can challenge.

## Appendix A. §3.2 prototype output (revision 5)

`python3 -I docs/THINK-2026-10-27-ot90-s5-walk.py fixtures/acpi`, at this revision's head (identical to revision 4's). One line per table (the
strict pass), then the set's decode, then the totals.

```
  hp-elitedesk-800-g6-mini-s21-02.27.00    dsdt.dat     OK
  hp-elitedesk-800-g6-mini-s21-02.27.00    ssdt.dat     OK
  hp-elitedesk-800-g6-mini-s21-02.27.00    ssdt1.dat    OK
  hp-elitedesk-800-g6-mini-s21-02.27.00    ssdt10.dat   OK
  hp-elitedesk-800-g6-mini-s21-02.27.00    ssdt2.dat    OK
  hp-elitedesk-800-g6-mini-s21-02.27.00    ssdt3.dat    OK
  hp-elitedesk-800-g6-mini-s21-02.27.00    ssdt4.dat    OK
  hp-elitedesk-800-g6-mini-s21-02.27.00    ssdt5.dat    OK
  hp-elitedesk-800-g6-mini-s21-02.27.00    ssdt6.dat    OK
  hp-elitedesk-800-g6-mini-s21-02.27.00    ssdt7.dat    OK
  hp-elitedesk-800-g6-mini-s21-02.27.00    ssdt8.dat    OK
  hp-elitedesk-800-g6-mini-s21-02.27.00    ssdt9.dat    OK
hp-elitedesk-800-g6-mini-s21-02.27.00 -> ('OK', 'dsdt.dat', '0x20516', [7, 0])
  lenovo-thinkcentre-m75q-gen1-m2fkt21a    dsdt.dat     OK
  lenovo-thinkcentre-m75q-gen1-m2fkt21a    ssdt.dat     OK
  lenovo-thinkcentre-m75q-gen1-m2fkt21a    ssdt1.dat    OK
  lenovo-thinkcentre-m75q-gen1-m2fkt21a    ssdt2.dat    OK
  lenovo-thinkcentre-m75q-gen1-m2fkt21a    ssdt3.dat    OK
  lenovo-thinkcentre-m75q-gen1-m2fkt21a    ssdt4.dat    OK
  lenovo-thinkcentre-m75q-gen1-m2fkt21a    ssdt5.dat    OK
  lenovo-thinkcentre-m75q-gen1-m2fkt21a    ssdt6.dat    OK
lenovo-thinkcentre-m75q-gen1-m2fkt21a -> ('OK', 'dsdt.dat', '0x3aeb', [5, 0])
  lenovo-thinkcentre-m910q-m1akt5aa        dsdt.dat     OK
  lenovo-thinkcentre-m910q-m1akt5aa        ssdt.dat     OK
  lenovo-thinkcentre-m910q-m1akt5aa        ssdt1.dat    OK
  lenovo-thinkcentre-m910q-m1akt5aa        ssdt2.dat    OK
  lenovo-thinkcentre-m910q-m1akt5aa        ssdt3.dat    OK
  lenovo-thinkcentre-m910q-m1akt5aa        ssdt4.dat    OK
  lenovo-thinkcentre-m910q-m1akt5aa        ssdt5.dat    OK
  lenovo-thinkcentre-m910q-m1akt5aa        ssdt6.dat    OK
lenovo-thinkcentre-m910q-m1akt5aa -> ('OK', 'dsdt.dat', '0xa6ce', [7, 0])
  msi-ms-7e57-1.a70                        dsdt.dat     OK
  msi-ms-7e57-1.a70                        ssdt.dat     OK
  msi-ms-7e57-1.a70                        ssdt1.dat    OK
  msi-ms-7e57-1.a70                        ssdt10.dat   OK
  msi-ms-7e57-1.a70                        ssdt11.dat   OK
  msi-ms-7e57-1.a70                        ssdt12.dat   OK
  msi-ms-7e57-1.a70                        ssdt13.dat   OK
  msi-ms-7e57-1.a70                        ssdt14.dat   OK
  msi-ms-7e57-1.a70                        ssdt2.dat    OK
  msi-ms-7e57-1.a70                        ssdt3.dat    OK
  msi-ms-7e57-1.a70                        ssdt4.dat    OK
  msi-ms-7e57-1.a70                        ssdt5.dat    OK
  msi-ms-7e57-1.a70                        ssdt6.dat    OK
  msi-ms-7e57-1.a70                        ssdt7.dat    OK
  msi-ms-7e57-1.a70                        ssdt8.dat    OK
  msi-ms-7e57-1.a70                        ssdt9.dat    OK
msi-ms-7e57-1.a70 -> ('OK', 'dsdt.dat', '0x3698', [5, 0])
  optiplex-7080-1.37.0                     dsdt.dat     OK
  optiplex-7080-1.37.0                     ssdt.dat     OK
  optiplex-7080-1.37.0                     ssdt1.dat    OK
  optiplex-7080-1.37.0                     ssdt3.dat    OK
optiplex-7080-1.37.0 -> ('OK', 'dsdt.dat', '0x59b0', [7, 0])
  optiplex-7080-linux-1.37.0               dsdt         OK
  optiplex-7080-linux-1.37.0               dynamic-ssdt10 OK
  optiplex-7080-linux-1.37.0               dynamic-ssdt11 OK
  optiplex-7080-linux-1.37.0               dynamic-ssdt12 OK
  optiplex-7080-linux-1.37.0               dynamic-ssdt13 OK
  optiplex-7080-linux-1.37.0               dynamic-ssdt7 OK
  optiplex-7080-linux-1.37.0               dynamic-ssdt8 OK
  optiplex-7080-linux-1.37.0               dynamic-ssdt9 OK
  optiplex-7080-linux-1.37.0               ssdt1        OK
  optiplex-7080-linux-1.37.0               ssdt2        OK
  optiplex-7080-linux-1.37.0               ssdt3        OK
  optiplex-7080-linux-1.37.0               ssdt4        OK
  optiplex-7080-linux-1.37.0               ssdt5        OK
  optiplex-7080-linux-1.37.0               ssdt6        OK
optiplex-7080-linux-1.37.0 -> ('OK', 'dsdt', '0x59b0', [7, 0])
  qemu-q35-ovmf-smp1                       dsdt.dat     OK
qemu-q35-ovmf-smp1 -> ('OK', 'dsdt.dat', '0x1eaa', [0, 0])
  qemu-q35-ovmf-smp4-viommu                dsdt.dat     OK
qemu-q35-ovmf-smp4-viommu -> ('OK', 'dsdt.dat', '0x1fa9', [0, 0])
  qemu-q35-seabios-smp1                    dsdt.dat     OK
qemu-q35-seabios-smp1 -> ('OK', 'dsdt.dat', '0x1eaa', [0, 0])
  zenbook-s14-ux5406aa-303                 dsdt.dat     OK
  zenbook-s14-ux5406aa-303                 ssdt.dat     OK
  zenbook-s14-ux5406aa-303                 ssdt1.dat    OK
  zenbook-s14-ux5406aa-303                 ssdt10.dat   OK
  zenbook-s14-ux5406aa-303                 ssdt11.dat   OK
  zenbook-s14-ux5406aa-303                 ssdt12.dat   OK
  zenbook-s14-ux5406aa-303                 ssdt13.dat   OK
  zenbook-s14-ux5406aa-303                 ssdt14.dat   OK
  zenbook-s14-ux5406aa-303                 ssdt15.dat   OK
  zenbook-s14-ux5406aa-303                 ssdt16.dat   OK
  zenbook-s14-ux5406aa-303                 ssdt17.dat   OK
  zenbook-s14-ux5406aa-303                 ssdt18.dat   OK
  zenbook-s14-ux5406aa-303                 ssdt19.dat   OK
  zenbook-s14-ux5406aa-303                 ssdt2.dat    OK
  zenbook-s14-ux5406aa-303                 ssdt20.dat   OK
  zenbook-s14-ux5406aa-303                 ssdt21.dat   OK
  zenbook-s14-ux5406aa-303                 ssdt22.dat   OK
  zenbook-s14-ux5406aa-303                 ssdt23.dat   OK
  zenbook-s14-ux5406aa-303                 ssdt24.dat   OK
  zenbook-s14-ux5406aa-303                 ssdt25.dat   OK
  zenbook-s14-ux5406aa-303                 ssdt26.dat   OK
  zenbook-s14-ux5406aa-303                 ssdt3.dat    OK
  zenbook-s14-ux5406aa-303                 ssdt4.dat    OK
  zenbook-s14-ux5406aa-303                 ssdt5.dat    OK
  zenbook-s14-ux5406aa-303                 ssdt6.dat    OK
  zenbook-s14-ux5406aa-303                 ssdt7.dat    OK
  zenbook-s14-ux5406aa-303                 ssdt8.dat    OK
  zenbook-s14-ux5406aa-303                 ssdt9.dat    OK
zenbook-s14-ux5406aa-303 -> ('OK', 'dsdt.dat', '0x9a46', [7, 0])
tables walked to end: 93/93; sets decoded: 10/10
```
