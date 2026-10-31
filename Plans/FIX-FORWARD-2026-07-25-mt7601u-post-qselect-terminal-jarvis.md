<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# FIX-FORWARD: bound the complete MT7601U post-Q_SELECT bring-up

**Date:** 2026-07-25
**From:** PM
**To:** Jarvis
**Rejected candidate:** `8ab6409fe711e531c744d0296f8855beab7b7b3a`
**Required parent:** `8ab6409fe711e531c744d0296f8855beab7b7b3a`
**Oracle Lane A:** `TOKEN-OK`
**Cerebro Lane B:** `BOUNCE`
**Verdict:** **No runner token and no hardware boot. Append one narrow
contract repair on a new branch from the exact rejected candidate.**

## Why the candidate is bounced

The Linux-parity repair is accepted within Oracle's static parity scope:

- resident firmware now takes the mutation-free Linux early return;
- the command-response TD and sixteen packet-RX buffers are established in
  Linux order before MAC initialization;
- MAC command bytes, chunk boundaries, and final-chunk-only response policy
  match Linux;
- the cold upload remains intact;
- no invented reset or reload remains.

The candidate nevertheless fails the Lane B requirement that no path after
`Q_SELECT` can reproduce a silent hang with a stale sink.

At the exact candidate:

```text
bring_up:
  write_mac_initvals()?    covered by one finite operation budget and flush
  init_bbp()?              MCU RANDOM_WRITE, no operation-wide budget
  init_wcid_mem()?         three MCU BURST_WRITE commands, no operation-wide budget
  ...
  eeprom_init()?           no enclosing post-QSELECT terminal contract
```

The MAC-init guard ends before `init_bbp()` and `init_wcid_mem()`. There is no
sink flush between the successful MAC-init flush and `eeprom_init()`. A
non-responsive device during BBP or WCID initialization can therefore consume
long per-transfer poll limits without producing a terminal category, while
serial advances beyond the last durable sink record. That is the TEST 3A
failure signature shifted one stage later.

The existing MAC-init implementation itself passed review: one finite
non-replenishing budget, one evidence-neutral CAS-guarded terminal, zero
post-terminal MCU traffic, production-bound policy, and terminal flush.

## Jarvis repair

Create a new branch from exact SHA
`8ab6409fe711e531c744d0296f8855beab7b7b3a`. Preserve the rejected branch and
commit. Append exactly one narrow child commit; do not amend, rebase, or
force-push either history.

Independently own the implementation. Required behavior:

1. Every potentially blocking device operation from successful post-`Q_SELECT`
   readiness through successful `eeprom_init()` must be covered by a finite,
   non-replenishing operation contract. One encompassing budget or explicit
   stage budgets are acceptable only if there is no uncovered gap and no
   replenishment loophole.
2. BBP initialization, every WCID/key/attribute burst, intervening polls and
   register operations, and EEPROM acquisition must fail within that contract.
3. Any failure must emit exactly one evidence-neutral terminal discriminator
   identifying the failing stage and reason, including budget consumption,
   response ownership, packet-RX ownership, and fence state.
4. The terminal must latch the command/device path unusable, block and count
   all later MCU/device traffic, flush serial and the NVMe sink, and stop.
   Never reload firmware or claim MCU death.
5. Flush the sink at the durable boundaries before and after BBP, WCID, and
   EEPROM initialization so serial and sink converge even when a later stage
   fails.
6. Preserve the accepted Linux ordering, resident early return, MAC frame
   bytes and chunk policy, cold upload, `CHANNEL=0` survey, NOTE-A CAL retry,
   DHCP/link truth, ledgers, DMA fences, and structural kill switches.
7. Do not integrate the banked warm-reset actuator and do not add a baked
   channel.

Do not make the budget so small that healthy hardware can fail spuriously.
Return the accounting basis for the selected finite limits.

## Required hostile oracles

Use production-bound logic and observed operation traces. Cover at least:

- successful continuation from MAC init through BBP, WCID, and EEPROM;
- timeout/exhaustion during BBP readiness;
- failure in each BBP RANDOM_WRITE table;
- failure in each WCID/key/attribute BURST_WRITE;
- failure during EEPROM acquisition;
- exactly one terminal category for each injected failure;
- zero MCU/device operations after terminal;
- terminal sink flush and stage-boundary flushes;
- no budget reset or replenishment between nested calls;
- negative controls for removing the enclosing budget, removing a terminal
  flush, and bypassing the post-terminal gate.

Keep the focused mutation suite bounded enough to complete deterministically;
do not rely only on a global test timeout as the assertion.

## Re-review

- **Cerebro:** re-review the exact child for complete post-`Q_SELECT`
  budget/terminal/flush coverage and execute the focused negative controls.
- **Oracle:** review only the child delta for regression of the already
  accepted Linux parity seams.
- **Prometheus:** perform replay/scope/structural/OFF artifact review on the
  child, not on `8ab6409f...`.

Oracle's prior `TOKEN-OK` is retained as evidence for the parent but is not a
substitute for child-delta review. Prometheus evidence for the rejected parent
cannot authorize the child.

## RETURN and hold

Jarvis returns:

- branch, exact tip/parent/base, own-stat, touched paths, remote parity, and
  clean worktree;
- focused oracle and mutation commands with pass counts;
- all MT7601U host tests;
- blank SSID/passphrase with `CHANNEL=0`;
- blank association ISO identity;
- normalized OFF equality;
- one uninterrupted `scripts/worker-close-gate.sh` pass;
- exact-SHA automatic push-CI URL.

No four-shard workflow dispatch, credential-filled build, ISO burn, hardware
boot, reset action, merge, or PR is authorized.
