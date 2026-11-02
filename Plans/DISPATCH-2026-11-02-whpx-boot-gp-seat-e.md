<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Dispatch: the preview kernel #GPs at boot under WHPX acceleration (Seat E)

**Reported by J.c. (2026-11-02), running the `preview` release** (`LogansOS-preview-bef433fabbe7.zip`, built from
`claude/preview-vm-image` @ `bef433fa`, which carries the RDRAND fix) with `Start LogansOS.bat`, i.e. QEMU for
Windows (MSYS2 ucrt64 build) with **WHPX** acceleration. The same package passed its CI boot check under **TCG**.
The screen (his photo, transcribed):

```
[ kernel ] smp: started 3 AP(s) (serialized bringup)
[ kernel ] smp: AP ready-barrier cleared (3/3 AP(s) up)
[ kernel ] acpi: clocks hpet=Some(fed00000) pm_timer=Some(PmTimer { port: 608, wide: false })
[ kernel ] timer: hpet at 0xfed00000 hz=100000000 counter=64-bit
[ kernel ] sched: BSP per-CPU scheduler ready (cpu=0, bootstrap='main', ready_queue empty)
[ kernel ] timer: tsc rate=4400238030 source=hpet checked=acpi-pm:4401672844 (within 5%)
[ kernel ] timer: lapic rate=1000163409 source=hpet checked=acpi-pm:1000165687 (within 5%)
[ trap ] GENERAL_PROTECTION_FAULT (#GP) err=0x0 (selector index)
[ trap ] #GP rip=0xffffffff9d79b01c rsp=0xffff800077b02bc8 cs=0x0008 ss=0x0010 rflags=0x0000000000010046
[ trap ] GENERAL_PROTECTION_FAULT (#GP) InterruptStackFrame { instruction_pointer: VirtAddr(0xffffffff9d79b01c),
         code_segment: SegmentSelector { index: 1, rpl: Ring0 }, cpu_flags: RFlags(RESUME_FLAG | ZERO_FLAG |
         PARITY_FLAG | 0x2), stack_pointer: VirtAddr(0xffff800077b02bc8), stack_segment: SegmentSelector { index: 2 } }
```
then nothing more on the console (the watchdog overlay keeps updating: `aps alive=3/3`, `sys: start tk now=1`,
`model=-`, `cpu gen=c4294967295 MHz=n/a | hwp=none perf-req was=0 set=0`). Earlier lines: x2APIC enabled on all
CPUs, `FPU save=xsave area=576 mask=0x7`, SMEP+SMAP true, `i8042 present`, `rtl8125 absent`, `accel` 2 devices.
The host CPU is a real x86 (TSC ~4.4 GHz).

## What to do

1. **Find the faulting instruction.** Build the product kernel at `bef433fa` exactly as the preview workflow does
   (`scripts/preview/build-in-container.sh`), find the KASLR slide (the kernel's load base is derivable from the
   build plus the printed addresses; or add a one-line `kernel base=` print if there's no way), and map
   `rip=...9d79b01c` to the function and instruction. Kernel-mode #GP with error 0 right after timer calibration is
   most likely an `rdmsr`/`wrmsr` of an MSR that Hyper-V/WHPX doesn't expose (perf/HWP, energy, platform-info,
   TSC-deadline, a debug MSR), or an `xsetbv`/`wrmsr` value the hypervisor rejects. Confirm; don't assume.
2. **Fix it the robust way:** gate the access on the CPUID feature bit that architecturally promises the MSR, and
   make any remaining best-effort MSR probe fault-tolerant (an exception-fixup `rdmsr_safe`/`wrmsr_safe`, using the
   same fixup-table mechanism as #13's user-copy). A missing optional feature degrades (logs and continues); it never
   halts the boot. Audit every other `rdmsr`/`wrmsr` in the kernel for the same pattern and list them in the PR.
3. **Make it testable without WHPX:** a QEMU scenario booting the product kernel on a CPU model without the
   feature (e.g. `-cpu qemu64` or `-cpu max,-<flag>`), so the gate covers "the MSR isn't there".
4. A fatal kernel #GP should also show a clear panic screen on the framebuffer, not just a frozen log; check why it
   didn't, and fix it if it's cheap.
5. Usual rules: close-gate, gate, PR, `PM:` comment. It's kernel and touches exception handling, so a Codex review
   follows. It's a priority: J.c. is waiting to try the preview with acceleration.
