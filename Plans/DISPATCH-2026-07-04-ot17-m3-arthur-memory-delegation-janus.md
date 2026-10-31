<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-17 M3 — Arthur memory capability delegation (unblocked by truncate) → Janus

**Date:** 2026-07-04
**From:** PM (Claude)
**To:** Janus (main-tier — built A1/A2/A3 + the NeoFS truncate primitive that unblocks this). New branch off `origin/main` HEAD (`78c5455`): `codex/ot17-m3-memory-delegation-janus`.
**Status:** DISPATCHED — the **next slice of the ratified OT-17 trilogy** (Arthur's persistent-memory substrate). M1 (dir+format+fallback, `2fe9a6e`) and M2 (per-category ChaCha20-Poly1305 encryption, `8ef7f89`/ADR-065) landed; **M3 = capability delegation**, which was flagged *"needs a NeoFS truncate primitive"* — and truncate just landed (`add51e5`). **Non-conflicting** with Oracle (forward/SMP) and Jarvis (observability). **Kernel-touching ⇒ 4-shard QEMU + AAG.**

## 0. What this build is
Arthur's durable personal memory (relationships/history/context) lives as NeoFS objects under a capability, encrypted per-category (M1/M2). **M3 delegates a scoped memory capability** — so a specific memory category can be granted/derived to a consumer with **least privilege** (read-only or read-write on one category), through the **existing I-15 file-cap graph** (child ⊆ parent), and revocably. This is the OT-17 D-series design (D1–D6, ratified) realized for delegation; the truncate primitive is needed because rewriting/compacting an encrypted memory object on delegation requires shrinking the backing file cleanly.

## 1. References
```
git fetch origin
git show origin/main:docs/OPEN-THREADS.md                                # OT-17 (D1-D6 ratified; M1+M2 landed; M3 = cap delegation) + OT-18 (the grant graph M3 rides)
git show origin/main:THINK-2026-06-18-arthur-memory-substrate-design.md  # the ratified design (or docs/ path)
git show origin/main:kernel/src/fs/mod.rs                                # Inode::truncate (add51e5) — the primitive M3 uses; byte_len
git show origin/main:kernel/src/fs/neofs/inode.rs                        # NeoInode truncate + the M1/M2 memory-object storage
git show origin/main:kernel/src/sched/file_cap_logic.rs                  # the I-15 FileRights↔CapRights mapping M3 delegates through (ADR-061)
# the M1/M2 Arthur-memory code (userland/kernel) — the substrate M3 extends
```

## 2. ISCs
- **ISC-1 — Scoped memory-capability delegation.** Delegate a **per-category** memory capability (OT-17 D3: per-category USE-only caps) through the **unchanged I-15 derive/grant graph** — a delegated cap is a subset of its parent (read-only ⊆ read-write; one category, not all). No new `Handle` kind; rides `Handle::File`/`FileDir` over the memory object. Least-privilege by default (OT-18 D7).
- **ISC-2 — Truncate-backed compaction/rewrite.** Where delegation or a category rewrite requires shrinking the encrypted memory object (M2 is single-blob whole-file-rewrite), use the new **NeoFS truncate** to size the backing file exactly — no leaked blocks, no stale ciphertext tail. Byte-oracle the rewrite (surviving/rewritten bytes exact; the M2 nonce-counter/write-ahead discipline preserved).
- **ISC-3 — Revocation.** A delegated memory cap revokes through the existing `revoke_handle` derivation walk (OT-18 D6) — revoking a delegated category cap cuts that consumer's access without touching the owner's or sibling categories.
- **ISC-4 — Kill-switch + the M2 boundary.** M3 default-off ⇒ **≡ M2 bit-for-bit** (no delegation surface; the memory substrate behaves exactly as M1+M2). Prove the equality. The per-category **key-burn = crypto-erase** (D5) and **fail-safe ≡ ephemeral** (D6) invariants from M2 must still hold — a delegated cap must not survive a key-burn of its category.

## 3. Standing-rule sections
- **Kill-switch (Rule 1):** ISC-4 — M3 off ≡ M2 bit-for-bit.
- **Invariant budget (Rule 2):** preserve the ledger; **no new invariant** — M3 rides **I-15** (file-caps, the delegation substrate) + the existing OT-18 grant graph + the M2 crypto invariants. State "no new invariant" (or justify+guard one if truly earned).
- **Keystone (29.3):** delegation goes through the **unchanged** cap-check; a delegated memory cap is enforced by the kernel, not by a UX record. `grants.json`/audit is not the boundary.
- **Byte-oracle:** ISC-2's truncate-backed rewrite ships a hand-laid fixture (the memory-object bytes after a delegated rewrite + the M2 encryption discipline intact).
- **I-14:** N/A by construction (fs + cap graph; no driver ISR/DMA).
- **Privacy note (this is the most privacy-sensitive data in the system):** a delegation must not widen access beyond the delegated category, and must not leak plaintext across the cap boundary (M2 keeps `FileRights` semantic; the cap-graph meta-rights are what delegate).
- **Scope guard:** M3 delegation + truncate-backed rewrite + revocation ONLY. **Do NOT:** implement M4 (forgetting — the next slice); change the M2 crypto or the `.neom`/forward; touch the agentic-action grammar (A1/A2/A3), Oracle's forward/SMP, or Jarvis's observability; add an invariant/Handle kind.

## 4. Gate + close
- **Kernel-touching ⇒ mandatory 4-shard QEMU `NEO_SMP=4` + AAG.** Scenarios: delegate-read-only → consumer reads OK / writes EACCES; revoke → consumer denied; owner + sibling categories unaffected; truncate-backed rewrite byte-oracle; key-burn kills the delegated cap; kill-switch ≡ M2. Push-CI green (host KATs). **Green without the CI URLs is not done.**
- **Close-gate `scripts/worker-close-gate.sh`** (rebase; re-run; host set; SPDX; self-stat).
- **RETURN** the QEMU shard URLs + SHA, the delegation/revocation failure matrix, the truncate-rewrite byte-oracle, the kill-switch ≡ M2 proof, and the 29.3/no-new-invariant statements. **Disk reclaim** after merge. **On merge, OT-17 advances to M4 (forgetting) as the next slice.**

---
## Relay — what to tell Janus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-04-ot17-m3-memory-delegation-janus.md` (+ OT-17/OT-18, the memory-substrate THINK, `Inode::truncate`, `file_cap_logic.rs`). **Branch:** `codex/ot17-m3-memory-delegation-janus` off `origin/main` HEAD (`78c5455`).
- **One-line:** **Build OT-17 M3 — scoped, revocable delegation of Arthur's per-category memory capability** (your truncate primitive unblocked it). Delegate a per-category memory cap through the **unchanged I-15 graph** (child ⊆ parent, least-privilege), use **NeoFS truncate** for any encrypted-object rewrite (byte-oracle it; keep M2's nonce/write-ahead/key-burn discipline), revoke via the existing `revoke_handle` walk. **Keystone (29.3):** the kernel cap-check enforces it, not a UX record. **Kill-switch** M3 off ≡ M2 bit-for-bit; key-burn must kill a delegated cap; fail-safe ≡ ephemeral. **No M4 (forgetting) yet; no M2-crypto/forward/agentic-grammar change; no new invariant/Handle kind.** 4-shard QEMU + AAG. RETURN shard URLs + SHA + the delegate/revoke matrix + the rewrite byte-oracle + kill-switch≡M2 + the 29.3 statement.
