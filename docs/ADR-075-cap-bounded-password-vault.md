<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-075: Cap-bounded password vault — split operations, never master material

| | |
|---|---|
| **Status** | Proposed — Phase A implementation evidence complete; PM/PL ratifies |
| **Date** | 2026-07-20 |
| **Dispatch** | `Plans/DISPATCH-2026-07-20-password-vault-splitgpg-janus.md` |
| **Scope** | Threat model, crypto core, stable record/protocol bytes, host KATs |
| **Deferred** | Phase B vault process/IPC/persistence/QEMU; GUI shelf and browser/autofill |

## 1. Decision

Adopt Qubes split-GPG's boundary as a capability-native vault:

- one userland vault process holds the master key and encrypted store;
- the vault is spawned with a dedicated operation channel and storage caps, and **no network capability**;
- channel possession plus attenuated operation rights authorizes requests; process identity and ambient names do not;
- callers submit operations and one-shot least-rights grants. The master key, derived key, and ciphertext store never leave the vault. An authorized plaintext input/output crosses only through the grant named by that operation;
- Phase A supplies a `#![no_std]` pure core. Phase B binds it to `libneo::sys_getrandom`, IPC, grants, storage, and I-14/QEMU proofs.

No new kernel primitive or kill-switch is introduced in Phase A.

**Dependency policy:** the PL ratified external RustCrypto crates for userland
applications on 2026-07-20. The kernel's in-tree/hand-rolled policy is unchanged.

## 2. Threat model

### 2.1 Protected assets

The user's master password while unlocking, the Argon2id-derived master key, plaintext secret records, the full ciphertext store and its metadata/index, and authorization boundaries between callers.

### 2.2 Adversary

A caller may be fully compromised, send malformed/replayed/concurrent requests, retain every output it is authorized to receive, offer a short/wrong-rights grant, guess passwords, and observe response class/timing. An attacker may steal the at-rest store. The vault process may crash. Phase A does not claim protection against a compromised kernel, physical memory acquisition, malicious firmware/DMA outside I-14, or same-process arbitrary code execution inside the vault.

### 2.3 Guarantees

- A caller can obtain only the result of a verb authorized by its channel cap and operation rights.
- `retrieve` writes one selected plaintext into the explicitly offered output grant, up to its declared capacity. It never returns the master key, another record, the store, or an address into vault memory.
- `store` reads plaintext from a one-shot caller grant. `unlock` reads the password from a one-shot caller grant. Both inputs are wiped after consumption where the vault controls the backing memory; Phase B must revoke/unmap promptly.
- A wrong password, corrupt record, wrong AAD, or wrong key yields one public `AUTH_FAILED` class. No “unknown identifier vs wrong password/tag” oracle is returned while locked.
- AEAD authenticates the complete 40-byte header as AAD, binding algorithm identifiers, KDF cost, lengths, and ciphertext length.
- Each record gets an independent 128-bit salt and 96-bit nonce from `SYS_GETRANDOM`. A nonce is never reused with the same derived key. Generation failure fails closed.
- Passwords, master keys, plaintext inputs, authenticated output buffers, and failure-path scratch are zeroized. RustCrypto/`zeroize` supply the primitive; Phase B must avoid accidental copies, logging, formatting, core dumps, and swap.
- A vault crash exposes no response and cannot stall AI governance. Phase B proves bounded per-channel work, no governance lock, no DMA, and restart containment.

### 2.4 No-network-cap invariant

The Phase B spawn manifest/handle vector is an allow-list containing only its operation channel, store cap(s), and required runtime handles. It contains no socket, interface, DNS, network-service channel, or network meta-cap. Tests must inspect the actual child handle set. Grep is supporting evidence, not the proof. The vault cannot grant authority it never received.

Metadata, access pattern, record count/size, IPC timing, and an authorized retrieved plaintext remain observable to their respective observers. Padding and side-channel equalization are future hardening, not claims here.

## 3. Cryptography

| Purpose | Choice | Stable identifier |
|---|---|---|
| Password KDF | Argon2id v1.3 (`argon2` 0.5.x, RustCrypto) | `kdf_id = 1` |
| Record AEAD | ChaCha20-Poly1305, 256-bit key, 96-bit nonce (`chacha20poly1305` 0.10.x) | `aead_id = 1` |
| Randomness | Existing kernel ChaCha20 DRBG through `libneo::sys_getrandom` | no fallback in vault |
| Erasure | `zeroize`/`Zeroizing` | process-local discipline |

The v1 interactive baseline is 64 MiB, three passes, one lane, 32-byte output. Phase B benchmarks target hardware before ratification of production parameters; stored parameters permit future increases. Limits must be validated **before allocation** to prevent a hostile record/request from forcing unbounded memory. Salt is 16 bytes, nonce 12 bytes, tag 16 bytes, key 32 bytes.

The kernel DRBG is reused only as a service; the vault does not import kernel RNG code or implement another PRNG. Unlike `libneo`'s compatibility path for other consumers, the vault must treat `SYS_GETRANDOM` failure/absence as fatal and must not fall back to direct RDRAND.

## 4. Encrypted-secret record v1 (I-12 authority)

All integers are unsigned little-endian. Concatenation is:

```text
header[40] || salt[16] || nonce[12] || ciphertext[ciphertext_len] || tag[16]
```

The exact header is:

| Offset | Size | Field | v1 value / rule |
|---:|---:|---|---|
| 0 | 8 | magic | ASCII `NEOVLT01` |
| 8 | 2 | version | `1` |
| 10 | 2 | header_len | `40` |
| 12 | 2 | flags | `0`; reject unknown bits |
| 14 | 1 | kdf_id | `1` = Argon2id |
| 15 | 1 | aead_id | `1` = ChaCha20-Poly1305 |
| 16 | 4 | memory_kib | bounded nonzero Argon2 memory cost |
| 20 | 4 | iterations | bounded nonzero pass count |
| 24 | 4 | parallelism | bounded nonzero lanes |
| 28 | 2 | salt_len | `16` |
| 30 | 2 | nonce_len | `12` |
| 32 | 4 | ciphertext_len | plaintext/ciphertext byte count |
| 36 | 2 | tag_len | `16` |
| 38 | 2 | reserved | zero; reject otherwise |

The header bytes are AEAD AAD. Parser arithmetic is checked before slicing/allocation, exact file length is required, unknown versions/algorithms/flags are rejected, and authentication completes before any plaintext is returned.

### 4.1 Hand-laid worked byte example

This I-12 layout fixture deliberately uses conspicuous non-cryptographic body bytes so it is independent of the writer, reader, and AEAD KAT. Parameters are 65536 KiB / 3 passes / 1 lane, ciphertext length 3; salt is `a0..af`, nonce `b0..bb`, ciphertext `c0 c1 c2`, tag `d0..df`:

```text
0000: 4e 45 4f 56 4c 54 30 31 01 00 28 00 00 00 01 01
0010: 00 00 01 00 03 00 00 00 01 00 00 00 10 00 0c 00
0020: 03 00 00 00 10 00 00 00 a0 a1 a2 a3 a4 a5 a6 a7
0030: a8 a9 aa ab ac ad ae af b0 b1 b2 b3 b4 b5 b6 b7
0040: b8 b9 ba bb c0 c1 c2 d0 d1 d2 d3 d4 d5 d6 d7 d8
0050: d9 da db dc dd de df
```

`record::tests::hand_laid_record_byte_oracle` asserts these literal bytes against assembly and parsing.

## 5. Capability protocol v1

One dedicated anonymous channel endpoint is the session cap. Authority is the endpoint plus its derivation rights; IDs in payloads are selectors, never authority. Rights are `UNLOCK`, `LOCK`, `STORE`, and `RETRIEVE`; child/granted rights are intersections of donor actual, offered, and recipient maximum, with existing transitive revocation.

Capability transfer uses the existing explicit bounded handshake: `CAP_OFFER` → `CAP_READY` → `sys_grant`/`sys_recv_grant` → `CAP_ACCEPT`. A verb is processed only after its named transfer is accepted; request/transfer IDs bind the two sequences. The vault rejects unsolicited, duplicate, excessive-rights, wrong-kind, short, or reused transfers and closes them on every exit.

### 5.1 Common 24-byte envelope

| Offset | Size | Field |
|---:|---:|---|
| 0 | 4 | ASCII `NVP1` |
| 4 | 1 | version = 1 |
| 5 | 1 | kind: 0 request, 1 response |
| 6 | 2 | opcode LE |
| 8 | 4 | status LE signed; zero in requests |
| 12 | 8 | request_id LE, nonzero and unique while live |
| 20 | 4 | payload_len LE |

The host byte-oracle asserts a literal envelope for every verb, using request ID `0x0102030405060708` (wire `08 07 06 05 04 03 02 01`).

### 5.2 Verb payloads

| Opcode | Verb/right | Request payload (LE) | Success result |
|---:|---|---|---|
| 1 | `UNLOCK` | 0: `password_transfer_id u64`; 8: `password_len u32`; 12: flags/reserved `u32=0`; 16: `client_nonce u64` (24 B) | opaque `session_id u64`; password grant consumed/closed |
| 2 | `LOCK` | `session_id u64` (8 B) | no payload; all master/plaintext state wiped |
| 3 | `STORE` | 0: `session_id u64`; 8: `secret_id[16]`; 24: `input_transfer_id u64`; 32: `plaintext_len u32`; 36: flags `u32=0` (40 B) | record generation/version, not ciphertext/store |
| 4 | `RETRIEVE` | 0: `session_id u64`; 8: `secret_id[16]`; 24: `output_transfer_id u64`; 32: `output_capacity u32`; 36: flags `u32=0` (40 B) | bytes-written count after plaintext is placed in least-rights output grant |

`secret_id` is random/non-secret and cannot authorize access. The output grant must be writable by the vault only for the requested range; the vault receives no caller address-space or unrelated rights. Phase B defines the concrete grant kind and its least-rights mapping before implementation.

Public statuses are deliberately coarse: `OK`, `BAD_REQUEST`, `NOT_AUTHORIZED`, `LOCKED`, `AUTH_FAILED`, `OUTPUT_TOO_SMALL`, `BUSY`, `INTERNAL`. Authentication/corruption/wrong-password failures collapse to `AUTH_FAILED`; internal details are never sent or logged with secret material. Requests are bounded in size and per-channel concurrency.

## 6. Raw-source KAT provenance

Exact vector bytes were acquired on 2026-07-20 with `curl.exe -fL --proto '=https' --tlsv1.2` directly from RFC Editor raw text, never WebFetch or an LLM-rendered page:

| Vector | Raw authority | Downloaded file SHA-256 | Literal expected output |
|---|---|---|---|
| ChaCha20-Poly1305 | `https://www.rfc-editor.org/rfc/rfc8439.txt`, §2.8.2 | `25bef70fbf7a07ff45c2fe4cb7c6ce954eac687413d8610603268b4e4415324c` | 114-byte ciphertext `d31a…6116`, tag `1ae10b594f09e26a7e902ecbd0600691` |
| Argon2id v1.3 | `https://www.rfc-editor.org/rfc/rfc9106.txt`, §5.3 | `855c06f060379e34285e83a217e9069b5c72e161a1e54df9af5cd88dbb231f31` | `0d640df58d78766c08c037a34a8b53c9d01ef0452d75b65eb52520e96b01e659` |

The production `derive_master_key` path has a separate fixed-input → fixed-key
oracle: password ASCII `NeoPlutarch vault production KAT v1`, salt ASCII
`NeoVaultKATsalt!`, Argon2id v1.3, 65536 KiB, three passes, one lane, 32-byte
output → `572f73475f2deb7cf26e6c4c6fb1cfaf4f89314bcf731db40415ce7c24f7c4bb`.
The expected key was computed outside this crate by cloning the official
`P-H-C/phc-winner-argon2` reference implementation at commit
`f57e61e19229e23c4445b85494dbf7c07de721cb` from raw Git source, building its
C CLI, and running:

```text
printf '%s' 'NeoPlutarch vault production KAT v1' |
  ./argon2 'NeoVaultKATsalt!' -id -t 3 -m 16 -p 1 -l 32 -r
```

(`-m 16` means 2^16 KiB = 65536 KiB.) This literal test calls the production
function, so an algorithm/version/password-salt/parameter wiring change fails.

The Argon2id RFC vector includes password `01×32`, salt `02×16`, secret `03×8`, associated data `04×12`, memory 32 KiB, three passes, four lanes, and 32-byte tag. The KAT exercises those complete inputs. These small RFC parameters are test-only, never production policy.

## 7. Phase A evidence and Phase B obligations

`vault-core` is `#![no_std]`, uses only `alloc` plus pure-Rust RustCrypto/zeroize dependencies, and performs no syscall/I/O/network work. Host tests bind the actual functions and prove:

- RFC 8439 AEAD bytes and tag exactly;
- RFC 9106 Argon2id tag exactly;
- caller password and plaintext input are zeroed;
- tampering yields only authentication failure and exposes no plaintext;
- independent record and per-verb envelope byte fixtures.

Phase B must add: fail-closed `sys_getrandom` adapter, bounded parameter/parser policy, atomic persistence and nonce uniqueness across crashes, vault process spawn allow-list/no-network assertion, cap-transfer/grant protocol, lifecycle/concurrency limits, QEMU scenarios, I-14 analysis, and end-to-end memory-copy/zeroize audit. The GUI shelf, browser integration, autofill, signing, and generic decryption remain deferred.

## 8. Alternatives rejected

- AES-CCM in the WPA path: protocol-specific kernel code and wrong boundary.
- Home-grown AEAD/KDF/PRNG: unnecessary cryptographic risk.
- Returning secrets in IPC bytes: creates uncontrolled copies and violates least-rights transfer.
- Identity policy or ambient service names: coarser and less revocable than existing caps.
- One cap with all verbs: prevents attenuation and makes caller compromise broader.
- Persisting a password verifier: adds an offline oracle beyond the encrypted store; successful record authentication is the unlock proof.
- Building the process or GUI in Phase A: violates the stagger and gate boundary.
