<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: MT7601U warm post-firmware hang repair → Jarvis

**Date:** 2026-07-23  
**From:** PM  
**To:** Jarvis  
**Priority:** Release-blocking WiFi LINK-UP follow-up; build now, QEMU gate staggered behind C1.1 and the PM/PL runner token.  
**Base:** current `origin/main` (contains `33bade4765d8963f99abaa1834ee48164a633d69`)

## Read first

```powershell
git fetch origin
git show origin/main:Plans/DISPATCH-2026-07-23-mt7601u-warm-postfw-fix-jarvis.md
git show origin/main:Plans/DISPATCH-2026-07-23-baremetal-3-tests.md
git show origin/main:docs/runbook-wlan0-dora-optiplex.md
git show origin/main:Plans/PM-HANDOFF-2026-07-23.md
```

Use the semantic-search RAG at `http://127.0.0.1:8404` for unfamiliar paths; the PL owns startup. If unavailable, note it and fall back to `rg`/direct reads.

## Finding to fix

TEST 3 is RED, conclusively **before scan**. The dongle enumerated as `148f:7601`; warm-resident firmware resync reported:

```text
mt7601u: WARM-RESYNC before fce_preload_setup+vendor_reset (COM_REG0=1)
mt7601u: WARM-RESYNC after setup=OK
mt7601u: firmware already resident — FCE resync applied ready-poll=OK; upload skipped
mt7601u: firmware running
```

Serial then remained byte-for-byte stationary for more than 4½ minutes. There was no EEPROM/calibration, survey, SSID, association, post-association CAL, DHCP, PASS, or FAIL. The cleared NVMe sink stayed all-zero because USB bind wedged before sink installation.

This is **not** a scan failure, association failure, or NOTE-A. NOTE-A remains armed.

The first operation after `firmware running` is the post-firmware `WPDMA_GLO_CFG` idle poll. Each vendor register read can inherit xHCI's full wait-event budget and Linux-style retry loop, allowing a nominal bounded poll to expand into a multi-minute pre-sink stall. Confirm or refute that localization with new breadcrumbs; do not assume it.

Evidence:

- serial SHA-256 `35666214d98b88a261644cf7169cff6f5d4f22a1cc3fd3265253290ae61d8e12`
- zero sink SHA-256 `080acf35a507ac9849cfcba47dc2ad83e01b75663a516279c8b9d243b719643e`
- source lane tip used for the run: `33bade4765d8963f99abaa1834ee48164a633d69`
- credential hygiene was clean

## Driver trap-mining pass — mandatory before implementation

Trace and cite the upstream Linux MT7601U warm/resident-firmware sequence and our xHCI control-transfer completion/retry semantics. Record:

- what reset/resync is required before the first post-firmware register read;
- whether `0xffff_ffff` must fail fast rather than be treated as busy;
- the interaction among xHCI wait budget, `vend_retry`, and the outer register poll;
- whether a host endpoint/port reset or a genuine USB power-cycle is required for recovery.

Reference only; import no GPL code. Add/update the derivation citation and license sanity note if the build relies on a new source or method.

## Build scope

1. Instrument every boundary from `firmware running` through WPDMA idle, ASIC-ready, CSR/BBP reset, USB-DMA init, and MCU Q-select. Each operation needs a before/after or terminal-failure breadcrumb.
2. Bound the first post-firmware operation in **real elapsed behavior**, including nested xHCI waits and vendor retries. `0xffff_ffff` must classify device-unresponsive immediately. A dead warm device must reach a clean bind failure and terminal scenario FAIL, never stall indefinitely.
3. Retain post-firmware state and the last WPDMA value across pre-sink bind, then replay them after sink installation so sink-only evidence localizes the failure.
4. Repair the warm-resident path sufficiently that the required warm reboot proof reaches LINK-UP. If hardware requires a port reset/power-cycle that software cannot provide safely, return that as a separately evidenced hardware constraint for PL decision; do not silently redefine success as fail-fast.
5. Preserve the cold path and the scan-and-join `CHANNEL=0` behavior. No baked-channel fallback.

**Non-goals:** do not redesign scan/association unless the new run reaches and finds a separate failure there. Do not disarm NOTE-A or weaken LINK-UP.

## PM draft — evidence only, never a merge source

The PM crossed the seat boundary and preserved an untrusted diagnostic sketch at:

```text
wip/pm-draft-mt7601u-warm-postfw-bound
7b35d1d71bd88d99181eec5a5bd9e9891ffa5513
```

It feature-compiles and builds the bare-metal image, but it has no hardware proof and is **not a repair claim**. Inspect if useful; independently re-derive, rewrite/reject, and return your own branch/commit. Never merge or cherry-pick the PM draft as authority.

## Required invariants / ISC

- **Kill-switch:** `mt7601u` OFF is bit-for-bit the prior kernel.
- **I-14 liveness:** every nested wait/retry composition is bounded; show the worst-case iteration structure without human-time estimates.
- **I-14 memory/fault containment:** no new unfenced DMA, stale endpoint ownership, buffer lifetime extension, or device-controlled index.
- **Concurrency:** consult `docs/concurrency-pitfalls.md` for atomics/IRQ/lock changes; no lock may be held across a wait that can require the same completion path.
- **Cold-path preservation:** cold firmware upload and later proven calibration bounds remain unchanged unless evidence requires a scoped change.
- **Credential safety:** SSID/passphrase remain local-only and are absent from commits, logs, branch history, and RETURN.
- **On-wire/on-disk:** if no frame or sink format changes, state that and show the diff. If one changes, add an independent byte oracle.

## Blind-hardware discriminator bank for the rerun

The next joint session has two required boots, each with fresh serial and cleared sink:

1. **Genuinely cold dongle boot:** establish the functional floor. Must reach scan, find the configured AP without a baked channel, associate, complete post-association calibration, obtain DHCP, and emit LINK-UP/PASS. Preserve NOTE-A discriminators.
2. **Immediate warm reboot:** must also reach LINK-UP/PASS. If it fails, the terminal frame must identify exactly one of:
   - WPDMA device-unresponsive (`last=0xffff_ffff`);
   - WPDMA busy timeout (non-all-ones last value);
   - ASIC-ready failure;
   - CSR/BBP or USB-DMA failure;
   - MCU Q-select failure;
   - later scan/association/NOTE-A/DHCP discriminator.

No identical blind warm rerun. No PASS from fail-fast alone.

## Gates and deliverable

Before RETURN:

1. Branch from current `origin/main`; keep credentials absent.
2. Run driver logic host KATs and add a falsifiable all-ones/busy/done decision oracle where applicable.
3. Run `scripts/worker-close-gate.sh`.
4. Fire the 4-shard `workflow_dispatch` only when the PM/PL grants a runner slot. All shards must run on the exact head SHA.
5. Build `mt7601u_assoc_baremetal` with the verified 45,412-byte out-of-band firmware and local credentials; record source SHA, ISO SHA/size/manifest, and prove placeholders restored afterward.
6. Commit and push. **Green without a CI URL is not done.**

RETURN: branch, exact SHA, own-stat, trap-mining findings/citations, close-gate output, host KAT evidence, 4-shard CI URL, ISO manifest/hash, credential-hygiene proof, cold/warm discriminator table, and deletable worktree path.

