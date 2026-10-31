<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Bare-metal Arthur on the OptiPlex — the headline run (OT-28 + OT-41) → Cerebro + PL

**Date:** 2026-07-18
**From:** PM (Claude)
**To:** Cerebro (builds the ISO + the diagnostic instrumentation + reads the serial) **and the PL** (burns media, boots the OptiPlex, photographs the screen). A joint blind-hardware run.
**Status:** DISPATCHED — **bare-metal blind-hardware run** (MMIO/serial breadcrumb observability only; no JTAG). Everything is coded + merged; this proves it on silicon. Closes **OT-28** (a real Qwen2.5 brain runs on bare metal) **and OT-41** (Arthur's reply legible on the physical framebuffer) in ONE run.

> **Run procedure (authoritative): `docs/runbook-baremetal-arthur-optiplex.md`** — `git show origin/main:docs/runbook-baremetal-arthur-optiplex.md`. SSH access (Ubuntu live USB), disk layout (`NEOFSv1` model volume + the NVMe log sink at 256 MiB / `NEO_LOG_v1`), `efibootmgr -n 0001` one-shot boot, PuTTY serial → `C:\Users\<user>\neoplutarch-serial.log`, **let it run to completion ~3-4 min (do not halt early)**, photograph the full reply, boot-select back. The run is **JOINT** (PL boots/photographs/serial; coder drives SSH + reads the log/sink). Per the standing rule, every bare-metal dispatch references this runbook.

## The goal (what a PASS looks like)
Burn the ISO → boot the OptiPlex → the framebuffer console shows: **butler first-boot greeting → the thinking-liveness indicator while a real `sys_infer_run` forward pass runs on the OptiPlex CPU → Arthur's decoded reply, legible on the physical monitor.** A photo of that screen + the serial log is the deliverable. (Interactive keyboard input is a stretch goal — see failure-mode F6.)

## Pre-run build (Cerebro)
1. **Bundle the ISO:** the real `.neom` + vocab sidecar (OT-28 L1/L2/F3) into the bootable image, NeoFS large-file storage for the model. **Start with the SMALLEST proven config for the FIRST boot** (de-risk the boot+load path on real RAM), then re-run at 1.5B. State which config each run uses.
2. **Arm the serial breadcrumbs (capture-before-you-need-it):** the run must be conclusive whether it passes OR fails, from the serial log alone. Emit a serial marker at EVERY stage so a hang/fault localizes to one: `BOOT: limine handoff` → `FB: GOP mode WxH stride=…` → `HEAP: N MiB` → `NEOFS: mounted, model sidecar N bytes` → `MODEL: .neom parsed, L layers, quant=…` → `INFER: forward pass begin` → per-layer trace (reuse B3's `REAL_LAYER_TRACE`) → `INFER: complete tokens=N` → `REPLY: <decoded>`. The screen shows the human-facing version; the serial is the diagnostic.
3. **Fault capture:** on any `#PF`/`GP`/`RING_FAULT`, print the faulting address + type + the current stage marker to serial (not just halt) — a blind fault you can't localize is a dispatch defect, not bad luck.

## Expected-failure-mode analysis + discriminators (MANDATORY — design for failure, not just success)
The base rate on a first blind bring-up is "it reveals the next layer." Enumerate the likely failures even if the primary path is correct, each with the observation that maps to the cause:

| # | Failure mode | Discriminator (what to observe → which cause) |
|---|---|---|
| F1 | **Won't boot** (media/UEFI/Limine) | No serial at all → media/UEFI boot-order (try USB vs the burn; confirm UEFI not legacy). First serial line present → past Limine. |
| F2 | **Boots, blank/garbled screen** | `FB: GOP mode …` serial present but nothing on screen → GOP mode/stride mismatch or the iGPU fighting scanout (ADR-032: fb is expendable during compute — confirm we're NOT modesetting). Serial reply but blank fb → `fb_console` init/format. |
| F3 | **Heap OOM on real RAM** | `HEAP: N MiB` < model need → the OptiPlex RAM vs the model's heap requirement (2.75 GiB for Qwen2.5-3B class). Discriminator: the heap-size marker vs the model-size marker. Fix = smaller config or confirm RAM. |
| F4 | **Model load fails** (NeoFS read / `.neom` parse) | `NEOFS: mounted` but no `MODEL: parsed` → the large-file sidecar read path on real NVMe/USB storage (not QEMU virtio). Per-stage markers localize NeoFS-mount vs sidecar-read vs parse. |
| F5 | **Forward pass faults or wrong output** | `INFER: begin` then a fault → the per-layer trace names the failing layer (reuse the B3 localizer); the byte-oracle/expected-token check catches a wrong-but-non-faulting result (real CPU vs TCG numerics — magnitudes differ, top-1 should not). |
| F6 | **No keyboard input** (stretch goal) | The v1 surface auto-greets + auto-runs a forward pass with NO input needed (proves OT-28+OT-41). Interactive typing needs a real HID/PS-2 keyboard driver — breadcrumb whether the OptiPlex keyboard enumerates. If input is dead, the run STILL passes on the auto path; log it as the next lane. |

## Prior-art trap-mining (before the first boot)
Mine the OptiPlex-specific gotchas: the exact model (7050/7060-class? confirm chipset), its UEFI boot quirks (Secure Boot off? UEFI vs legacy), the GOP framebuffer format, NVMe-vs-USB boot order, and the iGPU-scanout interaction (ADR-032 — do NOT modeset; inherit Limine's GOP). Seed each into the failure bank above with its discriminator, not from first principles.

## Scope guard
The proof is a physical PASS (photo + serial), not more code. Do NOT modeset the display (inherit Limine's GOP — the compositor/OT-63 owns that later). Do NOT block the headline on interactive input (F6 is a stretch). If the run reveals a real code fault, that becomes its own bounded fix dispatch with the captured discriminator — don't rabbit-hole live.

## Gate + close (blind-hardware, no CI gate — this IS the gate)
- **RETURN:** a **photo of the OptiPlex screen** showing the legible greeting + liveness + reply, the **full serial log** (the breadcrumb chain), the config used, which failure modes (if any) fired + their discriminator readings, and the explicit statement of which rows are proven (OT-28 Stage-B, OT-41 legibility). A failure RETURN is equally valuable if it's self-diagnosing (names the stage + captured fault).
- On a clean PASS: OT-28 Stage-B + OT-41 close; the headline goal ("boot the OptiPlex and talk to Arthur") is met on the smallest config, with the 1.5B run as the immediate follow-on.

---
## Relay — what to tell Cerebro (+ the PL runs the hardware)
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-18-baremetal-arthur-optiplex-cerebro.md` (+ OT-28 [what's landed — the real `.neom`/sidecar/NeoFS/forward path], OT-41 [the v1 visual surface + liveness], ADR-032 [iGPU-scanout: fb is expendable, don't modeset], ADR-063/064/066 [the model bundle/storage/forward path]). **This is a joint run: you build the ISO + arm the serial breadcrumbs + read the log; the PL burns the media, boots the OptiPlex, and photographs the screen.**
- **One-line:** **Bare-metal Arthur on the OptiPlex — burn the ISO, boot, and prove Arthur greets + runs a real forward pass + the reply is legible on the physical screen (closes OT-28 Stage-B + OT-41 in one run).** Bundle the real `.neom` + vocab sidecar (**smallest proven config first to de-risk the boot/load, then 1.5B**); **arm serial breadcrumbs at EVERY stage** (limine→GOP→heap→NeoFS→model-parse→per-layer-trace→tokens→reply) so any hang/fault self-localizes; capture the faulting address+type+stage on any #PF/GP. Design for the failure bank (won't-boot / blank-fb / heap-OOM / model-load-fail / infer-fault / no-keyboard) each with its discriminator; trap-mine the OptiPlex UEFI/GOP/boot-order/iGPU-scanout gotchas first (don't modeset — inherit Limine's GOP, ADR-032). Deliverable = a photo of the legible screen + the full serial log + which rows proven. A self-diagnosing FAILURE is a good RETURN too. **The FIRST run's job is to be conclusive, not necessarily green.**
- **RAG:** use it for our own boot/fb/model surfaces (`limine boot handoff framebuffer GOP`, `fb_console init stride`, `neom parse load sidecar`, `NeoFS large file mount`, `sys_infer_run forward pass`, `heap size init`). Health-check `Invoke-RestMethod http://127.0.0.1:8404/health`; if down, note + grep/Read.

---
## ADDENDUM (2026-07-18, PM): the hardware census — make the OS self-report before it loads the model

The principle behind the whole failure/discriminator bank: **don't hand-catalogue the OptiPlex — make the OS tell you what it's on.** The first bare-metal boot's *first job*, BEFORE the model load, is a **hardware census dumped to serial**. This is the capture-before-you-need-it rule made concrete: a boot that only prints the census and then dies is still a big win — you now know the exact machine and every later failure has a frame to localize against. It also front-loads the F3 (heap-OOM) decision: the census `RAM:` line vs the model's heap need tells you which config can boot *before* you spend a boot discovering it can't.

### (1) Census boot-phase — emit to serial before `MODEL:` (each line answers a discriminator)
Add these breadcrumbs at the very start of boot, ahead of the model-load chain already specced above. Keep them cheap and unconditional (they must print even if the model path is broken):

| Serial breadcrumb | Answers / de-risks |
|---|---|
| `CENSUS: CPU family=… model=… stepping=… cores=N RDRAND=y/n features=…` | SMP degree for `NEO_SMP`; confirms **RDRAND returns real entropy on silicon** (unlike pure-TCG where the CPUID bit lies) so the CSPRNG/getrandom path is sound on hardware |
| `CENSUS: RAM usable=N MiB (map: …)` | **F3 heap-OOM** — compare directly against the model's heap need (qwen35 ≈ 1 GiB heap; 3B ≈ 2.75 GiB). The single most likely first-boot blocker; this line makes the config decision falsifiable |
| `CENSUS: FB GOP mode=WxH stride=… bpp=… addr=…` | **F2 blank/garbled** — the exact framebuffer geometry Limine handed off; the number that predicts whether `fb_console` renders. (Do NOT modeset — inherit Limine's GOP, ADR-032) |
| `CENSUS: STORAGE controller=… kind=NVMe/SATA/USB boot_dev=… sidecar=<path> found=y/n bytes=N` | **F4 model-load** — the real NVMe/USB read path (not QEMU virtio); localizes mount-vs-sidecar-read-vs-parse |
| `CENSUS: INPUT keyboard=USB/PS2/none enumerated=y/n` | **F6** — whether interactive typing is even possible (stretch goal; the auto-greet + auto-forward-pass path runs regardless of this) |

The census reuses facts the OS already has at hand (the Limine memory map + framebuffer response, CPUID, the storage enumeration, the HID probe) — it's a serial dump of existing state, not new drivers. If any census line is data the driver doesn't yet expose, adding that exposure is part of THIS dispatch (capture-before-you-need-it — you do not get to discover the gap on the failing run).

### (2) Facts only the physical box / BIOS can give (PL reads these off the machine before the burn)
The census can't report what the firmware decides. Pin these first — they're the F1 "won't boot" discriminators and the config input:
1. **Exact model + chipset** — 7050 / 7060 / 7070-class OptiPlex? (Still unconfirmed in v1 of this dispatch.) Drives which UEFI/GOP/boot-order gotchas to trap-mine.
2. **UEFI settings:** Secure Boot **OFF**; **UEFI mode, not Legacy/CSM**; **boot order** (does USB precede the internal NVMe?).
3. **Total RAM installed** — the expected value the census `RAM:` line is checked against, and the input to the config choice.
4. **iGPU is the scanout** — UHD 630 (Gen9); confirm we inherit Limine's GOP and do **not** modeset (ADR-032, fb is expendable during compute).
5. **Where the `.neom` lives** on the boot media — same USB as the ISO, a second stick, or the internal drive (sets the F4 read path).

### (3) Trap-mine the EXACT model before the first burn
Seed the failure bank from documented OptiPlex-specific gotchas (vendor UEFI notes, OSDev, the exact model's forums) — the UEFI boot quirks, GOP framebuffer format, NVMe-vs-USB boot order, and iGPU-scanout interaction — each with its discriminator, BEFORE booting. The cheapest iteration is the one prior art already paid for.

### (4) First-config decision (state it in the serial log)
**Smallest proven config first** to de-risk the boot+load path on real RAM, then step to 1.5B. Every run's serial log states which config it used, so a PASS/FAIL is attributable to a specific config against the census `RAM:`/`HEAP:` lines.

### (5) Model selection — LOCKED (PL-ratified 2026-07-18)
**The bare-metal run boots the MERGED Qwen2.5 brain from `main`, NOT the Qwen3.5 B3 brain.** Verified composition:
- **Model:** `qwen2.5-0.5b-instruct-q4_k_m.neom` (staged as `model-qwen2.5-0.5b.neom`) first; **1.5B → 3B are the follow-on tiers.**
- **Forward path:** `neo-test-arthur-real-brain-boot = ["neo-test","serial-stdin","kernel-sysstat","phase9-real-quant"]` — the **hardcoded Qwen2 attention/FFN forward = kill-switch-1**. It does **not** pull `ot16-graph-walker` or the DeltaNet ops.
- **Base:** main-lineage (Qwen2.5-merged), **not** Jarvis's B3 branch.

**Rationale:** the Qwen2.5 hardcoded forward is the gate-proven, byte-identical-to-prior-layer path on `main`, so a bare-metal PASS is unambiguous and a FAIL localizes to hardware/boot. The **B3/Qwen3.5 Gated-DeltaNet brain** (separate `ot16-walker`/DeltaNet feature, Jarvis's unmerged worktree) is **AAG-bounced** (self-referential oracle, not yet anchored to an independent llama.cpp golden — see `Plans/DISPATCH-2026-07-18-b3-aag-bounce-byte-oracle-provenance-jarvis.md`) and **not on `main`** — booting it first-silicon would judge the headline against an unaudited oracle. **Swapping to the Qwen3.5/DeltaNet brain is a DELIBERATE follow-on run** (different `.neom` + the walker feature + B3 landed/mergeable), never the default and never for run 1.
