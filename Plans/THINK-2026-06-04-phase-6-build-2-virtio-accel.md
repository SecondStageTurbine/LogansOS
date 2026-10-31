<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Phase 6 BUILD-2 — the CI accelerator ("virtio-gpu-compute") design pass

**Date:** 2026-06-04
**From:** PM/Cerebro
**Status:** THINK (design investigation — no code). Feeds a substrate ADR + PLAN once the slice is chosen.
**References:** ANSPEC-001 §9.2 (names "virtio-gpu-compute" as the Phase 6 first deliverable), ADR-030 (Phase 6 substrate, BUILD-2 = the multi-device CI driver), bible Articles 2.2/6.4/6.5.

---

## 0. Why this needs a THINK pass

ADR-030 / ANSPEC-001 §9.2 name **`virtio-gpu-compute`** as the CI-testable Phase 6 accelerator that "validates the multi-device path in CI under QEMU/KVM." Before dispatching a BUILD, I checked whether that device actually exists. **It does not** — so this pass establishes what we can really build, and recommends a slice.

## 1. Findings (grounded, not assumed)

### F1 — "virtio-gpu-compute" is not a real QEMU device
QEMU's `virtio-gpu` is **graphics only**: a 2D backend plus two accelerated 3D backends (`virglrenderer`/`gl` and `rutabaga_gfx`/`rutabaga`). There is no compute/tensor mode and no device called `virtio-gpu-compute`. The name in §9.2 is **aspirational** — it describes the *intent* (a paravirtual compute accelerator for CI), not a shipping device. **§9.2 must be amended.**

### F2 — The real mechanism is vhost-user, and a *custom* device needs no QEMU patch
QEMU ships a generic **`vhost-user-device`** (with `virtio-mmio` and `virtio-pci` variants) "intended for expert use while developing new backends." The user specifies the **`virtio-id`** and number of virtqueues (**`num_vqs`**), plus a **`--chardev`** socket to a host-side **vhost-user daemon** that implements the data plane. So we can present a *custom* virtio-accel device to the guest **without forking QEMU** — the "accelerator" is a daemon we ship; QEMU only relays virtqueue kicks/IRQs. (Flagged dev/non-production — fine for CI.)

### F3 — The kernel already has a mature virtio-pci/virtqueue stack
virtio-**blk** (Phase 3) and virtio-**net** (Phase 4) are in-tree and CI-exercised: modern virtio-pci probe (`for_each_virtio_vendor_cap`, vendor `0x1af4`), capability parsing, split virtqueues, IRQ handling. A virtio-accel guest driver **reuses this transport** rather than building it from scratch — the `vhost-user-device` pci variant appears as an ordinary virtio-pci device the existing probe path can discover by device-id.

### F4 — What BUILD-2 is actually *for* (two distinct de-risks)
`accel_cpu` (BUILD-1) deliberately stubbed the hard parts: it's the **default** device, its "device memory" **is** host RAM (identity copies), and `dispatch` is **synchronous/always-Complete** (`wait()` never sees `Pending`). So two separate things are still unproven:
- **(a) Logical multi-device + async:** a *non-default*, *non-CPU* `DeviceId::Accel(n)` with genuinely **separate device memory** (real, non-identity `copy_to/from_device`), real cross-device `tensor_migrate`, and **asynchronous** completion (`poll` returns `Pending` then `Complete`). This shakes out latent "the driver is always accel_cpu" assumptions in `ai::exec`, and resolves the two BUILD-1 deferred notes (`wait()`-on-`Pending` semantics; whether `&mut OpBatch` survives an async device).
- **(b) Hardware transport:** real **virtqueue / DMA buffers / IRQ-driven used-ring completion** — the plumbing a real GPU driver (BUILD-3/4) needs. If we never do a virtqueue accelerator, the real GPU is our *first* contact with accelerator DMA/IRQ **and** real hardware at once.

## 2. Options

| | Approach | De-risks | Cost / risk |
|---|---|---|---|
| **A** | **Custom virtio-accel device via a vhost-user daemon.** Guest driver reuses virtio-pci/virtqueue (F3); op-batches + tensor DMA buffers ride a virtqueue; a Rust daemon in `tools/` runs the ops and signals completion via the used ring + IRQ. | **both (a) + (b)** | High. Needs: a device-protocol ADR **with a hand-laid byte oracle** (CLAUDE.md rule — the descriptor/op-batch layout is an on-wire format); a vhost-user daemon (the vhost-user protocol is non-trivial); CI plumbing to launch the daemon + socket **before** QEMU and tear it down. |
| **B** | **Patch QEMU with a built-in device model.** | both | **Rejected.** Shipping a QEMU fork in CI is the same anti-pattern as a custom bootloader (Article 4.4 chose Limine precisely to avoid bespoke boot/emulation forks). Fragile, heavy. |
| **C** | **In-kernel `accel_sim` device — no virtio.** A second `AccelDriver` on `Accel(1)` with its own **arena** "device memory" (a distinct allocation pool, real copies) and **simulated async** completion (deferred via a kernel worker/timer so `poll` actually returns `Pending`→`Complete`). Pure kernel + test. | **(a) only** | Low. Zero host/daemon/CI-socket dependency; lands fast and green. Does **not** touch virtqueue/DMA/IRQ. |

## 3. Recommendation — split §9.2's monolithic deliverable; do the cheap logical de-risk first

The project's discipline is incremental slicing (Phase 5 sliced L0/L1/L2; ADR-030 explicitly "prove the abstraction against a testable driver before exotic hardware"). Apply it here — **re-slice the single "virtio-gpu-compute" deliverable into two builds**:

- **BUILD-2 = `accel_sim` (Option C).** Cheap, immediate, pure kernel+test. Proves the logical multi-device + async + real-copy + migration surface and resolves the two BUILD-1 notes — catching latent non-CPU-device bugs *now*, before we invest in the heavy transport lift.
- **BUILD-3 = virtio-accel via vhost-user (Option A).** The transport de-risk. Deserves its **own substrate ADR** (with a byte-layout oracle for the device protocol) and its own PLAN — it is a substantial standalone effort (daemon + CI plumbing) that should not be rushed inside BUILD-2.
- **BUILD-4 = one real GPU** (Intel/AMD; §9.2 unchanged in intent).

**Alternative (if PL prefers one virtio build, no test-double):** skip `accel_sim`, make **BUILD-2 = virtio-accel via vhost-user** directly, accepting the larger single lift and CI daemon plumbing up front. This keeps Phase 6 at three builds but front-loads all the risk into one dispatch.

Either way, **`accel_sim` is a legitimate de-risk, not makework**: it exercises code paths (`Pending` completion, non-identity device copies, a non-default device in `exec`) that `accel_cpu` *cannot*, regardless of whether a virtio device follows.

## 4. Governance impact (PM/PL)

- **ANSPEC-001 §9.2 amendment (required):** "virtio-gpu-compute" is not a real device. Replace with: *the Phase 6 CI accelerator is a **custom virtio-accel device** backed by a **vhost-user daemon** (QEMU generic `vhost-user-device`, pci variant — arbitrary `virtio-id`/`num_vqs`), reusing the kernel's existing virtio-pci transport.* If we adopt the recommended slice, also record the `accel_sim` interim build.
- **Build-sequence amendment** to ADR-030 / the Phase 6 ledger line if the slice changes (BUILD-2 → BUILD-3 → BUILD-4).

## 5. Open questions the PLAN/ADR must answer (for the virtio-accel build, whichever number)

1. **Device protocol** — the op-batch descriptor + tensor-buffer layout on the virtqueue. This is an on-wire format ⇒ **mandatory hand-laid byte oracle** (CLAUDE.md), independent of writer/reader.
2. **Device memory model** — are tensor buffers guest-physical DMA buffers the daemon maps, or copied through bounce buffers? How does `DeviceStorage` represent virtio device memory vs `accel_sim`'s arena?
3. **Async completion** — used-ring IRQ → driver marks the `DispatchToken` complete; how `poll`/`wait` integrate with the IRQ (this is where `wait()`-on-`Pending` gets its real semantics).
4. **Daemon** — Rust, in `tools/`; vhost-user protocol surface needed; CI launch/teardown ordering vs QEMU; does it run the reference op math or just echo for a transport-only test?
5. **Transport reuse** — factor a shared virtqueue module out of virtio-net/blk, or copy the pattern? (Coherence vs. coupling.)

## 6. Scope guard

THINK only — **no code in this pass.** Next gate: PM/PL pick the slice (§3), then a substrate ADR (for the virtio-accel build) + a focused PLAN. The `accel_sim` build, if chosen, can proceed on a lighter PLAN (no new ADR — it's an application of ADR-030's existing trait).

---

**Sources (F1/F2):**
- QEMU virtio-gpu (graphics only): https://www.qemu.org/docs/master/system/devices/virtio/virtio-gpu.html
- QEMU vhost-user back ends + generic `vhost-user-device` (`virtio-id`/`num_vqs`/`chardev`): https://www.qemu.org/docs/master/system/devices/virtio/vhost-user.html
- Writing VirtIO backends for QEMU: https://www.qemu.org/docs/master/devel/virtio-backends.html
