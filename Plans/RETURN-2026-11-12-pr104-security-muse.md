<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Relay 393, Muse security read — PR #104 (OT-100 R2, USB mass-storage write path)

> **Banked by the PM (2026-11-12).** Muse has read-only access, so J.c. relayed this text. **Head note:** Muse
> reviewed `16bb8f56`, the head J.c. relayed; the dispatch had named the older `2cf42002`. The PM checked the current
> head, `b6ee1046`, against `16bb8f56`:
> - `kernel/src/drivers/block/` and `kernel/Cargo.toml` are unchanged.
> - The only change in `msc.rs` maps the new `FlushResult::TimedOut` to `BlockError::Io`.
> - `msc_logic::flush` still checks the `sync_ok` latch first.
>
> So this PASS carries over to `b6ee1046`.

- **PR:** #104, USB stick writing (OT-100 R2)
- **Reviewed commit:** `16bb8f56` (as relayed by J.c.; see Limitations for the head discrepancy)
- **Checklist source:** the "Muse security read" section of `Plans/DISPATCH-2026-11-12-relay-393-pr104-usb-write-review.md` on `main`
- **Reviewer:** Muse (Gwen) — a different model from the writer (Claude), so this review counts under the model-difference rule
- **Overall verdict: PASS — all 5 items.** No FAIL; no concrete input produces a write, flush or probe to a device the guard has not armed.

## Item 1 — No new code path writes, flushes or probes a block device the guard hasn't armed — PASS

- **Probe:** the only registry entry point is `block::write_probe` (`kernel/src/drivers/block/mod.rs:862-869`, `#[cfg(feature = "usb-storage-write")]`), which clones the slot's `GuardedDevice` and calls its `write_probe`. `GuardedDevice::write_probe` (`mod.rs:611-616`) returns `None` before touching the device unless `writes_allowed()` — i.e. the judgement's claims are non-empty (`mod.rs:458-460`). Only after that gate does it call `MscDisk::write_probe` (`kernel/src/drivers/usb/msc.rs:945-997`), the sole caller of `msc_logic::write_probe` (`kernel/src/drivers/usb/msc_logic.rs:1321-1347`: MODE SENSE(6), TEST UNIT READY, one SYNCHRONIZE CACHE(10)). An unarmed stick is therefore never sent MODE SENSE, SYNC or WRITE.
- **Writes:** every registered device is wrapped in a `GuardedDevice` at registration (`mod.rs:767-786`: the guard is what `registry.devices` holds). `GuardedDevice::submit` refuses a `Write` outside an armed extent with `WriteRefused` (`mod.rs:578-598`); `GuardedDevice::write_block` does the same (`mod.rs:624-638`). `MscDisk::write_block` (`msc.rs:834-902`) has no other caller path: the write-path `Port`/`Transport::run_dir` machinery is constructed only inside `MscDisk::write_block` (`msc.rs:879`), `MscDisk::flush` (`msc.rs:922`) and `MscDisk::write_probe` (`msc.rs:963`) — nothing at bind/enumeration time sends a write-path command, and `command_dir`'s allowlist (`msc_logic.rs:494-518`, enforced at `msc_logic.rs:633-650`) is a second gate behind the guard.
- **Flushes:** the new flush, `MscDisk::flush` (`msc.rs:913-934`), sends SYNCHRONIZE CACHE(10) only when the `sync_ok` latch is set. That latch starts `false` (`msc_logic.rs:987-1004`) and is set `true` only by `msc_logic::write_probe` (`msc_logic.rs:1340-1341`) — which, per above, runs only on an armed device through the guard; `msc_logic::flush` re-checks it before sending anything (`msc_logic.rs:1291-1294`). A never-probed (hence never-armed-and-probed) stick receives no SYNC from `flush()`; it returns `Unsupported`. The install-claim bypass (`GuardedDevice::claimed_write`, `mod.rs:691-697`) is pre-existing OT-87 code, requires the `claimed` flag, and is unchanged by R2's additions.

## Item 2 — `PartitionDevice` forwards `write_probe` and `write_protected` without widening — PASS

`kernel/src/drivers/block/partition.rs:141-149`: both methods are pure forwards — `write_protected` returns `self.inner.write_protected()` and `write_probe` returns `self.inner.write_probe()`, the `Option` values unchanged, no caching, no fallback, no arming of its own. `inner` is the registered (guarded) device (partition.rs:8-11), so a probe through a partition view still hits the `GuardedDevice::write_probe` armed gate (item 1). The partition's own writes stay offset-and-bounded by `gpt_logic::partition_lba` (`partition.rs:98-107`, applied at `partition.rs:151-159`) before reaching that same guard.

## Item 3 — With `usb-storage-write` off, none of the new trait methods or probe code is compiled — PASS

- The trait methods and their types are `#[cfg(feature = "usb-storage-write")]`: `WriteProbe`/`SyncSupport` (`mod.rs:97-107`, `mod.rs:118`), `BlockDevice::write_protected`/`write_probe` defaults (`mod.rs:169-182`), the `GuardedDevice` impls (`mod.rs:603-616`), and `block::write_probe` (`mod.rs:862`).
- `PartitionDevice`'s forwards are cfg'd identically (`partition.rs:141-149`).
- On the driver side, `MscDisk`'s `write_block` has a `#[cfg(not(feature = "usb-storage-write"))]` stub returning `Unsupported` (`msc.rs:822-826`); the real `write_block`, `supports_flush`, `flush`, `write_protected`, `write_probe`, the `WriteState`/`Port` types and `Transport::run_dir` are all under `#[cfg(feature = "usb-storage-write")]` (e.g. `msc.rs:833`, `904`, `913`, `935`, `944`, `500-556`, `331`).
- The feature itself is opt-in: `usb-storage-write = ["usb-storage"]` (`kernel/Cargo.toml:461`), absent from `default` (`kernel/Cargo.toml:81`).

## Item 4 — No guard-control feature becomes buildable together with `bare-metal`, `product` or `fb-progress-console` — PASS

The guard-off control judge (`mod.rs:539-562`, `#[cfg(feature = "neo-test-block-write-guard-control")]`) still carries its hard build ban at `mod.rs:1000-1010`: `#[cfg(all(feature = "neo-test-block-write-guard-control", any(feature = "neo-test-bare-metal-mode", feature = "product", feature = "fb-progress-console")))] compile_error!(...)`. R2 adds no new guard-control feature: its only feature additions are `usb-storage-write` (`kernel/Cargo.toml:461`) and the scenario feature `neo-test-usb-msc-write = ["neo-test", "usb-storage-write"]` (`kernel/Cargo.toml:2201`), neither of which pulls in a guard-control feature.

## Item 5 — Nothing in R2 can reach the Zenbook's internal NVMe — PASS

The entire R2 write/probe/flush implementation lives in the USB mass-storage driver (`kernel/src/drivers/usb/msc.rs`, `msc_logic.rs`): `MscDisk` is created only by the USB Bulk-Only-Transport bind and is registered through the same `register_device` guard wrap as every other disk (`mod.rs:767-786`). No other driver overrides the new trait methods — the defaults return `None` (`mod.rs:170-182`) — so `block::write_probe` on an NVMe index returns `None` and sends nothing, and NVMe writes still pass the unchanged `GuardedDevice` armed-extent checks (`mod.rs:578-598`, `624-638`). There is no path from the R2 code to an NVMe command: the write path's only transport is the MSC `Transport` over the stick's bulk endpoints (`msc.rs:550-556`, `331-355`).

## Observations (not FAILs)

1. `GuardedDevice::flush` (`mod.rs:599-601`) and `GuardedDevice::write_protected` (`mod.rs:604-606`) forward to the inner device without an armed check of their own. For the MSC disk this sends nothing to an unarmed device — `flush` is latch-gated (item 1) and `MscDisk::write_protected` (`msc.rs:936-938`) only reads the cached latch, performing no device I/O — but the guard's armed gate for these two methods lives in the MSC driver, not in the guard itself. A future driver that implements `flush` or `write_protected` with live device I/O would inherit an ungated path through `GuardedDevice`.
2. `GuardedDevice::submit` passes `BlockOp::Flush` through without the armed check applied to `Write` (`mod.rs:578-598`). Harmless for the stick: `MscDisk` has no `submit` override, so a Flush submit returns the trait default `Unsupported` (`mod.rs:155-157`); the real flush entry is `flush()`. Recorded because the asymmetry is visible at the guard.
3. The pure-logic write half of `msc_logic.rs` (`command_dir`, `WRITE_PATH_COMMANDS`, the CDB builders, from `msc_logic.rs:449`) is not itself `#[cfg]`-gated inside that module; it is unreachable when the feature is off because every call site is cfg-gated (item 3), and the module header states exactly that (`msc_logic.rs:449-450`). The checklist item asks about trait methods and probe code, which are cfg'd out; this is a note, not a gap in that answer.

## Limitations

- **Head discrepancy (flagged, review still at the relayed commit):** the dispatch names PR #104's head as `2cf42002`, and its Muse section says "Read the files at `2cf42002`". Per J.c.'s relay instructions this review was done at `16bb8f56`. If `16bb8f56` is not the head the PM intends to bank this RETURN against, the verdict should be re-run at that head.
- Files read in full at `16bb8f56`: `kernel/src/drivers/block/mod.rs`, `kernel/src/drivers/block/partition.rs`, `kernel/src/drivers/usb/msc.rs`, `kernel/src/drivers/usb/msc_logic.rs`, `kernel/Cargo.toml`. Other block drivers (NVMe/AHCI/virtio) were not individually re-read; item 5 rests on the trait defaults in `mod.rs` and the guard wrap in `register_device`, which bind regardless of what a driver overrides.
- GitHub code search's index proved incomplete during this review (it did not return `msc.rs` for `write_probe`), so no negative claim above rests on search results; each rests on the fetched files.
- One transient HTTP 503 on the first `partition.rs` fetch; the retry succeeded. No 429 was hit.
- `kernel/src/test_harness.rs` was not read (not named by the Muse section; ~1.5 MB, over the MCP read limit in past relays). The scenario-side counts are Janus's section 393, not this read.
