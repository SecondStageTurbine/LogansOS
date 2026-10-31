<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 7.5 BUILD-W0 — the WPA2/3 crypto + EAPOL library (dongle-independent) → Jarvis

**Date:** 2026-06-13
**From:** PM/Cerebro
**To:** Jarvis (new branch off `origin/main` `2f23965`: `codex/phase-7.5-build-W0-wpa-crypto-jarvis`)
**Status:** DISPATCHED. **Host-tier only — NO hardware, NO dongle, NO QEMU needed. Gate = IEEE/RFC known-answer vectors (the byte-oracle discipline, I-12, applied to crypto).**
**Ratified design:** `Plans/THINK-2026-06-13-phase-7.5-wifi-arc-mt7601u-supplicant.md` (PL ratified Q1–Q4, 2026-06-13): three-way slice (W0 crypto / W1 driver / W2 on-air), **W0 first** (dongle-independent, critical path); WPA2 first + WPA3/SAE on its own vectors; minimal STA scope.
**Why now:** the research proved no chip eliminates the host supplicant — so the WPA crypto is the hardest *software* in WiFi, it's fully host-testable without a radio, and building it now means W2 composes quickly once the `148f:7601` dongle arrives.

## 0. The decision this build makes
A self-contained, host-tested **WPA2/WPA3 key-management + EAPOL + CCMP library** — the crypto core the W2 on-air supplicant will drive. It computes the keys and the encrypted frames; it does **not** touch hardware, the MLME state machine, or kernel glue (those are W1/W2). Proven entirely against **published known-answer test vectors** before a single radio frame exists.

## 1. Reuse, don't reimplement — the one hard rule
**Do NOT hand-roll AES, SHA, HMAC, or the ECC field math.** kTLS already establishes the precedent (it uses the `chacha20poly1305` RustCrypto crate). Pull the **vetted RustCrypto primitives** (`aes`, `ccm`, `sha1`, `sha2`, `hmac`, `pbkdf2`, `p256` — all `no_std`-compatible, the same dependency posture as the kTLS AEAD crate) and write **only the WiFi-specific protocol layers** on top. Hand-rolled crypto primitives are a security footgun and an Article 15.7 violation (unearned machinery). The new crate deps are justified — they buy the WPA capability.

## 2. ISCs — the WiFi-specific layers (each gated by KATs)
- **ISC-1 — WPA2 PSK→PMK.** `PBKDF2-HMAC-SHA1(passphrase, SSID, 4096, 256 bits)`. KAT: the published IEEE 802.11i vectors (e.g. passphrase `"password"`, SSID `"IEEE"` → the known PMK).
- **ISC-2 — The 802.11 PRF + PTK/GTK derivation.** `PRF-n(PMK, "Pairwise key expansion", Min(AA,SPA)‖Max(AA,SPA)‖Min(ANonce,SNonce)‖Max(ANonce,SNonce))` via HMAC-SHA1; PTK length per cipher (CCMP = 384 bits → KCK/KEK/TK split). **Trap: the Min/Max ordering of the two addresses and the two nonces is the classic PTK bug** — KAT it explicitly. GTK derivation likewise.
- **ISC-3 — EAPOL-Key frame construct/parse + MIC.** Build/parse the EAPOL-Key descriptor (key-info bits, replay counter, nonces, key-data); compute the **MIC over the EAPOL frame with the MIC field zeroed**, selecting the algorithm by key-descriptor version (v2 = HMAC-SHA1, v3 = AES-CMAC). **Traps: the zero-the-MIC-before-compute step, and the version→algorithm selection** — KAT both.
- **ISC-4 — AES-CCMP.** CCMP encrypt/decrypt: the **nonce** (flag ‖ A2 ‖ PN, 13 bytes), the **AAD** from the masked 802.11 header, PN/replay handling. KAT: the CCMP test vectors published in the IEEE 802.11 standard (known plaintext/key/PN → known ciphertext+MIC). **Trap: which header fields are masked into the AAD** — KAT against the standard's worked example.
- **ISC-5 — PMKID + the 4-way-handshake key-confirmation glue (pure, no I/O).** PMKID = `HMAC-SHA1-128(PMK, "PMK Name"‖AA‖SPA)`. The handshake *logic* (which MIC to verify, key-install ordering) as a pure state function that W2 will drive — no frames sent here, just the verify/derive steps, KAT-able.
- **ISC-6 — WPA3 SAE (the hard one; its own vectors, per Q3).** The dragonfly handshake over P-256: **hash-to-element** (constant-time hunting-and-pecking or hash-to-curve — fixed iteration count, blinded, to avoid the timing side-channel that has bitten real supplicants), commit (scalar + element), confirm (the confirm hash). KAT: the RFC 7664 / IEEE 802.11 SAE test vectors. **This is gated on its own KATs and may land as a later commit within W0 if WPA2 (ISC-1..5) is ready first** — do not let SAE block the WPA2 core.
- **ISC-7 — Standards.** Pure **host-testable tier** (a `no_std` crypto lib + a host KAT test suite, the `*_logic`/`ai-ops-tests` pattern — path-`mod`'d or a workspace lib so `cargo test` runs the vectors); **no kernel glue, no hardware, no `unsafe`**; the new RustCrypto deps added to the kernel `Cargo.toml` (no_std, default-features off where needed); `cargo fmt`; `clippy -D warnings`; the host KAT suite green (every vector a known-answer assert — the I-12 oracle, NOT round-trip: a self-consistent encrypt/decrypt that's both wrong still passes, so assert against *published* answers). Open **ANSPEC §10.9** (the WiFi security/supplicant model — key hierarchy, the handshake, CCMP, SAE) + **ADR-051**.

## 3. Gate (host only — no Stage B, no silicon)
- **Stage A (host):** the KAT suite green (WPA2 PMK/PTK/GTK, EAPOL MIC v2/v3, CCMP encrypt+decrypt against the standard's vectors, PMKID, and — when ready — SAE commit/confirm against RFC 7664); clippy; fmt; builds on+off (the `wifi`/`mt7601u` feature, default-off, gates the deps). **No QEMU run is required** — this is a pure crypto library; the vectors are the proof.
- **No Stage B.** W0 touches no hardware. (W1 is the blind-hardware build; it needs the dongle.)

## 4. Scope guard
The crypto + EAPOL + CCMP + SAE *library* only. **Do NOT** build the MLME state machine, the driver, the `NetDevice` glue, or anything on-air (W1/W2); **do NOT** hand-roll AES/SHA/HMAC/ECC (reuse RustCrypto, ISC-1); **do NOT** add WPA-Enterprise/802.1X, AP-mode keys, or FT/roaming key hierarchies (follow-ons); **do NOT** assert round-trip only — every gate is a *published* known-answer vector. If a needed primitive isn't in RustCrypto's no_std surface, BLOCKER-to-PM rather than hand-rolling it.

## 5. Merge (PM, on Stage-A PASS)
Close-gate: rebase onto current `main`, re-run the host KAT suite, push. PM `pm-merge-check` → `--ff-only` → `main`. Then **BUILD-7.5-W1** (the MT7601U USB driver + scan — blind hardware, needs the `148f:7601` dongle; opens with its pre-seeded trap bank from the arc THINK §4). With W0, the hardest WiFi *software* is proven against the standard's own numbers before the radio exists — so W2 (the on-air supplicant) is mostly wiring W0's proven crypto to W1's frames.

---

## Relay — what to tell Jarvis
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-13-phase-7.5-build-W0-wpa-crypto-jarvis.md`
- **THINK (the arc design + the W-slice):** `Plans/THINK-2026-06-13-phase-7.5-wifi-arc-mt7601u-supplicant.md`
- **Read them from `main`** (your tree is on a feature branch — a local file-open won't find them):
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-13-phase-7.5-build-W0-wpa-crypto-jarvis.md
  git show origin/main:Plans/THINK-2026-06-13-phase-7.5-wifi-arc-mt7601u-supplicant.md
  ```
- **Branch:** `codex/phase-7.5-build-W0-wpa-crypto-jarvis` off `origin/main` `2f23965`.
- **One-line:** A host-tier WPA2/WPA3 crypto + EAPOL + CCMP **library** — no hardware, no QEMU. **Reuse RustCrypto** (`aes`/`ccm`/`sha1`/`sha2`/`hmac`/`pbkdf2`/`p256`, the kTLS-`chacha20poly1305` dependency precedent) and write only the WiFi-specific layers: PBKDF2 PSK→PMK, the 802.11 PRF + PTK/GTK (trap: Min/Max addr+nonce ordering), EAPOL-Key construct/parse + MIC (trap: zero-MIC-before-compute, v2-HMAC-SHA1 vs v3-AES-CMAC), AES-CCMP (trap: nonce = flag‖A2‖PN, the masked-header AAD), PMKID, and WPA3 **SAE** (P-256 dragonfly, constant-time hash-to-element — its own RFC-7664 vectors, may land after the WPA2 core). **Gate = published IEEE/RFC known-answer vectors** (I-12 oracle, NOT round-trip). No kernel glue, no `unsafe`, feature-gated deps, default-off. Open ANSPEC §10.9 + ADR-051. Host KATs are the whole gate — `[skip ci]` on merge (a pure crypto lib doesn't need the 45-min QEMU family). Next: W1 (the driver, needs the dongle).
