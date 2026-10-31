<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: RTL8125 rung-1 checklist review — Janus

Reviewed PR #101 at exactly `612c0283db3ff1cb07ea483bbf43a2e797236539`.
Review branch: `codex/rtl8125-review-janus`. Source base:
`84b6686dbb881e1afa06b9c52a64796b7fec7cad`.
Dispatch: `Plans/DISPATCH-2026-10-23-rtl8125-review.md` at the pinned head;
background: `Plans/DISPATCH-2026-10-23-rtl8125-ethernet.md`.

**Result: FAIL. 32 checklist items PASS; items 12 and 28 FAIL.**
There is also an unchecked RX-completion-length finding (F3) beyond the literal
FCS/recycle requirements of item 33. No driver changes were made.

## Evidence and validation

- Used grep, direct source reads, and the PR description/ready comment. No RAG queries.
- Independently ran `cargo test -p rtl8125-logic-tests --target x86_64-unknown-linux-gnu --target-dir target/rtl8125-review-check`
  inside the Docker development image: **25 passed, 0 failed**, doc-tests 0.
- Then ran `cargo clean --target-dir target/rtl8125-review-check`: **106 files,
  17.1 MiB removed**, exit 0. The directory is gone. The prior image A hand-off
  ISO was preserved. No raw/internal disk operations or bare-metal run occurred.
- Did not rerun QEMU, the full close gate, or the five release identity builds.
  Their reported results below are attributed to the PR, not independent reruns.
- [Pinned-head ready comment](https://github.com/SecondStageTurbine/LogansOS/pull/101#issuecomment-6356748262)
  names `612c0283` and reports absent PASS, control FAIL `bus_master_set`, VT-d PASS,
  and close-gate exit 0. The identity table is in the PR description.

## Checklist

All source references are at the reviewed SHA. For brevity in this table,
`rtl8125.rs` and `rtl8125_logic.rs` mean files under `kernel/src/drivers/net/`;
`tests/lib.rs` means `tools/rtl8125-logic-tests/src/lib.rs`.

| # | Result | Enforcing source / evidence |
|---:|---|---|
| 1 | PASS | `rtl8125_logic.rs:785`: arena refusal has first precedence, then VT-d, then opt-in, then NoIommu. Exactly the eight combinations below. |
| 2 | PASS | `rtl8125_logic.rs:797`: only Iommu and ArenaOnly match. |
| 3 | PASS | `tests/lib.rs:206`: eight explicit rows at lines 210–217 assert both mode and bus-master permission. Independently passed. |
| 4 | PASS | `rtl8125.rs:333`: checks `iommu::enabled()`, computes `(bus << 8) | (device << 3) | function`, returns `fence_source_id(sid)`; no-vtd implementation at line 343 returns false. Bind uses `arena && vtd_fence(dev)` at line 356. |
| 5 | PASS | `rtl8125.rs:322`: production opt-in is precisely the named cfg. `kernel/Cargo.toml:249` defines it separately; no other feature lists it. Scenario control injects a bool only into the test seam, not the production feature. |
| 6 | PASS | `rtl8125.rs:362`: sole driver call to `pci::enable_bus_mastering`, at line 366, is after `bus_master_allowed` refusal. |
| 7 | PASS | `rtl8125.rs:353`: clears BME before arena/fence work and before `map_registers`/first register read at lines 989/995. `rtl8125_logic.rs:819` sets MEMORY and INTX_DISABLE, clears IO and BME; test at `tests/lib.rs:193`. |
| 8 | PASS | `rtl8125.rs:1069` refills RX; lines 1082/1083 write both fenced bases; line 1086 grants BME. No earlier grant on production bring-up. |
| 9 | PASS | `rtl8125.rs:1052`: blocked mode returns at line 1059, before `alloc_rings` at 1062, base writes at 1082, and TX/RX-enable write at 1088. `ensure_arena` reserves an arena, but this is not allocation of driver DmaBuffers/rings. |
| 10 | PASS | `kernel/src/test_harness.rs:24783` presets and checks BME, then line 24790 binds, line 24791 reads back clear-at-bind, and lines 24794/24806 check hardware BME. Control at 24789 reaches FAIL `bus_master_set` at 24807. Stand-in lookup at 24760; xtask adds rtl8139 at `tools/xtask/src/main.rs:8435`. Runtime results are PR-reported. |
| 11 | PASS | `rtl8125.rs:971`: `write_ring_base` requires `FencedDmaAddr`; only ring-base writes are its hi/lo writes at 972/973, called with fenced results at 1082/1083. |
| 12 | **FAIL** | `rtl8125.rs:418` pops an RX pool buffer; fence refusal at 421 returns at 423 without putting it back. No publish occurs, but the explicit pool-return requirement fails. TX refusal at 589 correctly returns the buffer and fails submit. Concrete input: F1 below. |
| 13 | PASS | `rtl8125.rs:938`: all four allocation sites (939, 940, 943, 947) are `alloc_in_arena`. No `alloc_zeroed` call in the driver. |
| 14 | PASS | `rtl8125.rs:299`: volatile addr at 305, opts2 at 306, Release fence at 307, opts1/OWN at 308. |
| 15 | PASS | `rtl8125_logic.rs:654`: OWN completion breaks at 657 before acquire/event; acquire at 660 precedes both event kinds. Kernel closure is `fence(Acquire)` at `rtl8125.rs:463`. |
| 16 | PASS | `rtl8125.rs:594` posts; SeqCst at 600 precedes 16-bit TX_POLL at 602. |
| 17 | PASS | `rtl8125_logic.rs:559`: reaps only `complete`, stops on OWN at 567, then advances in order. `tests/lib.rs:307` independently passed the out-of-order completion case. |
| 18 | PASS | `rtl8125_logic.rs:610` initializes next=fill=0, empty=len. `refill` at 635 advances fill and decrements empty; `reap` at 646 advances next and increments empty before either frame or drop event. Both preserve `next == (fill + empty) % len`. Drops take the slot buffer at `rtl8125.rs:474`, return it to the pool, and refill via the fill cursor, not the dropped slot directly. |
| 19 | PASS | `rtl8125.rs:417` refills only while `next_fill_slot` exists, using `RxRing::refill` at 425. `rtl8125_logic.rs:625` selects only fill while empty>0. Initial `rx_slot` is None (`rtl8125.rs:962`); both reap event paths take it before refill. This is an invariant, not an explicit kernel assertion; host harness asserts None at `tests/lib.rs:376`. |
| 20 | PASS | `rtl8125_logic.rs:551` and `:637` pass EOR iff slot == len−1. Encoders at 453/459 add no other EOR. Host wrap tests at `tests/lib.rs:283` and `:423` pass. |
| 21 | PASS | `rtl8125_logic.rs:421` defines 16 bytes; bit definitions at 426–436; encoder at 442 emits opts1, opts2, addr LE. Hand-laid byte oracles at `tests/lib.rs:24`, `:39`, `:50`, `:67` match these independent literal bytes and passed. Raw-source provenance is recorded at `tests/lib.rs:4`; it was not independently re-fetched in this review. |
| 22 | PASS | `rtl8125_logic.rs:211` never sets bit 24 for any version; common OCP entry at 247 clears `0xeb58` bit 0. `tests/lib.rs:88` tests both choices and DESC_LEN. |
| 23 | PASS | `rtl8125_logic.rs:33`/`:35` define IMR=0x38, ISR=0x3c; driver accesses via r32/w32 at `rtl8125.rs:492`, `:499`, `:820`, `:821`, `:1096`, `:1097`. TX_POLL=0x90 at logic line 51, written w16 at driver line 602. No old 8168-width IMR/ISR access. |
| 24 | PASS | `rtl8125_logic.rs:173` accepts only 609/641/688/689 after documented XID masking. Refusal tests at `tests/lib.rs:117` pass. `rtl8125.rs:751` names and refuses PCI 10ec:8126; unsupported XID exits at 1005. |
| 25 | PASS | `rtl8125.rs:824` sets MISC RXDV gate during reset. Only clearing write is 1098, after RX_CONFIG at 1089, TX_CONFIG at 1090 and MAR writes at 1092/1093. |
| 26 | PASS | `rtl8125.rs:411` is the sole driver mutex, constructed ordered at 1110. `kernel/src/sync/mod.rs:67` sets NET_DRIVER=29 above AI levels ≤28. Direct lock sites are all `inner`; no AI-governance acquisition in their call paths. Arena fence at `kernel/src/drivers/dma_arena.rs:145` is lock-free. Existing lockdep is debug-assertion gated (`kernel/src/sync/switch_mutex.rs:42`); no claim of a new release-mode assertion. |
| 27 | PASS | No IRQ handler/vector registration in driver. IMR writes are zero (`rtl8125.rs:820`, `:1096`), COMMAND disables INTx (`rtl8125_logic.rs:819`), and console prints `irq=polled` (`rtl8125.rs:711`). |
| 28 | **FAIL** | MMIO/OCP/reset/PHY/autoneg waits are finite (`rtl8125.rs:205`, `:233`, `:243`, `:819`, `:884`, `:926`). Outer DHCP/ARP loops are counted (1159/1209), but both call `pump_once`, whose `while let Some(frame)` at 1222 can remain nonempty indefinitely as buffers are recycled and hardware completes more RX. Concrete traffic/register sequence: F2 below. |
| 29 | PASS | Ring reaps use REAP_BUDGET at `rtl8125.rs:439`/`:459`; refill has at most RX_SLOTS iterations. `rtl8125_logic.rs:730` declares storms at STORM_PASSES=64 and masks after STORM_LIMIT=4. `rtl8125.rs:524` counts fruitless TX passes, sets tx_wedged at 527; `tx_reserve` refuses at 567. A single service pass is bounded even though the boot caller's pump is not (item 28). |
| 30 | PASS | New runtime items all feature-gated: `kernel/src/main.rs:737`, `kernel/src/drivers/net/mod.rs:7`, `kernel/src/drivers/pci.rs:879`, `:885`, `:891`, `kernel/src/sync/mod.rs:66`. The logic module is reachable only through gated rtl8125; scenario paths have their own feature gates. |
| 31 | PASS | PR description's identity table supplies equal `.text` SHA-256 prefixes for default, desktop, product, boot-eth0 and image A at its cited base cb02dc9d. Ready comment pins head 612c0283. `git diff cb02dc9d 84b6686d -- kernel tools userland Cargo.toml Cargo.lock` is empty; the rebase-base change is Plans-only. Static gating anchors are item 30. PASS is for the requested evidence table, not a fresh independent binary comparison or a comparison to today's main. |
| 32 | PASS | `rtl8125.rs:577` rejects 0, >1514 and >buffer length, returning buffer at 578. Fence and full-ring failures return it at 590/595. Short frames zero-pad at 586 to max(len,60), using this driver's 2048-byte TX buffers allocated at 943. |
| 33 | PASS | Literal FCS/recycle checklist: `rtl8125_logic.rs:494` subtracts four; `rtl8125.rs:612` hands out that length, and recycle at 620 returns buffer to pool and refills. **Additional malformed-length failure F3 below remains**, despite these two specific behaviors being present. |
| 34 | PASS | Registry API unchanged, called after device construction (`rtl8125.rs:1114`). DHCP early-outs for already-Bound client at 1134, before changing interface at 1146; production-only gates at 775 and 1130 exclude neo-test. No edits to virtio-net, CDC-ECM or shared DHCP/ARP implementations. This is source-level integration evidence; existing-device runtime regressions were not independently rerun. |

## Containment truth table (items 1–3)

| arena | VT-d fenced | opt-in | mode | bus master allowed |
|---|---|---|---|---|
| false | false | false | Blocked(NoArena) | false |
| false | false | true | Blocked(NoArena) | false |
| false | true | false | Blocked(NoArena) | false |
| false | true | true | Blocked(NoArena) | false |
| true | false | false | Blocked(NoIommu) | false |
| true | false | true | ArenaOnly | true |
| true | true | false | Iommu | true |
| true | true | true | Iommu | true |

## Failure inputs and consequences

### F1 — item 12: RX refusal loses a pool entry (defensive-path defect)

At `kernel/src/drivers/net/rtl8125.rs:418`, take an Inner with one empty RX
slot and an RX pool whose last buffer has physical interval outside the arena;
for example arena `[0x100000,0x140000)`, buffer phys `0x200000`, len 2048.
Call `refill_rx`. `fence_buffer` refuses; line 423 returns after the pop.
Pool length decreases by one, no rx_slot receives it, and no descriptor is
published. The local Arc is dropped (and if it was the last reference, the
buffer is freed), rather than returned to the pool as item 12 requires.

This is a source-derived failure-injection input, not observed under ordinary
driver allocations: all normal buffers originate inside the immutable arena.
DMA refusal itself remains fail-closed. It needs an explicit tested buffer
disposition rather than silently losing the pool entry; do not claim this is
an ordinary-packet DMA escape or an allocation leak.

### F2 — item 28: sustained RX prevents boot pump completion

Call `pump_once` while the link supplies a continuous stream of valid
60-byte broadcast Ethernet frames (non-DHCP EtherType such as 0x88b5 suffices).
For each next RX slot the chip clears OWN, sets FS|LS and wire length 64:
opts1 `0x30000040`, plus EOR on slot 15. Every `rx_take` drains the ready
queue or calls `reap_rx`; every `rx_recycle` refills the empty slot. Schedule
the next descriptor completion before the ready queue empties and the next
reap sees only OWN-set descriptors. Repeat around the 16-slot ring.

`pump_once` at `kernel/src/drivers/net/rtl8125.rs:1222` has no frame budget,
deadline, or cancellation check, so under this admissible completion schedule
the inner loop never returns. Consequently the DHCP pass counter at 1159,
`net::tick()` at 1161, lease check at 1162, and ARP bound at 1209 cannot
advance. Finite rings and bounded individual reaps do not bound a loop that
immediately recycles and refills them. Add a per-pump work/time bound and
test continuously replenished RX. This is source-derived liveness evidence,
not a claimed silicon or QEMU reproduction.

### F3 — additional finding: device-reported RX length can panic the kernel

Input for an armed 2048-byte RX buffer: device completes opts1 with OWN clear,
FS|LS set, no error, length 0x3fff: **`0x30003fff`** (OR EOR for the last slot).
`kernel/src/drivers/net/rtl8125_logic.rs:480` returns
`RxCompletion::Frame { len: 16379 }`. Neither `RxRing::reap` (line 646),
`rtl8125.rs:467`, nor `rx_take` at 612 checks it against BUF_LEN/buffer.len().
The normal stack call `frame.as_slice()` at `rtl8125.rs:1223` reaches
`kernel/src/net/device.rs:59`, slicing a 2048-byte buffer to 16379 and panics.
`DmaBuffer::alloc_in_arena` preserves the requested len (`kernel/src/drivers/block/dma.rs:131`),
so page rounding does not make this slice valid.

The same failure already occurs with wire length 2053 (returned length 2049).
This requires a malformed device completion; no claim that a conforming NIC
generates it merely from normal network traffic. DMA fencing does not validate
metadata written inside a fenced descriptor ring. Reject/drop a completion
whose wire length exceeds the armed capacity before constructing RxFrame,
and add an oversized-length register-model case. Existing 25 host tests do
not include this case. This finding is established by tracing the pinned
source; the kernel panic was not executed in this review.

## Remaining acceptance scope

The PR explicitly leaves PHY firmware/per-stepping tuning and real 2.5G link
behavior for silicon. It also reports that the stack lacks IPv4 ICMP echo and
an ARP responder, using DHCP plus outbound gateway ARP as its round trip.
Those are not proven by the stand-in PCI scenarios or by the 25 host tests.
No acceptance decision or no-IOMMU opt-in was made here.
