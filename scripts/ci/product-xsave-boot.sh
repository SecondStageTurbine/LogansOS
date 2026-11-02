#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# The product kernel, booted with the preview's WHPX CPUID (#52 review,
# item 3). Under WHPX, CPUID.(0DH,0):EBX said 576 with AVX enabled in XCR0;
# a kernel that trusts EBX overruns every XSAVE area and takes #GP(0) on the
# first LAPIC tick's iretq. The `xsave_ebx_understated` scenario proves the
# fix on a `neo-test` kernel; this boots the kernel the image ships
# (`iso --product-kernel`: its features and opt-level, no model) with the
# `test-xsave-ebx-576` hook, under QEMU `-cpu max` with two CPUs.
#
#   witness (default): EBX=576, sized from the components. PASS when the
#     boot line reports the understated EBX, the AP comes online under the
#     boot CPU's policy, and the scheduler runs on LAPIC ticks with no panic.
#   control: `test-xsave-ebx-trusted` (EBX trusted, as before the fix).
#     PASS when it ends in the product panic screen with the #GP diagnostics
#     (#52 review, item 4: the fatal-trap path panics before it prints).
#
# Hermetic: no model, no network. Runs in the dev container (QEMU, Limine).
set -euo pipefail

mode=${1:-witness}
case "$mode" in
    witness) features=test-xsave-ebx-576 ;;
    control) features=test-xsave-ebx-trusted ;;
    *)
        echo "usage: $0 [witness|control]" >&2
        exit 2
        ;;
esac

out=target/product-xsave-boot
mkdir -p "$out"
log="$out/$mode.serial.log"
rm -f "$log"

cargo xtask iso --product-kernel --features "$features"

qemu-system-x86_64 \
    -machine q35 -cpu max -m 2048M -smp 2 \
    -display none -monitor none -no-reboot \
    -serial "file:$log" \
    -boot d -cdrom boot/neoplutarch.iso &
qemu=$!
trap 'kill $qemu 2>/dev/null || true' EXIT

has() { grep -qF -- "$1" "$log" 2>/dev/null; }

verdict=FAIL
reason=timeout
for _ in $(seq 1 60); do
    sleep 5
    if [ "$mode" = witness ]; then
        if has 'KERNEL PANIC' || has 'panicked at'; then
            reason='kernel panic'
            break
        fi
        if has 'cpuid_0d_ebx=576 (understated; sized from the components)' &&
            has '[ kernel ] smp: AP cpu_id=1 ready' &&
            has '[ kernel ] sched: preemption ready (cpu=0, LAPIC timer' &&
            has '[ kernel ] sched: smoke test passed'; then
            verdict=PASS
            reason='EBX=576 understated; AP admitted; LAPIC ticks and context switches ran'
            break
        fi
    else
        if has '!!  KERNEL PANIC  !!' &&
            has 'fatal CPU exception in kernel mode: GENERAL_PROTECTION_FAULT (#GP) err=0x0' &&
            has 'link_rip='; then
            verdict=PASS
            reason='EBX trusted: #GP(0), ended in the panic screen with its diagnostics'
            break
        fi
    fi
    if ! kill -0 "$qemu" 2>/dev/null; then
        reason='QEMU exited before a verdict'
        break
    fi
done

echo "---- serial ($mode) ----"
cat "$log" 2>/dev/null || true
echo "----------------"
echo "PRODUCT_XSAVE_BOOT mode=$mode verdict=$verdict reason=$reason"
[ "$verdict" = PASS ]
