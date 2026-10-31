<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH 3a: `product_integration_baremetal` — the ISO the 0.1 acceptance run boots → Chad

**Date:** 2026-08-11 · **From:** PM (Claude) · **To:** Chad (Codex)
**Status:** DISPATCHED — kernel-touching, **full 4-shard QEMU `workflow_dispatch` required**.
**Blocks:** queue item 3 (the 0.1 bare-metal showcase). Item 3 cannot start until this merges.
**Queue:** `Plans/QUEUE-2026-08-07-single-lane-priority-order.md` (item 3, the ⛔ 3a block).

## Why this exists

The ISO burned to the OptiPlex NVMe **predates your own item-1 work**. Build stamp
`2026-08-01-01-52-04`; 1a/1b/1c landed 08-07. The PL's `strings` check on that kernel, with a proper
positive control: `"neoplutarch|neofs|kernel"` → **71,200 hits**, while `"PRODUCT rung"`,
`"product_integration"`, `"SHOWCASE product"`, `"DW1 focus"`, `"arthur"` → **0 each**. Booting it would
produce a serial log with **zero rung tokens** — not a failed acceptance run but an **uninterpretable**
one, which the bare-metal runbook's conclusive-either-way rule exists to forbid.

And the obvious fix — build `product_integration` with `--bare-metal-mode` — is **rejected in two
different code paths**, so fixing the first only defers the second:
- the **sink allowlist** during feature assembly (`tools/xtask/src/main.rs` ~`:3325-3336`) fails at build;
- the **reboot-scenario guard** (~`:4063`) would then fail at run: *"--bare-metal-mode is not supported
  for reboot scenarios."* `product_integration` is in `is_reboot_persistence_scenario` (`:4290`).

## The design decision — read this before writing anything

**SINGLE BOOT PER INVOCATION, booted twice by hand. Do NOT build two-boot orchestration.**

`userland/neo-init/src/main.rs:252` selects the boot arm **from disk state**, not from a host-injected
boot index:

```rust
if let Some(readback) = product_integration_tests::read_prior_boot() {
    return if readback == 0 { 78 } else { readback };   // boot 2: short-circuits
}
let desktop = dw1_desktop_wm::run();                     // boot 1: rungs 2..5
...
product_integration_tests::write_first_boot()
```

`sys_open(PATH)` returns `ENOENT` on a fresh volume and `Ok` once the file exists. **The two-boot
driver exists only because QEMU has no human.** At the box the PL runs `efibootmgr -n 0001 && reboot`
twice and the userland self-selects. So:

- the new variant is **single-boot per invocation**;
- it is **deliberately NOT added to `is_reboot_persistence_scenario`** — that *dissolves* the `:4063`
  guard rather than relaxing it. Do not weaken that guard; other scenarios depend on it;
- **no per-boot expected-arm machinery, no host reboot driver, no runner changes.**

## Scope

**1. The scenario + its two features.** Mirror `arthur_compositor_baremetal` exactly — it is the
closest precedent and it is already wired end to end:
- kernel: `neo-test-product-integration-baremetal = ["neo-test-product-integration"]`
- userland: `product-integration-baremetal = ["product-integration"]` in `userland/neo-init/Cargo.toml`
- `TestScenario { name: "product_integration_baremetal", feature: …, user_feature: Some(…),
  ignored: true, limine_kaslr: None }`

**`ignored: true`** — same as every other bare-metal scenario. See *What the gate cannot prove* below;
say it in the RETURN rather than letting a green read as more than it is.

**2. NVMe sink, not AHCI — this one is load-bearing, not a copy-paste choice.**
`"product_integration_baremetal" => Ok("neo-test-log-sink-target-nvme")` in the allowlist. **NeoFS lives
on `sda`, a SATA/AHCI device** (Samsung 860 EVO), and it is the volume under test — an AHCI sink would
write into the same device the scenario is proving. The boot NVMe is the only free channel, and it
already carries the documented `NEO_LOG_v1` sink at offset 256 MiB. `vtd_fence` picks AHCI for the
mirror-image reason; do not pattern-match off it.

**3. The call-site audit — this is the crux of the build, not step 3 of 4.**
`product_integration` is matched **by exact name at eight or more sites**. The new variant needs the
*right subset* and there is no compiler error for getting it wrong:

| site (`tools/xtask/src/main.rs`) | what it does | variant needs it? |
|---|---|---|
| `:802` | per-scenario timeout | **yes** — bare-metal is long; `arthur_compositor_baremetal` uses 3600 s |
| `:4290` | `is_reboot_persistence_scenario` | **NO — must not match** (see design decision) |
| `:4340` | shard run-cost proxy | judgement — `ignored` scenarios still partition |
| `:5784`, `:6080` | QEMU memory / caps (`4096M`) | judgement — harmless if set, state which |
| `:6657`, `:7029` | storage-disk staging + brain model | **judgement — the real model comes from `sda` on hardware** |
| `:7329` | mkfs variant (`arthur-memory`) | **yes** if a disk is staged at all |
| `:9119-9168` | expected-output arms | judgement for an `ignored` scenario |

**State your decision per row in the RETURN, including the ones you decide are N/A and why.** A silent
omission here reproduces **OT-75** exactly — *"compile success doesn't prove the scenario received its
registered authority"* — where DW-1/DW-2 were in the inner push, missing from the outer `cfg`, and
booted to an opaque exit 40. That is the failure mode this table exists to prevent.

**4. Kill-switch (ISC).** The new features OFF ⇒ every existing build is **bit-for-bit unchanged**:
`product_integration` keeps its `ignored: false` QEMU behaviour, its reboot-persistence classification,
its budget, and its expected output. The new scenario is purely additive. State the equality claim in
the form that is *true* — you have been burned by blanket wording before (Cerebro's rung-5 F1/F2 note).

## Explicitly out of scope

- **No bare-metal run.** That is item 3, PL-joint. You produce the artifact; you do not boot it.
- **No touching `sda`.** The volume is rebuilt, verified, and stable — three superblocks, free map
  MATCHES, all five content hashes including the 485 MB model at `a32e4a1b…`. Do not re-`mkfs` it, do
  not write to it, do not stage over it.
- **No weakening of `is_reboot_persistence_scenario`** or the `:4063` guard.
- **No budget raises** on any existing scenario.
- **Not OT-82.** *(The `product` feature lacking the compositor/HID/Arthur is a real finding and a PL
  decision about what 0.1 ships — do not "fix" it here.)*

## What the gate CANNOT prove — say this in the RETURN

`ignored: true` means **the 4-shard QEMU gate never executes this scenario.** A green gate proves the
tree compiles, the shard partition stays complete and disjoint, and no existing scenario regressed. It
proves **nothing about whether the new bare-metal variant works** — the first real exercise is the
OptiPlex boot. That is expected and correct (this is a bare-metal-only scenario), but it must be stated
plainly, because a green 13/13 on this branch reads like more assurance than it carries. This project
has spent the week finding assertions with nothing behind them; do not let the gate itself become one.

## Failure-mode + discriminator bank (blind-hardware rule — this build owns the run's diagnosability)

You are not booting it, but **whether item 3's run is conclusive is decided here**. For each mode below,
the discriminator must exist in the artifact you ship, not be improvised at the box:

1. **The instrumentation is absent again** (wrong ISO burned, stale artifact). *Discriminator:* the
   build must emit a stamp the PL can check on the burned ISO **before** booting — same shape as the
   `strings` check that caught the stale one. Name the exact grep in the RETURN.
2. **Boot 1 short-circuits into the readback arm** because `.arthur/memory` is non-empty. *Discriminator:*
   the boot-2 line (`readback bytes=39 source=prior_boot …`) appearing on what was supposed to be boot 1
   **is** the signal — and it means rungs 2 and 3 never ran. **The run is NOT idempotent**; a retry after
   any completed boot 1 requires resetting the volume. Say so in the RETURN so it reaches the runbook.
3. **Boot 2 looks like a failure because it is short.** It short-circuits before desktop and Arthur,
   emits one line, and returns **78 — a nonzero success**. *Discriminator:* document the expected boot-2
   serial shape explicitly so a correct run is not misread as a broken one.
4. **The eth0 latch aborts before any rung token.** `kernel/src/test_harness.rs:2263` fails on
   `debug_status() != NoDevice` **before** the rung-1/rung-4 emissions, so a plugged-in AX88179 kills the
   run with no tokens at all. *Discriminator:* the `reason=product_eth0_expected_clean_no_dongle_degrade`
   fail string is already distinctive — confirm it survives into the bare-metal build and name it, so the
   PL can tell "dongle in" from "boot broke."
5. **The sink is wrong or unreadable.** *Discriminator:* the NVMe `NEO_LOG_v1` magic at 256 MiB is the
   readback contract; confirm the variant targets it and the runbook's `dd … | strings` recovery applies.

## Governance

- **I-14 fault containment:** N/A with reason — no new driver, no new device-initiated DMA. The sink
  selection reuses the existing NVMe log-sink path. State this rather than omitting it.
- **Concurrency pitfall bank:** N/A with reason — no cross-CPU, interrupt, lock, atomic, TLB-shootdown
  or scheduler-wake code. State it.
- **Byte oracle:** N/A — no new on-disk or on-wire byte layout.

## Close-gate + merge

Run **`scripts/worker-close-gate.sh`** (rebase-freshness, the exact CI host-check set, SPDX,
self-inspection). Then the **4-shard `workflow_dispatch` on the exact SHA** — paste the URL; *green
without a CI URL is not done.* Rebase onto `origin/main` immediately before declaring close.

**The RETURN is a pushed artifact** at `Plans/RETURN-2026-08-11-3a-product-integration-baremetal-chad.md`
— it must carry the per-row call-site table, the kill-switch equality claim in its true form, the
gate-cannot-prove paragraph, and the five discriminators.

**Disk reclaim (after PM confirms merge):** once the commit is on `origin/main`, `cargo clean` this
build's tree to reclaim `target/`. **Never delete a tree whose branch is unpushed/unmerged** — verify
the work is on `origin/main` first.

## Relay — Chad

- **WORKTREE:** `E:\AI Automation\workers\chad` (yours alone — sibling outside the repo, never a nested `Workers/`)
- **BRANCH:** `codex/product-integration-baremetal-chad`
  (`git fetch origin && git checkout -B codex/product-integration-baremetal-chad origin/main`)
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-08-11-3a-product-integration-baremetal-chad.md`;
  the queue's item-3 block via `git show origin/main:Plans/QUEUE-2026-08-07-single-lane-priority-order.md`;
  **OT-82** and **OT-75** in `git show origin/main:docs/OPEN-THREADS.md`.
- **One-line:** **The ISO on the OptiPlex predates your own item-1 work** — stamp `2026-08-01`, and its
  kernel has **0 hits** for `PRODUCT rung` / `product_integration` / `arthur` against a **71,200-hit**
  positive control, so booting it yields a log with no rung tokens: uninterpretable, not failed. Build
  the bare-metal variant that item 3 will actually boot. **The design is already decided and it is the
  thing most likely to be over-built: SINGLE BOOT PER INVOCATION, booted twice by hand — do NOT write
  two-boot orchestration.** `neo-init/src/main.rs:252` picks the arm from **disk state**
  (`read_prior_boot()` → `None` on `ENOENT`), so the QEMU reboot driver exists only because QEMU has no
  human; at the box the PL presses the button twice. Therefore the variant is **deliberately NOT** in
  `is_reboot_persistence_scenario`, which *dissolves* the `--bare-metal-mode` guard instead of relaxing
  it — **do not weaken that guard.** **Four things: (1)** the scenario + `neo-test-product-integration-baremetal`
  + `product-integration-baremetal`, `ignored: true`, mirroring `arthur_compositor_baremetal`. **(2)**
  **NVMe sink, not AHCI** — NeoFS lives on `sda` (SATA/AHCI) and is the volume under test, so an AHCI
  sink would write into the device being proven; the boot NVMe already carries `NEO_LOG_v1` at 256 MiB.
  **(3) The crux: the call-site audit.** `product_integration` is matched **by exact name at 8+ sites**
  (`:802` timeout, `:4290` reboot-persistence — **must not match**, `:4340` cost proxy, `:5784`/`:6080`
  memory, `:6657`/`:7029` disk+brain staging, `:7329` mkfs variant, `:9119` expected output). **State
  your decision per row in the RETURN including the N/As** — a silent omission here reproduces **OT-75**
  verbatim, where a scenario compiled, booted, and got an opaque exit 40 because two config surfaces
  disagreed. **(4)** Kill-switch: features OFF ⇒ existing builds bit-for-bit unchanged, stated in the
  form that's *true*. **Say in the RETURN what the gate cannot prove:** `ignored: true` means the 4-shard
  gate **never executes this scenario** — green proves it compiles and the partition holds, nothing more;
  the first real exercise is the OptiPlex boot. **Out of scope:** no bare-metal run, **do not touch `sda`**
  (rebuilt and verified — free map MATCHES, all five content hashes incl. the 485 MB model at
  `a32e4a1b…`), no guard weakening, no budget raises, not OT-82. **Discriminators matter more than usual
  here** — you're not booting it, but you decide whether the PL's blind run is diagnosable: an ISO-stamp
  grep the PL can run *before* booting, the non-idempotency warning (boot 1 only exercises rungs 2–3
  while `.arthur/memory` is empty), the fact that **boot 2 is one line and returns 78 — a nonzero
  success**, and the `reason=product_eth0_expected_clean_no_dongle_degrade` string that distinguishes
  "dongle left in" from "boot broke."
- **RAG** (semantic code search) is available — use it for conceptual "where is X done?" questions
  (grep still wins for exact symbols). Health-check first:
  `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`. If down/degraded:
  the PL manages RAG infra — note it in your RETURN and continue with direct file reads (never a
  blocker). Optional self-serve start (PowerShell):
  `powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"` (if `ollama:false`), then
  `cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
  Query e.g.: `$body=@{query='bare metal log sink target selection'; top_k=5}|ConvertTo-Json;
  Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`
  — also try `product_integration scenario registration`, `is_reboot_persistence_scenario`,
  `arthur_compositor_baremetal wiring`.
