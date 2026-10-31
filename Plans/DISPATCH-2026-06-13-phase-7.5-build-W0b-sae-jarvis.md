<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 7.5 BUILD-W0b — WPA3 SAE (dragonfly / P-256, H2E) → Jarvis

**Date:** 2026-06-13
**From:** PM/Cerebro
**To:** Jarvis (new branch off `origin/main` `45b9389`: `codex/phase-7.5-build-W0b-sae-jarvis`)
**Status:** DISPATCHED. **Host-tier only — NO hardware, NO dongle, NO QEMU. Gate = the verified cross-source SAE vectors now on `main` (I-12 byte-oracle applied to crypto).**
**Unblocks/closes:** the ISC-6 carve-out from BUILD-W0 (the WPA2 core, merged `ba1d511`). With W0b the W0 crypto lib is WPA2 **and** WPA3 complete — the full supplicant crypto core, host-proven, before the radio exists.
**The vector is already in hand (the blocker that carved this out is resolved):** `Plans/RESEARCH-2026-06-13-sae-vector-sourcing-FINDINGS.md` (on `main`, PM-verified — on-curve PASS, PWE byte-identical across hostap+iwd, P1+P2=PT checked by independent EC add). Read it first; it IS this build's oracle.

## 0. The decision this build makes
The WPA3 **SAE (Simultaneous Authentication of Equals — "dragonfly")** handshake over **group 19 (NIST P-256)**, added to the existing `net/wpa/` library as a pure host-tested module. It computes the PWE, the commit/confirm exchange values, and the SAE-derived PMK/KCK that the W2 on-air state machine will drive. It touches no hardware, no MLME, no kernel glue.

## 1. Two hard rules (carried from W0)
1. **Reuse, don't hand-roll.** Use the **vetted RustCrypto primitives already pulled by the `wifi` feature** — `p256` (field arithmetic, scalar mult, point ops, and its `hash2curve`/SSWU surface if it matches), `sha2`, `hmac`; **add the RustCrypto `hkdf` crate** (the H2E derivation is HKDF-Extract/Expand) under the same `wifi`-gated, `no_std`, default-off posture. **Do NOT hand-roll P-256 field math, the SSWU map, HKDF, or HMAC** (Article 15.7; the W0 / kTLS precedent). If a needed primitive isn't in RustCrypto's `no_std` surface, BLOCKER-to-PM, do not hand-roll.
2. **Every gate is a *published* known-answer assert, never round-trip (I-12).** The vectors are the FINDINGS bytes copied verbatim from hostap `common_module_tests.c` (IEEE 802.11-2020 Annex J.10) + iwd `unit/test-sae.c`. A self-consistent generate-then-verify passes even when both halves are wrong — that is exactly why SAE was carved out until a published vector existed. Assert against the FINDINGS numbers.

## 2. Implement H2E, NOT hunting-and-pecking (the scope lock)
The sourced vector is the **Hash-to-Element (H2E)** derivation — the modern, constant-time-by-construction method (password → PT via SSWU, then PWE = val·PT). **Implement H2E only.** Do **NOT** implement the legacy hunting-and-pecking loop: it is deprecated, carries the timing side-channel that has bitten real supplicants, and we have no vector for it. H2E is what WPA3 negotiates; minimal-sufficient-mechanism says build the one that matters and is provably correct.

## 3. ISCs — each gated by a FINDINGS vector
- **ISC-1 — Wire the oracle.** Bring the FINDINGS vectors into the host KAT suite (`tools/ai-ops-tests`) as `const` arrays, copied verbatim (do not reformat the hex). This is the pinned oracle the rest of the ISCs assert against. (Inputs: SSID `"byteme"`, password `"mekmitasdigoat"`, pwid `"psk4internet"`; PWE-pair MACs `00:09:5b:66:ec:1e`/`00:0b:6b:d9:02:46`; handshake MACs `4d:3f:2f:ff:e3:87`/`a5:d8:aa:95:8e:3c` — note the FINDINGS' two MAC pairs, §"Inputs".)
- **ISC-2 — PT derivation (H2E, password-only, MAC-independent).** pwd-seed = HKDF-Extract → u1/u2 = HKDF-Expand + reduction to field elements → SSWU map u1→P1, u2→P2 → **PT = P1 + P2**. KAT **every** intermediate against the iwd vector (`prk`, `okm1`/`u1_data`, `okm2`/`u2_data`, `p1x/y`, `p2x/y`, `ptx/y`). Trap: the HKDF-Expand→field-element reduction; the SSWU constant-time mapping (prefer `p256`'s `hash2curve` SSWU if its DST/construction matches 802.11 H2E — verify against P1/P2, don't assume; else drive SSWU from `p256` field ops, never hand-rolled field math).
- **ISC-3 — PWE = val · PT.** val = HKDF-Extract over the **sorted** MAC pair. KAT `val_data` then the final **PWE** (x,y) — cross-validated across both sources and on-curve (the highest-value KAT; the PWE is the timing-trap stage). **Trap: the MAC sort order feeding `val`** — KAT it explicitly.
- **ISC-4 — Commit construct/parse + serialization.** With the **fixed** `local_rand`/`local_mask`, commit-scalar = `(rand + mask) mod q`, commit-element = the SAE negation/inverse of `mask·PWE`. Serialize the 98-byte commit message (group-id ‖ scalar ‖ element-x ‖ element-y) and KAT byte-for-byte against hostap `local_commit[]`; parse `peer_commit[]` and validate the group id + that the element is on-curve. **Traps: the element is the inverse (the classic SAE bug), and the wire endianness/field-element width** — both KAT'd by the 98-byte oracle.
- **ISC-5 — Shared secret → KCK/PMK + PMKID.** From `peer_commit` + the local private, compute the SAE shared point, take `k = K.x`, run the SAE KDF → **KCK ‖ PMK**, and PMKID. KAT **KCK**, **PMK**, **PMKID** against hostap (published). **`k` needs no separate published oracle — KCK/PMK are published and `k` is their sole upstream input, so deriving the published KCK/PMK proves `k` transitively** (state this in the test comment). If you want an explicit `k` intermediate assert, compute the expected `k` with an **independent** tool (e.g. a documented Python EC-mult over the published commit values) and cite it — never assert our impl against our own impl.
- **ISC-6 — Confirm (the one reconstructed value — do it right).** confirm = `CN(KCK, send-confirm-counter, scalar_local, element_local, scalar_peer, element_peer)` = HMAC-SHA256. **Neither source publishes a confirm vector.** So the KAT must compute the expected confirm with an **independent HMAC-SHA256** over the **published** inputs (the published KCK + the published commit scalars/elements + the known counter) — an auditable, documented one-liner in the test, applying an already-KAT'd primitive to published bytes. This is a *known-answer* test (trusted primitive over published inputs), **NOT** a round-trip of our SAE code against itself. Make the independence explicit in the test so a reviewer can see the expected value did not come from the code under test.
- **ISC-7 — Standards.** All SAE code under `#[cfg(feature = "wifi")]` (default-off ⇒ bit-for-bit today, the kill-switch; add `hkdf` as a `wifi`-gated optional dep). No kernel glue, no hardware, no `unsafe`. `cargo fmt`; `clippy -D warnings`; the host KAT suite green (every SAE vector a published-answer assert). **Extend ANSPEC §10.9** (the SAE subsection: the H2E derivation, the commit/confirm exchange, the SAE PMK) + **amend ADR-051** (it already scopes the W0 crypto lib as WPA2/3 — record SAE as landed, H2E-only, with the dual-source vector provenance).

## 4. Gate (host only — no Stage B, no silicon)
- **Stage A (host):** the SAE KAT suite green — PT pipeline (pwd-seed/u1/u2/P1/P2/PT), val, PWE (both sources + on-curve), the 98-byte commit serialization, KCK/PMK/PMKID, and the independently-computed confirm; clippy; fmt; builds on+off the `wifi` feature. **No QEMU.**
- **No Stage B.** W0b touches no hardware.

## 5. Scope guard
The SAE crypto only. **Do NOT** build the MLME/auth state machine, the driver, or `NetDevice` glue (W1/W2); **do NOT** implement hunting-and-pecking (H2E only, §2); **do NOT** hand-roll P-256/SSWU/HKDF/HMAC (reuse RustCrypto); **do NOT** add SAE-PK, FT, or anonymous/identifier-less SAE variants beyond what the vector exercises (follow-ons); **do NOT** assert round-trip — the confirm is the only reconstructed value and it is computed by an *independent* primitive over *published* inputs. If `p256`'s `hash2curve` SSWU does not match the 802.11 H2E construction (verify against P1/P2), BLOCKER-to-PM rather than hand-rolling the map.

## 6. Merge (PM, on Stage-A PASS)
Close-gate: rebase onto current `main`, re-run the host KAT suite, push. PM `pm-merge-check` → `--ff-only` → `main`; ledger the audit log + ANSPEC §10.9 + ADR-051 amendment. Then the W0 crypto lib is **WPA2+WPA3 complete and host-proven** — and the WiFi arc's only remaining work is the hardware: **BUILD-7.5-W1** (the MT7601U USB SoftMAC driver, blind, needs the `148f:7601` dongle) → W2 (on-air, wiring W0/W0b's proven crypto to W1's frames).

---

## Relay — what to tell Jarvis
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-13-phase-7.5-build-W0b-sae-jarvis.md`
- **The oracle (on `main`, PM-verified):** `Plans/RESEARCH-2026-06-13-sae-vector-sourcing-FINDINGS.md`
- **Read them from `main`** (your tree is on a feature branch — a local file-open won't find them):
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-13-phase-7.5-build-W0b-sae-jarvis.md
  git show origin/main:Plans/RESEARCH-2026-06-13-sae-vector-sourcing-FINDINGS.md
  ```
- **Branch:** `codex/phase-7.5-build-W0b-sae-jarvis` off `origin/main` `45b9389`.
- **One-line:** Add **WPA3 SAE (dragonfly, P-256 group 19, H2E only — NOT hunting-and-pecking)** to `net/wpa/`. **Reuse RustCrypto** (`p256`/`sha2`/`hmac` + add `hkdf`, all `wifi`-gated, default-off) — never hand-roll the field math/SSWU/HKDF. **Gate = the verified vectors already on `main`** (`…sae-vector-sourcing-FINDINGS.md`, PM-checked on-curve + cross-source-identical): KAT the full PT pipeline (pwd-seed→u1/u2→SSWU P1/P2→PT), val, PWE (the timing-trap stage — cross-validated), the 98-byte commit serialization (trap: element is the inverse), KCK/PMK/PMKID (published; `k` is pinned transitively, no separate oracle), and the confirm hash (the **only** reconstructed value — compute it with an **independent** HMAC-SHA256 over **published** inputs, documented, NOT a round-trip). No `unsafe`, no hardware, no QEMU, host KATs are the whole gate, `[skip ci]` on merge. Extend ANSPEC §10.9 + amend ADR-051. This closes the WPA2+WPA3 crypto core; only the hardware (W1 driver, needs the dongle) remains in the WiFi arc.
