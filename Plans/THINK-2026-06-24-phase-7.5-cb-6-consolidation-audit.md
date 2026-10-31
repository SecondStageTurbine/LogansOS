<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Phase-7.5 CB-6 consolidation audit — the close-audit, runner-free prep

**Date:** 2026-06-24
**From:** Prometheus (research / consolidation-support)
**Branch:** `prometheus/cb-6-consolidation-prep` off `origin/main` at `8396710`
**Scope:** Host-only THINK — no kernel code changes, no QEMU. The output is four lists + a fresh-reader legibility pass.

---

## 0. One-line

Runner-free CB-6 groundwork: enumerate the clippy-debt with pay-or-bank per cluster, inventory dead scaffolding (excluding gate discriminators), verdict on the invariant-ledger page-fit, and the OPEN-THREADS prune list.

---

## 1. ISC-1 — Clippy-debt inventory + pay-or-bank recommendation

**Source:** `cargo clippy -p neoplutarch-kernel --all-targets --features mt7601u` — **33 warnings total**, matching CLAUDE.md's "banked ~33-warning mt7601u clippy-debt" exactly. All are pre-existing; none are new to this branch.

### Cluster A — `mt7601u` driver (22 warnings)

| # | Warning | Location | Pay-or-bank | Rationale |
|---|---------|----------|-------------|-----------|
| A1 | `doc_lazy_continuation` ×3 (module doc indentation) | `mt7601u/mod.rs:5-7` | **PAY** | Trivial 2-char fixes, zero blast radius |
| A2 | `manual_range_contains` | `mt7601u/phy.rs:386` | **PAY** | One-line idiom fix (`!(1..=14).contains(&ch)`) |
| A3 | `manual_range_contains` | `mt7601u/mod.rs:2849` | **PAY** | One-line idiom fix |
| A4 | `collapsible_if` ×10 | `mt7601u/mod.rs:385,450,467,504,505,506,1568,1569,1680` + one in the scan loop | **PAY** | Mechanical `&& let` collapses; 19 of 33 clippy warnings are auto-fixable and these dominate. Reduces nesting, improves readability for the ch6+ wedge fix that follows CB-6. |
| A5 | `unused_variables` + `unused_assignments` (`wedge_seen`) | `mt7601u/mod.rs:1658,1714` | **PAY** | Dead variable from the ch6+ wedge discriminator — the variable is assigned but never read. Remove or prefix `_`. |
| A6 | `result_large_err` (`ClaimedDevice` tuple) | `mt7601u/mod.rs:2943` | **BANK** | Requires boxing the `(ClaimedDevice, &'static str)` — a structural change to the error type. Not worth a CB-6-only refactoring when CB-4's xHCI endpoint-reset will touch this surface anyway. Note as CB-4 follow-on. |

### Cluster B — `cdc_ecm` / USB core (2 warnings)

| # | Warning | Location | Pay-or-bank | Rationale |
|---|---------|----------|-------------|-----------|
| B1 | `unused_import: UsbInterface` | `usb/mod.rs:47` | **PAY** | One-line removal — the re-export is unused downstream |
| B2 | `result_large_err` (`ClaimedDevice`) | `usb/mod.rs:120` | **BANK** | Same as A6 — structural error-type change. CB-4 follow-on. |

### Cluster C — `rng` / CSPRNG (9 warnings)

| # | Warning | Location | Pay-or-bank | Rationale |
|---|---------|----------|-------------|-----------|
| C1 | `dead_code: next_u64, next_u32` | `rng/drbg.rs:66,73` | **BANK** | Intentionally retained API surface — a future consumer (e.g., per-packet IV generation) may need them. Not dead scaffolding; the ChaCha20 DRBG is production-critical and these are convenience wrappers over `fill_random`. |
| C2 | `dead_code: KatResult, run_all_kats, kat_* ×3` | `rng/kat.rs:19-106` | **BANK** | The KAT functions ARE the I-12 byte-oracle for the CSPRNG. They're dead at compile-time because the KAT runs via the self-test path (`csprng_selftest` scenario), not direct calls. Removing them would delete the oracle. |
| C3 | `needless_borrows_for_generic_args` | `rng/drbg.rs:105` | **PAY** | One-line cleanup |
| C4 | `needless_borrows_for_generic_args` ×2 | `rng/kat.rs:115,116` | **PAY** | One-line cleanups |
| C5 | `useless_conversion` ×2 | `rng/kat.rs:120,130` | **PAY** | One-line cleanups |

### Cluster D — cdc_ecm error type (1 warning)

| # | Warning | Location | Pay-or-bank | Rationale |
|---|---------|----------|-------------|-----------|
| D1 | `result_large_err` (`ClaimedDevice` tuple) | `cdc_ecm.rs:404` | **BANK** | Same structural issue as A6/B2 — three functions share the same oversized error type. Fix all three together in CB-4 or a dedicated USB-error refactoring. |

### Summary table

| Cluster | Total | PAY | BANK | Auto-fixable (clippy --fix) |
|---------|-------|-----|------|----------------------------|
| A: mt7601u | 22 | 20 | 2 | 19 of the collapsible_if/doc_lazy/manual_range are auto-fixable |
| B: USB core | 2 | 1 | 1 | — |
| C: rng/CSPRNG | 9 | 5 | 4 | All 5 PAY items are auto-fixable |
| D: cdc_ecm | 1 | 0 | 1 | — |
| **Total** | **33** | **26** | **7** | **~24 of 26 PAY** |

**Recommendation:** CB-6 should `cargo clippy --fix` the auto-fixable cluster (A4/A1/A2/A3 + C3/C4/C5), then manually fix A5 (wedge_seen) + B1 (UsbInterface). The 7 BANKED items are structurally non-trivial or intentionally retained — note them in the close report.

**CB-6 line-item:** If CB-6's coder runs `cargo clippy --fix --bin kernel -p neoplutarch-kernel --features mt7601u`, then manually addresses A5 + B1, the mt7601u-config clippy surface should be clean. The 3 `result_large_err` items (A6/B2/D1) are a CB-4 follow-on; the rng dead-code (C1/C2) is intentional API/oracle surface.

---

## 2. ISC-2 — Dead-scaffolding inventory

**Scope:** Superseded fixtures, one-off helpers, obsolete scaffolding. **Excludes gate discriminators** (Rule 5 — they stay armed permanently).

### Confirmed safe to remove (4 items)

| # | Location | What | Why it's dead | Risk |
|---|----------|------|---------------|------|
| S1 | `loader/elf.rs:39` | `const PF_R: u32 = 4` | ELF program-header read-flag constant. The loader only checks W+X for W^X enforcement. Never referenced downstream. Bare `#[allow(dead_code)]`, no Rule-5 label. | Trivial — one constant, zero callers |
| S2 | `arch/mod.rs:54` | `pub fn interrupts_enabled() -> bool` | Wrapper that calls `x86_64::interrupts_enabled()`. Never invoked via the `arch::` path — all callers use `x86_64::` directly. Bare `#[allow(dead_code)]`, no Rule-5 label. The sibling `disable_interrupts()` has a Phase-1.2 comment but is also unused; both are multi-arch forward scaffolding. | Low — remove or keep as a pair for future aarch64. Given the phase-7.5 consolidation scope, removing S2 alone is clean. |
| S3 | `drivers/usb/cdc_ecm.rs:838` | `pub fn debug_pipe_drops() -> u64` | Feature-gated (`neo-test`). Has `#[allow(dead_code)] // gate seam` but **no test scenario consumes it**. The sibling cdc_ecm debug helpers (`debug_bulk_in_route`, `debug_ax88179_diag`) ARE consumed. This one is not. | Low — no scenario references it. If a future USB-storm scenario needs pipe-drop counting, it's trivial to re-add. |
| S4 | `drivers/net/virtio_net.rs:325-335` | `debug_used_ring_acquire_barriers()` + `debug_primary_device()` | Both feature-gated (`neo-test`). Neither is called from any test scenario. The sibling virtio_net debug functions (`debug_tx_submissions`, `debug_rx_completions`, etc.) are actively consumed in 20+ places. | Low — no consumer exists. `debug_primary_device` is the virtio_net analog of cdc_ecm's (which IS used). |

### Not dead — keep (with reason)

| Item | Location | Why it stays |
|------|----------|--------------|
| `arch/mod.rs:47` `disable_interrupts()` | Same multi-arch scaffolding as S2 | Comment says "first caller lands in Phase 1.2" — paired with S2. Keep the pair or remove both. Keeping is lower-risk for a consolidation pass. |
| `fs/mod.rs:419` `MountEntry.flags` | Written at mount, never read back | Part of live syscall ABI (`sys_mount` accepts flags). Removing changes the contract. Forward scaffolding for flag enforcement. |

---

## 3. ISC-3 — Invariant-ledger page-fit verdict

### Current state

`docs/complexity-governance.md` §1 carries **I-1 through I-15** (15 rows). The ADR-062 update (line ~17) states: *"the literal Markdown section exceeds a printed page because I-14/I-15 carry long guard prose."* The most recent consolidation re-verification (ADR-067 lane, 2026-06-22) confirms: **"Page-fit: the alarm remains RAISED."**

### Root cause analysis

The overflow is concentrated in two rows:

| Row | Issue | Prose length |
|-----|-------|--------------|
| **I-14** (Subsystem fault containment) | The `Guarded by` column contains a full proof narrative: three vectors (a)/(b)/(c), each with silicon evidence, BDF addresses, DMAR fault iova values, and stage-A/B verdicts. This is ~40 lines of guard prose in one cell. | ~3,500 characters |
| **I-15** (File handles as capabilities) | The `Stated where` column contains the full kill-switch description + mapping details. The `Guarded by` column lists 5+ distinct guards with test counts. | ~1,200 characters |

The remaining I-1 through I-13 are concise — each fits in 2-4 lines. They're not the problem.

### Recommended trims (specific, actionable)

**Trim 1 — Move I-14(c) silicon evidence to ADR-045/ADR-050.** The guard cell for I-14 contains full proof narratives with BDF addresses, iova values, and stage verdicts. These belong in the ADRs (where they already live). The ledger row should keep only: *"fenced DMA arena (ADR-045/c1) + VT-d/IOMMU fence domains (ADR-050/c2); both silicon-proven; see ADRs for evidence."* That's ~80 characters vs ~3,500.

**Trim 2 — Move I-14(a)/(b) proof detail to ADR-039.** The lock-isolation and liveness-degradation guards are referenced in ADR-039/ANSPEC §9.12. The ledger row should keep the mechanism name + "see ADR-039" rather than the full degradation narrative.

**Trim 3 — Condense I-15 guard column.** Replace the enumeration of 5+ individual guards with *"pure `FileRights↔CapRights` mapping (host KATs, 15 tests); subset-enforcement assertion; QEMU `file_cap_derive_grant` no-bypass at SMP=4; see ADR-061."*

**Trim 4 — Move the I-14 "Three vectors" framing to the Stated-where column.** The `(a) lock isolation / (b) liveness degradation / (c) memory isolation` taxonomy is architectural context, not a guard mechanism. It belongs in the `Stated where` reference, not bloating the `Guarded by` cell.

**Estimated savings:** ~4,200 characters removed from the table cells, replaced with ADR cross-references. This should bring §1 back within one printed page for the I-1..I-15 table proper.

### Verdict

**CB-6 MUST re-fit the ledger before adding any I-16.** The alarm has been raised since ADR-062 and remains active. Per Rule 2/§3.1: *"compress row prose or split evidence detail out so the one-page alarm remains enforceable."* The trims above are pure prose moves — no behavioral change, no re-verification needed (the guards don't change, only where the evidence is recorded).

**CB-6 line-item:** Apply Trims 1-4 to `complexity-governance.md` §1. Then verify the table fits one page. Flag the ADR-062 "Page-fit flag" text as resolved.

---

## 4. ISC-4 — OPEN-THREADS prune list

**Source:** `docs/OPEN-THREADS.md` on `origin/main`. The register's own maintenance rule: *"RESOLVED (kept briefly with closing evidence, then removed at next audit)."*

### RESOLVED OTs to prune at CB-6 close

| OT | Item | Closing evidence | Safe to prune? |
|----|------|------------------|----------------|
| **OT-2** | `gen_snonce` → CSPRNG | CLOSED 2026-06-21 (`d1c8bb6`); KAT + push-CI `27910976427` + QEMU `27911002255` green | **YES** — evidence is on `main`, CI URLs are permanent |
| **OT-30** | Forward-path performance | CLOSED 2026-06-22 (`e517a52`); F4 merged `546ef3d`; QEMU `28002712628` green; 1435 ms/token, top-1 parity | **YES** — evidence is on `main`, commit + CI URLs are permanent |
| **OT-37** | Stage-B degenerate reply | RESOLVED 2026-06-24 (`d801f70`); fix `2a4843c`; 4-shard QEMU `28099858869` green; Arthur replies coherently | **YES** — evidence is on `main`, commit + CI URLs are permanent |
| **OT-29** | Kernel thread-slab UB | CLOSED 2026-06-22 (`1c3ee65`); QEMU `27957728489` + push `27957709384` green; ADR-067 + evidence file | **YES** — fully closed with ADR and discriminator armed |

### Already-pruned (in the current file, no action needed)

The file already has a "Pruned at the 2026-06-20 Rule-4 audit" section listing OT-23, OT-24, OT-25, plus an older "Dispositioned at the 2026-06-17 audit" table. These are already marked for removal — CB-6 should **delete these two pruned tables entirely** (the closing evidence is in the git history).

### Flagged: OTs that say CLOSED/RESOLVED but need verification

| OT | Status text | Concern |
|----|-------------|---------|
| **OT-27** | Partial items closed (cold self-upload CLOSED, ch6+ wedge NEW) | This is a multi-item OT — only the (a) sub-item is closed. The OT itself stays open for (b)/(c)/(d). Do NOT prune; update the status text to reflect (a) done, (b/c/d) open. |
| **OT-18** | E4 RESOLVED inline, but the OT itself is still active for E3-revoke/E5 | The E4 user-gate was DROPPED by PL ruling. The OT stays open. Do NOT prune. |

### Prune action for CB-6

Remove **OT-2, OT-29, OT-30, OT-37** from the active table. Preserve each closing evidence (commit SHA + CI URLs) in the commit message. Delete the two "Pruned at..." and "Dispositioned at..." historical tables — their content is in git history. The maintenance rule says RESOLVED items are *"kept briefly with closing evidence, then removed at next audit"* — CB-6 IS that next audit.

---

## 5. ISC-5 — Fresh-reader legibility pass

What would confuse a fresh reader of the close tree:

1. **CLAUDE.md Genesis row** — The off-boarded Genesis seat (GLM-5.2) still occupies a full paragraph with historical A/B details. A fresh reader might think it's an active coder. Add "OFF-BOARDED" to the bullet or collapse to one line.

2. **complexity-governance.md audit log sprawl** — The audit log at the bottom of §1 is now ~4,000 words spanning 20+ builds. A fresh reader looking for the invariant ledger scrolls past a wall of prose before finding the table. Consider moving the audit log to a separate `docs/ledger-audit-log.md` or an ADR appendix.

3. **OPEN-THREADS.md OT-12 history bloat** — OT-12 (brain last-mile) contains the entire Phase-8 brain build history inline: L1/L2/F1/F2/F3 status, OT-30 resolution, Stage-A/B progress. This is ~600 words of narrative in one table cell. A reader scanning for "what's open" has to parse through a build chronicle. The F2/F3 merges and OT-30 close should be summarized in 1-2 lines; the detail belongs in the respective ADRs.

4. **Stale cross-refs (Janus Charter C items):**
   - `docs/ADR-057` still says "Stage-B pending" after W2 association landed (`4f82155`)
   - `README.md` still phrases OT-2 as future work (it's closed at `d1c8bb6`)
   - `docs/bible.md` Art. 9.10 firmware-bundled wording vs ADR-054's staged-blob-only rule

5. **The "two-worktree" section in CLAUDE.md** — The STATUS note says the PM worktree is now OPTIONAL (cloud PM), but the full two-worktree dance is still documented at length. A fresh reader following the instructions would create a worktree they don't need. Add a clearer "SKIP THIS IF USING CLOUD PM" callout.

---

## 6. Summary — the four lists for CB-6

### List 1: Clippy-debt pay-or-bank
**26 PAY, 7 BANK.** Auto-fixable via `cargo clippy --fix` for ~24 items. Manual fixes: A5 (`wedge_seen`), B1 (`UsbInterface`). Banked: 3× `result_large_err` (CB-4 follow-on), rng KAT surface (intentional oracle).

### List 2: Dead scaffolding
**4 items safe to remove:** S1 (`PF_R` constant), S2 (`interrupts_enabled()` wrapper), S3 (`debug_pipe_drops()`), S4 (virtio_net debug pair). Zero gate discriminators touched.

### List 3: Ledger page-fit
**Alarm RAISED, trims specified.** Move I-14 silicon evidence to ADRs (Trims 1-2), condense I-15 guard column (Trim 3), move vector taxonomy to Stated-where (Trim 4). Estimated ~4,200 char savings. Must do before any I-16.

### List 4: OT prune list
**4 OTs to remove:** OT-2, OT-29, OT-30, OT-37. Plus delete the two historical "Pruned at..." tables. Closing evidence preserved in commit message.

---

*Host-only deliverable. No code changes, no QEMU gate. `cargo fmt` clean (no source files modified). SPDX headers present on this file.*
