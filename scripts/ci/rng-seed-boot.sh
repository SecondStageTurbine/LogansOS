#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# The production CSPRNG seed path, booted (escaped-defect contract item 4,
# #43). Every `neo-test` scenario kernel seeds the DRBG with a fixed seed
# (`kernel/src/rng/mod.rs`), so no scenario reaches `Csprng::init` and its
# RDRAND seed. An inverted RDRAND carry check there made every non-test
# kernel with `kernel-rng` (the product image) panic at boot, and the gate
# stayed green. This boots a kernel built WITHOUT `neo-test`, WITH
# `kernel-rng`, under QEMU `-cpu max` (TCG's RDRAND works, CF=1), and passes
# only when the serial log shows the DRBG seeded and no panic.
#
# Hermetic: no model, no network. Runs in the dev container (QEMU, Limine).
set -euo pipefail

out=target/rng-seed-boot
mkdir -p "$out"
log="$out/serial.log"
rm -f "$log"

cargo xtask iso --features kernel-rng

qemu-system-x86_64 \
    -machine q35 -cpu max -m 1024M -smp 2 \
    -display none -monitor none -no-reboot \
    -serial "file:$log" \
    -boot d -cdrom boot/neoplutarch.iso &
qemu=$!
trap 'kill $qemu 2>/dev/null || true' EXIT

ready='[ kernel ] rng: CSPRNG initialized'
verdict=FAIL
reason=timeout
for _ in $(seq 1 120); do
    sleep 5
    if grep -q -e 'KERNEL PANIC' -e 'panicked at' "$log" 2>/dev/null; then
        reason='kernel panic'
        break
    fi
    if grep -qF "$ready" "$log" 2>/dev/null; then
        verdict=PASS
        reason='DRBG seeded from RDRAND'
        break
    fi
    if ! kill -0 "$qemu" 2>/dev/null; then
        reason='QEMU exited before the DRBG was seeded'
        break
    fi
done

echo "---- serial ----"
cat "$log" 2>/dev/null || true
echo "----------------"
echo "RNG_SEED_BOOT verdict=$verdict reason=$reason"
[ "$verdict" = PASS ]
