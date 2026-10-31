<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase-7.5 closeout prep (host-only) — T-039 WPA3-GKH raw-source confirm + CB-6 status-drift repair → Prometheus

**Date:** 2026-06-23
**From:** PM (Claude)
**To:** Prometheus (research / doc-consolidation / bounded host-only lane). New branch off `origin/main` HEAD: `prometheus/cb-prep-t039-statusdrift`.
**Status:** DISPATCHED — **runner-free closeout prep that unblocks CB-5 and pays down CB-6's checklist while the kernel/QEMU seats are full** (Cerebro on CB-1, Oracle on Stage-B, Jarvis/Janus out until tomorrow). **Scope: HOST-only + docs — push-CI only. No kernel-touching code, no QEMU-gated build, no driver. Stays inside the Prometheus scope (CLAUDE.md).**

## 0. The decision this build makes
Close the two non-kernel gaps that otherwise block the close tail: **(A)** turn T-039 (the WPA3 Group-Key-Handshake GTK byte layout — currently IEEE-derived, *needs-raw-source-confirm*) into an **oracle-grade** trap so CB-5's bare-metal SAE run has no open byte-layout risk; **(B)** repair the **status drift** on CB-6's checklist so the close tree's docs tell the truth.

## 1. References
```
git fetch origin
git show origin/main:Plans/RESEARCH-2026-06-23-phase-7.5-closeout-wifi-trap-mine-FINDINGS.md  # T-039 entry (line ~252) + the "needs-confirm" note (~431)
git show origin/main:Plans/PLAN-2026-06-23-phase-7.5-closeout.md            # §CB-5 (R1: WPA3 on-air required) + §CB-6 status-drift list
git show origin/main:kernel/src/net/wpa/eapol.rs                            # EAPOL-Key parse (READ for layout context — DO NOT modify kernel code)
git show origin/main:docs/ADR-057-phase-7.5-w2-mlme-supplicant.md           # status line "Stage-B pending" (line 4) — drift target
git show origin/main:docs/ADR-054-phase-7.5-mt7601u-driver.md               # staged-blob-only rule (vs bible Art. 9.10 wording)
git show origin/main:docs/ADR-017-wifi-support-strategy.md                  # point-to-the-amendment-first
git show origin/main:docs/bible.md                                          # Art. 9.10 firmware-bundled wording
git show origin/main:README.md                                             # OT-2 caveat (line ~195 — OT-2 is closed, d1c8bb6)
```

## 2. ISCs

### ISC-A — T-039 WPA3-GKH raw-source confirm (research → oracle-grade)
- **Acquisition rule (CLAUDE.md byte-oracle):** pull the iwd Group-Key-Handshake handler from **raw source** — `git`/`curl` the literal `iwd/src/sm.c` (the GKH / EAPOL-Key GTK-extraction path) — **NOT WebFetch / any summarizing read** (a mangled offset poisons the oracle). Read the literal bytes.
- Confirm T-039's GKH-Message-1 layout (EAPOL-Key with GTK KDE, Key-Data encrypted, GTK-KDE keyid/reserved/gtk as in T-038) against iwd's actual extraction. Update the **T-039 entry in the FINDINGS** to oracle-grade (cite the raw iwd file + line), and bump the summary table (currently "T-039 needs confirmation").
- **Deliverable: a hand-laid GKH-Message-1 GTK-KDE byte fixture** (host test — e.g. alongside the existing eapol host oracle) asserted against the parse, independent of writer/reader. This is the same byte-oracle deliverable CB-1 shipped for the 4-way M3 GTK; CB-5 reuses it for the SAE GKH path. *(Host-testable assertion only — do not modify kernel `eapol.rs`; if the oracle needs a parse entry point that isn't host-reachable, report it as a CB-5 note rather than refactoring kernel code here.)*

### ISC-B — CB-6 status-drift repair (doc-only)
Repair the four drifts on the CB-6 checklist so the close docs are truthful:
1. **`docs/ADR-057-phase-7.5-w2-mlme-supplicant.md`** status line (line 4): "CODE-COMPLETE, **Stage-B pending**" → "**association proven on-air; usable-link tail (GTK/DHCP = CB-1) + WPA3-on-air (CB-5) open**" (match the PLAN's actual close state).
2. **`README.md`** OT-2 caveat (~line 195): "W2's SNonce swap **still closes** OT-2" implies OT-2 open — **OT-2 is closed** (`d1c8bb6`, real-CSPRNG SNonce). Correct to past-tense/closed and point at `OPEN-THREADS.md` for the live set.
3. **`docs/bible.md` Art. 9.10** firmware-bundled wording vs **ADR-054**'s staged-blob-only rule — reconcile so the bible doesn't read as "blob bundled in image" against ADR-054 (no blob in git; staged at build time).
4. **`docs/ADR-017`** — make it point-to-the-amendment-first (the standing ADR-017 hygiene fix).

## 3. Scope guard
**HOST-only + docs.** **Do NOT:** modify any kernel/driver code (`kernel/**`), add a QEMU scenario, touch a Cargo feature that triggers the kernel build, or open a kernel-touching gate. The T-039 oracle is a **host test**; everything else is docs. If a task seems to require a kernel-code change, **stop and report it** as a CB-1/CB-5 note — do not cross the scope line.

## 4. Gate + close (PM)
- **Host:** `cargo fmt --all --check`; `cargo test --workspace --exclude neoplutarch-kernel` (the new GKH oracle green); SPDX on any new test file. **No QEMU gate** (host-only — push CI only; the doc-only changes paths-ignore CI by design).
- **Close-gate: `scripts/worker-close-gate.sh`** (rebase LAST; host-check set; self-stat). RETURN the **green push-CI URL**, the **T-039 oracle pass + the raw iwd source cite**, and the four doc diffs. PM `pm-merge-check` → `--ff-only`/cherry-pick.
- **Disk reclaim (after PM confirms merge):** delete the build tree once `prometheus/cb-prep-t039-statusdrift` is on `origin/main`. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-23-phase-7.5-cb-prep-prometheus.md` (+ the FINDINGS T-039 entry, the PLAN §CB-5/§CB-6, and the four doc targets). **Branch:** `prometheus/cb-prep-t039-statusdrift` off `origin/main` HEAD.
- **One-line:** **Runner-free closeout prep — no kernel, no QEMU.** **(A)** Confirm the **WPA3 Group-Key-Handshake GTK byte layout (T-039)** from **raw `iwd/src/sm.c`** (`git`/`curl` the literal file — **NOT WebFetch**; byte-oracle acquisition rule), upgrade the FINDINGS T-039 entry to oracle-grade with the source cite, and **ship a hand-laid GKH-Message-1 GTK-KDE byte fixture as a host test** (reuses CB-1's M3-GTK oracle pattern; pre-clears CB-5's only open byte-layout risk). **(B)** Repair four **status drifts** (doc-only): ADR-057-w2 "Stage-B pending" → association-proven/tail-open; README OT-2 caveat (OT-2 is **closed**, `d1c8bb6`); bible Art. 9.10 firmware-bundled wording vs ADR-054 staged-blob-only; ADR-017 point-to-the-amendment-first. **Scope: HOST-only + docs — do NOT touch `kernel/**`, no QEMU, no Cargo kernel-build feature; if a task needs kernel code, STOP and report it as a CB-5 note.** **Close-gate `scripts/worker-close-gate.sh`; RETURN the green push-CI URL + the T-039 oracle pass + raw iwd cite + the doc diffs.**
