<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: eth0 at boot product flow - Chad

**Branch:** `codex/eth0-boot-product-flow-chad`
**Date:** 2026-07-29
**Dispatch:** `Plans/DISPATCH-2026-07-29-eth0-boot-product-flow-chad.md`

## What changed

- Added `boot-eth0`, a default-off boot-network kill switch.
- Extended `product`, an explicit product feature set that composes the current default posture with independent boot flows (`boot-neofs` and `boot-eth0`) instead of silently changing `default`.
- Added `kernel/src/drivers/usb/boot_eth0.rs`: production USB init now reuses the proven path when enabled: xHCI enumeration completes in `drivers::usb::init()` -> CDC-ECM/AX88179 bound as a `NetDevice` -> Ethernet-class ADR-068 identity (`eth0` on the product bare-metal path) -> bounded DHCPv4 attempts through `dhcpv4::demux_eth_rx`.
- Added feature-only boot-frame status rendering in `kernel/src/init/boot_frame_eth0.rs` when `boot-eth0` is enabled: `eth0: link up ip=<a.b.c.d>`, `eth0: no device`, `eth0: no link`, or `eth0: no lease`. The default `boot_frame.rs` stays byte-restored.
- Added `xtask build/iso/run* --features <list>` support so product boots are operator-reachable via `cargo xtask run-headless --features product` instead of a manual kernel-only build.
- Added `tools/boot-eth0-logic-tests` for the status/decision edge cases.
- Tightened the no-DHCP degraded path after QEMU showed the first bounded budget was too slow under TCG: four attempts now use 750,000 pump passes each, and CDC-ECM serial prints link notifications only on state change while keeping the notification counter intact.

## Degrade behavior

No CDC-ECM/AX88179 device bound:
`eth0: no device`, serial breadcrumb, boot continues.

Explicit link-down with no RX evidence after bounded settle:
`eth0: no link`, serial breadcrumb, boot continues.

No DHCP answer:
four DISCOVER attempts, each with a bounded 750,000-pass pump budget; final status `eth0: no lease`, serial breadcrumb, boot continues.

Lease acquired:
serial emits `link up ip=... router=... dns=... fence_refusals=...`; boot frame displays the IP.

## Kill-switch / feature posture

`default` is unchanged. `boot-eth0` is not in `default`.

`boot-eth0 = ["usb-host", "interface-identity"]`.

`product = ["kernel-diagnostic", "accel-sim", "inference-preemptible", "boot-neofs", "boot-eth0"]`.

OFF path containment:
the only production call site is at the tail of `drivers::usb::init()` and compiled under `#[cfg(all(feature = "boot-eth0", not(feature = "neo-test")))]`; the USB-local `boot_eth0` module and alternate boot-frame status module are also cfg-gated. With `boot-eth0` off, boot does not compile or call USB/product DHCP code through this dispatch.

## I-14 containment analysis

I-14(a), no governance lock on driver/ISR path:
the enabled path reuses ADR-047/ADR-046. xHCI ISR drains only `IrqSafeMutex` event state and atomics; CDC-ECM device locks are ordered in the USB band above AI levels. `boot_eth0.rs` itself imports no AI governance lock (`INFLIGHT`, `RECENCY`, `ARENA`, reservation, or `TABLE`) and runs from USB init after enumeration, not in ISR context.

I-14(b), driver stall degrades liveness only:
every product wait is bounded: 256 link-settle pump passes, four DHCP attempts, and 750,000 pump passes per attempt (3,000,000 maximum across all attempts). The pump services only the selected `NetDevice`, drains already-posted RX frames through ARP/DHCP demux, ticks the net stack, spins inside the fixed budget, then exits on lease or budget. A wedged NIC can delay only this bounded boot-network window; it cannot stall governance or panic the kernel.

I-14(c), DMA fencing:
no new DMA path was added. USB/xHCI and CDC-ECM still allocate connectivity buffers from `DmaBuffer::alloc_in_arena`, and every programmed xHCI/CDC address flows through `dma_arena::fence(...)` into `FencedDmaAddr`. The product status logs `fence_refusals`; close evidence requires `fence_refusals=0`.

No new invariant:
none added. The flow composes existing ADR-045/046/047/068 contracts.

## Verification so far

- RAG health check: `Invoke-RestMethod http://127.0.0.1:8404/health` - healthy (`ollama=true`, `qdrant=true`) on 2026-07-29.
- `cargo test -p boot-eth0-logic-tests` - PASS, 6 tests.
- `cargo check -p neoplutarch-kernel --features product` - PASS.
- `cargo check -p neoplutarch-kernel` - PASS.
- `(cd kernel && cargo build -Zjson-target-spec --bin kernel --features product)` - PASS.
- `cargo check -p xtask` - PASS.
- `cargo xtask build --features product` - PASS.
- `cargo fmt --all --check` - PASS.
- `cargo xtask build` - PASS in the real checkout after the USB-local + alternate boot-frame module source shape.
- Docker product ISO after rebase: `docker compose run --rm neo bash -lc 'set -o pipefail; mkdir -p target/eth0-product; env LIMINE_DIR=/opt/limine cargo xtask iso --features product 2>&1 | tee target/eth0-product/iso-product-rebased-daec5164.log'` - PASS; ISO ready at `/workspace/boot/neoplutarch.iso`.
- Product Docker QEMU/no-dongle proof after rebase: `target/eth0-product/qemu-product-no-dongle-rebased-daec5164.log`.
  - Serial includes `[ kernel ] eth0: no device (no CDC-ECM/AX88179 bound); boot continues`.
  - Serial includes `[ boot  ] boot sequence rendered (1280x800)` and `[ kernel ] phase 0 boot complete`.
- Product Docker QEMU/USB-net DHCP proof after rebase: `target/eth0-product/qemu-product-usbnet-dhcp-rebased-daec5164.log`.
  - QEMU device: `qemu-xhci` + `usb-net` on slirp user netdev.
  - Serial includes CDC-ECM bind as `eth0`.
  - Serial includes `[ kernel ] eth0: link up ip=10.0.2.15 router=Some([10, 0, 2, 2]) dns=Some([10, 0, 2, 3]) fence_refusals=0`.
  - Serial includes `[ boot  ] boot sequence rendered (1280x800)` and `[ kernel ] phase 0 boot complete`.
- Product Docker QEMU/USB-net no-DHCP proof after rebase: `target/eth0-product/qemu-product-usbnet-no-dhcp-rebased-daec5164.log`.
  - QEMU device: `qemu-xhci` + `usb-net` attached to isolated `hubport` with no host network.
  - Serial includes all four bounded DHCP attempts ending in `Selecting`.
  - Serial includes `[ kernel ] eth0: no lease after 4 bounded attempts; boot continues (frames=0 arp=0 ipv4=0 offer=0 ack=0 other=0 fence_refusals=0)`.
  - Serial includes `[ boot  ] boot sequence rendered (1280x800)` and `[ kernel ] phase 0 boot complete`.
- Strict panic/fatal marker scan across the three product QEMU logs above returned no hits.
- QEMU `usb-net` exposes no link-down property (`qemu-system-x86_64 -device usb-net,help`); explicit link-down remains covered by `boot-eth0-logic-tests`.
- Clean comparison worktree `E:\AI Automation\AltimitNeo-eth0-offcheck` at `origin/main` + eth0-only USB-local/alternate-boot-frame patch:
  - `cargo xtask build` - PASS.
  - `cargo test -p boot-eth0-logic-tests` - PASS, 6 tests.
  - `cargo check -p neoplutarch-kernel --features product` - PASS.
  - `(cd kernel && cargo build -Zjson-target-spec --bin kernel --features product)` - PASS.
  - `cargo fmt --all --check` - PASS.
  - Runtime/loadable ELF sections with `boot-eth0` OFF match `origin/main` byte-for-byte: `.text`, `.rodata`, Limine request sections, `.eh_frame_hdr`, `.dynsym`, `.dynstr`, `.hash`, `.gnu.hash`, `.rela.dyn`, `.data`, `.got`, and `.dynamic`.
  - `llvm-strip --strip-debug` output matches byte-for-byte: SHA-256 `1CBBCA8AB2CED494BD0BD23B170A8EC612782E52A4B3240FAB6028904F3F9970`.
  - Full debug ELF still does not match: even the minimal `init/mod.rs` module-selection edit changes DWARF/raw debug metadata. No allocated/loadable section drift remains after the alternate boot-frame rewrite.
- Worker close-gate: `C:\Program Files\Git\bin\bash.exe scripts/worker-close-gate.sh` - PASS after rebase on `origin/main=144e0da`; formatting, host workspace tests, workspace clippy, blocking kernel clippy, SPDX, and self-inspection all passed.
- Final manual four-shard CI gate: [run 30738551036](https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/30738551036) - PASS on exact SHA `7d795d4ed2f83e1ef5017aa4e2b5a1149a9503ed`.
  - Host checks and SPDX passed; QEMU shards 1/4, 2/4, 3/4, and 4/4 all passed.
  - The new `Feature-Gated Coverage (NON-BLOCKING, matrix boot-eth0)` job ran and passed its `cargo clippy kernel --features boot-eth0` step. This closes the OT-69 compile-coverage hole identified by the AAG.

## Still owed (recorded non-blockers)

- Decide whether the dispatch's "bit-for-bit" bar means full debug ELF equality or boot/runtime image equality. The latter is proven; the former remains false because selecting the feature-only boot-frame module still edits one default-compiled module file and DWARF records that source.
- JOINT OptiPlex bare-metal boot with USB-Ethernet dongle showing real boot-time lease with no test scenario, plus no-link/no-DHCP graceful evidence.
