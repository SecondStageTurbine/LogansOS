<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-051: Phase 7.5 BUILD-W0 (+ W0b) — the WPA2/3 supplicant crypto + EAPOL + CCMP + SAE library

**Status:** Accepted — WPA2 core (ISC-1..5) **PROVEN 2026-06-13** (11/11 host KATs); **WPA3 SAE (H2E) LANDED in W0b 2026-06-13** (5/5 SAE KATs; 16/16 total). The full supplicant crypto core is WPA2 **and** WPA3, host-proven before the radio exists.
**Date:** 2026-06-13 (W0); amended 2026-06-13 (W0b — SAE)
**Design:** `Plans/THINK-2026-06-13-phase-7.5-wifi-arc-mt7601u-supplicant.md` (PL-ratified: three-way slice W0 crypto / W1 driver / W2 on-air; W0 first, dongle-independent; WPA2 first, SAE on its own vectors)
**Dispatch:** `Plans/DISPATCH-2026-06-13-phase-7.5-build-W0-wpa-crypto-jarvis.md`; `Plans/DISPATCH-2026-06-13-phase-7.5-build-W0b-sae-jarvis.md`
**SAE oracle:** `Plans/RESEARCH-2026-06-13-sae-vector-sourcing-FINDINGS.md` (PM-verified cross-source vectors)
**Builds on:** the kTLS `chacha20poly1305` RustCrypto precedent (Article 15.7 — reuse vetted crypto, never hand-roll)
**Ledger:** no new invariant; I-14 untouched (host-tier, moves no memory — see "Containment")

## The decision

The WiFi supplicant's hardest part is **software** crypto, and it is fully
testable without a radio. So build it first, alone, proven against the
standard's own numbers — then W2 (on-air) is mostly wiring this proven crypto to
W1's frames. BUILD-W0 is a self-contained, host-tested **WPA2/WPA3 key-
management + EAPOL + CCMP (+ SAE) library**: it computes keys and encrypted
frames and touches **no** hardware, MLME state machine, or kernel glue (those are
W1/W2).

## The one hard rule — reuse, don't reimplement

**Do not hand-roll AES, SHA, HMAC, or the ECC field math.** kTLS already set the
precedent (it delegates AEAD to the `chacha20poly1305` RustCrypto crate). W0
pulls the vetted RustCrypto primitives — `aes`, `ccm`, `cmac`, `sha1`, `sha2`,
`hmac`, `pbkdf2`, `p256` (all `no_std`, the same dependency posture as the kTLS
AEAD) — and writes **only the WiFi-specific protocol layers** on top. Hand-rolled
primitives are a security footgun and an Article 15.7 violation (unearned
machinery); the new deps buy the WPA capability and are justified.

## The proof discipline — I-12 applied to crypto

Every layer is asserted against a **published** IEEE 802.11 / RFC known-answer
vector in the host suite (`tools/ai-ops-tests`, which path-`mod`s the kernel's
`net/wpa/*` files and runs `cargo test`). **Never round-trip:** a self-consistent
encrypt/decrypt that is *both* wrong still passes, so a writer + reader built by
the same team from the same (mis)reading of the spec agree with each other while
the bytes are off-spec. Only a published answer — or, for a byte layout, an
independent hand-laid oracle — proves correctness. (Method note, now standing:
exact crypto hex is transcribed from **raw source** via the shell, never via a
summarizing fetch tool that mangles hex.)

## The layers and their KATs (ISC-1..5 — PROVEN)

- **ISC-1 — PMK.** `PBKDF2-HMAC-SHA1(passphrase, SSID, 4096, 256 bits)`. KAT: the
  802.11i `"password"`/`"IEEE"` vector + a second (IWD) vector.
- **ISC-2 — the 802.11 PRF + PTK/GTK.** `PRF(PMK, "Pairwise key expansion",
  Min(AA,SPA)‖Max(AA,SPA)‖Min(ANonce,SNonce)‖Max(ANonce,SNonce))` over HMAC-SHA1;
  CCMP PTK = 384 bits → KCK/KEK/TK. **The Min/Max ordering is the classic PTK
  bug** — pinned two ways: directly against a published CCMP TK (IWD
  `test-crypto.c`) and transitively via a real frame-2 MIC. The PRF itself is
  pinned by the three published PRF-512 vectors. GTK likewise.
- **ISC-3 — EAPOL-Key MIC.** Computed over the frame **with the MIC field
  zeroed** (the zero-before-compute trap), algorithm by key-descriptor version:
  **v2 = HMAC-SHA1-128**, **v3 = AES-CMAC-128**. KAT: a published v2 frame end-to-
  end (IWD `test-eapol.c`) + the RFC 4493 AES-CMAC vectors for v3.
- **ISC-4 — AES-CCMP.** AES-CCM (M=8, L=2, 13-octet nonce) via the `ccm` crate,
  driven exactly as kTLS drives its AEAD (`AeadInPlace::encrypt_in_place_detached`).
  The CCM core is pinned by the **RFC 3610** packet vectors (the identical CCM
  configuration); the **802.11-specific construction** — the `priority‖A2‖PN`
  (big-endian) nonce and the masked-`FC`/masked-`SeqCtrl`/optional-`QoS` AAD — is
  pinned by an **independent hand-laid byte oracle** (the repo's "format/ABI byte
  oracle" rule), for both a non-QoS and a QoS data frame; the encap/decap wiring
  traces every output byte to a proven sub-oracle.
- **ISC-5 — PMKID + handshake key-confirmation.** PMKID =
  `Truncate-128(HMAC-SHA1(PMK, "PMK Name"‖AA‖SPA))`, pinned **end-to-end** against
  the canonical published example (passphrase → PMK via ISC-1 → PMKID reproduces
  the published answer, which a wrong label/order/truncation could not do given
  HMAC preimage resistance). The 4-way-handshake key-confirmation is
  `eapol::verify_mic` over the derived KCK (proven by ISC-2/3) — pure, no I/O.

## ISC-6 — WPA3 SAE (LANDED in W0b, H2E only)

SAE is the dragonfly PAKE over P-256 (group 19). **W0b implements Hash-to-Element
(H2E) only** — the modern, constant-time-by-construction PWE derivation WPA3
negotiates. The legacy **hunting-and-pecking** loop is **deliberately not
implemented**: it carries the timing side-channel that has bitten real
supplicants, H2E is what WPA3 uses, and minimal-sufficient-mechanism says build
the one that matters and is provably correct. The `p256` crate (with the
`hash2curve` + `expose-field` features) supplies the RFC 9380 SSWU map
(`MapToCurve`) and the OKM→field reduction (`FromOkm`); `hkdf`/`hmac`/`sha2`
supply the rest — **no hand-rolled field math, SSWU, or HKDF** (Article 15.7).

The pipeline (`net/wpa/sae.rs`): **PT** (MAC-independent) `= SSWU(u1)+SSWU(u2)`,
`u_i = HKDF-Expand(HKDF-Extract(SSID, pw‖id), "SAE Hash to Element u{i} P{i}", 48)
mod p`; **PWE** `= val·PT`, `val = (HKDF-Extract(0, MAX‖MIN macs) mod (q−1))+1`
(the MAC-sort trap); **commit** `scalar=(rand+mask) mod q`, `element=−(mask·PWE)`
(the inverse trap), wire `group(2 LE)‖scalar(32)‖x(32)‖y(32)`; **keys**
`keyseed=HMAC-SHA256(0³²,k)`, `KCK‖PMK=KDF-SHA256(keyseed,"SAE KCK and PMK",
(scalar+peer-scalar) mod q, 64)`, `PMKID` = that context's first 128 bits;
**confirm** `= HMAC-SHA256(KCK, send-confirm‖scalar1‖elem1‖scalar2‖elem2)`.

**The vectors (PM-verified, cross-source) and one honest subtlety.** The PT
pipeline (pwd-seed, u1/u2, SSWU P1/P2, PT) is KAT'd intermediate-by-intermediate
against iwd `test-sae.c`; the **PWE** point is KAT'd directly and is byte-identical
across hostap + iwd and on-curve — confirming `p256`'s `FromOkm` equals the
802.11 mod-p reduction and `MapToCurve` equals the 802.11 SSWU (verified against
P1/P2, not assumed). **Subtlety:** hostap's §J.10 *handshake* vector
(`local_commit`/KCK/PMK/PMKID) derives its PWE by **hunting-and-pecking**
(`sae_prepare_commit`), while only the separate `pwe_19` sub-test is H2E — so
there is no published *H2E handshake* vector. Because the commit-element /
key-schedule / confirm math is **PWE-agnostic**, ISC-5 KATs it against the
published KCK/PMK/PMKID by recovering the handshake's PWE algebraically from the
published commit (`PWE = mask⁻¹·(−element)` — pure published-byte algebra, **no
hunting-and-pecking implemented**); ISC-4 KATs the commit-scalar + the 98-byte
wire round-trip + on-curve directly; ISC-6 computes the confirm by an independent
HMAC-SHA256 over the published commit bytes. The published values remain the
oracle throughout — no round-trip, no fabricated vector. (Also corrected: the
FINDINGS rendered the PWE-pair `mac2` as `46` decimal/0x2e; the real byte is
**0x46**, confirmed by `val_data` matching only with 0x46 and by hostap's literal.)

## The kill-switch (ISC-7) and scope

Compiled only under the **`wifi`** feature: `wifi`-off (the default) ⇒ the crypto
deps are not pulled and the module does not exist ⇒ the kernel is **bit-for-bit
pre-WiFi**. No kernel glue, no `unsafe`, no MLME/driver/`NetDevice` work (W1/W2),
no WPA-Enterprise/802.1X, AP-mode, or FT/roaming hierarchies (follow-ons). The
gate is host-only: the KAT suite green + `clippy` + `fmt` + builds on/off. **No
QEMU run, no Stage B** — a pure crypto library; the published vectors are the
proof.

## Containment (I-14)

No new vector and no change to the ledger. The library moves no memory and runs
in the host tier. The firmware adversary it exists to talk to (W1's MediaTek
MT7601U blob) is contained by **USB itself**: a USB device cannot initiate DMA —
the xHCI controller mediates all memory movement, and that controller already
sits in VT-d's §10.6 **fence domain** (proven on silicon at 7.5-V, ADR-050). So
the WiFi crypto needs no containment machinery of its own.

## Consequences

- The hardest WiFi *software* — WPA2 **and** WPA3/SAE — is proven against the
  standards' own numbers before the radio exists; W2 becomes mostly wiring W0/W0b's
  proven crypto to W1's frames.
- With W0b the supplicant crypto core is **complete**: PMK/PRF/PTK/GTK, EAPOL-MIC,
  CCMP, PMKID (WPA2) + the full H2E SAE handshake (WPA3). The WiFi arc's only
  remaining work is hardware.
- Next: **BUILD-7.5-W1** — the MT7601U USB driver + scan (blind hardware, needs
  the `148f:7601` dongle), opening with the arc THINK's pre-seeded trap bank.
