<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — MT7601U candidate `00b45d4` AAG, **Lane A (Oracle): driver & transport correctness**

**Review dispatch:** `Plans/REVIEW-DISPATCH-2026-07-24-mt7601u-candidate-00b45d4-aag.md`
**Candidate reviewed (exact remote SHA):** `00b45d418428680b0ede8d727664df27cda8c6ab`
**Branch under review:** `codex/mt7601u-aag-bound-dhcp-jarvis`
**Frozen base:** `33030355499fb7d270e42dd2955e12c1d1012437`
**PM-cited push CI (green):** `https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/30120749427`
**Author:** Oracle (Claude family) — **adversarial, default-suspicious. Read-only inspection; no production/test code edited.**

## Verdict

**`TOKEN-OK`** — the production channel-command path is **bounded**, **evidence-neutral**, and **response
ownership is structurally sound**. Two non-blocking RETURN notes (N1, N2) are recorded below; neither is a
BOUNCE finding.

## Method / commands

```
git fetch origin
git worktree add --detach E:/AI Automation/AltimitNeo-aag-wt 00b45d418428680b0ede8d727664df27cda8c6ab
git show --stat 00b45d418428680b0ede8d727664df27cda8c6ab
git diff 33030355499fb7d270e42dd2955e12c1d1012437..00b45d418428680b0ede8d727664df27cda8c6ab \
    -- kernel/src/drivers/usb/mt7601u/mcu_logic.rs kernel/src/drivers/usb/mt7601u/wedge_logic.rs \
       kernel/src/drivers/usb/mt7601u/phy.rs kernel/src/drivers/usb/xhci.rs
# + read-only inspection of mod.rs response/terminal/probe seams (immutable detached HEAD)
# + one independent completeness sweep for forbidden reload / COM_REG0=0 / post-terminal / dead-MCU / URB-ZLP patterns
git worktree remove E:/AI Automation/AltimitNeo-aag-wt
```

Reviewed the immutable SHA only (detached worktree, since removed), never Jarvis's mutable working state. No
`workflow_dispatch`, ISO build, hardware boot, merge, or PR. Verdict rests on read-only structural inspection
plus the PM-cited green push CI; the four-shard gate was **not** re-run (not required for the
transport-correctness verdict, and no token is authorized by the review dispatch).

## Findings against each Lane-A item (exact file:line on `00b45d4`)

### 1. MCU response-TD ownership — SOUND
- Response TD armed **before** the OUT (`mod.rs:1382-1386`), the load-bearing mainline ordering.
- `ensure_mcu_response_armed` is **idempotent** → exactly one outstanding TD (`mod.rs:1345-1346`) and
  re-validates the fence DMA address did not move (`mod.rs:1350-1352`).
- Wait loop: unowned events counted-not-consumed (`mod.rs:1425-1428`); RXFCE **copied before buffer reuse**
  (`mod.rs:1451-1459`); re-arm after consume (`mod.rs:1469`); `Invalid` → re-arm + `Err` (`mod.rs:1461-1464`).
- Covers the completion / mismatch / timeout / endpoint-recovery / retry / next-survey-hop windows; the outer
  loop is bounded to 5 (`mod.rs:1399`, terminal `Err` at `mod.rs:1482`).

### 2. xHCI completion ownership / abandoned-lap — CANNOT be consumed as a current VALID response
- Ownership is index-based: `mcu_logic::classify_response_transfer` returns `Unowned` when
  `event_index != pending_index` (`mcu_logic.rs`; wired `mod.rs:1418-1428`).
- The transport probe restarts both rings and nulls `cmd_resp_pending`, then **drains** abandoned events and
  counts them (`mod.rs:2187-2191`); ring restart re-laps the cycle so stale TRBs are cycle-mismatched.
- **Backstop:** the rotating command seq (1..=15, never 0 — `mod.rs:1371-1373`) means a stale-buffer read
  carries the *old* seq, so `resp_is_done(rxfce, new_seq)` fails → bounded mismatch → re-arm → timeout →
  terminal. Worst case degrades to a bounded terminal; **never** a false `CMD_DONE`/PASS.

### 3. RF17-20 frequency-plan retry ordering / genuinely bounded — BOUNDED
- Exactly **one probe + one retry**, structural (no loop): `wedge_logic::channel_failure_action` returns
  `ProbeEndpointsAndRetry` only when `endpoint_probes_used == 0` and the operation reserve remains; wired
  `mod.rs:2232-2261`. Double-bounded by the per-operation `OperationBudgetGuard(CHANNEL_OPERATION_BUDGET)`
  (`mod.rs:2226`).
- The freq-plan single chunk still requests `CMD_DONE`: `wait_response_for_chunk(0, 1) == true`
  (`mod.rs:1500-1503`) — the ch7 timeout point is preserved, not masked.

### 4. One-probe rule / endpoint recovery is diagnostic, not liveness — CORRECT
- Retry runs **regardless** of the probe's reported success; only the retried command's completion proves
  usability (`mod.rs:2243-2247`).
- The probe logs `command_path_proven_usable=0 destructive_reload=0 com_reg0_write=0` (`mod.rs:2199-2214`);
  `clear_endpoint_halt` doc now states success "does not establish MCU liveness" (`xhci.rs`, revised comment).

### 5. Absence of forced reload / forced residency clearing / post-terminal traffic — CONFIRMED (independent sweep)
- `bring_up` / `load_firmware` / `dma_fw` are reachable **only** from device bind (`bring_up` sole call site
  `mod.rs:3973`, before `scan()`); no timeout/recovery path reaches them.
- **Zero** writes to `MCU_COM_REG0` anywhere; it appears only as reads/polls (`mod.rs:1547,1690,1694,1850,1880`).
- Post-terminal MCU traffic is hard-gated by the `channel_command_path_unusable` latch (`mod.rs:1367-1369`,
  counter surfaced as `blocked_commands_after_terminal`), plus error propagation that exits the survey loop
  (`mod.rs:2495`), the targeted path (`mod.rs:2457`), and association (`mod.rs:2817`); bind then fails
  (`mod.rs:3979`).
- Fail-hard disposition is all-false: `attempt_destructive_reload / continue_survey / allow_association /
  allow_dhcp / allow_link_up` (`wedge_logic.rs:112-116`).

### 6. FCE descriptor-index evidence / channel-vs-cold-upload distinction — PRESENT
- `fce_upload_kicks` increments **only** inside `dma_fw` (`mod.rs:1592`); the channel terminal logs
  `fce_upload_kicks_before/after` (`mod.rs:2269-2270`, `2390-2391`), so `before == after` structurally proves
  no firmware upload rode the channel-command path.

### 7. Upstream consistency without importing a URB/ZLP mechanism — CONSISTENT
- `mcu_write_reg_pairs` now requests `CMD_DONE` only on the final chunk (mainline `cnt == n`), via
  `wait_response_for_chunk` (`mod.rs:1500-1504`, `mcu_logic.rs`) — an upstream-fidelity improvement over the
  prior wait-every-chunk behavior.
- **No** `URB_ZERO_PACKET` / ZLP / `transfer_flags` / `struct urb` anywhere; the only related token is the
  native xHCI Interrupt-on-Short-Packet (ISP) TRB control bit (`xhci.rs:1875`), not an imported Linux mechanism.

### 8. Corrected 40-byte / 14,344-byte framing — INTACT
- `build_mcu_frame` (`mcu_logic.rs`) and `build_chunk` (`fw_logic.rs`) are unchanged; the freq-plan
  `RANDOM_WRITE` remains the **first** MCU command in `set_channel` (`phy.rs`), which now also carries a
  `ChannelError{phase,reason}` provenance (`phase=RF_FREQPLAN_RF17_20_RANDOM_WRITE`) feeding the terminal.
  The 40-byte (freq-plan) and 14,344-byte (first fw chunk) facts hold.

### Cross-cutting check — evidence-neutral terminal (required of every lane)
- Terminal is `category=CHANNEL_COMMAND_PATH_UNUSABLE … root_cause=undistinguished` (`mod.rs:2360-2361`).
- **No** "dead MCU" / "dead silicon" / "proven host-desync" / "only replug" / "bricked" / "unrecoverable"
  assertion exists in the driver or logs (sweep confirmed). The proximate cause is left **open**, matching the
  Lane-2 analysis and the dispatch's standing requirement.

## Non-blocking RETURN notes (NOT BOUNCE findings)

- **N1 — seq-wrap aliasing (theoretical).** The stale-buffer defense (item 2) relies on the 4-bit command seq
  (1..=15) not colliding with a completion arriving 15+ commands late at the same ring slot. Not practically
  reachable in this single-outstanding-TD, ring-restart-flushed design, and it is **upstream-consistent**
  (mainline `mt7601u` also uses a 4-bit seq). Recorded for completeness, not as a defect.
- **N2 — residue trust.** `classify_response_transfer` accepts xHCI completion code 13 (Short Packet) with
  `actual_len >= 4` as a valid RXFCE read; correctness depends on the xHC's reported `transfer_residue`. The
  seq-match (N1 backstop) still bounds any mis-sized read to a mismatch/timeout, so worst case is a bounded
  terminal. Fine as-is; noted.

## Scope / boundaries

- Read-only inspection only; **no** production/test edits, PR, merge, or rewrite.
- This document is the Lane-A deliverable. Lane B (Cerebro: hostile-oracle binding, sink, LINK-UP truth) and
  Lane C (Prometheus: `!mt7601u` OFF-equality, accounting, reproducibility) return independently.
- No runner token is granted by this review; even a four-shard green is not merge/hardware authorization
  (Chad's FADT-reset qualification + Janus review remain separate prerequisites, per the review dispatch).
- **Cross-model note:** this is a Claude-family lane; the panel's independence comes from Cerebro (Claude) +
  Prometheus (Qwen) + the Codex-family author (Jarvis).
