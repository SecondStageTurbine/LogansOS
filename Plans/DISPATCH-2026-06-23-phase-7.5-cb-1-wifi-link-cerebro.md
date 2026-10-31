<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase-7.5 CB-1 (P0) — cold end-to-end WiFi link: association + GTK/broadcast-RX + DHCP-over-`wlan0` → Cerebro

**Date:** 2026-06-23
**From:** PM (Claude)
**To:** Cerebro (owns the WiFi lane — W2 MLME/CCMP, the targeted-connect A.2 path). New branch off `origin/main` HEAD: `codex/phase-7.5-cb-1-wifi-link-cerebro`.
**Status:** DISPATCHED — **the P0 headline gate of the Phase-7.5 close: a real AP, cold, gives `wlan0` an IP.** Synthesized from `Plans/PLAN-2026-06-23-phase-7.5-closeout.md` (§CB-1) + the trap-mine FINDINGS (`38bf28c`). **Connectivity + bare-metal ⇒ this dispatch carries the mandatory I-14 governance fault-containment analysis, the blind-hardware failure-mode/discriminator section, the prior-art trap-mine pre-seed, and the kill-switch ISC.**

## 0. The decision this build makes
Turn "association is proven, but the link is unusable (no broadcast RX, no DHCP)" into "**a booted NeoPlutarch cold-uploads the mt7601u firmware, targeted-associates to the configured AP, completes the 4-way + installs the GTK, and `wlan0` gets a DHCP lease**" — first in a hermetic QEMU scenario, then the PL's self-diagnosing bare-metal DORA-over-`wlan0` run. **This is wedge-free by construction:** use the Linux-faithful targeted-connect path (single tune to the AP's configured channel), never the cumulative 1→14 sweep that wedges (that wedge is CB-2's problem, not CB-1's).

## 1. References
```
git fetch origin
git show origin/main:Plans/PLAN-2026-06-23-phase-7.5-closeout.md                       # §CB-1 — the authoritative spec; §2 R1/R2 rulings
git show origin/main:Plans/RESEARCH-2026-06-23-phase-7.5-closeout-wifi-trap-mine-FINDINGS.md   # the pre-seeded trap bank (T-028…T-048) — READ FIRST
git show origin/main:kernel/src/net/wpa/eapol.rs            # 4-way handshake; parse GTK KDE from Message 3 (T-038)
git show origin/main:kernel/src/net/wpa/keywrap.rs          # NIST AES key-unwrap for the GTK KDE
git show origin/main:kernel/src/net/wifi/mlme_logic.rs      # assoc state machine; GTK-before-DHCP ordering (T-043)
git show origin/main:kernel/src/drivers/usb/mt7601u/key_logic.rs   # WCID key/attr programming — install GTK to WCID 0 (T-036/T-041)
git show origin/main:kernel/src/drivers/usb/mt7601u/rx_logic.rs    # RX filter — accept broadcast/multicast (T-047); hw_key_idx path (T-046)
git show origin/main:kernel/src/net/device.rs              # NetDevice — 802.11→Ethernet addr translation (T-048)
git show origin/main:kernel/src/net/dhcpv4.rs             # Phase-4 DHCP state machine (reused) — BROADCAST flag (T-045)
```
**Trap-bank acquisition rule (CLAUDE.md):** the GTK byte layout (**T-038**) and the WPA3 GKH layout (T-039, CB-5's) are flagged **needs-raw-source-confirm** in the FINDINGS. Confirm T-038's GTK-KDE byte offsets against **raw wpa_supplicant source** (`curl`/`git` the literal file — NOT WebFetch/any summarizing read; a mangled offset poisons the oracle). The hand-laid GTK-KDE byte fixture is a required deliverable (see ISC-3).

## 2. ISCs (the build)
- **ISC-1 — GTK install + broadcast RX (the missing piece for a usable link).** Parse the GTK KDE from 4-way **Message 3** (T-038, byte layout raw-source-confirmed), AES-key-unwrap it, install it to **WCID 0** (the reserved multicast slot — T-036: `MT_WCID_KEY(0)=0x8000`, `MT_WCID_ATTR(0)=0xa800` with `PAIRWISE=0`, cipher `AES_CCMP=4`), explicitly program the WCID-attr off its all-1s init (**T-041**), and open the RX filter to broadcast/multicast (**T-047**).
- **ISC-2 — DHCP-over-`wlan0`.** Enforce **GTK-before-DHCP** ordering (**T-043** — the DHCP Discover broadcast can't pass until the port is authorized + GTK installed); set the DHCP **BROADCAST** flag (**T-045**); do the **802.11→Ethernet addr translation** in the NetDevice (**T-048**); route through the hardware-encryption WCID path (`hw_key_idx != 0xff`, **T-046**). Reuse the Phase-4 DHCP state machine unchanged — only the WiFi-link differences above are new.
- **ISC-3 — Independent GTK-KDE byte oracle.** A **hand-laid-out GTK-KDE byte fixture** (offsets confirmed from raw wpa_supplicant source) asserted against the parser, independent of writer/reader — round-trip alone is insufficient (CLAUDE.md byte-oracle rule).
- **ISC-4 — Hermetic QEMU scenario.** A QEMU scenario exercising **GTK-install → broadcast-RX → DHCP DORA** that runs hermetically (no live network, no host-variable RNG — `-cpu max` if any RNG is consumed, per the hermeticity invariant). At `NEO_SMP=4`.
- **ISC-5 — Serial breadcrumbs (capture-before-you-need-it, for bare metal).** One breadcrumb per link stage so a bare-metal freeze self-localizes: `fw_upload ok` → `targeted_tune ch=N` → `auth ok` → `assoc ok` → `4way m3 gtk_kde_len=…` → `gtk_install wcid=0 cipher=ccmp` → `rx_bcast_filter on` → `dhcp discover` → `dhcp offer` → `dhcp request` → `dhcp ack ip=…`.
- **ISC-6 — Kill-switch.** The GTK/broadcast-RX path ships behind a switch; **disabled ≡ the prior pairwise-only-link behavior, bit-for-bit** (a regression proving the off state equals the W2 association-only baseline). Complexity you can provably switch off is bounded (CLAUDE.md rule 1).
- **ISC-7 — standards.** SPDX on every changed file; `cargo fmt --all --check`; clippy `-D warnings` (kernel blocking) via Docker. No new cross-cutting invariant. No new `unsafe` without justification.

## 3. Governance fault-containment analysis (I-14 — mandatory for the connectivity lane)
The link path must not let a driver fault touch the AI-governance invariants (I-1…I-13):
- **(a) Lock isolation.** The GTK-install / RX-filter / DHCP path acquires **no** AI-governance lock (`INFLIGHT`/`RECENCY`/`ARENA`/reservation/`TABLE`). Grep-assert it; no new lock crosses into governance. *(Holds — unchanged from W2.)*
- **(b) Liveness degradation.** A broadcast-RX storm or DHCP flood cannot monopolize governance: the residency daemon degrades to the P4b synchronous direct-reclaim backstop (I-2 kill-switch) — governance slows, never fails.
- **(c) Memory isolation.** Every mt7601u DMA buffer on this path (the bulk RX ring for broadcast frames) routes through `dma_arena::fence(...)` under the existing VT-d/IOMMU containment (I-14 c1/c2) — unchanged from W1/W2; **no new unfenced device-initiated DMA**. State this explicitly for the broadcast-RX ring.

## 4. Blind-hardware failure-mode + discriminator (the bare-metal `wlan0`-DORA run — mandatory)
No debugger — ISC-5 breadcrumbs only. Pre-seeded from the trap bank; design conclusive on pass AND fail:
- **GTK installs but broadcast RX still silent** ⇒ breadcrumb reaches `gtk_install` then no `dhcp offer`. Discriminator: dump the WCID-0 attr register post-write (T-041 — confirm it left the all-1s init) and the RX-filter register (T-047).
- **DHCP Discover never leaves / never returns** ⇒ stops at `dhcp discover`. Discriminator: was GTK installed *before* the Discover (T-043 ordering)? Is the BROADCAST flag set (T-045)? Is the 802.11→Ethernet addr translation correct (T-048)?
- **Offer returns but encrypted-RX garbage** ⇒ `dhcp offer` seen but ACK never completes ⇒ hw_key_idx / WCID cipher mismatch (T-046); dump the decrypt status.
- **GTK-KDE parse wrong (off-spec offset)** ⇒ `gtk_kde_len` implausible ⇒ the byte fixture (ISC-3) localizes parser-vs-spec; re-confirm against raw wpa_supplicant.
- **Wedge appears despite targeted path** ⇒ if the cumulative-sweep wedge shows up, that's CB-2's xHCI-endpoint-reset territory, not CB-1 — note and hand off; CB-1 stays on the single targeted tune.

## 4.5 Prior-art trap-mine (pre-seeded — read the FINDINGS first)
The bank is already mined: `Plans/RESEARCH-2026-06-23-phase-7.5-closeout-wifi-trap-mine-FINDINGS.md` (21 traps, T-028…T-048). CB-1's live traps: **T-036/T-041** (WCID-0 attr programming off all-1s init), **T-038** (GTK-KDE byte layout — raw-source-confirm), **T-043** (GTK-before-DHCP ordering), **T-045/T-046/T-047/T-048** (DHCP BROADCAST flag / hw_key_idx / RX bcast filter / addr translation). Each already carries its discriminator in §4.

## 5. Scope guard
The usable cold link **only** — association (proven, reuse) + GTK/broadcast-RX + DHCP-over-`wlan0`. **Do NOT**: build the 14-channel survey / xHCI endpoint-reset (that's **CB-2**, Oracle); touch WPA3/SAE on-air (that's **CB-5**); change the SAE/CCMP crypto or the Phase-4 DHCP state machine internals; add a new invariant. Stay on the **single targeted tune** — never the cumulative sweep.

## 6. Gate + close (PM)
- **Hermetic QEMU `wlan0` DORA scenario green** (ISC-4) at `NEO_SMP=4` — the integration proof. **Kernel-touching ⇒ 4-shard QEMU `workflow_dispatch` mandatory.**
- **Bare-metal cold-end-to-end run = the PL's** (cold-upload → targeted-associate → GTK → DHCP DORA over `wlan0`, sink-captured), made self-diagnosing by ISC-5 + the §4 discriminators. Iterative bring-up loop expected — design each iteration to localize.
- **Close-gate: `scripts/worker-close-gate.sh`** (rebase LAST; full host-check set; SPDX; self-stat). RETURN the green **push-CI URL + the 4-shard QEMU `workflow_dispatch` URL** + the QEMU DORA `ip=…` line. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger.
- **Disk reclaim (after PM confirms merge):** once `codex/phase-7.5-cb-1-wifi-link-cerebro` is on `origin/main`, delete the build tree to reclaim `target/`. Never delete an unpushed/unmerged tree. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Cerebro
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-23-phase-7.5-cb-1-wifi-link-cerebro.md` (+ the FINDINGS `RESEARCH-2026-06-23-...-wifi-trap-mine-FINDINGS.md` **first**, + `eapol.rs`/`keywrap.rs`/`mlme_logic.rs`/`key_logic.rs`/`rx_logic.rs`/`device.rs`/`dhcpv4.rs`). **Branch:** `codex/phase-7.5-cb-1-wifi-link-cerebro` off `origin/main` HEAD.
- **One-line:** **The P0 headline close gate — make `wlan0` get a real IP, cold.** Reuse the proven targeted-connect association (single tune, NEVER the 1→14 sweep), then add the two missing pieces: **(1) parse the GTK from 4-way Msg-3 (T-038, confirm byte layout against raw wpa_supplicant — no WebFetch) → install to WCID 0 (T-036/T-041) → open broadcast RX (T-047)**, and **(2) DHCP-over-`wlan0`: GTK-before-DHCP ordering (T-043), BROADCAST flag (T-045), 802.11→Ethernet addr xlate (T-048), hw-encryption WCID path (T-046)** — Phase-4 DHCP SM reused. **Ship a hand-laid GTK-KDE byte oracle** (ISC-3) + a **kill-switch** (disabled ≡ W2 pairwise-only link, bit-for-bit) + **serial breadcrumbs at each stage** (ISC-5) so the bare-metal DORA run self-localizes. **I-14:** broadcast-RX DMA stays fenced through `dma_arena::fence` under VT-d; no governance lock on the path. **Scope: usable cold link ONLY** — no survey/xHCI-reset (CB-2), no WPA3/SAE (CB-5), no crypto/DHCP-SM-internals changes, no new invariant. **Gate:** hermetic QEMU DORA scenario at `NEO_SMP=4` + the bare-metal run (PL's). **Close-gate `scripts/worker-close-gate.sh`; RETURN the green push-CI + 4-shard QEMU `workflow_dispatch` URLs + the QEMU `ip=…` line.**
