<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Bare-metal OptiPlex session — 3 silicon proofs → Oracle, Chad, Jarvis

**Date:** 2026-07-23
**From:** PM (Claude)
**To:** Oracle (Arthur-perf), Chad (Slice-4 EU), Jarvis (WiFi scan-and-join)
**Status:** DISPATCHED — **bare-metal JOINT runs (PL + coder), NO new code.** Coding is frozen (PL decision); this is the pivot to hardware. Each run is one boot cycle on the OptiPlex 7080; each proves something QEMU structurally cannot.

## Read first (all three coders)
`git fetch origin && git show origin/main:docs/runbook-baremetal-arthur-optiplex.md` — the authoritative access/boot/capture procedure. Do NOT re-derive it. Summary of the mechanics it defines:
- **Access:** `ssh -i ~/.ssh/id_ed25519 ubuntu@<optiplex-ip>` (control OS = Ubuntu live USB; passwordless `sudo -n`). NeoPlutarch has no sshd — you drive the *run* over SSH, the OS-under-test has no network back.
- **Boot the target:** `sudo efibootmgr -n 0001 && sudo reboot` (one-shot). After the run the box falls back to **Windows** unless boot-selected → **the run is JOINT** (the PL is physically at the box for boot-select + the live on-screen read; the coder drives SSH + analysis).
- **Capture (the durable proof):** the **PL** runs PuTTY as a serial terminal on their PC, logging to **`C:\Users\<user>\neoplutarch-serial.log`** — that file is your analysis input AND the deliverable. Backup channel = the **NVMe log sink**: after the box returns to Ubuntu, `sudo -n dd if=/dev/nvme0n1 bs=1M skip=256 count=16 2>/dev/null | strings`.
- **Let it run to completion (~3-4 min) — do NOT halt early.** There is **NO framebuffer photo** (the PL does not take them); the serial `REPLY:`/`PASS` line on a proven-non-blank framebuffer (PL's live eyes-on) IS the legibility proof.
- **If `/tmp` was cleared by a reboot,** the staged ISO is gone → rebuild from your branch (below) and `scp` it to `ubuntu@<optiplex-ip>:/tmp/` first.

## Run order (proposed): Arthur-perf → Slice-4 EU → WiFi. Each is a separate boot.

---
### TEST 1 — Arthur perf tok/s (Oracle)
- **Branch / ISO:** `codex/arthur-perf-lane-fast-forward-oracle` (staged `~11cdb364…` at `/tmp/`).
- **Proves:** the **real tokens/sec magnitude** on silicon — TCG cycles proved only the wiring (AVX2 fork + threaded matmul). This closes OT-52's last perf-caveat open item.
- **Expected serial:** `INFER: pos=0..21` (prefill) → `sample step=0..31` (gen) → `REPLY: <text>` + the timing/tok-s readout.
- **Failure-mode + discriminator bank (run must be conclusive pass OR fail):**
  1. **Slow-gen mistaken for hang** — ~3-4 min linear generation (0.5B, scalar single-thread, ~4 s/tok) is EXPECTED. Discriminator: if `sample step=N` keeps advancing, it's generating, NOT hung; only a fixed step stalled >30s is a hang. **Do not halt early** (the OT-28 premise).
  2. **Blank framebuffer but live compute** — serial captures independently of the framebuffer. Discriminator: `REPLY:` on serial + PL sees blank screen → framebuffer issue, not compute (report separately).
  3. **Perf number itself is the deliverable** — capture tok/s (or total gen wall-time / 32 tokens). Label it `real_silicon` (honest magnitude), distinct from the TCG `tcg_cycles_virtualized` breadcrumb.

---
### TEST 2 — Slice-4 EU byte-equality (Chad)
- **Branch / ISO:** `codex/gpu-p1-slice4-gen9-eu-silicon-gate-chad` (rebuild + stage if `/tmp` cleared).
- **Proves:** Gen9 **EU compute == CPU floor, byte-for-byte** — the glass GPU-accel **silicon** proof QEMU's accel-sim cannot give (the accel-sim shares the CPU Q16 math, so its parity is scalar==scalar; only real EU execution proves the accel path).
- **Discriminator (the load-bearing one, proven non-vacuous):** **`BLUR_WALKER_KICKS`** (EU walker dispatched) vs **`BLUR_COMPLETIONS`** (EU completed) — both must be **>0** AND the output **byte-equal** to the CPU floor. Both-zero or floor-only = the EU didn't execute (vacuous).
- **Failure-mode + discriminator bank:**
  1. **EU never executes / falls to floor** — `BLUR_WALKER_KICKS=0` or `BLUR_COMPLETIONS=0`. Discriminator: both counters logged; both must be >0.
  2. **Byte mismatch (EU ≠ floor)** — the byte-oracle assert fails → a real EU compute discrepancy. Capture the first mismatching offset + bytes.
  3. **GGTT/PPGTT (A64 address-space) fault** — `RING_FAULT` + a fault *address*. Capture-before-you-need-it: the ISO must log the faulting address, not just the fault type (the BUILD-4a lesson). Discriminator: GGTT vs PPGTT from the address range.
  4. **EU walker kicks but hangs** — `BLUR_WALKER_KICKS>0, BLUR_COMPLETIONS=0` → firmware/EU hang, not a dispatch failure.

---
### TEST 3 — WiFi scan-and-join LINK-UP (Jarvis)
- **Branch / ISO:** `codex/mt7601u-scan-path-caldefer-jarvis` (staged `~ecab1793…` at `/tmp/`).
- **Proves:** mt7601u **LINK-UP via scan-and-join** on real silicon — SSID + password only, **no baked channel** (the "ship correct, never a crutch" decision). Scan finds the AP's channel, calibrates, associates, DHCP.
- **⚠ SECURITY (hard rule):** the WiFi **SSID + password are filled in LOCALLY by the PL and NEVER committed** — the ISO is built with real creds locally, run, and the creds reverted. No credential ever reaches `origin`. Confirm the branch has empty placeholders before/after.
- **Failure-mode + discriminator bank (NOTE-A armed):**
  1. **Scan finds no AP** — empty scan result → RF/antenna/firmware or AP out of range/band. Discriminator: the scan result list (which SSIDs/channels seen).
  2. **NOTE-A (the armed expected-failure): scan finds AP but uncalibrated assoc fails** — the CAL-BW-defer fix recalibrates post-channel-select; if recal didn't complete, assoc times out. Discriminator: `SCAN` shows the AP + `ASSOC` fails → the **CAL-retry fallback** path (NOT a baked-channel fallback — never that). This is the most likely non-trivial failure; the run must localize it.
  3. **MCU wedge on channel-switch** — the `MCU_CAL_BW`-in-`set_bw_filter` wedge the fix addressed; firmware unresponsive after channel-switch. Discriminator: MCU state / firmware heartbeat.
  4. **Assoc succeeds, no IP** — DHCP `chaddr` empty (the documented prior-art trap). Discriminator: LINK-UP but no DHCP lease → check `chaddr` non-empty.

---
## Deliverable (each coder)
Paste the **serial log** to the PM (the PM does the byte-level read: PASS/FAIL + which discriminator fired → next step). A run is only done when it's **conclusive pass OR fail** — a failure you can't localize from the captured serial/sink is a dispatch defect, not bad luck. No code changes during the session (coding is frozen); a *fix* that a failure reveals is a **post-session coding task**, dispatched after the PL lifts the freeze.
