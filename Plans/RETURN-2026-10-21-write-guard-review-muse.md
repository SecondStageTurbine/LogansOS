<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: PR #85 write-guard hardening review — Muse (Gwen)

> Banked by the PM from the PL's paste (2026-10-21). Muse took the cross-family seat because the Codex
> models refused this security review even on Daybreak Blue. The verbatim text follows.

**Date:** 2026-10-21 · **Reviewer:** Gwen (Muse Spark family) · **Reviewed head:** `8ce803101b22a0617ae03c7773a30c93b8a43348` · **Base:** `main` `d4244be5`

**Disposition: PASS on all eight checks.** The F1–F4 and R1-1..R1-6 fixes are in the code at this head, each with its
enforcing line (see the PL's paste in the PM session for the full per-check evidence).

1. GPT integrity — PASS (`write_guard_logic.rs:259` crc32; `validated_gpt` :434, :452, :464, :488; backup via `read_table` :293; opaque-gpt-invalid :357; LBA-1 unreadable counts as a GPT that was seen, :420).
2. No partition table — PASS (opaque-unreadable :301; `whole_disk_contents` :378, :409).
3. MBR sanity and hybrid MBR — PASS (opaque-mbr-invalid :360; `out.extend(mbr_parts)` :352).
4. Arming extent / NEOFS volume — PASS (`write_range_allowed` block/mod.rs:184; `judge` :534, :550).
5. Every bare-metal block write goes through GuardedDevice — PASS (block/mod.rs :272, :291; virtio_blk.rs :545, :553; block/mod.rs :239, :366).
6. Guard-off control cannot be built for bare-metal, product or fb-progress-console, in any spelling — PASS (block/mod.rs:401-415; xtask :4552, :4565).
7. `log-sink-header` writes only a regular file — PASS (xtask :4348, :4368).
8. Panic path reaches the screen — PASS (panic.rs :36, :42; serial.rs :49, :58, :59, :64, :111; fb/progress.rs :66, :131, :138, :173; fb/mod.rs :179, :195).

**Observations (not fails):**
- Item 4: agreeing secondary superblocks set the size when there is no valid primary. This is R1-6's prescribed fallback.
- Item 3: MBR entries aren't checked for overlapping each other. That's cosmetic, since the vetoes are unioned.
- Item 6: the image scan is a second layer; the kernel `compile_error!` is the load-bearing check.
- Item 8: when SERIAL is held, the raw UART copy can interleave with other output. The screen copy is written first.

**Not verified:** the KATs were read, not run, and CI status was not checked.

— Gwen (Muse Spark)
