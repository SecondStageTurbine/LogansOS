<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-048: Phase 7.5 — BUILD-K, the Connectivity-Lane Consolidation (second Rule-4 digest)

**Status:** Accepted
**Date:** 2026-06-13
**Authority:** `docs/complexity-governance.md` Rule 4 (second invocation; the connectivity lane is four feature builds deep — 7.5-0 / A / B + branding). ADR-041 (BUILD-C1) is the precedent; this ADR is its connectivity counterpart.
**Supersedes:** the from-memory chat preview of this build, which was wrong on three items (corrected below).

## 0. The honest scope — small by design

The lane was built disciplined, so the code-collapse portion is near-empty: the `dead_code` allows were already per-item Rule-5-labeled (zero blanket), I-14 was already split into its three vectors, and the descriptor walk was already DRY. This consolidation is therefore **primarily the mandated doc-hygiene + ledger re-audit + the fresh-reader test** — not a refactor. Zero new features / invariants / `unsafe`; zero behavior change. The diff is docs + one binary delete + three comment-only `§`-bumps.

**Audit scorecard — what the preview got wrong, corrected against the tree @ `c611960`:**

| Preview claimed | Tree truth | Disposition |
|---|---|---|
| "four blanket `dead_code` allows" to remove | 21 allows, all per-item Rule-5-labeled; **zero blanket** | ISC-3 = verify-only (nothing to remove) |
| `xhci.rs` is some other size | **1787 lines** (`wc -l`; a host PowerShell `Measure-Object` miscount said 1676) | confirmed 1787 |
| duplicated descriptor-walk in `xhci.rs` | already DRY — `xhci.rs:1128` calls `usb_parse::parse_configuration_set` | ISC-4 = **leave** (claim did not survive) |
| I-14 needs splitting (a)/(b)/(c1)/(c2) | already split | ISC-5 = wording refinement only |
| a "31-vs-25 lock drift" | no drift: 31 was the C1 *total*, 25 the *leaf* subset (31 − 6 ordered) | ISC-6 = re-measure + document the method |

## 1. The §10 renumber (ISC-1, the one non-trivial item)

ANSPEC-001 (`docs/specs/neoplutarch-ai-subsystem-spec.md`) §10 carried **two `### 10.5`** — "USB host" (BUILD-A) and the original "Model-as-attack-vector" — with the side-channels / connectivity sections interleaved. Renumbered to a single clean scheme: the **threat-model block contiguous** (10.1–10.5) and the **connectivity block contiguous** (10.6–10.8).

| Old § | Section | New § |
|---|---|---|
| 10.1 | The threat model | 10.1 |
| 10.2 | Capability discipline | 10.2 |
| 10.3 | Model integrity | 10.3 |
| 10.6 | Side channels | **10.4** |
| 10.5 (dup, original) | Model-as-attack-vector | **10.5** |
| 10.4 | DMA isolation (ADR-045) | **10.6** |
| 10.5 (dup, BUILD-A) | USB host (ADR-046) | **10.7** |
| 10.7 | USB Ethernet (ADR-047) | **10.8** |

The ordering was chosen to keep **Model-as-attack-vector at §10.5**, so ANSPEC-002 §10.3's by-number cross-ref (`ANSPEC-001 §10.5 (model-as-attack-vector)`, L410/L412) is preserved unchanged — the highest-fan-in pointer takes zero churn.

**Cross-refs reconciled (grep-proven zero dangling `§10.x` in the live set):**
- ANSPEC-001 internal: "the §10.4 fenced arena" → §10.6; "the §10.5 seam" → §10.7.
- `docs/ADR-047` `**Spec:**` line: §10.7 → §10.8.
- `docs/bible.md` (Phase-7.5 ledger L221 + the append-log L352): §10.4+ADR-045 → §10.6; §10.5+ADR-046 → §10.7; §10.7+ADR-047 → §10.8 (anchored on the ADR number; the bare model-as-attack-vector §10.5 has no `+ ADR` suffix and was untouched).
- `Plans/INDEX.md`: BUILD-A §10.5 → §10.7; BUILD-B §10.7 → §10.8.
- The three `usb/` module heads: `xhci.rs` §10.5 → §10.7; `cdc_ecm.rs` + `usb_parse.rs` §10.7 → §10.8.

**Intentionally left as historical record** (outside the ISC-1 reconcile scope of docs/ + ADR Spec lines + bible + INDEX): the closed `Plans/DISPATCH-2026-06-11-phase-7.5-build-A/B-*.md` (which cite the then-current numbers) and everything under `Plans/archive/`. These are point-in-time records, not live pointers.

## 2. The screenshot (ISC-2)

`latest-screenshot.png` (267 KB) was tracked at the repo root with zero references anywhere (grep-confirmed) — an ephemeral "latest" capture. Deleted. A keepable boot shot belongs at `Plans/evidence/<named>.png`.

## 3. `dead_code` sweep (ISC-3 = verify, already clean)

21 `#[allow(dead_code)]` across the connectivity code — `usb/xhci.rs` (5), `usb/cdc_ecm.rs` (8), `usb/mod.rs` (2), `dma_arena.rs` (6) — every one **per-item with a Rule-5 justification** ("I-14(c1) discriminator surface — gate-consumed", "kill-switch gate seam", "I-14(b) storm-gate seam", "Stage-B probe consumer", etc.). **Zero blanket `#![allow(dead_code)]`.** Nothing stripped — these are the ledger's armed guard column (Rule 5).

## 4. `xhci.rs` assessed and LEFT (ISC-4)

1787 lines, the lane's largest file. The refactor bar — *genuine duplication or a materially-legibility-improving split* — is **not met**:

- **No descriptor-walk duplication.** `xhci.rs:1128` calls `super::usb_parse::parse_configuration_set(&raw)`; the descriptor walk is a single pure-tier implementation in `usb_parse.rs`, consumed by both `xhci` (enumeration) and `cdc_ecm` (class bind). The correct factoring already exists. The preview asserted this duplication without verifying it.
- **The only duplication is a 2-line fence wrapper** — `xhci::fence_buffer` ≡ `cdc_ecm::fence_whole`, both `dma_arena::fence(buf.physical_addr(), buf.len())`. Self-documenting and local to each driver; hoisting it adds indirection, churns two silicon-validated drivers, and escalates the gate — below the "real win" bar. Left in place (more legible local than hoisted).
- **The file is cohesive-but-large** — inherent xHCI surface (register maps, the bring-up sequence, the ISR/event path, enumeration + control transfers, the `ClaimedDevice`/`EndpointPipe` class-facing handle) — and already factored where it pays (`xhci_ring_logic.rs` + `usb_parse.rs` pure tier, `mod ring` extracted).

Per ISC-4 ("if cohesive-but-large: leave it and say so; do NOT churn a just-silicon-validated driver for cosmetics"), the split is declined. This keeps the build in the doc-only gate lane.

## 5. The ledger re-audit (ISC-6, Rule 4) — all 14 rows

Every row walked; its guard named; its guard confirmed present and armed in the tree. (This is a doc-only digest with no `ai/` or gate-scenario diff, so the guards stand exactly as the C1 / O-arc / 7.5 builds proved them — the audit re-verifies existence and arming, not a re-run.)

| Row | Guard named | Lives / runs in |
|-----|-------------|-----------------|
| I-1 byte-oracle | byte-identical asserts in every gate scenario | all QEMU scenarios, both stress loops |
| I-2 kill-switch | `W=0`≡P2, decay-off≡latch unit equalities; **arena-off≡pre-arena, USB-off≡pre-USB, `cdc_ecm`-off≡BUILD-A**; prior-layer family | host suite + full family every build |
| I-3 single-mutator (relaxed/re-proven) | P4a stress gate + generation counter / quiesce / migrate-owner uniqueness | `accel_migration_reservation` @ NEO_SMP=4 |
| I-4 unique-strong-owner-at-rest | claim truth-table tests; `get_mut`; O1a mint-vs-claim-vs-free gate; `unsafe`-diff scrutiny | host suite + O1a gate + CI |
| I-5 one lock order | the debug lockdep (`new_ordered`; panics at the offending acquire) | every dev-profile QEMU run |
| I-6 accounting exactness | per-migration delta asserts + balance-to-zero teardown | every gate @ NEO_SMP=4 |
| I-7 residency ≤ capacity | §9.5 high-water (peak) asserts | every pressure gate |
| I-8 anti-thrash | no-partial-eviction + thrash-guard unit tests; oscillation/mispredict counters | host suite + P3/P4b gates |
| I-9 pin protection | falls out of I-4 + explicit pinned-set checks | P2/P3/O1b gates |
| I-10 temporal isolation | admission/`block-on-defer` scenarios + INFLIGHT attribution | `accel_admission*` |
| I-11 no lost wakeup | park/retry balance + quiesce + teardown drain asserts | P4a/P4b gates |
| I-12 byte-layout oracle | the committed `.neom` hand-laid fixture tests | host suite / CI |
| I-13 stale handles fail safe | generation-mismatch counter (>0 generated, 0 stale resolves ever); reuse-attack truth tables; keyed byte-tag oracle | O1a stress gate |
| I-14 subsystem fault containment | (a) no-AI-lock-in-IRQ (grep-check + runtime assertion); (b) governance-green-under-simulated-storm scenario; (c1) `dma_arena_fence` adversarial gate **+ silicon BUILD-A/B** (47 admits / 0 refusals on real DMA); (c2) deferred to 7.5-V | usb storm/fence gates + the AX201/AX88179 bare-metal runs |

**14/14 verified; zero rows added (Rule 2).** I-14's (a)/(b)/(c1) wording promoted from "guarded (QEMU)" to **silicon-PROVEN** (ISC-5); (c2) stays mandated-not-yet-proven (VT-d at 7.5-V).

## 6. The structural facts, re-measured to a documented method (ISC-6)

C1 reported "31 lock statics" but recorded no reproducible counting method — the root cause of the "31-vs-25" confusion (different greps, different numbers). This build defines and records **the** method:

> **Lock-statics census (canonical).** Count `static` items in `kernel/src/**/*.rs` whose declared type is a lock primitive — `SwitchMutex`, `IrqSafeMutex`, or `spin::Mutex` (including behind `Lazy<…>`). Excludes atomics and generic `Mutex<…>` in non-static positions. NEO_TEST `test_harness.rs` locks are counted separately (they are not production lock surface).

Applied @ `c611960`:

- **37 production lock statics** (55 grand total − 18 in `test_harness.rs`). C1's 31 grew by the connectivity lane's **9** (USB ×8: `xhci` CONTROLLER/EVENT/PENDING/EP_ROUTES, `cdc_ecm` AX_BIND_FAIL/PRIMARY, `usb/mod` CLASS_DRIVERS/DEVICES; + the `dma_arena` SPANS lock) plus the O1 slot-arena locks landed since C1.
- **12 ordered** (via `new_ordered`, lockdep-tracked) / **25 leaf** (37 − 12). The "31-vs-25" was never a drift: 25 is the leaf subset (then 31 − 6 ordered, now 37 − 12 ordered — the leaf count is coincidentally still 25).
- **Ordered band fits `1..=31`, ceiling reached.** Levels in use: ADMISSION_QUEUES 10, ARENA 12, TENSOR_SLOT 15, RESERVATION 20, ACCT_INFLIGHT 22, ACCT_TABLE 25, ACCT_RECENCY 28, **USB_DEVICE 29 / USB_XHCI 30 / USB_CORE 31** (the USB band above every AI level by I-14(a)). Max ordered level = **31 = the documented ceiling**. **Forward finding (no BLOCKER — nothing exceeded 31):** a future lock needing to order *above* the USB band would require widening the domain; interior slots (1–9, 11, 13–14, 16–19, 21, 23–24, 26–27) remain free, so VT-d at 7.5-V can be slotted (or kept leaf/unordered).

The other structural fact — **pure-tier share (31% policy+glue / 44% with CPU op kernels)** — is an `ai/`-scoped ratio, and BUILD-K added **zero `ai/` LOC**, so it stands exactly as C1 measured it.

## 7. The fresh-reader test + kill-switch re-prove (ISC-7)

A cold read of `kernel/src/drivers/usb/mod.rs`: its module doc-comment already **is** the lane's table of contents — it walks arena → `xhci` (controller) → `usb_parse` (pure descriptors) → the enumerated-device model → the `UsbClassDriver` seam → `cdc_ecm` → `NetDevice`, and states the full I-14 (a)/(b)/(c1) containment posture inline. The layering reconstructs without `git log`; no ToC edit was required (unlike C1, where `ai/mod.rs` had to be rewritten into a ToC). The §-pointers in the three `usb/` heads were the one drift, fixed in ISC-1.

Kill-switches re-proven by construction (no policy/gate code touched): **arena-off ≡ pre-arena, USB-off ≡ pre-USB, `cdc_ecm`-disabled ≡ BUILD-A** — the disabled legs build green in the gate (§8) and their equality scenarios are untouched.

## 8. Net effect

- **Net LOC: positive in docs, zero in code.** Unlike C1 (−295 code LOC collapsing real duplication), BUILD-K found the connectivity code already clean — no dead scaffolding to delete, the descriptor-walk already DRY, `xhci` correctly left whole — so there is no code to remove. The digest is *additive documentation* (this ADR + the ledger's I-14 silicon note, census re-measure, and BUILD-K audit-log entry). The only deletion is the 267 KB `latest-screenshot.png` binary and the duplicate `§10.5` heading. Reported honestly rather than gamed: a clean lane has nothing to subtract.
- **Diff:** ANSPEC-001 §10 reorder (16/16, net 0 lines); 8 cross-ref number-swaps; the screenshot delete; the ledger updates; this ADR.
- **14/14 ledger rows verified; 0 added; 0 new `unsafe`; 0 behavior change.**

**Noted, not folded in** (scope guard — a mid-digest observation is the PM's, not a silent rider):

- `Plans/INDEX.md` still marks the closed BUILD-B dispatch "active" (it silicon-closed 2026-06-13, `766478f`). A one-word status fix, left for the PM to land separately rather than churn into a §-renumber commit.
- **Repo-wide stale-CRLF (≈22 files).** `git add --renormalize .` (the ISC-8 mechanism) revealed that ~22 files are committed on `origin/main` with CRLF despite `.gitattributes`' `* text=auto eol=lf` — predating its enforcement. Two are files this build edits (`complexity-governance.md` = 126 CR, `INDEX.md` = 249 CR); the rest are untouched here (`.gitignore`, `kernel/src/init/boot_logo.rs`, `kernel/src/sync/mod.rs`, `tools/neomc/src/fixtures.rs`, the `tools/*-tests/Cargo.toml`, every `Plans/evidence/*.log`). Running `renormalize .` would flip all 22 to LF and fold a repo-wide EOL cleanup into a connectivity-lane consolidation — a scope-guard breach. **Decision:** BUILD-K commits its content changes preserving each file's existing EOL (verified to introduce **zero** new CR bytes — the two stale files stay 126/249, the LF files stay LF), and the repo-wide renormalization is flagged to the PM as **its own dedicated commit** (it touches kernel source and all evidence logs; it should not ride a doc digest, and it risks colliding with in-flight branches).

## 9. The gate

Per the dispatch's ISC-4-conditional scaling, **no `xhci` refactor was taken**, so the doc-only gate applies: the `§10.x` grep-clean (zero dangling pointers in the live set), this ledger audit table, builds-green on + off + the kill-switch feature legs (arena/USB/`cdc_ecm` disabled), `clippy`/`fmt`, line-endings verified (the changes introduce **zero** new CR bytes; the literal `renormalize .` was deliberately *not* applied — see §8's repo-wide stale-CRLF finding, left to its own commit), `[skip ci]`. No QEMU family re-run was required (no policy/gate-scenario diff). The `.rs` changes are comment-only `§`-pointer bumps — compilation-inert by construction, and confirmed by the on/off builds.
