<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: B1 acceptance on the OptiPlex — Arthur thinks from the USB stick

**Type:** a joint bare-metal run, PL + coder. **Coder:** Janus (he made the model files in B1-1).
**Deliverable:** one pushed RETURN with the three serial logs.

**What this proves (the acceptance bar for B1a + B1b + B1c):** the 0.1 USB image boots on real
silicon with no model disk:
- Limine loads the 463 MiB model as a boot module;
- the kernel checks its BLAKE3 pin, builds the model, and gives the module's memory back;
- the first-boot intro shows;
- Arthur answers a typed question with the real 0.5B.

Merging B1 claimed none of this. This run is the claim.

**Read first:**
- `docs/runbook-baremetal-arthur-optiplex.md`: access, disks, PuTTY, boot control.
- `docs/B1-boot-module-model.md`: the images, the minimum RAM, and the failure bank F1–F7.

```
git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-20-b1-optiplex-acceptance-run.md
git show origin/main:docs/runbook-baremetal-arthur-optiplex.md
git show origin/main:docs/B1-boot-module-model.md
```

## Three images, three boots

| Run | Image | Why | Serial | NVMe sink |
|---|---|---|---|---|
| **A** | The product image (`xtask iso --with-model`) | **The acceptance itself**: the desktop, the intro, and Arthur answering | yes | **no**: the product kernel installs no log sink |
| **B** | The instrumented bundle `arthur_boot_module_answers` | The reply **text** on serial, a breadcrumb on every step, and the sink as a second copy | yes | yes |
| **C** | `real_lm_peak_baremetal` | The 305 MiB heap tier's release gate: a 2,048-token prompt and 256 new tokens; it records the inference-stack high-water and the TSC cycles | yes | yes |

Run all three, in order, even if A passes.
- **Why B and C are not optional:** the product image prints neither the reply text nor per-token
  progress on serial. It has no sink either.
- **What B and C add:** B gives the text, the sink and the per-step breadcrumbs. C gives the memory and
  stack numbers.
- **Capture-before-you-need-it:** a failure in A is then already localized by B and C, with no new code
  and no second visit.

## Coder: build the three images (Janus)

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/b1-optiplex-acceptance-janus  (git fetch origin && git checkout -B codex/b1-optiplex-acceptance-janus origin/main)
```

In PowerShell:

```powershell
cd "E:\AI Automation\workers\janus"
git fetch origin; git checkout -B codex/b1-optiplex-acceptance-janus origin/main

# 1. Copy your retained B1-1 files into the tree (the build container only sees the tree).
New-Item -ItemType Directory -Force target\b1-model | Out-Null
Copy-Item "E:\AI Automation\b1-1\run1\model-qwen2.5-0.5b.neom", "E:\AI Automation\b1-1\run1\model-qwen2.5-0.5b.vocab" target\b1-model\
b3sum target\b1-model\model-qwen2.5-0.5b.neom target\b1-model\model-qwen2.5-0.5b.vocab
#   must print:  a32e4a1ba94a850724c15d840f3ae2c44902d9f1f34d8069bac5fcb37e1ccafe  (.neom, 485,489,152 bytes)
#                e89741214838ae7af6277122c1093e899138884ea3bea4f83ff5671e76c05265  (.vocab, 7,990,862 bytes)

# 2. Run A: the product image  ->  boot\neoplutarch.iso
docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom
Copy-Item boot\neoplutarch.iso target\b1-model\A-product.iso

# 3. Run B: the instrumented bundle
docker compose run --rm -e NEO_STAGE_B_NEOM=target/b1-model/model-qwen2.5-0.5b.neom neo `
  cargo xtask neo-test --build-only --bare-metal-mode arthur_boot_module_answers
Copy-Item target\neo-test\vmware\arthur_boot_module_answers\arthur_boot_module_answers.iso target\b1-model\B-bundle.iso

# 4. Run C: the peak run (an ignored scenario, so it needs --include-ignored)
docker compose run --rm -e NEO_STAGE_B_NEOM=target/b1-model/model-qwen2.5-0.5b.neom neo `
  cargo xtask neo-test --build-only --include-ignored --bare-metal-mode real_lm_peak_baremetal
Copy-Item target\neo-test\vmware\real_lm_peak_baremetal\real_lm_peak_baremetal.iso target\b1-model\C-peak.iso
```

- **If xtask refuses a file** (wrong size, pin mismatch, or `F7` for too many modules), stop. The error
  names the reason, and that is a finding for the RETURN. Never edit a pin to make a build pass.
- **If a bundle's ISO is not at that path,** use the path xtask prints.
- Record `git rev-parse HEAD` for the RETURN; the images are built from that `main`.

## PL: the run, step by step

**Once, before the first boot:**
1. **Power the OptiPlex off.** Unplug the SATA data cable from the Samsung 860 EVO (the model disk).
   - It stays unplugged for all three runs; this proves the model comes from the USB stick and nothing
     else.
   - Leave the internal NVMe in place: Runs B and C write their log to it.
2. **Write `A-product.iso` to a spare USB stick** with Rufus. Pick the ISO, press Start, and choose
   **"Write in DD Image mode"** when asked.
   - Not the Kingston stick: that one is the Ubuntu stick we need afterwards.
     *(Stale since 2026-10-21: the Kingston stick was overwritten and is now the Zenbook image stick; see `docs/runbook-baremetal-zenbook-s14.md`.)*
   - The same spare stick is rewritten with `B-bundle.iso`, then `C-peak.iso`, for the later runs.

**Each run:**
1. **Start PuTTY on the OptiPlex's serial port, logging to `C:\Users\<user>\neoplutarch-serial.log`**
   (runbook, "Serial capture"). Make sure logging is on before you power up.
2. **Power on, tap F12, and choose the spare USB stick** (the UEFI entry). Note the time you press
   Enter. **F2:** the coder needs the seconds from here to the first serial line.
3. **Watch, and do not halt early.** A real 0.5B answer takes minutes. A hang shows as the serial log
   stopping, not as a slow screen.
   - **Run A:** the desktop comes up with the intro card. Click into Home (or just start typing), type
     `What is the capital of France?`, and press Enter. Wait for Arthur's answer in its window.
     **Read it by eye. Is it real English?** Then press nothing else for a minute.
   - **Run B:** there is nothing to do on the OptiPlex itself. **In the PuTTY window** type `arthur`
     Enter, then `hello world` Enter, wait for the reply, then `exit` Enter.
   - **Run C:** type nothing. It runs on its own and prints a PASS or a FAIL line.
4. **Save the PuTTY log, then rename it** to `run-A.log`, `run-B.log` or `run-C.log` in the same folder,
   so the next run doesn't overwrite it.
5. **Power off.** For B and C, F12 into the Kingston Ubuntu stick afterwards, so the coder can read
   the NVMe sink over SSH (runbook, "Disk layout").

**No photos, ever.** Your live read of the screen in Run A, plus the serial logs, is the proof.

## What a pass looks like on serial (the witness lines)

**Run A (product).** Each line is printed by the product build. The kernel lines come in this order; the
userland lines (`BOOTMODULE`, `ARTHUR`, `DESKTOP`) can interleave with each other:
```
BOOTMOD count=<n> capacity=8                                       n ≤ 8
BOOTMOD module path=/models/model-qwen2.5-0.5b.neom bytes=485489152 …
BOOTMOD module path=/vocab/model-qwen2.5-0.5b.vocab bytes=7990862 …
[ kernel ] mm: primary zone … max_order=16                          note trim_pages
[ kernel ] inference stack: 256 KiB at …
[ kernel ] boot-neofs: no valid NeoFS superblock; boot continues   (F6: the NVMe holds an ISO, the SATA is unplugged)
BOOTMODULE digest path=/vocab/model-qwen2.5-0.5b.vocab bytes=7990862 expected=e8974121… got=e8974121… verified=true
CENSUS label=model_before_take …
BOOTMODEL take path=/models/model-qwen2.5-0.5b.neom bytes=485489152
BOOTMODEL digest path=/models/model-qwen2.5-0.5b.neom expected=a32e4a1b…1ccafe got=a32e4a1b…1ccafe match=true
CENSUS label=model_built …
BOOTMOD reclaim path=/models/model-qwen2.5-0.5b.neom bytes=485489152 result=Ok(<pages>)
CENSUS label=model_reclaimed … admitted_pages=<≈118,000> …
BOOTMODEL loaded path=/models/model-qwen2.5-0.5b.neom bytes=485489152 reclaimed_pages=<pages>
ARTHUR model source=boot-module state=ready
DESKTOP intro shown variant=ready usb=1 rows=<n> persistent=0
DESKTOP arthur line=What is the capital of France?
decode done tokens=<N> ms=<M> per_token_ms=<K>                     N > 0: a real forward ran
```
- **What the product build doesn't print:** the reply text. So **Run A's pass is the serial lines above
  plus your live read** of a sensible English answer.
- **`heap_high_water=0` in the product census is expected:** it is tracked only in instrumented builds.

**Run B (bundle):** the same `BOOTMOD…`, `BOOTMODEL…` and `CENSUS…` lines, plus:
- `neofs_mount skipped source=boot-module`;
- Arthur's reply text after `hello world`;
- `NEO_TEST/1 event=PASS name=arthur_boot_module_answers … source=boot-module digest=match module_reclaimed=true neofs=absent reply=model`.

The NVMe sink holds the same text (`sudo -n dd if=/dev/nvme0n1 bs=1M skip=256 count=16 2>/dev/null | strings`).

**Run C (peak):**
- `CENSUS label=real_lm_peak_start`, then `real_lm_peak_loaded`, then `real_lm_peak_end`, each with its
  `inference_stack_high_water` companion line;
- `real_lm_peak prompt=2048 new=256 tsc_cycles=<c> inference_stack_high_water=<b> first=[…]`;
- `NEO_TEST/1 event=PASS name=real_lm_peak_baremetal …`.

## Failure modes and their discriminators (pre-seeded; screen vs serial)

| # | Failure even if B1 is right | Screen (PL, live) | Serial / sink (coder) |
|---|---|---|---|
| F1 | Limine cannot load the ≈485 MB module | Limine's own error, and the kernel never starts | No first kernel line. If the kernel does start: `BOOTMOD count=` and one `BOOTMOD module` line per module, before any byte is touched |
| F2 | The firmware or Limine is too slow reading 485 MB from USB; a watchdog fires | A long black screen, or a reset loop | **Timed externally:** the seconds from Enter in the F12 menu to the first serial line (PuTTY timestamps or a stopwatch). Compare with QEMU's 132 s for the whole fixture scenario. The kernel has no clock that early |
| F3a | The reclaim is refused | Nothing visible; Arthur still answers | `BOOTMOD reclaim … result=Err(Plan(<rule>))` or `Err(Admit(…))`, and `admitted_pages=0`. It fails closed, and the rule is named |
| F3b | The reclaim admits pages still in use | A crash, or a garbage answer after the load | A trap after `CENSUS label=model_reclaimed`, or `refused_frees>0`. Run B's reply text tells garbage from English |
| F4 | A digest mismatch on silicon only (a bad stick, a truncated write) | Arthur says its model is damaged | `BOOTMODEL digest … match=false`, then `ARTHUR model … state=damaged`. The coder re-hashes the stick from Ubuntu with `b3sum` to tell the stick apart from the code |
| F5 | Out of memory at load | Arthur says its model failed to load | `BOOTMODEL load_failed … err=Tensor(OutOfMemory)`, then `state=load_failed`. `model_before_take` against `model_built` shows which term ran out |
| F6 | A wait for a NeoFS disk that is absent | The desktop stalls before Home | Product: `[ kernel ] boot-neofs: no valid NeoFS superblock; boot continues` (the NVMe holds an ISO; the SATA is unplugged). Bundle: `neofs_mount skipped source=boot-module`. A stall after neither line is this mode |
| F7 | Too many modules | — | xtask refuses at build time (`F7` in the error). On boot, `BOOTMOD count=<n> capacity=8` prints before any panic |
| B1c-1 | The inference stack has no slot (the KASLR placement fixed in B1c) | A panic screen at boot | `inference stack: no unmapped region beside the kernel heap`. Fixed; a recurrence is a finding |
| B1c-2 | A forward overflows the inference stack | The screen freezes mid-answer | `[ trap  ] inference stack overflow guard=Low`, then a halt (the guard page turns it into a named `#DF`). Run C's `inference_stack_high_water` shows the margin left from 262,144 |
| B1c-3 | No order-16 block for the 256 MiB `output.weight` tensor (fragmentation, or a big trim) | Arthur says its model failed to load | `BOOTMODEL load_failed … OutOfMemory`, **with** `tensor_block_capacity_pages` in `model_before_take` well above 65,536. Enough pages but no aligned block means fragmentation, not size. `trim_pages=` on the `primary zone` line gives the trim |
| B1c-4 | The inference-stack lock wedges (`BUSY` is never released) | "Thinking..." forever | `DESKTOP arthur line=…` with no `decode done` after it, while the serial stays quiet. To tell it from a slow answer: Run B prints every step, and Run C's `tsc_cycles` gives the real per-token rate. A wedge that also stalls in B is a finding |
| B1c-5 | Arthur generates before its model is ready | An error answer instead of a reply | Line order: `ARTHUR model … state=ready` must come **before** the first `DESKTOP arthur line=`. Arthur loads the model before its input loop, so the reverse order is a finding |

**Not this run's failure:** a NeoFS mount (none is used), WiFi or Ethernet, and the 1.5B/3B (B1 is
the 0.5B only).

## Coder: after each run

1. **Copy the three logs** from `C:\Users\<user>\` into `Plans/evidence/b1-optiplex/` on your branch:
   `run-A.log`, `run-B.log`, `run-C.log`.
2. **For B and C, also save the NVMe sink text** over SSH:
   `ssh -i ~/.ssh/id_ed25519 ubuntu@<the OptiPlex's address> "sudo -n dd if=/dev/nvme0n1 bs=1M skip=256 count=16 2>/dev/null | strings" > run-B.sink.txt`
   - Confirm the address first; it moves (runbook).
   - Save `run-C.sink.txt` the same way after Run C. Run C writes over B's sink, so save B's first.
3. **On any `match=false`,** also run `b3sum` on the stick's `/models/` and `/vocab/` files from Ubuntu.

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-20-b1-optiplex-acceptance-run.md` and the evidence folder to
`codex/b1-optiplex-acceptance-janus`, and **push**. The RETURN states:
- **The verdict per run:** PASS, or FAIL with the failure-bank row it matched (or "new mode").
- **Run A:** the PL's one-line read of Arthur's answer, and the witness lines found or missing.
- **The numbers:**
  - F2: seconds to the first serial line;
  - `trim_pages`, `admitted_pages` and `reclaimed_pages`;
  - `frames_in_use` at each census;
  - the load-time peak, as `max(frames_in_use × 4096 + unreclaimed module bytes)`, against the doc's
    ≈1.47 GiB estimate and its 1.75 GiB falsifier;
  - Run C's `inference_stack_high_water` and `tsc_cycles`.
- **The build:** the `main` SHA the images came from, and the two `b3sum` lines.

Paste the pushed commit SHA in your reply to the PL. The PM records the acceptance milestone from
the RETURN.

## Disk reclaim

- **The model files:** delete `E:\AI Automation\b1-1` **only after this run's RETURN is pushed.** It
  holds the only local copy of the retained B1-1 imports, and this run needs them.
- **Your tree:** delete `target\b1-model` and run `cargo clean`. Never delete the worktree itself.
- **The SATA model disk:** plug it back in after the run if other lanes still need it (NeoFS runs do).

## RAG

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?" questions
(faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='boot module model load digest reclaim census'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`
