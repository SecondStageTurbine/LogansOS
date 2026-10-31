<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# BARE-METAL SESSION PLAN — Arthur-perf → WiFi → (Slice-4 EU, blocked)

**Date:** 2026-07-26
**From:** PM (Claude)
**Authoritative runbooks:** `docs/runbook-baremetal-arthur-optiplex.md` (Arthur archetype), `docs/runbook-wlan0-dora-optiplex.md` (WiFi), `docs/runbook-ci-runner-capacity.md` (GPU-target facts).
**Readiness at a glance:**

| # | Test | Target | Ready? | Notes |
|---|---|---|---|---|
| 1 | **Arthur-perf tok/s** | OptiPlex **CPU** | ✅ **READY** (rebuild from `main`) | perf coverage `32c8ca7` + serial throttle `7237491` both on `main`; pure LLM inference, no GPU |
| 2 | **WiFi `f4a0940` LINK-UP** | OptiPlex + MT7601U | ✅ **READY** (gate GREEN, PM-verified) | run sheet `Plans/RUNSHEET-2026-07-26-wifi-f4a0940-silicon-linkup.md` |
| 3 | **Slice-4 EU byte-equality** | **M710q (Gen9)** | ⛔ **BLOCKED** | two blockers below — do NOT run on the OptiPlex |

**Run order (the runbook's two-boot pattern):** Arthur first (validates the boot/serial/capture pipeline + banks the tok/s number), then WiFi. Each is one `efibootmgr -n 0001 && reboot`; JOINT (PL at box for boot-select + serial + creds). **One driver — Jarvis — runs both builds + both boots** (Oracle is NOT needed at the box this session).

---
## TEST 1 — Arthur-perf tok/s (READY, OptiPlex CPU)
**What's new since the old staged ISO:** the perf bracket now times the **inventory `sys_generate`** path (not just the graph path) and the coverage gate is **non-vacuous** (`inventory_forward_perf`), so a fresh boot reports the *real* generation timing labeled **`real_silicon`** — the old ISO carried `passes=0` to silicon. **Rebuild required** (don't reuse the stale ISO).

**Driver: Jarvis** (single SSH-host driver for BOTH boots this session — Arthur-perf + WiFi — so the whole session is one seat + the PL).
**Build (Jarvis, SSH host), from current `origin/main`:**
```
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --build-only arthur_real_brain_boot --bare-metal-mode --include-ignored
Get-Content target\neo-test\vmware\arthur_real_brain_boot\manifest.txt   # confirm log-sink disk + the AHCI model-volume disk
```
**Stage + boot:** per `runbook-baremetal-arthur-optiplex.md` — the Arthur model volume already lives on `/dev/sda` (persisted, NOT a dd target); `dd` only the fresh **ISO → `nvme0n1`** boot region + clear/use the **nvme sink** (offset 256 MiB). Boot, **let it run to completion (~3-4 min for 0.5B; do NOT halt — it's linear scalar generation, not a hang)**: `INFER: pos=…` → `sample step=0..N` → `REPLY: <text>`.
**Deliverable:** the composited real answer on the framebuffer (PL's eyes-on) + the **`real_silicon` tok/s** number on serial. Discriminators: `sample step` advancing = generating (not hung); blank framebuffer + `REPLY:` on serial = framebuffer issue not compute.

---
## TEST 2 — WiFi `f4a0940` LINK-UP (READY, OptiPlex)
Full checklist in **`Plans/RUNSHEET-2026-07-26-wifi-f4a0940-silicon-linkup.md`** — gate GREEN (PM-verified all 4 shards). You supply a real 2.4 GHz WPA2-PSK AP + byte-exact SSID/passphrase (filled local-only, reverted after). PASS chain: `scan → deferred CAL_BW → WPA/GTK → DHCP DORA → nonzero IPv4 → LINK UP → fence_refusals=0`. NOTE-A armed. **Credential grep before sharing evidence.**

---
## TEST 3 — Slice-4 EU byte-equality (BLOCKED — do NOT run on the OptiPlex)
The glass GPU-accel **silicon** proof (Gen9 EU compute == CPU floor byte-for-byte; `BLUR_WALKER_KICKS`/`BLUR_COMPLETIONS` discriminator). **Two real blockers:**

1. **Hardware — the safe target is the M710q, and it's PSU-blocked.** Per `runbook-ci-runner-capacity.md`, the **M710q (Intel HD 530, Gen9)** is the designated bare-metal Gen9 GPU target *specifically to protect the OptiPlex from blind-EU-bring-up bricking its iGPU* — and it's **"waiting on PSU (ThinkCentre-Tiny 65W slim-tip)."** Running a from-scratch EU bring-up on the OptiPlex's UHD 630 risks bricking the box we need for Arthur + WiFi. **→ PL: is the M710q powered now?** If not, Slice-4 EU should wait; it is not worth risking the OptiPlex.
2. **Software — the candidate is 4 days stale + never merged.** `codex/gpu-p1-slice4-gen9-eu-silicon-gate-chad` @ `493dbd5` is based on pre-C1.0/Slice-6/consolidation `main` (2026-07-22) and the `BLUR_WALKER_KICKS` discriminator is **not on `main`**. It needs a rebase-fresh + re-gate before any silicon build. **→ Dispatched to Chad** (rebase + re-gate) so the *software* half is ready by the time the M710q is up.

**So:** Slice-4 EU is a **next session** once (a) the M710q has a PSU and (b) Chad's refreshed branch is gated green. Prepping the software half now (Chad) so hardware is the only remaining gate.
