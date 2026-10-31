<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Slice-4 EU — accept the M710q HD 530 (Skylake Gen9) in the Intel accel probe

**Date:** 2026-07-27 · **From:** Cerebro · **Dispatch:** `Plans/DISPATCH-2026-07-27-slice4-m710q-hd530-gen9-allowlist-cerebro.md`
**Branch:** `codex/slice4-m710q-hd530-gen9-allowlist-cerebro` @ `4b4f9609`
**Base:** rebased onto `origin/main` @ `fee90a81` — **ff-clean, no stacked dependency**

## Base decision (deviates from the dispatch's default — flagged)

The dispatch said "base on Chad's `0eae1fc9` … take the branch or stack a followup on it, your call."
`0eae1fc9` was **not** on `main`, and `main` had advanced 6 commits — all **doc-only** (verified:
`git diff --name-only 70f41c5f origin/main` returns nothing outside `Plans/`+`docs/`). So I **took the
branch**: Chad's single xtask commit is carried forward into mine and rebased onto current `main`. The
result is ff-clean, so the PM merges **one** branch with a trivial `--ff-only` instead of sequencing a
stacked pair. **Chad's commit is in my branch** (`f06cd53b`, "xtask: route slice4 EU log sink to AHCI") —
his original `0eae1fc9` should be considered superseded, not merged separately, or it will duplicate.

## What shipped

1. **`0x1912` accepted.** The bare `matches!` is now a named `SUPPORTED_GEN9_IGPUS: [(u16, &str); 5]`
   table that records *why* one hardcoded thread limit is legal for every entry (below).
2. **Renamed the lie.** `is_supported_uhd630_id` → `is_supported_gen9_igpu_id`,
   `IntelGpuKind::Uhd630Candidate` → `Gen9IgpuCandidate`. The old names would tell a future reader that
   HD 530 is unsupported — the exact wrong conclusion this build exists to correct.
3. **Two capture additions** (blind-hardware "capture-before-you-need-it", both cheap):
   - the attach line now names the bound SKU (`sku=HD530-SKL-GT2`), and
   - the scan now logs Intel display devices the allowlist **rejects**. Run 1 printed only
     `candidates=0`; the device ID had to be recovered by hand afterwards. The next unknown-SKU box is
     now a one-line diagnosis instead of another blind iteration.
   - the iGPU `rev=0x..` (PCI revision) is logged. It was already read into `Device` and never printed.
     **Skylake specifically** gates several i915 Gen9 workarounds on graphics stepping
     (`WaInPlaceDecompressionHang` on `STEP_A0..STEP_H0`), so a stepping-attributable failure would
     otherwise be undiagnosable, and stepping is unrecoverable after the run.
4. **ADR-032 C4** reconciled — its allowlist sentence still claimed UHD 630/Gen9.5 only.
5. **`docs/runbook-baremetal-m710q-gen9-eu.md` §6a** — the trap bank, seeded before the boot.
6. **The allowlist is promoted to a host KAT** (see below) — the coverage gap that let this class
   escape to bare metal in the first place.

## The coverage gap, and closing it (an addition, not in the dispatch)

**QEMU has no Intel iGPU**, so the 4-shard gate *structurally cannot* exercise an allowlist: every
scenario reports `candidates=0` no matter what the table says. That is precisely why a missing device ID
escaped all the way to a bare-metal boot and cost a full iteration cycle. Under the escaped-defect
contract's fourth clause — *the escaped scenario gets promoted* — a green gate on this branch would
otherwise mean nothing about the one line the build is actually about.

So the table moved into a dependency-free `kernel/src/drivers/accel/accel_intel_ids.rs` (the project's
existing `*_logic.rs` + `#[path]`-include pattern) and picked up four host KATs:

| KAT | What it pins |
|---|---|
| `…binds_the_m710q_hd530` | `0x1912` is bound and labelled `HD530-SKL-GT2` — **the exact regression that cost the cycle** |
| `…keeps_the_proven_uhd630_ids` | the four OptiPlex IDs proven on silicon stay bound — widening must never silently drop known-good hardware |
| `…refuses_unlisted_intel_display_devices` | it is still an *allow*list; uses `0x1916`, a real Skylake GT2 ULT part deliberately left out, so the lookup cannot degenerate into "any Gen9" |
| `…agrees_with_the_baked_vfe_thread_limit` | **the load-bearing one** — see below |

**The fourth one is the real prize.** `intel_gen9_4a` bakes **one** `MEDIA_VFE_STATE` Maximum-Number-of-
Threads value (167, bias-1 of 168) for every device it can dispatch to. That is legal *only* while every
accepted device has the same EU count. The KAT asserts the baked field equals the value **derived** from
each entry's own EU column, so adding a GT1/GT3/GT4 Gen9 part now **fails the test** instead of silently
over-programming the thread limit on that SKU — wrongness a blind bare-metal run would surface only as an
unexplained hang. An invariant that was a doc comment is now a regression.

**Falsified, not merely green.** I ran the negative control rather than trusting four passing tests:
removing `0x1912` fails `…binds_the_m710q_hd530`; substituting a 48-EU GT3 entry fails
`…agrees_with_the_baked_vfe_thread_limit` with the actionable message ("has 48 EUs, which does not agree
with the single baked MEDIA_VFE_STATE thread limit — make that per-SKU first"), while the other two
correctly stay green (so it is not a blanket failure). The table was then restored and verified
byte-identical (`cmp`) before re-running.

`MEDIA_VFE_STATE_DW3` was made `pub` for the cross-check; **its value is unchanged**, so the existing
command byte-oracle is unaffected.

## Scope item 2 — is the accept-ID sufficient, or does Skylake need adaptation?

**Answer: sufficient, on the evidence available without silicon.** Not asserted from first principles —
each claim below is pinned to raw upstream source, pulled with `curl` (never WebFetch, per the
exact-bytes rule).

**Provably identical, so the bring-up path transfers:**

| Claim | Raw-source oracle |
|---|---|
| `0x1912` and `0x9BC8`/`0x3E92` are **both `1x3x8` = 24 EU GT2** | compute-runtime `shared/source/dll/devices/devices_base.inl` @ **`24.26.30049.6`** — the pinned finalizer version that built `BLUR_KERNEL_GEN9`: `NAMEDDEVICE(0x1912, SklHw1x3x8, "Intel(R) HD Graphics 530")`, `NAMEDDEVICE(0x9BC8, CflHw1x3x8, …)` |
| Threads/EU = 7 on both ⇒ our hardcoded `MEDIA_VFE_STATE` max-threads **167** (bias-1 of 168) is correct on both | `hw_info_skl.cpp` and `hw_info_cfl.cpp` both: `gtSysInfo->ThreadCount = gtSysInfo->EUCount * 7u` |
| Platform capabilities do not diverge (GGTT/PPGTT, engine mask, MMIO layout) | i915 `i915_pci.c`: `skl_gt2_info` and `cfl_gt2_info` are the **same definition** — `GEN9_FEATURES, .gt = 2` — differing only in the `PLATFORM()` tag |
| `0x1912` is SKL GT2 desktop | `include/drm/i915_pciids.h:357` — `INTEL_VGA_DEVICE(0x1912, info), /* DT GT2 */` inside `INTEL_SKL_GT2_IDS` |
| **VT-d is not quirked off** for this iGPU (I-14(c2) transfers) | `drivers/iommu/intel/iommu.c` — `0x1912` is **absent** from `quirk_iommu_igfx`, which covers gen4 (`0x2a40`, `0x2e0x`) and Broadwell (`0x16xx`) only |

**The finding that inverts the risk framing:** the embedded EU kernels were compiled **`ocloc … skl`**
(`intel_gen9_kernels/README.md` build recipe). The binaries are natively targeted at **Skylake** — the
M710q is the *closer* target and the OptiPlex Gen9.5 was the further one. Byte-equality is expected to
transfer because it was already transferring in the harder direction.

Two of our own hardcodes were the things that could have been GT-shaped, and both check out:
`MEDIA_VFE_STATE` max-threads (identical, above), and `RING_R_PWR_CLK_STATE` — which our driver only
ever **reads**, never programs, so no slice/subslice/EU-enable value is baked to a GT config.

## Scope item 3 — prior-art trap-mine (i915 SKL vs CFL/CML)

We implement **none** of i915's workarounds on either SKU, and Gen9.5 worked without them. So the only
SKU-shaped hypotheses are the ones i915 applies to SKL and **skips** on CFL/CML. Full table in
runbook §6a; the ranked summary:

| Workaround | Register | Standing |
|---|---|---|
| `WaDisableKillLogic:bxt,skl,kbl` | `GAM_ECOCHK \|= ECOCHK_DIS_TLB` | **Sharpest delta.** `gen9_gt_workarounds_init` applies it under `if (!IS_COFFEELAKE && !IS_COMETLAKE)` — i.e. exactly on SKL and *not* on our proven silicon. GAM/TLB path. First suspect on a fault/hang. |
| `WaInPlaceDecompressionHang:skl` | `GEN9_GAMT_ECO_REG_RW_IA` | Hang-class, `STEP_A0..STEP_H0` = effectively every SKL stepping. Correlate with the new `rev=`. |
| `WaDisableGafsUnitClkGating:skl` | `GEN7_UCGCTL4` | Clock-gating; SKL/KBL, not CFL/CML. Intermittent rather than deterministic. |
| `skl_tune_iz_hashing` | `GEN7_GT_MODE` | **Explicitly NOT a suspect** — Intermediate-Z hashing for the 3D rasterizer; our path is GPGPU-only. Recorded so nobody burns a cycle chasing it. |

**Bounded honestly:** these are *hang/stability* workarounds, not functional prerequisites for a single
GPGPU dispatch. The mine narrows where to look **on a failure**; it does not predict one.

## Standing rules

- **Kill-switch — unaffected, and here is the precise reason.** `gpu_compute::blur_backdrop` branches on
  `GPU_COMPUTE_ENABLE` *before* touching the driver: off ⇒ unconditional `blur_pass_h`/`_v` CPU floor, no
  submit, `COMPUTE_ADMITS` never advances. The kill-switch is orthogonal to the allowlist, so EU-OFF
  output stays **bit-for-bit** identical. **The honest nuance:** on the OptiPlex this change is a strict
  no-op (that ID was already accepted). On the M710q, EU-OFF compute results are bit-identical, but the
  device now *probes* — it is registered and its GTTMMADR BAR is mapped. That is the intended effect of
  the build, not a kill-switch leak, but it is a real behaviour delta on that machine and I am not going
  to describe it as "no change."
- **I-14(c):** unchanged. No DMA path was touched; `fence_refusals=0` remains the acceptance criterion and
  VT-d is confirmed un-quirked for this SKU (above).
- **Non-vacuous proof:** unchanged and untouched — `BLUR_WALKER_KICKS>0` AND byte-equality AND
  `fence_refusals=0`. This build removes a gatekeeper; it does not weaken an assertion.

## Evidence

- Both M710q scenarios **build**: `neo-test-accel-intel-blur-silicon` and `…-narrowed`, exit 0.
- **Clippy debt attribution, measured not assumed.** Kernel clippy under the scenario features reports 46
  errors — I checked out Chad's untouched base `0eae1fc9` and re-ran: **46, identical**. My change adds
  zero. (Pre-existing scenario-feature debt; CI's blocking kernel clippy uses
  `neo-test,neo-test-arthur-agentic-actions` and is green.) Flagged as a **cross-seat note**, not fixed
  here — it is out of this dispatch's scope and touching it would widen the diff on a GPU lane.
- Close-gate: **4/4 green** — Step 1 ff-clean vs `origin/main`, Step 2 all four CI host checks
  (fmt, host tests, workspace clippy, blocking kernel clippy), Step 3 SPDX, Step 4 self-stat inspected,
  no phantom deletions.

- **The close-gate caught my own fmt violation** on the new KAT code (`cargo fmt --all --check` FAIL,
  Step 2). Fixed and re-run to 4/4 green. Recording it because last build's lesson was that
  *approximating* the gate is not running it — this is the gate doing its job at the worker, not at merge.

### CI evidence (gate SHA `df3c636e`)

- **4-shard `workflow_dispatch` QEMU gate — GREEN (`conclusion=success`, all 7 jobs):**
  https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/30322578973
  `Host Tests, Format & Lint` + `SPDX License Headers` + `Monolithic Identity Guard` +
  `QEMU Integration Tests` shards 1/4, 2/4, 3/4, 4/4 — all success. No retries, no re-fires.
- **Push CI — GREEN:** run `30322579364` (`conclusion=success`).
- Push CI green on the intermediate SHA `4b4f9609`: run `30321600573` (`conclusion=success`).
- **Two earlier gates were cancelled by me, not failed** — `30321323632` (SHA `8e59d537`) and
  `30321600187` (SHA `4b4f9609`). Each time I added capture/coverage after firing, which changed the SHA.
  A gate on a superseded SHA is not evidence, and runner capacity is the binding constraint, so I
  cancelled rather than let them run to completion on stale code. **Process note on myself:** firing the
  gate before the work was actually finished cost two runner slots. The fix is to fire once, after the
  close-gate passes on the final tree — not to fire optimistically and patch afterwards.

## What this build does and does not prove

**Device-less QEMU cannot exercise HD 530.** A green gate here proves the change did not regress anything,
that the M710q scenarios build, and — via the new host KATs — that the allowlist contains what it must.
It is **not** Slice-4 evidence. The real proof is the next **JOINT
M710q boot** (Janus drives, `docs/runbook-baremetal-m710q-gen9-eu.md`). The single observation that
discriminates unblocked-from-still-blocked on that run:

```
[ kernel ] accel: intel scan complete functions=13 candidates=1 probe_ok=1
[ kernel ] accel-intel: attached Gen9 iGPU candidate bdf=00:02.0 device_id=0x1912 sku=HD530-SKL-GT2 …
```

`candidates=1 probe_ok=1` means the allowlist miss is closed and the run reached the EU path. If it still
reads `candidates=0`, the new rejected-device line names the actual ID and the diagnosis is one line.

## Notes for other seats

- **Chad:** your `0eae1fc9` is carried into this branch as `f06cd53b` (rebased). Merging both would
  duplicate the xtask commit.
- **Pre-existing kernel-clippy debt under the `accel-intel` scenario features** (46 errors, in
  `boot_frame.rs`, `present_backend.rs`, `atrium/*`, `accel_intel.rs`, `idt.rs`, …). Not merge-blocking
  (CI's blocking clippy config is green) but it means `--features neo-test-accel-intel-blur-silicon`
  cannot be clippy-clean-gated today. Worth a consolidation lane.
- `accel-intel` **alone** is not a valid feature combination on any base — `configure_execlist_engine`
  and `dump_execlist_csb` are `neo-test`-gated. Use a `neo-test-accel-intel-*` feature.

## Merge

**No auto-merge**, per the dispatch: PM verify + **AAG with the DMA-fence / memory-safety lens** before it
lands. Blast radius is small (a device-ID table, a rename, three log lines) and no DMA/`unsafe` path was
touched, which the completeness lens should confirm rather than take from me.

**Disk reclaim (after PM confirms merge):** `E:\AI Automation\AltimitNeo\Workers\cerebro` is a reusable
per-worker worktree — run `cargo clean`, do not delete it.
