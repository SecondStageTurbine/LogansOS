#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# The product kernel, booted with the preview's WHPX CPUID (#52 review,
# item 3; #57 re-check). Under WHPX, CPUID.(0DH,0):EBX said 576 with AVX
# enabled in XCR0; a kernel that trusts EBX overruns every XSAVE area and
# takes #GP(0) on the first LAPIC tick's iretq. The `xsave_ebx_understated`
# scenario proves the fix on a `neo-test` kernel; this boots the kernel the
# image ships (`iso --product-kernel`: its features and opt-level, no model)
# under QEMU `-cpu max` with two CPUs.
#
# Two ways to make EBX read 576:
#   cpu-witness / cpu-control: the EMULATOR answers it. QEMU runs with its
#     gdbstub and scripts/ci/cpuid-0d-model.py sets EBX to 576 after every
#     CPUID.(0DH,0) on every vCPU, as WHPX did. The kernel carries no CPUID
#     hook (`--no-kaslr`, so the ELF's `cpuid` addresses are the run-time
#     ones). The model's log must show the rewrite on both vCPUs.
#   witness / control: the kernel's own `test-xsave-ebx-576` hook.
#
# The witnesses PASS when the boot line reports the understated EBX, the AP
# comes online under the boot CPU's policy, and the scheduler runs on LAPIC
# ticks with no panic. The controls add `test-xsave-ebx-trusted` (the area
# sized from EBX alone, as before the fix) and PASS when the boot ends in the
# product panic screen with the #GP diagnostics (#52 review, item 4).
#
# Hermetic: no model, no network. Runs in the dev container (QEMU, Limine,
# python3, objdump).
set -euo pipefail

mode=${1:-witness}
cpu_model=no
case "$mode" in
    witness) features=test-xsave-ebx-576 ;;
    control) features=test-xsave-ebx-576,test-xsave-ebx-trusted ;;
    cpu-witness) features= cpu_model=yes ;;
    cpu-control) features=test-xsave-ebx-trusted cpu_model=yes ;;
    *)
        echo "usage: $0 [witness|control|cpu-witness|cpu-control]" >&2
        exit 2
        ;;
esac

out=target/product-xsave-boot
mkdir -p "$out"
log="$out/$mode.serial.log"
model_log="$out/$mode.cpuid-model.log"
rm -f "$log" "$model_log"

iso_args=(--product-kernel)
[ -n "$features" ] && iso_args+=(--features "$features")
gdb_args=()
if [ "$cpu_model" = yes ]; then
    iso_args+=(--no-kaslr)
    gdb_args=(-S -gdb tcp:127.0.0.1:12345)
fi
cargo xtask iso "${iso_args[@]}"

if [ "$cpu_model" = yes ]; then
    objdump -d --no-show-raw-insn target/x86_64-neoplutarch/debug/kernel |
        awk '$2 == "cpuid" { sub(":", "", $1); print $1 }' >"$out/cpuid-sites.txt"
fi

qemu-system-x86_64 \
    -machine q35 -cpu max -m 2048M -smp 2 \
    -display none -monitor none -no-reboot \
    -serial "file:$log" \
    "${gdb_args[@]}" \
    -boot d -cdrom boot/neoplutarch.iso &
qemu=$!
model=
if [ "$cpu_model" = yes ]; then
    python3 -I scripts/ci/cpuid-0d-model.py 12345 "$out/cpuid-sites.txt" "$model_log" &
    model=$!
fi
trap 'kill $qemu 2>/dev/null || true; [ -n "$model" ] && kill $model 2>/dev/null || true' EXIT

has() { grep -qF -- "$1" "$log" 2>/dev/null; }
# The CPUID model rewrote EBX on both vCPUs (the condition came from the
# emulator, not from a kernel hook).
model_applied() {
    [ "$cpu_model" = no ] && return 0
    [ "$(grep -F -- '-> 576' "$model_log" 2>/dev/null | grep -v '576 -> 576' |
        awk '{ print $4 }' | sort -u | wc -l)" -ge 2 ]
}

verdict=FAIL
reason=timeout
for _ in $(seq 1 60); do
    sleep 5
    if [ "${mode#cpu-}" = witness ]; then
        if has 'KERNEL PANIC' || has 'panicked at'; then
            reason='kernel panic'
            break
        fi
        if has 'cpuid_0d_ebx=576 (understated; sized from the components)' &&
            has '[ kernel ] smp: AP cpu_id=1 ready' &&
            has '[ kernel ] sched: preemption ready (cpu=0, LAPIC timer' &&
            has '[ kernel ] sched: smoke test passed' && model_applied; then
            verdict=PASS
            reason='EBX=576 understated; AP admitted; LAPIC ticks and context switches ran'
            break
        fi
    else
        if has '!!  KERNEL PANIC  !!' &&
            has 'fatal CPU exception in kernel mode: GENERAL_PROTECTION_FAULT (#GP) err=0x0' &&
            has 'link_rip=' && model_applied; then
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

if [ "$cpu_model" = yes ]; then
    echo "---- CPUID model ($mode) ----"
    cat "$model_log" 2>/dev/null || true
fi
echo "---- serial ($mode) ----"
cat "$log" 2>/dev/null || true
echo "----------------"
echo "PRODUCT_XSAVE_BOOT mode=$mode verdict=$verdict reason=$reason"
[ "$verdict" = PASS ]
