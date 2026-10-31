# Dispatch: Block-Device Log Sink + Framebuffer Progress Indicator for Bare-Metal Scenarios

**Date:** 2026-05-22
**Recorder:** PM
**Dispatched to:** Jarvis (Codex)
**Authority:** PL (verbal, 2026-05-22)
**Status:** ACTIVE — PL signed off 2026-05-22 (worker: Codex/Jarvis, framebuffer indicator larger, strict-halt policy).
**Authority chain:**
- `Plans/PROCEDURE-2026-05-22-phase-3-build-5b-bare-metal-optiplex.md` (parent procedure; this dispatch unblocks its execution)
- `Plans/PHASE-3-BARE-METAL-OPTIPLEX-7080-BOOT-SMOKE-EVIDENCE-2026-05-22.md` (predecessor: target boots, framebuffer reaches end of boot path)
- Bible Article 13 (verification requirements)
- Bible Article 14 (algorithm: OBSERVE / THINK / PLAN / BUILD-VERIFY)

---

## 1. Why This Exists

BUILD-5b bare-metal storage validation on the OptiPlex 7080 Micro is blocked
by absence of a serial capture path on the target hardware. The Micro chassis
has no rear DB9 port, no internal COM header populated, no PCIe expansion
slot, and no exposed UART debug header. Dell's optional serial add-on module
is purchasable but requires ~1 week shipping.

The kernel already has working AHCI and NVMe block-device drivers (BUILD-5a
PASS, VMware-verified 2026-05-21). The hardware is two healthy SSDs: a
Samsung 860 EVO 250GB on AHCI (SATA-0) and an Intel SSD E6100p Series 256GB
on NVMe (M.2 PCIe SSD-3). Using one as scenario-disk while the other absorbs
log output gives us full-log evidence on bare metal without serial.

A small framebuffer phase indicator alongside the log sink lets the operator
distinguish "still running" from "hung" in real time without serial access.

## 2. Goal

Add two small capabilities to the kernel and one host-side extraction tool
such that BUILD-5b can execute on the OptiPlex 7080 Micro with no serial
hardware and produce evidence of comparable rigor to
`PHASE-3-VMWARE-CLEAN-COPY-RERUN-EVIDENCE-2026-05-21.md`.

The two capabilities are:

- **(L) Block-device log sink** — a kernel log destination that writes log
  output to a configurable region on a block device for post-run extraction.
- **(F) Framebuffer phase indicator** — a small dedicated screen region that
  shows the current scenario phase as plain-text large-font output, updated
  by the scenario harness at well-defined transitions.

## 3. Scope

In scope:

- Kernel code: `BlockDeviceLogSink` type and its registration as an
  additional log destination alongside the existing serial sink.
- Kernel code: framebuffer phase indicator rendering routine and a
  `kernel::scenario::phase()` callable that updates it.
- Scenario harness integration: kernel command-line argument or compile-time
  flag that selects the log-sink target device + offset, and a corresponding
  build-system hook so `cargo neo-test --build-only <scenario>` produces
  bare-metal-mode ISOs with the sink wired up.
- Host-side extraction tool: a small Rust binary or shell script under
  `tools/neo-extract-log/` that reads the magic header from a target device
  region and writes the captured log to a file.
- Unit tests for the sink's framing logic.
- A short README at `tools/neo-extract-log/README.md` documenting the
  extraction procedure.

Out of scope (defer to follow-up work if needed):

- USB stack of any kind.
- Network-based log capture.
- Live log streaming during execution (post-run extraction only).
- Log compression.
- Log rotation (the region is sized once at build time, never wraps).
- Multi-target log sinks (one block device per run is sufficient).
- Persisting kernel logs across reboots in production (this is for scenario
  runs only).
- Any change to existing scenarios' PASS/FAIL semantics or their serial
  output format. The block-device sink is additive.

## 4. Detailed Requirements

### 4.1 Block-Device Log Sink (L)

**Interface.** A new module `kernel/src/log/block_sink.rs` exposing:

```rust
pub struct BlockDeviceLogSink {
    device: BlockDeviceHandle,
    region_offset: u64,      // byte offset on the device
    region_size: u64,        // total bytes available; never written past
    write_cursor: AtomicU64, // current write offset within the region
}

impl BlockDeviceLogSink {
    pub fn new(device: BlockDeviceHandle, region_offset: u64, region_size: u64) -> Result<Self>;
    pub fn install(self) -> Result<()>;  // registers as a kernel log destination
}
```

**Region layout.** The region begins with a 4 KiB header page:

```
offset 0x000  : magic     = b"NEO_LOG_v1\0" (11 bytes, NUL-padded to 16)
offset 0x010  : region_size_bytes : u64 little-endian
offset 0x018  : write_cursor      : u64 little-endian (updated on each flush)
offset 0x020  : kernel_build_id   : [u8; 32] (the existing kernel build hash)
offset 0x040  : scenario_name     : [u8; 64] (NUL-padded ASCII)
offset 0x080  : start_timestamp_ns: u64 little-endian
offset 0x088  : reserved_zero     : remaining bytes of the header page
offset 0x1000 : start of log payload
```

The header is rewritten on every flush so `write_cursor` reflects how much
of the log is valid. The extraction tool reads the header, then reads
`write_cursor` bytes starting at `0x1000`.

**Buffering.** Lines are batched in a kernel-owned 4 KiB staging buffer.
The staging buffer is flushed to the device when:
- it is full;
- a log line at log-level ERROR or higher is emitted (forces immediate flush);
- the scenario harness calls `kernel::log::flush()` at a phase transition;
- the scenario emits its terminal PASS or FAIL line.

**Failure behavior.** If the block-device write fails, the sink logs a
single error line to the serial destination (which is benign on bare metal
where no serial reader is attached, and visible in VMware/QEMU where serial
IS attached) and sets a "sink degraded" flag. Subsequent writes are
attempted but not retried more than once per line. The kernel does not
panic on log-sink failure.

**Log recursion silencing.** The block-device write path internal to the
sink (block-device handle dispatch, completion handling, etc.) MUST NOT
itself emit log lines while inside the sink's flush operation. The sink
holds a per-CPU "in flush" flag during its write; any log emission from
inside that flag is dropped silently. This prevents infinite recursion if
the underlying block driver logs anything during sink-initiated writes.
The drop is recorded as a counter and emitted as a single summary line
on sink degraded transition, never inline.

**Region size.** Default 16 MiB. Configurable per scenario at build time.
16 MiB at ~80 bytes per log line gives headroom for ~200 000 lines, which
is dramatically larger than any current scenario emits.

**Region offset.** Default `0x10000000` (256 MiB into the device). This
leaves the first 256 MiB available for the scenario fixture if needed and
puts the log region in well-aligned, predictable territory.

**Concurrency.** The sink is `Send + Sync`. Internal writes use a spinlock
around the staging buffer; the device handle is assumed to be itself
thread-safe per the existing block-device contract. No new lock-ordering
constraints introduced.

### 4.2 Framebuffer Phase Indicator (F)

**Interface.** A new module `kernel/src/scenario/phase_indicator.rs` exposing:

```rust
pub enum ScenarioPhase {
    Boot,
    EnumerateDevices,
    MountSource,
    WritePhase,
    ReadbackPhase,
    Fsync,
    Cleanup,
    Pass,
    Fail(&'static str),  // short reason
}

pub fn set_phase(phase: ScenarioPhase);
```

**Render location.** A reserved rectangle, default 800 × 200 pixels,
positioned in the lower-right of the framebuffer (right edge and bottom
edge aligned to the framebuffer extent, no margin). The rectangle is
cleared to a solid color (default: dark blue) and the current phase text
is rendered centered in large monospace font (the existing kernel font
scaled 8×, giving 64×128-pixel characters; up to 12 characters of phase
text fit horizontally with margin). Sized for readability at ~2 m
standing distance from a typical 1080p monitor.

If the framebuffer is smaller than 1280×800, the rectangle may be reduced
proportionally but must never go below 640×160 with 6×-scaled font. If the
framebuffer is larger, the rectangle remains at 800×200 (it does not
need to grow with display size).

`Pass` renders the rectangle solid green with the text `PASS` in white.
`Fail(reason)` renders the rectangle solid red with `FAIL: <reason>` in
white. All other phases render the rectangle dark blue with the phase
name in white. Color values may be tuned in code review; the requirement
is high-contrast and visually distinct between phases.

**Update timing.** `set_phase` is callable from any context that can
already write to the framebuffer. It must be fast enough to call at every
scenario phase transition without measurably affecting scenario timing
(target: under 1 ms per call).

**Concurrency.** The phase indicator's framebuffer region is reserved at
init; no other framebuffer code may render into the same rectangle.

### 4.3 Scenario Harness Integration

Extend `tools/xtask` or wherever the scenario build mechanics live (per
`Plans/PROCEDURE-2026-05-20-phase-3-vmware-scenario-harness.md`) so that:

- `cargo neo-test --build-only <scenario> --bare-metal-mode` produces an
  ISO whose kernel is configured to:
  - Install the block-device log sink against the *non-scenario* device
    at offset `0x10000000` with a 16 MiB region;
  - Wire `kernel::scenario::phase()` calls into the existing scenario
    code at every documented phase transition.
- The two existing exemplar scenarios are updated to call `phase()` at
  their phase transitions. Other scenarios may be left unchanged for this
  dispatch; they remain VMware-only.

Scenario-to-device pairing (the harness picks the log-sink target as
"whichever block device the scenario does NOT exercise"):

| Scenario | Scenario disk | Log sink target |
|---|---|---|
| `phase3_ahci_neofs_mount_and_readwrite` | SATA / AHCI | NVMe |
| `phase3_nvme_neofs_mount_and_readwrite` | NVMe | SATA / AHCI |

If only one of the two block devices is present at runtime, the kernel
strictly halts: it emits a startup ERROR to the framebuffer phase
indicator (`Fail("no log sink device available")`) and does NOT proceed
to run the scenario. The framebuffer indicator stays at the failure
state until power-cycle. This is intentional — a bare-metal scenario
that runs without log capture produces no evidence of value, and silent
degradation here would defeat the dispatch's entire purpose. There is
no fallback to serial-only mode on bare metal.

### 4.4 Host-Side Extraction Tool

A new tool at `tools/neo-extract-log/` (Rust binary or shell script;
implementer's choice based on what fits the existing tools/ conventions).

Behavior:

```
$ neo-extract-log --device /dev/sda --output ahci-scenario.log
Found NEO_LOG_v1 header at offset 0x10000000
Scenario: phase3_ahci_neofs_mount_and_readwrite
Start: 2026-05-22T20:14:33Z
Captured bytes: 184224
Wrote ahci-scenario.log
```

Validates the magic header. Refuses to extract if the magic does not match
or the build_id field is zeroed. Outputs a plain-text log file.

### 4.5 Tests

- Unit tests for the header serialization/deserialization round-trip.
- Unit test for the staging-buffer flush logic (no losses across batched
  writes).
- Unit test for the "device write fails" path (sink degrades, does not
  panic).
- Integration test under QEMU or VMware: a scenario runs with the
  block-device sink configured, extracts the log post-run, asserts the
  log contains the expected PASS sentinel as its final line.

### 4.6 Documentation

- `tools/neo-extract-log/README.md`: how to use the extraction tool.
- A new section in `Plans/PROCEDURE-2026-05-22-phase-3-build-5b-bare-metal-optiplex.md`
  describing the bare-metal-mode execution procedure with this sink in
  place. (PM will file this update after Jarvis confirms the capability
  is landed; do not edit the procedure file in this dispatch.)

## 5. Anti-Goals (Reiterated)

- This dispatch does NOT add a USB stack.
- This dispatch does NOT add network logging.
- This dispatch does NOT change serial logging behavior. The serial sink
  remains the primary log destination on platforms where serial is
  reachable; the block-device sink is additive.
- This dispatch does NOT change scenario PASS/FAIL semantics, NEO_TEST/1
  output format, or the existing VMware procedure.
- This dispatch is NOT Phase 3 scope creep — the deliverable is a small
  capability that unblocks an existing Phase 3 verification step.

## 6. Acceptance Criteria

The dispatch is complete when all of the following hold:

- **(C1)** Block-device log sink module compiles, passes its unit tests,
  and is wired into the kernel log infrastructure.
- **(C2)** Framebuffer phase indicator module compiles, renders a phase
  on a running kernel, and is observable in QEMU and VMware.
- **(C3)** Scenario harness produces bare-metal-mode ISOs for both
  exemplar scenarios.
- **(C4)** Both exemplar scenarios run cleanly under VMware with the
  block-device sink enabled; the post-run extraction tool retrieves a
  log file whose final line is the PASS sentinel.
- **(C5)** `tools/neo-extract-log` builds and extracts a valid log
  region; refuses on invalid magic.
- **(C6)** Documentation lands per §4.6 (excluding the procedure-file
  update, which PM owns).
- **(C7)** No regressions in existing VMware scenario harness behavior
  when bare-metal-mode is not selected.

## 7. Out-of-Scope Decisions Reserved For PM

- Whether to ratify this as an ADR (likely yes — log-sink design is the
  kind of thing future-us will want to find). PM files ADR after
  acceptance, not blocking on it.
- Whether to retrofit other scenarios with `phase()` calls. Out of scope
  for this dispatch.
- Whether to extend the sink to multi-target later. Defer until needed.

## 8. Estimated Effort

Jarvis estimate (PM-side guess, subject to Jarvis's own sizing): roughly
half a day to a full day of focused implementation, plus testing. The
work is small in surface area but touches the kernel log infrastructure,
which warrants care.

## 9. Closure Path

When acceptance criteria are met:

1. Jarvis files a CLOSE-REPORT artifact at
   `Plans/CLOSE-REPORT-2026-05-XX-block-device-log-sink-and-phase-indicator.md`.
2. PM consumes via PM-DISPOSITION and updates BUILD-5b procedure (§4.6).
3. BUILD-5b execution session unblocks; PL runs both scenarios on the
   OptiPlex 7080 Micro with log capture and framebuffer phase indicator.
4. BUILD-5b evidence artifact lands with extracted log files attached.
5. Phase 3 formal close PM-DISPOSITION lands.

---

**End of dispatch.**
