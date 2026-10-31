<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase-7.5 CB-6 — consolidation + status-repair (the close pass, ahead of the tag) → Janus

**Date:** 2026-06-29
**From:** PM (Claude)
**To:** Janus (main-tier). New branch off `origin/main` HEAD (`4279683`): `codex/cb-6-consolidation-janus`. Author `SecondStageTurbine`.
**PL reassign note:** main-tier + needs the QEMU runner — if Janus is heads-down on the Phase-8 Arthur slice, reassign to whoever's free among **Oracle / Jarvis**. Cerebro is held on the bare-metal gold-standard session; this consolidation must land **before** the final bare-metal close-bar run so the tagged tree == the proven tree.
**Status:** DISPATCHED — the **runner-gated** half of CB-6 (Rule-4 consolidation + status-drift repair + OT prune), kernel-touching ⇒ **mandatory 4-shard QEMU `NEO_SMP=4` + AAG**. The **squash + phase tag** is the PM's final step and is **HELD** (see §5) until the two bare-metal proofs land. This build produces the clean close tree; it does **not** tag.

---

## 0. What this build is (and is not)

CB-6 in the closeout PLAN (`PLAN-2026-06-23-phase-7.5-closeout.md` §1, Janus checklist 8–10) is the phase close: consolidation + status-repair + integrated proof + tag. The technical close-set is **green on `main`**: CB-1 (`3224b34`), CB-1b (`6c1c94f`), CB-prep/T-039 (`3594abe`), CB-3 identity (`9900c51`), CB-2 (`37e91b5`, QEMU-waived per its silicon gate), plus the post-close SMP AP-ready barrier and `byte_len` (`4279683`). The only items still outstanding for the tag are **two PL bare-metal proofs** — CB-5 (WPA3-SAE on-air, ratification R1) and CB-1b's real `wlan0` cold lease (gated on the PL staging `mt7601u.bin`). Neither is code.

So CB-6 splits cleanly:

- **THIS build (Janus, now):** the consolidation + status-repair + OT prune coder pass on the final tree, gated by 4-shard QEMU + AAG. Turns the tree clean and legible.
- **The PM's held step (after this merges + the two bare-metal proofs land):** integrated QEMU close gate on the final tree, archive the bare-metal close-bar evidence, **squash + phase tag** on `main`.

**The whole analysis is already done.** Prometheus's `THINK-2026-06-24-phase-7.5-cb-6-consolidation-audit.md` (on `main`) is the spec: four lists (clippy pay-or-bank, dead-scaffolding, ledger re-fit, OT prune) + a fresh-reader legibility pass. Your job is to **execute** it, not re-derive it — but you are the coder on the final tree, so **re-confirm each item against HEAD before you touch it** (the audit was cut at `8396710`; HEAD is `4279683`).

## 1. References (read first)
```
git fetch origin
git show origin/main:Plans/THINK-2026-06-24-phase-7.5-cb-6-consolidation-audit.md   # THE SPEC — four lists + legibility pass
git show origin/main:Plans/PLAN-2026-06-23-phase-7.5-closeout.md                     # §1 CB-6 scope + §2 R1/R2 rulings + close-set status
git show origin/main:docs/complexity-governance.md                                  # the I-1..I-15 ledger; page-fit alarm is LIVE (:16-19)
git show origin/main:docs/OPEN-THREADS.md                                           # the OT register to prune
git show origin/main:docs/adversarial-audit-gate.md                                 # the AAG you'll be RETURNED against
```

## 2. ISCs (ordered; each its own sub-commit)

### ISC-1 — Clippy pay-down (audit List 1: 26 PAY / 7 BANK)
Run `cargo clippy -p neoplutarch-kernel --all-targets --features mt7601u` to reproduce the **33-warning** set. Then:
- **`cargo clippy --fix`** the auto-fixable PAY cluster (A1 `doc_lazy_continuation`, A2/A3 `manual_range_contains`, A4 `collapsible_if` ×10, C3/C4 `needless_borrows_for_generic_args`, C5 `useless_conversion`).
- **Manually fix** A5 (`wedge_seen` dead variable — `mt7601u/mod.rs:1658,1714`; prefix `_` or remove) and B1 (`unused_import: UsbInterface` — `usb/mod.rs:47`).
- **Leave BANKED, with a one-line `// clippy-bank:` rationale at each site** (do NOT fix): A6/B2/D1 `result_large_err` ×3 (structural error-type boxing — **CB-4 follow-on**, the xHCI endpoint-reset touches that surface), and C1/C2 rng `dead_code`. **C2 is load-bearing — do NOT remove it:** the `rng/kat.rs` KAT functions look compile-dead but **are the I-12 byte-oracle** for the CSPRNG (they run via the `csprng_selftest` path, not direct calls). Deleting them deletes the oracle. C1 (`next_u64/next_u32`) is intentional retained API.
- **Acceptance:** `cargo clippy -p neoplutarch-kernel --all-targets --features mt7601u` shows only the 7 documented BANKED warnings; default-feature clippy is `-D warnings` clean.

### ISC-2 — Dead-scaffolding removal (audit List 2: 4 items, EXCLUDE discriminators)
Remove exactly the four confirmed-dead items; **touch no gate discriminator** (Rule 5 — discriminators stay armed permanently even when "unused"):
- **S1** `loader/elf.rs:39` `const PF_R: u32 = 4` (zero callers; loader checks W+X only).
- **S2** `arch/mod.rs:54` `pub fn interrupts_enabled()` wrapper (all callers use `x86_64::` directly). The audit pairs it with `arch/mod.rs:47 disable_interrupts()` — **remove the pair together or neither**; recommend removing both (multi-arch forward scaffolding with no Phase-7.5 consumer). Use judgment; if you keep them as an aarch64 seam, say so in the commit.
- **S3** `drivers/usb/cdc_ecm.rs:838` `debug_pipe_drops()` (`neo-test`-gated, no scenario consumes it; siblings `debug_bulk_in_route`/`debug_ax88179_diag` ARE consumed — keep those).
- **S4** `drivers/net/virtio_net.rs:325-335` `debug_used_ring_acquire_barriers()` + `debug_primary_device()` (`neo-test`-gated, zero consumers; the 20+-consumer siblings stay).
- **Keep (do not remove):** `fs/mod.rs:419 MountEntry.flags` (live `sys_mount` ABI). 
- **Acceptance:** kernel + `neo-test` features still compile; no scenario references a removed symbol (grep the test tree); the AAG memory/completeness lens will check nothing live was cut.

### ISC-3 — Invariant-ledger re-fit (audit List 3: page-fit alarm, Trims 1–4)
The page-fit alarm in `complexity-governance.md` (:16-19) is **RAISED** and **blocks any future I-16**. Apply the four prose trims — pure prose moves, **no behavioral change, no guard changes, no re-verification** (the guards don't move, only where the evidence is recorded):
- **Trim 1** — move I-14(c) silicon evidence (BDF/iova/stage verdicts) to ADR-045/ADR-050; ledger row keeps the mechanism name + "see ADRs for evidence."
- **Trim 2** — move I-14(a)/(b) proof detail to ADR-039/ANSPEC §9.12; keep mechanism + "see ADR-039."
- **Trim 3** — condense the I-15 `Guarded by` enumeration to the mapping-oracle + subset sweep + the QEMU `file_cap_derive_grant` scenario + "see ADR-061."
- **Trim 4** — move the I-14 `(a)/(b)/(c)` vector taxonomy from `Guarded by` into `Stated where` (it's architectural context, not a guard).
- Then **clear the "Page-fit flag" text** (:16-19) — restate it as resolved (the I-1..I-15 table now fits one printed page) and add a one-line audit-log entry dated 2026-06-29 noting the CB-6 re-fit.
- **Acceptance:** the I-1..I-15 table proper fits one printed page; no invariant row's *guarantee* changed (diff is prose location only).

### ISC-4 — Status-drift repair (PLAN §1 Janus Charter-C + audit List 5 legibility)
Fix the stale pointers a fresh reader of the close tree would trip on:
- **ADR-057** stale "Stage-B pending" → "association proven on silicon (`4f82155`); tail = usable-link hardening (CB-1/CB-1b)."
- **README** OT-2 phrased as future work → it's **CLOSED** (`d1c8bb6`); restate as done.
- **`docs/bible.md` Art. 9.10** firmware-bundled wording → reconcile with **ADR-054's staged-blob-only** rule (no model/firmware blob in git; staged from disk).
- **ADR-017** → point-to-the-amendment-first (the `iwlwifi → MT7601U` ratification, `docs/bible.md:179` Art. 24.1) so a reader lands on the live decision, not the superseded target.
- **CLAUDE.md** legibility: mark the off-boarded **Genesis** seat "OFF-BOARDED" (one line, not a paragraph); add a "SKIP IF USING CLOUD PM" callout to the two-worktree section.
- **Fold the CB-3 hardening note** (PLAN close-set status, non-blocking): add the **registration-time assert** tying the registry idx ≡ the ipv4/ipv6 `interface_id` (today upheld by caller convention; mismatch falls back to interface 0, never OOB — benign, but the assert makes it machine-checked). This is the one ISC that touches live kernel logic → it is the reason the QEMU gate is non-waivable.
- **Acceptance:** every cross-ref above resolves to the live fact; the CB-3 assert compiles and the `file_cap`/identity scenarios still pass.

### ISC-5 — OPEN-THREADS prune (audit List 4)
Per the register's own maintenance rule ("RESOLVED kept briefly with closing evidence, then removed at next audit" — **CB-6 IS that audit**):
- **Remove** the fully-RESOLVED entries, **preserving each closing SHA + CI URL in the commit message**: **OT-2** (`d1c8bb6`), **OT-29** (`1c3ee65`), **OT-30** (`e517a52`), **OT-37** (`d801f70`). Re-confirm each shows CLOSED on HEAD before removing. Also sweep the newer RESOLVED entries the audit predates — **OT-40** (RESOLVED `9b4a875`) is a prune candidate; **re-verify its closing evidence is on `main`** before removing.
- **Do NOT prune** the multi-item OTs: **OT-27** (only (a) closed; (b)/(c)/(d) open — update the status text, keep the entry), **OT-18** (E4 dropped by PL ruling, but E3-revoke/E5 active). **OT-39** (CB-1b carry-forward) and **OT-41** (console fb-label nit) stay open.
- **Delete** the two historical "Pruned at the 2026-06-20 Rule-4 audit" / "Dispositioned at the 2026-06-17 audit" tables outright — their content lives in git history.
- **Acceptance:** the active table holds only genuinely-open threads; every removal's evidence is in the commit message; no open multi-item OT lost.

## 3. Scope guard
- **In scope:** the five ISCs above — clippy pay-down, the 4 scaffolding removals, the 4 ledger prose trims, status-repair (incl. the one CB-3 registration assert), the OT prune.
- **Out of scope / STOP and note as a line-item:** any clippy BANK item (A6/B2/D1, C1/C2), any new feature/behavior, any QEMU scenario change beyond keeping the existing gate green, any model/firmware blob, the WPA3/CB-5 work, the squash+tag (PM step). **Do not remove a gate discriminator or the rng KAT oracle.** If something the audit listed turns out to need real logic surgery (beyond the CB-3 assert), stop and flag it — don't expand the build.

## 4. Gate + close (the close-gate ritual)
- **Kernel-touching ⇒ mandatory 4-shard QEMU `workflow_dispatch` at `NEO_SMP=4`, `-cpu max`** on the final tree — **non-waivable** (ISC-2 removes compiled code and ISC-4 adds a live assert; the default binary changes, so QEMU must exercise it). **Green is not done without the CI run URL** — put all four shard URLs + the head SHA in the RETURN.
- **push-CI** green (host/clippy/fmt + byte-oracles). `cargo fmt` + SPDX clean on every touched file. **Never `[skip ci]`.**
- **Close-gate `scripts/worker-close-gate.sh`** (host set; SPDX; self-stat). Immediately before declaring close: `git fetch origin && git rebase origin/main`, re-run the gate after the rebase, then push the rebased branch (so the PM merge is a trivial `--ff-only`).
- **RETURN** `Plans/RETURN-2026-06-29-phase-7.5-cb-6-consolidation-janus.md` with: the 4 QEMU shard URLs + SHA; the final clippy count (should be the 7 documented BANKED only); the scaffolding-removed list; the ledger before/after page-fit note; the OT-prune list with preserved evidence; and an explicit line confirming **no gate discriminator and no rng KAT oracle was removed**.
- **PM on RETURN:** verify-before-merge (`git show --stat` self-inspect) + **AAG** (lenses: did `clippy --fix` change behavior anywhere; did any scaffolding removal cut something a gate/oracle needs — esp. the rng KAT C2; is the CB-3 assert correct and non-regressive; completeness vs the audit's four lists), then `pm-merge-check` → `--ff-only`/cherry-pick to `main`.

## 5. The tag is HELD (PM step, after this merges)
After this consolidation lands on `main` AND the two PL bare-metal proofs are captured — **CB-5 WPA3-SAE on-air** (R1) and **CB-1b `wlan0` cold lease** (gated on the PL staging `mt7601u.bin`, 45412 bytes) — the PM runs the **integrated QEMU close gate at `NEO_SMP=4` on the final tree**, archives the bare-metal close-bar evidence (Janus's 7-point bar), and lands the **squash + Phase-7.5 tag** on `main`. This dispatch does **not** tag; it produces the tree the tag will point at.

---
## Relay — what to tell Janus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-29-phase-7.5-cb-6-consolidation-janus.md` — and **the spec it executes**, `git show origin/main:Plans/THINK-2026-06-24-phase-7.5-cb-6-consolidation-audit.md` (Prometheus's four lists). **Branch:** `codex/cb-6-consolidation-janus` off `origin/main` HEAD (`4279683`), Author `SecondStageTurbine`.
- **One-line:** **Execute the CB-6 consolidation audit on the close tree — pay-down clippy (26 PAY, leave 7 documented BANK), remove the 4 dead-scaffolding items, re-fit the I-14/I-15 ledger prose (page-fit alarm is live, blocks I-16), repair the status-drift cross-refs (ADR-057/README/bible Art.9.10/ADR-017/CLAUDE.md) + add the one CB-3 registration assert, and prune the RESOLVED OTs (OT-2/29/30/37 + verify OT-40) preserving evidence in the commit message.** Five ordered sub-commits = ISC-1…ISC-5.
- **Do NOT:** remove a gate discriminator or the **rng KAT functions** (they look dead but ARE the I-12 CSPRNG byte-oracle); fix any BANK clippy item (A6/B2/D1 `result_large_err` → CB-4; C1/C2 rng → intentional); touch CB-5/WPA3; or squash/tag (PM step). If anything needs real logic surgery beyond the CB-3 assert, **stop and flag it**.
- **Gate:** kernel-touching ⇒ **mandatory 4-shard QEMU `NEO_SMP=4` workflow_dispatch** (non-waivable — code changes) + push-CI green; **green without the CI run URLs is not done.** Rebase onto `origin/main` and re-run the gate immediately before close. Close-gate `scripts/worker-close-gate.sh`. **RETURN** `Plans/RETURN-2026-06-29-phase-7.5-cb-6-consolidation-janus.md` with the 4 shard URLs + SHA, the final clippy/scaffolding/ledger/OT-prune lists, and the explicit "no discriminator/oracle removed" line. PM verifies + AAGs + merges; the **squash + Phase-7.5 tag** is the PM's held final step after CB-5 + CB-1b bare-metal proofs land.
