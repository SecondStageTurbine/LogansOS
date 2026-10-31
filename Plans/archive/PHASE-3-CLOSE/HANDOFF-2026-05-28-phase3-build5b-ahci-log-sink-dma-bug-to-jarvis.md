# HANDOFF — Phase 3 BUILD-5b NVMe-lane DMA bug → Jarvis

**Date:** 2026-05-28
**From:** PM
**To:** Jarvis
**Re-open of:** Phase 3 BUILD-5b (AHCI lane already GREEN, NVMe lane PARTIAL — one bug separating us from full BUILD-5b close)
**Blocker:** [BLOCKER-2026-05-28-phase3-build5b-ahci-log-sink-dma-wrong-lba-and-rodata-leak.md](BLOCKER-2026-05-28-phase3-build5b-ahci-log-sink-dma-wrong-lba-and-rodata-leak.md)

## Context Jarvis needs to internalize before touching code

1. **Your prior fix `72a0a41` is good and accepted.** AHCI lane completed bare-metal with 9202 bytes clean extraction. Don't second-guess that work. See [PM-DISPOSITION-2026-05-28-phase3-build5b-apic-fix-accepted.md](PM-DISPOSITION-2026-05-28-phase3-build5b-apic-fix-accepted.md).
2. **The NVMe-lane scenario itself PASSED on bare metal** — kernel-side `NEO_TEST/1 event=PASS name=phase3_nvme_neofs_mount_and_readwrite ... create_write_fsync_readback=ok bytes=30`. NeoFS read/write/fsync/readback on NVMe is proven; that's not what you're fixing.
3. **What you ARE fixing:** the AHCI driver's *log-sink write submission*. It puts data at the wrong LBA AND DMAs hundreds of bytes of `.rodata` instead of a 10-byte magic + header. The other three storage paths (AHCI-as-scenario, NVMe-as-scenario, NVMe-as-log-sink) work correctly.
4. **Read the BLOCKER end-to-end** before grep'ing code. The forensic evidence narrows the bug to a very specific surface (AHCI write submission for the log-sink target) and the symptom pattern (wrong LBA + over-DMA of source). Don't waste time on the IRQ path, the FS layer, or the NVMe driver — none of those are implicated.

## Files most likely involved (PM hypothesis — confirm via grep, don't trust blindly)

- `kernel/src/drivers/ahci/` — AHCI driver code, especially any `write_block` / `write_blocks` / `submit_write` entry points
- `kernel/src/drivers/block/` or `kernel/src/log/sink.rs` (or wherever the bare-metal log-sink lives) — the caller side that hands a buffer + LBA to the AHCI driver
- The shared block-device write interface — both AHCI and NVMe driver back-ends should conform to the same trait; if the trait is by-byte-offset and AHCI's impl is by-sector-LBA (or vice versa), that's the LBA bug
- The DMA-region setup for AHCI command FIS — if the source physical address is a raw pointer rather than the heap-allocated log entry buffer, that explains the `.rodata` leak

Search hints:
```
rg -n "bare_metal_log_sink|log_sink_install|init_block_log_sink" kernel/src
rg -n "fn write_block|fn write_blocks|fn submit_write|fn issue_write" kernel/src/drivers/ahci kernel/src/drivers/block
rg -n "NEO_LOG_v1" kernel/src
```

## Two co-occurring bugs (both must be fixed)

### Bug 1 — Destination LBA arithmetic

Kernel logs `offset=0x10000000`; data lands at byte offset `0x434A51` on disk. Numerical relationship: not a clean integer factor (≈ 60.87×). Likely candidates:
- Bytes-vs-sectors unit confusion somewhere in the LBA computation
- An off-by-some-base mistake (a static buffer offset being added or subtracted incorrectly)
- A truncation (e.g., 28-bit LBA mode silently clamping a 48-bit LBA)
- AHCI write command using PRDT entry with wrong size and the FIS LBA being filled from a stale/uninitialized variable

The NVMe driver's log-sink write path works at the same `0x10000000` offset (see AHCI-lane evidence). Compare the two driver implementations side-by-side; the AHCI version diverged.

### Bug 2 — DMA source pointer + length

The data on disk after the magic contains verbatim kernel `.rodata` strings. Excerpt (decoded ASCII):
```
NEO_LOG_v1\0kernel/src/arch/x86_64/syscall.rs\0#tlb_shootdown_slot_busy target_cpu=\0[ kernel ] paging: GLOBAL fixup complete - 4K leaves: \0...
```

Hypothesis: the write submission is passing the *address of the `NEO_LOG_v1` magic constant in `.rodata`* as the DMA source, with a length much larger than 10 bytes. The DMA then walks into surrounding `.rodata` strings.

Fix: the source must be a properly-sized log entry buffer (built on the heap or in a per-CPU log-staging area), and the length must equal the actual log entry size — NOT a fixed "page" or "block" size that overshoots the header.

## Security must-fix

Even after relocating the writes to the right LBA, the DMA length bug means **kernel memory still gets written to disk**. The fix is incomplete unless the source/length is bounded to the intended log entry. Anything else leaks kernel `.rodata` (and potentially `.text` / stack / heap depending on where the source pointer points) to a disk region that any subsequent OS can read.

If you split the LBA fix and the source/length fix into separate commits, that's fine — but BOTH must land before PM accepts. Call out the split explicitly in the second commit's message.

## Return contract (what to put in your completion report)

Match the format you used for `72a0a41` so PM verification is uniform:

```
Root cause: <one-paragraph explanation, citing the exact source file(s) and lines>

Commit(s):
<full-sha> <subject line>
[<full-sha> <subject line>  — if you split LBA fix from length fix]

Validation:
- cargo k: PASS
- neo-extract-log tests: PASS
- QEMU AHCI: PASS
- QEMU NVMe: PASS (with valid extraction at default offset from Samsung-equivalent log sink)
- AHCI bare-metal build-only: PASS
- NVMe bare-metal build-only: PASS
- neo-extract-log release build: PASS

Restaged SHA256:
<7 lines of sha256  path entries — rerun all 7 staging hashes>

PL rerun instruction:
<concrete steps; AHCI lane already passed so NVMe-only rerun is sufficient unless you changed AHCI-side code that could regress AHCI lane>
```

## Open question for you to answer in your report

**Why didn't QEMU NVMe lane catch this?** The pre-commit validation matrix included QEMU NVMe PASS, but the bug landed on bare metal in this exact scenario. Two possibilities:

1. QEMU's emulated AHCI/SATA controller hides the destination-LBA bug (e.g., QEMU silently translates byte-offset writes correctly where real AHCI hardware doesn't)
2. The bare-metal log-sink path wasn't actually exercised in QEMU — only the NeoFS scenario writes were

Whichever it is, the QEMU test matrix needs the gap closed so the next iteration of this bug class gets caught before bare metal. Name the gap in your report; we'll file a follow-up to harden the QEMU lane.

## Do NOT

- Push to `origin/main` if you haven't first verified locally that QEMU NVMe lane reproduces the failure on the OLD kernel (commit 72a0a41 without your patch). If QEMU doesn't reproduce, that confirms gap #1 above and you need an additional fix to the QEMU test before the BLOCKER can close.
- Touch any code outside the AHCI driver + log-sink + minimal interface changes. Specifically: do not refactor the block-device trait, do not "improve" the NVMe driver, do not adjust the APIC ordering you already fixed.
- Skip the `.rodata`-leak security fix. Half-fixing is worse than not fixing — it would close the BLOCKER from the extractor's perspective while the disk still leaks kernel memory.
- Commit on detached HEAD. `git checkout main` first.

## Out-of-scope (for clarity)

- The original APIC panic — already fixed in `72a0a41`, AHCI-lane proven
- Any 5456d23-style unrelated commits — they stay local, do not push
- ai_digest.py and Phase 2 evidence — leave untouched

## Acceptance gates PM will check on your return

1. Both commits present on `origin/main`, on branch `main` (not detached)
2. Commit message body names this BLOCKER explicitly
3. Code diff matches stated root cause — PM will inspect every changed file
4. All 7 staging hashes re-verified independently via `Get-FileHash`
5. QEMU + bare-metal build-only validation log present in report
6. The security `.rodata`-leak fix is included or explicitly deferred-with-rationale
7. PL rerun instruction is concrete and minimal (NVMe lane only unless AHCI lane code changed)

Once accepted, PL reruns NVMe lane on real hardware. Same gate ledger as before: framebuffer PASS, extracted PASS marker, Captured bytes > 0, no Degradation block. After NVMe-lane bare-metal green, Phase 3 BUILD-5b formally closes.

Standing by for your report.
