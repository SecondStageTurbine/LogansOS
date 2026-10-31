<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: AAG WiFi `mt7601u` cross-family review (Janus)

**Date:** 2026-08-01  
**Candidate:** `origin/pm/wifi-merge` @ `3e47727ac9bcfc96ef1a0620567440108382c703`  
**Scope:** doc-only adversarial review; no build, runtime gate, hardware run, rebase, or source modification.

## Verdict: BOUNCE

One narrower blocking finding survives re-verification. The hostile oracle does bind two real, shipped `wedge_logic` decision seams: `ProductionPolicy` calls `channel_failure_action()` and `channel_fault_disposition()` from a read-only `#[path]` import, and production calls those same functions. The header's “Production code touched: None” describes the test's change footprint, not its coverage. The binding gap is instead the third production decision: `channel_retry_action()` is not invoked by the hostile oracle.

## Blocking finding — retry authorization is unbound from the hostile oracle

`ProductionPolicy::on_tune_failure()` calls production `wedge_logic::channel_failure_action()` and `channel_fault_disposition()` (`mt7601u_hostile_oracle.rs:680-714`). `qualify_policy(ProductionPolicy::default)` asserts that binding (`:2155-2167`). Those two calls are genuine production-policy coverage; the earlier broader finding is withdrawn.

The actual production decision sequence in `Mt7601uDev::set_channel_accounted()` is:

- starts `xhci::OperationBudgetGuard` and calls `phy::set_channel` (`kernel/src/drivers/usb/mt7601u/mod.rs:2796-2810`);
- collects the actual remaining ledger and decides whether to probe (`:2814-2822`);
- performs endpoint recovery, queue drain, and command-path proof (`:2823-2845`);
- authorizes or refuses the retry through `wedge_logic::channel_retry_action(wedge_logic::recovery_retry_observation(...))`, then conditionally executes the second `phy::set_channel` (`:2833-2898`).

The oracle contains no call to `channel_retry_action`. That omission is load-bearing: the retry is precisely the action guarded by the production comment citing `cea89e6a`, where re-issuing RF17–20 after only endpoint recovery wedged with no terminal (`mod.rs:2823-2831`). The oracle verifies the initial decision to enter recovery and the terminal disposition, but not the production predicate that requires the command path to be re-proven before retrying the operation.

**Required repair:** bind `channel_retry_action(recovery_retry_observation(...))` into `ProductionPolicy`/the hostile trace with negative controls for (1) transport recovery without decoded `CMD_DONE`, and (2) decoded `CMD_DONE` after the required probe. The driver and oracle must consume the same production helper at this authorization seam. The existing model runner and ledger may remain as hostile scaffolding; they are not the finding.

## Lens results

### 1. I-14(c) DMA fencing — clean by source review

The branch introduces no raw-address descriptor path in the reviewed MT transfer/recovery code. `send_staging()` fences the staging allocation, verifies the returned witness carries the exact physical address, then passes only `FencedDmaAddr` to `inband_out_sync()` (`mod.rs:1569-1576`). `ensure_mcu_response_armed()` performs the equivalent fence/address check before posting the IN TD (`:1602-1614`). xHCI `EndpointPipe::submit()` accepts `FencedDmaAddr`, not `PhysAddr` (`xhci.rs:1874-1889`); the recovery Set-TR-Dequeue uses the already-fenced pipe-ring address (`xhci.rs:1945-1977`).

Packet RX and data TX are also arena-fenced before submission (`mod.rs:2414-2420`, `:3341-3343`). The arena's witness constructor preserves the supplied address (`kernel/src/drivers/dma_arena.rs:142-157`), so the two explicit address-stability checks are meaningful and pass the address actually programmed to the device.

`fence_refusals=0` remains non-evidence for the device-less logical scenario. No new hardware-DMA observation was made in this review.

### 2. Shared xHCI / eth0 blast radius — no confirmed regression

The new endpoint-reset, stop, dequeue-reset, and clear-halt entry points are all `#[cfg(feature = "mt7601u")]`, and their only production caller is the MT recovery path. With `mt7601u` disabled, the pre-existing xHCI source behavior remains selected.

When MT is enabled, the operation ledger is process-global while active: `wait_event()` charges it (`xhci.rs:900-931`) and `OperationBudgetGuard` swaps the global wait budget (`:210-241`). That is shared-host infrastructure, not device-local storage. I traced the proven CDC-ECM data path: it uses async `EndpointPipe::submit()`/`completions()` under its own device lock (`cdc_ecm.rs:577-687`), not `wait_event()`. The recovery reset functions are never called on CDC endpoints. I found no current CDC-ECM control transfer that can overlap the MT guarded scan/recovery sequence, so this is not a confirmed eth0 regression.

**Maintenance note:** a future concurrent xHCI synchronous-control client would consume the active MT ledger and inherit its lowered `WAIT_BUDGET`; the guard has no controller-wide ownership. That coupling should be documented or made device/operation scoped before adding such a client.

### 3. I-14(b) liveness and lock isolation — bounded iterations, not wall-clock bounds

No unbounded wedge loop was found in the new recovery path. The channel operation has one non-replenishing ledger; the recovery reserve is compile-time checked against a maximum 13-command drain, five response attempts, 2,000 drain polls, and 20,000 probe polls (`mod.rs:178-221`; `wedge_logic.rs:217-229`). Its maximum structural budget is 1,330,070 charges. The post-firmware boundaries each use a 50,000-charge ledger, and firmware load, command waits, vendor retries, response retries, and endpoint commands all have finite loop/attempt caps.

These are **spin-count bounds, not wall-clock deadlines**. Both `udelay()` and `msleep()` are busy loops (`mod.rs:1263-1275`), and the code itself acknowledges this for xHCI waits. Thus the branch prevents infinite device-induced looping but does not prove a fixed boot-stall duration across CPU speeds. It must not be represented as a wall-clock timeout guarantee.

The xHCI ISR still drains only its bounded event cap and touches USB event state/atomics; the MT driver has no separate ISR. No AI-governance (`INFLIGHT`, `RECENCY`, arena, reservation, or table) lock acquisition was found on that path.

### 4. Structural kill switch — clean by source review

`mt7601u = ["wifi", "usb-host"]` remains default-off (`kernel/Cargo.toml:151-164`). The MT module, operation-budget module, driver registration, recovery APIs, and all new xHCI behavior are feature-gated (`kernel/src/drivers/usb/mod.rs:42-46, 241-245`; `xhci.rs:37, 109, 903`). The changed early sink setup is gated by MT bare-metal scenario features, each of which pulls `mt7601u`.

I found no unconditional reference outside the feature boundary that assumes the driver exists. `usb-host` without `mt7601u` therefore retains the prior USB path structurally. The candidate also adds a `usb-host + !mt7601u` compile KAT to the close-gate script, but it was not run for this doc-only review.

### 5. Hostile oracle — blocking finding above

The oracle genuinely binds the initial recovery-entry and terminal-disposition helpers from the shipped `wedge_logic` file. It does not bind `channel_retry_action`, the production authorization that distinguishes a merely recovered transport from a re-proven MCU command path. The finding is limited to that missing seam.

### 6. Completeness and credential hygiene

**Coverage:** the exact candidate workflow has the original blocking jobs only; it does not contain the later feature-gated coverage job. Since `mt7601u` is default-off, the exact green run did not compile or lint `kernel/src/drivers/usb/mt7601u/mod.rs` or the MT-gated xHCI additions. The host oracle compiles/imports selected pure modules, not `Mt7601uDev`'s production transfer/recovery implementation. QEMU has no MT7601U, so it cannot exercise the real device path. The later `origin/main` feature-gated matrix is non-blocking and is not an ancestor of this candidate; a merge-time rebase may add compilation coverage, but not hardware coverage. The only real-DMA/hardware evidence remains the prior bare-metal observation cited in the dispatch, not evidence collected on this exact SHA.

**Credentials:** `configured_target()` ships `SSID = b""`, `PASSPHRASE = b""`, and `CHANNEL = 0` (`mod.rs:590-616`). I reviewed the changed test/oracle material and found no real SSID or passphrase; the non-empty credential-like literals in shared host tests are synthetic protocol/KAT fixtures, while the hostile oracle contains no credentials. No secret is reproduced here.

## Method boundary

RAG health was verified healthy before discovery and was used for the requested xHCI, DMA, budget, and kill-switch searches. Every load-bearing conclusion above was rechecked against `origin/pm/wifi-merge` source at `3e47727a`. No build, test, CI rerun, hardware action, rebase, or gate was performed.
