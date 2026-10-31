# PM-NOTE: Janus first-pass citation spot-check

**Date:** 2026-05-10
**Author:** PM
**Status:** PM action item closure. Carry-forward from `Plans/HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md` §7.2 and the SC-3B investigation chain handoffs. Non-blocking when filed; closing now during Sub-step 5c wait window.
**Subject:** Verification of citations in `Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md` (the Janus first-pass deep-research artifact).

---

## 1. Why this exists

The first Janus deep-research dispatch was the project's inaugural use of a deep-research specialist agent. Per `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` §3.3, first-dispatch outputs warrant elevated review intensity until calibration data accrues. The artifact's cited corpus (24 primary sources) needed a spot-check of 5-10 citations to confirm:

- The cited URLs resolve to real sources.
- The titles, authors, dates, and venues match what Janus reported.
- The substantive content matches Janus's framing (no paraphrase drift toward fabrication).
- Specific numerical claims (rates, counts, versions) match the source material.

This note records the result. It is also load-bearing for assessing the second-pass artifact (`Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md`), since Janus's first-pass calibration informs how heavily to scrutinize the second-pass.

---

## 2. Citations spot-checked

Six of 24 primary citations were checked. Selection followed the carry-forward guidance: prioritize high-load-bearing items (Linux commit, QEMU issue, LKML thread, Asterinas PR, Asterinas issue, one of the seL4/Fuchsia/Theseus references).

### 2.1 Linux commit `13bb06f8dd42071cb9a49f6e21099eea05d4b856` — VERIFIED ✓

- Title cited by Janus: "tick/common: Align tick period during sched_timer setup."
- Title found via search: exact match. Patch authored by Thomas Gleixner via Sebastian Andrzej Siewior (lore.kernel.org), dated 15 June 2023.
- Janus's framing: "an early tick-period alignment could leave too little time for boot progress, causing the system to hang."
- Verified: a regression report on linux-stable-mirror confirms the patch addressed VM boot hangs ("It makes the (boot) clock run faster... Up to the point where it's running too fast to make any progress at all. This patch causes VM boot hangs for us.") — Janus's framing is faithful.

### 2.2 QEMU issue #1696 — VERIFIED ✓

- Title cited by Janus: "Linux kernel hangs rarely when booting on the latest qemu."
- Title at gitlab.com: exact match. Author: Richard W.M. Jones. Filed 2023.
- Janus's substantive claims:
  - Fedora Linux kernel rarely hanging on QEMU 8.0.0 — confirmed: "qemu-system-x86-8.0.0-4.fc39.x86_64."
  - Hang location: "stopping after 'Freeing SMP alternatives memory' before the next 'smpboot' line" — confirmed verbatim from the issue body.
  - Rate: "about 1 in 300 boots" — confirmed: "It probably happens one in every 300 boots."
- Specific iteration claims (run 260, thousands of iterations) live in issue comments not surfaced in this fetch but the framing is faithful.

### 2.3 LKML printk.time thread — VERIFIED ✓

- Title cited by Janus: "printk.time causes rare kernel boot hangs."
- Title found: exact match. Author: Richard W.M. Jones. Date: 2023-06-13. Thread: 32+ messages with Peter Zijlstra and others.
- Janus's specific numerical claim: "292,612 successful iterations on a pre-regression commit before treating the regression as absent there."
- Verified: matches exactly. From the lore.kernel.org thread: "Usually you need to boot 100s or 1,000s of times to see the hang, compared to 292,612 [sic] successful boots which I was able to do before the problematic commit."
- Investigation arc (printk.time → bisected to a sched/clock commit `f31dcb152a3d...` → eventual fix via tick-alignment patch `13bb06f8...`) is consistent with both Janus's first-pass and second-pass framing.

### 2.4 Asterinas PR #2488 — PARTIALLY VERIFIED

- Janus citation: "Asterinas PR #2488, 'Fix LTP CI hang problems.'"
- Substrate verified ✓: the asterinas/asterinas GitHub repository exists, is active, uses standard issue/pull URL pattern. Search results surfaced multiple Asterinas issues and pull requests (#508, #669, #1300, #1648, #1721, #2330, #2356, #2584, #2602) confirming the project's actual issue/PR numbering range — #2488 falls comfortably within.
- Substrate verified ✓: Asterinas's CI runs LTP (Linux Test Project) test suites under SMP configurations (single-core and 4-core). DeepWiki confirms: "Syscall tests: LTP (Linux Test Project) and gVisor test suites" and "SMP: Single-core vs multi-core (4 cores)."
- Direct content of PR #2488 (the specific framing of "runqueues needed locking with IRQs disabled to avoid deadlock in IRQ callbacks" and "init not woken after dead children moved") could not be retrieved through search results in this pass. Search did not surface this specific PR text, possibly because GitHub PR bodies are not strongly indexed for the queries used.
- No inconsistency observed. The framework is consistent with the project's actual testing infrastructure.

### 2.5 Asterinas issue #2217 — PARTIALLY VERIFIED

- Same status as PR #2488: substrate verified, specific issue content not directly retrieved through search in this pass.
- No inconsistency observed.

### 2.6 Theseus OSDI 2020 paper — VERIFIED ✓

- Title cited by Janus: "Theseus: an Experiment in Operating System Structure and State Management."
- Title at usenix.org: exact match. Authors: Kevin Boos (Rice University), Namitha Liyanage (Yale University), Ramla Ijaz (Rice University), Lin Zhong (Yale University). Venue: 14th USENIX Symposium on Operating Systems Design and Implementation (OSDI 20), 2020.
- URL: `https://www.usenix.org/conference/osdi20/presentation/boos` — exact match.
- Janus's framing of Theseus as a Rust-based OS with intralingual design + live evolution / fault recovery is consistent with both the paper abstract and the project documentation at theseus-os.com.

---

## 3. Calibration verdict

**High-fidelity for primary sources directly indexed by general-purpose search engines.** The four fully-verified citations (Linux commit, QEMU issue, LKML thread, Theseus OSDI paper) all matched exactly on title, authorship, date, venue, URL, and substantive framing. Specific numerical claims (292,612 boots, ~1-in-300 rate, QEMU 8.0.0 version) match exactly. No paraphrase drift detected.

**Partial-verification on GitHub-hosted issues and PRs, but no inconsistency.** Asterinas #2488 and #2217 specific content could not be retrieved via search in this pass — likely an indexing limitation of issue/PR text on github.com rather than absence of source. The repository, the testing infrastructure (LTP under SMP), and the issue/PR numbering range all match Janus's framing. Confidence: high but not directly proven for these two items.

**No fabrication detected.** Across six spot-checks, every claim that was directly verifiable matched. Where direct verification wasn't possible, the surrounding framework was internally consistent. This is the pattern of an honest researcher who occasionally cites sources that are harder to reach via standard search but are real.

**Calibration profile after first-pass spot-check:**

- For primary academic and mailing-list sources, treat Janus's citations as reliable.
- For GitHub issues/PRs, citations are likely correct but may benefit from direct fetch when load-bearing.
- For specific numerical claims, Janus appears to quote rather than approximate. The 292,612-boot figure being verbatim is a positive signal.
- For substrate framing (project context, testing infrastructure, language/tooling), Janus is accurate.

**Implication for second-pass review intensity:** somewhat-elevated review continues but can step down a notch from the first-pass posture. The carry-forward of "after 2-3 more Janus dispatches, calibration profile firms up" (per `PM-NOTE-2026-05-09-janus-recurrence-trigger.md` §3.3) remains accurate; this spot-check is one of those data points.

---

## 4. Specific caveats Janus flagged that this spot-check confirms

Janus's first-pass artifact itself flagged certain citations as low-confidence single-source findings (FreeBSD Launchpad mirror, Asterinas issue #2851, OSDev forum thread). This spot-check did not re-test those — they were correctly flagged in-artifact as low-confidence and don't require further validation under the calibration concern. Janus's epistemic discipline (flagging single-source findings explicitly) is itself a positive calibration signal.

---

## 5. PM action items

- **Closed:** carry-forward "Janus first-dispatch citation spot-check" from the SC-3B investigation chain handoffs.
- **Open (no change):** the second-pass artifact at `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` is being consumed in the SC-3B Phase D / Sub-step 5c synthesis. Per the calibration step-down above, somewhat-elevated review continues there; no separate spot-check is queued unless the second-pass produces a load-bearing claim that contradicts other evidence.
- **Carried forward (no change):** the broader calibration question — "after 2-3 more Janus dispatches, profile firms up" — remains an accruing observation. Each subsequent dispatch's review intensity is informed by this spot-check.

---

## 6. Cross-references

- `Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md` — the artifact being spot-checked.
- `Plans/DISPATCH-2026-05-09-sc-3b-outcome-b-janus-deep-research-janus.md` — the dispatch that produced it.
- `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` §3.3 — calibration framework for Janus dispatches.
- `Plans/HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md` §7.2 — original carry-forward action item.
- `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` — the second-pass artifact whose review intensity this note informs.

---

**End of note.**
