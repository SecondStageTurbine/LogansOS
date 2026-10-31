<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: how Arthur gets a real model on a fresh install / first boot

**Date:** 2026-10-19 · **Lane:** design only (docs, no gate) · **Branch:** `claude/arthur-first-boot-model-think` from `main`
@ `19d476a4` · **Cross-check:** `Plans/DISPATCH-2026-10-19-arthur-first-boot-model-codex-crosscheck.md` (Chad).

Every factual claim below cites the file and line it rests on, measured against `main` @ `19d476a4`. Where a
claim rests on something outside the repo (Limine, xorriso, upstream model files), it is marked **to verify** and
names the raw source to read. Effort is stated in dependencies, cycles and gates, never in calendar time.

---

## For the PL (plain language)

**Where things stand.** Arthur runs a real model on real hardware today, but only because the model was copied by
hand onto a second disk inside the OptiPlex. The ISO carries no model (`docs/ADR-063-arthur-boot-model-provisioning.md:48-49`).
On a fresh machine with nothing but our USB stick, Arthur says *"I can hear you, but no model is installed yet."*
(`userland/neo-arthur/src/main.rs:790`).

**The one surprise this study found.** The kernel **cannot read a USB stick** after it boots. Its only storage
drivers are AHCI (SATA), NVMe and virtio (`kernel/src/drivers/block/`: `ahci.rs`, `nvme.rs`, `virtio_blk.rs`). The
firmware and the Limine bootloader can read the stick, but our kernel cannot. So "put the model on the USB in a
NeoFS partition" does not work on its own; it needs a new USB-storage driver. The same gap means a fresh machine has
no place to persist your work until an installer or that driver exists.

**Decisions you need to make.** Each is a short choice. My recommendation is first.

1. **How does the model reach a fresh 0.1 machine?**
   - **(a) Recommended.** The bootloader loads the model from the USB into memory at power-on, as a boot file. This
     needs no new driver. It reverses part of ADR-063, which ruled this out to save memory. The memory is handed
     back once the model is loaded, so the lasting cost is today's (see Q1).
   - (b) Write a USB-storage driver first, then keep the model in a NeoFS partition on the stick. This is the right
     long-term answer and is needed for 1.0 anyway, but it is a new driver brought up blind on real hardware.
   - (c) Download the model at first boot. Not for 0.1: there is no download client, and it fails with no network.
   - (d) The user prepares a disk. This is today's developer path, and not a beginner path.
2. **Which brain does 0.1 ship?**
   - **(a) Recommended.** The 0.5B, the only one proven on silicon. Add the 1.5B to the same image once it passes
     its bare-metal run. The OS picks the biggest one the machine can hold.
   - (b) Ship the 1.5B as the default now, as ADR-063 says. It is unproven on silicon.
3. **Does the 3B ship?**
   - **(a) Recommended.** Not in 0.1. It is not under Apache-2.0 (Qwen License: attribution plus a use policy), it
     needs a 2.75 GiB budget, and it would make the image about 3.7 GB.
   - (b) Yes, with its licence and use policy shipped beside it.
4. **Does 0.1 need a read-only model volume?**
   - **(a) Recommended.** No, *provided* the model's fingerprint is checked every time it loads. A damaged model is
     then refused with a clear message instead of misbehaving.
   - (b) Yes, build the volume-role selector before 0.1.
5. **On a fresh machine with only the USB, is "your work persists" required for 0.1?**
   - **(a) Recommended.** Not required. Arthur answers, but persistence is honestly off until the installer or USB
     storage lands.
   - (b) Required, which pulls the USB-storage driver into 0.1.

---

## What is already on record

- **ADR-063.**
  - Weights never ship as Limine modules, because "that forces the payloads into boot-time RAM" (`docs/ADR-063-arthur-boot-model-provisioning.md:10-14`).
  - The registry has `1.5b` as the default and `0.5b` as the fallback (`:30-33`).
  - `/model` keeps the previous model when a load fails (`:35-38`).
  - The default ISO stages no model (`:48-49`).
- **The Bible.**
  - Models are provisioned out-of-band through `neofs-mkfs --extra-model` (`docs/bible.md:282`).
  - The `.neom` cap is 2 GiB (`:288`).
  - The `.vocab` sidecar and large-file NeoFS come from ADR-064 (`:290`).
- **The OptiPlex layout.**
  - The ISO is on NVMe, and the model is on a separate SATA `NEOFSv1` disk: `model-qwen2.5-0.5b.neom` at 485,489,152 B
    plus its `.vocab` (`docs/runbook-baremetal-arthur-optiplex.md:33-40`).
  - The stubs `model.neom` and `hello.txt` are load-bearing (`:42-48`). The mount fails without them
    (`kernel/src/fs/neofs/mount.rs:329-341`).
- **The 0.5B is locked for the first silicon run, and 1.5B → 3B are follow-on tiers**
  (`Plans/DISPATCH-2026-07-18-baremetal-arthur-optiplex-cerebro.md:79-82`). The 1.5B is proven only in QEMU; OT-28
  lists "the bare-metal 1.5B run" as the remaining item (`docs/OPEN-THREADS.md:39`).
- **"The brain is NEVER the tier axis".** The OS auto-selects the best brain the machine can run
  (`Plans/RESEARCH-2026-07-08-beginner-poweruser-os-substrate-FINDINGS.md:39`).
- **The model volume is the persistence volume for 0.1.** The ratified risk is that the model and user data share
  one free map (`Plans/DECISION-2026-08-01-unified-neofs-product-volume.md:8, 20-22`). The way out is a "volume
  role/label selector" (`:26`).
- **0.1 ships from a USB/ISO with no installer** (`Plans/LEDGER-2026-07-29-logansos-release-ledger.md:30`). OT-65 (the
  installer and first-boot provisioning) is 1.0 scope with zero code (`docs/OPEN-THREADS.md:75`).
- **Licences.**
  - 0.5B and 1.5B are Apache-2.0. The 3B is under the "Qwen License (attribution + use-policy)"
    (`Plans/RESEARCH-2026-06-17-arthur-model-and-intent-FINDINGS.md:20-22, 38-39`).
  - The Bible's licence article covers code, docs and firmware, and not weights (`docs/bible.md:387`).
  - `NOTICE` lists firmware (`NOTICE:33`) and fonts (`:48`) but no weights.
- **Integrity (OT-73, OPEN).**
  - Under `arthur-real-brain-boot`, the read-side content check skips `Model` inodes
    (`kernel/src/fs/neofs/inode.rs:1521-1527`, the same at `:1530-1536`).
  - The mount side never verifies `Model` content in any build (`mount.rs:343-345`).
  - The planned fix is to hash during the single load pass and fail closed (`docs/OPEN-THREADS.md:83`).
  - The 0.5B's blake3, `a32e4a1b…ccafe`, exists only in docs. Code pins only the byte size
    (`tools/xtask/src/main.rs:68`).
- **OT-81 (OPEN).** The documented model-staging path did not exist on the build box (`docs/OPEN-THREADS.md:91`).
- **OT-51 (update/rollback).** It recommends write-beside → verify → atomic boot-pointer flip, keeping the old
  deployment (`docs/THINK-2026-07-10-ot51-update-rollback-model.md:20, 144, 149`). It never mentions model weights.
- **Mechanics that constrain every option.**
  - The heap budget is a compile-time `const fn` chosen by build features (`kernel/src/mm/heap_policy.rs:38-59`):
    768 MiB for 0.5B, 1.75 GiB for 1.5B and 2.75 GiB for 3B (`:9-36`). One image runs one tier.
  - Usable RAM is known only as a kernel boot log line (`kernel/src/mm/mod.rs:158-166`). `SysStatV1` has no total-RAM
    field (`kernel/src/arch/x86_64/syscall.rs:453-461`).
  - The ISO contains no NeoFS. NeoFS is always a separate raw disk image (`tools/xtask/src/main.rs:4592` ff., `:9570`).
  - No partition-table parsing was found in the block or NeoFS code.
  - The ISO already stages `/licenses/{NOTICE,OFL-IBM-Plex.txt}` (`tools/xtask/src/main.rs:50-55`).
  - Sizes: 0.5B `.neom` is 485,489,152 B; 1.5B is 1,111,411,200 B; 3B is 2,099,030,016 B; each vocab is 7,990,862 B
    (`tools/xtask/src/main.rs:68-70, 76-77`).

---

## Q1. How the model reaches a fresh 0.1 system

| Option | Works offline | New code | Blind-hardware risk | Image size | Verdict |
|---|---|---|---|---|---|
| **A. Boot module on the USB image** (Limine loads it; the kernel builds the model from it, then frees the module's memory) | yes | model load from a boot-module range; module-memory reclaim; amend ADR-063 | low (firmware + Limine read the stick; already proven for our modules) | ISO + model | **0.1** |
| **B. NeoFS partition on the USB** + USB mass-storage (Bulk-Only/SCSI READ(10)) driver over the existing xHCI | yes | new driver + partition probe + appended partition | high (new device class, blind) | ISO + NeoFS partition | **1.0 path** |
| C. Fetch at first boot | **no** | HTTP client, real CA roots, network posture (OT-77), a writable target | high | small | not before 1.0 |
| D. User-supplied disk | yes | none | n/a | small | developer path only |

### Why A for 0.1

A is the only option that makes "boot the USB, Arthur answers" true without a new device driver.

- **What A replaces.** ADR-063's objection is boot-time RAM.
- **How today's load works.** The real-brain path reads the whole file into a heap buffer: 768 MiB of heap for a
  485 MB file (`heap_policy.rs:11, 26`). Each weight is then copied into the governed AI arena
  (`kernel/src/ai/model.rs:123-150`).
- **A1, recommended for 0.1: build from the module, then reclaim it.** A boot module takes the place of that file
  buffer. The model is built from the module's bytes exactly as it is from the buffer today, and the module's frames
  are then returned to the frame allocator.
  - The peak at load should be no higher than today's file buffer plus arena. **To verify** with the boot census.
  - After the load, the module memory is gone.
  - Whether the kernel may free Limine's module region, which sits in the executable-and-modules memory-map type,
    is **to verify** from the raw Limine spec. If it may not, A1 holds the module for the whole session, costing
    the model size again.
- **A2, deferred: adopt the module in place.** The arena would reference the module's frames directly, with no copy.
  That means the arena accepting memory it did not allocate, which is a change to ARENA, one of the AI-governance objects
  I-14 protects. It takes the relax-and-re-prove route (Rule 2), not a 0.1 build.
- **What A1 requires.** An ADR-063 amendment, and one new source for the model load: a boot-module range alongside
  the NeoFS file.

### B is still needed

B is needed for 1.0 regardless, for two reasons:
- the installer (OT-65) has to read the USB to copy anything;
- USB persistence needs it.

B is a full driver lane. It needs the driver standing rules: trap-mining, an I-14 analysis and a discriminator bank.
xHCI bulk transfers already exist for `cdc_ecm` and `mt7601u`, which lowers B's risk but does not remove the bring-up
cycles.

**Packaging B.** B also needs an image that is both bootable and carries a data partition. xorriso can append a
partition to an ISO (`-append_partition`; **to verify** from the raw xorriso man page). The kernel would then need to
read the partition table to find NeoFS inside the USB device. Today `find_neofs_block_device` probes whole devices
only (`kernel/src/fs/mod.rs:675`).

### The no-network first boot

A and B both work fully offline. C does not, and C also needs somewhere writable to put the file, which a fresh
machine does not have (see D5).

### Image size budget

The image is the ISO plus the files below. The ISO itself is small; measure it in B1.

| Contents | Approximate size | Fits |
|---|---|---|
| 0.5B (`.neom` + vocab) | ≈ 0.49 GB | a 1 GB stick |
| + 1.5B | ≈ 1.61 GB | a 2 GB stick |
| + 3B | ≈ 3.72 GB | a 4 GB stick, barely |

The vocab is byte-identical across the three tiers (`tools/xtask/src/main.rs:73-77`), so ship it once. An ISO9660 file in one extent holds just under
4 GiB, and larger files need ISO level 3 multi-extent. Some readers also mishandle files of 2 GiB or more. **To verify**
against the raw xorriso docs and against Limine's ISO reader. The 3B `.neom` is 2,099,030,016 B, just under 2 GiB.

### Integrity (OT-73)

**What a known-good value is.** It is a pair, recorded per tier in the source tree:
1. the sha256 of the raw upstream GGUF, from the Hugging Face LFS pointer read with `curl` or `git`, never through a
   summarizing fetch (CLAUDE.md byte-oracle corollary);
2. the blake3 of the `.neom` that `neomc import` produces from it.

The registry entry pins the `.neom` digest (`userland/neo-arthur/src/main.rs:250-283`).

**At load.** The kernel hashes during the single read pass, as planned in OT-73 (`docs/OPEN-THREADS.md:83`), and
compares the result with the pinned digest. On a mismatch it fails closed with the "damaged" message (Q4).

For option A, Limine documents a `#<hash>` suffix on module paths that it checks before handing the module over.
This is **to verify** from Limine's raw `CONFIG.md`. It would be a second, earlier check, not a replacement.

**Falsifier for this approach:** two `neomc import` runs of the same GGUF give different `.neom` bytes. Then the
pinned digest cannot be reproduced from upstream, and the pin has to move to the GGUF sha256 plus a reproducible
importer, which is a prerequisite build.

**Falsifiers for Q1.**
- A fails if UEFI or Limine on the OptiPlex cannot load a ≈0.5 GB module from the USB, whether through a size limit,
  memory-map fragmentation, or file-protocol speed past the boot watchdog. Discriminator: B1's serial breadcrumbs (below).
- A1 also fails if Limine's module memory cannot be reclaimed. The model then costs twice its size for the whole
  session, which on a ≤2 GB machine moves B forward. Discriminator: the memmap census before and after the reclaim.

---

## Q2. Which model 0.1 ships, and how "auto-select the best brain" picks a tier

### Resolve ADR-063 against the locked 0.5B

ADR-063's "1.5B default" is a *preference*. The 2026-07-18 lock is about *proof order*. Both give way to the PL
ruling that the OS picks the best brain the machine can actually run
(`Plans/RESEARCH-2026-07-08-beginner-poweruser-os-substrate-FINDINGS.md:39`). So:
- **The default is replaced by a rule.** ADR-063's fixed default becomes "the largest **shipped and
  silicon-proven** tier that fits".
- **For 0.1, that is the 0.5B.** It is the only tier proven on silicon.
- **The 1.5B joins when it is proven.** It ships in the image after its bare-metal run (OT-28), and the same rule
  then picks it on any machine that fits it.

### The selection rule

The rule must be concrete and testable. With *U* = usable RAM in MiB (the Limine memmap sum,
`kernel/src/mm/mod.rs:158-166`) and *R* = a fixed reserve for everything that is not the model heap:

> Pick the largest tier *T* in the shipped, proven set {1.5B, 0.5B} (3B only if D3 = yes) such that
> **heap(T) + R ≤ U**, its model file is present, and its digest matches.
> Each step down is logged with its reason: `ARTHUR tier=<t> reason=<fits|absent|digest|ram>`.

- **The proposed reserve.** *R* = 1024 MiB to start. It covers kernel non-heap memory, page tables, user processes,
  the framebuffer and the desktop.
- **Calibrating it.** *R* is set from the measured boot census, not guessed: the peak of non-heap frames in use
  during a desktop session plus a 50% margin.
- **Thresholds that follow from it** (using `heap_policy.rs:26, 32, 36`):

  | Tier | Needs usable RAM of at least |
  |---|---|
  | 0.5B | 1792 MiB |
  | 1.5B | 2816 MiB |
  | 3B | 3840 MiB |

**The CPU does not gate a tier in 0.1.** The AVX2 path is default-off (`kernel/src/ai/ops/qk_matmul.rs:997-1012`), and
the scalar forward is the one proven. A tokens-per-second floor measured on the first load could later demote a tier.
That is recorded as a follow-on, not a 0.1 rule.

**The fallback when nothing fits** (*U* < 1792 MiB). Arthur boots in honest no-model mode with the "not enough
memory" message (Q4). It never boots a model it cannot hold.

**The build this rule needs.** The heap budget must become a boot-time choice, not a compile-time one. Today it is
`heap_size_bytes(features)` (`heap_policy.rs:42-59`).

There is a chicken-and-egg ordering problem. The heap is sized before NeoFS can mount, so the kernel cannot check
which files are present when it sizes the heap. The order is therefore:
1. size the heap from *U* for the largest tier that *could* fit;
2. let Arthur pick the largest tier that *is* present, with a matching digest, at or below that size.

**Falsifiers for Q2.**
- A machine with exactly heap(T) + R runs out of memory in a desktop session with tier T. Then *R* is too small.
- Two machines with the same *U* get different tiers. Then the rule is not a function of *U* alone.

The rule's host KAT pins both: given *U*, the tier is fixed.

---

## Q3. Licence and NOTICE files beside the weights

**Shipping the files.**
- **A `MODEL WEIGHTS` section in `NOTICE`.** It follows the firmware section (`NOTICE:33-46`). Per tier it gives:
  - the upstream repository and file;
  - the GGUF sha256 and the `.neom` blake3;
  - the licence, and a statement that this is third-party content and not MPL-2.0.
- **The full licence texts beside the weights.**
  - The Apache-2.0 text for 0.5B and 1.5B.
  - For 3B, if it ships, the Qwen License text *and* its use policy. Its attribution requirement is shown by
    Arthur's `/model` output and the `/licenses` directory.
- **Where they go.** Into `/licenses/` in the image, by extending `LICENSE_FILES` (`tools/xtask/src/main.rs:50-55`).

**Fetching the texts.** Each text is fetched raw from the model's upstream repository with `curl` or `git`, never
through a summarizing fetch. If an upstream repository ships its own `NOTICE`, Apache-2.0 §4(d) requires
redistributing it. **To verify** for each Qwen2.5 repository.

**Bible Article 26** (`docs/bible.md:387`) gets one sentence. Bundled model weights are third-party content that
keeps its own licence, enumerated in `NOTICE`, the same as firmware.

**Falsifier:** an upstream model repository carries a `NOTICE` or licence file that is missing from `/licenses/` in
a built image. The check is a host test that diffs the staged `/licenses/` against the registry's licence list.

---

## Q4. What Arthur says and does with no model, or a failed load

**Today.**
- The desktop path says *"I can hear you, but no model is installed yet."* (`userland/neo-arthur/src/main.rs:790`, logged as
  `model=absent reply_mode=no_model_notice` at `:497`).
- The non-desktop real-brain boot **exits** with code 15 (`:485-487`).
- `/model` reports `load failed <x> active <y>` (`:3015`).

**Proposed.** Four states replace the single "absent" one. Each reply is one plain sentence and one next step. The
detail goes to `/model`, for power users.

| State | Arthur says (the beginner line) | `/model` shows |
|---|---|---|
| absent | "I can hear you, but my model isn't on this system yet. Boot the LogansOS USB it came with." (2026-10-26, J.c.: shortened from "…that came with it." so the row fits the intro card in Inter) | `tier=none reason=absent` |
| not enough memory | "This computer has *N* GB of memory; my smallest brain needs *M* GB. I can still take notes, but I can't think yet." | `tier=none reason=ram U=… need=…` |
| damaged | "My model file is damaged, so I won't use it. Re-create the USB from the original image." | `tier=… reason=digest expected=… got=…` |
| load failed | "My model didn't load, so I'm running without it. The details are in /model." | `reason=load err=…` |

- **The exit-15 path becomes a degrade.** Arthur never takes the session down because its model is missing. That
  is the rule the desktop path already states at `:487-489`.
- **The test.** Each state is a QEMU scenario with the model missing, too little RAM, a flipped byte, or a
  truncated file. Each one asserts the exact line.
- **Falsifier:** a beginner reading only the screen cannot tell which of the four states they are in. In the
  scenarios, the four lines must be pairwise distinct and none may contain an error code.

---

## Q5. Model updates and rollback; the 1.0 installer (scope only)

### A model is a deployment artifact

OT-51's model applies unchanged:
- **write the new tier beside the old one**, as a new, versioned file name: `model-qwen2.5-1.5b-<blake3[:8]>.neom`;
- **verify it as data**, using its digest;
- **flip one pointer**, the registry entry;
- **keep the previous one** until the next boot succeeds (`docs/THINK-2026-07-10-ot51-update-rollback-model.md:144, 149`).

The existing `/model` rule, "a failed load keeps the previous model" (`docs/ADR-063-arthur-boot-model-provisioning.md:35-38`),
is the runtime half of the same idea.

**Out of scope here:** the transport, meaning how a new model reaches the machine. That is option B or C.

### The 1.0 installer (OT-65) inherits three things from this THINK

1. The installer copies the model files and the manifest from the USB to the target NeoFS, and verifies every
   digest on the copy. It needs option B to read the USB.
2. It runs the tier rule once at install time and records the chosen tier. The boot-time rule still re-checks it.
3. It brings over the licence directory as part of the install.

**Falsifier:** a model update that has no way back after a failed first boot. The rollback scenario must boot the
previous tier.

---

## Q6. The read-only model volume vs user data

- **With option A, 0.1 does not need the volume-role selector.** The model is not on the writable volume at all in
  the USB-only case. On the OptiPlex-style two-disk layout, the risk the decision records stays as ratified
  (`Plans/DECISION-2026-08-01-unified-neofs-product-volume.md:20-22`). What changes is that the load-time digest
  (Q1) turns *silent* corruption into a *detected* one, with the "damaged" message.
- **The selector stays a post-0.1 build, as recorded** (`:26`). Option B makes it necessary: once the USB carries
  both a model partition and a persistence partition, the kernel must tell them apart by declared role.
- **Falsifier:** any NeoFS write bug that corrupts the model during 0.1 testing. Then the selector moves before 0.1.

---

## The smallest first build: "boot the USB, Arthur answers"

**B1: one USB image with the model as a boot module, reclaimed after the load.**

**Scope.**
1. `xtask iso --with-model <neom>` stages the `.neom` and `.vocab` as Limine modules, together with the licence files.
2. The kernel builds the model object from the module's bytes, through the existing arena path, then returns the
   module's frames to the frame allocator (A1).
3. Arthur's registry gains a boot-module source for the 0.5B.
4. Integrity: the load hashes the module bytes and compares them with the digest pinned in the registry (the OT-73
   fix, applied to this source).
5. The Q4 messages for the absent and damaged states.
6. The ADR-063 amendment.

**Dependencies.** The pinned digest needs the reproducible-import check from Q1 first. The tier rule (Q2) and the
runtime heap are **B2**: B1 ships the 0.5B only, on its existing compile-time heap.

**Kill-switch.** The `model-boot-module` feature. Off, the image and the kernel are bit-for-bit today's: no module
staged, registry unchanged. A regression scenario proves that (Rule 1).

**Discriminator (QEMU).**
- `arthur_boot_module_answers`: boot the image with no NeoFS disk attached. Arthur loads the module, the digest
  matches, and a real model reply appears.
- Controls:
  - a flipped byte must give the "damaged" line and no reply;
  - a missing module must give the "absent" line.
- A frame-count witness proves the module was reclaimed: free frames after the load are at least
  (before the load − arena use), within a small bound.

**Gates.** It touches the kernel, so it needs the 4-shard QEMU gate and an AAG that includes a different-family
lens on the module-reclaim code (memory-safety: no live reference into reclaimed frames, the frame lifetime).

**Cycles.** One build cycle plus one bare-metal run. More than one bare-metal cycle is expected if the module load
trips a firmware limit. Those cycles are what the bank below exists to localize.

### Blind-hardware failure-mode bank (the OptiPlex run)

The run procedure is `docs/runbook-baremetal-arthur-optiplex.md`. Read it with
`git fetch origin && git show origin/main:docs/runbook-baremetal-arthur-optiplex.md`. Serial is primary; the NVMe
sink corroborates. For this run, **detach the SATA model disk**, so the USB is the only source.

| # | Likely failure even if B1 is right | Discriminator (captured before it is needed) |
|---|---|---|
| F1 | Limine fails to load the ≈485 MB module (size limit or memory-map fragmentation) | Limine's own error on screen; the kernel never starts (no first serial line). Pre-seeded: log the module count and sizes in the first kernel breadcrumb. |
| F2 | The module loads but the firmware or Limine is too slow, and a watchdog or timeout fires | Timestamps on the breadcrumbs from "kernel entry" to "module seen"; compare with the QEMU baseline. |
| F3 | The module's memory-map region cannot be reclaimed, or reclaiming it frees frames still mapped elsewhere | Breadcrumb `module base=… len=… memmap_type=…` before the load; free frames before and after the reclaim; a fail-closed refusal to reclaim any type other than the expected one. |
| F4 | A digest mismatch on silicon only (a bad stick, or a truncated `dd`) | `ARTHUR digest expected=… got=…`. Then re-hash the stick from Ubuntu with `b3sum` to tell the stick apart from the code. |
| F5 | Out of memory at load: the module, the compile-time 0.5B heap and the arena copy all live at once | The memmap census line (`usable physical memory`, `mm/mod.rs:165`) plus heap ready (`heap.rs:174`) plus module bytes plus arena use: does the peak sum exceed usable? |
| F6 | The kernel waits for a NeoFS disk that is now absent | `boot-neofs no block device` handling, which is already a scenario family (`kernel/src/fs/mod.rs:677-682`). The breadcrumb must show it continuing. |

**Trap-mining before the run.**
- Limine's module-loading limits and its `#hash` path suffix, from its raw `CONFIG.md`.
- The documented UEFI quirks for large files read from USB on Dell OptiPlex firmware.
- xorriso's and Limine's handling of files near 2 GiB and 4 GiB (only relevant once the 3B ships).

---

## Build DAG (no calendar time)

```
reproducible-import check ─┐
                           ├─► B1 (boot-module model + reclaim, 0.5B, digest, Q4 absent/damaged, licences) ─► OptiPlex run
OT-73 hash-in-load-pass ───┘                                                                         │
B2 (runtime heap + tier rule + all Q4 states) ◄──────────────────────────────────────────────────────┘
1.5B bare-metal run (OT-28) ─► 1.5B joins the image
B (USB mass storage + partition probe) ─► OT-65 installer ─► model updates over B/C (OT-51 shape)
```

## Open threads touched

| Thread | How this THINK affects it |
|---|---|
| OT-28 | The 1.5B silicon run gates its entry into the image. |
| OT-38 | The 3B needs a licence decision (D3) and a ≥3840 MiB machine. |
| OT-51 | Models fit its write-beside, verify, flip shape. |
| OT-65 | The installer inherits Q5's three items. |
| OT-73 | Its fix is a B1 dependency. |
| OT-81 | B1's xtask path documents and checks where the model comes from. Running `xtask iso --with-model` with a missing path fails loudly. |

ADR-063 needs an amendment (B1).
