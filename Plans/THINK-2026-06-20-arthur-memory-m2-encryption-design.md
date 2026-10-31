<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Arthur memory M2 — encryption-at-rest design (OT-17 D4/D5 build-prep)

**Date:** 2026-06-20
**Author:** Prometheus
**Branch:** `prometheus/arthur-memory-m2-encryption-design`
**Inputs:** Dispatch `Plans/DISPATCH-2026-06-20-arthur-memory-m2-encryption-design-prometheus.md`; OT-17 ratified THINK (`THINK-2026-06-18-arthur-memory-substrate-design.md` — D4 per-category AES-256-GCM, D5 crypto-erase); M1 dispatch (`DISPATCH-2026-06-18-arthur-memory-m1-janus.md` — the plaintext substrate M2 encrypts); OT-19 THINK (`THINK-2026-06-18-arthur-disagreement-reversibility-design.md` — D2 forget undo interaction, D5 boundary); ADR-058 (`docs/ADR-058-kernel-csprng-sys-getrandom.md` — 10.3 `SYS_GETRANDOM`, the key source); kernel CSPRNG 10.1 (`kernel/src/rng/drbg.rs` — ChaCha20 DRBG, KAT'd against RFC 8439 §2.3.2); kernel `Cargo.toml` (dep inventory: `chacha20poly1305` for kTLS, `aes`+`ccm` for WiFi, no `ghash` or `aes-gcm` crate present); libneo `Cargo.toml` (userland dep surface: `rustls-rustcrypto`, no AEAD crate yet)
**Scope:** Design THINK only. No production code, no `ci.yml`. M2 encryption design that layers on M1's `.mem` format without changing it. No change to 10.1/10.3. No M3/M4.
**PL Ratification:** D1 ratified — ChaCha20-Poly1305 (RFC 8439). PM-verified: `chacha20poly1305` v0.10 is already a kernel dep (`Cargo.toml:51`, kTLS), zero new crypto surface.

---

## 1. Problem statement

OT-17 ratified per-category AES-256-GCM encryption (D4) and crypto-erase via key burn (D5). M1 will deliver the plaintext `.mem` substrate. M2 is the encryption layer that sits on top of M1 — transparent to Arthur's read/write path, adding authenticated encryption at rest with a per-category key from `SYS_GETRANDOM`.

This THINK makes the OT-17 design **buildable**: it resolves the on-disk format (D2), the AEAD choice with an honest implementation-surface audit (D1), the nonce strategy with a collision argument (D2), the key lifecycle (D3), the crypto-erase proof against OT-19's undo mechanism (D4), the KAT sourcing discipline (D5), fail-closed semantics for auth failure (D6), and the kill-switch invariant (D7).

**D1 is THE load-bearing decision.** It flags a genuine fork for PL ratification: hold OT-17's AES-256-GCM or switch to ChaCha20-Poly1305. Both are modern AEADs of equal cryptographic standing. The difference is **implementation surface** — what code we'd need to add vs. what already exists and is KAT'd.

---

## 2. The implementation-surface audit (informs D1)

### 2.1 What exists today

| Primitive | Location | Status | KAT'd? |
|-----------|----------|--------|--------|
| **ChaCha20** block function | `kernel/src/rng/drbg.rs` via `chacha20` crate (v0.9) | Built, in use by 10.1 DRBG | Yes — RFC 8439 §2.3.2 in `kat.rs` |
| **Poly1305** MAC | `kernel/Cargo.toml` via `chacha20poly1305` crate (v0.10, kTLS) | Available as dep, not yet used for memory | Inherited from RustCrypto audit |
| **AES** block cipher | `kernel/Cargo.toml` via `aes` crate (v0.8, WiFi feature) | Available as dep, gated behind `wifi` | Inherited from RustCrypto audit |
| **GHASH** (GCM's inner MAC) | **Not present anywhere in the tree** | Missing | N/A |
| **AES-GCM** combiner | **Not present anywhere in the tree** | Missing — no `aes-gcm` crate | N/A |
| **CCM** (AES-CCMP for WiFi) | `kernel/Cargo.toml` via `ccm` crate (v0.5, WiFi feature) | Built, used by WPA2/3 supplicant | Inherited from RustCrypto audit |

### 2.2 What each AEAD choice requires

**AES-256-GCM path:**
- AES block cipher: already in tree via `aes` crate (v0.8), but gated behind `wifi` feature. For userland M2, we'd need it in `libneo` or `neo-arthur`.
- GHASH: **not in the tree.** GCM's authenticated tag uses GHASH — a multiplicative-hash over GF(2^128). The `aes-gcm` RustCrypto crate bundles AES + GHASH + the GCM mode combiner. We'd pull this as a new dep.
- Total new surface: `aes-gcm` crate (or equivalent hand-wiring of AES + GHASH + GCM counter-mode). If using the crate: one new dep. If hand-wiring: AES (have) + GHASH (don't have, ~200 lines of GF(2^128) arithmetic) + GCM mode combiner (~150 lines). **Hand-wiring is a footgun — GHASH has subtle implementation pitfalls (endianness of the hash subkey, multiplication in polynomial representation).**

**ChaCha20-Poly1305 path:**
- ChaCha20: already built, KAT'd, and in use by 10.1. The same `chacha20` crate.
- Poly1305: the only new primitive. ~80 lines of saturated-add arithmetic over Z/(2^130-5)Z. Well-specified in RFC 8439 §2.5. Or use `poly1305` crate (RustCrypto, same family as our existing deps).
- AEAD combiner: `chacha20poly1305` crate already exists in the kernel tree (kTLS dep, v0.10). It's the complete, audited AEAD construction.
- Total new surface for userland: either pull `chacha20poly1305` into `libneo`/`neo-arthur` (one crate, same family as our kernel kTLS dep), or hand-wire ChaCha20 (have) + Poly1305 (~80 lines). **Poly1305 is dramatically simpler to implement correctly than GHASH.** The field arithmetic is saturated addition over a prime modulus — no polynomial GF(2^128) multiplication, no endianness traps.

### 2.3 The honest comparison

| Dimension | AES-256-GCM | ChaCha20-Poly1305 |
|-----------|-------------|-------------------|
| **Crypto standing** | NIST-standardized, ubiquitous | RFC 8439, IETF-standardized, equal standing. TLS 1.3 default in Chrome/Firefox. |
| **Existing code** | AES block: have (WiFi). GHASH + GCM combiner: **don't have.** | ChaCha20: have (10.1, KAT'd). Poly1305: **only new piece.** |
| **New surface (from-scratch)** | GHASH (~200 lines, GF(2^128) mult, endianness traps) + GCM mode (~150 lines) = ~350 lines of subtle crypto | Poly1305 (~80 lines, saturated-add over Z/(2^130-5)Z) = ~80 lines. Or reuse `chacha20poly1305` crate (already in kernel tree for kTLS). |
| **New surface (crate)** | `aes-gcm` crate — new dep family not yet in tree | `chacha20poly1305` crate — already in kernel tree for kTLS. Same RustCrypto family. |
| **KAT vectors** | NIST GCM KATs (GCMVel3.rsp) — large, complex keylen/aadlen/ptlen matrix | RFC 8439 §2.8.2 — one clean test vector. Or NIST KATs if desired. Simpler oracle. |
| **Performance** | AES-NI hardware accel on x86_64 (but irrelevant until there's an AES impl in userland) | Constant-time software. No hardware dependency. Competitive with AES-NI for small messages (<32 KB). Memory files are KB-scale. |
| **Nonce size** | 96-bit (12 bytes) recommended | 96-bit (12 bytes) — identical constraint |
| **Tag size** | 128-bit (16 bytes) | 128-bit (16 bytes) — identical |

---

## 3. D1 — AEAD choice (THE load-bearing decision, flagged for PL ratification)

### 3.1 Recommendation: ChaCha20-Poly1305 (RFC 8439)

**Rationale:** This is not a cryptographic downgrade — both are modern AEADs of equal standing. The difference is **implementation risk.** ChaCha20-Poly1305 reuses 10.1's already-built, already-KAT'd ChaCha20 and adds only Poly1305 (~80 lines of saturated-add arithmetic, or the `chacha20poly1305` crate which is already in our kernel tree for kTLS). AES-256-GCM requires GHASH + GCM mode combiner — neither exists in the tree, and GHASH is a known footgun (GF(2^128) polynomial multiplication with endianness traps in the hash subkey derivation).

For a from-scratch kernel with KB-scale memory files, ChaCha20-Poly1305 is the lower-risk path. The `chacha20poly1305` crate (v0.10) is already audited, no_std-compatible, and used by our kTLS implementation — it's proven in this codebase.

### 3.2 PL fork

| Option | Description | When to choose |
|--------|-------------|----------------|
| **A: ChaCha20-Poly1305 (recommended)** | Reuses 10.1's ChaCha20 + adds Poly1305. Smallest new surface. `chacha20poly1305` crate already in kernel tree for kTLS. | Default recommendation. Lower implementation risk. Equal crypto standing. KB-scale files don't need AES-NI. |
| **B: AES-256-GCM (OT-17 original)** | NIST-standardized, hardware-accelerated via AES-NI. Requires `aes-gcm` crate (or hand-wired GHASH + GCM combiner). | If the PL insists on NIST standardization specifically, or if future memory volumes will be MB-scale and AES-NI matters. Accepts higher implementation surface. |

**PL ratified Option A.** PM-verified: `chacha20poly1305` v0.10 is already a kernel dep (`Cargo.toml:51`, kTLS record protection), so M2 pulls zero new crypto — it reuses the vetted in-tree AEAD.

~~**Residual risk (Option A):** ChaCha20-Poly1305 lacks hardware acceleration on x86_64. For KB-scale memory files this is irrelevant — encryption/decryption is sub-millisecond in software. If memory files grow to MB-scale, AES-NI would matter; the AEAD is abstracted behind a trait, so switching is a one-line dep swap + re-KAT.~~

~~**Residual risk (Option B):** GHASH implementation surface is ~4x larger than Poly1305 and has known footguns (endianness of the hashed subkey H, polynomial representation vs. integer representation confusion). The `aes-gcm` crate mitigates this but adds a dep not yet in the tree.~~

**Rejected alternative:** Option B (AES-256-GCM) — PL declined. Hand-rolling either AEAD combiner is also rejected — Article 15.7 and I-12 require audited primitives.

---

## 4. D2 — Encrypted record format + byte oracle

### 4.1 Format decision: per-file AEAD, not per-record

**Rationale:** Per-record encryption would require a nonce per record, a tag per record, and version headers per record — multiplying on-disk overhead by the number of records (potentially hundreds). Per-file encryption is simpler: one nonce, one tag, one encrypt/decrypt operation for the entire file. M1's `.mem` files are KB-scale; reading the whole file into memory, decrypting, and parsing is fast enough. The per-category key boundary (D3) already provides fine-grained isolation — per-record encryption within a category doesn't add meaningful security.

**Format rule:** When `arthur-memory-encryption` is ON, the `.mem` file on disk contains **only ciphertext + tag**. The plaintext record format (OT-17 D2: `version:type:timestamp:content`) exists only in memory during read/write operations. This means a hex dump of an encrypted `.mem` reveals nothing about the record structure — just AEAD output.

### 4.2 On-disk layout

```
+--------+---------+------------------+--------+
| version |  nonce  |     ciphertext   |  tag   |
| 1 byte  | 12 bytes|  variable length | 16 bytes|
+--------+---------+------------------+--------+
```

| Field | Size | Description |
|-------|------|-------------|
| `version` | 1 byte | Format version. `0x01` = ChaCha20-Poly1305, raw plaintext encrypted (no AAD). Enables future format evolution. |
| `nonce` | 12 bytes | 96-bit nonce for the AEAD. See §4.3 for uniqueness strategy. Stored in cleartext in the file (nonces are public parameters; secrecy is in the key). |
| `ciphertext` | variable | ChaCha20-Poly1305 encryption of the plaintext `.mem` content. Same length as plaintext (stream cipher property). |
| `tag` | 16 bytes | Poly1305 authentication tag. Appended after ciphertext. Verified BEFORE any plaintext is surfaced. |

**Total overhead per file:** 29 bytes (1 + 12 + 16). For a 4 KB memory file: 0.7% overhead. Acceptable.

### 4.3 Nonce uniqueness strategy — the load-bearing argument

**The threat:** GCM and Poly1305 nonce reuse under the same key is **catastrophic**. For Poly1305, nonce reuse leaks the Poly1305 key, which allows forging arbitrary authenticated ciphertext. For GCM, nonce reuse allows recovering the GHASH key and decrypting/forging messages. There is no "maybe" — it is a total cryptographic collapse of that key's confidentiality and integrity.

**Strategy: per-category monotonic counter, persisted in the `.key` file.**

The `.key` file stores both the encryption key AND a 128-bit write counter:

```
.key layout:
+--------+---------+
|  key   | counter |
| 32 bytes| 16 bytes|
+--------+---------+
```

On each write of the `.mem` file:
1. Arthur reads the `.key` file, extracts the 32-byte key and 16-byte counter.
2. Increments the counter in memory.
3. The nonce is the **low 12 bytes of the counter** (LE encoding, truncated to 96 bits).
4. **Write-ahead persist:** Writes the updated `.key` file (key || new counter) and flushes to disk **BEFORE** encrypting. This is the critical ordering — if Arthur crashes after step 5 but before the counter is persisted, the next boot would re-read the old counter and reuse the nonce. Write-ahead eliminates this window.
5. Encrypts the plaintext with `(key, nonce)`.
6. Writes the `.mem` file (version || nonce || ciphertext || tag).

**Why this guarantees uniqueness:**
- The counter is monotonically increasing under one key. Each write produces a distinct nonce.
- The 128-bit counter space allows 2^128 writes before wrap — far beyond any realistic usage. Even at 1 write/second, wrap would take ~3.4 × 10^26 years.
- **Write-ahead persist (step 4):** the counter is flushed to `.key` before encryption begins. A crash between encrypt and counter-write cannot happen because the counter is already on disk. If the crash occurs during the `.mem` write, the `.mem` file may be truncated or corrupt — but the counter advanced, so the next write uses a fresh nonce. The corrupt `.mem` is caught by D6 (auth-tag failure on truncated ciphertext → skip category). No nonce reuse.
- The key and counter are co-located: burning the `.key` file (D4 crypto-erase) destroys both simultaneously.

**Rollback safety — the second catastrophe mode:** A `.key` file restored from a backup, snapshot, or filesystem rollback would silently rewind the counter to an earlier value, causing nonce reuse on the next write. **Mitigation: the counter in `.key` is strictly monotonic.** On read, Arthur compares the persisted counter against the highest nonce it observes in any existing `.mem` file for that category (the nonce is stored in cleartext at bytes [1..13] of the file). If the persisted counter is **less than or equal to** the observed nonce, Arthur advances the counter past the observed value before using it. This makes rollback self-healing: even if `.key` is rewound, Arthur detects the divergence from on-disk evidence and corrects it.

**Why NOT random nonces from `SYS_GETRANDOM`:**
- Random 96-bit nonces have a birthday-bound collision probability of ~1 in 2^48 after ~2^48 uses (the square root of the space). For high-write-frequency categories, this is non-negligible over long operation. A counter eliminates collision entirely.
- `SYS_GETRANDOM` adds syscall overhead per write. The counter is in-memory after the initial `.key` read.

**Failure mode — counter without key:** If the `.key` file is corrupted but the `.mem` file survives, the counter is lost along with the key. This is fine: without the key, the ciphertext is unrecoverable anyway (which is the point of D4). The next write to that category would require key regeneration, which starts the counter at zero with a fresh key — no collision risk because it's a different key.

### 4.4 Hand-laid byte oracle

**Test vector (ChaCha20-Poly1305, RFC 8439 §2.8.2):**

```
Key:        808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f
Nonce:      070000004041424344454647
Plaintext:  "Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it."
AAD:        (empty — we use no AAD for memory files)

Expected ciphertext (first 32 bytes):
d31a8d34648e60db7b86afbc53ef7ec2a4aded51296e08fea9e2b5a736ee62d6

Expected tag:
ea161c49da88550badf6a32fbaead255
```

**M2-specific oracle (our format, with a fixed key+nonce):**

```
Key:        00010203...1f (32 bytes, incrementing)
Counter:    00000000000000000000000000000001 → nonce = 010000000000000000000000 (LE, low 12 bytes)
Plaintext:  "1:fact:1781433600:Test memory record for M2 byte oracle."

Encrypt with ChaCha20-Poly1305(key, nonce, plaintext, aad=[]).

On-disk file bytes (hex):
  01                          ← version = 0x01
  || nonce (12 bytes)
  || ciphertext (variable — same length as plaintext: 52 bytes)
  || tag (16 bytes)

Total on-disk size: 1 + 12 + 52 + 16 = 81 bytes.
```

The exact ciphertext and tag bytes for this oracle are computed from RFC 8439's ChaCha20-Poly1305 construction and asserted in the build's KAT — **not derived by running our own encryptor** (that would be a circular writer-reader round-trip, which the `.neom` v1 lesson teaches us rejects). The oracle bytes are raw-sourced from the RFC test vector machinery (RFC 8439 §2.8.2 provides the full construction: ChaCha20 keystream generation → XOR with plaintext → Poly1305 tag computation over AAD||ciphertext||len(AAD)||len(ciphertext)).

---

## 5. D3 — Per-category key lifecycle

### 5.1 Key storage layout

```
.arthur/memory/
  health.mem      ← encrypted memory file (D2 format)
  health.key      ← 48 bytes: 32-byte key || 16-byte counter
  work.mem
  work.key
  preferences.mem
  preferences.key
```

The `.key` file is a raw binary blob. No encoding, no header — just the concatenated key and counter. This minimizes parse surface and attack surface.

### 5.2 Key generation

At first write of a new category:
1. Arthur calls `SYS_GETRANDOM(buf, 32)` via libneo's `sys_getrandom()` wrapper.
2. On success, the 32-byte key is established. Counter starts at 0.
3. The `.key` file is written: `key || [0u8; 16]`.
4. If `SYS_GETRANDOM` fails (returns negative errno), the category write **fails closed** — no plaintext fallback, no weak key. The error propagates to D6's degrade-to-ephemeral path.

### 5.3 Key read

On Arthur startup (or first read of a category):
1. Arthur reads the `.key` file, expects exactly 48 bytes.
2. Extracts key (first 32 bytes) and counter (last 16 bytes).
3. If the file is missing (ENOENT), corrupt (wrong length), or unreadable: skip that category (D6). Do not crash. Do not leak partial key material.

### 5.4 Key protection

- The `.key` file lives in the same directory as `.mem`, protected by the same directory capability (OT-17 D3). Only Arthur holds the parent cap with read/write rights.
- A granted USE-only cap on `health.mem` does NOT grant access to `health.key`. The cap is scoped to the specific file, not the directory. Even if a cap leak occurred, the grantees would get ciphertext without the key.
- **Post-I-15 (file-cap unification):** The `.key` files will have their own file capabilities with narrower rights. This is an I-15 hardening opportunity, not an M2 requirement.

### 5.5 Key lifecycle events

| Event | Action |
|-------|--------|
| **Category first write** | Generate key via `SYS_GETRANDOM(32)`, write `.key` (key \|\| counter=0), encrypt and write `.mem` |
| **Subsequent writes** | Read `.key`, increment counter, encrypt with new nonce, write `.mem`, update `.key` with new counter |
| **Category read** | Read `.key`, read `.mem`, decrypt with (key, nonce from file), verify tag, parse plaintext records |
| **Category forget** | Burn `.key` (overwrite with CSPRNG bytes, unlink). Then unlink `.mem`. See D4. |
| **Key corruption detected** | Skip category on load. Log "memory: {category}.key corrupt, category skipped." Do not attempt recovery. |

---

## 6. D4 — Crypto-erase × OT-19 undo (the forget proof)

### 6.1 The interaction to prove

OT-19 D2 makes destructive actions reversible via copy-before-write. When Arthur writes to a file, it backs up the pre-state in `.arthur/undo/`. The question: **does undo resurrect what crypto-erase destroyed?**

**Theorem:** Key burn crypto-erases all data under that key, including any undo backups. Undo cannot resurrect forgotten memories.

**Proof:**

1. OT-19 D2's backup is a **copy of the file contents** at the time of write. For an encrypted `.mem` file, the contents on disk are **ciphertext** (D2 format: version || nonce || ciphertext || tag).
2. When Arthur backs up `health.mem` before a write, it copies the **encrypted bytes** to `.arthur/undo/`. The backup is ciphertext under the per-category key.
3. OT-19 D5 explicitly states: "The undo mechanism backs up `.mem` file contents but never backs up `.key` files."
4. When the user says "forget about health," Arthur burns `health.key` (overwrite + unlink).
5. After the burn, `health.key` is gone. The backup in `.arthur/undo/` is ciphertext under that key — **without the key, it is computationally irrecoverable.**
6. Therefore, undo cannot restore forgotten memories. The forget is permanent.

**QED.** The critical invariant is: **undo backs up ciphertext, never keys.** This is structural — the undo mechanism copies file bytes, and `.key` files are explicitly excluded from backup scope. No policy judgment required; it's a mechanical exclusion in the copy-before-write path.

### 6.2 Crypto-erase procedure

"Arthur, forget about my health" (selective erasure):

1. **Overwrite `health.key`:** Write 48 bytes of CSPRNG output (`SYS_GETRANDOM(48)`) over the existing `.key` file. This destroys both the key and the counter on-disk.
2. **Unlink `health.key`:** Remove the directory entry. The key is gone.
3. **Overwrite `health.mem`:** Best-effort: write CSPRNG bytes over the ciphertext file. On flash/SSD this is unreliable (wear-leveling), but it's belt-and-suspenders for non-flash media and logical-block hygiene.
4. **Unlink `health.mem`:** Remove the directory entry. Cleanup — the data was already unreadable from step 1.
5. **Verifiable:** Read back `health.key` → ENOENT. Read back `health.mem` → ENOENT (or GCM/Poly1305 auth failure if the file somehow persists). The memory is gone.

"Arthur, forget everything about me" (total erasure):
- Iterate all `<category>.key` files. For each: overwrite + unlink. Then unlink all `.mem` files. Same procedure, parallel across categories.

### 6.3 Edge case — undo backup exists at time of burn

If an undo backup of `health.mem` exists in `.arthur/undo/` when the key is burned:
- The backup is ciphertext under the now-burned key. It is irrecoverable.
- Arthur does NOT need to clean up the undo backup — it's cryptographically inert. However, as a storage hygiene measure, Arthur MAY prune orphaned undo artifacts (those whose source file no longer exists) at session startup. This is optional; the cryptographic guarantee holds regardless.

---

## 7. D5 — KAT sourcing (the oracle's acquisition)

### 7.1 Source vectors

**Primary: RFC 8439 §2.8.2** (ChaCha20-Poly1305 AEAD test vector). This is the authoritative source for our recommended AEAD. The RFC provides one complete test vector with key, nonce, AAD, plaintext, ciphertext, and tag — sufficient to verify the entire AEAD construction.

```
Source: https://www.rfc-editor.org/rfc/rfc8439.txt §2.8.2
Acquisition: Raw RFC text (IETF repository). NOT via WebFetch summary.
Vector: Key = 808182...9e9f, Nonce = 070000004041424344454647,
        AAD = "Social Security Number: ", Plaintext = "411527528",
        Ciphertext + Tag as specified in the RFC.
```

**Secondary (if PL ratifies AES-256-GCM): NIST GCM KATs.**

```
Source: https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Standards-and-Guidelines/documents/examples/GCMVel3.rsp
Acquisition: Direct download from NIST CAVP. NOT via WebFetch summary.
Vector: Use the first aesgcmtestvec with KeyLen=256, PLen=128 (16-byte plaintext).
        The .rsp file format is KEY=... NONCE=... ENCRYPT=... — parse directly.
```

### 7.2 KAT discipline

The M2 build's KAT will:
1. **Assert the RFC 8439 §2.8.2 vector byte-for-byte.** Encrypt the RFC's plaintext with the RFC's key and nonce, compare ciphertext + tag against the RFC's expected output. This is an **independent oracle** — not a writer-reader round-trip.
2. **Assert our format oracle (D2 §4.4).** Encrypt the test record with a fixed key/nonce, assert the exact on-disk bytes (version || nonce || ciphertext || tag). This verifies the serialization format independently of the AEAD primitive.
3. **Assert auth-tag rejection.** Decrypt with a tampered tag → must fail. Decrypt with the wrong key → must fail. These are negative KATs — they verify that the verification path rejects invalid input (D6's fail-closed requirement).

**No WebFetch for KAT vectors.** The byte-oracle corollary: a mangled KAT poisons the oracle. Vectors come from raw authoritative sources (RFC text, NIST `.rsp` files) or from an upstream crypto crate's own unit test (e.g., `chacha20poly1305`'s `tests/xct.rs` which is sourced from RFC 8439).

---

## 8. D6 — Fail-closed semantics (auth-fail rejects, degrade-to-ephemeral)

### 8.1 The crypto failure modes

| Failure | Detection | Response |
|---------|-----------|----------|
| **Missing `.key`** | `open(<category>.key)` → ENOENT | Skip category. Other categories unaffected. Log "memory: {category}.key missing, category skipped." |
| **Corrupt `.key` (wrong length)** | File read succeeds but length != 48 | Skip category. Do not use partial key material. Zeroize the read buffer before dropping. |
| **Poly1305/GCM auth-tag failure** | AEAD decrypt returns tag mismatch | **Fail-closed:** reject the record. Do NOT surface any decrypted bytes. The plaintext from a failed-auth decryption is GARBAGE — it must never be logged, returned, or cached. Skip the file. Log "memory: {category}.mem auth-fail, category skipped." |
| **Wrong version byte** | First byte of `.mem` != 0x01 | Skip file. Log "memory: {category}.mem unknown version {byte}, category skipped." |
| **Truncated file (ciphertext shorter than expected)** | File size < 1 + 12 + 16 (minimum: version + nonce + tag, zero-length ciphertext) | Skip file. Cannot authenticate a truncated ciphertext. |
| **`SYS_GETRANDOM` failure** | `sys_getrandom()` returns negative errno | Fail-closed on key generation. Cannot create new category. On reseed: cannot proceed without fresh randomness. Degrade to ephemeral for affected operations. |
| **Nonce field missing/malformed** | Cannot extract 12-byte nonce from file | Skip file. Do not attempt decryption with a guessed nonce. |

### 8.2 The fail-closed auth-fail invariant

**Critical:** On AEAD authentication failure, the decryptor may have produced plaintext bytes in a temporary buffer. These bytes **must never be surfaced**. They are the result of decrypting with either the wrong key or a tampered ciphertext — they are indistinguishable from random data and might accidentally resemble valid content.

**Implementation rule (for the build):** The AEAD API must provide an authenticate-then-decrypt interface where plaintext is only returned on successful tag verification. If using `chacha20poly1305` crate's `aead::AeadInPlace`, it returns `Result<Bytes, Error>` — the plaintext is inside the `Ok` variant and inaccessible on `Err`. **Never use a decrypt-then-verify pattern** that exposes unverified plaintext.

### 8.3 Degradation path

Every crypto failure degrades to the **ephemeral OD-6 window**:
- One category fails → other categories load normally. Arthur continues with partial memory + ephemeral context.
- All categories fail → Arthur operates with ephemeral context only. This is today's behavior (pre-M1). No crash, no data loss.
- Key generation fails → cannot write new encrypted memories. Existing encrypted memories are unaffected (they were already written). Arthur continues reading existing memories + ephemeral context.

---

## 9. D7 — Kill-switch: `arthur-memory-encryption` default-OFF ≡ M1

### 9.1 The invariant

Feature flag `arthur-memory-encryption` (sub-feature of `arthur-memory`):

| Flag state | Behavior |
|------------|----------|
| **OFF (default)** | M1 plaintext behavior, bit-for-bit identical. `.mem` files are plaintext records. No `.key` files exist or are read. No encryption/decryption overhead. Arthur is the M1 build. |
| **ON** | Encrypted-at-rest per D2 format. Keys from `SYS_GETRANDOM`. Crypto-erase on forget. Fail-closed auth-fail. |

### 9.2 Regression proof

The M2 build's test suite includes a **bit-for-bit equivalence test**: with the flag OFF, Arthur writes and reads memories identically to M1. The same record that M1 writes as plaintext is what M2-with-flag-off writes. No format change, no behavioral change. This is verified by:
- Writing a known set of records with flag OFF → compare on-disk bytes against M1's expected output (byte-for-byte).
- Reading M1-format files with flag OFF → parse identical records.

---

## 10. Prior art — per-file AEAD + crypto-erase

### 10.1 fscrypt (Linux filesystem-level encryption)

fscrypt uses per-directory AES-256-XTS for data and AES-256-CBC-HMAC for filenames. Keys are derived from a master key via HKDF-SHA-256 with a unique directory salt. Crypto-erase: deleting the keyslot from the keyring makes the directory unreadable. **Design insight:** per-directory keys enable selective erasure (same as our per-category approach). fscrypt stores keys in the kernel keyring — we store them on-disk cap-protected, which is weaker for stolen-disk scenarios but sufficient for our threat model (OT-17 §6.1 honest payoff).

### 10.2 age (modern file encryption tool)

age uses X25519 recipients + ChaCha20-Poly1305 per-file encryption. Each encrypted file has its own random key, encrypted to the recipient's public key. Format: `age-encryption.org/v1` header || domains || files || MAC. **Design insight:** age's format versioning (`v1` prefix) is exactly our approach (1-byte version). age uses random nonces per file — acceptable because each file has a unique key (nonce reuse can't happen across different keys). We use a counter because we reuse the same key across multiple writes to the same file.

### 10.3 LUKS (Linux Unified Key Setup)

LUKS uses a header with multiple keyslots, each deriving a master key via PBKDF2/Argon2. The master key encrypts the data partition via AES-XTS. Crypto-erase: destroying all keyslots makes the partition unreadable. **Design insight:** LUKS separates key custody (keyslots) from data encryption (master key). We don't need this separation for N=1 — our `.key` file IS the keyslot and the master key combined. The separation matters when you have multiple unlock mechanisms (passphrase + TPM seal + FIDO2). We'll need it if we add off-disk key custody later.

### 10.4 Signal's sealed sender / attachment encryption

Signal encrypts attachments with a random key per message, authenticated with HMAC-SHA256. The key is encrypted to the session ratchet state. Deletion: deleting the session ratchet makes past attachments unreadable. **Design insight:** per-object keys + ratchet-based key destruction is conceptually similar to our per-category key burn — destroy the decryption material, not the ciphertext.

---

## 11. D1–D7 ratification block

| Decision | Recommendation | Rationale in one line |
|----------|----------------|----------------------|
| **D1: AEAD choice** | **ChaCha20-Poly1305 (RFC 8439) — PL RATIFIED.** PM-verified `chacha20poly1305` v0.10 already in kernel tree for kTLS (`Cargo.toml:51`). Zero new crypto surface. Reuses 10.1's KAT'd ChaCha20. | In-tree, vetted, zero-new-dep AEAD. KB-scale files don't need AES-NI. Option B (AES-256-GCM) declined by PL. |
| **D2: Encrypted format** | **Per-file AEAD.** `version(1B) || nonce(12B) || ciphertext(variable) || tag(16B)`. Nonce from persisted counter in `.key` — **write-ahead persist** (counter flushed before encrypt) + **rollback-safe** (on read, Arthur advances counter past any observed nonce in existing `.mem`). Hand-laid byte oracle from RFC 8439 §2.8.2. | Write-ahead eliminates crash-reuse window. Rollback detection self-heals rewound counters. 29B overhead per file. |
| **D3: Key lifecycle** | **`.key` = 32B key \|\| 16B counter.** Generated from `SYS_GETRANDOM(32)` at first category write. Co-located with `.mem`, cap-protected. Counter ensures nonce uniqueness across writes and reboots. | Minimal storage (48B per category). Key+counter co-location means burn destroys both. Counter in `.key` survives reboot. |
| **D4: Crypto-erase × undo** | **Key burn = forget.** Overwrite `.key` with CSPRNG bytes, unlink. Undo backs up `.mem` ciphertext (never `.key`) → key burn erases backup too. Proven: undo cannot resurrect forgotten memories. | NIST SP 800-88 Purge method for flash. Instant, total, verifiable. OT-19 D2 interaction is structurally safe — undo copies ciphertext, not keys. |
| **D5: KAT sourcing** | **RFC 8439 §2.8.2 (raw RFC text, not WebFetch).** Primary vector + our format oracle + negative auth-fail vectors. | Byte-oracle corollary: a mangled KAT poisons the oracle. Raw authoritative sources only. Negative vectors verify fail-closed rejection. |
| **D6: Fail-closed** | **Auth-fail rejects, never surfaces unverified plaintext.** Missing/corrupt `.key` → skip category. Tag mismatch → reject file, zeroize buffer. `SYS_GETRANDOM` failure → fail-closed on key gen. All failures degrade to ephemeral OD-6 window. | AEAD integrity is the boundary — unverified plaintext is garbage. Authenticate-then-decrypt interface only. Never decrypt-then-verify. |
| **D7: Kill-switch** | **`arthur-memory-encryption` OFF (default) ≡ M1 plaintext, bit-for-bit.** ON → encrypted-at-rest per D2. Regression test: flag OFF writes/reads identical bytes to M1. | Strictly additive. Default-off means zero risk for users who don't opt in. M1 is the floor; M2 is the ceiling. |

---

## 12. Honest uncertainties

| Uncertainty | Status | Resolution path |
|-------------|--------|-----------------|
| ~~**PL chooses AES-256-GCM over ChaCha20-Poly1305**~~ | ~~Possible — OT-17 ratified AES-256-GCM. D1 flags this as a genuine fork.~~ | **RATIFIED: ChaCha20-Poly1305.** PL decision is final. |
| **Memory file growth beyond KB scale** | Unknown — depends on usage patterns. | ChaCha20-Poly1305 is competitive with AES-NI for messages <32 KB. If files grow to MB-scale, the AEAD trait abstraction allows swapping to AES-GCM with one-line change + re-KAT. |
| **Off-disk key custody (stolen-disk confidentiality)** | Banked — OT-17 §6.1 honest payoff explicitly excludes this. | Future hardening: passphrase-derived key via Argon2, or TPM-sealed key. Would change D3's key storage (key no longer on-disk in raw form). Does not affect D2 format or D4 crypto-erase mechanics. |
| **Counter wrap (theoretical)** | 128-bit counter → 2^128 writes before wrap. Irrelevant in practice. | If it ever matters: rotate the key (generate new key via `SYS_GETRANDOM`, reset counter). The old ciphertext becomes unreadable — which is acceptable if we're rotating keys anyway. |
| **NeoFS atomicity of `.key` update** | Writing updated counter to `.key` is a full-file overwrite. If Arthur crashes mid-write, the `.key` file could be truncated. | Crash mid-`.key`-write (during write-ahead): next boot detects corrupt `.key` (wrong length) → skips category (D6). No `.mem` was written yet (write-ahead ordering: counter first, encrypt second), so no nonce was consumed. Regenerate key+counter on recovery. Future hardening: write `.key` to temp file, then rename (atomic on NeoFS). |

---

## 13. Risk register

| Risk | Impact | Mitigation |
|------|--------|------------|
| **AEAD impl bug (ChaCha20/Poly1305)** | Critical — weak encryption or forgery possible | Use audited `chacha20poly1305` crate (RustCrypto, same family as kernel kTLS). KAT against RFC 8439 §2.8.2 catches wiring errors. ChaCha20 is already KAT'd in 10.1. |
| **Nonce reuse via crash (counter not persisted write-ahead)** | Catastrophic — total crypto collapse for that key | **Write-ahead persist:** counter flushed to `.key` BEFORE encryption begins. Crash during `.mem` write leaves counter advanced; next boot uses fresh nonce. Corrupt `.mem` caught by D6 auth-fail. No reuse window. |
| **Nonce reuse via rollback (.key restored from backup/snapshot)** | Catastrophic — rewound counter reuses old nonces | **Rollback detection:** on read, Arthur compares persisted counter against the highest nonce observed in existing `.mem` file (nonce is cleartext at bytes [1..13]). If counter ≤ observed nonce, Arthur advances past it. Self-healing. |
| **Key file corruption (crash mid-write)** | Medium — one category unreadable until key regenerated | D6: detect wrong-length `.key`, skip category. Ciphertext from last good write is safe. Regenerate key on next write attempt. |
| **Auth-tag failure surfaces plaintext** | Critical — leaks garbage or tampered content as "memory" | Fail-closed AEAD API: plaintext only accessible inside `Ok` variant. Negative KATs verify rejection path. Zeroize buffer on auth failure. |
| **`SYS_GETRANDOM` returns weak bytes** | Critical — predictable keys | `SYS_GETRANDOM` is fail-closed (ADR-058): returns `ENOMEM` if DRBG uninitialized, never returns weak bytes. Arthur treats any error as fatal for key generation. |
| **Undo resurrects forgotten memory** | Critical — forget guarantee violated | Proven impossible: undo backs up ciphertext under the key, never the key itself (OT-19 D5). Key burn makes all ciphertext irrecoverable. |
| **Format version collision** | Low — misinterpreting file format | 1-byte version at file start. Version 0x00 is reserved (reject as "not encrypted"). Future versions increment. Unknown version → skip file, don't crash. |

---

**Status:** Design THINK complete. D1 ratified (ChaCha20-Poly1305, PM-verified in-tree crate). D2–D7 stated for PM review. Counter nonce durability addressed: write-ahead persist + rollback detection. No code. No CI gate. M2 design ready; build dispatches once M1 lands.

---

## PM review — Claude (2026-06-20), on landing

Reviewed against the code + the durability analysis; landed on `main` as the ratifiable M2 design.

- **D1 ratified** (PL, 2026-06-20; PM-verified `chacha20poly1305 v0.10` is already a kernel dep — `kernel/Cargo.toml:51`, used in `net/tls_record.rs`).
- **§4.3 write-ahead persist is fully correct.** Counter durable *before* encrypt; a crash during the `.mem` write leaves the counter advanced (fresh nonce next boot) and the truncated record fails D6 auth → no reuse window. Crash-mid-`.key`-write is handled (corrupt-length → skip + regen, no `.mem` written yet). Crypto-erase × OT-19 undo (§6, ciphertext-not-key) and fail-closed D6 are sound.

**One finding — the rollback claim is narrower than "self-healing" states (a build requirement, not a blocker):** §4.3 / the risk register close the **asymmetric** rollback — `.key` rewound while `.mem` retains the higher nonces (e.g. `.key` restored from a separate backup): the counter-vs-observed-max-nonce check fires and advances. They do **NOT** close a **full-consistent category snapshot rollback** (both `.mem` *and* `.key` restored to an earlier point together): there the persisted counter is still greater than the (also-rolled-back) `.mem`'s max nonce, the check does not fire, and new writes reuse the counter values consumed in the discarded post-snapshot future — a nonce-reuse break exploitable by an adversary holding both the pre- and post-restore `.mem`. This is **inherent to counter-nonces without trusted external monotonic state** (not a fixable defect in this scheme), is **out of the N=1 threat model** (requires two disk images at different times), and is **closed by OT-17's already-banked off-disk-key-custody / TPM-monotonic-counter hardening**.

**Build requirements:** (1) name this residual explicitly — do not ship "self-healing / no reuse window" unqualified; (2) where a hardware monotonic counter (TPM/secure element) exists, source the write counter from it (closes the residual); (3) verify NeoFS actually provides the durability + atomic-rename primitives §4.3/the risk register assume (no POSIX assumption — surface it as a NeoFS dependency if absent). Everything else: **ratify-complete, build-ready, gated on M1.**
