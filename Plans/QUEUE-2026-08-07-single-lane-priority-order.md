<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# QUEUE — single-lane priority order (PL directive, 2026-08-07)

**Directive:** *"lets bring the project back online — however for now I want to keep it 1 coder at a time, lets work our way down from most important to least important."*

**Operating mode:** **one seat active at a time.** The PM dispatches item 1 only; item 2 is not relayed until item 1 merges. This is the standing order so priority is not re-derived every time a lane closes — it is amended when reality changes, not rebuilt from scratch.

**Why single-lane is cheap right now:** the binding constraint is runner capacity (**4 QEMU slots** since the M75q left the pool), and a 4-shard gate saturates it. With one lane there is no gate contention, no rebase treadmill, no stacked-branch orphaning, and the tree-state audit is trivial. Throughput loss is smaller than it looks.

---

## The order

### 1. 0.1 integration build — the `product` ISO, all five rungs, QEMU-proven  ✅ **COMPLETE 2026-08-07**
**Landed in three parts, all Chad, all gate-verified on the exact SHA and PM tree-identity-checked at merge:**
- **1a `966bdabf`** — `product` goes from built-by-nothing to a **blocking** CI job; five-rung `product_integration` scenario; all three `boot-neofs` degrade arms registered as real scenarios. **Closes OT-78.**
- **1b `b89fa226`** — AAG (Cerebro) returned MERGE + 2 findings. **F1: rung 3 asserted the prompt at the sender only** — Arthur fired on any non-empty input, never compared the buffer, and the fixture picked the same token regardless, so dropped keystrokes still went green while two strings claimed `typed_prompt=hello_world`. Now validates the **received** buffer; dropped/reordered keystrokes return 61 and turn the rung red. **F2:** boot 1 relabelled `same_boot_fsync_readback` (it had claimed a two-boot readback on the line carrying all five rung tokens — the line a reader would cite).
- **1c `2d630348`** — **found by Chad's own disclosure that CI needed 4 attempts.** `product_integration` had no budget arm and fell to `TEST_TIMEOUT = 180 s` while taking **176.9 s ⇒ 1.02× headroom** — *less margin than the scenario we removed a machine from the pool over.* Now **420 s per boot**, sized from measurement: worst observed **146.6 s ⇒ 2.86× headroom**; attempt-1 rate 3/3 local + 1/1 CI; 4-way partition complete and disjoint at 7485/7481/7442/7440.

**Worth keeping:** F1 was the **seventh** instance of the week's assertion-with-nothing-behind-it class **and the first to survive a dispatch that explicitly demanded it not happen** — stating the requirement is not the same as testing for it, and the AAG's *mutation table* is what caught it. The 1c budget defect was caught by the coder disclosing a retry he could have reported as green.

**Carried into item 3 (bare-metal), from AAG lens 5:** rung 4 asserts the **no-dongle degrade by equality**, so this scenario **goes red on hardware with a working AX88179** and cannot be reused as the hardware rung-4 proof; and **the booted binary is `neo-test-product-integration`, not the shipping artifact** — `product-build` compiles the ISO's composition but nothing boots it.
**Why first:** it is the thing that ships 0.1, and it is also the fix for **OT-78** — *the configuration that actually ships is the one configuration nothing builds.* You cannot make a five-rung showcase ISO without building `product`, and no CI job or xtask scenario does. This single build closes a coverage hole and produces the artifact the bare-metal run needs.
**Also closes:** the three `boot-neofs` degrade arms and the device-selection probe path, all currently exercised by nothing.
**Seat:** Chad — owns eth0 (rung 4) *and* rung 5, and composed the `product` feature when the two collided on rebase. Most context, least re-derivation.

### 2. OptiPlex disk prep + OT-73 offline verification — **PL-joint, no code first**  ✅ **COMPLETE 2026-08-11**
The physical `sda` is the **`normal`** mkfs variant: `hello.txt` and `model.neom`, **no `.arthur/memory`**. Rung 5's demo is *"Arthur remembers you across a reboot,"* which needs that directory. The obvious fix — re-`mkfs` — **destroys `model.neom`**, and under the unified-volume decision that disk *is* rung 3.
**Do the cheap thing first:** verify `model.neom`'s hash **offline over SSH from the live USB** before any code flips verification on (OT-73). Three outcomes: matches ⇒ OT-73 lands risk-free; mismatches ⇒ real corruption found *before* it could be blamed on new code; never populated ⇒ a re-`mkfs` is a prerequisite and that is a scheduling fact worth knowing now.
**Then decide:** add the directory in place, or re-`mkfs` with the model re-staged from a backup taken off-box first.

**✅ INTEGRITY PHASE COMPLETE 2026-08-11 — the volume is fully verified and clean.** Offline inspection ran read-only over SSH (`tools/neofs-inspect.py`). **Every layer the format defines now checks out:** three valid superblock copies at gen=1, `root_inode_hash` MATCHES, free map MATCHES (6 blocks @ 16385), all five inode checksums pass, and **all five content hashes verify — including all 485,489,152 bytes of `model-qwen2.5-0.5b.neom` against `a32e4a1b…`**. 768 MiB extent, 535.6 MiB used. The root holds **five** entries, not two; the earlier 512 MiB string-scan simply ended before `model-qwen2.5-0.5b.neom`, its `.vocab` sibling, and `subdir`.

**⇒ The remaining work in this item is purely a FORMATTING question, not an integrity one.** The clean bill of health means the on-disk model is a **trustworthy source to re-stage from**, which makes the rebuild cheaper and removes any need to regenerate from the source GGUF.

> **⚠ BEFORE TOUCHING THIS VOLUME: `/model.neom` (4,100 B) and `/hello.txt` (37 B) are LOAD-BEARING.**
> `validate_children` (`kernel/src/fs/neofs/mount.rs:332`) fails the mount without a root entry named
> **literally `model.neom`** of kind `Model` (`UnsupportedType`), or without `hello.txt` of kind
> `Regular` (`NotFound`). The real model is `model-qwen2.5-0.5b.neom` and does **not** satisfy that
> check. Both files look like leftover `neofs-mkfs` scaffolding and are not — a re-`mkfs` that stages
> only the real model, or an in-place tidy that deletes the 4 KB stub, kills rung 5 with an error
> that names neither file. This is the concrete trap the "verify offline first" step existed to find.

**⇒ DECISION: rebuild the volume with `mkfs`; do NOT tidy it in place.** Two code facts settle it, both
PL-found and PM-confirmed:

1. **`mkfs` cannot omit the trio.** `format.rs:1264` builds `names` as an unconditional
   `vec!["hello.txt", "model.neom", "subdir"]` and only ever *appends* `.arthur`, with a type check at
   `:1262` and a post-format verifier at `:1159`. So a re-`mkfs` **structurally satisfies** the
   mount-required names — the constraint that looks like an argument against re-`mkfs` is actually
   discharged by it. **The in-place tidy is the only path that can violate it**, because it bypasses
   all three guards.
2. **`.arthur` is absent because the volume was formatted without the flag**, not because something
   removed it — `include_arthur_memory` / `_reversibility` / `_stakes_policy` gate it
   (`format.rs:1162-1175`). Adding it is a format-time option, so it wants a rebuild regardless.

**Do the rebuild on the BUILD BOX, not on the disk.** `neofs-mkfs verify` works fine on a regular file
(the `stat()`-returns-0 problem is block-device-only), so the replacement image can be verified
**completely — including the 485 MB content hash at native speed — before it ever touches `sda`**. The
plumbing exists: `prepare_neofs_storage_disk_with_model_files(path, variant, model, extra_files)` in
`tools/xtask/src/main.rs` already takes a variant plus extra model/vocab files, and `NEO_STAGE_B_NEOM`
(`:59`) supplies the real `.neom`. Write to the disk only after the file verifies.

**This also de-risks the pending hash.** If the volume is rebuilt from the source model on the build
box, a corrupt on-disk blob is *overwritten* rather than trusted — so the hash stops being a gate on
the disk prep. What it still answers is the sharper question: **has this NeoFS volume been silently
corrupting data at rest?** A mismatch there is a filesystem finding, not a model-provenance one, and
under the unified-volume decision that is the more important of the two.

**VARIANT SETTLED — `arthur-memory`, and the mismatch is confirmed.**
`tools/xtask/src/main.rs:7269` maps scenario → mkfs variant, and the entry is
**`"product_integration" => Some("arthur-memory")`**. So the QEMU scenario that proves the five rungs
formats its volume `arthur-memory`, while the physical `sda` is `normal` — **the disk does not match
the configuration the gate validates.** Corroborated from the other direction by what the code opens:
rung 5's persistence path is `/neofs/.arthur/memory/product-0.1.persist`
(`userland/neo-init/src/main.rs:390`), and `arthur_compositor.rs:36` + `memory_logic.rs:10` both root
at `/neofs/.arthur/memory`. `arthur-reversibility` is **not** wanted here — it adds `.arthur/undo`
for `arthur_reversibility`, a different scenario.

**Recipe — EMPIRICALLY VERIFIED 2026-08-11, not inferred from the source:**

```
neofs-mkfs create --output <img> --variant arthur-memory --size-mib 768 \
  --extra-model model-qwen2.5-0.5b.neom=<real .neom> \
  --extra-file  model-qwen2.5-0.5b.vocab=<vocab>
```

Built and inspected; the variant path does **not** diverge from the required-name list. Produces
`hello.txt` · `model.neom` (4,100 B) · `model-qwen2.5-0.5b.neom` · `model-qwen2.5-0.5b.vocab` ·
`subdir` · `.arthur` · `.arthur/memory`, and `neofs-mkfs verify` passes. It reproduces the physical
volume's layout exactly — `total_blocks=196608`, `root_inode_block=16391`, free map 6 blocks @ 16385
— i.e. **the same volume plus `.arthur/memory`.**

**Do NOT pass `--model-path`.** Left at its default, `model.neom` keeps the 4,100-byte stub content
that hashes to `428e958f…`, byte-identical to what is on `sda` today. The real model goes in through
`--extra-model`; `--model-path` would overwrite the stub the mount-time name check depends on being
cheap.

**Which source to re-stage from — decided by a check, not a judgement.** `content_hash` is
`BLAKE3(file bytes)`, so the verified on-disk value is now a **fingerprint for the build-box copy**:
a local `.neom` that hashes to `a32e4a1ba94a850724c15d840f3ae2c44902d9f1f34d8069bac5fcb37e1ccafe`
is byte-identical to what is on `sda`. Match ⇒ use the local copy, no 485 MB transfer, provenance
confirmed from both ends. Missing or mismatched ⇒ `tools/neofs-inspect.py /dev/sda --extract
model-qwen2.5-0.5b.neom=<path>` (the live USB's root is tmpfs — extract somewhere with real space).
The same check applies to the vocab, whose recorded hash the inspector prints.

Verify the resulting **file** with `neofs-mkfs verify` (full content hashes at native speed — the
`stat()` limitation is block-device-only), then write to `sda`.

**This is why item 2 was sequenced before the bare-metal run.** The mismatch is invisible from the
booted system — rung 5 would fail on hardware at the persistence step, after four rungs had already
passed, on the run that was supposed to be the acceptance.

**✅ DONE 2026-08-11 — `sda` now carries the `arthur-memory` volume, verified end to end.** Read-back
is byte-exact: `md5(first 768 MiB of sda) = c213b498…`, identical to the built image, taken **after
dropping caches** so it is a real SSD read rather than page cache. Fresh inspection of `/dev/sda`
confirms three valid superblocks, `free_map MATCHES`, `root_inode_hash MATCHES`, and all seven entries
with **`.arthur/memory` present**.

**The chain, each link checked against the one before:**

```
sda(old) → extract → transfer → hash --expect → mkfs → verify → write → sda(new)
 a32e4a1b          485,489,152 B    exit 0    recorded a32e4a1b   c213b498 both ends
```

Every original entry's hash survives unchanged — `hello.txt`, the 4,100 B `model.neom` stub, the
model, the vocab, `subdir`. The stub is byte-identical because `--model-path` stayed at its default,
so the mount-time name check still resolves to a **4 KB read**. `.arthur/memory` is the sole addition,
which is exactly the mismatch this item existed to fix. Pre-write guard confirmed `sda` was unmounted,
still carried `NEOFSv1`, and logged the device identity (Samsung SSD 860, 488,397,168 sectors) so the
target could not have been a renamed device.

**Reproducible, not single-copy.** `mkfs` is deterministic by spec (§6; `neofs-mkfs sanity` proves it
by building twice and comparing), the model + vocab now live on the build box under
`E:\AI Automation\Models\`, and the recipe above is committed — so the volume can be rebuilt with one
command from a machine that stays powered. The OptiPlex's copy lived in tmpfs and does not survive the
item-3 reboot; that is fine and was not worth preserving.

**Carried into item 3:** the inspector must be re-copied to the box after the reboot (tmpfs wipe), and
it then supplies an **independent, offline rung-5 proof** — read `/neofs/.arthur/memory/product-0.1.persist`
off `sda` from the live USB and verify its content hash, rather than trusting a serial line that claims
persistence worked. The pre-run baseline is fully recorded above, so any post-run difference is
attributable.

### 3. 0.1 bare-metal showcase — one ISO, **TWO boots**, five rungs  ⬅ **ACTIVE — 3b DISPATCHED 2026-08-26**

> **→ 3b DISPATCHED 2026-08-26 — `Plans/DISPATCH-2026-08-26-3b-product-iso-burn-and-baremetal-run-chad.md` → Chad + PL (joint).**
> Build the artifact from current `main`, **prove the burn** (`dd` to `nvme0n1`, hash-back after a cache
> drop, stamp grep against the *device*), then the acceptance run: **two boots of one ISO**. Carries the
> three pre-burn captures (`.arthur/memory` empty, sink baseline zero, the box's DHCP address), the
> nine-mode discriminator bank, the volume-reset path for a boot-1 retry, and the **runbook amendment**
> the lane owes — the runbook is single-boot throughout, has **no burn procedure at all**, and still
> says `sda` has no `.arthur/memory` (item 2 closed that on 08-11). Docs-only commit; no QEMU gate owed.
> **RETURN:** `Plans/RETURN-2026-08-26-3b-product-iso-burn-and-baremetal-run-chad.md`.

> **✅ 3a MERGED 2026-08-11 — `7b2ada27` (Chad).** `product_integration_baremetal` exists: single-boot
> per invocation, `ignored: true`, NVMe-sink-only, deliberately absent from `is_reboot_persistence_scenario`.
> Gate `31454931270` re-verified by the PM on the exact SHA `71641e94` — 13 jobs, 0 failed, **all four QEMU
> shards genuinely executed** (20–31 min each). AAG: 5 lenses, no surviving findings; the sharding attack was
> refuted by `select_test_scenarios` filtering `!s.ignored` upstream of `shard_partition`, with the partition
> weights byte-identical to 1c (72/7485, 72/7481, 72/7442, 74/7440). **The gate did NOT execute the new
> scenario** (`ignored: true`) — the first exercise is the OptiPlex boot.
>
> **⛔ THE BURNED ISO IS STILL THE STALE ONE. Build and re-burn before booting.**
>
> **The burned ISO predates item 1.** Build stamp `2026-08-01-01-52-04`; 1a/1b/1c landed 08-07. PL's
> `strings` check on the kernel: `"neoplutarch|neofs|kernel"` → **71,200 hits** (positive control — the
> pipeline works and it is a NeoPlutarch kernel), while `"PRODUCT rung"`, `"product_integration"`,
> `"SHOWCASE product"`, `"DW1 focus"`, `"arthur"` → **0 each**. A kernel with no `arthur` string was not
> built with `neo-test-arthur-compositor`. **Booting it would produce a log with zero rung tokens —
> not a failed acceptance run but an *uninterpretable* one**, which is precisely what the runbook's
> conclusive-either-way rule forbids.
>
> **And `--bare-metal-mode` is rejected twice, in two different code paths**, so fixing the first only
> defers the second: the sink allowlist during feature assembly (`xtask` ~`:3336`) fails at build; the
> reboot-scenario guard (~`:4063`) would then fail at run with *"--bare-metal-mode is not supported for
> reboot scenarios."* `product_integration` is in `is_reboot_persistence_scenario` (`:4290`).

**3a — the `product_integration_baremetal` lane (code, own dispatch, QEMU-gated).**
**Design decided: SINGLE-BOOT PER INVOCATION, booted twice by hand — not two-boot orchestration.**
`userland/neo-init/src/main.rs:252` selects the arm **from disk state**, not from a host-injected boot
index: `read_prior_boot()` returns `None` on `ENOENT` and `Some(_)` once the file exists. **The
two-boot driver only exists because QEMU has no human.** At the box the PL is the driver, so the
variant needs no reboot machinery — and therefore is **deliberately NOT added to
`is_reboot_persistence_scenario`**, which dissolves the `:4063` guard rather than relaxing it.

What the lane needs:
- the `product_integration_baremetal` scenario + kernel feature + user feature + **sink allowlist entry**;
- **NVMe sink, not AHCI** — NeoFS lives on `sda`, a SATA/AHCI device, so an AHCI sink would collide with
  the volume under test. The boot NVMe is the only free channel and already carries the documented
  `NEO_LOG_v1` sink at 256 MiB;
- a runbook amendment for **two captures** (its procedure is single-boot throughout).

**Run procedure — three facts that otherwise misfire:**
1. **Boot 2's log is SUPPOSED to be short.** It short-circuits before the desktop and Arthur, emitting
   only `readback bytes=39 source=prior_boot byte_equal=true fsync_prior_boot=true`, and returns **78 —
   a nonzero success**. One line, not a second five-rung block. A correct boot 2 otherwise reads as a
   failure.
2. **The run is NOT idempotent.** Boot 1 exercises rungs 2 and 3 only while `.arthur/memory` is empty.
   It is empty now. Any retry after a completed boot 1 takes the readback path and **silently skips the
   desktop and Arthur**. A re-run requires resetting the volume first — which is why the deterministic
   `mkfs` recipe and the restored build-box model matter operationally.
3. **The dongle stays OUT.** `test_harness.rs:2263` latches `debug_status() != NoDevice` → `fail(...)`
   **before** the rung-1 and rung-4 emissions, so a plugged-in AX88179 aborts the run with no rung
   tokens at all.

**The real brain comes from the DISK, not the build env.** `NEO_STAGE_B_NEOM` governs *QEMU* staging
(`prepare_product_integration_brain_model` falls back to a synthetic `valid_embedding.neom` whose vocab
maps token 3 to the literal `"Arthur is ready."` — so **the QEMU green never proved real inference**).
On hardware the model is read from `sda`, already verified byte-exact at `a32e4a1b…`. Keep
`neofs-mkfs hash --expect` as the check on anything staged; it is not the gate for this path. **Note the
build output cannot tell you which path was taken** — `prepare_stage_b_real_brain_model` returns the env
path silently and the fixture path has no marker, so the positive control must be on the artifact
(size 485,489,152 + hash), never on the log.

**Rung 4 on hardware is out of scope here** — the scenario asserts the no-dongle degrade *by equality*,
so it can only ever prove absence. Positive eth0 (link-up + lease, dongle in) is a separate run and
belongs with item 11.


The acceptance run. **Blocked on 1 and 2.** JOINT PL+coder per `docs/runbook-baremetal-arthur-optiplex.md`; the PL reads legibility live, the serial log is the durable record.

### 4. OT-70 — `as_bytes_mut` unsoundness  ✅ **NARROWED 2026-10-16** (aliasing-`&mut` handout removed; a smaller data-race residual stays OPEN — see `docs/OPEN-THREADS.md` OT-70)
`as_bytes_mut(&self) -> &mut [u8]` hands out aliasing `&mut` from a shared borrow — **undefined behaviour, not style**. Cross-cutting: `ai/tensor.rs` plus all three accel drivers. **Needs a THINK before any build** (candidate for Sage: design the migration, hand the mechanical edit to a fast seat).

### 5. OT-59 — the `arthur_reversibility` step-index logging  ✅ **DONE 2026-10-16** — and it immediately named the stall; both `arthur_reversibility` hangs are now root-caused + fixed (OT-74 row)
Cheapest high-value instrumentation on the board. Log the step index before each serial wait; the run then names *which* `ARTHUR_PROMPT` never arrived, discriminating three mechanisms in one run. Attacks a **37.5%** attempt-1 failure that has been taxing every lane behind a retry wrapper. Instrumentation only — no fix, no budget change (`xtask` explicitly forbids the latter).

### 6. OT-69 — pay the debt down, then flip the job blocking  ✅ **DONE 2026-10-16** (12 findings fixed/banked; `feature-gated-coverage` is blocking)
27 `mt7601u` clippy errors + the `accel-intel` breakage, then make `feature-gated-coverage` blocking. **That flip is OT-69's actual close condition** and it is now decidable for the first time, since the job finally reports via annotations.

### 7. OT-74 — `threads_futex`'s lost wakeup  ⬅ `threads_futex` did not reproduce locally (0/10 TCG); the `arthur_reversibility` half is fixed (see OT-74 row)
Sage closed the reservation-leak theory: `threads_futex` never calls `wake_thread_front`. It is a **lost-wakeup family defect**; chase it with the existing `wake_trace` PARK/WAKE_PICK/WOKEN discriminators. Highest attempt-1 failure rate of all 284 scenarios (**2/3**).

### 8. OT-72 — BAR over-mapping
`xhci` maps **4×** its BAR0, `ahci` **2×** its ABAR — measured, not theorised. Real MMIO the devices do not decode. Own build, own gate; deliberately *not* ridden on another lane (doing that is what caused the `1255b665` regression).

### 9. Consolidation class — OT-75, OT-76, OT-79
Capability-assembly split-brain (make two config surfaces one surface, via the existing `neo-test-atrium` umbrella-marker pattern); the `boot_frame_eth0.rs` 127-line fork; the positional inherited-handle ABI. All three are legibility/fragility debt — fold into one consolidation build per the cadence rule.

### 10. NeoFS `mkdir`/`unlink`
`create_regular` needs a pre-existing directory. **Gates a real editor ⇒ prerequisite of OT-67.**

### 11. Acceptance runs — WiFi LINK-UP, M710q EU byte-equality
Merged code, unproven hardware. M710q needs **VT-d enabled in firmware** *and* the **error-kind latch** (Jarvis's proof boundary) before its boot, or the run is ambiguous on failure. Hardware-window dependent, PL-joint.

### 12. The browser C-ladder — 1.0 critical path
C1.2 → C2 → C3 → SWGL → modern-site battery. **Unchanged as the 1.0 gate.** Resumes once 0.1 is demonstrated; 0.1 does not sit on this path.

---

## Standing rules for every dispatch in this queue
- **One seat at a time.** Item N+1 is not relayed until item N merges.
- **The RETURN is a pushed artifact**, and the dispatch names its path (`docs/OPEN-THREADS.md` tree-state rule).
- **Handoff close-out** if a lane changes seats: the outgoing tree is audited and cleared first.
- **Worktree named in every relay**, sibling under `E:\AI Automation\workers\<coder>` — never a nested `Workers/`.
- **Disk reclaim after merge**, `cargo clean` not delete.
