<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# BREAK GATE — ✅ **MET 2026-08-02. All eight lanes closed. The break begins.**

| lane | closed as |
|---|---|
| DW-2 scene policy (Janus) | `88330e8c` |
| OT-69 CI coverage + reporting fix (Prometheus) | `3f756daa`, `33eae405` |
| M75q out of the QEMU shard pool (Prometheus, PL-ratified) | `47aa35dc` |
| **WiFi `mt7601u`** (Oracle) | `6dda37e2` — integration set |
| **Slice-4 M710q Gen9 EU** (Cerebro) | `6dda37e2` — integration set |
| **Rung-5 persistence** (Chad) | `6dda37e2` — integration set |
| **eth0 at boot** (Chad) | `f898569b` |
| **C1.1 `threads_futex`** (Janus, from Dexter) | `6611d353` |

**Every merged lane cleared the same bar:** gate green and **PM-verified on the exact SHA** (never a pasted URL), own `git show --stat` inspected for phantom deletions, and an **adversarial-audit panel** — cross-family wherever the rule required it. **Six AAGs ran; four found real blockers in code that was already gate-green and stat-clean.** None of those four was catchable by any gate we have.

**What the day actually proved, beyond the merges.** The dominant defect class was not bad code — it was **assertions with nothing behind them**: DW-1's focus-atomicity check that was a printed string, rung-5's `stage=write` literal, OT-69's `|| true` that made a CI job unable to ever report failure, Oracle's op-count assertion that silently encoded the assumption under review, C1.1's `filter_map` that could iterate zero times while printing `ok`, and the three-join assertion that replaced it and was itself unreachable. **Six instances, in six different subsystems, in one day.** The AAG found them; no gate could.

**Three coders corrected themselves in public** — Oracle withdrew a fleet-wide inference after checking the host topology, Cerebro retracted a green signal that favoured his own branch, Janus narrowed a broad finding to the one part that survived re-verification. That is the behaviour that makes the funnel worth its cost.

**The original directive and closeout rules are preserved below for the record.**

---

# BREAK GATE — close out the open lanes, then hold. **PL directive, 2026-08-01**

**Directive (PL, verbatim):** *"After all these open lanes start closing I want to come to a break, so no more dispatches after everyone is green."*

**What this is and is not.** This is the PL calling a break at a defined gate — **not** the PM offering one (the standing forward-momentum rule forbids the PM from proposing a pause, and that rule is unchanged). The break begins when the table below is all-green; until then the PM drives every in-flight lane to close and **opens no new lanes**.

**Rule for the PM while closing out:**
- **Allowed:** artifacts required to *close something already in flight* — AAG lens dispatches for candidate branches, bounce docs, decision answers, merge records, ledger updates.
- **Not allowed:** any dispatch that opens **new** scope. The post-break queue at the bottom is recorded, not dispatched.
- **Exactly one new artifact is anticipated:** the WiFi AAG lens (lane 2), and only because Oracle authored that branch and therefore cannot lens it.

---

## The gate — "everyone is green" means this table is all ✅

| # | Lane | Coder | Candidate | State (2026-08-01) | Closing condition |
|---|---|---|---|---|---|
| 1 | **eth0 at boot** (0.1 rung 4) | Chad (Codex) | `codex/eth0-boot-product-flow-chad` @ `aa6c5b27` | 4-shard `30678862941` **green, PM-verified per-shard on the exact SHA**; stat 13 files +870/−39, zero deletions; **silicon-proven** (real DHCP lease, run 3) | ✅ **AAG RETURNED** (Oracle, artifact `Plans/RETURN-2026-08-01-aag-eth0-boot-product-flow-oracle.md`): **3 findings + 3 notes, none blocking correctness.** Headline: **I-14(c) is the exact inverse of `accel_intel`** — all four `.submit()` sites fence-preceded through `dma_arena::fence`, every buffer from `alloc_in_arena`, so `fence_refusals=0` on run 3 **measures something**. I-14(a) clean. He killed his own strongest lead on record (the AX88179 RX-header hypothesis, refuted *by the lease itself*). **Process miss named by the reviewer: the review was completed 2026-08-01 but delivered only in-session and never committed, so the board correctly read it as unstarted — it silently blocked rung 4 for a day.** **2 merge conditions → Chad:** (N3) his RETURN's I-14(b) argument is computed on `10_000` pump passes but `750_000` shipped — 75×, so the merge record must be corrected; (F1) add **`boot-eth0`** to the OT-69 coverage matrix, else 523 new gated lines land in the exact never-compiled hole. Then rebase → close-gate → 4-shard → PM merge. **Recorded, not blocking:** OT-76 (the `boot_frame_eth0.rs` fork), OT-77 (ASIX vendor-ID-only gating; pre-IOMMU product boot with no `vtd`) |
| 2 | **WiFi `mt7601u`** | Oracle (Claude) | `codex/mt7601u-post-qselect-terminal-jarvis` @ `4f06bdbb` | ✅ **GATE GREEN on the repaired SHA** — run `30716037690`, 11/11 jobs, single attempt. Largest branch on the table | 🔴 **AAG BOUNCED** (Janus, 6 lenses). 5 clean **with reasoning** — incl. the two strongest: DMA fencing clean **by construction** (`EndpointPipe::submit()` takes `FencedDmaAddr`, not `PhysAddr` — unfenced submission is unrepresentable) and **eth0 NOT regressed** (new xHCI entry points all `mt7601u`-gated; CDC-ECM never uses `wait_event()`). Blocker: **`channel_retry_action` unbound from the hostile oracle** — and per Lens 6 that oracle is the **only** verification instrument (gate never compiled this code; QEMU has no MT7601U). `Plans/AAG-BOUNCE-2026-08-01-wifi-retry-authorization-unbound-oracle.md` | ✅ **AAG SATISFIED.** `channel_retry_action(recovery_retry_observation(..))` now bound to `ProductionPolicy` through the shipped `wedge_logic`, with both negative controls, a full truth-table binding assertion, and a new `transport_up_mcu_latched` script isolating the `cea89e6a` world. **Mutation-verified:** forcing unconditional `Retry` fails exactly the two new tests and nothing else. Janus's 5 clean lenses transfer by **code-subtree tree-identity** (`3e47727a`→`4f06bdbb` identical). **MERGE-READY** |
| 3 | **Slice-4 M710q Gen9 EU** | Cerebro (Claude) | `codex/slice4-m710q-hd530-gen9-allowlist-cerebro` @ `72570f72` | ✅ **GATE GREEN, PM-verified 11/11 jobs on the exact SHA** (all 4 shards step-16 `success`, dumps `skipped`, attempt 1). FF-safe. I-14(c) repair + both decisions implemented; `1255b665` was Cerebro's own BAR regression, self-caught and reverted | ✅ **AAG CLEAR** (Jarvis delta, 2026-08-01): *"I-14(c) remains closed; no rebound finding."* Bus mastering granted only after `containment_armed()`, every earlier return precedes `enable_bus_mastering`; `alloc_gpu` returns `ContainmentRequired` with no buddy path for bound hardware; **the repurposed assertion reads real PCI config space** (`debug_command_register()` → CF8/CFC port read of the matched BDF's COMMAND register — not a cached value or printed string); VT-d-off degrades structurally with the breadcrumb distinguishing firmware-off from domain-arming failure; BAR validation correctly scoped to `accel_intel::probe_pci` only. **MERGE-READY** — 2 proof boundaries recorded below, neither a blocker |
| 4 | **Rung-5 persistence** (0.1 rung 5) | Chad (Codex) | `codex/rung5-boot-neofs-product-flow-chad` @ `b1e2c975` | ✅ **GATE GREEN, PM-verified on the exact SHA** (`30682429604`, attempt 1). FF-safe. **F1/F3/F4 PM-confirmed present**; F2 ratified as the *unified NeoFS product volume* (`Plans/DECISION-2026-08-01-unified-neofs-product-volume.md`). F4's new scenario **immediately caught a boot panic before scheduler init** that nothing else covered | ✅ **AAG CLEAR** (Cerebro re-check): *"no blocking findings… Merge."* Mount-after-`init_bsp` traced CLEAN — BSP is a registered thread with preemption armed, **no lock held across a park**, CP-2 respected, ordering-before-userland holds on both paths. **F1 did NOT delete model integrity** — a streaming blake3 `verify_inode_content_hash_cached` survives at read time; F1 removed the redundant-and-catastrophic check and kept the correct one, *a strict improvement*. Degrade is byte-identical to the known-good pre-branch spawn; `89`/`90` do **not** leak the handle. **MERGE-READY** |
| 5 | **C1.1 `threads_futex`** | **Janus (Codex)** — *reassigned from Dexter, weekly usage limit, PL 2026-08-01* | `codex/ot61-c1.1-aag-fix-forward-cerebro` @ `4c29f63a` | **Root framing retired: the exits were 39 at 7b, never 35.** All four old attempts reached check 7. Fixes landed: exit-site tags, no silent `None` join skips, exact three-join assertion, sound 7b discriminator. `threads_futex` **green, full PASS, exit 0**. 🔴 **Gate red on OTHER scenarios** — `dw1_desktop_wm`/`dw2_scene_policy` exit 40, `sys_generate_kv_persist_equivalence` times out. Diagnostics working (serial dumps for all 4 attempts, in the job log — the artifact-quota fix held). **Exit 35 undiagnosed**; Dexter's conclusion was rejected by the PM (false premise — 4 `return 35` sites, not 1; "buffered `say()`" is not a mechanism; only 1 of 4 attempts read). Established: check 7's join is eliminated. ⚠️ **48 commits behind `origin/main`** | **The red is the branch's, not `main`'s** — PM-verified: three independent gates on bases containing DW-1/DW-2 were green today (`4f06bdbb` 11/11, `72570f72` 11/11, `b1e2c975`). Both DW scenarios live in `userland/neo-init/src/main.rs`, the same file C1.1 edits, and share one exit code ⇒ one shared cause in the edit. `sys_generate_kv_persist_equivalence` is separate: measured **84.5 s** today, near budget ⇒ likely contention |
| 6 | **DW-2 scene policy** | Janus (Codex) | — | ✅ **CLOSED.** Merged to `main` as `88330e8c` | — **disk-reclaim** `E:\AI Automation\workers\janus` build tree |
| 7 | **OT-69 CI coverage** | Prometheus (Qwen) | `prometheus/ot69-report-fix` | 🔴 **REOPENED.** Merged as `3f756daa`, then Cerebro found the job **can never report a failure** — `continue-on-error: true` **and** a trailing `\|\| true`, so the step exits 0. Verified: all 4 matrix jobs `success` on `main`, including `gpu-iommu` on a tree where `accel-intel` provably does not compile. `Plans/BOUNCE-2026-08-01-ot69-job-cannot-report-prometheus.md` | 🔴 **PM SPEC WAS IMPOSSIBLE — my error.** `continue-on-error: true` is *defined* to overwrite the step `conclusion` with `success`; the pre-override result lives in `steps.<id>.outcome`, which the **REST jobs API does not expose**. "Step `failure` + job/run `success`" cannot exist. Prometheus's `status=$?` pattern is correct and his step genuinely exits 101. **Revised close condition:** keep `continue-on-error`, emit `::error::` **annotations** (retrievable via `GET /repos/{o}/{r}/check-runs/{id}/annotations` — genuinely machine-readable) + `$GITHUB_STEP_SUMMARY`; falsify at *that* layer |

### Slice-4 — two proof boundaries recorded at AAG clearance (Jarvis, 2026-08-01)
Neither blocks the merge; **the first is a pre-condition of the M710q bare-metal run.**
1. **The refusal scenario does not latch the error KIND.** It asserts "not registered, BME clear" — which an earlier **BAR/MMIO** failure would also satisfy. The hardware-register read has real teeth for the *safety property*, but the scenario cannot prove the **specific `ContainmentNotArmed` branch** fired. Per the blind-hardware rule, *the run must be conclusive whether it passes OR fails* — so an **error-kind latch is REQUIRED before the M710q boot**, not before this merge. The scenario is hardware-only and cannot run in CI, so merging changes nothing; booting without the latch would waste the run.
2. **"CPU floor + Arthur still answer with VT-d off" is structurally established, not exercised.** The 11/11 CI gate does not test it. The **firmware-off M710q refusal run is the operational proof** — an acceptance milestone, per the merge-bar/acceptance-bar split.

**Merge-record note on lane 6:** DW-2's four branch commits appear as "not in `main`" by SHA because the merge was a **cherry-pick**. The code subtrees (`kernel`, `userland`, `tools`, `Cargo.toml`, `Cargo.lock`) are **byte-identical to `main`** — verified, and that identity check is the only reason the lane didn't read as unmerged work. This is the tree-identity rule earning its place a second time.

---

## What does NOT hold the break open

Per the **merge bar vs acceptance bar** rule, the two outstanding bare-metal proofs are **acceptance milestones, not merge gates**, and therefore do not block the break:

- **WiFi LINK-UP on silicon** — the `mt7601u` code merges on gate + AAG; the ledger records that the link is not yet proven.
- **M710q EU byte-equality** — Slice-4 merges on gate + AAG; the EU proof is a milestone. **Blocked on a firmware prerequisite anyway:** VT-d must be enabled on the M710q before that boot, or the GPU correctly refuses (`Plans/DECISION-2026-08-01-slice4-i14c-fail-closed-cerebro.md`, recorded in the M710q runbook).

Holding gate-green branches for a hardware window does not make them safer — it makes them **staler**, and staleness is its own defect source (orphaned dependents, cherry-pick tax, phantom-deletion diffs). Merging makes no claim the feature works; the ledger says what is proven.

---

## Post-break queue — RECORDED, NOT DISPATCHED

Nothing here gets a dispatch until the PL lifts the break. Listed so the break costs nothing in lost context.

| Item | Why it's queued | Dependency |
|---|---|---|
| **OT-69 debt paydown + flip the job blocking** | The coverage job landed NON-BLOCKING; paying down the 27 `mt7601u` clippy errors and flipping it blocking is **OT-69's actual close condition**. Kernel-touching ⇒ main-tier seat. | after the gated surface stops moving |
| **OT-70 `as_bytes_mut` → `&mut self` refactor** | Genuine unsoundness (aliasing `&mut` from a shared borrow). Cross-cutting: `ai/tensor.rs` + all three accel drivers. Needs a THINK before any build. | **after Slice-4 lands** — same files |
| **NeoFS `mkdir`/`unlink`** | `create_regular` needs a pre-existing directory (tree baked by `tools/neofs-mkfs`). **Gates a real editor ⇒ prerequisite of OT-67.** | after rung 5 |
| **0.1 integration + bare-metal showcase** | The closing build: one ISO, one boot, **all five rungs in a single run**. This is what actually ships 0.1. | after rungs 4 + 5 merge |
| **WiFi LINK-UP + M710q EU runs** | Acceptance milestones (above). M710q needs **VT-d enabled in firmware** first. | hardware window + PL joint run |
| **OT-65 / OT-66 / OT-67 / OT-68 THINKs** | install, login/session, document/editor, print — the four 1.0 journeys with **zero code today**. Each needs a THINK before any build dispatch (print especially: zero prior art in-tree). | 1.0 scope, off the 0.1 path |
| **1.0 critical path — the browser C-ladder** | C1.1 → C1.2 → C2 → C3 → SWGL → modern-site battery. **Unchanged and still the 1.0 gate**; 0.1 does not sit on it. | resumes post-break |

---

## Disk reclaim at the break

Every merged lane's build tree is deletable once its work is confirmed on `origin/main`. At the break, reclaim for **every** lane in the table that reached ✅ — the discriminator is unchanged: **merged content on `main` = safe to delete; unpushed in-flight work = the only copy, keep.** Confirmed reclaimable now: `janus` (DW-2). **`prometheus` was reclaimed, then the lane reopened** — `cargo clean` keeps the checkout, so the tree is ready for the fix; this is exactly why `cargo clean` beats deleting the folder for a per-coder tree.

## Amendment — a closed lane can reopen, and one did
OT-69 was marked ✅ and then reopened within the hour on a landed defect. **The gate is the table being green *and staying* green, not each row reaching ✅ once.** Two PM-side lessons, recorded because they generalise:
1. **Verifying a change is SAFE is not verifying it WORKS.** The PM checked the coverage job could not redden `main` (no `needs:`, no `if:`, `continue-on-error: true`) and merged on that — never asking the inverse question, *can it ever report anything?* Both properties need checking; only the first was.
2. **A falsification proof must run against the artifact that reports.** OT-69's proof ran at the **command** level (planted syntax error; old path `Finished`, new path `error: unclosed delimiter`) and passed — while `\|\| true`, sitting between the command and the job's report, kept the job blind. Proving the detector fires is not proving the alarm rings. **Any future dispatch demanding a falsification proof must name the layer the proof is measured at.**
