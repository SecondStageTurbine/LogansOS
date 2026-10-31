<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 6 BUILD-3a — virtio-accel transport (probe + virtqueue + DMA + IRQ) → Jarvis

**Date:** 2026-06-05
**From:** PM/Cerebro
**To:** Jarvis (worker tree, feature branch — e.g. `codex/phase-6-build-3a-virtio-accel-transport-jarvis`)
**Status:** DISPATCHED — ready for BUILD
**Design contract:** **ADR-031** (the virtio-accel protocol + the §7 byte oracle — normative), ADR-030 (Phase 6 substrate), `Plans/THINK-2026-06-04-phase-6-build-2-virtio-accel.md` (why vhost-user, no QEMU fork).
**Builds on:** BUILD-2 (`accel_sim`). Current `main` tip: see origin/main.

---

## 0. Why this is sliced (read first)

BUILD-3 is the heaviest build in the phase — it introduces a real virtio device, DMA, IRQ-driven completion, a host daemon, and new CI plumbing. So we **slice transport-first**, single track:

- **BUILD-3a (this dispatch):** prove the *transport* end-to-end — probe, capability negotiation, the opq virtqueue, DMA buffers, and IRQ completion — using a **trivial `Identity` (echo) op**. No real op math. This isolates the riskiest new surface from the math.
- **BUILD-3b (follows):** the daemon runs the real op set (F32/I32, the BUILD-1/2 ops); register `virtio_accel` as a full `AccelDriver`; forward-pass equivalence (`virtio_accel == accel_cpu == oracle`) + cross-device migration. Reuses 3a's transport unchanged.

**Single track:** you own both the guest driver and the daemon, because they are joined at the §7 byte oracle — splitting them across workers would just add protocol round-trips.

## 1. Objective (3a)

A `.neom`-irrelevant transport proof: the kernel `virtio_accel` driver submits an `Identity` op over a real virtio-pci virtqueue to a host **vhost-user daemon**, which echoes the input to the output and completes via the used ring + IRQ — and the kernel reads back the exact bytes. Presented in CI via QEMU's generic `vhost-user-device` (pci), **no QEMU fork**. Everything is built to **ADR-031**.

## 2. Decisions locked (from ADR-031 — do not re-litigate)

- **Transport:** modern virtio-pci, **one `opq` virtqueue**, reusing the kernel's existing virtio-pci/virtqueue machinery (virtio-blk/net) and `DmaBuffer` (`drivers/block/dma.rs`).
- **Wire protocol:** the ADR-031 §D3 layouts (`VirtioAccelReqHeader`, `VirtioAccelTensorDesc`, `VirtioAccelStatus`, `VirtioAccelConfig`), little-endian, the descriptor chain readable-then-writable.
- **Async = IRQ-driven** (ADR-031 §D5): `dispatch` posts to avail + notifies, returns a token carrying the used-ring head; the opq interrupt handler marks the token complete. `wait` blocks until complete. **No tight unyielding spin** — park the thread on the token, or a *bounded* yielding wait (heed the TLB-shootdown TCG-spin flake; do not reintroduce that class of bug).
- **Device memory = `DmaBuffer`** (ADR-031 §D6).
- **`virtio-id`** is provisional `VIRTIO_ID_ACCEL` (candidate 63) — confirm non-colliding with the guest's other virtio drivers at BUILD.

## 3. Implementation Success Criteria

- **ISC-1 — Protocol types.** Implement the ADR-031 §D3 structs (LE, exact offsets) in the kernel driver, and mirror them in the daemon. Both sides MUST assert the §6 byte oracle independently (see ISC-6). (A shared `no_std` crate for the structs is acceptable and encouraged if it doesn't bloat the build; otherwise mirror + oracle-on-both-sides.)
- **ISC-2 — Guest driver probe + setup.** `kernel/src/drivers/accel/virtio_accel.rs`: PCI-probe `VIRTIO_ID_ACCEL` via the existing virtio-pci path (`for_each_virtio_vendor_cap`); read `VirtioAccelConfig` from config space; set up the `opq` virtqueue (avail/used rings) and its MSI-X/interrupt. **Feature-gated `virtio-accel`, default-OFF** (it requires the daemon — unlike `accel-sim`). Registers on `Device::Accel(2)` **only when the device is present**; device-absent must not panic and must not register.
- **ISC-3 — vhost-user daemon.** `tools/virtio-accel-host/` (new workspace member, Rust host binary). Implements the vhost-user backend (using the rust-vmm `vhost-user-backend` / `virtio-queue` / `vm-memory` crates is the sane path — justify the deps in its `Cargo.toml`; **do not hand-roll the vhost-user protocol**). Parses the chain per ADR-031, validates header/descs, and for 3a performs **`Identity` only**: copy the single input buffer to the output buffer, write `VirtioAccelStatus{ status: OK, output_len }`. Any op ≠ Identity → `status = UNSUPPORTED_OP`.
- **ISC-4 — Transport round-trip.** The driver's `dispatch(Identity)` builds the chain over `DmaBuffer`s, posts to avail, notifies; the daemon echoes; the used-ring IRQ marks the token complete; `wait` returns; **output bytes == input bytes**, end-to-end over real virtqueue/DMA/IRQ.
- **ISC-5 — IRQ-driven completion.** `poll` returns `Pending` before the used-ring entry lands and `Complete` after the IRQ; `wait` blocks correctly (park or bounded yield, per §2). `preemptible() == false`.
- **ISC-6 — Byte oracle (REQUIRED, both sides).** A hand-laid oracle for `Identity` of `[1.0, 2.0]` (f32, shape `[2]`) → `[1.0, 2.0]`: the full request/response bytes — `VirtioAccelReqHeader{op_kind=0,input_count=1}`, one input desc + one output desc (per ADR-031 §D3), input buffer `00 00 80 3F 00 00 00 40`, echoed output buffer (identical), `VirtioAccelStatus{0, _, 8}` — asserted **byte-for-byte independently in both the kernel driver test and the daemon test**. (This exercises the same framing as the ADR-031 §7 Add oracle; the Add *data* check lands in 3b.)
- **ISC-7 — CI plumbing.** `tools/xtask`: for virtio-accel scenarios, **launch the daemon + create the socket before QEMU**, pass `-chardev socket,... -device vhost-user-device-pci,virtio-id=$VIRTIO_ID_ACCEL,num_vqs=1,chardev=...`, and **reap the daemon after** (and on failure/timeout). New scenario `virtio_accel_identity_roundtrip` green at `NEO_SMP=4`.
- **ISC-8 — Errors / lifetime / robustness.** Daemon death or no-completion mid-scenario → driver **times out cleanly, no hang** (bounded wait, not infinite). `DmaBuffer`s freed; capability negotiation rejects a config that doesn't advertise `Identity` gracefully (probe fails, no register, no panic).
- **ISC-9 — Standards/gate.** Kernel: **no new external kernel dependency**; SAFETY comments on all new MMIO/DMA/virtqueue `unsafe`; SPDX on every new file. Daemon deps justified in its `Cargo.toml`. The new daemon crate joins the workspace and passes host-checks (fmt/clippy/test).

## 4. Close gate

- `git fetch origin && git rebase origin/main`; re-run the gate on the rebased tree.
- `virtio_accel_identity_roundtrip` green at `NEO_SMP=4` (daemon launched by xtask), and the **§6 byte oracle asserted on both sides**.
- **No regressions:** `accel_cpu_forward_migrate`, the three `accel_sim_*`, and the three `infer_*` scenarios still green; the default kernel build (which has `virtio-accel` OFF) unaffected.
- Host-checks green **including the new daemon crate** (`cargo fmt --all --check`, `cargo test --workspace --exclude neoplutarch-kernel`, `cargo clippy --workspace --exclude neoplutarch-kernel -- -D warnings`). **Run `cargo fmt --all --check` and confirm before close** (we ate a fmt break on main once already).
- Single squashed commit; push to your **feature branch, not `main`** — PM merges via `pm-merge-check` + `--ff-only`. Close report: ISC pass/fail, the oracle evidence (both sides), the chosen `virtio-id`, and any virtqueue-helper refactor you did (ADR-031 open-question #2).

## 5. Scope guard

Transport + `Identity` echo **only**. **No real op math** (3b), **no** multi-queue, **no** preemption, **no** f16/quantized dtypes, **no** inference-path wiring of `virtio_accel` beyond what the round-trip scenario needs. Do not touch `accel_cpu`/`accel_sim` behavior. If reusing virtio-net/blk virtqueue code tempts a large shared-module refactor, keep it minimal and note it for review rather than rewriting the transport layer in this build.

---

## Addendum 2026-06-05 — CI QEMU bump (PL decision); 3a finalization sequencing

3a's code is complete and oracle-verified, but the runtime scenario can't execute: the container's QEMU **7.2.22** (Debian bookworm default) lacks the generic **`vhost-user-device-pci`** (added in later QEMU; its *user-creatable* status flip-flopped across 8.x/9.x). **PL decision (2026-06-05): bump the CI/container QEMU.**

Jarvis finalizes 3a **on this branch as two separate commits** (separate so PM can assess the infra bump's blast radius distinctly from the feature):

1. **QEMU bump (own commit).** Move the dev/CI container to a QEMU with a *user-creatable* generic vhost-user device. **Target: `FROM debian:trixie-slim`** (QEMU 10.0.8; trixie = current Debian stable). **Verify empirically:** `qemu-system-x86_64 -device help | grep -i vhost-user-device` lists a user-creatable generic device and it instantiates. **Report the exact device name** (`vhost-user-device-pci` vs `vhost-user-test-device-pci`) — the xtask `-device` arg must match it. **Pin** the base image. **Re-run the FULL suite at `NEO_SMP=4`** — every existing scenario stays green (watch the SMP timing-sensitive ones — TLB-shootdown, sched, IPC; the existing fixes should survive a QEMU change, but confirm). Build + `--bare-metal-mode` unaffected. If the trixie base causes disproportionate breakage, fall back to building QEMU 10.x from source on bookworm and report.

2. **3a finalize (own commit).** Set the xtask `-device` arg to the confirmed device name; `virtio_accel_identity_roundtrip` green at `NEO_SMP=4`.

Close gate (§4) unchanged, plus: **full suite green on the NEW QEMU**, QEMU/base-image **pinned**, and the confirmed device name reported. PM reviews both commits and merges. Until this lands, BUILD-3b stays parked.
