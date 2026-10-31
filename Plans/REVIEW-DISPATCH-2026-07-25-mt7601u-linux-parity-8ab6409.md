<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW DISPATCH: MT7601U Linux-parity candidate `8ab6409f`

**Date:** 2026-07-25
**From:** PM
**Candidate:** `8ab6409fe711e531c744d0296f8855beab7b7b3a`
**Branch:** `codex/mt7601u-linux-parity-post-qselect-jarvis`
**Parent:** `22776a71678b8d390472d2e5923304a6c20e881d`
**Frozen base:** `5f00b691ea96c41ff059b1716db24b52c97f1d02`
**Push CI:** `30142938995` — exact SHA, attempt 1, green
**Status:** **REVIEW ONLY. No runner token, hardware boot, merge, reset
integration, PR, amendment, rebase, or further push.**

## PM identity verification

The PM independently verified:

- the remote branch resolves exactly to the candidate;
- the candidate is one child of the eight-commit replay parent;
- its merge-base with `origin/main` is the frozen base, with nine commits
  ahead and zero behind;
- the repair own-stat is exactly seven files, `+912/-93`;
- `git diff --check` is clean;
- the Jarvis worktree is clean at the exact candidate;
- the SSID and passphrase placeholders are `b""`, `CHANNEL=0`, and neither
  local credential string occurs in the committed tree;
- automatic push CI run `30142938995` is exact-SHA green on attempt 1:
  host tests/format/lint, SPDX, and monolithic identity passed; QEMU was
  skipped because no workflow dispatch was authorized.

This establishes review identity only. It is not a code-acceptance verdict.

## Claimed repair

Jarvis claims the candidate restores the known-good Linux initialization
order at the observed post-`Q_SELECT` failure boundary:

1. resident firmware returns after USB-DMA enable without cold FCE/PBF setup,
   vendor reset, reload, or `COM_REG0` mutation;
2. exactly one owned command-response TD is armed after `Q_SELECT`;
3. sixteen packet-RX buffers are posted before MAC initialization and reused
   by scan without double-posting;
4. MAC table writes use Linux chunking and final-chunk-only response policy;
5. MAC tables plus AUX write share one finite, non-replenishing operation
   budget;
6. failure emits one evidence-neutral `MAC_INIT_OPERATION_FAILED` terminal,
   flushes the sink, blocks later MCU traffic, and never reloads firmware.

The cold-upload path, genuine `CHANNEL=0` survey, NOTE-A CAL retry, DHCP/link
truth, ledgers, DMA fences, and structural kill switches are claimed
unchanged.

## Lane A — Oracle: primary-source parity

Review the exact frozen candidate against current mainline Linux:

- `drivers/net/wireless/mediatek/mt7601u/mcu.c`
- `drivers/net/wireless/mediatek/mt7601u/dma.c`
- `drivers/net/wireless/mediatek/mt7601u/init.c`

Independently verify:

- resident-firmware early return performs no cold-path device mutation;
- response-TD ownership and packet-RX posting occur in Linux order;
- the first MAC command and all chunk boundaries are byte/frame equivalent;
- response waiting occurs only on the final chunk;
- no invented reset, reload, or recovery behavior remains;
- cold upload remains intact.

Return `TOKEN-OK` or `BOUNCE` with file/line evidence. Analysis only; no
production changes or hardware.

## Lane B — Cerebro: contract and sink truth

Review the same exact candidate. Require:

- one finite, whole-MAC-init non-replenishing budget;
- production-bound hostile oracles for resident early return, ordering,
  final-chunk response, timeout, terminal traffic suppression, and flush;
- exactly one evidence-neutral terminal category on bulk or response timeout;
- zero post-terminal MCU traffic;
- serial/sink convergence through the final discriminator;
- no path that can silently hang after `Q_SELECT`;
- negative controls that fail when the rejected reset/order behavior is
  restored.

Run the pre-registered mutations against the exact candidate where practical.
Return `TOKEN-OK` or `BOUNCE`. Review only; no production changes or hardware.

## Lane C — Prometheus: replay, structural, and artifact evidence

Independently verify:

- all eight frozen-to-replay stable patch IDs;
- the repair own-stat and exact seven-path scope;
- credential placeholders blank and `CHANNEL=0`;
- MT7601U and xHCI repair structures are absent from representative
  `!mt7601u` artifacts;
- normalized OFF equality against
  `0be953de11695ced27ef86dfe9b4e06f2abfd18a` for default, `usb-host`, and
  `usb-hid`, using isolated targets, a fixed build identity, and an
  ELF-preserving normalizer;
- the claimed blank ISO identity if reproducible.

Return `TOKEN-OK` or `BOUNCE`, including paired sizes and SHA-256 values for
artifact claims. Evidence only; no candidate changes, workflow dispatch, or
hardware.

## Hold boundary

The candidate is frozen at `8ab6409fe711e531c744d0296f8855beab7b7b3a`.
Reviewers must not review a mutable branch tip or substitute the replay
checkpoint `22776a71...`.

Only after all required lanes return on this exact SHA will the PM decide
whether to grant a four-shard workflow token. A green four-shard run would
still not itself authorize a credential-filled build, ISO burn, or silicon
boot.
