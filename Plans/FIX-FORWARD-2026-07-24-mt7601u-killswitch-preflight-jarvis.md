<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# FIX-FORWARD: MT7601U operation ledger must preserve the OFF kernel → Jarvis

**Date:** 2026-07-24  
**From:** PM  
**To:** Jarvis  
**Priority:** Narrow pre-gate blocker; repair now  
**Rejected runner-token candidate:** `1e81219a60f25e7bed15c3cf1c8178c56b9d88e6`  
**Verified push CI:** `https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/30067353846` — exact SHA, push-green, QEMU correctly skipped  
**Verdict:** **HOLD — no four-shard runner token**

## Read first

```powershell
git fetch origin
git show origin/main:Plans/FIX-FORWARD-2026-07-24-mt7601u-killswitch-preflight-jarvis.md
git show origin/main:Plans/FIX-FORWARD-2026-07-23-mt7601u-aag-bound-dhcp-jarvis.md
git show origin/main:Plans/DISPATCH-2026-07-23-mt7601u-warm-postfw-fix-jarvis.md
```

The operation-wide ledger and truthful DHCP/LINK-UP terminal passed functional preflight. The token is withheld because the implementation breaks the dispatch's structural kill switch.

## Blocker — `mt7601u` OFF is no longer the prior USB kernel

In `1e81219a…`:

- `kernel/src/drivers/usb/mod.rs` compiles `operation_budget_logic` unconditionally whenever the USB module is built;
- `kernel/src/drivers/usb/xhci.rs` unconditionally adds the ledger atomics/types/functions;
- every existing `wait_event()` call now executes `charge_operation(WaitCall/WaitIteration)`;
- every existing EP0 control-event loop now executes the new late-event/desync accounting.

`charge_operation()` is a runtime no-op when no guard is active, but that is insufficient. With `usb-host` or `usb-hid` enabled and `mt7601u` disabled, the compiled xHCI code and data differ from the prior layer. The frozen requirement is `mt7601u` OFF **bit-for-bit prior kernel**, not merely behaviorally similar and not merely default-feature OFF.

## Required repair

- Compile `operation_budget_logic` only under `#[cfg(feature = "mt7601u")]` in the kernel USB module.
- Compile the xHCI operation-ledger import, atomics, snapshot/guard/accounting implementation, and all charge/note call sites only under the same feature.
- In `!mt7601u` builds, `wait_event` and the EP0 control loops must compile from the exact prior source path—no runtime inactive check, extra branch, counter, static, or wrapper.
- Keep the device-free host oracle available through its direct test-module include; do not broaden kernel reachability to make the test compile.
- Preserve the already-reviewed `mt7601u`-ON operation-wide accounting and DHCP/LINK-UP fail-hard semantics unchanged.

Use cfg-structural separation rather than a constant-false branch. Do not introduce a generic USB feature for this release-blocking fix; generalization is a later separately governed unit.

## Required kill-switch proof

Against baseline `0be953de11695ced27ef86dfe9b4e06f2abfd18a`, produce controlled same-toolchain artifact equality for representative USB compositions with `mt7601u` absent:

1. `usb-host + !mt7601u`;
2. `usb-hid + !mt7601u`;
3. the default kernel.

For each, record the exact build command, baseline artifact SHA-256, repaired-candidate artifact SHA-256, and byte-identical result. Also show:

```powershell
git diff 0be953de..REPAIRED_SHA -- kernel/src/drivers/usb/xhci.rs
```

and identify the cfg boundaries that remove every added hunk from `!mt7601u`.

Add a compile/KAT guard that builds the representative `usb-host` scenario without `mt7601u`; functional green alone does not replace artifact equality.

## Close gate and RETURN

1. Rebase onto current `origin/main` containing this dispatch.
2. Keep credentials committed as `SSID=b""`, `PASSPHRASE=b""`, `CHANNEL=0`.
3. Run the new kill-switch equality proof, MT7601U host oracles, format, host tests, workspace clippy, blocking kernel clippy, SPDX, and `scripts/worker-close-gate.sh`.
4. Rebuild the bare-metal ISO from the final repaired SHA and return its size/hash/manifest plus placeholder-restoration proof.
5. Commit and push; return exact SHA, base, own-stat, equality table, close-gate output, and exact push CI URL.
6. Do not fire `workflow_dispatch` until a new PM/PL exact-SHA token is issued.

No merge and no acceptance boot. After this narrow repair passes preflight, the PM will issue the four-shard token; the full AAG and cold→immediate-warm silicon close remain mandatory.
