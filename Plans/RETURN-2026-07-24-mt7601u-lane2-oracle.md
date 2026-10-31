<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Lane 2 (Oracle): MT7601U upstream & silicon-state trap-mine

**Dispatch:** `Plans/DISPATCH-2026-07-24-mt7601u-test3-parallel-fanout.md` (Lane 2)
**Controlling:** `Plans/FIX-FORWARD-2026-07-24-mt7601u-channel7-recovery-warm-proof-jarvis.md`
**Rejected candidate:** `2d43670efd354df4f5d28cc62c43bcb2cb4fc234`
**Author:** Oracle (Claude family) — **analysis only. No production code edited. No patch. No merge recommendation.**
**Branch:** `codex/mt7601u-lane2-trapmine-oracle` (this file only; cut from `origin/main` @ `d0a01797`).

> **Erratum — child correction commit (does not amend/rebase `98074b8`).** Four corrections to the parent
> commit, each below: (1) the first firmware-chunk framed length was mis-stated as **14,340 B**; the correct
> value is **14,344 B** = `DMA_HDR_LEN(4) + roundup4(14336) + trailer(4)` (the parent dropped the 4-byte
> trailer; verified against `fw_logic::build_chunk`, fw_logic.rs:141). (2) the HS `wMaxPacketSize = 512`
> assumption is now cited/qualified (§4a defs). (3) an explicit **INFO-word field-equivalence** table is added
> (§4a (vi)). (4) the S3 transport-flag defect is restated as **precedent for the host-desync *class*, not proof
> NeoPlutarch has that defect** (§4). The **512-multiple / ZLP conclusion is unchanged**: 14,344 mod 512 = 8, so
> the first fw chunk is still not a 512-multiple.

> **Untrusted-input discipline.** PM evidence and the four boundaries are treated as hypotheses. Every claim is
> tagged **[FACT]** (observed in source/evidence), **[INFER]** (deduction), or **[HYP]** (unverified). Jarvis
> owns the repair; this is review input only.
>
> **Byte-oracle discipline.** Every exact byte / patch / register value below was pulled from **raw** source
> (`curl` of `raw.githubusercontent.com`, patchwork `/mbox/`, and direct reads of the NeoPlutarch tree) — **not**
> through any summarizing fetch.

---

## 0. One-paragraph bottom line

The channel-7 timeout is structurally the **RF freq-plan `RANDOM_WRITE`** (the *first* MCU in-band command of the
new channel), not deferred `CAL_BW` → **H1 confirmed**; the CAL_BW-defer design cannot address it. Upstream Linux,
on an MCU-response timeout, **returns `-ETIMEDOUT` and does nothing else — no vendor reset, no firmware reload**;
our `recover_chip()` forced cold reload is an **upstream-unfaithful invention** → **H2 partially confirmed**
(futile, upstream-unfaithful; bounded on the survey path, `?`-propagating/strand-risk on the targeted path). The
one documented root cause of the *named* upstream "mt7601u dies during channel switch" wedge was a **host-side
transport-flag bug, recovered in-band with no replug/reload** — so the driver's in-source "unrecoverable silicon
death / replug-only" narrative is **not established** by the available evidence, which proves only *"USB transport
alive, MCU/FCE not draining."* **Missing ZLP is explicitly NOT established as TEST 3's cause** (see §4a): the
current NeoPlutarch MCU frame already carries the upstream four-byte zero trailer, the observed freq-plan command
is 40 B (not a 512-multiple), and *current mainline itself ships with no `URB_ZERO_PACKET`*. H3/H4 are Chad/Janus
lanes.

---

## 1. Sources (exact paths / commits / Message-ID)

| # | Source | Establishes |
|---|--------|-------------|
| S1 | `torvalds/linux drivers/net/wireless/mediatek/mt7601u/phy.c` — `__mt7601u_phy_set_channel`, `mt7601u_phy_set_channel`, `mt7601u_vco_cal` | Channel-switch op order; RF17–20 via `write_reg_pairs(MT_MCU_MEMMAP_RF)` (in-band MCU); `vco_cal` via direct `rf_wr` (RF_CSR_CFG); CAL_BW runs **every** hop; cal work queued **only when not scanning**. |
| S2 | `…/mt7601u/mcu.c` — `mt7601u_mcu_wait_resp`, `__mt7601u_mcu_msg_send`, `mt7601u_write_reg_pairs` | Timeout **5 × 300 ms**; final timeout **`return -ETIMEDOUT`**, **no reset/reload**; reg-pair writes wait CMD_DONE. |
| S3 | **LKML/Patchwork "mt7601u dies during channel switch"** — `https://patchwork.kernel.org/patch/5911331/`; **Message-ID `<CABikg9yCgELvoTMMf5bqfQTbL=VP-yMyMo3EnGC4CiN7OANqNA@mail.gmail.com>`**, Sergei Antonov `<saproj@gmail.com>`, **2 Mar 2015** | Root cause of the *named* wedge = host-side transport-flag defect; **fixed in-band, all 14 channels clean, no replug/reload**. IOMMU `IO_PAGE_FAULT` observed. See §4a for the verbatim diff. |
| S4 | `…/mt7601u/dma.c` `mt7601u_dma_submit_tx` (raw, current master) + `dma.h` `mt7601u_dma_skb_wrap` | Current **mainline sets NO `transfer_flags`** (no `URB_ZERO_PACKET`); frame trailer via `skb_put_padto(…, round_up(len,4)+4)`. See §4a. |
| S5 | `github.com/kuba-moo/mt7601u` #64; RHBZ 1264631 | Field corroboration of `mcu_wait_resp timed out` + `send MCU cmd failed:-110`; **disabling MCU calibration hangs the box** (do not "fix" by removing calibration). |

**NeoPlutarch anchors (read-only):** `phy.rs::set_channel` (RF_FREQPLAN first); `mod.rs::mcu_wait_resp` (996),
`mcu_write_reg_pairs` (1038, waits every chunk), `recover_chip` (1727), `scan` survey loop (1889) + targeted path
(1830, `recover_chip()?`), `inband_out_sync` (927), `send_staging` (946), `dma_fw` (1097); `mcu_logic::build_mcu_frame`
(mcu_logic.rs:38); `xhci.rs::submit` (1656).

---

## 2. Fact / inference / hypothesis table (per failure boundary)

### Boundary 1 — wedge is ch7 `RF_FREQPLAN`, not deferred `CAL_BW`
- **[FACT]** `phy.rs::set_channel`: the **first** MCU round-trip is the freq-plan `mcu_write_reg_pairs(MCU_MEMMAP_RF, 17–20)`; `CAL_BW` is *skipped* on survey. A timeout "during RF_FREQPLAN" is structurally *before* CAL_BW.
- **[FACT]** `mcu_write_reg_pairs` sends every chunk `wait_resp=true` (mod.rs:1050) → blocks in `mcu_wait_resp` → `Err("mcu response timed out")`. RF17–20 = one 4-pair chunk.
- **[FACT/S1]** Upstream also writes RF17–20 via `MT_MCU_MEMMAP_RF` in-band — mechanism is faithful.
- **[INFER]** **H1 CONFIRMED.** Deferring CAL_BW cannot prevent a wedge at the freq-plan write that precedes it. The defer is *harmless but non-curative*. ch7 freq-plan values are ordinary; ch1–6-then-ch7 death signals an **accumulating** condition, not a ch7-specific silicon fault.

### Boundary 2 — recovery clears EP state, forces cold reload, reload stalls at ILM byte 0, strands scan
- **[FACT]** `recover_chip` (1727): host EP recovery → `vendor_reset` → device `ClearFeature(ENDPOINT_HALT)` → **force `COM_REG0=0`** → `bring_up()` (full **cold** reload).
- **[FACT/evidence]** Forced reload's **first** fw chunk bulk-OUT times out (`ILM 0/45380`, `inband bulk-out timeout`).
- **[FACT/S2]** Upstream does **not** reload/reset on MCU timeout — the forced reload is **our** addition, with no upstream precedent for in-band mid-session MCU revival.
- **[FACT]** Survey path (1932): `recover_chip().is_ok()` — **bounded** (`MAX_SCAN_RECOVERY_ATTEMPTS=2`), non-propagating (`wedged+=1; continue`). Targeted path (1859): **`recover_chip()?`** — **propagates**, aborting `scan()` with no discriminator.
- **[INFER]** **H2 PARTIALLY CONFIRMED.** The reload is upstream-unfaithful and **futile** on this wedge, and mutates chip state twice before fast-skip. On the *survey* path (TEST 3 = `CHANNEL=0`) the loop **already bounds** it, so "prevents the bounded WEDGED/skip" is **not** the survey-path failure mode — the "no terminal discriminator" outcome is better explained by **H3** (sink ordering). The genuine H2 strand risk is the **targeted-path `?`-propagation**.

### Boundary 3 — sink after USB init; bind-time scan fails before sink; zero sink despite serial
- **[FACT]** scan runs during **pre-sink bind enumeration** (phy.rs:50–57 comment; scan is inside bind). Evidence: sink all-zero, serial non-empty → **RED** disagreement.
- **[INFER]** Consistent with H3. **Defer to Chad (Lane 5)** for the init-order file/line map + safe earlier-sink design.

### Boundary 4 — firmware residency not preserved across return/reboot
- **[FACT/evidence]** "Warm" boot did a cold `45412`-byte upload → non-residency.
- **[INFER]** Orthogonal to the channel-switch wedge. **Defer to Janus (Lane 4).**

---

## 3. State-machine trace — upstream vs rejected candidate vs required invariant

```
            UPSTREAM (post-fix, S1/S3)          REJECTED CANDIDATE (2d43670)         REQUIRED INVARIANT
ch tune  →  [bw unchanged: BBP R4 only]         [same early-return]                 same
         →  RANDOM_WRITE RF17-20 ─ CMD_DONE      RANDOM_WRITE RF17-20 ─ CMD_DONE     wait CMD_DONE; on timeout →
         →  rmw TX_ALC_CFG (direct)              rmw TX_ALC_CFG (direct)               NO reload; bounded truthful
         →  RANDOM_WRITE BBP_LNA ─ CMD_DONE      RANDOM_WRITE BBP_LNA ─ CMD_DONE       WEDGED/skip that REACHES
         →  vco_cal (RF_CSR_CFG, direct)         vco_cal (RF_CSR_CFG, direct)          serial + sink; preserve
         →  CAL_BW (MCU) ─ CMD_DONE              CAL_BW  *** DEFERRED/SKIPPED ***       budget + fence accounting
         →  agc_reset (scanning)                 (—)
   ch1..14: ALL COMPLETE                    ch1..6 OK; ch7 RF17-20 write TIMES OUT
                                                   │
                                                   ▼
                                            recover_chip():
                                              host EP reset (OK)   ← xHC ring reset only
                                              vendor_reset         ← chip-side; cannot touch USB SIE toggle
                                              clear-halt EP (OK)   ← USB SIE alive, NOT MCU alive
                                              COM_REG0:=0 ; bring_up ← FORCED cold reload (upstream never does)
                                                └─ fw chunk0 bulk-OUT TIMES OUT (ILM 0/45380)
                                                     = device NAKs forever = MCU/FCE not draining
                                            survey: bounded → wedged++ → fast-skip  (targeted: `?` → ABORT)
```

**Key divergence:** upstream reaches a *terminal* (14-ch success, or `-ETIMEDOUT` surfaced with **no reload**). The
candidate inserts a **forced reload known to fail**, converting a clean "MCU timed out" into a longer,
chip-state-mutating detour before the same skip.

---

## 4. The load-bearing upstream lesson (S3)

The **only** documented root cause of the *named* wedge was **host-side transport**: a URB transfer-flag defect
that produced an IOMMU `IO_PAGE_FAULT` and desynced the command pipe until the MCU stopped responding — **fixed
in-band, no replug, no reload, all 14 channels clean afterward.**

> **Precedent, not proof (per PL — explicit).** S3 is **precedent that this wedge *class* is host-side and
> in-band-recoverable** — it is **NOT** evidence that NeoPlutarch's driver contains that specific defect or any
> defect. S3's fix is a libusb/URB one-liner (`URB_ZERO_PACKET`) that **does not map** to our from-scratch
> xHCI + `dma_arena` transport, and §4a shows our MCU framing is byte-identical to upstream's *working* frame
> (so the S3 mechanism is *excluded* as our proximate cause). What S3 licenses is a **prior** — "prefer the
> host-side-desync hypothesis over 'silicon is bricked' until a transport repro decides it" — to be tested, not
> a finding to be asserted. The proximate cause of *our* wedge remains **[HYP]/open** (the four candidates
> below), and none is established.

- **[INFER]** Our wedge is at least as consistent with a **host-side USB/DMA transport defect** as with silicon
  death. "Survives ch1–6, dies at ch7" fits an **accumulating pipe/DMA desync** better than a ch7-specific fault.
- **[HYP]** Proximate mechanism is **open** and *not proven* silicon. Candidate host-side causes for Jarvis to
  audit (all in *our* new xHCI/FCE/fence seam; none provable from current evidence): (a) command/response-pipe
  **sequence or toggle desync** accumulating across hops (the upstream class); (b) **FCE /
  `TX_CPU_FROM_FCE_CPU_DESC_IDX`** ring-index drift so the engine is never correctly kicked for the reload chunk;
  (c) **`dma_arena::fence`** returning/programming a stale/aliased address under repeated submits (the upstream
  IOMMU-fault analogue — I-14 c1); (d) **`cmd_resp` buffer reuse** in `mcu_wait_resp` reading a stale front-word.

---

## 4a. Byte-level framing comparison  *(the specific ask)*

**Definitions.** Two distinct "zero" mechanisms must not be conflated:
- **Frame trailer** — a 4-byte zero appended to the DMA-wrapped *payload* (part of the buffer content).
- **Transport ZLP** — a USB zero-length packet emitted by the host controller when the *total* transfer length is
  an exact multiple of the bulk endpoint `wMaxPacketSize`, so the device sees a short packet to terminate the bulk
  transfer (`URB_ZERO_PACKET`).

> **`wMaxPacketSize = 512` — cited & qualified.** For a USB 2.0 **high-speed bulk** endpoint the spec fixes
> `wMaxPacketSize` at exactly **512 bytes** (USB 2.0 §5.8.3 "bulk transfers": HS bulk max data payload = 512;
> §9.6.6 endpoint-descriptor constraints). The dongle enumerated **at high speed** (TEST-3 evidence:
> "enumerated cleanly at high speed as `148f:7601`"), so 512 is the spec-mandated value for its bulk endpoints.
> **[Qualify]** The *authoritative* value is still the enumerated **endpoint descriptor `wMaxPacketSize`** — the
> analysis below should be re-checked against the descriptor actually read at bind, not the spec default, before
> any ZLP claim is treated as closed. (If the pipe were ever FS/LS the max would be 64/8; the multiple-of-N test
> shifts accordingly. None of the §4a numbers reach a 512- **or** 64-multiple, so the conclusion is robust to
> that qualification.)

### (i) Historically broken upstream framing — pre-2015-03-02 out-of-tree driver
Transport flag on the TX bulk URB (file `dma.c`, `usb_kick_out`):
```
q->e[e].urb->transfer_dma   = q->e[e].dma;
q->e[e].urb->transfer_flags |= URB_NO_TRANSFER_DMA_MAP;   // pre-mapped DMA; NO ZLP
```

### (ii) Corrected upstream framing — Antonov patch (Message-ID `<CABikg9y…OANqNA@mail.gmail.com>`, 2 Mar 2015)
Verbatim diff hunk (raw `patchwork /mbox/`):
```diff
--- a/dma.c
+++ b/dma.c
@@ -355,7 +355,7 @@ int usb_kick_out(struct mt7601u_dev *dev, struct sk_buff *skb, u8 ep)
  usb_fill_bulk_urb(q->e[e].urb, usb_dev, snd_pipe, skb->data, skb->len,
   mt7601u_complete_tx, q);
  q->e[e].urb->transfer_dma = q->e[e].dma;
- q->e[e].urb->transfer_flags |= URB_NO_TRANSFER_DMA_MAP;
+ q->e[e].urb->transfer_flags |= URB_ZERO_PACKET;
  ret = usb_submit_urb(q->e[e].urb, GFP_ATOMIC);
```
→ The out-of-tree fix **replaced** the pre-mapped-DMA hint with **transport ZLP**.

### (iii) SHIPPING mainline framing — `drivers/net/wireless/mediatek/mt7601u/` (current master)
`dma.c` `mt7601u_dma_submit_tx` (verbatim, raw): **sets NO `transfer_flags` at all** —
```c
usb_fill_bulk_urb(e->urb, usb_dev, snd_pipe, skb->data, skb->len,
                  mt7601u_complete_tx, q);
ret = usb_submit_urb(e->urb, GFP_ATOMIC);          /* no URB_ZERO_PACKET, no URB_NO_TRANSFER_DMA_MAP */
```
Frame content via `dma.h` `mt7601u_dma_skb_wrap` (verbatim, raw):
```c
info = flags | FIELD_PREP(MT_TXD_INFO_LEN, round_up(skb->len, 4)) | ... ;
put_unaligned_le32(info, skb_push(skb, sizeof(info)));      /* prepend 4-byte INFO word     */
return skb_put_padto(skb, round_up(skb->len, 4) + 4);       /* +4 = 4-byte ZERO TRAILER     */
```
→ Shipping mainline: **4-byte INFO word ++ payload ++ zero-pad ++ 4-byte zero trailer**, total
`round_up(P+4, 4) + 4`, and **no transport ZLP**. (The 2015 out-of-tree ZLP flag did **not** propagate to
mainline; mainline works without it.)

### (iv) Current NeoPlutarch — MCU frame + xHCI TRB
`mcu_logic::build_mcu_frame` (mcu_logic.rs:38, verbatim):
```rust
let total = roundup4(p + regs::DMA_HDR_LEN) + 4;   // DMA_HDR_LEN = 4  → roundup4(P+4) + 4
// dst = [4B INFO word][payload][zero pad to roundup4][4B zero trailer], fully zero-filled first
```
Firmware-chunk frame `fw_logic::build_chunk` (fw_logic.rs:141, verbatim):
```rust
let xfer = regs::DMA_HDR_LEN + roundup4(len) + 4;  // = 4 + roundup4(len) + 4  (trailer included)
// dst = [4B FCE TXINFO][data][zero pad to roundup4][4B zero trailer], fully zero-filled first
```
Bulk-OUT submit `xhci.rs::submit` (1656, verbatim intent): a **single Normal TRB**, `len = xfer`, control
`(1<<5)|(1<<2)` = **IOC | ISP**, ring doorbell. **No separate zero-length terminating TRB.**

### (v) Byte-length equivalence
| Frame | Total length formula | value |
|-------|----------------------|-------|
| freq-plan `RANDOM_WRITE`, P = 4 pairs × 8 = **32 B** — upstream `mt7601u_dma_skb_wrap` (iii) | `round_up(P+4, 4) + 4` | `round_up(36,4)+4` = **40 B** |
| freq-plan `RANDOM_WRITE`, P = **32 B** — NeoPlutarch `build_mcu_frame` (iv) | `roundup4(P+4) + 4` | `roundup4(36)+4` = **40 B** |
| INFO-word LEN field (freq-plan) | `round_up(P,4)` | `roundup4(32)` = **32** (both) |
| first fw chunk, P = `MCU_FW_URB_MAX_PAYLOAD` = **14,336 B** — NeoPlutarch `build_chunk` (iv) | `DMA_HDR_LEN(4) + roundup4(P) + trailer(4)` | `4 + roundup4(14336) + 4` = `4 + 14,336 + 4` = **14,344 B** |

**Byte-identical (MCU command).** NeoPlutarch's MCU frame matches upstream's shipping frame exactly, including
the 4-byte zero trailer. **40 B is not a multiple of 512**; the first fw chunk's **14,344 B is not a multiple of
512** either (`14,344 = 512 × 28 + 8`) → no transport ZLP would be emitted for either transfer even if
`URB_ZERO_PACKET`/an xHCI ZLP-TRB were present.

### (vi) INFO-word field equivalence  *(explicit, per PL)*
The 4-byte little-endian INFO/TXINFO word is bit-field-identical between upstream `dma.h` and NeoPlutarch
`regs.rs`/`mcu_logic::dma_cmd_info`. Upstream masks are `GENMASK(hi,lo)`; NeoPlutarch stores the equivalent
shift/mask (source lines cited):

| Field | Upstream `dma.h` | NeoPlutarch (`regs.rs`) | Match |
|-------|------------------|-------------------------|-------|
| LEN     | `MT_TXD_INFO_LEN = GENMASK(15,0)` → bits 0–15, mask `0xffff` | `TXD_INFO_LEN_MASK = 0xffff` (215) | ✓ |
| D_PORT  | `MT_TXD_INFO_D_PORT = GENMASK(29,27)` → shift 27 | `TXD_INFO_D_PORT_SHIFT = 27` (216) | ✓ |
| TYPE    | `MT_TXD_INFO_TYPE = GENMASK(31,30)` → shift 30 | `TXD_INFO_TYPE_SHIFT = 30` (217) | ✓ |
| CMD SEQ | `MT_TXD_CMD_INFO_SEQ = GENMASK(19,16)` → shift 16 | `TXD_CMD_INFO_SEQ_SHIFT = 16` (218) | ✓ |
| CMD TYPE| `MT_TXD_CMD_INFO_TYPE = GENMASK(26,20)` → shift 20 | `TXD_CMD_INFO_TYPE_SHIFT = 20` (219) | ✓ |
| D_PORT **value** | `CPU_TX_PORT = 2` (enum `WLAN_PORT=0, CPU_RX_PORT=1, CPU_TX_PORT=2`) | `CPU_TX_PORT = 2` (209) | ✓ |
| TYPE **value** (command) | `DMA_COMMAND = 1` (enum `DMA_PACKET=0, DMA_COMMAND=1`) | `DMA_COMMAND = 1` (212) | ✓ |
| TYPE **value** (fw chunk / packet) | `DMA_PACKET = 0` | `DMA_PACKET = 0` (211) | ✓ |

**Worked value — the freq-plan `RANDOM_WRITE` INFO word** (P = 32 ⇒ LEN = `roundup4(32)` = `0x20`), fixed bits
(excluding the per-message SEQ at 16–19 and the `CMD_RANDOM_WRITE` opcode at 20–26):
`LEN(0x20) | (CPU_TX_PORT=2 << 27 = 0x1000_0000) | (DMA_COMMAND=1 << 30 = 0x4000_0000)` = **`0x5000_0020`**,
identical under upstream `FIELD_PREP` and NeoPlutarch `dma_cmd_info`. The fw-chunk TXINFO differs only in TYPE =
`DMA_PACKET(0)` and carries no SEQ/CMD-TYPE (upstream `mt7601u_dma_skb_wrap_pkt`; NeoPlutarch `fce_txinfo`,
fw_logic.rs:125) — also identical field-for-field.

### (vii) Conclusion on ZLP — **NOT established as TEST 3's cause**  *(per PL, byte-confirmed)*
1. **[FACT]** NeoPlutarch already carries the **upstream 4-byte zero trailer**; frame length is byte-identical to
   shipping mainline.
2. **[FACT]** The observed freq-plan command is **40 B — not a 512-multiple**; the *first* fw chunk is
   `DMA_HDR_LEN(4) + roundup4(14336) + trailer(4) = 14,344 B` (`= 512 × 28 + 8`) — also **not** a 512-multiple.
   Neither implicated transfer would emit/require a ZLP.
3. **[FACT]** **Shipping mainline sets no `URB_ZERO_PACKET`** and works — so a transport ZLP is not load-bearing
   for MCU-command correctness on this part.
4. **[INFER]** Therefore **missing xHCI ZLP is NOT presently established as TEST 3's cause.** It remains a *latent*
   correctness gap (`xhci.rs::submit` emits no ZLP TRB for any exact-512-multiple transfer) worth closing on its
   own merits, but a ZLP change **must not** be claimed as a TEST-3 fix — it cannot alter the 40 B / 14,344 B
   transfers that actually failed.

---

## 5. xHCI host recovery vs device / MCU liveness — what the recovery trio proves

- **[FACT/INFER]** **Host EP recovery** (Reset Endpoint + Set-TR-Dequeue) resets only the **xHC's** endpoint-ring
  view. Proves nothing about the device.
- **[FACT/INFER]** **`ClearFeature(ENDPOINT_HALT)`** is serviced by the device's **default control endpoint (USB
  SIE)**, independent of the WiFi MCU. Success proves **USB transport alive** + bulk halt/toggle cleared — **NOT**
  that the MCU/FCE is alive or that bulk-OUT will now be drained.
- **[INFER]** The observed trio — *host reset OK + clear-halt OK + next bulk-OUT still times out* — is exactly the
  signature of **"USB transport alive, MCU/FCE not draining"** (device NAKs bulk-OUT forever). Equally consistent
  with (i) dead MCU **and** (ii) FCE not re-armed / host-side desync after our reset. The in-source assertion that
  (i) is *proven* ("nothing at the USB layer can [resurrect it]") **overstates** the evidence.
- **[FACT/S2]** Upstream's only firmware (re)load is at probe/init or after a full USB **re-probe**
  (disconnect→connect). "Firmware reload after an in-scan MCU timeout" has **no upstream sanction**.

---

## 6. Explicit warnings — attractive-but-invalid recovery paths

1. **Do NOT force a cold firmware reload after an MCU channel-switch timeout.** [S2] Upstream never does;
   [evidence] it stalls at ILM byte 0. Futile + mutates chip state twice before the same fast-skip. Treating
   `clear-halt = OK` as license to reload is a **category error** (§5).
2. **Do NOT deepen CAL_BW-defer as "the fix."** [INFER] The wedge is at the freq-plan write *before* CAL_BW;
   upstream does CAL_BW every hop and (post-fix) does not wedge.
3. **Do NOT conclude "unrecoverable silicon death / replug-only"** from current evidence. [S3] The documented
   root cause of this exact wedge was host-side and in-band-recoverable. Rule out the host-side transport class
   (§4/§4a) before shipping a "silicon is bricked" narrative into comments and terminal categories.
4. **Do NOT ship a ZLP tweak as a TEST-3 fix.** [§4a] Both implicated lengths (40 B, 14,344 B) are non-512-multiples
   and mainline ships without ZLP; a ZLP change cannot cure TEST 3 and must not be claimed as such.
5. **Do NOT leave the targeted-path `recover_chip()?`** (mod.rs:1859) if the goal is a bounded truthful terminal —
   a failed recovery there aborts `scan()` with no discriminator (the survey path already bounds it).
6. **Do NOT "fix" by disabling MCU calibration.** [S5] Field reports show it hangs the machine.

---

## 7. Scope / handoffs / contradiction to preserve

- **No patch, no merge recommendation** (lane contract). **No production/test code edited.** This document is the
  sole deliverable on branch `codex/mt7601u-lane2-trapmine-oracle`.
- **Contradiction to preserve (coordination contract §):** the driver's in-source assertion that the wedge is
  *proven* unrecoverable silicon death conflicts with upstream S3 (host-side, in-band-recoverable). Both recorded
  with evidence; PM adjudicates. My weight: **silicon-death unproven; host-side transport is the stronger prior**,
  but neither is closed without a transport-level repro Jarvis owns. Cerebro's hostile oracle (Lane 3) is the
  right instrument for the terminal-contract half; the silicon-vs-host proximate question needs a transport repro.
- **Cross-model note:** Claude-family analysis. A Codex-family (Jarvis/Chad) or Qwen (Prometheus) second read on
  §4/§4a/§5 would be genuine cross-model corroboration of the silicon-vs-host adjudication.

**Sources:** [patchwork — mt7601u dies during channel switch](https://patchwork.kernel.org/patch/5911331/)
(Message-ID `<CABikg9yCgELvoTMMf5bqfQTbL=VP-yMyMo3EnGC4CiN7OANqNA@mail.gmail.com>`) ·
`torvalds/linux drivers/net/wireless/mediatek/mt7601u/{phy,mcu,dma}.c`, `dma.h` ·
[kuba-moo/mt7601u #64](https://github.com/kuba-moo/mt7601u/issues/64) ·
[RHBZ 1264631](https://bugzilla.redhat.com/show_bug.cgi?id=1264631)
