#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# OT-61 C1.1 threads_futex hang repro loop (runs INSIDE the neo docker image).
#
# NEO_TEST_ATTEMPTS=1 is load-bearing: the built-in retry is exactly what erases
# a hang's evidence on CI, so a hang here must be a failure, not a silent retry.
# NEO_TEST_RUN_ID keeps each iteration's attempt log distinct.
#
# Two capture channels, because the kernel cannot always self-report:
#   - the in-guest wake-trace watchdog (fires on 5s of serial silence), and
#   - the QEMU monitor dump on timeout (NEO_QEMU_TIMEOUT_DUMP + MONITOR_PORT),
#     which still works when the BSP is wedged with interrupts off and therefore
#     takes no timer tick at all -- the one case the in-guest dump cannot cover.
set -uo pipefail

ITERS="${ITERS:-3}"
SMP="${SMP:-4}"
OUT=target/neo-test
mkdir -p "$OUT"

pass=0
fail=0
for i in $(seq 1 "$ITERS"); do
  echo "=========== iteration $i/$ITERS (NEO_SMP=$SMP) ==========="
  start=$(date +%s)
  if NEO_SMP="$SMP" NEO_TEST_ATTEMPTS=1 NEO_TEST_RUN_ID="loop$i" \
       NEO_QEMU_TIMEOUT_DUMP=1 NEO_QEMU_MONITOR_PORT=$((45000 + i)) \
       cargo neo-test threads_futex > "$OUT/loop$i.xtask.log" 2>&1; then
    pass=$((pass+1)); verdict=PASS
  else
    fail=$((fail+1)); verdict=FAIL
  fi
  echo "iteration $i: $verdict ($(( $(date +%s) - start ))s)"
  for f in "$OUT"/threads_futex*loop$i*.serial.log; do
    [ -f "$f" ] || continue
    n=$(grep -c 'wake-trace' "$f" 2>/dev/null || echo 0)
    echo "  $(basename "$f"): wake-trace lines=$n, last progress: $(grep 'threads-futex' "$f" | tail -1)"
  done
done
echo "=========== SUMMARY: pass=$pass fail=$fail of $ITERS ==========="
