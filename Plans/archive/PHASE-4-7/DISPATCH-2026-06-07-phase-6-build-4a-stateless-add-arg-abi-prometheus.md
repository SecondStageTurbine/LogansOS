<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 6 BUILD-4a — stateless A64 Add kernel + cross-thread arg ABI → Prometheus

**Date:** 2026-06-07
**From:** PM/Cerebro
**To:** Prometheus (amend the SAME branch — `prometheus/phase-6-build-4a-intel-finalizer`)
**Dispatch (authoritative, on main):** `Plans/DISPATCH-2026-06-07-phase-6-build-4a-stateless-add-arg-abi-prometheus.md`
**Status:** DISPATCHED — follow-on to your finalizer close (`2067efc`, Stage A ACCEPTED). Resolves your FLAG 3.
**Builds on:** `2067efc` (off Jarvis head `3c7a9d7`).

---

## 0. READ FIRST — fetch, then amend your existing branch

Your finalizer toolchain is accepted as-is; the no-op and the pinned ocloc
recipe are **final and untouched by this dispatch**. This is a focused change
to the **Add kernel only**. Continue on your existing branch:

```
git fetch origin
git checkout prometheus/phase-6-build-4a-intel-finalizer   # already at 2067efc
git show origin/main:Plans/DISPATCH-2026-06-07-phase-6-build-4a-stateless-add-arg-abi-prometheus.md
```

Re-squash into the single finalizer commit at close (§7) so the branch stays
one clean commit (toolchain + final no-op + **stateless** Add) for the
convergence merge.

## 1. Why — your FLAG 3, confirmed in the disasm

Your `add_f32x2.gen9.asm` shows the Add doing
`sends … untyped surface write … hdc.dc1` — i.e. **stateful BTI surface
access**. As you flagged, that real Add will not retire to `[4,6]` under the
minimal driver: stateful access needs a binding table + per-arg `SURFACE_STATE`
+ Surface State Base in `STATE_BASE_ADDRESS` — a large chunk of driver ABI in
Jarvis's command-encoder region that does not exist yet and is not in iter6.

**PM decision (engineering call, reversible):** for the silicon bring-up proof
we take the **stateless** path, not the stateful one. Rebuild the Add so it
accesses its buffers via **stateless A64** messages (raw GPU addresses passed
in the cross-thread payload), exactly the "stateless A64 / PPGTT / MOCS"
alternative the kernel README already names. This deletes the binding-table /
surface-state burden from the driver and is the shortest path to a retiring
`[4,6]`. The stateful BTI ABI remains the eventual production path (it's what
the DRM-expressible driver of ADR-023 will want) and is tracked as a **later
hardening build** — not this one. The no-op is unaffected (it touches no
memory).

## 2. Objective

`ADD_F32X2_KERNEL_GEN9` recompiled to a **stateless** Gen9 kernel whose
disassembly shows **A64 stateless** loads/stores (no `untyped surface` / BTI
`sends`), plus a checked-in, byte-exact **cross-thread argument ABI** table —
the contract Jarvis's driver populates so the kernel finds its three pointers.

## 3. Implementation Success Criteria

- **ISC-1 — Stateless compile.** In `tools/intel-gen9-finalizer/build-kernels.sh`
  (or a per-kernel options hook), compile `add_f32x2.cl` with stateless buffer
  access — `ocloc compile … -options "-cl-intel-greater-than-4GB-buffers-required"`
  (the documented switch that forces all `__global` access through stateless
  A64). Keep the no-op compile exactly as-is. Record the new exact command line
  in the README recipe.
- **ISC-2 — Disasm proves stateless.** The regenerated `add_f32x2.gen9.asm`
  must show A64 stateless data-port messages (e.g. `load`/`store` /
  `svm`/A64 block messages) and **must not** contain `untyped surface
  read/write` / BTI surface `sends`. Confirm in one human-readable line in the
  README, and that the terminating `{EOT}` send is still present.
- **ISC-3 — Cross-thread arg ABI table (the cross-lane contract).** Extract
  from the zebin metadata (`zeinfo` / `payload_arguments`) the **byte offsets,
  sizes, and meaning** of every kernel argument in the cross-thread/indirect
  (CURBE) payload — the three buffer pointers `a`, `b`, `o` and any implicit
  args ocloc inserts (global-id/global-offset/work-dim/etc.). Write this as an
  explicit table in `intel_gen9_kernels/README.md` ("Cross-thread argument
  ABI"), including the **total payload size** and required alignment. This is
  the spec Jarvis programs against; treat it as a byte-layout deliverable
  (CLAUDE.md): the zeinfo dump is its independent oracle — check the relevant
  `zeinfo` excerpt in alongside the table.
- **ISC-4 — Blob + tail oracle resync.** Replace the `ADD_F32X2_KERNEL_GEN9`
  bytes; update its length and `ADD_F32X2_KERNEL_EOT_TAIL_BYTES` /
  `add_kernel_has_pinned_eot_tail()` and the `tools/ai-ops-tests` Add-EOT
  assertion to the new stateless blob. No-op oracle unchanged. **Do not** touch
  Jarvis's command-stream oracle (`ADD_BATCH_*`) — §7.
- **ISC-5 — Determinism re-proven.** Two builds → identical `.krn` sha256 for
  the new stateless Add. Report both source→blob sha256 pairs (no-op pair
  unchanged — restate it).
- **ISC-6 — Standards + green.** SPDX intact; `cargo xtask` kernel build green
  (feature on ISO + off); host suite incl. the Add/no-op EOT tests +
  `shard-coverage 4` green; `fmt --check` + workspace `clippy -D warnings`
  clean. One-line ADR-032 D4 ledger note that the Add ships **stateless** for
  bring-up (stateful BTI deferred to a hardening build).

## 4. Scope guard (§7 still holds)

EU-kernel-blob region of `intel_gen9_4a.rs`, `intel_gen9_kernels/**`,
`tools/intel-gen9-finalizer/**`, and the EU-kernel oracle in
`tools/ai-ops-tests` **only**. Do **not** touch the command-encoder region
(`MEDIA_VFE_STATE`/`GPGPU_WALKER`/`ADD_BATCH_*`/`GGTT_*_BASE`/
`AddBatchLayout`/`STATE_BASE_ADDRESS`) — that's Jarvis. The cross-thread arg
ABI you document (ISC-3) is *consumed* by Jarvis but *authored* here as docs;
you don't write the driver code that fills it.

## 5. Close gate + report

- `git fetch origin && git rebase origin/codex/phase-6-build-4a-intel-gpu-bringup-jarvis`;
  re-run Stage-A gate after rebase. Single squashed commit; push to
  `prometheus/phase-6-build-4a-intel-finalizer` (**not main**).
- Report: the stateless ocloc command line · the new Add disasm one-line
  reading (A64, no BTI) + EOT confirmation · the **cross-thread arg ABI table**
  (offsets/sizes/total/alignment) · new Add source→blob sha256 ×2 (determinism)
  · oracle consts updated · §7 confirmation (encoder region untouched).

## 6. What this unblocks (for PM/Jarvis sequencing — not your task)

The no-op (already final) is the kernel that proves **dispatch** once Jarvis's
iter6 lands `STATE_BASE_ADDRESS`. Your stateless Add + this arg ABI table are
the inputs to **iter7** (Jarvis): populate the cross-thread payload with the
three A64 pointers per your table + General State Base, then the real Add
retires `[4,6]`. That's the BUILD-4a merge trigger. Nothing here reaches main
independently.
