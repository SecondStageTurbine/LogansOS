<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW DISPATCH: MT7601U TEST-3 repair candidate `00b45d4` AAG

**Date:** 2026-07-24
**From:** PM
**Candidate:** `00b45d418428680b0ede8d727664df27cda8c6ab`
**Branch:** `codex/mt7601u-aag-bound-dhcp-jarvis`
**Frozen base:** `33030355499fb7d270e42dd2955e12c1d1012437`
**Verified push CI:** `https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/30120749427`
**Verdict:** **REVIEW NOW. No edits, workflow dispatch, ISO credential build, hardware boot, merge, or PR.**

## Candidate claims under review

Jarvis reports:

- the channel-programming response TD is persistently pre-armed and retains exact completion ownership;
- one bounded, non-destructive endpoint probe and response-bearing retry may recover a transient transport stall;
- failure of that retry permanently classifies the channel command path unusable;
- no forced firmware reload, forced `COM_REG0=0`, continued survey, association, DHCP, LINK UP, or PASS follows that terminal;
- the evidence sink is installed and self-tested after block initialization but before network or USB initialization;
- terminal evidence includes response ownership, operation-budget, FCE-upload, DMA-address, endpoint-probe, and fence state;
- terminal PASS requires association, GTK, DHCP Bound, nonzero assigned IPv4, and LINK UP;
- the approved Cerebro oracle is preserved as blob `b47fc6f5f27cb7bb991cfa1a21d37c46a8922645`;
- committed credentials are blank and `CHANNEL=0`;
- normalized default, `usb-host + !mt7601u`, and `usb-hid + !mt7601u` artifacts remain byte-identical to baseline.

These are untrusted candidate claims. Review the exact remote SHA independently.

## Common review contract

```powershell
git fetch origin
git show --stat 00b45d418428680b0ede8d727664df27cda8c6ab
git diff 33030355499fb7d270e42dd2955e12c1d1012437..00b45d418428680b0ede8d727664df27cda8c6ab
```

Use an isolated detached worktree or immutable commit reads. Do not review Jarvis's mutable working state. Do not modify production or test code. Device-free builds/tests and read-only source inspection are authorized. Return exact commands and evidence.

Every lane must check that no claim of dead MCU, dead silicon, or a proven host-desynchronization root cause replaced the evidence-neutral terminal. The proximate cause remains open.

## Lane A — Oracle: driver and transport correctness

Review:

- MCU response-TD ownership before OUT, across completion, mismatch, timeout windows, endpoint recovery, retry, and the next survey hop;
- xHCI completion ownership and whether abandoned-lap events can be consumed as current responses;
- RF17-20 frequency-plan retry ordering and whether the retry is genuinely bounded;
- the one-probe rule and whether reported endpoint recovery is correctly treated as diagnostic rather than liveness proof;
- absence of forced reload, forced residency clearing, and post-terminal command traffic;
- FCE descriptor-index evidence and the distinction between channel commands and cold firmware upload;
- upstream consistency without importing an upstream-specific URB/ZLP mechanism;
- the corrected 40-byte and 14,344-byte framing facts.

Return `TOKEN-OK` only if the production path is bounded, evidence-neutral, and response ownership is structurally sound. Otherwise return `BOUNCE` with exact file/line findings.

## Lane B — Cerebro: contract binding, sink, and terminal truth

Review:

- the exact hostile-oracle blob and its binding to production decision seams;
- whether the accepted model rejects destructive reload in every accepted script;
- whether the early sink is installed and self-tested before any network/USB path can reach the MT7601U;
- whether sink failure halts before device activity;
- whether the channel terminal reaches serial and sink with one truthful category and complete budget/fence data;
- whether association, GTK, DHCP state, assigned IP, LINK UP, and PASS eligibility are production-derived and fail closed;
- no-offer, offer-without-ACK, invalid/zero lease, missing GTK, association failure, and NetDevice failure terminals;
- whether any human-only log interpretation is still required to prevent a false PASS.

Return `TOKEN-OK` only if the hostile oracle is production-bound and both evidence and LINK-UP truth are structural. Otherwise return `BOUNCE`.

## Lane C — Prometheus: kill switch, accounting, and reproducibility

Review:

- every operation-ledger type, static, branch, counter, and call site in xHCI is structurally absent under `!mt7601u`;
- `operation_budget_logic` remains host-testable without entering the OFF kernel;
- nested operation guards restore prior wait and operation state on every return;
- charging cannot replenish or exceed the operation-wide ceiling;
- no new IRQ-side lock, allocation, DMA surface, or governance-lock reachability;
- the added close-gate compile KAT actually builds `usb-host + !mt7601u`;
- committed `SSID=b""`, `PASSPHRASE=b""`, and `CHANNEL=0`;
- the three normalized OFF comparisons against baseline `0be953de11695ced27ef86dfe9b4e06f2abfd18a`.

Independently reproduce or audit the exact build and normalization procedure. Return a table with baseline SHA-256, candidate SHA-256, byte size, and equality for default, `usb-host + !mt7601u`, and `usb-hid + !mt7601u`. A single unpaired hash is insufficient.

Return `TOKEN-OK` only if the structural OFF contract and reproducibility proof both hold. Otherwise return `BOUNCE`.

## PM hold

No four-shard runner token is granted by this dispatch. After all three lanes return, the PM will independently verify their exact evidence, the candidate SHA, remote parity, push CI, and current runner queue before deciding whether to grant an exact-SHA token.

Even a four-shard green will not authorize merge or hardware. Chad's FADT reset qualification and Janus review remain a separate prerequisite for the controlled cold-to-immediate-warm silicon sequence.
