<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Runbook: CI runner capacity — the QEMU-gate RAM provisioning

**Status:** Operational runbook (PM/PL-maintained). **Created:** 2026-07-05.

The self-hosted CI runner box must have enough **container-visible memory** or the
4-shard QEMU gate thrashes to a mystery 240-minute cancel. This runbook is the
durable record of the fix, the apply procedure, and the symptom table — paired
with the CI **pre-flight RAM guard** (first step of the QEMU shard job) that fails
FAST with an explicit *INFRA, not test* verdict when the box is under-provisioned.

Doctrine (why a guard, not just a doc): the OT-35 precedent — **infra is never a
test verdict** — and its hermeticity mirror: a red gate must mean *the code is
wrong*, never *the host was starved*. A starved runner should fail in seconds with
correct attribution, not burn an hour and cancel.

---

## The measured reality (2026-07-05)

- **Box:** one machine — Intel i9-9900K, **8 physical cores / 16 threads**, 128 GB
  physical RAM.
- **All three self-hosted runners run on this one box**, in one WSL2 instance
  (`/opt/actions-runner{,-2,-3}` — the `gaming-pc-2`/`gaming-pc-3` names are
  cosmetic, not separate machines). GitHub runs **one job per runner**, so up to 3
  QEMU shards run concurrently on these 8 cores.
- **Docker/WSL2 is capped at 16 GB** by `%USERPROFILE%\.wslconfig` → `memory=16GB`
  (of the 128 GB physical). Every container sees `MemTotal ≈ 16 GiB`.
- Each QEMU shard = a kernel+userland `rustc` build (~2–3 GB peak) + a QEMU guest
  (512 MiB for ~every scenario; 4–6 GiB for the real-brain scenarios, which
  **skip in CI** — their `.neom` models are not in git) + the buildx layer cache +
  container overhead. Three concurrent shards' builds alone push ~9 GB; with
  guests, caches, and Docker/system overhead the working set exceeds 16 GB under
  load → **memory pressure → swap/thrash → every build and QEMU run slows**, and
  the heaviest shard (shard 4, 65 scenarios) tipped past the then-current 70-min
  job timeout.

## Current fleet (2026-10-26): read this first

This section supersedes the fleet tables below. Older sections name runners that are gone
(`ryzen-3/4`, and `ryzen-5/6` on the M75q under Docker/WSL2).

| Runner | Machine | Labels | Memory floor |
|---|---|---|---|
| `ryzen-1` | main Ryzen 9 box | self-hosted, Linux, X64, **qemu-shard**, light | 32 GiB |
| `ryzen-2` | main Ryzen 9 box | self-hosted, Linux, X64, **qemu-shard**, light | 32 GiB |
| `m75q-1` | Lenovo M75q, Ryzen 5 PRO 3400GE, 8 threads, 62,221 MiB | self-hosted, Linux, X64, **qemu-shard**, light | 32 GiB |
| `m75q-2` | same M75q | self-hosted, Linux, X64, light | 32 GiB |
| `m910q-1` | Lenovo M910q, i5-6500T, 4 cores, 15,881 MiB | self-hosted, Linux, X64, light; **qemu-shard** once the 14 GiB floor is on `main` | 14 GiB |

The memory floor is the `neo-test` pre-flight (`.github/workflows/ci.yml`, "Pre-flight — runner memory
capacity"). It only runs in QEMU shard jobs, so a `light`-only runner never meets it. 32 GiB is the default
`*)` arm. `m910q-1` has its own arm. An xtask host test (`tools/xtask/src/ci_memory_floor_tests.rs`) fails if
this column and the pre-flight's arms disagree, so change both together.

**Where jobs land:**
- Every job on `runs-on: ${{ fromJSON(vars.CI_RUNNER_LABELS ...) }}` (`CI_RUNNER_LABELS = ["self-hosted","light"]`)
  can land on any of the five runners. That covers nightly-skip, spdx-lint, monolithic-identity, host-checks,
  product-build and the feature-gated matrix.
- `neo-test` (the 4-shard QEMU gate) runs on `[self-hosted, qemu-shard]`: `ryzen-1`, `ryzen-2`, `m75q-1`, and
  `m910q-1` once its label is back. Never relax it to a bare `self-hosted`.
- **Why the M75q and M910q can take shards now** (`Plans/NOTE-2026-10-26-qemu-capacity-trials-m75q-m910q.md`):
  both ran the gate's scenarios through the non-gating `qemu-trial.yml` on 2026-10-26 on bare-metal Ubuntu.
  `m75q-1` passed all four shards with no retries or timeouts; its tightest run was 95.8 of 180 s (1.88×).
  `m910q-1` passed shard 1; its tightest run was 73.5 of 180 s (2.45×). This supersedes
  `Plans/DECISION-2026-08-02-m75q-out-of-qemu-shard-pool.md`, which was about the M75q's old 32 GB WSL2
  setup (`ryzen-5/6`, two runners contending for 4 cores). Only one M75q runner (`m75q-1`) carries
  `qemu-shard`, so the box never runs two shards at once.
- **Why `m910q-1` gets 14 GiB instead of 32:** it is one bare-metal runner, so it runs one shard at a time
  with no shared VM. The 32 GiB default is 2× the 16 GiB thrash point measured with three concurrent shards
  on one multi-runner WSL2 box, which doesn't describe this host. Measured peaks: QEMU RSS 1,024 MiB (M910q
  shard 1) and 1,049 MiB (M75q, all shards); whole-host memory in use 1,865 MiB on the M910q, and 6,777 MiB
  on the M75q with `m75q-2`'s light jobs running alongside. 14 GiB is 2× the worst of those and fits under
  the M910q's 15.5 GiB. Its 15.5 GiB fails the 32 GiB default in under a second (batch-2 gate `40383263248`,
  attempt 1, shards 1 and 4), which is why its label came off until this arm merged.
- If a QEMU timeout ever shows up only on `m75q-1` or `m910q-1`, compare it against the trial's headroom
  before calling it a code failure: these hosts are slower than the Ryzen box, and the 180 s budget is
  the same everywhere.

**The new hosts:**
- Bare-metal Ubuntu 24.04 LTS, headless, dedicated to CI. No Docker/WSL2 layer.
- Hosts: the M75q (runners `m75q-1`, `m75q-2`) and the M910q (runner `m910q-1`). Their LAN addresses and
  admin account are kept in the PL's own notes, not in the repo; the runners never use the admin account.
- Runners run as the non-admin user `logans`: no sudo, **in the `docker` group** (since 2026-10-26). Each is a
  systemd service `actions.runner.SecondStageTurbine-LogansOS.<name>`. Agent 2.338.1.
- Installed: build-essential, pkg-config, libssl-dev, python3, python3-pip, clang, lld, git, curl, jq. There is no
  system Rust; jobs bootstrap rustup and the pinned toolchain into the workspace. Workspaces persist between jobs.
- **Docker is Docker CE from `download.docker.com`, not Ubuntu's `docker.io`** (installed 2026-10-26 on both). Now:
  docker-ce / docker-ce-cli 29.9.2, containerd.io 2.4.2, docker-buildx-plugin 0.38.1, docker-compose-plugin 5.6.1.
  The install replaced the distro `docker.io` 29.1.3 (no containers existed, so nothing was lost) and the runner
  units were restarted so their processes carry the `docker` gid. Verified as `logans` on both boxes: `docker info`,
  `docker compose version`, `docker buildx version`, and `docker run --rm hello-world`.
- **Non-interactive-apt gotcha (hit on both boxes):** over plain SSH (no TTY) the docker-ce postinst prints
  `Could not execute systemctl` from `deb-systemd-invoke` and leaves `docker.socket` `failed`, so `dockerd` exits
  with `no sockets found via socket activation`. The apt run still reports success; the daemon does not start.
  After any such install, run — then confirm — this, and check `docker info` before calling the box ready:
  `sudo systemctl daemon-reload && sudo systemctl reset-failed docker.socket docker.service && sudo systemctl start docker.socket && sudo systemctl enable --now docker && systemctl is-active docker.socket docker`
- **crates.io downloads failing on the mini-PCs (2026-10-26):** cargo failed index downloads before compiling
  anything, with other jobs on the same boxes green and the Ryzen box never hit: `m910q-1` twice in two minutes
  with `curl failed … [16] Error in the HTTP2 framing layer`, and `m75q-1` once with `[56] Connection reset by
  peer`. Both workflows now set `CARGO_HTTP_MULTIPLEXING=false` and `CARGO_NET_RETRY=10` at the top level
  (`ci.yml`, `qemu-trial.yml`), and pass them into the QEMU step's
  container. If the errors come back anyway, it's the mini-PCs' network path: re-run once, then tell J.c.
- **OVMF for `vm_installed_two_boot` (OT-87 R1, 2026-10-27):** the scenario boots from a disk under UEFI, using the
  firmware in the dev container, never the runner host's. That firmware is the edk2 build QEMU v10.2.3 ships
  (edk2-stable202408.01, commit `4dfdca63`), which the Dockerfile's QEMU `make install` puts in
  `/usr/local/share/qemu`. Pinned bytes:
  - `edk2-x86_64-code.fd`: SHA-256 `33090cc07675baa5190d9f1e84bf5176b33bcbfa9bacac522961150cdb6dbb2a`
  - `edk2-i386-vars.fd`: SHA-256 `5d2ac383371b408398accee7ec27c8c09ea5b74a0de0ceea6513388b15be5d1e`

  The Dockerfile checks both hashes, so a different firmware fails the image build. xtask checks the same two files
  against BLAKE3 pins (`tools/xtask/src/vm_image.rs`). A scenario failure that starts `INFRA: no OVMF found` or
  `INFRA: the OVMF … is not the pinned build` is the container, not the code.

**Triage:** if a light job fails **only** on `m75q-*` / `m910q-*` because a system tool is missing, it's infra.
Tell J.c. the tool, and he installs it. Don't change the job. He owns the boxes: reboots, packages, taking a
runner offline, labels. Labels can also be changed in GitHub → Settings → Actions → Runners. **First datum:**
`m75q-1` passed the `preemptible-liveness` feature arm on 2026-10-25. Watch the first heavy host jobs on the
new runners (cargo test, clippy, product build).

## Fleet plan (post-PC-swap, 2026-07-13)

The 128 GB gaming-pc is gone; the fleet is now three small boxes with distinct roles:

| Box | Specs | Role | Status |
|-----|-------|------|--------|
| **Main / `ryzen-1..4`** | **128 GB DDR5** (2 of 4 DIMMs; headroom to 256), Ryzen 9 9900X (12c/24t) | **Primary CI fleet — 4 concurrent runners** (WSL2 capped 96 GiB, ~24 GiB each) running the full 4-shard QEMU gate in ONE ~20-min wave | **★ ONLINE — 4-runner concurrent fleet PROVEN 2026-07-19** (run `29680566823`, all 4 shards one-per-runner, ~19.5 min, each passed the restored 32 GiB guard) |
| **M75q-1 / `ryzen-5..6`** | **64 GB (32+32, upgraded 2026-07-23)**, Ryzen 5 PRO 3400GE (**4c/8t — UNCHANGED**), 256 GB NVMe | **Secondary CI host — 2 Docker runners**, each capped at 4 CPUs; WSL now exposes **~60 GiB** (was 29.3) — the 28 GiB named-runner guard has wide headroom, **memory pressure eliminated** | **ONLINE 2026-07-21** (`<runner-host>`, (LAN address in the PL's notes)); **RAM 32→64 GB 2026-07-23** |
| **M710q** | 16 GB, i5-6500T (4c/4t), **Intel HD 530 (Gen9)** | **Bare-metal Gen9 GPU target for OT-63 Rung 1** (its iGPU matches the OptiPlex UHD 630 family — the ONLY spare box with the right GPU; protects the OptiPlex from blind-hardware bricking). 16 GB fine for a boot target, NOT a QEMU-gate runner. | Waiting on PSU (ThinkCentre-Tiny 65W slim-tip) |

The M75q-1 supplies two additional slots without changing the primary host. After
the **2026-07-23 upgrade to 64 GB**, its physical RAM becomes **~60 GiB
container-visible** after Windows/WSL2 overhead (was 29.3 GiB), so `ryzen-5` and
`ryzen-6` now clear the 28 GiB preflight floor with wide headroom while the primary
`ryzen-1..4` fleet retains the 32 GiB floor. The M710q is not a runner; it remains
reserved for GPU-target duty.

**DONE (2026-07-19): main box → 128 GB, 4-runner concurrent fleet LIVE.** The main box (128 GB / 12-core Ryzen 9) runs the full concurrent 4-shard gate alone in ~20 min. The bring-up as executed (Chad, verified by PM run `29680566823`):
1. ✅ `.wslconfig` → `memory=96GB` + `swap=32GB`, `wsl --shutdown` (docker MemTotal 94.29 GiB confirmed).
2. ✅ Registered **4 runners** (`ryzen-1..4`, separate `~/actions-runner-N` dirs, one Ubuntu distro, 4 systemd services) → 4-shard gate runs concurrently in ONE wave (measured ~19.5 min; 96 GB/4 ≈ 24 GB each, 12 cores/4 = 3 cores each).
3. ✅ **Guard restored 20 → 32 GiB** (`ci.yml`, commit `7ae224e`, after the 96 GB `.wslconfig` went live) — all 4 shards passed the 32 GiB pre-flight in the proving run.
4. **Remaining (bonus, not necessities):** M75q-1 (PSU) → runner #5 (headroom / a concurrent 2nd gate); M710q (PSU) → Gen9 GPU target for OT-63 Rung 1.

**Infra failure mode banked (2026-07-19): a host reboot can leave WSL with `/mnt/c` EIO errors + Docker unreachable**, reddening whichever shard lands on that runner (seen once as a shard-3 fail). This is **infra, not a code escape** (runbook-exempt — re-fire, do not revert): `wsl --shutdown` + restart the distro/Docker, confirm `docker info`, then re-fire the gate. The clean same-SHA re-run is the proof of recovery.

**Infra failure mode banked (2026-07-21): M75q concurrent-QEMU self-contention.** The M75q is a **4c/8t** box hosting **two** runners (`ryzen-5`/`ryzen-6`), each capped at 4 CPUs. When the 4-shard scatter happens to land a QEMU shard on **both** M75q runners at once, the two TCG guests oversubscribe the one physical box and one can hit a **QEMU timing-timeout** — observed once on `ryzen-6` while `ryzen-5` ran a shard. Discriminator: it is **contention/timing, NOT OOM** — the host stayed at ~27 GiB free with **zero swap** (14 GiB cap held; no memory thrash), so a timing-timeout on `ryzen-5/6` with healthy memory = **re-fire (infra-exempt), do not treat as a code failure or a memory-guard failure.** A single M75q QEMU shard runs clean (proven: `ryzen-5` shard-3/4 green in run `29826380103`, 28 GiB preflight passed). If concurrent-QEMU contention timeouts recur, the fallback is **one QEMU runner on the M75q** (all 4 cores, 28 GiB — one reliable slot, no self-contention) rather than two contending 14 GiB runners; not needed yet (one datum ≠ a pattern). The host-aware 28 GiB preflight is the correct memory fix; do NOT lower the big-box 32 GiB floor.

**SUPERSEDED (2026-08-02, PL-ratified): the M75q is OUT of the QEMU shard pool entirely — the one-runner fallback above is NOT sufficient.** Measurement (Oracle, OT-74 scrape: 25 runs / 58 shard-jobs / 284 scenarios) found a **stronger and different** failure than the banked self-contention mode. `sys_generate_kv_persist_equivalence` has **never completed on the M75q — 0 of 16 attempts, 4/4 shard reds — including when only ONE M75q runner was busy**, while failing attempt 1 **0 of 5 times** on the main Ryzen 9 box. **On that host it is not flaky; it is deterministic failure**, and the mechanism is capacity rather than contention: main-box completions spread **84.5 → 116.6 s against a 180 s budget** (only 1.5–2.1× headroom) and the M75q is roughly **2× slower**. It does not *sometimes* miss — it **cannot fit**. **Therefore the documented fallback (one QEMU runner instead of two) does not help**: reducing self-contention cannot rescue a scenario that fails with the box otherwise idle. **The remedy is exclusion:** `neo-test` is label-gated to `runs-on: [self-hosted, qemu-shard]` (`ci.yml`, merged `47aa35dc`), with the `qemu-shard` label applied to **`ryzen-1..4` only**. The M75q **stays in the fleet** for SPDX / monolithic-identity / host jobs, where it is reliably green. **Rationale:** a runner that deterministically fails a gate scenario is not capacity — it is a 4-in-N chance of a **false red on every gate**, and each false red costs a ~35-minute re-fire on the binding constraint while training everyone to read reds as noise. Raising the budget instead would tax every run to accommodate one half-speed box. **Cost accepted:** QEMU shard capacity drops 6 → 4, so a 4-shard gate now saturates the pool and concurrent gates serialize. Four runners producing trustworthy verdicts beat six producing a coin-flip red. **⚠ Label-order hazard:** the labels must exist on `ryzen-1..4` **before** the `ci.yml` change lands, or no runner is eligible and every QEMU shard queues indefinitely. **Process note worth keeping:** the 2026-07-21/23 entries above already recorded that the M75q issue was **CPU-timing, not memory**, and that **more RAM would not fix the CPU axis** — and that was not re-read during the 2026-08-02 diagnosis, so a memory theory was pursued and a RAM bump applied before the measurement settled it. The banked discriminators were right and under-used. **Read this section before diagnosing an M75q symptom.** Full rationale: `Plans/DECISION-2026-08-02-m75q-out-of-qemu-shard-pool.md`.

**UPDATE (2026-07-23): M75q RAM 32→64 GB — memory axis resolved, CPU axis unchanged.** The upgrade eliminates any memory-pressure/OOM risk on the M75q (now ~60 GiB visible; two concurrent 28-GiB-preflight shards fit with room to spare) and makes the **one-reliable-runner fallback rock-solid** (one runner, all 4c/8t, ~60 GiB). **But the observed self-contention was diagnosed CPU-timing, NOT memory** (host had ~27 GiB free at the event), and the box is **still 4c/8t** — so the RAM upgrade does **not** by itself add concurrent-QEMU *throughput* or guarantee the concurrent contention is gone. If a `ryzen-5/6` timing-timeout recurs under concurrent QEMU, it is CPU-oversubscription (not memory now, with certainty) → the fix remains the documented **one-QEMU-runner-on-the-M75q** fallback (all 4 cores, no self-contention), not more RAM. Net capacity for concurrent *kernel gates* is essentially unchanged (main box still runs one 4-shard wave); the real freed slot this cycle came from moving host-checks to `ubuntu-latest` (`d6dc133`), not the M75q RAM.

## M75q Docker/WSL2 host profile (`ryzen-5..6`)

Host facts:

- Windows 11 Pro host `<runner-host>`, SSH
  `admin@<LAN address>`; Ryzen 5 PRO 3400GE (4c/8t), 32 GB RAM, 256 GB NVMe.
- Ubuntu runs under WSL2. Native Docker Engine runs inside Ubuntu; Linux
  `hello-world`, Docker Compose, Buildx, and a cold repository dev-image build
  must all pass before registration.
- `/etc/docker/daemon.json` disables Docker 29's containerd snapshotter
  (`{"features":{"containerd-snapshotter":false}}`) so the daemon uses
  `overlay2`. On this WSL kernel the snapshotter produced empty BuildKit
  executables and `exec format error`; a Buildx bootstrap is the discriminator.
- `%USERPROFILE%\.wslconfig` requests `memory=30GB`, `processors=8`, and
  `swap=16GB`. A 32 GB Windows host exposes about 29.3 GiB to WSL; this is
  expected and is why the named-runner preflight floor is 28 GiB.
- Windows Task Scheduler task `AltimitNeo-WSL-CI` runs at system startup under
  the Admin identity (S4U) and keeps Ubuntu resident with Docker started.
  Without it, an SSH-started WSL session exits when idle and Docker runners
  repeatedly deregister.
- Docker containers `gh-runner-ryzen-5` and `gh-runner-ryzen-6` use
  `myoung34/github-runner`, `--restart unless-stopped`, `--cpus 4`,
  `--memory 14g --memory-swap 16g`, separate persistent config volumes, and the
  host Docker socket. Each work directory is a WSL-host bind mounted at the
  identical container path (`/opt/actions-runner-work/ryzen-N`) and passed as
  `RUNNER_WORKDIR`; Docker Compose otherwise resolves the container-only path
  on the host and mounts an empty workspace. Set
  `CONFIGURED_ACTIONS_RUNNER_FILES_DIR=/runner` and
  `DISABLE_AUTOMATIC_DEREGISTRATION=true`; otherwise every container restart
  replaces its registration and can strand an offline/busy job lease. Do not
  add a third runner to this 4-core/32 GB host.

Onboarding checklist for another Windows Docker runner host:

1. Record physical RAM/CPU/disk and confirm virtualization, WSL2, Ubuntu, and
   Linux Docker. Run `docker run --rm hello-world` from the runner distro.
2. Set `.wslconfig` with Windows headroom, run `wsl --shutdown`, restart the
   distro, and measure `/proc/meminfo` from the actual runner container.
3. Cold-build the repository Dockerfile and run `cargo --version` inside the
   resulting image. Confirm Compose and Buildx from the runner container. The
   workflow bootstraps the pinned Rust toolchain when the remote cache is absent;
   a new runner must prove this cache-miss path rather than rely on manual seeds
   that `actions/checkout` removes.
4. Create one config volume and one identical-path host bind work directory per
   runner. Size total CPU/memory caps to the physical host; never infer capacity
   from the requested WSL limit.
5. Create and test an at-startup keepalive task so WSL/Docker survive the SSH
   session. Reboot-test this before treating the host as unattended capacity.
6. Generate a short-lived repository registration token, pass it only at
   container creation, and never record it in git or this runbook. Verify the
   expected `self-hosted`, `Linux`, and `X64` labels in GitHub.
7. Confirm at least 15 GiB Docker-host disk is free. Retain the workflow's
   buildx-volume/cache cleanup; on this 256 GB disk, inspect `docker system df`
   regularly and prune only unused state.
8. Fire `workflow_dispatch` at a quiet time. Close only when a QEMU shard names
   the new runner and is green, and the pre-existing runners remain online.

The named-runner 28 GiB workflow guard must land before leaving these runners
online for arbitrary branches. A branch using the older global 32 GiB guard
will correctly reject this 32 GB physical host at preflight; keep the new
containers stopped until the compatibility change is on that branch.

## Interim single-runner 32 GB provisioning (post-PC-swap, 2026-07-13)

**The 128 GB "gaming-pc" box was lost in a PC swap.** The replacement is **32 GB physical /
AMD Ryzen 9 9900X (12 physical / 24 logical cores)** — CPU-rich, RAM-starved (the inverse of the old
box). A 32 GB box **cannot give WSL2 the 32 GiB the standard guard demands** while keeping Windows
alive, so the fleet model changes until more RAM arrives:

- **Run exactly ONE self-hosted runner.** With one runner, GitHub schedules the 4 QEMU shards
  **serially** (one job at a time) — so only a *single* shard's working set (~12–16 GiB: one `rustc`
  build + one QEMU guest + caches) is ever resident, which fits in 32 GB. The gate takes ~4× the
  wall-clock (shards serial, not concurrent), but it is correct and green-able. The 12-core Ryzen
  chews a single 4-vCPU (`NEO_SMP=4`) shard fast, so CPU is not the bottleneck; RAM is.
- **`%USERPROFILE%\.wslconfig`:** `[wsl2]` `memory=24GB` + `swap=16GB` (swap absorbs rustc link
  spikes without OOM). Leaves ~7 GiB for Windows — fine during a drained gate window.
- **The pre-flight guard is lowered to 20 GiB (interim)** in `ci.yml` — the single-shard floor
  (24 GiB WSL > 20 GiB guard passes; a real under-provision still fails fast). **REVERT to 32 GiB +
  multi-shard concurrency when a big-RAM box returns** (add RAM to this box — 64–128 GB DDR5 is
  cheap on this board and restores the concurrent-shard model, or a dedicated runner box).
- Everything else below (WSL-integration re-verify, disk hygiene, stuck-runner cycle) still applies.

**This is a provisioning tourniquet, not a hygiene relaxation** — the guard still fails fast on a
genuinely-starved box; it just matches the single-serial-shard reality instead of the retired
3-concurrent-shard one.

## The fix — raise the WSL2 memory cap

Edit `C:\Users\<user>\.wslconfig`:

```ini
[wsl2]
memory=96GB        # was 16GB — leaves ~32 GB for Windows on a 128 GB box
processors=16      # all logical cores (explicit)
swap=32GB          # safety net for rustc link spikes
```

### Apply procedure (must be done while the box is idle)

```powershell
# 1. Drain: confirm no CI gate is mid-run (a running gate would be aborted).
gh run list --limit 8 --json status,event --jq '.[] | "\(.status) \(.event)"'
#    Cancel any in-flight run you don't want killed abruptly:  gh run cancel <id>
# 2. Stop the runner services cleanly (optional but tidy).
wsl -e bash -lc "sudo systemctl stop 'actions.runner.*'"
# 3. Restart WSL to apply the new cap (this recycles Docker + the runners).
wsl --shutdown
# 4. Verify the new total the CONTAINERS will see:
docker info --format '{{.MemTotal}}'        # ~103079215104 for 96 GiB
docker run --rm alpine sh -c 'free -g | awk "/Mem:/{print \$2\"GB\"}"'
#    …or the runner-VM view the guard reads:
wsl -e bash -lc "awk '/^MemTotal:/{printf \"%.1f GiB\n\", \$2/1024/1024}' /proc/meminfo"
# 5. RE-VERIFY DOCKER'S WSL INTEGRATION INTO THE RUNNER DISTRO — do NOT skip.
#    `wsl --shutdown` recycles Docker Desktop's backend, and its per-distro WSL
#    integration (the injected `/usr/bin/docker`) can come back INACTIVE, so the
#    engine works from Windows while the runner distro has no `docker` at all —
#    every QEMU shard then fails at "Run QEMU integration tests" with the pre-flight
#    guard already GREEN (it reads /proc/meminfo, not docker). This bit us applying
#    this very cap (2026-07-05). Test the runner distro's docker directly:
wsl -d Ubuntu-24.04 -e docker version --format '{{.Server.Version}}'
#    If it prints "command 'docker' could not be found in this WSL 2 distro":
#    Docker Desktop → Settings → Resources → WSL Integration → toggle the runner
#    distro OFF (Apply & Restart) then ON (Apply & Restart) to force re-injection;
#    re-run the check until it prints a version.
# 6. Resume: the runner services auto-restart; re-fire any gate you cancelled.
```

> ⚠️ Never `wsl --shutdown` while a gate is live — it aborts every running job and
> container on the box (including the runners themselves). And ALWAYS run step 5
> after the shutdown: a broken WSL integration makes the runners docker-less while
> the engine still answers from Windows — a silent shard-failure trap.

## The pre-flight RAM guard (CI/harness side)

The **first step** of the QEMU shard job (`.github/workflows/ci.yml`, the
`neo-test` job) reads the runner-VM `MemTotal` from `/proc/meminfo` — the same WSL2
cap the shard's container will experience — and asserts it is **≥ THRESHOLD**. On
failure it **exits 1 within a second** with a GitHub `::error` annotation that
(a) states this is **INFRA, not a test failure**, (b) prints **observed vs
required**, and (c) points here with the one-line remediation. It is **not**
`continue-on-error` — fast failure with correct attribution is the entire point.

**Scope:** the guard is on the QEMU shard job only (which runs on
`workflow_dispatch` / `schedule` / `main` — the gate where the thrash-timeout
manifests), **not** the every-push `host-checks` job, so a temporarily
under-provisioned box does not block routine branch pushes. The host build is also
RAM-sensitive but far below three concurrent QEMU shards; if capacity ever changes
such that host-checks thrashes, add the same guard there.

### THRESHOLD and its rationale

**THRESHOLD = 32 GiB by default; per-runner floors are in "Current fleet" above** (`m910q-1` is
14 GiB, a single bare-metal runner). Reasoning for the default, from measurement: 16 GiB demonstrably thrashes
(three concurrent shards' working set exceeds it → swap → the shard-4 tail tipped
the then-current 70-min timeout). 32 GiB is 2× that — it comfortably holds three concurrent shards
(~5 GiB each: ~3 GiB build + ~1 GiB guest + cache/overhead ≈ 15 GiB) plus Docker and
system overhead, with real headroom and no swap. It is the gate's **hard minimum**;
the `.wslconfig` fix above sets **96 GiB** for extra comfort (future real-brain
scenarios in CI, a fourth runner, larger link peaks). Any value ≥ 32 GiB passes the
guard. **To change it:** edit `REQUIRED_GIB` in the guard step, and update this line and the "Current
fleet" table (the xtask test checks the table).

## Symptom table

| Symptom | What you see | Cause | This runbook's fix |
|---|---|---|---|
| **Mystery long-run cancel** | A QEMU shard job cancelled at ~`timeout-minutes` (240) with 3 sibling shards green; `run_attempt=1`, no test `event=FAIL` | The heaviest shard's wall-clock, slowed by swap-thrash under 3-concurrent-shard memory pressure, exceeds the 240-min job timeout | Raise the cap; the guard would have failed it fast with the real reason |
| **exit-137 / runner loss** | A job dies with exit code 137 (OOM-kill) or the runner drops offline mid-run | Two heavy guests + concurrent builds momentarily exceed the 16 GiB cap → the OOM killer reaps a container (or the runner) | Raise the cap; the guard fails fast before the doomed run |
| **shard-4 tail contention** | The last/heaviest shard runs 2–3× slower than solo; only red under load, green when run alone | 3 QEMU × 4 vCPU oversubscribe 8 physical cores AND the working set pressures 16 GiB → thrash | Raise the cap (removes the memory-pressure half); see the CPU note below |
| **`arthur_memory_encrypted` never terminates** | The scenario reaches spawn then emits no PASS/FAIL for the full 420 s under a contended runner (**OT-43**) | A **real** intermittent post-spawn hang that **contention amplifies** — the thrash makes the racy window far likelier to trigger (Jarvis forensics, 2026-07-05) | Raising the cap reduces the amplification (less thrash ⇒ the race triggers less); the underlying bug is tracked separately as OT-43 — the RAM fix does **not** close it, only lowers its trigger rate |
| **std overlay: "names neoplutarch, but the overlay does not route it"** | A `std_*` scenario fails in `apply-overlay.sh` before any test runs: `apply-overlay: FAILED: <seam>/mod.rs: names neoplutarch, but the overlay does not route it (toolchain changed?)` | Before OT-61 C2.1b the overlay patched the **shared** toolchain rust-src in place and restored only the seams on its own list. A runner (or another checkout) that built a newer overlay leaves seams patched that an older branch's overlay does not know, so the older branch trips its own routing audit. Infra, not the branch's code | Rebase the branch onto `main`. From C2.1b on, xtask applies the overlay to a per-build copy (`userland/std-real-smoke/target/neo-rust-src/library`, build-std pointed at it via `__CARGO_TESTS_ONLY_SRC_ROOT`), so the toolchain is no longer patched; residue already in a toolchain is reset by the overlay's restore-every-backup step. Manual clean: copy each `*.neo-pristine` over its file and delete the `neoplutarch` modules under the toolchain's `library/std/src` |

## Note — RAM is the provisioning fix, not the whole story

The box is also **CPU-bound**: 3 runners × 4 vCPU (`NEO_SMP=4`) = 12 vCPU
oversubscribe 8 physical cores even with unlimited RAM. Raising the cap removes the
**memory-pressure** half of the slowdown (the half that causes the OOM/thrash class
above) but not the SMT-oversubscription half. A *fourth* concurrent QEMU shard would
2× SMT-oversubscribe the 8 cores and reintroduce the timeout class, so this box tops
out at **~3 smooth shards regardless of how much RAM it has** — the 4-shards-on-3-runners
wave (one shard waits for a slot) is the steady state, not a bug.

### Follow-on levers — the CPU-oversubscription residual (next tier)

The RAM cap is the immediate, high-ROI, zero-code fix. The CPU half is a separate,
higher-cost tier of durable levers, in increasing cost:

- **Drop to 2 runners on this box** — 8 threads / 8 physical cores at 1:1, no SMT
  oversubscription; trades peak shard-concurrency (3→2) for contention-free wall-clock
  on each.
- **Add a real second machine** — a cheap x86 mini-PC / spare desktop with a
  multi-core CPU + ≥ 32 GiB RAM takes a *full* shard with no guard special-casing.
  8 cores cap *this* box at ~3 smooth jobs no matter the RAM; more true concurrency
  needs more physical cores, not more runners on the same silicon. (A low-power NAS
  is the wrong tool for this — TCG is pure CPU, and a NAS trades clock for storage
  I/O; an x86 Synology was evaluated 2026-07-05 and parked. If ever wired in it would
  need a per-runner-label guard threshold, not the global 32 GiB floor, and only a
  deliberately small shard sized to finish under the 240-min timeout at its slower rate.)

## Shared-box workload discipline — heavy coder work pauses during a measurement gate

All three runners **and** any local coder builds share this one i9. For ordinary
**pass/fail** gates, contention only *slows* a run (the RAM/CPU notes above) — the
verdict is still valid, just later. But for a gate whose purpose is **measurement** —
the capacity demo that reads shard wall-clock, the OT-7 latency oracle, any timing or
throughput figure that will be *cited* — concurrent heavy CPU work (a coder's local
`cargo` build, a second heavy gate) **corrupts the number**, not merely the schedule:
the measured time inflates by whatever else the box was doing, and a figure taken
under hidden contention is simply wrong.

So the standing discipline: **before firing or trusting a measurement gate, drain the
box** — no heavy coder builds, ideally no sibling heavy gate — the same idle-box
requirement the RAM-cap apply procedure imposes. This is also why a coder **holds a
re-fired QEMU/measurement gate until a sibling coder's gate has cleared** rather than
stacking two heavy gates on ~3 runners: staggering keeps both the *schedule* and any
*measured numbers* honest. A correctness gate can share the box; a measurement gate
needs it to itself.

## PM merges are gate-firing events — pace them against coder gates (PM standing rule, 2026-07-08)

**A push to `main` runs the full 4-shard QEMU gate.** So a PM merge is NOT a free
book-keeping step — **every branch the PM cherry-picks/merges to `main` fires its own
~2-hour 4-shard QEMU gate on the merge commit**, contending for the same ~3 runners
coders need for their branch gates. This bit us (2026-07-08): the PM merged
**OT-49 → B1 → OT-9 in rapid succession**, each firing a main-push QEMU gate back-to-back;
combined with three coders re-gating at once, the runners were oversubscribed and
Jarvis's **OT-19 re-gate shard 1 hit the 70-min contention timeout** — a partly
self-inflicted stall (the PM's merge cadence was pouring fuel on the coder-gate
contention, not just the coders stacking gates).

**The discipline:**
- **Pace merges against live coder gates.** Don't merge a *batch* to `main` while
  coders need runners for their own gates. Stagger PM merges the same way coders stagger
  theirs — one heavy main gate at a time, and ideally not concurrent with a coder's
  QEMU gate. A queue of verified-and-ready branches can wait minutes for a drain; it
  costs nothing to hold.
- **A main-push QEMU gate that times out on shard-contention is INFRA, not a
  regression** (OT-35 doctrine). The merge commit is the same tree that gated green on
  the branch (modulo a docs-only cherry-pick), so a red main gate from a queue-timeout
  is the runner being starved, not the code — re-fire on a drained box, don't chase it.
- **Watch main's own gate after a merge**, precisely because a contention-timeout there
  reddens `main`'s status (bad optics / a false "main is broken" signal) even though the
  code is the branch-gated-green tree.

### RESOLVED 2026-07-09 (PL-approved) — main-push no longer runs the QEMU gate

The redundant main-push QEMU gate is **removed**. `ci.yml`'s `neo-test` job now runs on
`workflow_dispatch` + `schedule` **only** — NOT on push-to-main. So: the coder/PM
**pre-merge `workflow_dispatch` gate** (at the exact tree) is the per-change QEMU gate; the
**nightly `schedule`** (cron `0 8 * * *`, runs on main) is the integration net; and a
push to main runs **only** the light `host-checks` + `spdx-lint` + `monolithic-identity`
jobs. **Host-only / docs merges now cost ZERO QEMU**, and main-push gates no longer
cancel/fail each other under merge cadence. Cherry-pick integration is covered by the PM's
own re-stat inspection + the nightly. **A per-commit "run-on-main-only-if-kernel-changed"
refinement was evaluated and REJECTED** — `Cargo.lock` churns for host-crate additions too,
so path-based kernel detection is leaky in exactly the host-only case this targets, and a
buggy detector in a merge-blocking file isn't worth the marginal per-commit net over the
nightly. The "pace merges" discipline above still applies to `workflow_dispatch` gates
(coders staggering their own gates on the ~3 runners), but PM *merges* are now cheap.

<details><summary>Original open question (2026-07-08, superseded by the above)</summary>

#### Open efficiency question — does main-push need the FULL 4-shard re-run? (flagged 2026-07-08)

Every branch is already gated green (push-CI + the mandatory `workflow_dispatch` QEMU)
at its exact tree **before** the PM merges it. The main-push then re-runs the *entire*
4-shard QEMU on a tree that differs from the gated branch only by a docs-only
cherry-pick — i.e. **~a full redundant gate of runner load per merge**, on a ~3-runner
box where that load is the binding constraint. It IS a real post-merge safety net (it
would catch a bad cherry-pick *integration* — a conflict resolved wrong, a semantic
clash between two independently-gated branches), so it is not pure waste. But under
runner scarcity the full 4-shard cost every merge may not be worth it. Candidate
lighter designs (a CI-lane decision, not yet made): (a) main-push runs only the
**host-checks** job + SPDX, and the full QEMU gate on main only on a *schedule* (nightly)
or when the merge commit's code tree differs non-trivially from the branch-gated SHA;
(b) a `[gate-verified: <branch-sha>]` merge-commit trailer that lets main-push skip the
QEMU re-run when the tree is byte-identical to an already-green branch gate. Pair any
change with the OT-47 8-core stress lane thinking (both are "when do we spend the
scarce runners on QEMU"). **Until decided, main-push runs the full gate — so pace
merges (above).**

</details>


## Symptom: stuck-busy runner (online + busy, nothing in-progress)

**⚠️ FIRST rule out 'legitimately busy' — GitHub run-level status LIES.** A run stays `queued` at the RUN level until ALL its matrix jobs get a slot, so `gh run list --status in_progress` can show NO in-progress runs while that run's shards are ACTIVELY RUNNING (the last shard + the identity guard are just waiting for a runner slot). **The reliable signal is JOB-level, not run-level:** check `gh run view <run> --json jobs` (or the Actions UI job list) for jobs actually in `in_progress`, and/or live `docker compose ... cargo neo-test --shard` processes on the box. If any job/process is live, the runner is genuinely busy — **do NOT cycle (you'll kill live shards). Let it drain.**

**Signature of a TRULY stuck runner:** runners report `online, busy: true`, AND job-level shows NO `in_progress` jobs, AND no `neo-test`/docker workers alive on the box, AND queued jobs sit unstarted. **Cause:** a job was killed/cancelled uncleanly (box crash, `wsl --shutdown` mid-run, exit-137, a cancelled gate) so the `Runner.Listener` never reported completion — the runner still advertises itself busy and won't pick up new jobs. **This is a soft block on ALL subsequent gating** even if the in-flight gate that wedged it already 'completed'. **Fix (box-side, PL — ONLY after confirming no live job at the job level): cycle the runner service on each affected runner** — in the runner dir `sudo ./svc.sh stop && sudo ./svc.sh start` (or restart the systemd unit / kill+relaunch `Runner.Listener`). Confirm by re-checking `busy:` flips to false and any queued run starts flowing. Not a code issue, never a gate verdict — a stuck runner is infra, like an under-provisioned one (OT-35 doctrine).

**Concrete remediation (systemd, PL 2026-07-07):** restart the three runner services — `sudo systemctl restart actions.runner.SecondStageTurbine-NeoPlutarch.altimitneo-wsl.service` (and `.gaming-pc-2.service`, `.gaming-pc-3.service`). **What actually gets stuck:** orphaned `docker compose … cargo neo-test --shard` / QEMU processes from a *previously cancelled* run linger and keep the `Runner.Listener` advertising busy even after the live run drains — the cycle kills them. **Confirm unstuck:** a queued run starts flowing immediately + the orphan `neo-test`/QEMU processes are gone from the process list. **Caveat — restarting cancels in-flight jobs:** if a run had already picked up jobs when you cycle, those jobs get cancelled and show cancellation/failure entries — those are INFRA cancellations from the restart, NOT code verdicts (OT-35 doctrine); ignore them, don't chase them as gate failures. Minimize by cycling only after the active run drains (or accept the re-run cost).

## Failure signature: Docker Desktop VM disk full → docker CLI SIGBUS at setup-buildx (first seen 2026-07-10, OT-31 gate)

**Symptom:** a QEMU shard fails at the `docker/setup-buildx` step, BEFORE any kernel build; the job log shows
`/usr/bin/docker version → SIGBUS: bus error … panic during panic` (the docker **Go binary** crashing — NOT a kernel
panic, NOT a test verdict). Shards that started earlier may pass (they consumed the last free bytes first).

**Discriminator:** `df -h` in the job log (the disk-probe steps) shows the Docker Desktop VM disk at 100%:
`C:\Program Files\Docker\Docker\resources  931G  931G  0  100% /Docker/host`. Free space degrading across
consecutive runs (e.g. 521M → 0) confirms progressive fill, not a one-off.

**Classification:** INFRA, never a code verdict — the failure precedes the checkout of any kernel work. A worker
citing this signature with the df evidence is making a *legitimate* infra attribution (the OT-31 gate is the
precedent: the code shards that ran were green; the starved shards never built anything).

**Fix (runner box, PL):** `docker system prune -af --volumes && docker builder prune -af` (reclaims accumulated
per-shard cargo caches + build images), or grow the Docker Desktop virtual disk. Then rerun only the failed shards:
`gh run rerun <run-id> --failed`.

**Root cause (confirmed 2026-07-10 from the Docker Desktop volume list):** each CI job's `setup-buildx` action
creates a NEW builder whose named state volume (`buildx_buildkit_builder-<uuid>_state`, **~2.9 GB each**) SURVIVES
the post-job `docker buildx rm` — the builder dies, its volume leaks. Dozens accumulate (~70+ GB observed). The
targeted reclaim (a plain `docker system prune --volumes` does NOT remove named volumes on current Docker):

    docker volume ls -q --filter "name=buildx_buildkit_builder-" | ForEach-Object { docker volume rm $_ }
    # in-use volumes refuse and are skipped — safe to run any time

**NEVER delete `altimitneo-qdrant-storage`** — that is the RAG's Qdrant index (full re-ingest to recover).

**Prevention — (1) and (3) LANDED (ci.yml "Pre-flight — Docker disk hygiene", 2026-07-11): every docker-using job now self-heals by pruning stale buildx state volumes at start (in-use ones refuse — concurrent-safe; covers the killed/cancelled-job leak path where the post step never runs) AND fail-fasts with infra attribution below 15 GiB free. Remaining follow-up: (2) below.** Original list: (1) add a **disk-space pre-flight** step to the QEMU jobs, sibling to the
existing runner-memory pre-flight — fail fast with the df output and a "prune the runner" message instead of letting
docker SIGBUS mid-setup; (2) fix the leak at the source: have the workflow's post step remove the builder's state
volume too (or reuse ONE named builder across runs instead of minting a fresh builder per job); (3) until (2) lands,
a scheduled targeted volume prune (the command above) on the runner boxes — the per-run "Rotate buildx cache" step
bounds the layer cache but NOT these state volumes; the 2026-07-10 fill happened with rotation active. The
per-build-folder disk-bomb lesson (CLAUDE.md disk hygiene) applies to the Docker VM disk too: caches accumulate
invisibly until a hard 100%.

## `main` reddened after a merge — the escaped-defect flow (revert-first, PL-ratified 2026-07-12, OT-58)

**Trigger:** a post-merge signal reddens `main` — the nightly-full suite, a `workflow_dispatch` QEMU gate, or the
next coder's branch failing on `main`'s kernel (the OT-59 signature: defects that escaped because push-CI skips
QEMU under staging). **First, classify:**

1. **Is it INFRA?** If the red matches a runbook signature above (Docker-VM-disk SIGBUS, RAM-OOM, stuck-busy
   runner, artifact-quota), it is **exempt** — re-fire the gate, do not revert code, do not count it as an escape.
2. **Is it a real code defect on `main`?** Then apply the **escaped-defect contract** (CLAUDE.md §"The
   escaped-defect contract"):
   - **Revert-first is the DEFAULT.** Get `main` back to green now (revert the offending merge) so every seat
     rebasing off `main` isn't taxed by a red base. Confirm the revert is itself gate-green.
   - **Fix-forward only within ~4 working hours.** The owning coder may fix-forward *instead* of reverting only
     if they land a **rebased, gate-green** fix inside that window. Past it: revert now, re-land on a branch later
     (nothing is lost — the work sits on its branch).
   - **Promote the escaped scenario** into the merge-blocking tier (PM-reviewed) so the class can't escape twice.

**PM operational steps** for a revert: identify the merge (`git log --first-parent`), `git revert -m 1 <merge-sha>`
on a branch, gate it, `--ff-only` to `main`; then the coder re-lands the corrected work as a fresh branch. **Bisect
tip** (the OT-59 case): the introducing commit lives between the last **green** `workflow_dispatch` QEMU gate and
current `main`; because staging means push-CI didn't gate QEMU, do not trust green *push* runs as gate evidence —
only a green `workflow_dispatch` (or nightly) QEMU run counts as a bisect boundary.

**The latent-race exception (fix-forward by necessity).** Sometimes the bisect finds NO introducing commit — the
red **predates the last "green" gate** because it is a *latent contention race* the earlier gate simply didn't
happen to trigger (nondeterministic), not a recent regression. This is the OT-59 signature (Cerebro's ISC-2
finding on the three heavy Arthur scenarios). A latent race has **nothing to revert** — revert-first is
inapplicable, so it fix-forwards by necessity (diagnose → fix → falsifiable stress-repeat regression that
reliably triggers pre-fix). Distinguish it from a revertable escape by the bisect: a clean bisect to a commit ⇒
revert-first; no introducing commit (red reproduces at/behind the last green gate) ⇒ latent race, fix-forward.

## Failure signature: ALL FOUR QEMU shards cancelled at the same second, 0 artifacts (2026-07-30/31)

**Symptom.** A `workflow_dispatch` gate reports `failure`; every QEMU shard (1/4…4/4) ends
`cancelled`/`failure` at the **identical timestamp**, the `Run QEMU integration tests` step never
reaches `completed`, the `Dump`/`Upload neo-test serial logs on failure` steps stay `pending`, and the
run yields **zero artifacts**. The `ubuntu-latest` host job (fmt / workspace tests / clippy ×3 / SPDX /
identity) passes normally. Observed twice on the same SHA (runs `30507794327`, `30592440629`,
`codex/dw1-desktop-wm-jarvis` @ `7520337b`).

**This is INFRA, never a code verdict.** Re-fire; do not bounce the branch, do not revert, do not count
it as an escaped defect (escaped-defect contract §3).

**Diagnosis — it is the runner HOST, not the tests.** `ryzen-1..4` all live on the *same physical box*,
so a host-level event kills all four jobs in the same second. Four independent test failures cannot share
a timestamp, and a genuine test failure would leave the QEMU step `completed` **with** serial artifacts
uploaded. Zero artifacts ⇒ the job died before the on-failure capture could run.

**Ruled out (do not re-investigate):**
- **Concurrency cancellation.** `concurrency.group = ci-${workflow}-${ref}-${event_name}` with
  `cancel-in-progress: true` is **ref- and event-scoped** — another branch's gate, or a push-CI run,
  cannot cancel a `workflow_dispatch` on a different ref. Only a *second dispatch on the same ref* can.
- **Job timeout.** The QEMU jobs ran ~26 min against `timeout-minutes: 60`.

**Check, in order (PL — host-side):**
0. **INTERNET / network first — the confirmed cause of the 2026-07-30/31 occurrences (PL, 2026-07-31).**
   Self-hosted runners hold a **persistent connection to GitHub** (long-poll for jobs, stream status and
   logs). An internet outage severs it: GitHub loses every runner on that network **in the same second**,
   marks the jobs cancelled, and **no artifacts can upload** (the upload target is GitHub).

   > **⚠ THIS DISCRIMINATOR NO LONGER EXISTS IN-RUN (2026-08-10).** It used to read: *"the
   > `ubuntu-latest` host job is GitHub-hosted and keeps passing, while every `self-hosted` job dies — a
   > clean split along the network boundary."* **There is no longer a `ubuntu-latest` job in this
   > workflow** — the free-minutes migration below moved all six jobs to `self-hosted`, so a network
   > outage now kills the *entire run* with nothing left standing to contrast against. This is a real,
   > accepted cost of that migration, recorded here rather than discovered during an outage.
   >
   > **The replacement is out-of-band, and it is now step 0:** check the runner list (GitHub → Settings →
   > Actions → Runners) and the box's connectivity **directly**. The surviving in-run signature is
   > unchanged and still strong on its own — *all* jobs die at the **identical second** with **zero
   > artifacts**, which no set of independent test failures can produce.

   If you see that signature, check connectivity *before* anything else. Corollary: a LAN/router outage also breaks (a) SSH to the
   bare-metal boxes ((LAN address in the PL's notes), (LAN address in the PL's notes)) and (b) any bare-metal test needing a **DHCP server**
   (the eth0-at-boot lease proof) — one outage can present as three unrelated mysteries.
1. **Runner liveness** — GitHub → Settings → Actions → Runners: are `ryzen-1..4` `Idle` or `Offline`?
2. **Sleep/hibernate** (plausible but NOT the cause here — the `ubuntu-latest`-passes/`self-hosted`-dies
   split rules it out; keep as a secondary check). A desktop entering sleep kills every runner on it
   simultaneously:
   `powercfg /a` · `powercfg /query SCHEME_CURRENT SUB_SLEEP` · fix with
   `powercfg /change standby-timeout-ac 0` and `powercfg /change hibernate-timeout-ac 0`.
3. **WSL/Docker wedge** (the pre-existing entry above) — `wsl --shutdown`, restart the distro + Docker,
   confirm `docker info`, then re-fire.

**Do not re-fire a third time before the host is confirmed healthy** — each attempt costs ~26 min of the
whole fleet, and this signature blocks *every* branch's gate, not just the one that surfaced it.

---

## All CI jobs moved to `self-hosted` — the free-minutes migration (2026-08-10)

**The constraint inverted.** Every prior entry in this runbook treats **self-hosted runner capacity** as
the binding constraint, and `ubuntu-latest` as the cheap escape valve — that is why `host-checks` moved
to `ubuntu-latest` in `d6dc133` (2026-07-22, "the real freed slot this cycle came from moving host-checks
to `ubuntu-latest`"). On **2026-08-10** the account hit **~90% of its 3000 free GitHub-hosted minutes**
with the cycle not resetting until **09-01**, and the PL will not buy paid minutes. GitHub-hosted minutes
became the scarcer resource, so the trade reverses.

**What was consuming them.** Seven billable job-instances per workflow run:

| Job | Instances | Runner before | Runner now |
|---|---|---|---|
| `host-checks` | 1 | `ubuntu-latest` 💸 | `self-hosted` |
| `product-build` | 1 | `ubuntu-latest` 💸 | `self-hosted` |
| `feature-gated-coverage` | **5** (matrix) | `ubuntu-latest` 💸 | `self-hosted`, `max-parallel: 2` |
| `neo-test` | 4 (shards) | `[self-hosted, qemu-shard]` | unchanged |
| `spdx-lint`, `monolithic-identity` | 2 | `self-hosted` | unchanged |

At ~7–8 billable minutes per run, the remaining ~300 minutes was roughly **13 branch iterations** of
runway. **Self-hosted minutes are free and unlimited even on a private repo** — only GitHub-hosted
runners bill against the 3000. Billable usage is now **zero**.

**Where the capacity came from.** The M75q (`ryzen-5..6`) left the QEMU shard pool
(`Plans/DECISION-2026-08-02-m75q-out-of-qemu-shard-pool.md`) and is idle. These are host-only cargo jobs,
not QEMU shards — they carry none of the RAM/CPU-oversubscription profile that got the M75q removed, so
the box is a good home for exactly this work.

### What changed beyond `runs-on`

1. **The Docker wrapper was NOT restored.** The `d6dc133` migration also replaced the `docker compose
   run` wrapper with bare `cargo` invocations, and that half is kept — it is simpler and it sidesteps the
   Docker-disk-hygiene failure class documented above. **Only the runner label changed.**
2. **A `rustup` bootstrap step was added** to all three jobs. `dtolnay/rust-toolchain@v1` *drives*
   `rustup`; it does not *install* it. That is safe on `ubuntu-latest` (preinstalled) but not guaranteed
   on a WSL2 box where the toolchain historically lived inside the dev container. The step is idempotent
   — a no-op wherever `rustup` already exists.
3. **`actions/cache` was removed from all three.** On a self-hosted runner the workspace and `~/.cargo`
   persist between runs, so `target/` is already warm; uploading a multi-GB `target/` to the GitHub cache
   is actively harmful (the `neo-test` job carries the same note — *"self-hosted target uploads can run
   for 30+ min after a shard has already passed"* — and the 10 GB repo cache quota is shared). A run that
   lands on a cold runner pays a full rebuild in **wall-clock**, which is free; the cache paid for it in
   **minutes and quota**, which are not.
4. **`max-parallel: 2` on `feature-gated-coverage`.** Five arms could otherwise occupy five of ~six
   runners and starve the **blocking** `host-checks`/`product-build` jobs that a `workflow_dispatch` QEMU
   gate `needs:`. A deliberately non-blocking job must never delay the gate.

### Costs accepted, stated plainly

- **The network-outage discriminator is gone.** See the ⚠ block in the all-shards-cancelled section
  above. With no GitHub-hosted job left, an outage kills the whole run and there is no in-run contrast.
  Replacement: check runner liveness/connectivity out-of-band; the identical-second + zero-artifacts
  signature still stands on its own.
- **Push CI now competes for the fleet.** Previously a push ran 7 jobs on GitHub's infrastructure and
  2 on ours; now all 9 are ours. Expect longer wall-clock on push CI, and expect a push-CI run and a
  `workflow_dispatch` gate fired close together to queue rather than overlap. Wall-clock is the currency
  we have; minutes are not.
- **Reproducibility.** `ubuntu-latest` is a clean image every run; a self-hosted box is not. A
  host-environment drift now shows up as a CI failure rather than being reset away. The `rustup`
  bootstrap pins the toolchain, which covers the main axis, but this is the residual risk.

### Revert condition

If GitHub-hosted minutes stop being scarce — the cycle resets **09-01**, or the PL elects to pay — the
honest question is not "revert?" but **"is anything actually better on `ubuntu-latest`?"** The only real
answers are the two costs above: the outage discriminator and clean-image reproducibility. Restoring
**one** cheap job (`spdx-lint` or `monolithic-identity`, both seconds long) to `ubuntu-latest` would buy
back the discriminator for a negligible number of minutes, and is the recommended move if minutes free up
— not a wholesale revert.

## One gate per ref at a time: `cancel-in-progress` (PM lesson, 2026-10-27)

`ci.yml`'s concurrency group is `ci-<workflow>-<ref>-<event>` with `cancel-in-progress: true`. A second
`workflow_dispatch` on the **same branch** cancels the run already in progress on that branch, even when it
targets a different commit. On 2026-10-27 the PM dispatched batch 6 on the integration branch while batch 5 was still
running there, and batch 5 (run `40453393702`) was cancelled 43 minutes in. No gate time was lost only because batch 6's
tree was batch 5's tree plus one PR, so it gated all three.

**Rule:**
- To queue a second gate while one is running, either stack it so it's a strict superset of the running one and accept
  that the running one dies, or dispatch it on a **different ref**, for example the PR's own branch or a second
  integration branch.
- Never dispatch on a ref with a run in progress unless you mean to replace that run.

## No nightly tier (PL decision, 2026-10-27)

**J.c.:** "we don't need nightly tests when you are running tests all the time already." The `schedule` cron stays
removed from `ci.yml` (it was dropped 2026-10-17). Scenarios marked `ignored: true` are the only off-gate tier, and they
run **on demand**: `cargo neo-test <name> --include-ignored`, or a `workflow_dispatch` that includes them.

**The PM runs each off-gate scenario at the milestone it protects,** and records the run in the release ledger:
- `install_stop_finish_full` (all six installer stops, OT-87): before any bare-metal install run (R6), and after any
  change to the installer's W0–W6 state machine.

Older text that names a "nightly-full" signal (the escaped-defect contract, OT-58) now means these on-demand runs plus
the per-merge 4-shard gate.
