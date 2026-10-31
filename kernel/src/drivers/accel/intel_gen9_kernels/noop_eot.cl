// SPDX-License-Identifier: CC-BY-4.0
// Gen9.5 EOT-only no-op thread: the finalizer lowers an empty kernel body to a
// minimal prologue + the EU End-Of-Thread send. Used as the iter5/iter6 (A)/(B)
// dispatch bisector — it touches no memory, so a retire proves dispatch, a hang
// proves dispatch setup. Compiled by the pinned ocloc (see ../README.md).
__kernel void noop(void) {}
