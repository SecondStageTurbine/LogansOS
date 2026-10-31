<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE: desktop gate post-checks broken by serial interleaving (pre-existing; seen while testing x2APIC)

**Found:** 2026-10-21, while regression-running the `-cpu max` set on QEMU 10.2.3 (CI's QEMU) for the
x2APIC branch (`claude/x2apic-support`, CP-68). Under QEMU 10, `-cpu max` comes up in x2APIC mode after
CP-68.

## 1. What was seen

In one full pass of 32 `-cpu max` scenarios (all in x2APIC mode), two desktop scenarios passed **in the
guest** (`NEO_TEST/1 event=PASS`) but failed xtask's ordered serial post-check:

| Scenario | Post-check that missed | What the log shows |
|---|---|---|
| `desktop_idle_parks` | `DESKTOP launchcap advisory direct_spawn=allowed exit=1` | `DESKTOP launchcap advisory direct_spawn=allowed exit=[ kernel ] pmap: dropped — freed 138 frames + 1 PML…`: a kernel line landed in the middle of Arthur's line. |
| `desktop_resize_boot` | `DESKTOP arthur pointer buttons=0 phase=3 local=278,31` after byte 15126 | All the expected lines are present, but `local=278,169` arrived before `local=278,31`: the forwarded pointer events came in a different order than the script expects. |

Re-runs (same QEMU, same code):
- In x2APIC: `desktop_idle_parks` OK, `desktop_resize_boot` OK.
- With the `xapic-only` control: `desktop_idle_parks` OK.

The A/B is still running; its full table is in the PR.

## 2. `desktop_idle_parks`: the mechanism, and why it predates CP-68

`userland/neo-arthur/src/main.rs` (`stage_b_log_bytes` / `_u64` / `_newline`) writes this one line in
three `SYS_WRITE` calls: the prefix, the number, then `\n`. The kernel takes the serial lock per call, not
per line.

The interleaved kernel line comes from the reaper. It drops the just-exited worker's address space on
another CPU at the same moment Arthur's `sys_wait` returns and starts printing. Nothing orders the two, so
any change in cross-CPU timing can move the kernel line between Arthur's writes. Changing the APIC
register path (MSR in place of emulated MMIO) is one such change.

The code that splits the line is on `main` and untouched by CP-68. The race existed in xAPIC too; it was
just rarer under the old timing.

**Fix shape** (its own small lane):
- Have Arthur's `stage_b_log_*` helpers compose a whole line in a stack buffer and issue one `SYS_WRITE`.
  This covers every multi-part `DESKTOP …` line, not only this one.
- Alternatively, have the kernel's serial writer hold its lock until `\n` for a single `SYS_WRITE` buffer.
  That alone does not help a line split across calls.
- The xtask matcher should keep matching whole lines. Loosening it would hide real ordering bugs.

## 3. `desktop_resize_boot`: mechanism not established

The events are all present but reordered. The script injects USB-HID reports through QEMU's monitor, so a
plausible cause is report coalescing or ordering between the injected HID steps and the guest's polling,
which is timing-sensitive. This is **not proven**. It did not reproduce on re-run, and nothing in CP-68
touches the HID, pointer or WM paths.

**Falsification test:** run `desktop_resize_boot` N times in each mode on QEMU 10. If it fails only in
x2APIC, at a rate well above xAPIC's, it is an x2APIC timing interaction and belongs to CP-68. If it
fails in both, it is pre-existing.

## 4. Status

- `desktop_idle_parks`: pre-existing by construction (§2). The fix is a userland logging change, out of
  scope for CP-68.
- `desktop_resize_boot`: unknown mechanism, seen once in x2APIC. Being measured A/B.
- If CI's gate hits either on the x2APIC PR, the §2 fix is small and can be ported into it.
