<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: mt7601u channel-switch / cold-boot wedge — trap-mine the fix (step 1 of trap→fix→test) → Prometheus

**Date:** 2026-07-22
**From:** PM (Claude)
**To:** Prometheus (Qwen — the research/trap-mining seat; cross-model to Jarvis/Codex, who owns the WiFi lane and will build the fix from your findings)
**Status:** DISPATCHED — **super-deep prior-art trap-mining, HOST-ONLY, NO-GATE** (a FINDINGS doc that seeds the fix design + the failure/discriminator bank). The PL chose **trap-mine → fix → test**: ground the driver fix in prior art *before* touching code. **You are step 1.** Jarvis's fix is step 2; the JOINT bare-metal retry is step 3.

## Why this dispatch exists (the silicon result — read the RETURN first)
The 2026-07-22 JOINT bare-metal WiFi run (`mt7601u_assoc_baremetal`) went **RED** — LINK-UP is open. It is a **known hardware trap, not a code defect** (OT-39 in the ledger has the full RETURN). Two attempts:
- **Warm** (firmware resident from a prior boot): the resident-firmware branch **skipped upload**, then **MCU calibration timed out** → `mt7601u_bind_incomplete`.
- **Controlled cold** (Ubuntu driver blacklisted, dongle confirmed cold): Neo uploaded all 45,412 firmware bytes, booted the MCU, **DPD calibration completed** — then the **first `set_channel(6)` timed out and irrecoverably wedged the MCU**; endpoint recovery worked mechanically but three forced firmware reloads each transferred **0/45380 B** → `EndpointStall`; terminal `bind_fail=Some("firmware ready-poll timeout (-ETIMEDOUT)")`. **No scan / WPA2 / DHCP / LINK-UP; AP creds NEVER implicated** (never reached scan).

The June-21 `4f82155` proof passed because it used **Linux-warm firmware + channel 11 + stopped at association**. Today exercised the harder **cold-firmware + channel-6 + full DORA/LINK-UP** path — and **CB-2 already classified even a single channel-6 switch as capable of permanently wedging this dongle** (LKML "mt7601u dies during channel switch"; the wedge survives vendor_reset + host xHCI endpoint reset + device ClearFeature(ENDPOINT_HALT); **only a physical replug revives it**; **upstream Linux AVOIDS the wedge rather than recovering it**).

## What is ALREADY mined — do NOT re-derive it
The trap is heavily documented already. Read these first and **build on them, don't repeat them**:
- `Plans/RESEARCH-2026-06-14-mt7601u-bringup-trap-mine-FINDINGS.md` — the existing **27-trap bank** (§2). Your deliverable EXTENDS this bank; number your new traps continuing from it.
- `docs/ADR-054-phase-7.5-mt7601u-driver.md` — names F1-part-2 (the ch6 channel-switch wedge) as a characterized follow-on and mentions the **"upstream persistent-MCU-response-buffer pattern"** as the likely direction.
- The driver's own inline commentary (already cites LKML + CB-2): `kernel/src/drivers/usb/mt7601u/mod.rs` around L1620/1700-1800 (the wedge handling + fast-skip), `phy.rs` (`set_channel`), `mcu_logic.rs` / L1013 (`mt7601u_mcu_calibrate`), `upload_logic.rs` / `fw_logic.rs` (the cold-upload + ready-poll path, L1300-1355).
- OT-39 (the RETURN) + CB-2 in `docs/OPEN-THREADS.md` / `docs/bible.md`.

**Given how much is already known, this is a FOCUSED mine on TWO genuinely-open questions, not a survey.**

## The two open questions (mine upstream mt7601u/mt76 for the ANSWER, file:line-cited)
**Read the RAW Linux source** — `drivers/net/wireless/mediatek/mt7601u/` (`phy.c`, `mcu.c`, `initvals*.h`, `main.c`, `usb.c`, `dma.c`) — for the EXACT register/command sequences. **Never `WebFetch` for exact bytes/register sequences** (it mangles them and poisons the oracle — CLAUDE.md exact-bytes rule); pull the source directly and read the literal code.

**Q1 — how does upstream get the MCU onto the OPERATING channel WITHOUT a wedging mid-stream `set_channel`?** This is the primary question — the fix's whole shape depends on it. Chase, each file:line-cited:
1. **Is the wedge the channel-set itself, or the post-channel CALIBRATION?** `mt7601u_set_channel`/`mt7601u_phy_set_channel` fires `MCU_CAL` calibration commands (RXDCOC/BW/TX-group-delay/TXDCOC/RX-group-delay etc., `mt7601u_mcu_calibrate`) on every channel change. Determine precisely which MCU op wedges — the RF/BBP channel register writes, or the `MCU_CALIBRATE` command after. If it's the calibration, does upstream **skip or defer** calibration on the initial/targeted tune?
2. **Can the cold MCU come up ALREADY tuned to the target channel** (zero switch-delta)? Trace the upstream ordering: firmware boot → `mt7601u_init_hardware`/`mt7601u_phy_init` → the FIRST channel program. Is the initial operating channel programmed as part of PHY/RF bring-up (before the MCU enters the wedge-prone post-init state), rather than a separate later `set_channel`? Is there an initvals path that sets a default channel?
3. **Does upstream set channel via direct RF/BBP register writes vs the MCU command path?** If a register-write channel-set avoids the MCU round-trip that wedges, that is a candidate fix.
4. **The persistent-MCU-response-buffer pattern** ADR-054 named — what exactly is it (upstream `mt7601u_mcu_msg`/response-buffer reuse), and does adopting it change the wedge behavior, or only the timeout accounting?

**Q2 — the warm-firmware-resident resync path (the FIRST failure mode today).** When firmware is already resident (warm reboot), upload is skipped and **MCU calibration timed out**. Mine:
1. What does upstream do on re-probe when firmware may still be resident? Does it **always** re-upload, or detect-resident-and-fast-path? What reset/re-init (DMA re-init `mt7601u_dma_init`, WLAN reset, `mt7601u_reset_csr_bbp`, MCU FCE re-init) does it run **before** trusting resident firmware?
2. Is the warm calibration timeout because a required reset/DMA-init was skipped along with the upload? Name the exact upstream step our resident-fw branch omits.

## Design options to grade (VIABILITY verdict each — viable-now / viable-after-X / dead-because-Y + smallest de-risking step)
Given the answers, grade each candidate the fix could take:
- **(A) Bring firmware up already on the target channel** (no mid-stream switch). — the cleanest if Q1.2 supports it.
- **(B) Skip/defer the post-channel MCU calibration on the initial targeted tune** (calibrate lazily or once, off the wedge path). — if Q1.1 says calibration is the wedge.
- **(C) Channel-set via direct RF/BBP register writes instead of the MCU command.** — if Q1.3 supports it.
- **(D) Bound the channel-switch wait + a real resident-fw reset/resync** (Jarvis's (a)+(b)) — the incremental fix; grade whether it's sufficient ALONE or only a mitigation.
- **(E) Operational sidestep:** put the AP on the chip's power-on-default channel so no switch is needed — grade as a *test-unblocker* (get LINK-UP proof while the durable fix lands), and state which channel the cold MCU boots on.
Lead with the **1–2 options you'd actually build** and the ones you'd kill (with why). Cross-check against CB-2's "even one switch wedges" finding — a viable option must survive that, not assume it away.

## Deliverable + close
- `Plans/RESEARCH-2026-07-22-mt7601u-channel-switch-trapmine-FINDINGS.md` (CC-BY-4.0): the two questions ANSWERED with upstream file:line citations; **new traps appended to the 27-bank** (continue the numbering); each design option A–E with a VIABILITY verdict + the smallest de-risking step; and a **pre-seeded failure-mode/discriminator bank** for the eventual fix's bare-metal retry (what each expected failure looks like on serial, mapped to the two capture channels). Lead with the recommended fix shape for Jarvis to build.
- **HOST-ONLY, NO-GATE** (a research doc — no kernel build, no QEMU). Push to `Workers/prometheus` on `prometheus/mt7601u-channel-switch-trapmine`; PM cherry-picks the FINDINGS to `main` and scopes Jarvis's fix dispatch (step 2) from it.

## Relay — what to tell Prometheus
- **Read (all on `main`; your tree is on a feature branch, so `git show`, don't file-open):** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-22-mt7601u-channel-switch-trapmine-prometheus.md`. Prior art to build on (don't repeat): `git show origin/main:Plans/RESEARCH-2026-06-14-mt7601u-bringup-trap-mine-FINDINGS.md` + `…:docs/ADR-054-phase-7.5-mt7601u-driver.md` + OT-39/CB-2 in `…:docs/OPEN-THREADS.md`. The driver to trace: `kernel/src/drivers/usb/mt7601u/{mod.rs,phy.rs,mcu_logic.rs,upload_logic.rs,fw_logic.rs}` (already checked out in your tree). Work in `Workers\prometheus`; branch `prometheus/mt7601u-channel-switch-trapmine`.
- **One-line:** **Focused prior-art trap-mine (step 1 of trap→fix→test) for the mt7601u channel-switch MCU wedge that just went RED on silicon.** The trap is already documented (LKML/CB-2: cold-firmware + `set_channel(6)` irrecoverably wedges the MCU; upstream AVOIDS rather than recovers) — so DON'T re-derive it. Mine the RAW Linux `mt7601u` source (`phy.c`/`mcu.c`/`initvals`; never WebFetch for register sequences) for TWO answers: **(Q1)** how does upstream get the MCU onto the operating channel WITHOUT a wedging mid-stream switch — is the wedge the channel-set or the post-channel MCU *calibration*? can the cold MCU boot already tuned to the target channel? register-write vs MCU-command channel-set? and **(Q2)** the warm-firmware-resident resync path (resident-fw calibration timed out today — what reset/DMA-init does upstream run before trusting resident firmware?). Then grade design options A–E (boot-on-target-channel / skip-initial-calibration / register-write channel-set / bound-wait+resync / operational sidestep-to-a-no-switch-channel) with a VIABILITY verdict + smallest de-risking step each, cross-checked against CB-2's "even one switch wedges." Deliverable `Plans/RESEARCH-2026-07-22-mt7601u-channel-switch-trapmine-FINDINGS.md`, extending the existing 27-trap bank; lead with the fix shape you'd have Jarvis build. HOST-ONLY, no gate.
- **RAG** (semantic code search): use it for "where is the MCU calibration / channel-set / firmware ready-poll / resident-fw branch done?" lookups. Health-check `Invoke-RestMethod http://127.0.0.1:8404/health` first (expect `{"status":"healthy",...}`); if down, note it in your RETURN and fall back to grep/Read (the PL owns RAG startup; never a blocker). Optional self-serve start: `powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"` then `cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`. Query e.g.: `$body=@{query='mt7601u MCU calibration channel switch'; top_k=5}|ConvertTo-Json; Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.
