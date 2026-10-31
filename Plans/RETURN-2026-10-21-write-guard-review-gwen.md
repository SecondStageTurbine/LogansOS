<!-- SPDX-License-Identifier: CC-BY-4.0 -->
> Banked by the PM from the PL upload (2026-10-21). First Muse/Gwen pass on PR #85; the PM did not weight it (it missed Janus F1 and misread F2). The weighted Muse review is `RETURN-2026-10-21-write-guard-review-muse.md`.

<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: PR #85 write-guard hardening review — Gwen

**Verdict: MERGE-WITH-NITS**
**Scope reviewed:** branch `claude/write-guard-hardening` @ 6bfcfbea (3 commits over main: 5c4cdb50 write-guard hardening, 8ce6346d fb-progress-console hardening, 6bfcfbea log-sink-header runbooks). Files read at that ref: `kernel/src/drivers/block/write_guard_logic.rs`, `kernel/src/drivers/block/mod.rs`, `kernel/src/drivers/block/virtio_blk.rs`, `kernel/src/drivers/fb/mod.rs`, `kernel/src/drivers/fb/progress.rs`, `kernel/src/panic.rs`, `kernel/Cargo.toml`, `tools/xtask/src/main.rs`, `tools/block-guard-logic-tests/src/lib.rs`, `tools/fb-progress-logic-tests/src/lib.rs`.
**Prior trail banked:** F1 CONFIRMED (Janus); F2 RESOLVED (Janus — Cargo's accepted package-qualified syntax; consistent with the tree, the host KAT crates are invoked via `cargo test -p`). The code candidate matching Janus's F1 grep trail is P11 below; his local RETURN holds the specifics, so that identification is marked as inference.

## Properties (each reduced to yes/no with the enforcement line)

| # | Property | Holds? | Evidence |
|---|----------|--------|----------|
| P1 | An unmarked disk is never written (fail-closed default) | YES | Claims arise only from validated sealed markers in `judge()` (`write_guard_logic.rs`); empty claims make `write_allowed` return false (`block/mod.rs:178-195`); both write entry points refuse with `WriteRefused` (`mod.rs:254`, `mod.rs:273`). KATs: `an_empty_disk_never_arms` (tests lib.rs:364), `windows_gpt_disk_never_arms` (:184). |
| P2 | Every registry write path passes through the guard | YES | `register_device` wraps every device in `GuardedDevice` and the registry stores only the guard (`mod.rs:326-341`, guard pushed at :338). `submit` and `write_block` both gate before delegating (:253-258, :272-277). `read_block` passes through by design; `flush` passes through — it carries no payload and can only persist writes that were themselves gated (:260-270). |
| P3 | Arming is judged once and is immutable thereafter | YES | `spin::Once` + `call_once` (`mod.rs:112`, :167-169). No API in the tree mutates an `Arming` after judgment; there is no disarm or re-arm path. |
| P4 | A write must lie wholly inside a single claim | YES | `Extent::contains` requires the full range inside (`write_guard_logic.rs:100-102`); `Arming::allows` requires one claim to contain it (:137-139); the byte range is computed with saturating arithmetic (`mod.rs:180-184`). Partial overlaps are refused. |
| P5 | Markers are seal- and structure-validated before claiming | YES | NEOFS: magic, BLAKE3 checksum over bytes [0,480), format version, block-size log2, type version, non-zero block count via `checked_mul` (:182-196). Log-sink: magic, header version 3, BLAKE3 seal, region > header size, ≤ 1 GiB, 4 KiB multiple (:208-220). Scratch: magic, exact capacity match, seal (:241-245). |
| P6 | A partition table overrides markers; an unparseable table arms nothing | YES | `judge()` drops any claim overlapping an allocated extent; `Table::Opaque` clears all claims (`write_guard_logic.rs`, `judge`/`read_table` :259-301; GPT validation in `read_gpt`: entry size 128..=4096 and multiple of 8, entry array ≤ 512 KiB, first/last usable sanity — any failure → Opaque; an entry with last < first → Opaque). KATs: `windows_gpt_disk_never_arms` (:184), `protective_mbr_without_gpt_is_opaque` (:238), `sink_beyond_an_iso_partition_arms` (:282 — claims on genuinely free space still arm, so the veto is precise, not blanket). |
| P7 | The log-sink claim is scoped to its own region | YES | Claim extent is exactly [256 MiB, 256 MiB + region_size), region ≤ 1 GiB and must fit the device (`write_guard_logic.rs:208-220` + `judge`). KAT: `sink_claim_is_scoped_to_its_region` (:255). Nothing outside the region is writable on a sink disk. |
| P8 | The guard-off control cannot reach a real-hardware build | YES | The control `judge()` exists only under `cfg(feature = "neo-test-block-write-guard-control")` and returns a whole-device claim (`mod.rs:225-242`). Feature chain: control → `neo-test-block-write-guard` → `neo-test` (`kernel/Cargo.toml:637-638`); `neo-test` is absent from default features (:77). The only injection channel is `NEO_TEST_EXTRA_FEATURES`, consumed inside xtask's `build_test_kernel`, and hard-refused when `mode.is_bare_metal()` (`tools/xtask/src/main.rs:4451-4455`). The QEMU control scenario is expected to FAIL by design (scenario def + comment, `xtask:3390-3399`). |
| P9 | Markers are provisioned only host-side, by the operator | YES | `cargo xtask log-sink-header` writes the sealed provisioning header to a file (`xtask:4322-4329`; `provisioning_log_header`, `write_guard_logic.rs:225-235`). In-kernel, `judge()` performs reads only, via `GuardReader` over `read_block` (`mod.rs:117-165`). The kernel never writes a marker. |
| P10 | Panic and framebuffer-console paths cannot write or unseal | YES | `panic.rs` reaches only framebuffer drawing and `progress::panic_takeover` (`panic.rs:32-35`, :54-68). `panic_takeover` touches only the console mutex and framebuffer ownership (`fb/progress.rs:77-89`). No block-device calls exist in either path. `stop()` drains in-flight draws via `console_barrier` before ownership moves (`progress.rs:65-69`; `fb/mod.rs:163`; `steal_fb` owner check :173-197). |
| P11 | No in-kernel write path stands outside the guard | QUALIFIED — see N1 | `debug_submit_abandoned_write` (`virtio_blk.rs:527-553`) issues a Write directly to the primary virtio device without the guard. It is compiled only under `cfg(feature = "neo-test")` (:527), which is off by default, and no caller exists in the indexed tree. It does not exist in default or bare-metal builds. It is the single structural exception found, and it is test-harness surface by construction. |

## Nits

- **N1 (from P11):** Recommend an explicit scenario-manifest note for `debug_submit_abandoned_write`: inert and callerless today, but it is the one place the guard is structurally absent. If a `neo-test` scenario ever runs on real hardware (the Zenbook lane does run scenarios bare-metal), the helper is present in the binary. A comment tying it to the manifest would keep the exception visible.
- **N2 (verification gap):** Janus's host runs reported **0 tests** for both `block-guard-logic-tests` and `fb-progress-logic-tests`, yet the KAT sources encode the properties (9 block-guard tests, 5 fb-progress tests). I could not confirm from the remote tree which invocation actually executes them. A suite that reports ok with 0 tests is a silent hole — the lane should confirm the executing invocation (presumably the xtask host-test runner rather than `cargo test -p`).

## Design notes (not defects)

- The marker seal is an unkeyed BLAKE3 checksum — an ownership/integrity marker, not authentication. That is coherent within the stated threat model ("we never write to a disk that is not ours": accident and wrong-disk protection). It is not, and does not claim to be, a defense against an adversary who can already write the disk.
- A valid sealed NEOFS primary superblock suppresses the partition-table check (`judge`: `neofs_primary` → `Table::None`) — deliberate, since a sealed primary owns LBA 0 and its checksum bytes could otherwise mimic an MBR signature. Recorded here for the trail.

## Not verified

- Janus's local worktree and RETURN (not on the remote; only his pasted trail was available).
- KAT execution (see N2) and CI check status (not readable through my GitHub access).

— Gwen (Muse Spark family; writer lane was Claude, reviewer Janus/Codex — cross-family rule satisfied)
