<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# HANDOFF: WiFi mt7601u lane — Jarvis → Oracle

**Date:** 2026-07-27
**From:** PM (Claude)
**To:** Oracle (taking over the WiFi lane from Jarvis, per PL)
**Branch:** `codex/mt7601u-post-qselect-terminal-jarvis` @ `f5403d82` (parent `6cb0b6b2`) — **4-shard gate GREEN, PM-verified** (run 30318938406, exact SHA; push-CI 30318819852). Worktree clean, creds blank (`SSID=b""`, `PASSPHRASE=b""`, `CHANNEL=0`).

## State — what's DONE and CONFIRMED
- **CAL_BW-per-hop is upstream-faithful (Jarvis verified against raw torvalds/linux v6.6 `phy.c`).** `__mt7601u_phy_set_channel()` issues the 2 `MCU_CAL_BW` `wait_resp` commands at `phy.c:424`, *before* the `MT7601U_STATE_SCANNING` check at `456-457` — so upstream DOES run CAL_BW on scan hops. The prior "survey skips CAL_BW" (first Prometheus dig + the 2026-07-24 FIX-FORWARD) was **wrong**; Prometheus's second dig was right. Fix implemented: survey now runs inline CAL_BW per hop, CHANNEL=0 keeps genuine 14-channel survey.
- **Independent robustness fixes (stand regardless of root cause):** endpoint probe alone no longer counts as command-path-proven — **no identical RF17-20 retry unless `command_path_proven_usable=true`**; retry/failure **terminalizes to bounded `CHANNEL_COMMAND_PATH_UNUSABLE`, never hangs** (fixes the `cea89e6a` hang); cold FW reload stays rejected; between-hop response-slot drain logging retained.

## State — what is NOT yet proven (read this before the boot)
**The FIFO-saturation *mechanism* Prometheus proposed is NOT supported by our serial.** Jarvis's falsification: the 12 pre-ch7 commands *completed* (`resp_completions_delta=12`, `resp_timeout_windows_delta=0`) — nothing accumulated unprocessed. So we know we *diverged* from upstream (skipped CAL_BW) and matching upstream is correct, but **whether CAL_BW-per-hop actually clears the ch7 wedge is UNPROVEN.** The bare-metal LINK-UP is the real test, and **it may still wedge at ch7** — if so, the mechanism was wrong and the root cause is still open (the terminalization fix means it will now FAIL cleanly, not hang).

## Next steps (Oracle owns)
1. **Cut a CREDENTIALED ISO** — Jarvis built only an *uncredentialed* ISO from this tree (size 126687232, SHA256 `870A50EF…`); **no credentialed ISO exists yet.** Fill SSID+passphrase local-only per `Plans/RUNSHEET-2026-07-26-wifi-f4a0940-silicon-linkup.md` §2 (only `mod.rs`, keep `CHANNEL=0`, never commit/stash), build `mt7601u_assoc_baremetal`, confirm the manifest.
2. **Next JOINT OptiPlex LINK-UP boot** (PL at the box), per the run sheet. **The discriminator that makes this run conclusive:**
   - Survey clears **ch7 → ch14** (CAL_BW-per-hop worked) → continue to WPA/GTK/DORA/LINK-UP → **the CAL_BW mechanism is confirmed and this is the WiFi PASS.**
   - Survey still **wedges at ch7** (now a bounded FAIL, not a hang) → CAL_BW-per-hop was necessary-but-not-sufficient; the root cause is still open — capture the MCU/RF snapshot at the bounded terminal and re-diagnose (the latched-state / non-destructive-recovery angle, Prometheus Q-B, is the next lever).
3. **Security:** revert creds after (`git checkout -- kernel/src/drivers/usb/mt7601u/mod.rs`), grep serial+sink for the passphrase before sharing (must be absent). `fence_refusals=0` (I-14(c)) must hold.

## Merge
Not yet — the gate proves no regression, but WiFi merges only on a real **LINK-UP** (device-less QEMU can't exercise the MCU). At merge: PM verify + **I-14 containment AAG**. Continue on the same branch. Disk-reclaim after.

## Relay — Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/HANDOFF-2026-07-27-wifi-mt7601u-jarvis-to-oracle.md` + the run sheet + `Plans/DISPATCH-2026-07-27-mt7601u-ch7-latched-mcu-recovery-jarvis-prometheus.md` (Prometheus Q-B is your fallback if it still wedges). Branch `codex/mt7601u-post-qselect-terminal-jarvis` @ `f5403d82`, gate-green.
- **One-line:** You own WiFi now. Jarvis's CAL_BW-per-hop fix is upstream-verified and gate-green, BUT the causal mechanism is unproven (our serial shows the 12 pre-ch7 commands completed, no FIFO backlog) — so the next JOINT LINK-UP is the real test and **it may still wedge at ch7** (now a clean bounded FAIL, not a hang). Cut a credentialed ISO (none exists yet), run the LINK-UP: ch7→ch14 clear = WiFi PASS; still wedge = root cause open, capture the MCU/RF snapshot and pivot to the non-destructive-recovery angle (Prometheus Q-B). Creds local-only, `fence_refusals=0`, security grep before sharing.
- **RAG:** health-check first; e.g. `mt7601u set_bw_filter CAL_BW survey per hop`, `mt7601u command_path_proven_usable retry`, `mt7601u channel survey ch7 ch14 wedge terminal`. If down, note + grep/Read.
