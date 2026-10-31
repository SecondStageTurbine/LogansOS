<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 7.5 BUILD-W2 — MLME + 4-way handshake + CCMP + NetDevice (real association) → Cerebro

**Date:** 2026-06-16
**From:** PM (Claude)
**To:** Cerebro (he built W1; PL confirms). Branch `codex/phase-7.5-build-W2-mlme-ccmp-cerebro` **off `origin/main` HEAD once W1 has merged** (W2 builds directly on W1's driver + W0/W0b's supplicant crypto).
**Status:** DISPATCHED (pending W1 merge). **Host-tier KATs + a Stage-B OptiPlex bare-metal run.** This is the build that makes the WiFi *connect*: join **W0/W0b's supplicant crypto** (PTK/GTK/PMKID/EAPOL-MIC/CCMP keys, SAE) to **W1's MT7601U frames** through an 802.11 **MLME** state machine, ending in a **CCMP-encrypted associated link a DHCP lease rides over**.

## 0. The decision this build makes
Take W1 from *scan-only* to *associated + encrypted data*: MLME (auth → assoc → 4-way handshake) drives W1's TX/RX, the handshake reuses W0's KDF/MIC and W0b's SAE, CCMP (W0's AES-CCM) protects the data path, and the result is surfaced as a **NetDevice** so the existing IPv4/DHCP stack (Phase-4) gets a lease over WiFi. **Out of scope:** 802.11n aggregation/block-ack throughput tuning, roaming, multi-BSS, PMF/802.11w beyond what association needs (note as W3+).

## 0.1 F1-part-2 — front-loaded robustness (the W1 follow-on Cerebro flagged)
Before association: fix the **post-ch6 channel-switch wedge** (ch7 MCU-response timeout, ch8–14 bulk-out timeout) so a full 14-channel scan completes. The mined pattern: **persistent MCU-response buffer + per-channel/endpoint reset**; the **cheap first sub-item is a scoped bulk-poll budget over the scan loop** (do that first, measure, then add the reset only if still needed). Gate: **full ch1–14 scan on silicon** (extends W1's ch1–6). This lands at the *front* of W2, not a standalone build.

## 1. Authoritative references (on `main` — read first)
```
git fetch origin
git show origin/main:Plans/RESEARCH-2026-06-15-w2-mlme-supplicant-prep-FINDINGS.md   # the mined MLME/handshake/CCMP plan + traps
git show origin/main:Plans/THINK-2026-06-13-phase-7.5-wifi-arc-mt7601u-supplicant.md  # the W0→W1→W2 arc
git show origin/main:Plans/DISPATCH-2026-06-13-phase-7.5-build-W0-wpa-crypto-jarvis.md   # W0 PTK/GTK/EAPOL-MIC/CCMP keys
git show origin/main:Plans/DISPATCH-2026-06-13-phase-7.5-build-W0b-sae-jarvis.md         # W0b SAE (WPA3)
```
W0/W0b already give the crypto (RustCrypto-based, KAT'd); W1 gives the frames. W2 is the **MLME glue + CCMP-on-the-wire + the net-device seam** — reuse, don't reimplement, the crypto.

## 2. ISCs
- **ISC-0 — F1-part-2 full-channel scan** (see §0.1). Silicon gate: ch1–14 scan, no wedge.
- **ISC-1 — MLME state machine.** Build/parse 802.11 **management frames** (probe, **auth (Open System)**, **assoc-req/resp**) and the state machine scan→auth→assoc→4-way→connected, with timeouts/retries. **Byte-oracle (I-12):** hand-laid mgmt-frame fixtures (the 802.11 MAC header + fixed/tagged params) asserted field-exact, independent of the builder/parser.
- **ISC-2 — 4-way handshake (WPA2) + SAE bridge (WPA3).** Wire **W0's** PTK/GTK derivation + EAPOL-Key frame build/parse + **MIC** over **W1's** TX/RX; for WPA3, run **W0b's SAE** to the PMK then the same 4-way. **KAT:** the published 4-way EAPOL-Key vectors W0 already uses (raw-source, e.g. IWD `unit/test-*.c` / wpa_supplicant test vectors — **never WebFetch the exact bytes**).
- **ISC-3 — CCMP data-frame protect/unprotect.** Encrypt/decrypt 802.11 data frames with **W0's AES-CCM** (CCMP header: PN/KeyID; replay counter). **Byte-oracle:** the **IEEE 802.11 CCMP test vectors** from raw source (IWD `unit/test-*.c` or the 802.11 standard's worked example) — hand-verified, not round-trip. Replay-window enforced.
- **ISC-4 — NetDevice seam.** Present the associated, CCMP-protected link as a `NetDevice` (the Phase-4 generic link-device substrate) so `net::` sees it; **gate: a real DHCPv4 lease obtained over WiFi** on silicon.
- **ISC-5 — Standards + kill-switch.** Default-off `wifi` feature; **disabled ≡ pre-W2 (W1 scan-only), bit-for-bit** (the kill-switch ISC — required). Host-testable MLME/CCMP logic (`*_logic` + `ai-ops-tests`-style host suite); `fmt`/`clippy -D warnings`; **no new `unsafe`**. Open the **next free ADR** (≥057 — confirm against `docs/adr/`) + ANSPEC §; reconcile with ADR-054 (W1).

## 3. Governance fault-containment analysis (I-14 — REQUIRED for the connectivity lane)
W2 adds the **most** driver activity yet (sustained TX/RX, MCU traffic, CCMP on the data path), so re-assert all three vectors:
- **(a) Lock isolation:** the MLME/IRQ/RX path acquires **no** AI-governance lock (`INFLIGHT`/`RECENCY`/`ARENA`/reservation/`TABLE`). grep-checkable + runtime assertion. Even a handshake-storm or AP flood cannot stall governance.
- **(b) Liveness degradation:** if the RX/MLME path monopolizes a CPU, the residency daemon degrades to P4b's synchronous direct-reclaim backstop — governance slows, never fails.
- **(c) Memory isolation — the load-bearing one:** all W1/W2 DMA stays inside the **fenced descriptor-validated DMA arena** behind VT-d (W1 proved `i14=usb_contained`, `fence_refusals=0`). W2's added RX rings + CCMP buffers MUST stay in-arena; **assert `fence_refusals=0` and zero unfenced DMA on the Stage-B run** (W1's discriminator, carried forward). No governance/tensor memory reachable by the NIC.

## 4. Expected-failure-mode + discriminator section (REQUIRED — Stage-B is blind bare-metal)
Design the OptiPlex run to be **conclusive on pass OR fail**. Pre-seed the bank with each mode + its discriminator (breadcrumb/MMIO/serial-sink, the W1 pattern):
1. **Assoc never completes** → log the MLME state + last mgmt-frame status code (auth vs assoc-resp reason). Discriminator: which state stalled.
2. **4-way handshake MIC mismatch** → dump the derived PTK KCK/KEK + the computed-vs-received MIC + the ANonce/SNonce. Discriminator: KDF input mismatch vs MIC-algo (HMAC-SHA1 vs AES-CMAC for the AKM) vs frame-byte-order.
3. **CCMP decrypt/replay fail** → log PN, KeyID, computed-vs-frame MIC, replay-window state. Discriminator: key install vs PN/AAD construction vs replay-counter.
4. **DHCP never leases over WiFi** → is the link associated+encrypted but the NetDevice seam dropping frames? Log TX/RX counts at the seam. Discriminator: association-layer vs net-stack-seam.
5. **F1-part-2 still wedges past ch6** → the bulk-poll-budget breadcrumb + MCU-response-buffer state. Discriminator: budget-too-tight vs needs-the-reset.
Capture-before-you-need-it: add any counter (PN, MIC bytes, seam TX/RX, MLME-state) the discriminator needs **in this build**, not on the failing run.

## 5. Prior-art trap-mining pass (REQUIRED — mine before you boot)
From the W2-prep FINDINGS + Linux `mt7601u`/`mac80211`, IWD, wpa_supplicant, 802.11 spec — seed the bank with the **documented** gotchas, e.g.: the **AKM-specific MIC algorithm** (HMAC-SHA1 vs AES-CMAC) selection; **EAPOL-Key descriptor/Key-Info bit** layout; **CCMP AAD construction** (which header bits are masked/muted — the classic off-by-mask bug); **PN/IV byte order**; **GTK install timing** (msg 3/4) and the **replay-counter** reuse trap; the **MT7601U WCID/key-table** install path for hardware crypto vs software CCMP (decide which W2 uses and say so); the `chaddr`-non-empty DHCP requirement (already bit us in BUILD-B). Each trap goes in the bank with its discriminator **before** the first Stage-B boot.

## 6. Byte-oracle acquisition (the raw-source rule)
802.11 frame layouts + EAPOL-Key + **CCMP KATs** come from **raw source** — `git clone`/`curl` the upstream (IWD `unit/test-*.c`, wpa_supplicant, the 802.11 standard's worked examples) and read literal bytes. **Never** pull exact crypto/frame bytes through WebFetch/any summarizing read (the BUILD-W0 WPA-vector mangling is exactly why). The KAT's acquisition is part of the KAT.

## 7. Gate
- **Host:** MLME mgmt-frame byte-oracles; the 4-way EAPOL KAT (reusing W0's vectors); the CCMP KAT (raw-source 802.11 vectors, field-exact, not round-trip); kill-switch regression (disabled ≡ W1); `fmt`/`clippy -D warnings`; builds on+off `wifi`. Existing QEMU suite stays green (WiFi scenarios are baremetal-gated — do NOT add a QEMU shard that needs the dongle).
- **Stage-B (OptiPlex, PL-coordinated):** full ch1–14 scan; **associate to a real WPA2 AP** (4-way handshake completes, CCMP up); **obtain a DHCPv4 lease over WiFi**; `fence_refusals=0`, zero unfenced DMA (I-14(c)); the gate frame emitted to serial + the NVMe sink (W1 pattern).

## 8. Scope guard
MLME + handshake-wiring + CCMP-on-wire + NetDevice seam + F1-part-2 only. **Do NOT** reimplement W0/W0b crypto (reuse), build 802.11n aggregation/roaming/PMF (W3+), add a dongle-dependent QEMU scenario, or touch the AI hot-path/governance locks. Raw-source all exact bytes. No new `unsafe`; kill-switch required.

## 9. Merge (PM)
Close-gate: rebase onto current `main`, re-run the host KATs + kill-switch regression, **Stage-B silicon run green** (assoc + DHCP-over-WiFi + I-14 clean), push (CI host gate). PM `pm-merge-check` → `--ff-only` → ledger (Article 13 + ANSPEC § + the new ADR; reconcile ADR-054). This **closes the Phase-7.5 WiFi arc** (W0 crypto → W0b SAE → W1 scan → W2 associate) — the OS can join a real network.

**Disk reclaim (after PM confirms merge):** once this branch is on `origin/main` and the PM confirms it, delete this build's working tree to reclaim its `target/` — `git worktree remove <path>` (then `git worktree prune` if needed) if it's a worktree, or delete the folder if a standalone clone. **Never delete a tree whose branch is unpushed/unmerged** — verify the work is on `origin/main` first. (`cargo clean` is the interim if you want to keep the checkout — `target/` is ~80% of the folder.) See CLAUDE.md "Disk hygiene — reclaim the build tree after merge."

---

## Relay — what to tell Cerebro
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-16-phase-7.5-build-W2-mlme-ccmp-cerebro.md`
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-16-phase-7.5-build-W2-mlme-ccmp-cerebro.md
  git show origin/main:Plans/RESEARCH-2026-06-15-w2-mlme-supplicant-prep-FINDINGS.md
  ```
- **Branch:** `codex/phase-7.5-build-W2-mlme-ccmp-cerebro` off `origin/main` **after W1 merges** (W2 builds on W1 + W0/W0b).
- **One-line:** Make the WiFi *connect*. **Front-load F1-part-2** (full ch1–14 scan: bulk-poll budget first, MCU-response-buffer+reset if needed). Then **MLME** (auth→assoc→4-way) driving W1's frames, **reusing W0's PTK/GTK/MIC + W0b's SAE** for the handshake and **W0's AES-CCM for CCMP** on the data path, surfaced as a **NetDevice** so DHCPv4 leases over WiFi. Gate = host byte-oracle KATs (mgmt frames, EAPOL, **CCMP from raw-source 802.11 vectors — never WebFetch**) + kill-switch (disabled≡W1) + **Stage-B on the OptiPlex**: associate to a real WPA2 AP, DHCP lease, `fence_refusals=0` (I-14 clean). Carries the I-14 fault-containment, blind-hardware discriminator bank, and prior-art trap-mining sections (read §3–6). No new `unsafe`; WiFi scenarios stay baremetal-gated (no dongle-dependent QEMU shard). Closes the Phase-7.5 WiFi arc.
