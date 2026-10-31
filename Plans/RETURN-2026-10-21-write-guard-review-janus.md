<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: PR #85 write-guard hardening review — Janus

**Date:** 2026-10-21  
**Reviewed head:** `6bfcfbeac5396e0e0584afdb82cdebe801e616a0`  
**Base:** `2b3b509443dd8bc00de9838a64bd8bd4937a487b` (`origin/main`)  
**Disposition:** **Do not merge as the fail-closed boundary yet. F1 and F2 violate required fail-closed behavior.**

## Findings

### F1 — GPT metadata is trusted without CRC validation, so a corrupt primary can hide allocated Windows extents

`read_gpt` consumes `entries_lba` directly from the primary header at `kernel/src/drivers/block/write_guard_logic.rs:291` and reads that array at `:305`. It does not validate the GPT header CRC or partition-entry-array CRC before treating zeroed bytes as an empty entry array. That makes a structurally invalid primary header authoritative instead of opaque.

I confirmed the behavior with a temporary host-only regression probe, then removed the probe. The probe started with the existing Windows-shaped GPT fixture, changed the primary header's entry-array LBA from 2 to an unused zeroed area, and placed a valid sealed log header at 256 MiB. The expected fail-closed assertion failed with:

```text
Arming { claims: [Claim { marker: LogSink,
extent: Extent { start: 268435456, end: 285212672 } }],
blocked_by_table: 0, table: "gpt" }
```

The required behavior is `Table::Opaque` unless the header and selected entry array pass their CRCs and structural bounds. If recovery from a damaged primary is desired, validate the backup header and its entry array independently; disagreement must remain fail-closed.

### F2 — the bare-metal guard-control refusal is bypassed by Cargo's package-qualified feature spelling

The refusal compares only the exact string `neo-test-block-write-guard-control` at `tools/xtask/src/main.rs:4451`, while `build_member` forwards feature strings unchanged to Cargo at `:5092`. Cargo accepted this package-qualified form with exit status 0:

```text
cargo metadata --no-deps --format-version 1 \
  --features neoplutarch-kernel/neo-test-block-write-guard-control
```

Therefore `NEO_TEST_EXTRA_FEATURES=neoplutarch-kernel/neo-test-block-write-guard-control` misses the exact comparison while naming the same kernel feature. Normalize/parse feature specs before the bare-metal deny check, or reject any spec whose final feature component is `neo-test-block-write-guard-control`.

## Required safety-property answers

| Safety property | Preserved? | Enforcing or contradicting line |
|---|---:|---|
| Every block-write path goes through `GuardedDevice` | **No** | Normal registry writes are checked at `kernel/src/drivers/block/mod.rs:253` and `:272`, but `debug_submit_abandoned_write` obtains the raw virtio device at `kernel/src/drivers/block/virtio_blk.rs:528` and submits the write directly at `:548`. |
| Arming stays inside the sealed extent | **Yes** | `GuardedDevice::write_allowed` calls `arming.allows(start, end)` at `kernel/src/drivers/block/mod.rs:183`; `Arming::allows` delegates to half-open containment at `kernel/src/drivers/block/write_guard_logic.rs:119` and `Extent::contains` enforces both endpoints at `:90`. Arming is immutable after `spin::Once::call_once` at `kernel/src/drivers/block/mod.rs:168`. |
| The framebuffer-console panic takeover cannot deadlock | **No** | The takeover breaks only `CONSOLE`/`FB` locks (`kernel/src/drivers/fb/progress.rs:77`, `:81`; `kernel/src/drivers/fb/mod.rs:173`, `:176`). The panic handler then emits its first line at `kernel/src/panic.rs:40`, which blocks on `SERIAL.lock()` at `kernel/src/drivers/console/serial.rs:38`. A panic raised while that CPU holds `SERIAL` cannot unwind the guard, so the panic path can spin before the framebuffer mirror receives the message. |

## Checks by requested area

- **Sealed markers:** the log marker checks 11-byte `NEO_LOG_v1\0`, version 3, a BLAKE3 seal over bytes `0..0xfe0`, page-multiple size, lower bound, and 1 GiB ceiling (`write_guard_logic.rs:181-194`). Scratch checks the full 32-byte v2 magic, exact device capacity, and the seal (`:221-225`). NeoFS checks its 8-byte magic, checksum, format version, 4 KiB block size, type version, nonzero count, and checked byte-size multiplication (`:157-169`). Existing byte-oracle and mutation tests passed. F1 shows that marker validity alone is insufficient when GPT structure is not authenticated.
- **Extent-scoped arming:** the log claim is exactly `[0x10000000, 0x10000000 + region_size)` at `write_guard_logic.rs:385-388`; NeoFS claims `[0, total_blocks * 4096)` at `:364-367`; scratch claims `[0, capacity)` at `:399`. Device-fit checks precede each claim. The half-open containment above rejects writes before, after, or straddling an extent. `spin::Once` prevents re-arming. Arithmetic at the log marker is bounded to 1 GiB; device writes use saturating start/end arithmetic and backend geometry checks. Multiple registered devices receive distinct `GuardedDevice`/`Once` state in `block/mod.rs:320-338`; the NVMe driver exposes only NSID 1, so other namespaces are not write-addressable by this driver.
- **Partition-table rule:** valid Windows-shaped GPT fixtures, including 512-byte and 4 KiB logical sectors, block sealed NeoFS/log/scratch claims. Protective MBR without readable GPT becomes opaque. F1 is the missing fail-closed structural validation.
- **Write paths and non-data commands:** NeoFS and the log sink write through registry-provided `Arc<dyn BlockDevice>` handles and therefore through `GuardedDevice`. AHCI, NVMe, and ordinary virtio writes enter through guarded `submit`/`write_block`; the raw virtio debug path above is the exception. The public operation enum contains only Read, Write, and Flush (`block/mod.rs:30-34`). No trim, discard, sanitize, format, or write-zeroes command exists in the block drivers. Flush is unaddressed and carries no data: NVMe emits `NVM_FLUSH` at `nvme.rs:417`, AHCI emits `ATA_FLUSH_CACHE_EXT` with no buffers at `ahci.rs:338`, and virtio maps only `VIRTIO_BLK_T_FLUSH` at `virtio_blk.rs:1544`.
- **xtask and provisioning:** F2 defeats the bare-metal refusal. `cargo xtask log-sink-header` builds the canonical sealed 16 MiB-region header at `tools/xtask/src/main.rs:4329` and writes exactly that 4096-byte artifact at `:4330`; it does not write a disk itself. The printed `dd` recipe targets the fixed 256 MiB offset.
- **Framebuffer console:** ordinary console drawing is nonblocking on the framebuffer because `try_with_fb` uses `FB.try_lock()` at `kernel/src/drivers/fb/mod.rs:147-148`. `stop` clears `ACTIVE` before its framebuffer barrier (`progress.rs:66-67`), and draw closures recheck `ACTIVE` while holding the framebuffer lock. Overlay ownership is tagged around the callback at `fb/mod.rs:212-215`, so a foreign thread still invokes `take_screen`. The panic path does not establish the claimed no-deadlock property, as recorded above.

## Verification

- RAG health: `healthy`, `ollama=true`, `qdrant=true`.
- `cargo test -p block-guard-logic-tests`: **11 passed**.
- `cargo test -p fb-progress-logic-tests`: **5 passed**.
- `cargo test -p xtask`: **42 passed**.
- Temporary F1 probe: failed in the unsafe direction as quoted above, then was removed.
- Worktree before this RETURN: no tracked changes; only the pre-existing untracked `graft/` index.

