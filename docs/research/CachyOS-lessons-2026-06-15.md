<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# CachyOS — what's real, what's hype, and what LogansOS should take

**Date:** 2026-06-15
**Author:** PM (Claude)
**Status:** Research brief (deep-research fan-out, ~14 parallel agents across scheduler /
compiler-build / kernel-tuning / Nvidia-UX / benchmark-skeptic angles + a verification pass).
**One-line:** CachyOS is *fast-by-reputation, assembled-from-upstream*; LogansOS is
*fast-by-falsifiable-measurement, built-from-scratch*. The transferable lessons are about
**method and a few targeted levers**, not about adopting CachyOS's stack (none of which is
ours to adopt — different kernel, different everything).

> **Sourcing caveat (our own byte-oracle discipline, applied to ourselves).** Every primary
> host in this space — phoronix.com, openbenchmarking.org, LWN, LKML mirrors, the CachyOS
> wiki, sunnyflunk — returned **HTTP 403** to the automated fetcher this session. The numbers
> below came through WebSearch's *summaries* of those pages, except where noted as read from
> raw source (the sunnyflunk per-package table via `raw.githubusercontent.com`, Canonical's
> blog, and the CachyOS GitHub READMEs/PKGBUILDs, which fetched cleanly). Per the
> raw-source-KAT rule, **treat the exact percentages as second-hand** until re-pulled with a
> real UA / Wayback. The *directions* are cross-corroborated across many independent sources
> and are high-confidence; the precise digits are not oracle-grade.

---

## 1. TL;DR

- The viral "CachyOS beats Windows 11 by 8–19% in games" numbers are real **but** measure the
  **Linux+Proton+Mesa stack vs Windows** on **AMD** hardware (one YouTuber, n=1 system). The
  *same* tester's **NVIDIA** run had CachyOS ~**24% slower**. Not our story either way — we
  ship neither Proton nor Mesa.
- The honest, Phoronix-grade, **same-base** number is **CachyOS ≈ +5% geomean over upstream
  Arch** (EPYC 9655P, 75 tests, at equal power; +11.6% vs Ubuntu *LTS* is mostly the older
  comparator's stale kernel/packages). Strix-Point laptop run: CachyOS came **third**, behind
  Clear Linux and *plain Debian Testing*.
- Their flagship **x86-64-v3/v4 optimized-repo** feature is **near-inert for general code**
  (Blender/Python/Firefox unchanged; ~1% typical desktop; Canonical concurs ~1%), with real
  wins **only** on SIMD/crypto/codec/compile workloads — and a non-trivial tail of
  **regressions + higher power** (sunnyflunk: lz4 +2.9% slower & +10% power; python +3% slower).
- The famous **BORE scheduler** gives **~0 throughput** vs the default (Phoronix); `scx_lavd`
  *regressed* SC2/Diablo3 ~20% on one config and had a 34 GB memory-leak bug. Scheduler value
  is **latency/frame-time "feel," not FPS** — and *that* is real (scx_lavd +5.2% avg on BG3 on
  Steam Deck, EEVDF latency-nice ~30% better UI input-latency in one config).
- **-O3 is mostly hype at the geomean** (Linux kernel -O3 = **+1.3%**; GCC broad mix ~**3%**;
  Torvalds rejected it, kernel dropped the option in 6.0) but **bimodal** — big wins in tight
  compute loops, regressions elsewhere. **PGO is where the real, repeatable compiler win is**
  (~5–30% CPU-bound), and **AVX-512 is 2–3× on the narrow set that vectorizes** (Embree,
  y-cruncher 2×; OpenVINO >3×).

**The throughline that matters to us:** every credible CachyOS gain is *narrow, workload-specific,
and reproducible by anyone shipping the same kernel+flags* — i.e. "good defaults shipped early,"
not magic. That's an argument **for** our latency-oracle discipline (measure the specific path,
both ways) and **against** chasing headline percentages.

---

## 2. Real vs hype (the load-bearing table)

| Lever | Marketed | Independently measured | Direction for us |
|---|---|---|---|
| x86-64-v3 repos | "5–20%" | ~1% typical desktop; 1–5% on crypto/MD; **regressions + higher power** on non-SIMD code | Targeted, not blanket |
| x86-64-v4 (AVX-512) | "substantial" | wash on general apps (v3 sometimes wins); **2–3× on vectorizable** (Embree/y-cruncher/OpenVINO) | **High-upside on matmul** |
| -O3 | "blazingly fast" | kernel **+1.3%**; broad apps **~3%**; bimodal; rejected by Linus | Per-hot-path, measured |
| LTO | implied large | **~5%** on a single C codebase (Redis); **<1%** extra over -O3 on broad mix | Modest, cheap, keep |
| PGO/AutoFDO | "up to ~10%" (LLVM) | **5–30%** CPU-bound, up to 2× best case; needs representative profile | **Highest compiler leverage** |
| BORE scheduler | "snappiness" | **~0 throughput**; latency-only; can regress games | Latency is the real axis |
| sched-ext/scx | "gaming schedulers" | scx_lavd **+5.2% BG3** *and* **−20% SC2** on another config; stability bugs | Measure both ways |
| "beats Windows" | 8–19% | real on **AMD**, **−24% on NVIDIA**, n=1, whole-stack | Not our comparison |
| Full distro stack | "fastest distro" | **~5% vs Arch**; 3rd behind Clear Linux + Debian on one laptop | Ceiling is ~5–16% |

**Provenance footnotes (who authored what — relevant because it tells us what's a *technique*
vs a *product*):** BORE is **firelzrd's** out-of-tree patchset (CachyOS packages it). sched-ext
is **Meta/Tejun Heo**, mainline since 6.12. x86-64-v3/v4 are a **gcc/glibc psABI** feature
(Fedora & Ubuntu are now adopting v3 variants too). CachyOS's *genuinely original* engineering
is the **build/deploy infrastructure**: parallel per-microarch binary repos at distro scale, a
patched-pacman `Architecture=auto` v3/v4 autodetector, Clang-ThinLTO+AutoFDO kernel build
pipeline, and the Kernel-Manager scheduler-switch GUI. The performance ideas are upstream; the
*packaging and autodetection* are theirs.

---

## 3. ADOPT — the LogansOS shortlist (prioritized)

1. **Extend the latency oracle from inference to scheduling / tail latency.** CachyOS's whole
   real (non-hype) advantage is **frame-time consistency / 1%-lows**, and the industry
   *cannot cleanly measure it* — it's the "feel" everyone asserts and nobody benchmarks. We
   already treat cost as a falsifiable property (BUILD-LO, §4). The lesson: make **p99/tail
   latency under load** a first-class oracle metric, not just mean throughput. This is the
   single most transferable idea, and it's *method*, not code.

2. **Latency-criticality-aware scheduling for Arthur's path — measured both ways.** scx_lavd's
   thesis (boost the latency-critical interactive task under background load) is sound and maps
   directly onto Arthur's hard-latency requirement (Article 29). But the evidence is a
   cautionary tale: it helped BG3 +5.2% and *hurt* SC2 −20%. So adopt the **idea** (a
   latency-criticality hint on the governance/assistant path) behind a **kill-switch with a
   disabled≡baseline regression** (our Rule-1), and prove the win on *our* workload with the
   oracle — never ship it on reputation.

3. **`target-cpu` + LTO + (eventually) PGO on the inference op-kernels — the one place the
   compiler lever is genuinely large.** The OptiPlex 7080 is Comet Lake (AVX2/x86-64-v3; **no**
   AVX-512). Generic desktop code sees ~1% from v3 — *but our hot path is SIMD-heavy matmul/
   dequant*, exactly the workload where AVX-512 hit 2–3× and AVX2 vectorization pays. Concrete:
   build the `ai/ops` kernels with `target-cpu=x86-64-v3` (or `=native` for the local OptiPlex
   bring-up) + LTO now; hold **AVX-512** as a measured target for any v4 silicon we run on;
   treat **PGO** as a later, profile-driven pass (5–30% is worth it once we have a
   representative inference profile). Measure each with the oracle — the kernel-`-O3` lesson
   (+1.3%) is the warning against blanket `-O3` on the *whole* kernel.

4. **GPU/accel roadmap: the `nvidia-open` (GSP, Turing+) path + chwd-style PCI-ID autodetect.**
   When the accel-driver class reaches real GPUs, the open GSP-firmware kernel-modeset driver
   (Turing and newer) is the maintainable target, and CachyOS's **hardware-detection-by-PCI-ID
   → load-the-right-driver** tooling (`chwd`) is the pattern for our autodetect. Not now;
   bank it for the accel lane.

5. **Autodetect + sane-defaults as the Article-28/Arthur bar.** CachyOS's most-defensible
   *product* virtue is "fast and correct out of the box with zero tuning" — the installer
   detects CPU tier, SSD-vs-HDD, EFI, and picks the right repo/scheduler. For LogansOS that
   translates to: the system should **detect its own substrate and self-configure** rather than
   expose knobs. That's already our philosophy; CachyOS is a proof point that it's the right
   bar.

---

## 4. SKIP — and why

- **The entire Linux package/distro layer** — pacman, optimized repos, Proton/Wine/Steam, Mesa,
  Arch tooling, upstream Linux kernel patches (BORE/BBR3/ntsync/bcachefs). We are a sovereign
  from-scratch kernel; none of it is ours to take.
- **sched-ext / BPF-pluggable schedulers as a framework.** It's a *runtime-loadable-scheduler*
  mechanism whose value is letting *users* hot-swap policies. We have one governed scheduler
  with a falsifiable contract; a pluggable-BPF-scheduler surface is complexity-budget we
  shouldn't spend (and its flagship scheduler regressed games and leaked memory). Take the
  *latency-criticality idea* (ADOPT #2), not the framework.
- **Blanket `-O3` / blanket x86-64-v3 across the whole kernel.** Kernel -O3 = +1.3% with
  regressions; Torvalds rejected it for cause. Apply aggressive flags **per hot path with the
  oracle**, not globally.
- **Chasing the headline %.** The "beats Windows" and "5–20%" figures don't survive scrutiny
  (whole-stack / AMD-only / NVIDIA-reversed / general-code-inert). Our credibility is the
  oracle; we don't borrow marketing numbers.

---

## 5. Sources

Phoronix: CachyOS x86-64-v3/v4 repos; EPYC 9655P server preview (11.6% / 5% / 9.1%, 75 tests);
Thelio Major; Framework-13 Strix Point (CachyOS 3rd); CachyOS-vs-Ubuntu-2510/F43; BORE-on-CachyOS;
linux-kernel-O3 (+1.3%); GCC-11 opt-levels (~3%, LTO <1% extra); Clear-Linux-vs-Ubuntu (8.3% /
16%). Canonical: profile-workloads-on-x86-64-v3 (~1% typical; up to 60% on glibc Log2; significant
regressions). sunnyflunk: "x86-64-v3 Mixed Bag" (per-package table, read from raw source — flac
−20%, vorbis −21%, **lz4 +2.9% & +10% power**, **python +3% slower**) + "-O3 Can Transform
Performance." LWN 830300 (kernel -O3 regressed Redis on 5.3); LWN 887842/925371 (latency-nice /
EEVDF). LKML: Torvalds -O3 rejection ("loong history of generating worse code than -O2"); kernel
6.0 dropped `CC_OPTIMIZE_FOR_PERFORMANCE_O3`. sched-ext/scx GitHub #296 (scx_lavd −20% SC2/D3),
#3340 (34 GB leak); LPC 2024 scx_lavd Steam-Deck (+5.2% BG3). GitHub READMEs: firelzrd/bore-scheduler,
CachyOS/linux-cachyos (PKGBUILD: ThinLTO + -O3 default; AutoFDO/Propeller off by default),
CachyOS/scx-manager, CachyOS docker pacman-v4.conf (repo/mirrorlist layout, read directly). arXiv
2507.16649 (PGO 5–30%, up to 2×; sampling <2% overhead). Tom's Hardware / Phoronix AVX-512
(Embree/y-cruncher 2×, OpenVINO >3×). NJ Tech (YouTube) AMD + NVIDIA gaming runs as re-reported by
XDA/TechSpot/Notebookcheck/PCGuide (AMD +8–19%; NVIDIA −24%). Fedora wiki / Red Hat Developer
(x86-64-v3 "−5% to +10%, no discernible difference for most code"). Full URLs in the session
research transcript.
