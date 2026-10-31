<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Source a verified raw-source SAE (WPA3 dragonfly / P-256) KAT → Prometheus

**Date:** 2026-06-13
**From:** PM/Cerebro
**To:** Prometheus (research lane — a vector-sourcing mini-lane; NO kernel code)
**Status:** DISPATCHED. **Pure research/acquisition. The single deliverable is a *verified, literal-byte, non-round-trip* test vector — not an implementation.**
**Unblocks:** BUILD-7.5-W0b (WPA3/SAE), which is carved out of the merged W0 WPA2 core (`ba1d511` → `main`) precisely because this vector is missing.

## 0. Why this is its own lane
W0 shipped the WPA2 crypto core (ISC-1..5, 11 published KATs green). **ISC-6 — WPA3/SAE (the dragonfly handshake over P-256) — could not ship** because:
- The authoritative numeric vector lives in **IEEE 802.11 §J.10**, and the PL confirmed *"I don't have a copy of that PDF on this machine"* — the standard isn't freely fetchable.
- **Every open-source SAE implementation tests round-trip** (generate-then-verify against itself), which **I-12 forbids**: a writer and reader built from the same spec agree even when both are wrong. We will not ship SAE on a self-consistent oracle.
- **The raw-source-KAT rule (CLAUDE.md) binds this lane:** exact-byte crypto vectors are **NEVER** pulled through `WebFetch` or any summarizing/LLM-mediated read — those silently mangle hex (odd-length, dropped spaces, transposed digits) and *poison the very oracle meant to catch wrongness*. Pull the **raw authoritative file directly** (`git clone` / `curl` the source) and read the literal bytes.

## 1. The mission
Find — and copy **verbatim, from a raw authoritative source** — a complete numeric **SAE (RFC 7664 dragonfly) test vector over group 19 (NIST P-256)** sufficient to KAT each SAE stage **independently of our own implementation**. We need enough intermediate values that each step is a *known-answer* assert, not a round-trip.

### Required fields (the more intermediates, the better — each one is a separable KAT)
- **Inputs:** SSID/identifier, password (and any `password identifier`/`H()` salt the vector uses), the two MAC addresses (SAE mixes them into the PWE derivation).
- **PWE (password element):** the derived point on P-256 — `x` and `y` (this is the hash-to-element / hunting-and-pecking output; the classic timing-trap stage, so a KAT here is the highest-value one).
- **Commit:** the local **scalar** and **element** (x,y); the peer's scalar+element (the vector should fix the random `rand`/`mask` so the commit is deterministic and reproducible — without fixed randomness it's not a KAT).
- **Shared secret:** `k` (the agreed group element's x-coordinate after the commit exchange).
- **Keys:** **KCK** and **PMK** (and the `keyseed`/`KDF` intermediates if the source lists them).
- **Confirm:** the `confirm`/`peer-confirm` hash values.

## 2. Where to look (priority order — all are `git`/`curl`-able raw sources, NOT WebFetch)
1. **hostap (wpa_supplicant / hostapd)** — `git.w1.fi/hostap.git` (mirror: `github.com/latelee/hostap` or the official `w1.fi`). Check, by reading the literal files:
   - `src/common/sae.c` / `sae_pk.c` — comments and `#ifdef` test blocks sometimes carry the §J.10 numbers.
   - `tests/` — the Python hwsim suite (`tests/hwsim/test_sae.py`) and any `*.dat`/vector files; look for **fixed-randomness** SAE vectors (the ones that pin `rand`/`mask`), not the live-handshake tests.
   - Their `wlantest`/`sae` unit harness if present.
2. **The IEEE 802.11 §J.10 text itself**, if a *raw* copy is obtainable (a `.txt`/`.pdf` you can `curl` and read the literal digits from — NOT a summarizing fetch). The 802.11-2016/2020 §J.10 "SAE test vector" is the canonical source; D-revision drafts are sometimes openly mirrored.
3. **RFC 7664 (Dragonfly) appendix** — has worked examples, **but** confirm the group: 7664's primary examples may be FFC/other-curve; we specifically need **group 19 / P-256** to match our `p256` crate path. Use it as a cross-check, not the sole source, unless it carries a P-256 vector.
4. **FreeBSD / NetBSD `net80211` SAE**, **iwd's** (`git.kernel.org/pub/scm/network/wireless/iwd.git`) `unit/test-*.c` — iwd's unit tests are exactly the "published-vector, not round-trip" style we used for the WPA2 KATs (the CLAUDE.md rule cites them). Check `unit/test-sae.c` for fixed-input vectors.

## 3. Deliverable
`Plans/RESEARCH-2026-06-13-sae-vector-sourcing-FINDINGS.md` containing:
1. **The literal vector**, copied **verbatim** from the raw file (every hex string exactly as it appears — do not reformat, re-space, or "tidy" the digits).
2. **Provenance, exactly:** the source repo URL, the **commit hash**, the **file path + line numbers**, and **how you obtained the bytes** (`git clone`/`git show`/`curl` — state it explicitly; a WebFetch-sourced vector is disqualified and must be re-pulled raw).
3. **A coverage map:** which of the §1 fields the vector provides, and which SAE stages each one lets us KAT independently (PWE / commit / k / KCK / PMK / confirm).
4. **A sanity check (cheap, do it):** if the source ships the curve params, verify the PWE point actually lies on P-256 (`y² ≡ x³ − 3x + b mod p`) — a one-line check that the copied bytes aren't already corrupt. Report PASS/FAIL.
5. **The honest negative, if it happens:** if **no** source carries a fixed-randomness, published (non-round-trip) P-256 SAE vector, **say so plainly** — list what each source *did* have (most will have only live-handshake/round-trip tests) and stop. **Do NOT fabricate, guess, or compute-then-present a vector** — that defeats the entire purpose (it would be a round-trip oracle wearing a KAT's clothes). A clean "none of these four sources has a publishable §J.10-equivalent KAT; here's the evidence" is a *successful* outcome of this lane — it tells the PM the fallback (e.g. cross-validating two independent implementations on the same fixed input, or obtaining the IEEE PDF) is the next decision.

## 4. Scope guard
Research/acquisition only. **Do NOT** implement SAE, write Rust, or touch `net/wpa/`. **Do NOT** WebFetch the vector (raw `git`/`curl` only). **Do NOT** "clean up" the copied hex. **Do NOT** present a self-computed vector as a KAT.

---

## Relay — what to tell Prometheus
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-13-sae-vector-sourcing-prometheus.md`
- **Read it from `main`** (your tree is on a feature branch — a local file-open won't find it):
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-13-sae-vector-sourcing-prometheus.md
  ```
  Context (also on `main`): `git show origin/main:Plans/THINK-2026-06-13-phase-7.5-wifi-arc-mt7601u-supplicant.md` (the WiFi arc, §2 the W-slice) and the merged W0 dispatch `git show origin/main:Plans/DISPATCH-2026-06-13-phase-7.5-build-W0-wpa-crypto-jarvis.md` (ISC-6 is the SAE you're sourcing for).
- **One-line:** Find a **verified, literal-byte, NON-round-trip** SAE (WPA3 dragonfly, **group 19 / P-256**) test vector — PWE point + commit scalar/element + shared `k` + KCK + PMK + confirm — copied **verbatim from a raw `git`/`curl` source** (hostap `tests/`/`src/common/sae.c`, iwd `unit/test-sae.c`, RFC 7664 P-256 appendix, or a raw §J.10 copy). **Never WebFetch the bytes** (the raw-source-KAT rule — summarizing fetches mangle hex and poison the oracle). Verify the PWE is on-curve as a corruption check. Deliver `Plans/RESEARCH-2026-06-13-sae-vector-sourcing-FINDINGS.md` with the vector + exact provenance (repo/commit/file/lines/how-obtained) + a coverage map. **If no source has a publishable KAT, say so with evidence — do NOT fabricate one.** This unblocks BUILD-7.5-W0b (SAE); WPA2 already shipped.
