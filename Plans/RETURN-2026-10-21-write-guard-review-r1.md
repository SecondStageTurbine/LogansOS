<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: PR #85 write-guard hardening, second review (R1, pass 1)

**Date:** 2026-10-21
**Reviewer:** R1 (Claude review session)
**Reviewed head:** `6bfcfbeac5396e0e0584afdb82cdebe801e616a0` (`origin/claude/write-guard-hardening`)
**Base:** `2b3b509443dd8bc00de9838a64bd8bd4937a487b` (`origin/main`)
**Scope:** the eight checks in the R1 dispatch. Janus's F1 to F4 are not repeated here; S2 is fixing them.

> **This is a same-family review.** The author (S2) and this reviewer are both Claude. It adds to
> Janus's cross-family (Codex) review and does not replace it. Under CLAUDE.md §Cross-model diversity, a
> clean result from this review is weak evidence alone. The findings below are claims, each with a
> reproduction, and should be weighted on those reproductions.

**Disposition:** **Do not merge yet.** On top of F1 to F4 there are five new fails. Two of them
(R1-1 and R1-2) are cases where the partition-table rule **fails open**: a disk it cannot fully read,
or does not recognise, is treated as having no table. The dispatch requires those cases to be treated
as protected.

## New fails at a glance

| ID | Area | Severity | One line |
|---|---|---|---|
| **R1-1** | 4 partition table | **High** (spec violation, fails open) | An LBA-1 read error, a wiped primary GPT, or a whole-disk filesystem all give `Table::None`, so the claims are kept |
| **R1-2** | 4 partition table | Medium | On a hybrid MBR the MBR partitions are dropped once a GPT is found, so an MBR-only partition is unprotected |
| **R1-3** | 7 panic / fb | Medium | Another live CPU can switch the mirror off and paint over the panic, because nothing stops the other CPUs |
| **R1-4** | 6 xtask | Medium | `log-sink-header <out>` writes to whatever path it is given, including a raw disk device at offset 0 |
| **R1-5** | 6 xtask / kill-switch | Medium-low (defense in depth) | The guard-off control is refused on only one xtask path and has no kernel backstop |
| R1-6 | 2 markers | Low | When the primary NEOFS superblock is valid, a stale secondary copy can widen the armed extent past the volume |

Notes that are not fails (§Notes): 4 KiB handling in the drivers, seal versus provenance, and one runbook
wording issue.

---

## Checks, pass/fail with the enforcing line

### 1. Every write path goes through `GuardedDevice`: **PASS, except F3**

- Every device is wrapped when it registers, and only the wrapper is stored in `devices`
  (`kernel/src/drivers/block/mod.rs:331-338`). The only accessors hand out `devices` entries:
  `get_device` (`:343`) and `find_device_by_driver` (`:348`).
- `GuardedDevice` overrides both write entry points: `submit` (`:253-257`) and `write_block`
  (`:272-276`). `read_block` and `flush` pass through, and neither carries data to the disk.
- There are exactly four `impl BlockDevice` blocks: nvme, virtio_blk, ahci and the guard. No ramdisk,
  partition or loop wrapper exists that could re-export a raw device.
- The drivers keep private raw handles, but nothing writes through them:
  - `NVME_DEVICES` (`nvme.rs:286`) is never read.
  - `AHCI_DEVICES` is used only by `handle_interrupt` → `ack_interrupts` (`ahci.rs:285`).
  - virtio's `primary_device()` feeds `debug_submit_abandoned_write` (`virtio_blk.rs:528-548`). That is
    **F3**, the one exception.
- Non-data commands:
  - NVMe admin opcodes are limited to IDENTIFY, CREATE IO SQ/CQ and SET FEATURES (number of queues)
    (`nvme.rs:56-60`). There is no FORMAT, SANITIZE, DSM, write-zeroes or namespace management.
  - AHCI issues only IDENTIFY, READ/WRITE DMA EXT and FLUSH (`ahci.rs:70-73`).
  - virtio leaves `CONFIG_WCE` un-negotiated (`virtio_blk.rs:371`).
- Byte range: the guard's range equals the device's range. Every driver rejects a buffer that is not a
  whole multiple of the logical block (`nvme.rs:633`, `ahci.rs:500`, `virtio_blk.rs:602-604`), so the
  `len` checked at `mod.rs:180` is exactly what the controller writes.
- NEOFS (`fs/mod.rs:711, 914-941`) and the log sink both get their devices from the registry.

### 2. Marker validation: **PASS for "look-alike bytes never arm", with one low fail (R1-6)**

- **NEOFS:** checks the 8-byte magic, a BLAKE3 hash of bytes `0..480` against `480..512`, format v1,
  log2 block size 12, type v1, a non-zero count, and a checked multiplication
  (`write_guard_logic.rs:157-169`). The device-fit check is at `:362`.
- **Log sink:** checks the 11-byte magic, v3, a BLAKE3 seal over `0..0xfe0`, and a region size that is
  a multiple of 4 KiB, greater than 4096 and at most 1 GiB (`:181-192`). The fit check is at `:379-382`
  and cannot overflow because the size is capped at 1 GiB.
- **Scratch:** checks the full 32-byte magic, the exact device capacity, and the seal (`:221-225`).
- Bare magics do not arm. xtask's `prepare_unmarked_storage_disk` plants every pre-seal magic and the
  QEMU scenario expects a refusal.
- **R1-6 (low):** when the primary superblock at LBA 0 is valid, the table check is skipped (`:406-410`).
  Claims from copies 1 and 2 are de-duplicated only on an identical extent (`:369`). So a stale
  secondary copy with a larger `total_blocks` adds a second, wider claim. Probe P5 used a 65 MiB volume
  whose 64 MiB copy says 1 GiB, and it armed both `[0, 65 MiB)` and `[0, 1 GiB)`. The extra space is on
  a disk whose LBA 0 is already ours, so this is not a Windows exposure. The extent is still wider than
  the volume, though, and it covers the log region at 256 MiB.
  **Fix:** once the primary is valid, ignore the secondary claims. Otherwise require every valid copy to
  agree on `total_blocks`, and drop all NEOFS claims if they disagree.

### 3. Arming: **PASS**

- **Containment:** writes are checked with half-open containment (`:90-92`, `:119-121`).
- **Overflow:** `lba.saturating_mul(block)` and `start.saturating_add(len)` (`mod.rs:181-183`) saturate
  to `u64::MAX`, which no real claim contains, because every real claim ends at or below the device's
  capacity. Only the control's `[0, u64::MAX)` contains it.
- **Immutability:** the arming is set once through `spin::Once::call_once` (`mod.rs:167-169`), and there
  is no API that re-arms. A judge whose reads fail stays refused for the rest of the boot, which fails
  closed.
- **Independence:** each device gets its own `GuardedDevice` and its own `Once` (`mod.rs:331-336`).
- **NVMe namespaces:** only NSID 1 can be reached. IDENTIFY uses NSID 1 (`nvme.rs:203`), and so do every
  I/O command (`:444`) and FLUSH (`:417`). Other namespaces are never addressed.
- **Sector sizes:** see §Notes for 512 B versus 4 KiB.

### 4. Partition-table rule: **FAIL (R1-1, R1-2), plus F1**

**R1-1 — unreadable or unrecognised tables are treated as "no table", not as protected.**
`read_table` (`write_guard_logic.rs:244-285`) returns `Table::None` when LBA 0 has no `0x55AA` and no
`EFI PART` is read at LBA 1. That covers three cases the dispatch says must be protected:

- **(a) Read error at LBA 1.** `disk.read(ss, &mut hdr) && …` (`:273`) treats a failed read the same as a
  missing signature. With no MBR signature, the result is `Table::None` (`:279-280`).
  - **P1:** a Windows-shaped GPT with LBA 0 zeroed, LBA 1 unreadable, and a sealed sink header at
    256 MiB. Result: `Arming { claims: [LogSink 0x10000000..0x11000000], table: "none" }`.
- **(b) Wiped primary GPT with an intact backup.** The backup header in the last LBA is never consulted.
  - **P2:** the same disk with LBA 0 and LBA 1 zeroed and the backup header and entries left intact.
    Result: the sink arms with `table: "none"`.
- **(c) Whole-disk ("superfloppy") filesystems.** NTFS, FAT or exFAT with no partition table, or LUKS,
  BitLocker or ext4 on the raw device.
  - **P4:** an NTFS boot sector at LBA 0 (`EB 52 90 "NTFS    "`, `0x55AA`, all entry type bytes zero).
    Result: the sink arms with `table: "none"`.
  - Separately, the MBR parse does not sanity-check its entries (`:256-269`). It does not check that
    the status byte is 0x00 or 0x80, or that entries are in bounds and do not overlap. So a boot sector
    whose bytes `446..510` are code, not zeros, is accepted as `Allocated("mbr", garbage)` rather than
    treated as opaque.

**Realism.** The Zenbook's NVMe has a healthy GPT, so it is not exposed today. Each case needs a damaged
or unusual disk *and* a sealed marker at a probe offset. The partition rule is still the only control
that stops a *copied* sealed artifact from arming (see §Notes, seal versus provenance), and the
dispatch's requirement is that unknown or unreadable means protected. These cases break that.

**Fix (suggested):**
- Make a failed LBA-1 read `Opaque`.
- If there is no primary `EFI PART`, read the last LBA. If it holds `EFI PART`, the result is `Opaque`
  (or the backup is parsed after the F1 CRC checks).
- Make a recognised filesystem or container signature at LBA 0 or 1 `Opaque` unless the primary NEOFS
  superblock is valid. Candidates: a FAT/NTFS/exFAT BPB with a jump byte of EB or E9, `"NTFS    "` or
  `"EXFAT   "` at 3, ext magic `0xEF53` at 1080, `LUKS\xba\xbe` at 0, `-FVE-FS-` at 3, and LVM
  `LABELONE` at 512.
- Make any MBR entry with a status byte other than 0x00 or 0x80, or that runs past the device's
  capacity, `Opaque`.

**R1-2 — hybrid MBR partitions are discarded once a GPT is found.**
`read_table` collects `mbr_parts` (`:256-269`), but the GPT branch returns `read_gpt(...)` directly
(`:273-274`), and that result contains GPT extents only.

- **P3:** a hybrid MBR with a 0xEE entry over LBA 1..2047 plus a 0x07 entry over [200 MiB, 600 MiB), and
  a valid GPT listing only an ESP over [1 MiB, 100 MiB), with a sealed sink at 256 MiB. Result:
  `Arming { claims: [LogSink …], table: "gpt" }`. The NTFS partition that exists only in the MBR, and
  covers the region, is written.

**Fix:** when a GPT is accepted, push every MBR entry that is not 0xEE (and not type 0) into its
allocated set. In a hybrid MBR, the space either table allocates is allocated.

**What passes:**
- A valid GPT with 512 B or 4 KiB sectors blocks every claim (KATs `windows_gpt_disk_never_arms` and
  `gpt_with_4k_sectors_still_parses`).
- A protective MBR without a readable GPT is `Opaque` (`:276-277`).
- An entry with `last < first` is `Opaque` (`:336-337`).
- An implausible entry array is `Opaque` (`:295-302`).
- The size of a short or failed entry-array read is fixed by `array` (`:304`), and a failed read is
  `Opaque` (`:305-306`).

### 5. Read-path safety: **PASS, except R1-1(a)**

- Every probe read is bounds-checked before it is issued:
  - the NEOFS copies: `offset + 512 > cap` → skip (`:357`);
  - the log header: `0x10000000 + 4096 <= cap` (`:379`);
  - the scratch sector: `cap >= 512` and the read at `cap - 512` (`:394-395`);
  - the table reads: `cap < 2*ss` → none (`:247`, harmless because no claim fits a disk that small).
- `GuardReader::read` refuses `end > capacity` and `block == 0` (`mod.rs:134-136`). It reads aligned
  blocks in chunks of at most 64 KiB, and the copy-out arithmetic stays in bounds because capacity is a
  whole number of blocks.
- The `Disk::read` contract is all or nothing, and a failed read returns `false`.
- When a marker read fails, the marker is skipped and the disk is not armed. That fails closed. The one
  failed read that fails *open* is the LBA-1 read in R1-1(a).

### 6. xtask: **FAIL (R1-4, R1-5), plus F2**

- **`log-sink-header` bytes: PASS.** `provisioning_log_header` is the same function the kernel uses to
  validate (`write_guard_logic.rs:198-207`). The KAT `provisioned_log_header_bytes` checks it against a
  hand-laid byte oracle.
- **Nothing in xtask writes a host disk: FAIL (R1-4).** `cmd_log_sink_header` calls
  `fs::write(out, header)` (`tools/xtask/src/main.rs:4330`) with no check on what `out` is. If `out` is
  a block device, the call opens it for writing and puts 4096 bytes at **offset 0**. On a GPT disk that
  overwrites the protective MBR and the primary GPT header and entries.
  - On Linux this needs root.
  - On Windows, an administrator shell with `\\.\PhysicalDrive0` can do it, because sector 0 lies outside
    every mounted volume, so Windows does not block the write.
  - The printed recipe puts `<out>` next to `of=/dev/<log-disk>`, which makes the mix-up easy.
  - **Repro:** `cargo run -p xtask -- log-sink-header /dev/null` reports
    `wrote /dev/null: sealed NEO_LOG_v1 v3 header …`. The path is accepted with no file-type check.
  - **Fix:** refuse any `out` that already exists and is not a regular file, and refuse `/dev/…` and
    `\\.\…` prefixes. Use `OpenOptions::new().write(true).create_new(true)` or `create` + `truncate`,
    then check `metadata()?.is_file()` before writing.
- **Bare-metal refusal: FAIL beyond F2 (R1-5, defense in depth).** The only refusal is the string check
  on `NEO_TEST_EXTRA_FEATURES` inside `build_test_kernel` (`main.rs:4449-4456`). Two other paths build a
  guard-off, bootable image with no refusal:
  - `cargo xtask build|iso|run --features neo-test-block-write-guard-control`
    (`parse_build_options` `:4394-4413` → `build_kernel_default` `:4421` → `build_member`, and
    `cmd_iso` `:5113-5118`);
  - `neo-test-build` in VMware (non-bare-metal) mode.

  The kernel itself has no backstop: the control `judge` arms `[0, u64::MAX)` on **every** device,
  NVMe included (`mod.rs:227-241`).

  **Why this is defense in depth, not a live exposure:**
  - The control feature pulls in only the `block_write_guard` scenario, which reads LBA 0 and aborts
    with `guard_seed_missing` before it writes anything on a disk without its seed
    (`test_harness.rs:18480-18483`).
  - The bare-metal sink still refuses a disk without a sealed header (`block_sink.rs:149-159`).

  So today's protection comes from those two checks, not from the refusal.

  **Fix:** add two kernel-side guards. Together they also close F2 at the source, whatever spelling xtask
  receives:
  - `#[cfg(all(feature = "neo-test-block-write-guard-control", any(feature = "neo-test-bare-metal-mode", feature = "fb-progress-console")))] compile_error!(…)`;
  - make the control arm only devices whose `identity().driver == "virtio-blk"`, which exists only under
    QEMU.

### 7. fb console / panic path: **FAIL (R1-3), plus F4**

- **Ordinary drawing does not deadlock: PASS.**
  - `try_with_fb` uses `try_lock` (`fb/mod.rs:147-148`).
  - `write_fmt` uses `CONSOLE.try_lock` and counts the newlines it loses (`progress.rs:105-110`).
  - `stop` clears `ACTIVE` before the `FB` barrier (`progress.rs:65-68`), and draws re-check `ACTIVE`
    under `FB` (`:164-167`, `:124`).
  - The stop barrier `FB.lock()` (`fb/mod.rs:164`) runs only on the true→false transition. It runs from
    `take_screen` on the caller's thread, which does not hold `FB` at that point.
- **The force-unlock is not safe on SMP, and the panic message can be lost: FAIL (R1-3).** `on_panic`
  does not stop the other CPUs. No stop IPI or NMI exists: `arch::halt_forever` halts only the current
  CPU. That has two consequences.
  1. **Another CPU erases the mirror.** `panic_takeover` sets `ACTIVE = true` (`progress.rs:86`). Then
     the next draw by a still-running CPU (an Arthur or desktop frame, a splash) calls `take_screen` →
     `progress::stop()` (`fb/mod.rs:185-190`), which turns the mirror back off and paints over the
     screen. The panic lines after that are not drawn. In the product build the magenta bar's hint then
     says "the panic message is in the log above", which can be false. This is the Zenbook case: the
     screen is the only channel.
  2. **The force-unlock is unsound while the holder is still alive.** `CONSOLE.force_unlock()` and
     `FB.force_unlock()` (`progress.rs:80-82`, `fb/mod.rs:172-176`) are applied while another CPU may
     hold them. The two CPUs then mutate `Console` (cursor, `pending_clear`) at the same time, which is
     formally UB. When the holder drops its guard, it also releases the lock the panicking CPU just took.
     In practice the damage is garbled text, not memory corruption: `FbState` is replaced only at
     `init`, and `fill` and the glyph writes are clamped (`fb/mod.rs:126-139`).
- **The mirror is the last sink in `_print` (this generalizes F4).** `serial::_print` takes `SERIAL`,
  then calls `crate::log::write_fmt` (`serial.rs:49`), and only then the fb mirror (`:52`). The block
  sink's `append` can fill staging and flush, which means block I/O, a driver lock, and on the first
  write the guard's `spin::Once` judge. If the panic interrupted any of those on this CPU (a driver
  `inner` lock, or a judge in progress, whose `Once` waiters spin), the screen never gets the message.
  F4 names only `SERIAL`; S2's fix should cover the whole chain.
- **Panics are not guarded against recursion.** A panic inside the takeover or the console runs
  `on_panic` again and recurses until a double fault.
- **Fix (suggested):**
  - Add a `PANICKING: AtomicBool` that `on_panic` sets first. A second entry halts at once.
  - Make `progress::stop()` and `take_screen` no-ops while it is set, so no other CPU can switch the
    mirror off.
  - Have `on_panic` draw the message through the progress console *directly* (and first), before
    `serial_println!`, so the screen does not depend on SERIAL or the sink.
  - Optionally, send other CPUs a stop IPI before breaking the locks. With the latch in place, the
    force-unlock is needed only for a lock this CPU died holding.

### 8. Kill-switches: **PASS (by inspection)**

- **The guard is unconditional.** `register_device` always wraps (`mod.rs:331-338`). The only switch is
  the control's `cfg` on `judge` (`:197`, `:227`), and its exposure is R1-5.
- **`fb-progress-console` off matches the previous behaviour.** Every piece of new fb code is behind
  `cfg(feature = "fb-progress-console")`:
  - `progress` / `progress_logic` (`fb/mod.rs:28-31`);
  - `try_with_fb`, `console_barrier`, `steal_fb`, `OVERLAY_OWNER` and `overlay_tag` (`:146`, `:162`,
    `:172`, `:196-200`);
  - the body of `take_screen` (`:186`) and both sides of `overlay`;
  - the call in `serial.rs:51-52`;
  - the takeover in `panic.rs:32-36`.

  The panic hint uses `cfg!(feature = …)`, which folds to the original string when the feature is off.
- **Not measured:** I did not build both kernels and compare the bytes. The claim rests on reading the
  code. A byte comparison against `main` would differ anyway, because the block-guard changes are not
  behind that feature.

---

## Notes (not fails)

- **512 B versus 4 KiB in the kernel.** The 4 KiB GPT KAT tests the logic only. Every kernel driver
  fixes the logical block at 512.
  - NVMe *refuses* any other namespace format at probe (`nvme.rs:216`), so a 4Kn NVMe is never
    registered. That fails closed.
  - AHCI ignores IDENTIFY words 106/117 (`ahci.rs:479-490`), so a 4Kn SATA disk would get 512-byte
    geometry. Judge and write are scaled the same way, so the guard is not bypassed. Reads would most
    likely hit PRDT overflow, which fails closed as opaque. Still, it should refuse the disk the way NVMe
    does.
  - virtio always uses 512.
- **A seal authenticates format, not provenance.** BLAKE3 stops accidental look-alikes. It does not stop
  a *copy* of a real sealed artifact: the file `cargo xtask log-sink-header` writes, or a NEOFS image
  stored as a file. Such a copy can land on a probe offset inside a partition. The partition rule is what
  blocks it, which is why R1-1 and R1-2 matter. Also, a kernel write inside an armed extent (sink text,
  NEOFS file data) can mint a marker. That only matters on a disk that is already ours: it would need,
  for example, a log region ending at the device's last sector on a disk with no table. It is
  defense-in-depth only.
- **Runbook wording.** `docs/runbook-baremetal-zenbook-s14.md` says a Windows GPT is "allocated from its
  first usable LBA to its last". That is not true in general, because of tail slack and shrunk volumes.
  The safety argument holds because, on standard layouts, 256 MiB falls inside the ESP, MSR or C:.
  Suggested wording: "the 256 MiB probe lies inside a Windows partition, and any claim overlapping LBA 0
  or a partition is dropped."

## Verification

- `cargo test -p block-guard-logic-tests`: 11 passed (current head, with the probes reverted).
- **Temporary R1 probes** P1 to P5 were added to `tools/block-guard-logic-tests/src/lib.rs` and run, then
  reverted. The tree was clean before this commit. All five failed in the unsafe direction:
  ```text
  P1 Arming { claims: [LogSink 268435456..285212672], blocked_by_table: 0, table: "none" }   # LBA1 read error
  P2 Arming { claims: [LogSink 268435456..285212672], blocked_by_table: 0, table: "none" }   # primary GPT wiped, backup intact
  P3 Arming { claims: [LogSink 268435456..285212672], blocked_by_table: 0, table: "gpt"  }   # hybrid-MBR partition dropped
  P4 Arming { claims: [LogSink 268435456..285212672], blocked_by_table: 0, table: "none" }   # superfloppy NTFS
  P5 Arming { claims: [NeoFs 0..68157440, NeoFs 0..1073741824], blocked_by_table: 0, table: "none" }  # stale secondary widens
  ```
  The probe sources are summarised in the findings above. Each one makes a good regression KAT for S2's
  fixes. Every assertion has the form "`claims.is_empty()`" (P5: "every claim ends inside the primary
  volume").
- `cargo run -p xtask -- log-sink-header /dev/null`: accepted, and the path had no file-type check
  (R1-4).
- No QEMU run. This review covers code and host logic only.
- RAG: not used. This is a cloud reviewer session, and the work was direct reads and greps.

## Pass 2 (addendum)

**Reviewed head:** `8ce803101b22a0617ae03c7773a30c93b8a43348` (`claude/write-guard-hardening`; it includes
`origin/main` `d4244be5`, CP-68 x2APIC, which is outside the scope of this review).
**Delta reviewed:** `6bfcfbea..8ce80310`, limited to the write-guard, panic, fb, serial and xtask changes.
Line numbers below are at `8ce80310`.
**Still a same-family review** (Claude reviewing Claude). This round's cross-family reviewer is Muse; this
pass adds to that review and does not replace it.

**Disposition:** **F1–F4 and R1-2 to R1-6 PASS.** R1-1 is **PASS for every case in pass 1**, but the
whole-disk filesystem detection is a list of known signatures, and seven more formats still arm through
it. That residual is **R1-7, low**. Whether R1-7 blocks the merge is the PM's call. It does not affect
the Zenbook: its NVMe is GPT, and the NTFS, FAT, exFAT and BitLocker signatures are all detected. No
QEMU run was made here. The 4-shard gate on this SHA is still the PM's merge evidence.

### Janus F1–F4

| ID | Verdict | Enforcing line(s) |
|---|---|---|
| **F1** (GPT trusted without CRCs) | **PASS** | `validated_gpt` (`write_guard_logic.rs:434-529`) requires all of these: the signature; a header size from 92 to 512 bytes that fits a sector (`:445-448`); the header CRC32, computed with the CRC field zeroed (`:449-454`); `my_lba == lba`, `2 <= first_usable <= last_usable <= last_lba`, and the header outside the usable range (`:464-475`); an entry array on the device, clear of the header and the usable range (`:478-485`); the entry-array CRC32 (`:486-491`); and every partition inside the usable range (`:520-522`). If the primary fails, the backup in the last LBA is validated on its own (`:337-349`). A GPT signature or protective MBR with no copy that validates is `Opaque` (`:355-358`). Janus's moved-entry-array repro is now the KAT `f1_a_moved_entry_array_with_stale_crcs`, alongside `f1_b`…`f1_e`. The CRC32 itself passes the check value `crc32(b"123456789") == 0xCBF43926`, in KAT `crc32_known_answers` and my own probe. |
| **F2** (feature-spelling bypass) | **PASS** | There are three layers: (1) `feature_disables_write_guard` matches the last `/` segment after trimming (`xtask main.rs:4552`); (2) the kernel's `compile_error!` fires when the control is combined with `neo-test-bare-metal-mode`, `product` or `fb-progress-console` (`block/mod.rs:403-415`), so it keys on the *resolved* feature set whatever the spelling; (3) bare-metal bundles are scanned for `GUARD_OFF_MARKER` (`main.rs:6830`). KATs: `guard_control_feature_in_every_spelling`, `guard_control_feature_in_a_list` and `guard_off_image_is_refused`. |
| **F3** (raw virtio write) | **PASS** | `debug_submit_abandoned_write` now asks the same guard, `guard_allows_raw_write` (`block/mod.rs:366`), before it submits (`virtio_blk.rs:553`), and it is not compiled into bare-metal builds. The guard is looked up by full `BlockDeviceIdentity`, which includes the BDF, so it is the matching device's guard. |
| **F4** (panic blocks on `SERIAL`) | **PASS** | The panic handler calls `enter_panic` (`panic.rs:42`). After that, `_print` goes to `print_panicking` (`serial.rs:111`). That function draws on the screen first, then takes the UART only through `SERIAL.try_lock()`, falling back to a lock-free raw COM1 handle (`serial.rs:58-81`), and touches the log sink last. The QEMU scenario `fb_panic_holding_serial` asserts at least 20 panic glyph cells while SERIAL is still held, and its control must time out. |

### R1-1 … R1-6

| ID | Verdict | Enforcing line(s) / evidence |
|---|---|---|
| **R1-1(a)** LBA-1 read error | **PASS** | `read_gpt_header` turns an unreadable sector into `Some(zeroed)`, so it counts as "a GPT seen". Validation then fails and the result is `Opaque` (`write_guard_logic.rs:420-427`, `:355-358`). KAT `r1_p1_unreadable_primary_header`. An unreadable LBA 0 is `opaque-unreadable` (`:300-302`). |
| **R1-1(b)** wiped primary, backup intact | **PASS** | The backup is read when the primary is absent (`:337-349`). KAT `r1_p2_wiped_primary_backup_intact`. |
| **R1-1(c)** whole-disk filesystems | **PASS for the cases in pass 1; residual R1-7** | `whole_disk_contents` (`:378-416`) catches an x86 jump, NTFS, exFAT, BitLocker, FAT12/16/32, LUKS, any `0x55AA`, LVM, ext, and ISO 9660, and turns a failed probe read into `Opaque`. Also, `0x55AA` over an empty table is `Opaque` (`:396-398`). KAT `r1_p4_whole_disk_volumes_are_opaque`. |
| **R1-1** malformed MBR | **PASS** | A status byte other than 0x00 or 0x80, `len == 0`, or `start + len > total` gives `opaque-mbr-invalid` (`:320-323`, `:359-361`). The arithmetic is u32 + u32 in u64, so it cannot overflow. KAT `r1_malformed_mbr_is_opaque`. |
| **R1-2** hybrid MBR | **PASS** | On a valid GPT, `out.extend(mbr_parts)` (`:350-353`) allocates the non-0xEE MBR entries too. KAT `r1_p3_hybrid_mbr_partition_is_kept`. |
| **R1-3** panic mirror erased / force-unlock with live holders | **PASS** | (1) A `PANIC_CPU` compare-exchange elects the first panicking CPU; any later panic, on this CPU (recursion) or another, returns `false` and halts (`progress.rs:129-135`, `panic.rs:36-38`). (2) Every other CPU halts at its next `take_screen` (`fb/mod.rs:195` → `progress.rs:109-118`). `halt_forever` is `cli; hlt`. (3) After a panic, `stop()` is a no-op, so nothing can switch the mirror off, including `main.rs:793` (`progress.rs:66-70`). (4) Only the panicking CPU writes to the screen (`progress.rs:173`). (5) Before breaking the locks, the takeover waits up to `LOCK_WAIT_SPINS = 2_000_000` (`:82`, `:136-142`) for a holder on another CPU. The scenario's CPU-1 drawer must stop, and the mirror must stay active (`test_harness.rs` `fb_panic_holding_serial_verdict`), with a control that must FAIL `other_cpu_still_drawing`. Residuals are below. |
| **R1-4** `log-sink-header` writes a device | **PASS** | `write_regular_file` (`xtask main.rs:4368-4407`) applies four checks: (1) it refuses `/dev/`, `\\.\` and `\\?\` paths (`:4371`); (2) it refuses an existing path that is not a regular file, and `metadata` follows symlinks, so a symlink to a device is caught (`:4377`); (3) it refuses to overwrite an existing file without `--force` and otherwise uses `create_new`; (4) after opening, it re-checks `is_file()` on the open handle before writing (`:4401`). A block device opened with `O_TRUNC` is not truncated, and the re-check refuses it before any byte is written. KAT `log_sink_header_writes_only_regular_files`. |
| **R1-5** control has no kernel backstop | **PASS** | The control arms only `driver == "virtio-blk"` (`block/mod.rs:241`). Every other device is judged as usual, so a guard-off ISO booted on the Zenbook arms nothing on its NVMe. The `compile_error!` covers bare-metal, product and the fb progress console (`:403-415`). |
| **R1-6** stale NEOFS secondary widens the claim | **PASS** | A valid primary alone sets the extent. Without one, the valid copies must agree, or no NEOFS claim is made (`write_guard_logic.rs:540-566`). KAT `r1_p5_neofs_primary_alone_sets_the_extent`. |

### New, in the delta

**R1-7 (low): the whole-disk filesystem detection is a list of known signatures, so an unlisted format
reads as "no table".** `read_table` returns `Table::None` whenever `whole_disk_contents` recognises
nothing (`write_guard_logic.rs:369-372`). A temporary probe put a sealed sink header at 256 MiB on a
table-less disk and added one real magic at its real offset. These formats all armed the sink
(`table=none claims=1`):

- XFS: `XFSB` at 0;
- btrfs: `_BHRfS_M` at 0x10040;
- HFS+: `H+` at 1024;
- APFS: `NXSB` at 32;
- Linux md 1.2: `a92b4efc` at 4096;
- f2fs: `0xF2F52010` at 1024;
- ReFS: `ReFS\0\0\0\0` at 3. My understanding is that the ReFS boot record has neither an x86 jump nor
  `0x55AA`, but confirm that against raw bytes before relying on it.

ZFS and Linux swap are also absent from the list. To be exploitable, a disk needs **no** partition table
*and* a sealed marker copy sitting at an exact probe offset inside that filesystem's data. None of these
is a Windows format, and Windows always partitions its disks. That is why this is low.

But the rule stated in the dispatch was "unknown → protected", and a list of known signatures cannot
deliver that. **Fix options, cheapest first:**
- (a) Add the seven signatures above, plus ZFS (`0x00bab10c` uberblocks at 128 KiB) and swap
  (`SWAPSPACE2` at 4086).
- (b) Turn the check around for the one claim that can arm on a table-less disk without owning LBA 0,
  the log sink: on a `Table::None` disk, arm it only if the disk's head (for example LBA 0 to 2) is all
  zero, or carries xtask's own seed text.

  NEOFS-primary and scratch claims are unaffected: NEOFS owns LBA 0, and scratch is bound to the exact
  capacity.

**Residuals in R1-3, noted but not failed:**
- **The panic latch depends on CPUs having distinct APIC IDs.** `cpu_apic_id` reads CPUID leaf 0xB
  (`progress.rs:86-92`) whenever the maximum leaf is at least 0xB. On a CPU or hypervisor that reports
  max leaf ≥ 0xB but returns zeros for leaf 0xB, every CPU reads ID 0. `panicked_elsewhere` is then
  false on every CPU, so the other CPUs are not halted and can paint over the panic. The mirror itself
  stays on, because `stop()` keys on `panicking()`, not the CPU ID.

  Intel silicon (the Zenbook) implements leaf 0xB, so this does not affect the Zenbook. A cheap hardening
  step: fall back to leaf 1 when `leaf 0xB ebx[15:0] == 0`, the SDM's signal that the leaf is invalid.
- **An in-flight frame can outlast the bounded wait.** `LOCK_WAIT_SPINS = 2_000_000` iterations of
  `pause` take roughly 5 to 300 ms depending on the core. A CPU in the middle of a long blit (a 4K
  compositor frame) may outlast that, so its one in-flight frame can land over the first panic lines
  after the force-unlock. The next `take_screen` on that CPU halts it. The damage is bounded to one
  frame.
- **The log sink still runs on the panic path.** `print_panicking` puts the screen and UART first, then
  `crate::log::write_fmt` (`serial.rs:81`). The sink only try-locks `SINK`. But if its staging fills, it
  flushes through the block driver, and that would hang if this CPU panicked while holding the driver's
  lock. Each line is drawn on screen *before* its sink append, so the worst case is that lines after the
  hang point are lost, not the first one.

  This does not affect the Zenbook, where the sink refuses the Windows disk and is never installed.
  Optional hardening: on the panic path, append without flushing.

**Checked and clean in the delta:**
- **GPT backup extents when the backup copy is in use** (`write_guard_logic.rs:498-513`): everything past
  `last_usable` is allocated.
- **Partitions outside the usable range** now make the table `Opaque` (`:520-522`).
- **A stale `EFI PART` in the last LBA of a plain MBR disk** is validated and allocated, or else made
  `Opaque`. Both are conservative.
- **Every new probe read is bounds-checked:** whole-disk signatures at `:405-407`, and `read_gpt_header`
  through `Disk::read`. A failed read means `Opaque`.
- **The `GUARD_OFF` static and marker** are compiled only into the control (`block/mod.rs:398-401`).

### Verification (pass 2)

- `cargo test -p block-guard-logic-tests`: **25 passed** at `8ce80310`. That suite includes my pass-1
  probes P1–P5, now adopted as KATs.
- `cargo test -p xtask`: **47 passed**.
- **Temporary R1-7 probe** (seven signatures) and a CRC32 check-value probe were added and run, then
  reverted. The R1-7 probe failed as quoted above; the CRC32 check value matched.
- **Not run:** QEMU, including `fb_panic_holding_serial`, its two controls, `block_write_guard`, and the
  virtio abandoned-write scenario. The 4-shard gate on `8ce80310` is the evidence for those.
