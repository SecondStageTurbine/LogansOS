<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Bare-metal gold-standard session — wlan0 cold lease + the 485 MB Arthur reply, on the OptiPlex → Cerebro

**Date:** 2026-06-24
**From:** PM (Claude)
**To:** Cerebro (owns the WiFi/CB lane; CB-1/CB-1b author). New branch off `origin/main` HEAD: `codex/baremetal-gold-standard-cerebro`. **Cerebro builds the real-brain ISO, SSH-deploys it onto the OptiPlex's Ubuntu image and sets up the log sinks** (the established bare-metal pattern); **the PL's only manual step is the physical reboot.** After reboot the sink captures the serial/breadcrumbs; Cerebro pulls + reads it and drives the iteration loop.

> **CORRECTION ADDENDUM (PM, 2026-06-29) — three facts that supersede the original framing below:**
>
> 1. **OT-37 / Arthur is ALREADY PROVEN on silicon — it is not a pending half.** Commit `3a6671e` (gold ISC-3, `arthur_real_brain_baremetal`) ran the 485 MB real Qwen brain on the OptiPlex and it replied coherently. The three silicon-only walls that run hit are all merged to `main` via their own dispatches: the SMP AP-ready barrier (`86f2058`), the `Inode::byte_len` OOM fix (`4279683`), and OT-40 xsave-preemptible inference (`9b4a875`). **A real-brain ISO built off current `main` runs Arthur correctly on silicon today.** So §0's "both not yet confirmed" is stale on the Arthur half — the **only live silicon proof is the CB-1b `wlan0` cold lease.** Arthur still re-runs on the same boot (freshly, off current `main`), but its pass is expected, not the new evidence.
>
> 2. **Branch: re-cut onto current `main` + cherry-pick the 2 gold commits — do NOT ff-merge the stale branch.** `codex/baremetal-gold-standard-cerebro` is 33 commits behind `main` (merge-base = the dispatch commit itself); a `--ff-only` is refused by design. It has exactly 2 unique commits — `42a3d65` (ISC-2 T-047 RX-filter readback instrumentation — *exactly* the discriminator this run needs) and `3a6671e` (ISC-3 Arthur harness). Per the stale-base remedy: `git checkout -B codex/baremetal-gold-standard-cerebro origin/main && git cherry-pick 42a3d65 3a6671e`. **Conflict watch:** both touch the mt7601u/wlan0 surface that **CB-2** (recovery, `37e91b5`) and **CB-3** (interface-identity, `9900c51`) rewrote — the close-gate MUST re-confirm the T-047 readback still reads the right registers post-CB-2, and the non-zero-`interface_id` TX discriminator targets the CB-3 registry index. Those two discriminators are what makes the run conclusive; a silent mis-resolution makes them lie. Re-run `scripts/worker-close-gate.sh`, push `--force-with-lease`.
>
> 3. **DECISIVE CONFIG — the test AP MUST sit on ch6 (or a channel the scan reaches before the CB-2 wedge).** CB-2 (`37e91b5`) established the mt7601u channel-switch wedge is a hardware wall (dead MCU, only a replug revives it), **but the scan finds the home AP on ch6/ch11 before the wedging switch.** A cold lease is therefore achievable only if the AP is on a channel reached *before* the wedge: associate + 4-way + DORA there, never triggering the switch. The generic "targeted channel" in ISC-3/§3.5 below means **ch6** for this run. This is the single most important config detail — get it wrong and the run wedges before it can lease.
**Status:** DISPATCHED — **the on-silicon gold standard.** Two headline proofs are QEMU/host-gate-proven and merged but **not yet confirmed on real hardware**: **CB-1b** (a real DHCP lease over `wlan0`) and **OT-37** (the booted real Qwen2.5-0.5B emits a coherent reply). This dispatch turns one OptiPlex boot of the real-brain ISO into the conclusive on-silicon confirmation of both. **Bare-metal ⇒ carries the mandatory blind-hardware failure-mode/discriminator section + capture-before-you-need-it + a prior-art trap recap.**

## 0. The decision this run makes
Turn **"proven in QEMU/host"** into **"proven on silicon"** for both:
- **CB-1b gold standard:** cold mt7601u firmware upload → targeted associate → 4-way + GTK install → **DHCP DORA over `wlan0` → real `ip=…`** (the untested-on-silicon paths OT-39 named: broadcast-RX HW filter T-046/T-047, HW GTK decrypt, the real RX ring, the non-zero `interface_id` TX branch).
- **OT-37 gold standard:** boot the bundled real 485 MB `.neom` → NeoFS load → vocab → frame the prompt → **forward → coherent reply** ("Hello! How can I help you today?"–class), confirming the full in-kernel 485 MB autoregressive emission QEMU TCG can't run in-gate.

The run must be **self-diagnosing**: on pass, both gold standards captured; on any failure, the serial breadcrumb localizes it to a single cause via §3.

## 1. References
```
git fetch origin
git show origin/main:kernel/src/drivers/usb/mt7601u/mod.rs        # wlan0 RX loop + DORA breadcrumbs (~3186 set_interface_id, ~3204 demux_eth_rx); cold fw upload; GTK install
git show origin/main:kernel/src/net/dhcpv4.rs                     # demux_eth_rx, the offer_rx→request_tx→ack_rx→ip= path
git show origin/main:tools/xtask/src/main.rs                      # the real-brain ISO build + NEO_STAGE_B_GGUF / NEO_STAGE_B_NEOM staging (no blob in git)
git show origin/main:userland/neo-arthur/src/main.rs             # boot: model load → frame_turn_ids → sys_generate → reply; the prompt_ids / reply breadcrumbs
git show origin/main:docs/OPEN-THREADS.md                        # OT-39 (CB-1b bare-metal untested paths), OT-37 (resolved, the gold-standard note), OT-40 (IF=0)
git show origin/main:Plans/DISPATCH-2026-06-23-phase-7.5-cb-1b-real-lease-cerebro.md   # CB-1b §5 discriminators
git show origin/main:Plans/DISPATCH-2026-06-22-stage-b-bundled-brain-bringup-jarvis.md # Stage-B §3 boot/storage discriminators + the breadcrumb ladder
git show origin/main:Plans/RESEARCH-2026-06-23-phase-7.5-closeout-wifi-trap-mine-FINDINGS.md  # the mt7601u trap bank
```

## 2. ISCs
- **ISC-1 — One turnkey boot exercises BOTH proofs.** OBSERVE first: does a single real-brain-ISO boot already run *both* the `wlan0` cold-lease path **and** the Arthur load+reply, emitting every breadcrumb? If yes, no code change (pure run — preferred, no re-gate). If a trigger is missing (e.g. the wlan0 DORA loop isn't auto-driven at boot, or Arthur waits on input), wire the **minimum** auto-trigger. **Two sequential boots from the same ISO are acceptable** if combining is risky (e.g. a boot param selects `wifi` vs `arthur`) — don't force one boot. *If ISC-1 touches kernel code it is kernel-touching ⇒ 4-shard QEMU gate + AAG before the PL flashes; if it's boot-order/config only, no re-gate.*
- **ISC-2 — Capture-before-you-need-it: every §3 discriminator's data must already print.** Audit the breadcrumb ladder against §3 and **add any missing capture before the run** — specifically: the WCID-0 ATTR readback (T-041) and RX-filter register (T-047) on the WiFi side; the model-load byte/ms + arena-resident-vs-cap on the Arthur side; a fault *address* (not just type) on any #GP/#PF path. You do not get to discover a missing capture on the failing boot.
- **ISC-3 — Build, SSH-deploy, and set up the log sinks (the established pattern).** Build the real-brain ISO (`NEO_STAGE_B_GGUF=<path>` / `NEO_STAGE_B_NEOM=<path>` → `phase9-real-quant` + `arthur-real-vocab` ON; **no blob in git**), **SSH it onto the OptiPlex's Ubuntu image** the same way prior bare-metal tests were staged, and **wire the log sink(s)** so the serial/breadcrumb output is captured to a file on reboot (the breadcrumbs must land in the sink, not just the screen). Pre-attach/confirm the mt7601u dongle; set the AP config (targeted channel, WPA2; SSID/PSK supplied out-of-band by the PL). **The PL's only action is the reboot** — then Cerebro pulls the sink over SSH and reads it. State the expected pass-breadcrumb lines verbatim so PASS is recognizable on sight in the sink.

## 3. Blind-hardware failure-mode + discriminator truth-table (mandatory — conclusive on pass AND fail)
Each row: breadcrumb stall point → most likely cause → discriminator. Pre-seeded from OT-39, CB-1b §5, Stage-B §3, OT-37.

**Arthur (485 MB) path:**
- stops at `neofs_mount` / `model_open` ⇒ NeoFS-on-real-disk / 485 MB read over the real block device (first untested-on-silicon storage path) ⇒ log block-read progress + indirect-block levels.
- stops at `model_load_resident` ⇒ arena OOM at real RAM ⇒ log bytes-resident vs arena cap (D4 ~700 MB should fit; confirm OptiPlex RAM).
- `forward token=0 ms=…` very large ⇒ real-CPU per-token latency (the honest first silicon number; no SIMD-JIT/TCG penalty here, real AVX2 — expect *faster* than the ~1.5 s/token host figure, but capture it).
- reply prints but is garbage (not the coherent class) ⇒ compare the boot `prompt_ids` against the ISC-B1 `PROMPT` (the gate proves they match; a silicon mismatch ⇒ a real-tokenizer/sidecar divergence) → then forward vs sampling.
- **IF=0 note (OT-40):** a long in-kernel generation runs with interrupts disabled on the calling CPU — if the box looks "hung" mid-generation but serial later resumes, that's the known OT-40 degradation, *not* a crash. Distinguish via whether `decode … per_token_ms` eventually advances.

**WiFi (`wlan0`) path:**
- no `fw_upload ok` ⇒ dongle enumeration / cold firmware upload over real USB (xHCI) ⇒ log USB enum + the upload completion code.
- `assoc ok` then no `offer_rx` ⇒ **the OT-39 broadcast-RX HW filter (T-047)** — broadcast Offer never reaches `rx_take()` on silicon ⇒ dump the RX-filter register + WCID-0 ATTR readback (T-041) + frames-seen count.
- `offer_rx` but no `request_tx` ⇒ wrong interface bind (the non-zero `interface_id` branch, OT-39) ⇒ log the bound id vs wlan0's registered index.
- `request_tx` but no `ack_rx` ⇒ TX on wrong device / AP silent / `chaddr` empty (prior-art trap) ⇒ log TX device + chaddr.
- `ack_rx` but no `ip=` ⇒ HW GTK decrypt of the broadcast (OT-39) / lease parse ⇒ dump decrypt status + parsed lease.

## 3.5 Prior-art trap recap (already mined — confirm seeded, don't re-derive)
The mt7601u/DHCP traps are banked (`RESEARCH-2026-06-23-…-FINDINGS.md`, T-028…T-048) and CB-1's discriminators are armed. Bare-metal-new surface: real **USB dongle enumeration** + cold firmware upload over real xHCI; real **UEFI vs OVMF memory map** for the 485 MB model-load arena placement (Stage-B §3.5); the real **broadcast-RX + HW GTK decrypt** (device-less in QEMU). Confirm each sits in the §3 table with a discriminator before the PL boots.

## 4. I-14 / scope
No new device driver, no forward/`.neom`/vocab/NeoFS change, no new invariant. The mt7601u DMA stays on the already-fenced path (I-14c, W1/V proven); the run takes no AI-governance lock. **Scope: make the session turnkey + conclusive and drive the iteration — not re-architect anything.** Any fix that a failing boot reveals is its own scoped follow-on (the Phase-6 bring-up archetype), not this dispatch.

## 5. Gate + close (PM)
- **No CI gate for the run itself** (it's bare-metal). **If ISC-1 changes kernel code**, that change takes the normal **4-shard QEMU gate + AAG** before the PL flashes; if it's boot-order/config/docs only, no gate.
- **PASS = both gold standards captured on silicon:** the serial shows `ip=…` (real wlan0 lease) **and** a coherent Arthur reply with `prompt_ids` matching `PROMPT`. **PARTIAL/FAIL = self-localized** to a single §3 row (then a scoped follow-on iteration).
- **The run is iterative (Cerebro deploys, PL reboots, Cerebro reads the sink).** Each cycle: Cerebro builds + SSH-deploys the ISO + sink → PL reboots → Cerebro pulls the sink, reads it against §3, ships the next instrumentation/fix if needed, redeploys → PL reboots again. The PL's repeated action is only the reboot. RETURN per iteration; the close is the captured PASS sink → PM updates the ledger (**OT-37 + CB-1b bare-metal-confirmed; Phase 7.5 headline proofs on silicon**).
- **Close-gate** for any code in ISC-1/2: `scripts/worker-close-gate.sh` + RETURN the green push-CI (+ QEMU URL if kernel-touching). **Disk reclaim** after any merge per CLAUDE.md.

---
## Relay — what to tell Cerebro
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-24-baremetal-gold-standard-cerebro.md` (+ OT-39/OT-37/OT-40 in `OPEN-THREADS.md`, CB-1b §5, Stage-B §3, the trap FINDINGS). **Branch:** `codex/baremetal-gold-standard-cerebro` off `origin/main` HEAD.
- **One-line:** **Prove both headlines on silicon — you build + SSH-deploy the ISO + set up the log sinks (the usual pattern); the PL just reboots.** Build the real-brain ISO (`NEO_STAGE_B_GGUF=<path>`, `phase9-real-quant`+`arthur-real-vocab` ON, **no blob in git**); SSH it onto the OptiPlex Ubuntu image and wire the log sink so all breadcrumbs are captured to a file on reboot. Ensure one boot (or a `wifi`/`arthur` boot-param pair) runs **both** the `wlan0` cold-lease path **and** Arthur load→reply. **OBSERVE first** whether the boot already drives both — if so it's a pure run (no re-gate); if a trigger is missing, wire the minimum (kernel-touching ⇒ QEMU+AAG first). **Capture-before-you-need-it:** confirm every §3 discriminator lands in the sink (WCID-0 ATTR readback T-041, RX-filter reg T-047, model bytes-resident-vs-cap, a fault *address* on #GP/#PF) — add any missing BEFORE deploying. **Conclusive on pass AND fail** via the §3 truth-table (Arthur: neofs_mount/model_load/forward/garbage-reply/OT-40-IF=0; WiFi: fw_upload/offer_rx[T-047]/request_tx[iface-bind]/ack_rx/ip=[GTK-decrypt]). **Scope: build+deploy+sink+conclusive+iterate — no re-arch, no new driver/invariant.** Loop: Cerebro deploys → PL reboots → Cerebro pulls the sink + reads → redeploys; RETURN per cycle; close = the PASS sink → PM marks OT-37 + CB-1b on-silicon-confirmed.
