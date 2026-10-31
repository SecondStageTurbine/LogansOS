<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: i8042 (PS/2) keyboard driver — cross-family review

- **To:** a Codex-family seat (Jarvis by default; the PM may reassign). The author, S3, is Claude.
- **MODEL:** GPT-6-Luna · **EFFORT:** high. It is a contained driver (one subsystem, no SMP core, no new
  syscall), but it adds an IRQ path and the first I/O APIC programming in the kernel.
- **Branch under review:** `claude/i8042-keyboard`, at the head named in its PR.
- **Your RETURN:** `Plans/RETURN-2026-10-22-i8042-review-<seat>.md` on branch `codex/i8042-review-<seat>`,
  cut from `origin/main` and containing only that file. Push it; a review that exists only in chat does not
  count.
- **Verdict:** SOUND, SOUND-WITH-NITS, or BOUNCE. Each item is PASS or FAIL with the enforcing `file:line`. A
  FAIL names a concrete input or sequence. An uncertain item is a note, not a FAIL.

## Read first

```
git fetch origin
git show origin/claude/i8042-keyboard:Plans/DISPATCH-2026-10-22-i8042-review.md
git show origin/claude/i8042-keyboard:docs/RESEARCH-2026-10-22-i8042-FINDINGS.md
git diff origin/main...origin/claude/i8042-keyboard --stat
```

Core files:
- `kernel/src/drivers/ps2/i8042.rs` (init, ISR, ring, pump);
- `kernel/src/drivers/ps2/i8042_logic.rs` (decoder, config byte);
- `kernel/src/arch/x86_64/ioapic.rs` and `ioapic_logic.rs`;
- `kernel/src/acpi/madt_logic.rs` and the two readers in `kernel/src/acpi/mod.rs`;
- the vector and stub in `kernel/src/arch/x86_64/idt.rs`;
- the pump call sites in `kernel/src/arch/x86_64/syscall.rs`.

## Checklist

1. [ ] **ISR takes no lock.** `handle_interrupt` → `try_drain_port` → `drain_port` takes no mutex and never
   spins on `PORT_OWNER` (a failed compare-exchange returns). Nothing it calls takes an AI-governance lock
   (`INFLIGHT`, `RECENCY`, `ARENA`, the reservation, `TABLE`) or `DELIVERY`/`STATE`.
2. [ ] **Ring correctness.**
   - There is one producer at a time: `HEAD` is written only under `PORT_OWNER`.
   - There is one consumer at a time: `TAIL` is written only under `STATE`.
   - A byte is stored before `HEAD` is published (Release), and read after `HEAD` is loaded (Acquire).
   - The full check uses the consumer's `TAIL`, and no byte can be overwritten before it is read.
   - The wrap arithmetic is correct when the u32 indices overflow.
3. [ ] **Edge-triggered IRQ1 (CP-72).**
   - The ISR leaves the output buffer empty, unless the 256-byte cap is hit (counted).
   - A byte present when IRQ1 is enabled is taken (the drain after enabling, or the polled fallback).
   - There is no path where a byte stays in the buffer with no ISR or pump ever reading it.
4. [ ] **Init is bounded and safe on absent or odd hardware.**
   - Every wait is bounded.
   - `0xFF` status returns before any write.
   - A self-test or port-test timeout continues; a wrong value aborts.
   - The keyboard is never reset.
   - The AUX port is only disabled, with its IRQ bit cleared.
   - Nothing here can hang the boot or write a port beyond 0x60/0x64.
5. [ ] **Config byte.** `config_quiesced`/`config_running` keep bit 2 (system flag) and the reserved bits.
   They keep port 2 off, turn on only IRQ1, and keep translation on (`i8042_logic.rs`). The KAT expectations
   (`tools/xtask/src/i8042_tests.rs`) are right for a firmware default of `0x47`.
6. [ ] **Set-1 decode.**
   - The `SET1` table and the `E0` table match the IBM PC/AT set-1 scancodes and the HID page-0x07 usages.
     Check them against the PC/AT reference and the HID Usage Tables, not against the code.
   - Fake shifts are dropped.
   - Pause gives exactly one press and one release.
   - A typematic make for a held key gives no event.
   - Filtering `0x00/0xEE/0xFA/0xFC/0xFD/0xFE/0xFF` never drops a real key.
   - Not filtering `0xAA` (left Shift break) is correct.
7. [ ] **Events match the USB keyboard's.** For the same keystrokes, the PS/2 pump emits the same
   `InputEventV1`s as `hid_keyboard`'s `BootKeyEventDecoder` path:
   - the same usage;
   - modifiers equal to the boot-report modifier byte at that moment;
   - modifier keys giving no events of their own;
   - the same SAK filter, stage and flush.
   Check `decode_ring` and `pump_routed_events` against `hid_keyboard.rs` `Inner::drain` and
   `hid_logic.rs` `BootKeyEventDecoder`.
8. [ ] **MADT parsing.**
   - Type-1 and type-2 offsets match ACPI 6.5 §5.2.12.
   - A bus other than 0 is ignored.
   - Polarity/trigger decoding treats `00` as the ISA default (edge, high), `11` as low/level, and the
     reserved `10` as the default.
   - Truncated or zero-length entries cannot read out of bounds.
   - The record limits (4 I/O APICs, 16 overrides) cannot overflow.
9. [ ] **I/O APIC programming.**
   - The register access order is: masked low dword, then high, then the unmasked low.
   - The entry is fixed delivery, physical destination, to the BSP's APIC ID.
   - An ID above 255 is refused.
   - `ioapic_for_gsi` picks the right controller and pin.
   - No other pin is touched.
   - The 8259's IRQ1 stays masked, so the interrupt is not delivered twice.
   - The MMIO page is mapped before use.
10. [ ] **Vector and EOI.** Vector `0xF5` collides with no other vector (`idt.rs`; the MSI vectors are
    0xF0–0xF4, the IPIs 0xFC/0xFD, spurious 0xFF). The body sends a LAPIC EOI on every path. The stub is the
    shared `swapgs_irq_entry`, which covers ring-3 entry (GS) and the CP-39/40/41 flags.
11. [ ] **Kill-switch.** With `ps2-keyboard` off, none of the driver, the I/O APIC code, the MADT readers, the
    IDT entry or the asm stub is compiled. The default and `product` builds are unchanged. The feature is on
    in `desktop-session` (hence `desktop` and image A).
12. [ ] **Coverage.** The `ps2_keyboard` scenario covers what FINDINGS §7 says:
    - 14 exact events;
    - every byte by IRQ;
    - the control fails on the decode.
    Is anything in FINDINGS §6 (the Zenbook failure bank) mis-stated, or is a likely Zenbook failure missing
    from it?

## Relay

```
WORKTREE: E:\AI Automation\workers\<seat>     (yours alone; never another coder's folder, never the repo root)
BRANCH:   codex/i8042-review-<seat>           (git fetch origin && git checkout -B codex/i8042-review-<seat> origin/main)
MODEL:    GPT-6-Luna      EFFORT: high

Read: git fetch origin && git show origin/claude/i8042-keyboard:Plans/DISPATCH-2026-10-22-i8042-review.md
```

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?"
questions; it is faster and more semantic than grep, though grep still wins for exact symbols and strings.
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` should return
`{"status":"healthy",...}`. If it is down or degraded, the PL manages the RAG infrastructure: note it in your
RETURN and continue with direct file reads (it is never a blocker). Optional self-serve start (PowerShell):

```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```

An example query:

```
$body=@{query='i8042 keyboard interrupt and focus stage'; top_k=5}|ConvertTo-Json
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"
```

**Disk reclaim (after the PM confirms the merge):** your RETURN branch holds one file. Once the PM has banked
it, `cargo clean` in your worktree if you built anything; never delete a tree whose branch is unpushed.
