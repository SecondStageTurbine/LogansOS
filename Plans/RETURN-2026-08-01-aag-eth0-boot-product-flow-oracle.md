<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: AAG lens on eth0 boot product flow (`aa6c5b27`) — Oracle

**Date:** 2026-08-01 (artifact committed 2026-08-02 — see *Provenance* below)
**Dispatch:** `Plans/DISPATCH-2026-08-01-aag-eth0-boot-product-flow-oracle.md`
**Candidate:** `codex/eth0-boot-product-flow-chad` @ `aa6c5b27` (Chad, Codex-family)
**Reviewer:** Oracle (Claude-family) — cross-family, not the author
**Mode:** doc-only. No build, no gate, no branch, no runner.
**RAG:** healthy (`ollama=true`, `qdrant=true`).

**Verdict: 3 findings + 3 notes. None blocks correctness of the merged code. The I-14 core is
genuinely clean — including the exact trap Slice-4 fell into, which this code does not.**

## Provenance — why this artifact is dated after the review

The five-lens review was completed on 2026-08-01 and delivered to the PL in-session. It was
never written to a committed file, so the board correctly showed the dispatch as unstarted.
That is a process miss by the reviewer, not a re-run: the candidate branch tip is **still exactly
`aa6c5b27`**, unchanged since the review, and the two load-bearing findings (F1, F2) were
re-verified against `origin/main` before this artifact was written. The analysis below is the
original review, not a reconstruction from memory of a moved target.

## Headline: I-14(c) is real here, unlike `accel_intel`

The dispatch asked for reasoning from the code path, not the counter. Every `.submit()` in
`cdc_ecm.rs` is fence-preceded:

| Line | Submit | Fence |
|---|---|---|
| 546 | `int_in.submit` | 544 `fence_sub_span` |
| 628 | `bulk_in.submit` | 627 `fence_whole` |
| 738 | `int_in.submit` (repost) | 737 `fence_sub_span` |
| 805 | `bulk_out.submit` | 804 `fence_whole` |

Both helpers bottom out in `dma_arena::fence(...)`; every buffer comes from
`DmaBuffer::alloc_in_arena`. **`fence_refusals=0` on run 3 therefore measures something** — the
fence was called on every DMA path, so zero refusals means zero refusals. This is the opposite
result from `accel_intel`, where `dma_arena::fence` is never called at all.

**I-14(a) clean:** `boot_eth0.rs` contains no governance lock and no `lock()` call; it runs from
`usb::init()` in thread context after enumeration, never in ISR.

## Findings

### F1 — `boot-eth0` is absent from the OT-69 coverage matrix (MEDIUM, one-line fix)

The matrix in `.github/workflows/ci.yml` covers four combos: `mt7601u,wpa3-sae`;
`accel-intel,vtd`; `atrium-ai`; `misc`. **No `boot-eth0`, no `product`** — `grep -c boot-eth0
ci.yml` returns **0** on current `main`. The job's own comment claims *"4 builds, covers every
non-default feature"*, which becomes false on merge. The 523 new lines gated on `boot-eth0`
(`boot_eth0.rs` 319 + `boot_eth0_logic.rs` 70 + `boot_frame_eth0.rs` 134) land in exactly the
never-compiled hole OT-69 exists to close. `cdc_ecm.rs` *is* covered (wifi-usb pulls `usb-host`);
the three new files are not. **Fix: one matrix entry.**

### F2 — no gate scenario can ever exercise `bring_up()` (MEDIUM, by construction)

```rust
#[cfg(all(feature = "boot-eth0", not(feature = "neo-test")))]
boot_eth0::bring_up();
```

Every scenario carries `neo-test`, so the call is compiled out of every gated build **by design**.
The green 4-shard gate provides *zero* runtime coverage of this flow; Chad's three product-QEMU
proofs (no-dongle / usb-net DHCP / usb-net no-DHCP) were run by hand and are not in CI. The gating
is the right call — boot DHCP does not belong inside test scenarios — but combined with F1 the
product path is **neither compiled nor run** by CI.

### F3 — `boot_frame_eth0.rs` is a 127-line fork differing in ~7 lines (LOW-MED)

`init/mod.rs` swaps the whole module via `#[path]`. `diff` of the two files is **14 lines**; the
only real change is the status string. Any future fix to `boot_frame.rs` silently never reaches the
product build. The file also contains a dead `#[cfg(not(feature = "boot-eth0"))]` branch inside a
file only compiled when the feature is **on**. Same "converge duplicate implementations" shape as
the DW-2 bounce; a cfg'd status line in the single `boot_frame.rs` does the same job in ~7 lines.

## Notes — missing fact named, not speculative blockers

### N1 — ASIX vendor init is gated on vendor ID only

`if device.vendor_id == ASIX_VENDOR_ID` (0x0b95) — **no product-ID check**, though
`device.product_id` is used in logging two lines away. Any ASIX device exposing a CDC-ECM function
receives AX88179-specific `SET_CONFIGURATION(1)` → `AX_ACCESS_MAC` reads/writes → PHY-id-0x03
PHYSR polling → medium composition → `SET_CONFIGURATION(0)`. A graceful path exists (vendor read
fails → `None` → "TX unreachable"), so a chip that rejects degrades safely; the risk is a chip that
**accepts with different register semantics**. No second ASIX dongle available — unresolved, not
disproven.

### N2 — the whole boot exchange runs pre-IOMMU, and `product` has no `vtd`

`main.rs:645` runs `usb::init()` (where `bring_up` executes) **before** `drivers::iommu::init()`,
and `vtd` is in **neither `default` nor `product`**. I-14(c2) is therefore absent from the product
boot; containment rests entirely on (c1), the fenced arena. Not a regression this branch
introduced — `usb::init` always preceded `iommu::init` — but the exposure changed from
"enumerate and bind" to "run a multi-second DHCP exchange against a live LAN". Stated as product
posture, not a bounce.

### N3 — the RETURN's I-14(b) argument is computed against a number that did not ship

Chad's RETURN says *"four attempts now use 10,000 pump passes each"* and bases its liveness case on
that. The final commit changed exactly that line:

```
-const DHCP_PUMP_PASSES_PER_ATTEMPT: u64 = 10_000;
+const DHCP_PUMP_PASSES_PER_ATTEMPT: u64 = 750_000;
```

Shipped worst case is **4 × 750,000 = 3,000,000** pump passes — 75× the documented figure. **The
containment claim still holds** (run 2 completed all four attempts on silicon and boot continued),
but the written evidence no longer describes the merged code. Recommend correcting that paragraph
so the merge record is accurate.

## Hypotheses formed and then killed — recorded so the coverage is on record

**RX aggregation header — REFUTED, and this was the strongest lead.** There is **no** AX88179
RX-header parsing anywhere in `cdc_ecm.rs`; `rx_take` returns `RxFrame { offset: 0, len }` with
`len` = the whole URB, and `boot_eth0::pump_once` reads the ethertype at `bytes[12..14]`. Given the
author's own comment that the device aggregates into 20 KiB URBs, that looked like a live bug.
**The lease refutes it:** native ax88179 RX prepends 2 alignment bytes before each frame, so
`bytes[12..14]` would land inside the source MAC and DHCP could never have demuxed. Run 3 obtained
a real lease ⇒ frames arrive raw at offset 0 ⇒ true ECM semantics, no vendor RX header. The 20 KiB
size is a SuperSpeed queue/burst requirement, not aggregation.
*Residual, low:* with `RX_BUFFER_LEN` raised 2048 → 20480 there is no framing logic to split two
frames sharing one URB should a device ever omit the terminating ZLP. Pre-existing shape, blast
radius 10× wider. Not reachable on evidence to date.

**Unbounded inner pump loop — REFUTED.** `while let Some(frame) = device.rx_take()` looked
unbounded under a broadcast flood. But `EndpointPipe::completions()` (`xhci.rs:1896`)
`mem::replace`s a finite queue capped at `EP_QUEUE_CAP` and **does not poll the controller** —
`poll_events()` runs only in `service()`. Each `pump_once` drains a finite queue and terminates.

**Length validation — clean.** `len = RX_BUFFER_LEN.saturating_sub(residue)` with
`len > 0 && len <= RX_BUFFER_LEN`; a hostile oversized residue saturates to 0 and reposts.
`pump_once` checks `bytes.len() >= 14` before indexing. No OOB path found from the URB length.

**Kill-switch OFF — clean, and the author's evidence is unusually strong.** All three gate points
check out (module `cfg`d, call site `cfg`d, `default` unchanged), and Chad proved loadable-section
byte-equality vs `origin/main` (`.text`, `.rodata`, `.data`, `.got`, `.dynamic`, …) plus
`llvm-strip --strip-debug` SHA-256 equality. Only DWARF differs, because `init/mod.rs` is edited at
all. Nothing outside the `cfg` assumes eth0 exists — **the rung-5 shape is not present.**

## What the lease proves, and what it does not

**Proven:** one AX88179, one LAN, one DHCP server, one boot — real lease, correct MAC (the server
honoured the address Ubuntu holds), I-14(c1) fence called on every DMA path with zero refusals,
boot continues in all four status paths.

**Not disproven (no evidence either way):** non-AX88179 ASIX hardware (N1); unplug mid-DHCP; a
hostile or malformed OFFER — `demux_eth_rx` and `arp::inject` are unchanged by this branch but are
**newly reachable at boot from the wire**, which is a real exposure change even with no code
change; multi-frame URBs.

**Zero coverage from either channel:** QEMU has no AX88179, so every vendor-specific path
(`ax88179_vendor_config_init`, PHYSR polling, the false-link-down veto) is exercised only on the one
OptiPlex dongle — and per F1/F2 it is neither compiled nor run in CI at all.

**No byte-oracle** covers any AX88179 register or frame layout independently; the sole evidence is
that one dongle worked once.

## Recommendation

None of the three findings blocks correctness of the merged code. F1 is a one-line CI fix worth
taking before or immediately after merge; F2 should be recorded as accepted scope; F3 is a
convergence item for a follow-on lane. N3 wants Chad's I-14(b) paragraph corrected to 750,000 so
the merge record is accurate.
