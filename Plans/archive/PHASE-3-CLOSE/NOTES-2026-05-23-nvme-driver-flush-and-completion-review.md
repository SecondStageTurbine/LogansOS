# NOTES: NVMe driver flush/completion review (post BUILD-5b run #2)

- Author: PM (Cerebro/Claude)
- Date: 2026-05-23
- Status: Triage input. Not a dispatch. Not PL-ratified.
- Files reviewed:
  - `kernel/src/drivers/block/nvme.rs` (724 lines)
  - `kernel/src/drivers/block/ahci.rs` (lines 1–450)
  - `kernel/src/log/block_sink.rs` (post-degradation-visibility build, commit `2223617`)

## 1. Why this document exists

BUILD-5b run #2 (post-degradation-visibility kernel commit `2223617`) produced
the same evidence as run #1:

- Framebuffer: plain **FAIL** (red). NOT `FAIL LOG SELFTEST`.
- Extraction on power-cycled NVMe (`/dev/nvme0n1`, Intel E6100p):
  - Magic at `0x10000000` = `NEO_LOG_v1\0` ✓
  - `header_version` at `0x088` = `02` ✓ (this build's header)
  - `write_cursor` at `0x018` = `0`
  - `captured_bytes` from `neo-extract-log` = `0`
  - All four degradation fields = `0` (no degradation marker observed)
  - Hexdump of payload start (`0x10001000`) = all zeros

Plain FAIL means scenario reached its FAIL branch. The selftest gated install
on the framebuffer, so its absence in the framebuffer state means selftest
PASSED — selftest's in-band re-read returned `cursor > 0` and the sentinel
bytes. Yet on the power-cycled disk we see `cursor == 0` and no payload.

The selftest's re-read goes through the controller's view (DRAM cache OR
NAND, controller's choice). Power-off discards DRAM. So a successful
selftest is consistent with writes that landed only in the controller's
volatile cache and were never committed to NAND.

This NOTES doc records the driver review, the most plausible root cause,
and three candidate remediations, so we have ammunition to act on once
the serial console arrives 2026-05-24.

## 2. Hypothesis ladder

Ordered from most-to-least likely given the evidence:

**H1 — Intel E6100p does not honor NVMe FLUSH for durability (or honors it
slowly past our timeout).** Selftest sees writes via controller view; FLUSH
returns completion but cache is not actually committed to NAND before halt.
Power-cycle wipes DRAM cache, leaving NAND in whatever pre-run state it was.
The only reason `install()`'s header magic survives is that it was written
early enough for the controller's background flush (timer-based or idle-based)
to commit it before halt. Subsequent writes happened faster than background
flush could keep up.

**H2 — NVMe driver's command-timeout spin (`COMMAND_TIMEOUT_SPINS = 10_000_000`,
≈ 3 ms on a 3 GHz core) is too short for real-hardware FLUSH.** Consumer SSDs
can take 10–100 ms to flush their writeback cache under load. If the driver
times out, it returns `BlockError::Io` to `flush_locked`, which would trigger
`mark_degraded_locked` and write the degradation marker. But the marker write
itself goes through the same driver — if that also times out, we'd see no
marker on disk. This is consistent with the evidence (no marker observed).

**H3 — Stale completion from a prior timed-out submit poisons a subsequent
submit's CID match.** Driver returns `Err` on `completion_cid != cid` (line
505). If a FLUSH timed out earlier and the controller eventually posted its
completion, the next submit would see a CID mismatch and fail. Lower
likelihood because we'd expect the first failed submit to mark degraded.

**H4 — Something else entirely.** Scenario-side bug, framebuffer wiring,
extraction tool offset bug. Ranked low because we already verified magic
and `header_version=2` at the right offset on the extracted disk.

H1 is the leading hypothesis. H2 is a contributing factor. Both are
addressable with small driver changes.

## 3. NVMe driver review findings

### Finding 1 — `submit()` correctly waits for completion

Lines 470–520 of `nvme.rs`. After ringing the SQ doorbell, the driver
polls the CQ head entry's dw3 phase tag bit, advances `cq_head`, writes
the CQ doorbell, and returns `Ok(dw0)` on phase match with matching CID
and zero status. My earlier hypothesis (from the previous session) that
"flush returns before completion" is **wrong**. The driver does wait.

This means: when `flush_locked`'s `device.flush()` call returns `Ok`, the
NVMe controller has acknowledged the FLUSH command per NVMe spec. Per
spec, that acknowledgement means cache is committed to NAND. If reality
diverges from spec, that's a hardware/firmware behavior, not a driver
ordering bug.

### Finding 2 — No FUA bit on NVM_WRITE

Lines 444–449 of `nvme.rs`, `submit_data`:

```rust
cmd.set_cdw10((request.lba & 0xffff_ffff) as u32);
cmd.set_cdw11((request.lba >> 32) as u32);
cmd.set_cdw12((sector_count as u32) - 1);
```

`cdw12` is set only to `(sector_count - 1)`. Per NVMe Base Spec §6.15 NVM
Write Command (cdw12 layout), bit 30 of cdw12 is the **FUA (Force Unit
Access)** bit. When FUA=1, the controller must commit the write to
non-volatile media before posting completion — bypassing volatile write
cache entirely. We do not set this bit. Every log-sink write is therefore
free to land in the controller's DRAM cache.

This is the cleanest fix for H1: setting FUA on log-sink writes forces
durability per-write without depending on the controller's interpretation
of FLUSH.

### Finding 3 — Command timeout is fixed at ~3 ms

Line 69:

```rust
const COMMAND_TIMEOUT_SPINS: usize = 10_000_000;
```

This is a spin count, not a wall-clock value. On the OptiPlex's Comet
Lake CPU (~4.3 GHz turbo), `spin_loop` is roughly one cycle, so this is
~2.3 ms wall-clock. QEMU's emulated NVMe completes in microseconds, but
real consumer SSDs can take much longer for FLUSH:

- Samsung 860 EVO SATA FLUSH typical: 1–5 ms idle, 10+ ms under load.
- Intel E6100p NVMe FLUSH: undocumented for consumer firmware. Likely
  similar or worse, especially the first FLUSH after sustained writes.

A 3 ms ceiling is borderline for FLUSH. Increasing to e.g. 100 ms or
converting to a wall-clock-based timeout would eliminate this as a
factor.

### Finding 4 — No Volatile Write Cache disable at probe

The driver never issues `SET_FEATURES` for feature ID 0x06 (Volatile
Write Cache). Per NVMe Base Spec §5.27.1.6, setting VWC=0 disables the
controller's volatile write cache entirely. Every write becomes
synchronously durable at the cost of throughput. For BUILD-5b's
diagnostic purpose, this is acceptable. For production it's not.

This is Plan B if FUA-per-write proves insufficient or if the controller
ignores FUA (some consumer SSDs do).

### Finding 5 — Stale-completion handling is brittle

Lines 503–507:

```rust
let completion_cid = (dw3 & 0xffff) as u16;
if completion_cid != cid {
    return Err(BlockError::Io);
}
```

If a previously-timed-out command's completion lands during a later
submit's poll window, the CID mismatch fails the new submit. Better
behavior would be to drain stale completions (advance cq_head and
re-poll) instead of failing the new request. Low priority — only
matters if Finding 3 lets timeouts fire in the first place.

### Finding 6 — Polled-only completion blocks the kernel thread

MSI/MSI-X is configured (lines 230–240) but the interrupt handler just
increments a counter (line 290). All completion detection is synchronous
polling inside `submit()`. This is not a correctness bug; it's a
throughput/scheduling concern. Out of scope for the durability bug.

### Finding 7 — `next_cid` wraps to 0 then snaps to 1

Line 472:

```rust
self.next_cid = self.next_cid.wrapping_add(1).max(1);
```

CID 0 is valid per NVMe spec but the driver avoids it (probably as a
sentinel). Cosmetic.

### Finding 8 — AHCI driver is structurally equivalent

`kernel/src/drivers/block/ahci.rs` flush path (lines 336–339) issues
`ATA_FLUSH_CACHE_EXT` (0xea) and waits for `PxCI` bit clear with the
same `COMMAND_TIMEOUT_SPINS = 10_000_000`. Same trust in the device's
FLUSH honoring spec. Same timeout ceiling.

Implication: if H1 generalizes (consumer SSDs broadly under-honor
FLUSH), AHCI scenario writes to Samsung 860 EVO have the same exposure
on power-off. We have not seen evidence of that yet because the
scenario doesn't power-cycle and re-read its own AHCI writes. The log
sink is on NVMe, not AHCI, so we don't see AHCI durability in BUILD-5b's
post-mortem.

## 4. Candidate remediations

### Option A — Add FUA bit to log-sink writes (preferred)

Smallest, most targeted change. Either:

1. Add a `BlockOp::WriteDurable` variant or a `BlockRequest` flag, and
   plumb it through to set `cdw12 |= 1 << 30` on NVM_WRITE, OR
2. Hard-code FUA for all writes in the NVMe driver. Throughput impact
   is real but irrelevant for BUILD-5b.

For diagnostic purposes (option 2) is simpler. We can refine to (option 1)
later when throughput matters.

Estimated scope: ~10 lines in `nvme.rs`. No header changes. No bible
changes.

### Option B — Disable Volatile Write Cache at probe

Issue `SET_FEATURES` with feature ID `0x06`, value `0`, after Identify
Namespace. If the controller supports VWC disable, every write becomes
synchronously durable.

Estimated scope: ~15 lines in `nvme.rs::probe_pci`. Need to check
Identify Controller's VWC field (byte 525, bit 0) to confirm support
before issuing.

### Option C — Increase / harden the completion timeout

Bump `COMMAND_TIMEOUT_SPINS` to e.g. 1 billion (~250 ms wall-clock) or
convert to TSC-based wall-clock timeout. Pair with Option A or B; alone
this only addresses H2/H3.

Estimated scope: 1 line for the constant bump. Wall-clock conversion
is more invasive (need TSC calibration in kernel).

### Recommendation

Do not implement any of these tonight. The marginal value of code
changes before we have serial console output is low — we'd be patching
the most-likely hypothesis without confirming it. With the serial
console (arriving 2026-05-24), we can see:

- Whether the selftest actually emits its expected PASS line.
- Whether `flush_locked` reports degradation in its `serial_println!`
  even when the on-disk marker fails to land.
- The exact sequence of events between install, selftest, scenario
  start, scenario steps, and FAIL.

If serial output confirms H1 (writes succeed in driver, FLUSH returns
Ok, but post-mortem disk is still empty), implement Option A + Option C
together as a single Codex dispatch. ~20 lines, one acceptance run.

If serial output reveals a different bug, fix that first.

## 5. Sidebar: why `install()`'s header magic survives but nothing else

Best guess: the controller's background flush (idle-triggered or
periodic) committed install()'s header write to NAND between probe and
the scenario's first heavy I/O. After that, the scenario emitted a
burst of `serial_println` calls, which buffered into 4 KB staging, then
flushed in rapid succession — fast enough that background flush
couldn't keep up before kernel halt.

This is consistent with H1 and would be confirmed if a future run with
a long sleep after the last write (before halt) shows partial data
captured on the disk. Not worth testing as a standalone experiment;
the FUA/VWC-disable fix would prove the same thing more cleanly.

## 6. Items not addressed tonight

- AHCI durability under the same hypothesis (out of scope for this
  document; same fix pattern would apply).
- The `next_cid.max(1)` cosmetic.
- The polling-vs-interrupt-driven completion question (Finding 6).
- Whether `mark_degraded_locked`'s `serial_println` itself contributes
  to the recursive-flush guard via `IN_FLUSH` correctly. Spot-checked
  the swap/restore at line 268; looks correct.

## 7. References

- Commit `2223617` — kernel+tools: block-sink degradation visibility.
- Commit `bdc7a4f` — kernel+tools: block-device log sink.
- BUILD-5b execution guide: `Plans/BUILD-5b-EXECUTION-GUIDE-2026-05-22-optiplex-7080.md`.
- NVMe Base Spec 2.0c §6.15 (NVM Write Command, cdw12 FUA bit).
- NVMe Base Spec 2.0c §5.27.1.6 (Volatile Write Cache feature).
- ATA8-ACS §7.21 (FLUSH CACHE EXT).
