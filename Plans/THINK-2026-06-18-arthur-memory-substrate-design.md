<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Arthur's persistent-memory substrate design (OT-17)

**Date:** 2026-06-18
**Author:** Prometheus
**Branch:** `prometheus/arthur-memory-substrate-design`
**Inputs:** Dispatch `Plans/DISPATCH-2026-06-18-arthur-memory-substrate-design-prometheus.md`; live code at `origin/main` HEAD `bf30a04`; OT-17/OT-18/OT-20/OT-22 from `docs/OPEN-THREADS.md`; Article 10 (NeoFS), Article 23.2 (no telemetry), Article 29 (Arthur), Article 15.7 (lean patch); capability primitives (`process::derive_handle`, `process::revoke_handle`, `ipc::grant` at `kernel/src/sched/process.rs:866-930`); CSPRNG 10.1 landed (`bf30a04`); ADR-042 (slot arena — tensor-only, not for durable context); destination red-team THINK (`THINK-2026-06-18-arthur-destination-red-team.md`)
**Scope:** Design THINK only. No production code, no `ci.yml`, no NeoFS change. One pillar (OT-22): memory substrate + access-control to memory.

---

## 1. Problem statement

Arthur's context is a deliberately ephemeral userspace sliding window (OD-6). It remembers nothing across reboot, nothing across process restart. This is the correct floor — it means Arthur cannot accidentally retain what the user wants forgotten. But it is not the ceiling. A companion that forgets everything every session is a tool, not a relationship.

The generational slot arena (ADR-042) is correctly **tensor-optimized**: RAM-resident, generational/reclaimable, dedup-able — right for *model weights*, wrong for *durable personal context*. Arthur's memory (facts about the user, preferences, relationships, project state, conversation history) is the opposite resource: **small, durable across reboot, mutable, and privacy-critical.**

This design specifies **where memories durably live, what they are, who can read them, how they are encrypted, how they are forgotten, and how failure degrades safely** — so that when the build comes it has a PL-ratified design instead of an improvised one.

**Non-negotiables (from the constitution):**
- **Article 23.2:** On-device, on the user's data, **no telemetry ever.** Memory never leaves the device. Ever.
- **Article 29.4:** Arthur is userland, capability-bounded, model-agnostic. The memory substrate is userland.
- **Article 15.7:** Minimal-sufficient. The smallest memory model that makes Arthur usefully remember a real user, with a real privacy/forget guarantee. Reject machinery that doesn't earn its place.

---

## 2. Constraints and boundaries

### 2.1 What exists today (the substrate this builds on)

- **NeoFS (Article 10):** Durable filesystem, capability-gated file handles, content-addressed objects. Phase 3 complete, bare-metal verified. The only durable storage Arthur has access to.
- **Capability system (Phase 2, silicon-proven):** `Handle` with `CapRights` (USE/WRITE/DERIVE/REVOKE meta-rights). `process::derive_handle(pid, h, child_rights)` attenuates rights (requires parent DERIVE; child ⊆ parent). `process::revoke_handle(pid, h)` revokes through the derivation graph (requires parent REVOKE). `ipc::grant` delegates handles across process boundaries. **These are built.** OT-20 confirmed they work.
- **CSPRNG 10.1 (BUILD-10.1, `bf30a04`):** Kernel CSPRNG with RDRAND + timer-jitter mixing, ChaCha20 DRBG, KAT'd against RFC 8439 + NIST SHA-256 vectors. `fill_random()` is a **kernel-internal API** (10.1). The ambient userland surface (`SYS_GETRANDOM`) is **10.2 (OT-2, unbuilt)**. Arthur is userland — it needs `SYS_GETRANDOM` or a libneo wrapper to reach the CSPRNG.
- **Arthur Slices 1-4 (landed):** neo-shell, action surface, Arthur process, intent→action loop. Context = userspace sliding window. No durable memory.

### 2.2 What this does NOT design (OT-22 boundaries)

| Deferred | Why | Cross-link |
|----------|-----|-----------|
| General grant ergonomics (per-action friction, scope granularity, re-request) | Companion-vs-compliance-officer tradeoff is its own THINK | **OT-18** — this THINK only designs the memory-access cap, not how caps are granted generally. The grant surface is OT-18's problem. |
| Multi-Arthur memory sharing / provenance | One Arthur today; N=1 floor first | **OT-20** — when a second Arthur principal arrives, memory provenance ("Arthur-Ops read this because Arthur-Coder delegated cap Y") is OT-20's accountability layer. This THINK designs the single-Arthur memory substrate with a clean seam for OT-20's future action-provenance chain. |
| Model context-engineering (prompt injection, retrieval strategy) | Userland, model-specific | Not this pillar. Arthur's *query* strategy over its own memory is a model-layer concern, not a substrate concern. |

---

## 3. Design space: D1 — Where do memories durably live?

### 3.1 Options

| Option | Description | Properties |
|--------|-------------|------------|
| **A: NeoFS objects under a capability** | Memories are NeoFS files/objects. A dedicated directory (e.g., `.arthur/memory/`) on NeoFS. Arthur holds a capability to this directory. | Durable, survives reboot, mutable, capability-gated by construction. Uses existing NeoFS (Phase 3 complete). No new storage layer. |
| **B: Dedicated memory store (new kernel object)** | New kernel object class `Handle::MemoryStore`, analogous to `Handle::Model`. Kernel-managed, refcounted, with its own syscalls. | Purpose-built, clean semantics. Requires new kernel objects, new syscalls, new invariant. |
| **C: Structured DB layer (userland LMDB/RocksDB)** | Userland embedded database on top of NeoFS files. Indexing, queries, transactions. | Rich query surface, mature tooling. Adds a dependency, adds complexity, adds a failure mode layer. |

### 3.2 Tradeoffs

**Option B (new kernel object):** Violates Article 15.7. The memory substrate is a *userspace* concern — Arthur is userland (Article 29.4). A new kernel object class for Arthur's memory adds kernel surface for a userspace problem. The kernel already gives us NeoFS + capabilities, which compose to solve this. **Rejected.**

**Option C (embedded DB):** Violates Article 15.7 at the wrong layer. An embedded DB is machinery that doesn't earn its place for N=1 Arthur with kilobytes-to-megabytes of memory data. It's a solution searching for a problem — useful when Arthur has millions of indexed memories and complex queries. Today Arthur needs: append, read-all-in-category, delete-one, delete-all. A flat file per category with line-delimited records handles this without a database engine. **Rejected for now.** The seam to add indexing later is just "parse the files differently."

**Option A (NeoFS objects):** Minimal-sufficient. NeoFS is durable, capability-gated, and exists. A directory of files on NeoFS is the simplest durable store Arthur can use. The capability on the directory governs access. No new kernel surface. No new syscall. No new invariant. **Recommended.**

### 3.3 Recommendation: A — NeoFS objects under a capability

Memories live as files in a NeoFS directory (conventionally `.arthur/memory/` under the user's home). Arthur holds a `Handle::FileDir` capability scoped to this directory with USE + WRITE rights. The directory is created at Arthur first-boot; its capability is granted during Arthur process spawn (the same cap-bundle mechanism Slice 1-4 already use).

**Why NeoFS:**
1. **Durable by construction.** NeoFS objects survive reboot. Phase 3 proved this on bare metal.
2. **Capability-gated by construction.** The directory handle is a capability — unforgeable, attenuable, revocable. This is the authority primitive D3 needs.
3. **No new kernel surface.** Uses existing NeoFS syscalls (open, read, write, create, unlink). No ABI change.
4. **Fail-safe by construction.** If the directory is missing or corrupt, Arthur opens with EIO/ENOENT and degrades to ephemeral mode (D6). No crash.

**Why not a single file:** Multiple files per category enable fine-grained deletion (D5). "Arthur, forget about my health" deletes `health.mem` without touching `work.mem`. A single monolithic file requires full rewrite on partial delete.

---

## 4. Design space: D2 — What is a "memory" (the object model)?

### 4.1 Options

| Option | Description | Properties |
|--------|-------------|------------|
| **A: Opaque blobs** | Arthur writes whatever it wants; the substrate doesn't know the structure. | Maximum flexibility for the model. No schema to maintain. But no verifiable forgetting (D5 can't prove "all health memories deleted" if it can't parse them). |
| **B: Typed records (fact / preference / event / relationship)** | Each memory has a type tag, timestamp, category, and content. Structured enough for the substrate to reason about what it holds. | Enables verifiable forgetting by category. Enables Arthur to query by type. Minimal structure — just enough to govern. |
| **C: Append log + derived index** | Every write is appended; a separate index file maps categories → offsets. Supports efficient partial reads. | Efficient for read patterns where Arthur only needs "health" not "everything." Adds index consistency as a new failure mode. |

### 4.2 Tradeoffs

**Option A (opaque blobs):** Cannot satisfy D5's verifiable-forgetting requirement. If the substrate can't parse what's in the file, it can't prove "all memories about X are gone." The privacy promise (Article 23.2) requires verifiable erasure. **Rejected.**

**Option C (append log + index):** Premature optimization. Arthur's memory volume is small (kilobytes to low megabytes for a real user). Reading a 50 KB file into userspace and parsing line-delimited records is fast enough. An index adds consistency requirements (what happens when the index and data diverge due to crash mid-write?) that don't earn their place at N=1. **Rejected for now.** The seam to add an index later is trivial — the records are already structured.

**Option B (typed records):** Minimal-sufficient structure. Enough type information for verifiable forgetting, not enough to constrain what Arthur can remember. **Recommended.**

### 4.3 Recommendation: B — Typed records, minimal-sufficient schema

Each memory file (e.g., `health.mem`, `work.mem`, `preferences.mem`) contains line-delimited records. Each record is a single line with this structure:

```
<version>:<type>:<timestamp>:<content>
```

**Fields:**
- **version:** Single-digit schema version (currently `1`). Enables future format evolution without breaking old readers.
- **type:** One of `fact`, `preference`, `event`, `relationship`. These four cover the semantic space:
  - `fact` — Something Arthur learned about the user or their world ("your project is called LogansOS")
  - `preference` — Something the user prefers ("you prefer Rust over C++")
  - `event` — Something that happened ("we deployed Phase 8 on June 16")
  - `relationship` — How entities relate ("LogansOS is your kernel project")
- **timestamp:** Unix epoch seconds (LE u64, decimal). Enables temporal reasoning and TTL.
- **content:** UTF-8 text, the actual memory content. No newlines within a record (the line-delimiter is the record boundary).

**Example:**
```
1:fact:1781433600:Your kernel project is called LogansOS, named for your son.
1:preference:1781433600:You prefer capability-based security over discretionary access control.
1:event:1781433600:Arthur Slice 4 landed — model proposes, capability disposes.
```

**Why this schema:**
1. **Verifiable forgetting (D5):** The substrate can parse every record, filter by category/type, and prove deletion. Each file is a category; deleting the file deletes all memories in that category. Deleting individual records within a file is a line-by-line operation.
2. **Minimal:** One line per memory. No nesting, no references between records (yet — OT-20's multi-Arthur provenance will add cross-references later). The model writes structured lines; the substrate stores them.
3. **Forward-compatible:** The version field permits schema evolution. A future reader that sees `version != 1` knows to use a different parser.
4. **Human-readable:** The PL can inspect Arthur's memory with `cat`. This is intentional — transparency is a feature for on-device privacy.

**File naming convention:** Files are named by category: `health.mem`, `work.mem`, `personal.mem`, `preferences.mem`, `relationships.mem`. The category set is open — Arthur creates new files as needed. The substrate treats all `.mem` files in the memory directory uniformly.

---

## 5. Design space: D3 — The capability/privacy model over memory

**This is the load-bearing decision.** Arthur's memory of the user is the most privacy-sensitive data in the system. Who can read it? Who can write it? How is access governed?

### 5.1 The authority question

Arthur holds a `Handle::FileDir` capability for `.arthur/memory/`. This capability has rights (USE + WRITE + DERIVE + REVOKE). The question is: **can anything else hold a capability to this directory?**

The built primitives make three access patterns possible:
- **Direct grant:** Arthur delegates a derived handle to another process via `ipc::grant`. That process can read/write the memory.
- **Attenuated grant:** Arthur derives a read-only handle (`derive_handle` with USE-only rights, stripping WRITE/DERIVE/REVOKE) and grants it. The grantee can read but not write.
- **Revocation:** Arthur revokes the original directory handle; all derived handles in all grantees are revoked through the derivation graph (`revoke_handle` walks the `derivations` map).

### 5.2 Options

| Option | Description | Properties |
|--------|-------------|------------|
| **A: Single-cap, Arthur-only** | Only the Arthur process holds a capability to `.arthur/memory/`. No other process can access it, ever. The user reads memory via Arthur (natural language: "Arthur, what do you remember about me?"). | Maximum privacy. Simplest model. One principal, one cap. But: no tool integration. A calendar app can't ask Arthur "what meetings did the user tell you about?" |
| **B: Per-category caps** | Arthur holds the root directory cap. It can derive per-category read-only caps and grant them to specific tools (e.g., grant `health.mem` READ to a health dashboard). Revocation is per-capability. | Privacy by compartmentalization. A tool granted "calendar" can't read "health." Uses existing `derive_handle` + `ipc::grant`. Revocable through the derivation graph. |
| **C: Ambient directory access** | The memory directory is world-readable (or readable by all userland processes). Arthur writes, anyone reads. | Maximum interoperability. Minimum privacy. Violates the spirit of Article 23.2 — if any process can read Arthur's memory, the privacy boundary is ambient, not capability-secured. **Rejected.** |

### 5.3 Tradeoffs

**Option A (Arthur-only):** Correct for today (N=1 Arthur, no other principals). But it makes every future integration require Arthur as a proxy — the calendar app talks to Arthur, Arthur reads memory, Arthur answers. This is workable but adds latency and couples everything to Arthur's availability. **Too restrictive for the near future.**

**Option B (per-category caps):** Uses the capability primitives exactly as built. Arthur derives a read-only handle for `health.mem` (`derive_handle(root, child_rights=USE)`), then grants it to the health dashboard via IPC. The dashboard can read but not write. When the user wants to revoke, Arthur calls `revoke_handle` on the parent — and all derived handles in the dashboard are revoked through the derivation graph. This is the capability system doing what it was built for: **fine-grained, revocable delegation.**

**Why Option B satisfies the privacy boundary:**
1. **Read/write separation:** Tools get READ-only by default. Only Arthur writes memories. A compromised tool can't inject false memories.
2. **Category isolation:** A calendar tool gets `calendar.mem`, not `health.mem`. Blast radius is bounded by category.
3. **Revocation is real and immediate:** `revoke_handle` walks the derivation graph and kills all derived handles. No stale access, no "hope they close their file descriptor."
4. **Arthur remains sovereign:** Arthur holds the parent cap with DERIVE + REVOKE. It decides what to grant and can revoke at any time. The user directs Arthur via natural language ("Arthur, stop sharing my health data").

### 5.4 Recommendation: B — Per-category capabilities, read-only by default

**The model:**
- Arthur holds `Handle::FileDir` for `.arthur/memory/` with full rights (USE + WRITE + DERIVE + REVOKE).
- When another process needs memory access, Arthur derives a **read-only** handle (`derive_handle(parent, USE)`) and grants it via `ipc::grant`.
- The grantee can read the specific file(s) but cannot write, derive further, or revoke.
- Arthur can revoke at any time via `revoke_handle` on the parent — cascading through the derivation graph to all grantees.

**The OT-18 boundary:** This THINK designs *what* the memory-access capability is (per-category, read-only-by-default, revocable). It does NOT design *how* the user grants or manages these capabilities — that's OT-18's grant-ergonomics problem. OT-18 will design the user surface for "Arthur, share calendar with the calendar app" and "Arthur, stop sharing." This THINK assumes that surface exists and focuses on the substrate mechanics.

**The OT-20 seam:** When a second Arthur principal arrives (OT-20), each Arthur holds its own memory directory capability. Cross-Arthur memory sharing is a new delegation path that OT-20's provenance layer will govern. This design's per-category caps compose naturally: Arthur-A grants a derived cap to Arthur-B, with the same read-only-by-default semantics.

---

## 6. Design space: D4 — Encryption at rest

### 6.1 The threat model and the honest payoff

| Threat | What encryption achieves |
|--------|--------------------------|
| **Stolen disk / physical access** | **NOT protected by this design.** Keys reside on the same disk as ciphertext (cap-protected `.key` files beside `.mem` files). A disk imager recovers both. A capability is a runtime primitive — it does not protect offline bytes. Stolen-disk confidentiality requires off-disk key custody (passphrase-KDF or TPM-seal) — banked as hardening, not claimed now. |
| **Compromised non-Arthur process** | Encryption is defense-in-depth here, but the capability system already blocks access. A process without a memory cap cannot open the file, encrypted or not. |
| **Crypto-erase enablement (D5)** | **This is the primary payoff.** Per-category keys make cryptographic erasure the natural selective-forget mechanism: burn category X's key → category X is unreadable. Fast, total, verifiable. No block overwrite needed. |
| **Kernel compromise** | ACCEPTED RISK. If the kernel is compromised, caps and keys in memory are both exposed. |

### 6.2 The key granularity question

D3 is already per-category (each category has its own file and its own capability). The natural extension is **per-category keys**: one 256-bit AES-GCM key per category, stored in `.arthur/memory/<category>.key`. This makes D4 and D5 fall out cleanly:

- **D4 payoff:** burning `health.key` cryptographically erases all health memories. No block overwrite, no flash-reliability concerns.
- **D5 mechanism:** "forget about X" = burn X's key. Total wipe = burn all keys. One operation, one guarantee.
- **D3 alignment:** the same category boundary that governs capability delegation also governs key custody and erasure granularity.

### 6.3 Options

| Option | Description | Properties |
|--------|-------------|------------|
| **A: Per-category AES-256-GCM keys** | Each category file (`health.mem`) has its own key (`health.key`). Key generated from CSPRNG at category creation. Key stored on-disk, cap-protected. Burn key to erase category. | Per-category crypto-erase is free. D3/D4/D5 share the same boundary. Key management is simple: one key per category file. |
| **B: Single master key** | One key for all memory files. Total wipe is one key burn. Selective erasure requires block overwrite (unreliable on flash). | Simpler key store. But selective erasure falls back to block overwrite — see §7.3 below. |
| **C: Defer encryption (plaintext at rest)** | Memory files are plaintext. Rely on capability isolation. Add encryption later. | Minimal now. Loses the crypto-erase mechanism entirely. D5 becomes block-only. |

### 6.4 Tradeoffs

**Option B (single master key):** Collapses the erasure granularity to all-or-nothing. "Forget about my health" requires decrypting `health.mem`, filtering records, re-encrypting the remainder — or relying on block overwrite (unreliable on SSD/flash per NIST SP 800-88 §2.3). Neither is clean. **Rejected.**

**Option C (defer):** Loses the crypto-erase mechanism that makes D5 real and verifiable. Without encryption, forgetting is file deletion + best-effort overwrite — which is unreliable on flash. The privacy promise (Article 23.2) is weaker without crypto-erase. **Rejected.**

**Option A (per-category keys):** D3's per-category boundary extends naturally to key custody. Each category has its own file, its own cap, and its own key. Burning the key is instant, total, and verifiable erasure for that category. The key management surface is small: one 32-byte key per category (4–10 categories for a real user). Keys are stored as raw blobs in `.arthur/memory/<category>.key`, protected by the directory capability (only Arthur holds the cap to create/read them).

### 6.5 Recommendation: A — Per-category AES-256-GCM keys

**The construction:**
- When Arthur creates a new category file (e.g., `health.mem`), it generates a **256-bit key** via the CSPRNG and stores it in `health.key` (raw 32-byte blob, same directory).
- Each `.mem` file is encrypted with AES-256-GCM using its category's key. The nonce is 96-bit, generated from the CSPRNG per-file-write.
- Arthur decrypts on read, encrypts on write. Crypto overhead for KB-scale files is negligible (sub-millisecond).
- Keys are cap-protected: only Arthur holds the directory capability to read `.key` files. A granted USE-only cap on `health.mem` lets a tool read the *ciphertext* — but without the key, the ciphertext is useless. (In practice, Arthur grants the *decrypted* content over IPC, not the raw file.)

**Honest statement of what this protects against:**
- **Runtime defense-in-depth:** A process with a USE-only cap on `health.mem` gets ciphertext without the key. This adds a layer above the capability boundary — if a cap leak occurs (e.g., a bug in the IPC grant path), the data is still encrypted. The primary boundary remains the capability; encryption is defense-in-depth, not the sole guard.
- **Crypto-erase enablement:** Burning `health.key` makes all health memories cryptographically inaccessible. This is the primary D4 payoff and the mechanism D5 relies on.
- **NOT stolen-disk confidentiality:** Keys and ciphertext are on the same disk. A disk imager recovers both. Stolen-disk protection requires off-disk key custody (passphrase-KDF via Argon2, or TPM-sealed key) — banked as post-ratification hardening. The PL is deciding whether stolen-disk confidentiality is a day-one goal; the default is no.

**Dependency:** `aes-gcm` crate (RustCrypto). Same family as the WiFi lane's `ccm`/`aes` deps. Article 15.7 compliant — vetted RustCrypto primitive, not hand-rolled crypto. The dep is `arthur-memory`-gated, default-off.

**CSPRNG dependency:** Key generation requires `fill_random()` or `SYS_GETRANDOM`. `fill_random()` is kernel-internal (10.1). Arthur is userland — it needs **10.2 (`SYS_GETRANDOM`, OT-2)** to generate keys. **Slice M2 is blocked on 10.2.** See §10.1 for sequencing.

---

## 7. Design space: D5 — Real and verifiable forgetting

### 7.1 The privacy promise

Article 23.2: "Inference happens on the user's hardware on the user's data and stays there." The corollary: **when the user says "forget," it must be real and verifiable.** Not "we'll try to delete it next time we compact." Not "it's marked as deleted but still on disk." Real erasure.

This is the Article 23.2 analogue of GDPR's Article 17 (right to erasure) — but stronger, because on-device means there's no distributed system to chase. One device, one store, one erase.

### 7.2 Options

| Option | Description | Properties |
|--------|-------------|------------|
| **A: Per-category key burn (crypto-erase)** | Burn the key for category X. All memories in that category become cryptographically inaccessible. Fast, total, verifiable. Works per-category thanks to D4's per-category keys. | Selective erasure by category. Instant. No block I/O. NIST SP 800-88 §2.3: crypto-erase is the mandated method for flash/SSD (overwrite is unreliable due to wear-leveling, over-provisioning, and controller remapping). |
| **B: File unlink + block overwrite** | Unlink the file, overwrite freed blocks with CSPRNG bytes. | **Unreliable on SSD/flash.** Wear-leveling means the FTL may have copied the block elsewhere; overwriting the logical address doesn't touch all physical copies. NIST SP 800-88 §2.3 explicitly states Clear (overwrite) is ineffective on flash. Best-effort belt-and-suspenders at best. |
| **C: Soft-delete (tombstones)** | Mark records as deleted but don't remove them. Periodic compaction removes tombstones. | Fast "delete." But: data is still on disk until compaction. Not verifiable erasure. Violates the privacy promise — the data exists until an uncertain future cleanup. **Rejected.** |

### 7.3 Recommendation: A primary, B best-effort secondary — Per-category key burn is the erasure mechanism

**One operation, two guarantees:**

**"Arthur, forget about my health" (selective erasure — the common case):**
1. Arthur overwrites `health.key` with CSPRNG bytes (`fill_random(32)` → write).
2. Arthur unlinks `health.key`.
3. `health.mem` is now cryptographically inaccessible. GCM authentication fails on any read attempt.
4. Arthur unlinks `health.mem` (cleanup — the data was already unreadable from step 1).
5. **Best-effort:** Arthur overwrites `health.mem` with CSPRNG bytes before unlinking (belt-and-suspenders). On SSD/flash this is unreliable (wear-leveling may preserve copies in over-provisioned blocks), but on HDD or for the logical block it provides an extra layer. **The erasure guarantee comes from the key burn, not the overwrite.**
6. **Verifiable:** Read back `health.key` → ENOENT. Read back `health.mem` → GCM auth failure (or ENOENT after unlink). The memory is gone.

**"Arthur, forget everything about me" (total erasure):**
1. Arthur iterates all `<category>.key` files in `.arthur/memory/`.
2. For each: overwrite with CSPRNG bytes, unlink.
3. All categories are cryptographically inaccessible simultaneously.
4. Arthur unlinks all `.mem` files (cleanup).
5. **Verifiable:** No `.key` files remain. Every `.mem` file fails GCM auth (or is unlinked). The memory is gone.

**Why key burn, not overwrite, is primary:** NIST SP 800-88 Rev.1 §2.3 states that Clear methods (overwrite) are ineffective on solid-state storage due to Flash Translation Layer wear-leveling, over-provisioning, and controller remapping. The same bytes may exist in multiple physical locations unknown to the OS. Cryptographic erasure (Purge in NIST terminology) is the mandated method for flash media. Burning the key makes every copy — known or unknown — unreadable simultaneously. **This is why per-category keys (D4) and per-category erasure (D5) are the same design decision.**

**Why overwrite is still done (best-effort secondary):** On non-flash media (HDD), overwrite is effective. Even on flash, overwriting the logical block handles the case where a future reader can access the FTL directly but doesn't have the key. It costs nothing and adds a layer. But the **verifiable guarantee** comes from the key burn alone.

---

## 8. Design space: D6 — Fail-safe semantics

### 8.1 The requirement

Memory must fail safe: **corrupt or missing memory ⇒ Arthur degrades to today's ephemeral window, never crashes, never leaks the wrong user's memory.** The kill-switch: memory-substrate-off ≡ today's ephemeral Arthur, bit-for-bit.

### 8.2 Failure modes and responses

| Failure | Detection | Response |
|---------|-----------|----------|
| **Memory directory missing** | `open(.arthur/memory/)` → ENOENT | Arthur degrades to ephemeral mode. Logs "memory: no directory, ephemeral mode." No crash. No data loss (there was nothing to lose). |
| **Memory directory unreadable (cap revoked)** | `open()` → EACCES | Same as missing. The capability was revoked — Arthur respects it and degrades. |
| **Individual file corrupt (bad GCM tag)** | AES-GCM decrypt → auth failure | Skip the file. Log "memory: {file} corrupt, skipped." Other files unaffected. Arthur continues with remaining memories + ephemeral window. |
| **Individual file corrupt (bad record format)** | Parse fails on a line (bad version, bad type, truncated) | Skip the corrupt line. Log "memory: {file}:{line} malformed, skipped." Continue parsing remaining lines. |
| **Per-category key missing** | `open(<category>.key)` → ENOENT | Skip that category. Other categories unaffected. Log "memory: {category}.key missing, category skipped." |
| **Per-category key corrupt (wrong length, bad bytes)** | Key is not 32 bytes or GCM auth fails on first read | Same as missing key for that category. Skip the category, continue with others. |
| **NeoFS full (cannot write new memories)** | `write()` → ENOSPC | Arthur stops writing new memories. Continues reading existing ones + ephemeral window. Logs "memory: storage full, write suspended." Resumes when space is available. |
| **Arthur process restarts** | Normal process lifecycle | Reopens memory directory, re-reads memories into the sliding window. No data loss (memories are durable on NeoFS). |

### 8.3 The kill-switch shape

**Feature flag: `arthur-memory`** (default: off).

When `arthur-memory` is **off** (default):
- The memory module does not compile into Arthur
- Arthur uses the ephemeral sliding window (OD-6, today's behavior)
- No NeoFS memory directory is created
- No key file exists
- Arthur is bit-for-bit identical to today

When `arthur-memory` is **on**:
- Arthur creates `.arthur/memory/` on first boot (if it doesn't exist)
- Arthur generates the encryption key (if it doesn't exist)
- Arthur reads memories into the sliding window at startup
- Arthur writes new memories to NeoFS during conversation
- If any failure occurs, Arthur degrades to ephemeral mode (the off state)

**The invariant:** `arthur-memory` off ≡ ephemeral Arthur. Always. The memory substrate is strictly additive. Turning it off restores today's behavior. There is no "broken middle state."

---

## 9. Prior art — raw-sourced, design-level only

### 9.1 On-device assistant memory patterns

**Apple Shortcuts / Siri memories:** Apple stores user context in an on-device SQLite database (`Shortcuts.sqlite`), encrypted with the device key (File Protection class `completeUntilFirstUserAuthentication`). The model (Siri) reads from this store at query time. **Design insight:** per-domain encryption keys enable selective erasure. We use per-category keys (one key per category file) — same principle, flatter structure.

**Google Assistant on-device profile:** Android's `ProfileData` stores user preferences in encrypted SharedPreferences. Deletion is immediate (SharedPreferences commit + SecureElement wipe for biometric-bound keys). **Design insight:** immediate deletion over deferred compaction — matches our D5 choice.

**Mycelial memory architectures (research-space):** Several agent frameworks (MemGPT, Letta) use vector-store-backed memory with explicit "core memory" vs "archival memory" separation. Core memory is small, always-in-context; archival is large, retrieved by embedding similarity. **Design insight:** the core/archival split is a retrieval strategy concern (model layer), not a substrate concern. Our substrate stores records; the model decides what to load into context. The seam is clean.

### 9.2 Right-to-be-forgotten implementations

**GDPR Article 17 erasure patterns:** Cryptographic erasure (key burn) is the industry-standard approach for encrypted-at-rest data. NIST SP 800-88 Rev.1 ("Guidelines for Media Sanitization") defines three methods: Clear (overwrite), Purge (crypto erase), Destroy (physical). **NIST §2.3 mandates Purge (crypto-erase) for flash/SSD** — Clear is ineffective due to wear-leveling and FTL remapping. Our D5 uses Purge (per-category key burn) as the primary mechanism for both selective and total erasure, matching NIST's flash guidance. Best-effort Clear (block overwrite) is secondary belt-and-suspenders.

**Signal's message deletion:** Signal overwrites deleted message blocks with random bytes before freeing them. Our design uses overwrite only as best-effort secondary — the primary guarantee comes from key burn, which is correct for flash media where Signal's overwrite-only approach would leave FTL copies.

---

## 10. D1–D6 ratification block

| Decision | Recommendation | Rationale in one line |
|----------|----------------|----------------------|
| **D1: Where memories live** | **NeoFS objects under a capability** (`Handle::FileDir` for `.arthur/memory/`) | Durable, capability-gated, no new kernel surface. NeoFS is Phase-3-proven. |
| **D2: Memory object model** | **Typed records** (`version:type:timestamp:content`, line-delimited, per-category files) | Minimal-sufficient structure for verifiable forgetting. Human-readable. Forward-compatible via version field. Four types: fact, preference, event, relationship. |
| **D3: Capability/privacy model** | **Per-category caps, read-only by default.** Arthur holds parent cap (USE+WRITE+DERIVE+REVOKE). Derives USE-only for grantees. Revokes through derivation graph. | Uses built primitives exactly as designed. Category isolation bounds blast radius. Arthur remains sovereign. OT-18 boundary: grant ergonomics deferred. OT-20 seam: composes with multi-Arthur delegation. |
| **D4: Encryption at rest** | **Per-category AES-256-GCM keys.** Each category has its own key (`<category>.key`, 32-byte CSPRNG blob, cap-protected). Honest payoff: runtime defense-in-depth + crypto-erase enablement. **NOT stolen-disk confidentiality** (keys and ciphertext share the same disk; caps are runtime-only). Stolen-disk protection requires off-disk key custody (passphrase-KDF or TPM-seal) — banked as hardening. `aes-gcm` RustCrypto dep, `arthur-memory`-gated. **Blocked on 10.2 (`SYS_GETRANDOM`, OT-2) for userland key generation.** | Per-category keys align D3/D4/D5 on the same boundary. Crypto-erase is the primary payoff. Negligible overhead for KB-scale files. |
| **D5: Forgetting** | **Per-category key burn (crypto-erase) is PRIMARY.** Burn `<category>.key` → category unreadable. Total wipe = burn all keys. Best-effort block overwrite secondary (belt-and-suspenders, unreliable on flash per NIST SP 800-88 §2.3 — the guarantee comes from the key burn). Verifiable: read back key → ENOENT; read back ciphertext → GCM auth failure. | Real erasure, not deferred compaction. NIST SP 800-88 Purge method for flash. Per-category keys (D4) make selective crypto-erase free. |
| **D6: Fail-safe** | **Feature `arthur-memory` default-off ≡ ephemeral Arthur.** Missing dir/corrupt file/missing key → degrade to ephemeral mode. Per-line skip on malformed records. Per-file skip on GCM auth failure. Never crash, never leak. | Strictly additive. Kill-switch off restores today bit-for-bit. Every failure mode degrades to the known-good floor (ephemeral OD-6 window). |

### 10.1 Dependencies and sequencing

**Requires (must exist before build):**
- NeoFS (Phase 3, complete)
- Capability primitives: `derive_handle`, `revoke_handle`, `ipc::grant` (Phase 2, silicon-proven)
- Arthur Slices 1-4 (landed)

**Cross-pillar dependency:**
- **M2 requires 10.2 (`SYS_GETRANDOM`, OT-2):** Arthur is userland and cannot call kernel-internal `fill_random()`. It needs `SYS_GETRANDOM` (or a libneo wrapper) to generate per-category encryption keys. 10.1 (`fill_random`) is kernel-only. **M1, M3, M4 are independent of 10.2.**

**Build sequencing (post-ratification, one pillar per OT-22):**
1. **Slice M1:** Memory directory + typed record format + ephemeral fallback. Arthur can write and read memories. Plaintext (no encryption). Kill-switch: `arthur-memory` off ≡ ephemeral. **No 10.2 dependency.**
2. **Slice M2:** Per-category AES-256-GCM encryption + key management. Each category gets its own key (`<category>.key`). Transparent to Arthur's read/write path. Kill-switch: feature off → plaintext (Slice M1 behavior). **BLOCKED on 10.2 (`SYS_GETRANDOM`, OT-2) — userland key generation requires the ambient CSPRNG syscall.**
3. **Slice M3:** Per-category capability delegation + revocation. Arthur can derive USE-only caps and grant them. Kill-switch: no delegation → Arthur-only access (D3 Option A, which is safe). **No 10.2 dependency.**
4. **Slice M4:** Forgetting operations — per-category key burn (primary) + best-effort block overwrite (secondary). Total wipe = burn all keys. Kill-switch: forgetting unavailable → memories persist until manual deletion. **Requires M2 (keys must exist to burn them).**

**Each slice is independently kill-switchable and degrades to the previous slice's behavior when off.**

### 10.2 Honest uncertainties

| Uncertainty | Status | Resolution path |
|-------------|--------|-----------------|
| Memory volume growth — how much memory does a real user accumulate? | Unknown | N=1 measurement. Start small, measure, adapt. The schema scales (more files, more lines). |
| Category taxonomy — are four types enough? | Tentative | The version field permits evolution. Add types in v2 if needed. |
| Encryption key derivation — CSPRNG-generated vs user-secret-derived? | CSPRNG now; user-secret is a hardening follow-on | A user-secret-derived key (e.g., from login password via Argon2) would provide stolen-disk confidentiality. But it requires a login system (unbuilt). CSPRNG key + cap protection is the N=1 floor. Off-disk key custody (passphrase-KDF or TPM-seal) is banked as post-ratification hardening — the PL is deciding whether stolen-disk confidentiality is a day-one goal. |
| NeoFS block overwrite API — does NeoFS expose freed block ranges? | Unknown (low priority) | Overwrite is best-effort secondary; the erasure guarantee comes from key burn. If NeoFS doesn't expose physical ranges, fall back to: write randomness to the file, then unlink. The file's logical blocks will contain randomness when freed. On flash this is unreliable — but the key burn already made the data unreadable. |

---

## 11. Risk register

| Risk | Impact | Mitigation |
|------|--------|------------|
| Memory directory corrupt (NeoFS block corruption) | Medium — Arthur loses memories | Per-file GCM auth detects corruption. Corrupt files are skipped, not crashed on. Remaining memories + ephemeral window continue. |
| Per-category key lost (one `.key` file deleted/corrupt) | Medium — one category unreadable | Skip that category on load. Log "memory: {category}.key missing, category skipped." Other categories unaffected. Degrades gracefully — one category lost, not all. |
| AES-GCM dependency adds compile time / binary size | Low — one RustCrypto crate | `arthur-memory`-gated, default-off. Only compiled when the feature is enabled. Same family as WiFi's crypto deps (proven in-tree). |
| Per-category caps create too many handles | Low — N=1 Arthur, few categories | The capability table scales to thousands of handles. Four-to-ten category files is trivial. If it becomes a problem, derive one cap for the directory with USE-only (coarser granularity). |
| User says "forget" but Arthur misidentifies the category | Medium — wrong memories deleted | This is a model-layer concern (retrieval accuracy), not a substrate concern. The substrate's job is to delete what it's told to delete, verifiably. The model's job is to tell it the right thing. |

---

**Status:** Design THINK complete. D1–D6 stated for PM review → PL ratification. No code. No CI gate. OT-17 remains OPEN until ratified.
