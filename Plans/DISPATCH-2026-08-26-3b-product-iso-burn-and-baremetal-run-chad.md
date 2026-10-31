<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH 3b: burn the real ISO and run the 0.1 acceptance — TWO boots → Chad + PL

**Date:** 2026-08-26 · **From:** PM (Claude) · **To:** **Chad** (builds, burns, drives SSH, reads the logs) **and the PL** (boot-select, live on-screen read, PuTTY serial capture). A **joint blind-hardware run**.
**Status:** DISPATCHED — **no kernel code**. Docs + a physical run. **No 4-shard QEMU gate** (nothing compiled changes); the artifact is built from `origin/main` as-is.
**Queue:** `Plans/QUEUE-2026-08-07-single-lane-priority-order.md` — **item 3**, unblocked by 3a (`7b2ada27`).
**Run procedure (authoritative):** `docs/runbook-baremetal-arthur-optiplex.md` — `git show origin/main:docs/runbook-baremetal-arthur-optiplex.md`.

## Why this exists

Item 1 built the five-rung `product` integration. Item 2 rebuilt `sda` so it carries `.arthur/memory`,
verified byte-exact. 3a built the scenario that boots it on hardware. **Nothing has booted any of it.**

And the ISO physically on `nvme0n1` is the **`2026-08-01`** build — it predates 1a/1b/1c *and* 3a. The
PL's `strings` check on that kernel, with a positive control: `neoplutarch|neofs|kernel` → **71,200
hits**; `PRODUCT rung`, `product_integration`, `SHOWCASE product`, `DW1 focus`, `arthur` → **0 each**.
**Booting it produces a log with zero rung tokens — uninterpretable, not failed**, which the runbook's
conclusive-either-way rule forbids. So the burn is not a formality; it is the first task.

## What a PASS looks like

**Two boots of ONE ISO**, and the second one is one line long:

- **Boot 1** — the five-rung showcase: boot/mount → build stamp → rung 1 → the DW-1 desktop → the typed
  prompt received and compared at the **receiver** → real inference off the model on `sda` → rung 4's
  no-dongle degrade → rung 5 writes `/neofs/.arthur/memory/product-0.1.persist`.
- **Boot 2** — `read_prior_boot()` now returns `Some(_)`, so neo-init **short-circuits before the
  desktop and Arthur** and emits exactly:
  `NEO_TEST/1 event=INFO name=product_integration stage=readback bytes=39 source=prior_boot byte_equal=true fsync_prior_boot=true`
  then returns **78 — a nonzero SUCCESS**, which the kernel converts to terminal PASS
  `second_boot=prior_disk_byte_readback`.

**A correct boot 2 reads like a failure if you do not know this.** It is in the bank below as its own
failure mode for exactly that reason.

Deliverable: **two serial logs** (+ the NVMe sink as backup), the PL's live on-screen read, and the
**independent offline rung-5 proof** — read the persist file off `sda` from the live USB and verify its
content hash, rather than trusting a serial line that claims persistence worked.

## Scope

### 1. Build the artifact from current `origin/main`

```
docker compose run --rm neo cargo xtask neo-test-build product_integration_baremetal \
    --include-ignored --bare-metal-mode
```

Output: `product_integration_baremetal.bare-metal.iso` (`tools/xtask/src/main.rs:4971`). **Record its
SHA-256.** 3a's build was `57ec997cd4b1f889e4e3fdf726ed4706452d7e30ff8f15a4fe1588956867ec73`; a
different value is not by itself wrong (`main` has moved), but an **unexplained** difference is a
finding — say which it is.

**The stamp grep, run on the FILE before it goes anywhere:**

```
strings -a product_integration_baremetal.bare-metal.iso \
  | grep -F -m1 'PRODUCT build_variant=product_integration_baremetal mode=single_boot_per_invocation sink=nvme'
```

Absent ⇒ stop. Also confirm the rung, receiver-prompt, DW-1 focus, boot-2, and eth0-failure literals —
the same set that had **0** hits on the stale ISO.

### 2. Pre-burn state capture — three things, all before anything is written

This is the capture-before-you-need-it clause. All three are cheap and all three become
uninterpretable after the fact:

1. **`.arthur/memory` MUST be empty.** Boot 1 only exercises rungs 2 and 3 while it is. Confirm with
   `tools/neofs-inspect.py /dev/sda` from the live USB (re-copy it — the box's root is tmpfs and was
   wiped by the item-2 reboot). If `product-0.1.persist` already exists, **do not boot** — reset first
   (§5).
2. **The NVMe sink region must be zero.** A stale `NEO_LOG_v1` at 256 MiB from an earlier run would be
   read back as if it were this run's:
   `sudo -n dd if=/dev/nvme0n1 bs=1M skip=256 count=16 | strings | head` — record what you see. (The
   burn overwrites it regardless; capture it so the post-run read is attributable either way.)
3. **The box's address.** DHCP-assigned and it *has* moved (the runbook carried a wrong one for three
   weeks). `ping -c1 <optiplex-ip>`, else `arp -a | findstr "<lan-prefix>"` from the PL's PC. Correct the
   runbook line in the same commit if it moved.

### 3. Burn, and prove the burn

`scp` the ISO to the live USB, then write it to **`nvme0n1`** (the boot device — **not `sda`**, which
is the NeoFS volume under test):

```
sudo -n dd if=<iso> of=/dev/nvme0n1 bs=4M status=progress conv=fsync
```

**Then prove it took**, because "dd returned 0" is not proof the device holds what you think. Drop
caches first (`echo 3 | sudo -n tee /proc/sys/vm/drop_caches`) so it is a real read and not page cache,
then read back exactly the ISO's byte count and compare SHA-256 against the file. This is the same
discipline item 2 used for `sda` (`md5(first 768 MiB) = c213b498…` after a cache drop), and it is what
makes "the right ISO is on the box" a *verified* claim rather than an assumption. Re-run the stamp grep
against the **device**, not the file, as the independent second check.

> **⚠ Target confirmation before `dd`.** Log the device identity (`lsblk -o NAME,SIZE,MODEL,SERIAL`)
> and confirm `nvme0n1` is the Intel NVMe boot device and `sda` is the Samsung 860 EVO
> (488,397,168 sectors). Item 2's pre-write guard existed for this reason; a renamed device is the one
> mistake that is not recoverable.

### 4. The run (JOINT — the PL is at the box)

Per the runbook: PL has PuTTY connected and **logging to `C:\Users\<user>\neoplutarch-serial.log`**;
Chad triggers `sudo -n efibootmgr -n 0001 && sudo -n reboot` over SSH.

**Boot 1 — the dongle stays OUT.** `kernel/src/test_harness.rs:2263` latches
`debug_status() != NoDevice` → `fail(...)` **before** the rung-1 and rung-4 emissions, so a plugged-in
AX88179 aborts with **no rung tokens at all**.

**Let it run to completion.** Expect the linear ~3-4 min generation (scalar per-token matmul). A true
hang is distinguished by the per-stage breadcrumbs stalling, **not** by wall-clock. Do not halt early.

**PL:** confirm legibility by eye, live. **There is no photo** — the serial `REPLY:` line on a
proven-non-blank framebuffer is the legibility proof, and the log is conclusive on its own.

**Save the log under a per-boot name** (`…-boot1.log`) before boot 2 overwrites the session — the
runbook's single fixed path assumes one boot per run, and **that is the amendment this dispatch owes**
(§6).

Then boot-select back to the Ubuntu USB, read the sink, and take the **offline rung-5 proof**:
`tools/neofs-inspect.py /dev/sda` → is `/neofs/.arthur/memory/product-0.1.persist` present, and does
its recorded content hash verify? This is an *independent* channel: it proves persistence from the
disk, not from a serial line asserting persistence.

**Boot 2** — same ISO, same command, second capture (`…-boot2.log`). One line, return **78**, terminal
PASS `second_boot=prior_disk_byte_readback`. **No DW-1 focus, no typed prompt, no inference, no Arthur
reply** — their absence is the expected result, not a regression.

### 5. If boot 1 must be retried — the volume reset

**The run is NOT idempotent.** Once boot 1 completes, every subsequent boot takes the readback arm and
**silently skips the desktop and Arthur**. A retry therefore requires resetting `sda` first — rebuild
the volume on the **build box** with the item-2 recipe (deterministic by spec; `neofs-mkfs sanity`
proves it), verify the **file**, then write:

```
neofs-mkfs create --output <img> --variant arthur-memory --size-mib 768 \
  --extra-model model-qwen2.5-0.5b.neom=<real .neom> \
  --extra-file  model-qwen2.5-0.5b.vocab=<vocab>
```

**Do NOT pass `--model-path`** — the 4,100-byte `model.neom` stub must keep its default content
(`428e958f…`); `validate_children` (`kernel/src/fs/neofs/mount.rs:332`) fails the mount without a root
entry literally named `model.neom` of kind `Model`, and the real 485 MB model does not satisfy that
check. The model + vocab live on the build box under `E:\AI Automation\Models\`; the expected model
hash is `a32e4a1ba94a850724c15d840f3ae2c44902d9f1f34d8069bac5fcb37e1ccafe`
(`neofs-mkfs hash --expect`).

### 6. The runbook amendment (a deliverable, not a nicety)

`docs/runbook-baremetal-arthur-optiplex.md` is single-boot throughout and is now **stale in three
places**. Amend it in this lane:

1. **Two captures.** A per-boot log naming convention, and the boot-2-is-one-line-and-returns-78 fact,
   so the next reader does not mistake a correct boot 2 for a broken run.
2. **The burn procedure** — it is not in the runbook at all. Every burn so far has been re-derived
   under fire. Add §3 above: target confirmation, `dd`, the drop-caches hash-back proof, the
   device-level stamp grep.
3. **The `sda` listing is out of date.** It still says "**No `.arthur/memory`** — that is the item-2
   disk-prep question." Item 2 closed on 2026-08-11: the volume is the `arthur-memory` variant, seven
   entries, `.arthur/memory` present, free map MATCHES, all content hashes verified. Fix that line and
   the address line (§2.3) in the same commit.

## Explicitly out of scope

- **No kernel or xtask code.** If the run reveals a real fault, it becomes its own bounded fix dispatch
  carrying the captured discriminator. Do not rabbit-hole live at the box.
- **No `sda` write unless boot 1 is being retried** (§5), and then only via the verified rebuild path.
- **Rung 4 positive eth0** (link-up + lease, dongle in) — the scenario asserts the no-dongle degrade
  **by equality**, so it can only ever prove absence. That run is item 11.
- **Not OT-82.** That the feature named `product` is not the shipping product is a real finding and a
  PL decision; it is not settled here.
- **No budget raises, no guard weakening, no `is_reboot_persistence_scenario` change.**

## Failure-mode + discriminator bank (blind-hardware standing rule)

The run must be conclusive whether it passes **or** fails. Modes 1–5 are 3a's, carried forward with the
run-side observation; 6–9 are new to the burn and the two-boot shape.

| # | Failure mode | Discriminator — what you observe ⇒ which cause |
|---|---|---|
| 1 | **Wrong/stale artifact booted** | The stamp grep on the **device** (not the file) before boot, plus the post-`dd` hash-back after a cache drop. Zero rung tokens **with** the stamp absent ⇒ burn, not code. |
| 2 | **Boot 1 short-circuits into the readback arm** | `stage=readback … source=prior_boot` appearing on the intended *first* boot ⇒ `.arthur/memory` was non-empty; rungs 2 and 3 never ran. The §2.1 pre-check is what prevents spending a boot to learn this. |
| 3 | **Boot 2 misread as a failure** | One line + return 78 + `second_boot=prior_disk_byte_readback` **is** the pass. Absence of desktop/prompt/inference/reply on boot 2 is expected. |
| 4 | **AX88179 left connected** | `reason=product_eth0_expected_clean_no_dongle_degrade` with zero rung tokens ⇒ "dongle in," **not** "instrumentation missing" and not "boot broke." The literal is present in the ISO. |
| 5 | **Sink wrong or unreadable** | `NEO_LOG_v1` missing at 256 MiB after a run ⇒ the evidence channel is absent. That is **not** permission to infer success from display behaviour — it is a failed run with the serial log as sole channel. |
| 6 | **`dd` wrote the wrong device** | The pre-`dd` `lsblk` identity log + post-write readback of **both** devices; `sda` still `NEOFSv1` with seven entries ⇒ untouched. This is the unrecoverable mistake; the guard is the log, taken before. |
| 7 | **Stale sink read as this run's log** | The §2.2 pre-burn sink capture. Zero before ⇒ any `NEO_LOG_v1` after is this run's. Non-zero before and after ⇒ ambiguous, and you will not be able to tell later. |
| 8 | **The box is unreachable mid-run** | Expected — NeoPlutarch has no sshd. Unreachable ⇒ almost always "booted into NeoPlutarch," not "moved." After the one-shot it falls back to **Windows**, so returning to SSH is a physical boot-select, not a network problem. |
| 9 | **Inference ran on a fixture, not the real brain** | On hardware the model comes from `sda`, but **the build output cannot tell you which path was taken** — `prepare_stage_b_real_brain_model` returns the env path silently and the fixture path has no marker. The positive control is on the **artifact** (size 485,489,152 + hash `a32e4a1b…` on `sda`), never on the log. The QEMU fixture's vocab maps token 3 to the literal `"Arthur is ready."`, so a reply of exactly that string is a red flag, not a pass. |

## Governance

- **I-14 fault containment:** N/A with reason — no new driver, no new device-initiated DMA; the sink
  reuses the existing NVMe log-sink path.
- **Concurrency pitfall bank (CP-1…CP-30):** N/A with reason — no cross-CPU, interrupt, lock, atomic,
  TLB-shootdown or scheduler-wake code changes.
- **Byte oracle:** N/A — no new on-disk or on-wire byte layout; this lane consumes the existing
  `.neom`/NeoFS oracles.
- **Kill-switch (ISC):** N/A — nothing compiled changes. The artifact is built from `origin/main` as-is.
- **AAG:** not triggered — no kernel-touching merge; the commit is doc-only.

## Close-gate + merge

Docs-only ⇒ the CI workflow paths-ignores `docs/**`, `Plans/**`, `**.md`, so no CI fires and none is
owed. Still run **`scripts/worker-close-gate.sh`** for rebase-freshness and SPDX, and inspect your own
`git diff --stat` before declaring close.

**The RETURN is a pushed artifact** at
`Plans/RETURN-2026-08-26-3b-product-iso-burn-and-baremetal-run-chad.md`, carrying: the ISO SHA-256 +
the stamp-grep output; the three pre-burn captures; the burn hash-back (post-cache-drop); **both serial
logs verbatim**; the offline `neofs-inspect` rung-5 proof with the persist file's content hash; the
PL's live legibility confirmation; and a per-rung verdict table (rungs 1–5, PASS/FAIL/not-exercised,
with the serial line that proves each). **A self-diagnosing FAILURE is a valid RETURN** — the first
run's job is to be conclusive, not necessarily green.

**Disk reclaim (after PM confirms merge):** once the commit is on `origin/main`, `cargo clean` this
tree to reclaim `target/`. **Never delete a tree whose branch is unpushed/unmerged.**

---

## Relay — Chad (+ the PL runs the hardware)

- **WORKTREE:** `E:\AI Automation\workers\chad` (yours alone — sibling outside the repo, never a nested `Workers/`)
- **BRANCH:** `codex/product-iso-burn-baremetal-run-chad`
  (`git fetch origin && git checkout -B codex/product-iso-burn-baremetal-run-chad origin/main`)
- **Read:**
  `git fetch origin && git show origin/main:Plans/DISPATCH-2026-08-26-3b-product-iso-burn-and-baremetal-run-chad.md`;
  the run procedure `git show origin/main:docs/runbook-baremetal-arthur-optiplex.md`;
  your own 3a RETURN `git show origin/main:Plans/RETURN-2026-08-11-3a-product-integration-baremetal-chad.md`
  (the five discriminators and the stamp string are yours);
  items 2 and 3 in `git show origin/main:Plans/QUEUE-2026-08-07-single-lane-priority-order.md`.
- **One-line:** **Everything for 0.1 is built and nothing has booted it — and the ISO physically on the
  OptiPlex is the `2026-08-01` build that predates your own item-1 work (0 hits for `product_integration`
  / `arthur` against a 71,200-hit positive control).** Build
  `product_integration_baremetal.bare-metal.iso` from current `main`, **prove the burn** (`dd` to
  `nvme0n1` — **not `sda`** — then hash it back **after dropping caches**, plus the stamp grep against
  the *device*), and run the acceptance with the PL: **TWO boots of ONE ISO.** **Three things will
  otherwise misfire: (1) boot 2 is SUPPOSED to be one line and return 78** — a nonzero success, no
  desktop, no Arthur; **(2) the run is NOT idempotent** — boot 1 only exercises rungs 2 and 3 while
  `.arthur/memory` is empty, so **verify it is empty before booting**, and know that a retry means
  rebuilding the volume from the item-2 recipe (**never pass `--model-path`** — the 4,100-byte
  `model.neom` stub is mount-load-bearing); **(3) the dongle stays OUT** — `test_harness.rs:2263`
  latches before the rung emissions, so an AX88179 aborts with zero rung tokens. Capture the sink
  baseline **before** the burn or you cannot tell a stale `NEO_LOG_v1` from this run's. Take the
  **independent offline rung-5 proof** — read `/neofs/.arthur/memory/product-0.1.persist` off `sda` with
  `neofs-inspect.py` from the live USB and verify its content hash, instead of trusting a serial line
  that claims persistence worked (re-copy the inspector; the box's root is tmpfs and was wiped).
  **Amend the runbook** — it is single-boot throughout, has no burn procedure at all, and still says
  `sda` has **no** `.arthur/memory`, which item 2 closed on 08-11. **Out of scope:** no kernel/xtask code
  (a real fault becomes its own dispatch with the captured discriminator — do not debug live at the box),
  no positive-eth0 rung 4 (equality-degrade only; that is item 11), not OT-82. **The PL is at the box**
  for the boot-select, the live on-screen read, and the PuTTY serial capture — **there is no photo**; the
  serial `REPLY:` line on a non-blank framebuffer is the legibility proof. **A self-diagnosing FAILURE is
  a good RETURN.**
- **RAG** (semantic code search) is available — use it for conceptual "where is X done?" questions
  (grep still wins for exact symbols). Health-check first:
  `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`. If down/degraded:
  the PL manages RAG infra — note it in your RETURN and continue with direct file reads (never a
  blocker). Optional self-serve start (PowerShell):
  `powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"` (if `ollama:false`), then
  `cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
  Query e.g.: `$body=@{query='bare metal ISO export and log sink raw path'; top_k=5}|ConvertTo-Json;
  Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`
  — also try `read_prior_boot persistence arm selection`, `product eth0 no dongle degrade latch`,
  `neofs inspect content hash verification`.
