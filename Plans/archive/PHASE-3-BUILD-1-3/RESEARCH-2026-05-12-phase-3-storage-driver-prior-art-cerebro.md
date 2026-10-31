# Research: Phase 3 Storage-Driver Prior Art

| | |
|---|---|
| **Date** | 2026-05-12 (fresh-context Cerebro session) |
| **Author** | Cerebro deep-research artifact (dispatched via `DISPATCH-2026-05-12-phase-3-storage-driver-prior-art-cerebro.md`) |
| **Type** | Research artifact (informational; not normative). Question-level prior-art survey within ADR-021 frame. |
| **Authority** | None. PM consumes and either confirms storage-driver sequencing or revises ADR-021. |
| **Dispatch type** | Question-level (not structural-challenge). Phase 3 frame established by Cerebro #1 and ADR-021. |
| **Time budget actual** | ~2.5 hours (three parallel research agents + synthesis) |

---

## 1. Executive Summary

**The BUILD-1 write-through correctness path rests on a spec case, not a generic guarantee, and the driver must assert it at probe time.** The OASIS virtio v1.2 spec defines three durability cases for virtio-blk write completion. Case (1) — which is what BUILD-1 relies on — requires that the device *offer* `VIRTIO_BLK_F_FLUSH` (bit 9) while the driver does *not* negotiate it. Under that condition, the spec mandates every completed write is stable on persistent storage (MUST-durable). If the device does not even offer `VIRTIO_BLK_F_FLUSH`, durability downgrades from MUST to MAY. NeoPlutarch's BUILD-1 driver must assert that the device offers F_FLUSH at probe time; if it doesn't, refuse to attach rather than proceed on the MAY-durable path. This assertion is the single most important BUILD-1 correctness gate.

**The ADR-021 IoRendezvous model is confirmed sound with three extensions.** The Plan 9 rendezvous invariant ("the lock must be held with interrupts disabled on this processor, so the interrupt occurs only on a different processor") is satisfied by SwitchMutex at IF=0 on both sides. Three additions: (1) the SwitchMutex must be held *across* the issuing thread's park transition — `park_self_and_release_mutex_atomically(guard)` — to prevent the lost-wakeup race; (2) `waiting_thread` should be `Option<Weak<Thread>>` rather than `Option<Arc<Thread>>` so the rendezvous doesn't fight the SC-4 exit path; (3) the DMA buffer must be `Arc<DmaBuffer>` attached to the rendezvous so it survives process exit. These mirror the Linux bio-refcount discipline translated to Rust idiom.

**Memory barriers at the descriptor ring boundary are subtle and architecture-dependent.** The OASIS spec mandates two distinct barriers in the submission path. Step-3 (before updating avail→idx to expose descriptors to the device) is satisfied by a compiler fence on x86-64 because TSO orders prior stores. Step-5 (before checking notification-suppression flags) requires a real `mfence` because TSO permits store-before-load reordering. Encoding both as `fence(Ordering::SeqCst)` is correct but unnecessarily expensive; encoding both as `compiler_fence(Ordering::SeqCst)` is wrong and will lose notifications. Audit per-step. Additionally: aarch64 breaks this differently (needs explicit `dsb st` before MMIO notify) — add an aarch64 CI lane before BUILD-1 closes.

**For DMA infrastructure, the Redox `Dma<T>` pattern stripped of physmap (because identity-DMA) with Arc lifetime management is the recommended BUILD-1 abstraction.** The submission API should take `&[Arc<DmaBuffer>]` (a slice, not a single buffer) from day one, so BUILD-5's AHCI PRDT and NVMe PRP scatter-gather patterns are forward-compatible without rewriting callers. Identity-DMA collapses phys == virt, making the abstraction lightweight.

**AHCI and NVMe are BUILD-5 but should be designed into the PCI enumeration layer now.** BUILD-1 needs only `(vendor=0x1AF4, device=0x1042)` recognition and BAR4 MMIO mapping for modern virtio-blk. The PCI walker should be written as `for_each_pci_function → dispatch_by_id_or_class(...)` so AHCI (class `0x010601`) and NVMe (class `0x010802`) entries in BUILD-5 are additions, not rewrites. Interrupt routing for BUILD-1: INTx, always works in QEMU. For BUILD-5 NVMe, MSI with INTx fallback via `PCI_IRQ_ALL_TYPES`-style probe; MSI-X deferred until multi-queue.

---

## 2. Sources Surveyed

### Primary sources — verified at URL

- OASIS virtio v1.2 spec (HTML): `https://docs.oasis-open.org/virtio/virtio/v1.2/csd01/virtio-v1.2-csd01.html` §2.7.5, §2.7.13, §2.7.14, §4.1.2, §4.1.4, §5.2.3, §5.2.6. Read for feature-bit definitions, descriptor ring protocol, memory barrier requirements, PCI capability structure layout.
- OASIS virtio-spec GitHub (LaTeX source): `https://github.com/oasis-tcs/virtio-spec` `device-types/blk/description.tex`. Read for normative MUST/SHOULD durability text verbatim.
- Linux `include/uapi/linux/virtio_blk.h` master: `https://raw.githubusercontent.com/torvalds/linux/master/include/uapi/linux/virtio_blk.h`. Read for full feature-bit table.
- Linux `drivers/block/virtio_blk.c` master: `https://raw.githubusercontent.com/torvalds/linux/master/drivers/block/virtio_blk.c`. Read for `virtblk_get_cache_mode`, blk-mq tag-set preallocation discipline.
- Linux `include/linux/pci_ids.h` master: `https://raw.githubusercontent.com/torvalds/linux/master/include/linux/pci_ids.h`. Read for PCI class codes.
- Linux `drivers/nvme/host/pci.c` master: `https://raw.githubusercontent.com/torvalds/linux/master/drivers/nvme/host/pci.c`. Read for NVMe admin queue depth (32), `PCI_IRQ_ALL_TYPES` usage, single-vector fallback.
- Linux `drivers/ata/libahci.c` master: `https://raw.githubusercontent.com/torvalds/linux/master/drivers/ata/libahci.c`. Read for AHCI initialization sequence, `ahci_kick_engine()`, interrupt model.
- Redox OS virtio-blkd driver (scheme.rs + main.rs): `https://gitlab.redox-os.org/redox-os/drivers/-/tree/master/storage/virtio-blkd`. Read for feature negotiation (no explicit block-specific features), 3-descriptor chain shape, `BlockRequestTy` enum.
- rcore-os/virtio-drivers `src/device/blk.rs`: `https://github.com/rcore-os/virtio-drivers`. Read for `SUPPORTED_FEATURES`, `QUEUE_SIZE=16`, conditional FLUSH path.
- Redox OS `Dma<T>` docs + source: `https://doc.redox-os.org/io_uring/syscall/io/struct.Dma.html`. Read for DMA abstraction pattern.
- Redox OS RSoC io_uring report: `https://www.redox-os.org/news/io_uring-3/`. Read for `physalloc3` scatter-gather rationale.
- Bonzini 2012 QEMU virtio-blk write-through patches: `https://patchwork.kernel.org/project/kvm/patch/1341321642-24598-3-git-send-email-pbonzini@redhat.com/`. Read for QEMU/Linux write-through convention (not a spec guarantee).
- Plan 9 rendezvous paper (Pike, Presotto, Thompson): `http://doc.cat-v.org/plan_9/4th_edition/papers/sleep`. Read for lost-wakeup invariant and SMP-safe discipline.
- Linux completion.c: `https://raw.githubusercontent.com/torvalds/linux/master/kernel/sched/completion.c`. Read for `done` counter pattern, `complete()` spinlock discipline.
- Linux completion docs: `https://www.kernel.org/doc/html/latest/scheduler/completion.html`. Read for IRQ-safe `complete()` semantics.
- QEMU manpage: `https://www.qemu.org/docs/master/system/qemu-manpage.html`. Read for `werror`/`rerror`/`cache` option semantics.
- QEMU NVMe device docs: `https://raw.githubusercontent.com/qemu/qemu/master/docs/system/devices/nvme.rst`. Read for NVMe emulation defaults (MDTS=7, MSI-X, spec version 1.4).
- NVMe NVM Command Set Spec v1.0a: `https://nvmexpress.org/wp-content/uploads/NVMe-NVM-Command-Set-Specification-1.0a-2021.07.26-Ratified.pdf`. Read for FLUSH "shall commit to non-volatile media" mandate.

### Cannot verify from primary source

- AHCI 1.3.1 specification PDF (Intel) — binary, not text-extractable. Register offsets and init sequence confirmed via Linux `libahci.c` source and well-corroborated secondary sources (OSDev wiki, multiple driver implementations).
- NVMe Base Specification 2.0c PDF — binary, not text-extractable. Queue setup confirmed via Linux `nvme/host/pci.c` and consistent secondary sources.
- Exact PCI class code QEMU emits for virtio-blk-pci — not directly confirmed. **Mitigation:** do not class-gate virtio-blk; key on (vendor=0x1AF4, device=0x1042).
- Theseus OS storage driver — no primary source found. Theseus confirmed as a non-load-bearing reference for this dispatch.

---

## 3. Q1 — virtio-blk BUILD-1 Write-Through Path

### 3.1 Spec durability cases (verified verbatim from OASIS LaTeX source)

The OASIS v1.2 spec defines three durability cases. The normative text (from `device-types/blk/description.tex`):

> "A write becomes stable once it is completed and one or more of the following conditions is true: (1) neither VIRTIO_BLK_F_CONFIG_WCE nor VIRTIO_BLK_F_FLUSH feature were negotiated, but VIRTIO_BLK_F_FLUSH was offered by the device; (2) the VIRTIO_BLK_F_CONFIG_WCE feature was negotiated and the writeback field in configuration space was 0 all the time between submission and completion; (3) a VIRTIO_BLK_T_FLUSH request is sent after the write is completed and is completed itself."

**Naming correction:** The dispatch's `VIRTIO_BLK_F_WCE` is the historical name for what the v1.2 spec formally defines as `VIRTIO_BLK_F_CONFIG_WCE` (bit 11). `VIRTIO_BLK_F_FLUSH` is bit 9. These are distinct bits with distinct semantics. `CONFIG_WCE` gates toggling cache mode via config space; `FLUSH` gates the flush command type.

**The BUILD-1 path is Case (1).** The device must *offer* `VIRTIO_BLK_F_FLUSH` while the driver does *not* negotiate it. Under this condition: "the device MUST ensure that stable writes are committed to it, before reporting completion of the write." **The driver must assert at probe time that the device offers F_FLUSH.**

If the device does not offer F_FLUSH: Case (1) does not apply, and the spec downgrades to "MAY commit writes." NeoPlutarch must refuse to attach to such a device in BUILD-1 rather than operate on the MAY-durable path silently.

**The QEMU/Linux convention:** Bonzini's 2012 patches established that QEMU exposes `VIRTIO_BLK_F_FLUSH` whenever the host-side backing may have a volatile write cache, and enters write-through mode when the guest does not negotiate `CONFIG_WCE`. This makes Case (1) semantics reliable for QEMU and most KVM/Linux environments — but it is a community convention, not a universal spec guarantee.

### 3.2 BUILD-1 feature negotiation profile

**Confirmed from `linux/virtio_blk.h` primary source:**

| Bit | Name | BUILD-1 disposition | Rationale |
|---|---|---|---|
| 32 | `VIRTIO_F_VERSION_1` | **MUST accept** | Modern v1.x transport compliance |
| 5 | `F_RO` | **Accept and honor** | Refuse writes if set |
| 6 | `F_BLK_SIZE` | Accept | Informational block size |
| 1 | `F_SIZE_MAX` | Accept | Bounds max request size |
| 2 | `F_SEG_MAX` | Accept | Bounds descriptor chain |
| 10 | `F_TOPOLOGY` | Accept | Informational |
| 33 | `F_ACCESS_PLATFORM` | Accept if offered | IOMMU/platform presence |
| 9 | `F_FLUSH` | **Reject** | Triggers Case-(1) write-through correctness |
| 11 | `F_CONFIG_WCE` | **Reject** | No togglable cache mode in BUILD-1 |
| 12 | `F_MQ` | **Reject** | Single queue; matches single-thread-per-process |
| 7 | `F_SCSI` | **Reject** | Security hazard; legacy |
| 0 | `F_BARRIER` | Reject | Legacy-only |
| 4 | `F_GEOMETRY` | Reject | CHS geometry irrelevant |
| 28 | `F_RING_INDIRECT_DESC` | Reject | 3-descriptor chains are direct |
| 29 | `F_RING_EVENT_IDX` | Reject | Complexity without need at BUILD-1 |
| 13/14/16/17 | DISCARD/WRITE_ZEROES/SECURE_ERASE/ZONED | Reject | Out of scope |

**Comparison:** Redox virtio-blkd implements no FLUSH and no block-specific feature bits — validates the BUILD-1 design. rcore-os/virtio-drivers opts into FLUSH — different from the BUILD-1 target, but confirms the negotiation-table approach.

### 3.3 BUILD-3 FLUSH upgrade profile

**What changes vs. BUILD-1:**
- Negotiate `VIRTIO_BLK_F_FLUSH` (bit 9).
- Add `VIRTIO_BLK_T_FLUSH = 4` request path.
- FLUSH uses a 2-descriptor chain: header (device-read) + status (device-write). Spec: `sector = 0`; SHOULD NOT include data.
- CoW commit sequence becomes: write content blocks → FLUSH → write superblock → FLUSH.
- Do NOT negotiate `CONFIG_WCE` (bit 11) — simpler; avoids togglable cache mode.

**QEMU alignment:** with `cache=writeback`, QEMU only commits data to backing store on FLUSH. BUILD-3 FLUSH tests should use `cache=writeback` to make FLUSH semantics observable.

---

## 4. Q2 — Descriptor Ring Discipline

### 4.1 Descriptor chain shape

**Confirmed from OASIS spec §5.2.6 and Redox scheme.rs:**

**Read (`VIRTIO_BLK_T_IN = 0`):** 3 descriptors
1. Header (`struct virtio_blk_req`: type, reserved, sector) — device-readable, `NEXT` set
2. Data buffer — device-writable (`VRING_DESC_F_WRITE` set), `NEXT` set
3. Status byte — device-writable, no `NEXT` (tail)

**Write (`VIRTIO_BLK_T_OUT = 1`):** 3 descriptors
1. Header — device-readable, `NEXT` set
2. Data buffer — device-readable (no `WRITE` flag), `NEXT` set
3. Status byte — device-writable, no `NEXT`

**FLUSH (`VIRTIO_BLK_T_FLUSH = 4`, BUILD-3 only):** 2 descriptors
1. Header (sector=0, no data) — device-readable, `NEXT` set
2. Status byte — device-writable, no `NEXT`

Constants: `VRING_DESC_F_NEXT=1`, `VRING_DESC_F_WRITE=2`. Spec §2.7.13.2: device-readable elements must precede device-writable ones.

### 4.2 Memory barrier requirements (OASIS spec §2.7.13)

**Two mandatory MUST barriers in the submission path:**

| OASIS step | Action | Barrier type on x86-64 |
|---|---|---|
| Step 4 | Before updating `avail→idx` (exposes descriptors to device) | **Compiler fence only** — x86 TSO orders prior stores |
| Step 6 | Before checking notification-suppression flags (`flags`/`avail_event`) | **Real `mfence`** — TSO permits store-before-load reordering |

**Critical:** encoding both as `compiler_fence(Ordering::SeqCst)` is wrong for step 6. Encoding both as `fence(Ordering::SeqCst)` is correct but wasteful. Audit per-step.

**Used-ring read side:** driver reads `used→idx` before `used→ring[N]`. Without a barrier, the compiler may reorder these. Use acquire-ordered atomic or `compiler_fence` before consuming used-ring entries.

**MMIO notify ordering on x86-64:** stores to UC MMIO regions are strongly ordered against prior WB stores by the platform (Intel SDM Vol.3 §11.3.1). Rely on this; document it as a platform assumption — aarch64 needs explicit `dsb st` before MMIO.

**aarch64 CI lane:** x86-TSO masks step-6 barrier bugs. Add an aarch64 QEMU lane before BUILD-1 closes.

### 4.3 No-allocation in IRQ context

**Confirmed from rcore-os (primary) and Redox (primary):**

- rcore-os: `VirtQueue<H, { QUEUE_SIZE as usize }>` with `QUEUE_SIZE: u16 = 16` — compile-time fixed, pre-allocated.
- Redox: descriptor pool implicit in the ring structure; no IRQ-context allocation.

**Recommended NeoPlutarch BUILD-1:**
```rust
// Static descriptor pool, sized at boot:
struct VirtQueue {
    ring: VirtqueueRing,        // physically contiguous
    slot_bitmap: AtomicU16,     // N=16 bit free-list
    side_headers: [BlockReqHeader; 16],   // pre-allocated
    side_status:  [u8; 16],              // pre-allocated
}
```
Slot acquisition: `slot_bitmap.fetch_and` to claim; IRQ handler clears the bit (no heap alloc/dealloc, just a bit flip). The caller's data buffer is registered directly as the data descriptor — caller is blocked, so the buffer is alive.

**Forward-compat note:** the submission API should take `&[Arc<DmaBuffer>]` (a slice, not singular) from BUILD-1. virtio-blk BUILD-1 passes a single-element slice. AHCI PRDT and NVMe PRP/SGL will pass N elements. Redox's `physalloc3` addition was made specifically because NVMe needed scatter-gather; avoid repeating this late-addition friction.

---

## 5. Q3 — DMA Infrastructure for Small Rust Kernels

### 5.1 Production Rust kernel patterns

| Kernel | Buffer type | UAF mechanism | Device-window |
|---|---|---|---|
| Redox | `Dma<T>` = `PhysBox` + virt mapping + `Arc` | Move semantics on wrapper | Driver structural discipline |
| Theseus | `MappedPages` (bijective, compiler-enforced lifetime) | Compiler enforces lifetime bounds | Structural (thread blocked) |
| Tock | `TakeCell<&'static mut [u8]>` | `take()` empties cell — can't double-hand | Static (cell is None while device owns) |
| embedded-dma | `Pin<B> + 'static` (ReadBuffer/WriteBuffer traits) | `'static` lifetime | Typestate `Transfer<B>` |
| Linux | `dma_handle` + CPU pointer | Driver discipline + bio refcount | bio refcount |

### 5.2 Recommended BUILD-1 DMA abstraction

```rust
pub struct DmaBuffer {
    phys: PhysAddr,   // == virt under ADR-021 identity-DMA invariant
    len: usize,
    // Drop returns frames to the DMA pool
}

impl DmaBuffer {
    pub fn alloc_zeroed(len_pages: usize) -> Result<Arc<Self>, DmaError>;
    pub fn phys(&self) -> PhysAddr;
    pub fn len(&self) -> usize;
    pub fn as_slice(&self) -> &[u8];        // safe: identity-DMA + Arc lifetime
    pub fn as_mut_slice(&mut self) -> &mut [u8]; // requires unique Arc
}
```

This is the Redox `Dma<T>` pattern stripped of `physmap` (because identity-DMA collapses phys==virt), with `Arc` as the lifetime carrier. The `Arc<DmaBuffer>` is attached to the `IoRendezvous` so the buffer outlives process exit (Linux bio-refcount discipline in Rust idiom).

### 5.3 Forward-compat for AHCI/NVMe BUILD-5

Three things BUILD-1 must leave room for without implementing:
1. **Scatter-gather:** `submit(&[Arc<DmaBuffer>])` — slice not singular. AHCI PRDT and NVMe PRP lists take N entries.
2. **Per-command DMA pool:** `DmaBuffer::alloc_zeroed(len_pages)` returning `Result<_, DmaError::Fragmented>` — BUILD-1 panics on fragmented; BUILD-5 falls back to scatter-gather. Mirrors Redox's `physalloc3`.
3. **Larger buffer sizes (≥256 KiB):** pure `len_pages` parameter; the type already supports it. Physical contiguity constraint is the implementation concern, not the API.

**Do not put descriptor-table logic in `DmaBuffer`.** virtio, AHCI, and NVMe each have different descriptor formats. Keep `DmaBuffer` as "physically contiguous frames"; descriptor construction lives in per-driver code.

---

## 6. Q4 — Storage IRQ Completion and SC-3C Wake Composition

### 6.1 ADR-021 rendezvous model: confirmed with three extensions

**Plan 9 invariant** (primary source, Pike/Presotto/Thompson): *"the lock must be set only with interrupts disabled on the current processor, so that if the interrupt comes during sleep it will occur only on a different processor; if it occurred on the processor executing sleep, the spin lock in wakeup would hang forever."*

NeoPlutarch's SwitchMutex at IF=0 on both sides satisfies this invariant. **Sound.**

**Three required extensions:**

1. **`park_self_and_release_mutex_atomically(guard)` must exist.** The issuing thread must check `done` and install `waiting_thread` inside the SwitchMutex, then park while atomically releasing the mutex. Without this atomic park-release, the lost-wakeup hole is: IRQ fires after `waiting_thread` is set but before the thread parks → wake is lost → thread parks forever. Linux `completion.c` solves this by updating `done` and calling `swake_up_locked()` both under the spinlock. NeoPlutarch must implement the same. **Confirm this primitive exists in SC-3C before BUILD-1 sub-commit start.**

2. **`waiting_thread: Option<Weak<Thread>>`**, not `Option<Arc<Thread>>`. The `Arc<Thread>` would keep the thread struct alive after process exit, fighting the SC-4 exit path. Use `Weak<Thread>` (or `Option<ThreadId>` with scheduler-side liveness check). IRQ handler upgrades to `Arc<Thread>` under the SwitchMutex; if upgrade fails (thread freed), skip the wake — no-op, clean.

3. **`DmaBuffer` must be `Arc`-managed and attached to the rendezvous.** When the process is terminated, the process drops its reference; the IRQ side's reference keeps both the rendezvous and the DMA buffer alive until the IRQ fires. This is the Linux bio-refcount discipline in Rust.

### 6.2 Memory ordering inside the rendezvous

Both sides operate under the SwitchMutex. Inside the critical section: use `Ordering::Relaxed` for `done` store/load — the mutex acquire/release pair provides the memory barrier. This is the Linux `completion.c` pattern.

**If a fast-path peek is added later** (check `done` before taking the SwitchMutex): the peek load must use `Ordering::Acquire`; the IRQ store must use `Ordering::Release`; must re-check under mutex on miss. Document which ordering is in force at each access site. Linux completion.c is famous for subtle correctness here — document it proactively.

### 6.3 Issuer killed while I/O outstanding

**Recommended discipline: Option A (Arc keeps rendezvous alive; wake is no-op if thread gone).** This mirrors Linux's bio-refcount approach. Process exit drops its reference; the IRQ handler still holds `Arc<IoRendezvous>`; rendezvous and DMA buffer stay alive; IRQ fires, stores `done=true`, attempts `Weak::upgrade(waiting_thread)` — fails → skips wake → drops its Arc → rendezvous freed.

Do NOT block exit until I/O completes. A stuck storage device would prevent process termination — violates SC-4 termination guarantees.

### 6.4 IRQ handler work breakdown

All five steps can run in the IRQ handler under BUILD-1:
1. Read used-ring entry — O(1) memory read
2. Update `used→idx` — O(1) store
3. Copy status byte — single byte read
4. Wake waiting thread via SC-3C cross-CPU IPI — non-blocking by design
5. Release descriptor slot — bit-clear in bitmap, under virtqueue-internal lock

**Lock ordering:** virtqueue-internal-lock → SwitchMutex, on both sides (submission path and IRQ path). Never reverse.

**IRQ must NOT do:** heap allocation; full blockstore scan; any blocking operation; acquiring more than two locks. Background GC / buffer-cache writeback belongs to a kthread, not the IRQ handler.

---

## 7. Q5 — PCI/PCIe Enumeration Staging

### 7.1 Minimum BUILD-1 scan for virtio-blk

1. Bus walk: read Vendor ID (offset 0x00) and Device ID (offset 0x02). Match `(0x1AF4, 0x1042)` for modern virtio-blk. **Refuse `(0x1AF4, 0x1001)` (legacy) — different ABI.** Do not class-gate virtio-blk; key on (vendor, device).
2. Check Status register bit 4 (capabilities list valid).
3. Walk PCI capabilities from offset `0x34`. For cap_id `0x09` (vendor), interpret as `struct virtio_pci_cap` per OASIS v1.2 §4.1.4: `{cap_vndr, cap_next, cap_len, cfg_type, bar, padding[3], offset, length}`.
4. `cfg_type` values: `COMMON_CFG=1`, `NOTIFY_CFG=2`, `ISR_CFG=3`, `DEVICE_CFG=4`, `PCI_CFG=5`.
5. Map BAR4 MMIO (typical for QEMU modern virtio-blk, per OASIS transport convention). Parse the four regions from cap-struct offsets.

**Forward-compat:** write the PCI walker as `for_each_pci_function → dispatch_by_id_or_class(driver_table)`. AHCI and NVMe become new entries in the table at BUILD-5 without rewriting the walker.

### 7.2 PCI class codes (confirmed from `linux/pci_ids.h`)

| Class code | Device type | BUILD |
|---|---|---|
| Key on (0x1AF4, 0x1042) | virtio-blk modern | BUILD-1 |
| `0x010601` | SATA AHCI | BUILD-5 |
| `0x010802` | NVMe | BUILD-5 |

### 7.3 Interrupt routing recommendation

| Driver | Mode | Rationale |
|---|---|---|
| BUILD-1 virtio-blk | **INTx** | One queue; no per-vector affinity needed; QEMU PIIX3/Q35 IOAPIC well-tested; delays MSI/MSI-X bring-up complexity |
| BUILD-5 AHCI | **INTx or single MSI** | One IS register fans out across all ports; single-IRQ path is libahci's architectural default |
| BUILD-5 NVMe | **Single-message MSI with INTx fallback** | Single queue doesn't need MSI-X; Linux `PCI_IRQ_ALL_TYPES` falls back automatically; MSI-X deferred to multi-queue |

**Strategic note:** starting with INTx everywhere means zero vector-pool accounting for BUILD-1/5. MSI-X brings (a) vector pool ownership, (b) per-CPU affinity policy, (c) programming-race concerns — defer all three until genuinely needed.

---

## 8. Q6 — AHCI Driver Architecture (BUILD-5)

### 8.1 Core data structures

**HBA Memory Registers** at ABAR+0x00: GHC (0x04, `AE`=bit31, `HR`=bit0, `IE`=bit1), IS (0x08, one bit per port), PI (0x0C, ports-implemented bitmap), VS (0x10), CAP (0x00, capability bits).

**Per-port block** at `ABAR + 0x100 + (port × 0x80)`: PxCLB (0x00, command list base, 1 KiB-aligned), PxFB (0x08, FIS receive base, 256B-aligned), PxIS (0x10), PxIE (0x14), PxCMD (0x18, `ST`=bit0, `FRE`=bit4, `CR`=bit15, `FR`=bit14), PxTFD (0x20, TFD.STS.BSY=bit7), PxCI (0x38, write bit N to issue slot N).

**DMA structures (per port):**
- Command List: 32 × 32-byte headers = 1 KiB, 1 KiB-aligned. Each header: `cfl:5, a, w, p, r, b, c, ..., prdtl:16, prdbc:32, ctba:32`.
- Received FIS: 256 bytes, 256B-aligned.
- Command Table: `cfis[64]` + `acmd[16]` + `rsv[48]` + `prdt_entry[N]` (N = prdtl), 128B-aligned.
- PRDT entry (16 bytes): `dba:32, dbau:32, rsv:32, dbc:22 (byte count − 1), rsv:9, i:1`.

**Memory budget per port for Phase 3 single-slot:** 1 KiB (CL) + 256 B (FB) + ~256 B (command table + 1 PRDT) ≈ 1.5 KiB. Reserve 4 KiB (one page) per port.

### 8.2 Initialization sequence (minimum to reach IDENTIFY)

1. PCI: class `0x010601`, enable Memory + Bus Master, map BAR5 (ABAR).
2. (Bare-metal only) BIOS handoff: if `CAP2.BOH=1`, set `BOHC.OOS`, wait `BOHC.BOS=0`.
3. Enable AHCI: `GHC.AE = 1`.
4. Optional HBA reset: `GHC.HR = 1`, poll until clears, re-enable `GHC.AE`.
5. Enumerate ports via PI register.
6. **For each port — STOP first (mandatory before CLB/FB write):** Clear `PxCMD.ST`, poll `PxCMD.CR=0` (≤500 ms). Clear `PxCMD.FRE`, poll `PxCMD.FR=0` (≤500 ms).
7. Allocate Command List (1 KiB) and FIS base (256 B), zeroed. Write `PxCLB`/`PxFB`.
8. Clear errors: `PxSERR = 0xFFFFFFFF` (RW1C). Clear `PxIS`.
9. Start: set `PxCMD.FRE`, then `PxCMD.ST`. Wait `PxTFD.STS.BSY=0` and `DRQ=0`. Check `PxSSTS.DET=3`.
10. Issue IDENTIFY (ATA cmd 0xEC) via slot 0, 512-byte DMA buffer.

**Do not skip steps 6 and 8.** Omitting port-stop before CLB/FB write silently wedges the port on second boot. Omitting PxSERR clear leaves error latched, corrupting the next command's completion.

### 8.3 AHCI BUILD-5 anti-scope

| Feature | Defer | Forward-compat invariant |
|---|---|---|
| Port Multipliers | Yes | Never use PMP field; leave `PxFBS=0` |
| Hotplug/SSP | Yes | Log `PxIS.PCS`/`PxIS.PRCS` and ignore; never panic |
| NCQ (FPDMA) | Yes | Use only slot 0 or one-at-a-time non-queued; ATA cmd 0x25 not 0x60 |
| Power Management | Yes | Leave `PxCMD.ICC` at 0; leave ALPE/ASP at 0 |
| ATAPI | Yes | Detect sig 0xEB140101 at IDENTIFY time; return ENOTSUP |
| Enclosure Management | Yes | Read CAP.EMS; don't touch EM_LOC/EM_CTL |

---

## 9. Q7 — NVMe Driver Architecture (BUILD-5)

### 9.1 Admin + I/O queue setup

**Controller register block** (BAR0 MMIO): CAP (0x00, 8B), VS (0x08, 4B), INTMS/INTMC (0x0C/0x10), CC (0x14, `EN`=bit0, `CSS`=bits6:4, `IOSQES`=bits19:16, `IOCQES`=bits23:20), CSTS (0x1C, `RDY`=bit0, `CFS`=bit1), AQA (0x24), ASQ (0x28, 8B), ACQ (0x30, 8B). SQ/CQ doorbells at `0x1000 + 2y × (4 << CAP.DSTRD)`.

**Initialization sequence:**
1. If `CC.EN=1`, clear and wait `CSTS.RDY=0`.
2. Allocate Admin SQ (64B entries, depth 32) and Admin CQ (16B entries, depth 32), page-aligned, physically contiguous.
3. Write AQA, ASQ, ACQ. Write CC with `IOSQES=6, IOCQES=4, CSS=0, MPS=0, EN=1`.
4. Poll `CSTS.RDY=1` (or `CSTS.CFS=1` → fatal).
5. Admin commands: Identify Controller → parse MDTS, NN. Identify Active NS List → get NSIDs. Identify each NS → LBA format.
6. Create I/O CQ (admin opcode 0x05) → Create I/O SQ (admin opcode 0x01).

**Queue depth:** Admin=32 (Linux default). I/O queue=64 entries for Phase 3.

### 9.2 Single-thread queue model

**Recommendation: Model A — one admin + one I/O queue pair, per-controller mutex on SQ-tail writes.** At Phase 3 with single-thread-per-process and one outstanding I/O per process, Model A is correct and simple. Linux's "broken_msi quirk" single-vector fallback uses the same shape. Forward-compat: wrap SQ in a struct owning the doorbell offset; multi-queue in BUILD-6+ instantiates N of them without changing the upper-layer `submit()` API.

**Critical: tag every in-flight command with a `cid` from a per-queue free-list bitmap.** The `cid` is the completion → request matching key. Same code structure Model B will use.

### 9.3 PRP scope for Phase 3

| Request size | PRP1 | PRP2 | List needed? |
|---|---|---|---|
| 4 KiB (one page) | Page phys addr | Unused | No |
| 8 KiB | Page 0 | Page 1 direct | No |
| N pages (N>2) | Page 0 | Pointer to PRP-list page | Yes |
| N > 511 pages | Page 0 | PRP-list page, last entry chains | Yes |

Implement single-page, 2-page, and list cases. Limit Phase 3 MDTS to ≤512 KiB so one PRP-list page always suffices. Defer SGL.

**Doorbell stride gotcha:** `doorbell_offset = 4 << CAP.DSTRD` — it is `4 << DSTRD`, not `4 * DSTRD`. QEMU emits `DSTRD=0` so this bug is invisible until bare-metal with non-zero DSTRD. Catch it with a DSTRD=1 QEMU-patched test or bare-metal, before BUILD-5 closes.

### 9.4 Error handling mandatory from BUILD-5 day one

| Requirement | What to implement |
|---|---|
| Per-command timeout | Every in-flight command has a `submitted_at`; watchdog scans; 30 s default |
| `CSTS.CFS` detection | Any I/O completion checks `CSTS`; CFS=1 → controller FATAL |
| CC.EN toggle reset | On FATAL or timeout: abort in-flight commands, `CC.EN=0`, poll `CSTS.RDY=0`, re-init steps 1–6 |
| CQ phase-bit tracking | Each CQ has an expected phase-tag that flips on wraparound; wrong initial value → miss entries |

**Safe to defer:** NSSR (subsystem reset), error log pages, AER, namespace re-enumeration on attach/detach events, multipath/ANA, firmware update.

---

## 10. Q8 — Verification Strategy

### 10.1 QEMU configurations

**BUILD-1 (virtio-blk write-through baseline):**
```
-drive if=none,format=raw,file=disk.img,id=hd0,cache=none,aio=threads
-device virtio-blk-pci,drive=hd0,bus=pcie.0
```
`cache=none` opens the host file with O_DIRECT. QEMU still advertises a volatile write cache to the guest. Key test: kill QEMU after write, verify data on restart.

**BUILD-3 (FLUSH correctness):**
```
-drive if=none,format=raw,file=disk.img,id=hd0,cache=writeback
-device virtio-blk-pci,drive=hd0
```
`cache=writeback`: only an explicit FLUSH pushes data to disk. Enables the definitive FLUSH test: write N blocks, no FLUSH, kill QEMU → data may be lost; write N blocks, FLUSH, kill QEMU → data must be present.

**BUILD-5 AHCI:**
```
-device ahci,id=ahci0
-drive if=none,format=raw,file=disk.img,id=hd0,cache=none
-device ide-hd,drive=hd0,bus=ahci0.0
```

**BUILD-5 NVMe:**
```
-drive if=none,format=raw,file=nvm.img,id=nvm0,cache=none
-device nvme,serial=neo-1,drive=nvm0
```
QEMU NVMe emulates spec version 1.4, MDTS=7, MSI-X only by default. INTx fallback for NVMe can be tested only on bare-metal or a custom QEMU build — flag this as a known gap in BUILD-5 closure.

### 10.2 Fault injection options

From QEMU manpage (primary):
- `werror=report` + `rerror=report`: surfaces write/read errors to guest as I/O errors (exercises guest error-handling path).
- `werror=stop` + `resume`: pauses VM on write error; `cont` to retry (exercises driver pause/resume tolerance).
- `werror=enospc` (default for writes): surfaces disk-full to guest.
- `rerror=enospc` is not valid for reads.
- "Lost completion" simulation: use `werror=stop` and do not resume (exercises driver timeout path).
- Device reset mid-I/O: `system_reset` QMP command for whole-VM reset; `device_del`/`device_add` for per-device (NVMe hotplug).

### 10.3 BUILD-1 scenario list

| Scenario | What it proves |
|---|---|
| 4 KiB write LBA 0 + readback equality | Single-buffer DMA correctness |
| 4 KiB write at last LBA + readback | LBA arithmetic / 32-vs-64-bit overflow |
| Sequential 64 MiB write + streaming readback equality | Descriptor reuse, completion-rendezvous loop |
| Random-LBA 1024-iteration write + per-LBA readback | Random access; no stale CID/tag matching |
| N=4 CPU parallel writes on disjoint LBA ranges (60 s stress) | Multi-CPU SQ-lock correctness, completion rendezvous |
| Kill process mid-write (Issue + terminate before IRQ) | Abandoned-I/O cleanup; CID returned; no Arc leak |
| Disk full (`werror=report`, small backing image) | Error path: errno surfacing through block layer to VFS |
| 64 KiB write (16 pages) + readback | Multi-page descriptor chain correctness |
| FLUSH after write with `cache=writeback` (BUILD-1 proto-gate) | FLUSH intent; verifies no regression when BUILD-3 FLUSH lands |
| Boot → open + read expected content | VFS-through-virtio integration smoke |

### 10.4 Bible Article 13 closure record

BUILD-1 closure must record:
1. Driver match: `(0x1AF4, 0x1042)` bound; `(0x1AF4, 0x1001)` refused with log evidence.
2. **F_FLUSH offered assertion**: log the feature bits offered by QEMU device; confirm F_FLUSH (bit 9) is present in the offered set.
3. Feature-negotiation log: bits offered vs. bits accepted.
4. Test matrix: scenarios above with pass/fail + QEMU command line.
5. Hung-completion proof: scenario "kill mid-write" with log showing CID returned, no descriptor leak.
6. Deviation log: anything skipped vs. virtio v1.2 §5.2. Format per `feedback_cross_ref_enumeration.md` — literal enumeration, not "all verified."
7. Cross-reference enumeration: Bible Article 9.9 sub-clauses (virtio-blk required from Phase 3) each paired with the test scenario that closes it.

BUILD-5 additionally requires bare-metal evidence: at least one real SATA + one real NVMe drive, confirmed by console log / S.M.A.R.T. dump referencing a specific physical device.

---

## 11. BUILD Mapping

| Feature / Finding | BUILD-1 | BUILD-3 | BUILD-5 |
|---|---|---|---|
| virtio-blk (modern, F_FLUSH offered, not negotiated) | ✓ | — | — |
| FLUSH command + 2-descriptor chain | — | ✓ | — |
| Static descriptor pool (N=16), no IRQ-alloc | ✓ | ✓ | — |
| Arc<DmaBuffer> identity-DMA abstraction | ✓ | ✓ | + scatter-gather |
| IoRendezvous (SwitchMutex + Weak<Thread> + Arc<DmaBuffer>) | ✓ | ✓ | ✓ |
| PCI walker with dispatch table | ✓ | ✓ | + AHCI/NVMe entries |
| INTx interrupt routing | ✓ | ✓ | AHCI; NVMe MSI w/fallback |
| AHCI driver | — | — | ✓ |
| NVMe driver (single I/O queue, PRP, single MSI) | — | — | ✓ |
| Bare-metal validation | — | — | ✓ |

---

## 12. Risks and Corrections to ADR-021

**ADR-021 storage-driver sequencing: CONFIRMED.** Virtio-blk → AHCI → NVMe ordering is correct.

**Three extensions to ADR-021:**

1. **Add probe-time F_FLUSH assertion.** "NeoFS becomes first non-scheduler SC-3C IPI cross-CPU-wake client" is confirmed correct. Add: "the driver MUST assert `VIRTIO_BLK_F_FLUSH` is in the offered feature set at probe time; refuse to attach if it is not." This puts the driver on the spec's MUST-durable Case (1) path.

2. **Update IoRendezvous type for `waiting_thread`.** Change from `Option<Arc<Thread>>` to `Option<Weak<Thread>>`. This prevents the rendezvous from fighting the SC-4 exit path and is required for correctness.

3. **Add aarch64 CI lane requirement to BUILD-1 acceptance gates.** Memory-barrier bugs that x86-TSO masks will surface on aarch64. The barrier distinction (compiler-fence vs mfence) is the highest-risk implementation pitfall; bare-metal aarch64 or QEMU aarch64 is the net that catches it.

**No ADR-021 contradictions.** The five BUILD split, virtio-blk write-through rationale, identity-DMA choice, and SC-3C IPI wake composition are all confirmed correct by primary-source evidence.

---

## 13. Forward Markers (Phase 4+ / Phase 5+)

1. **virtio-blk FLUSH (BUILD-3).** Negotiate F_FLUSH; add `VIRTIO_BLK_T_FLUSH` path; two-flush CoW commit sequence.
2. **MSI/MSI-X bring-up (BUILD-5).** NVMe single MSI first; MSI-X when multi-queue lands.
3. **AHCI NCQ (BUILD-6+).** Gate: profiling shows single-command latency dominates on SATA.
4. **NVMe per-CPU I/O queues (BUILD-6+).** Gate: CPU count exceeds effective single-queue saturation point.
5. **NVMe SGL (BUILD-6+).** Gate: non-page-aligned scatter needed for zero-copy paths.
6. **IOMMU / non-identity DMA (Phase 5+).** ADR-021 deferred identity-DMA to Phase 5 explicitly; IOMMU bring-up is its own dispatch.
7. **NVMe Namespace Management (Phase 4+).** Namespace attach/detach events, namespace sets — out of Phase 3 scope.

---

## 14. Calibration Notes

**Verified primary sources.** All virtio spec citations verified from OASIS HTML + LaTeX source. All feature-bit values confirmed from Linux `virtio_blk.h`. Linux libahci.c and nvme/host/pci.c read directly. QEMU docs fetched directly. Redox virtio-blkd and rcore-os/virtio-drivers read directly.

**Key calibration note — VIRTIO_BLK_F_WCE terminology:** the dispatch uses "F_WCE" which is the historical/informal name. The v1.2 spec formally calls it `VIRTIO_BLK_F_CONFIG_WCE` (bit 11), distinct from `VIRTIO_BLK_F_FLUSH` (bit 9). The corrected naming is used throughout this artifact.

**Single-source findings:** AHCI register offsets and init sequence confirmed via Linux libahci.c (primary) and consistent secondary sources (OSDev wiki cross-referenced). NVMe queue setup confirmed via Linux nvme/host/pci.c (primary) and consistent secondary sources. Theseus OS storage driver — no primary source found; not load-bearing.

**Time-budget actual:** ~2.5 hours (three parallel research agents). Within 4–8 hour estimate.

---

## 15. Cross-References

**This artifact consumes:**
- `Plans/DISPATCH-2026-05-12-phase-3-storage-driver-prior-art-cerebro.md`
- `docs/ADR-021-phase-3-design-substrate.md` — ratified substrate frame
- `Plans/PHASE-3-THINK-V2-2026-05-11.md` — current THINK v2

**This artifact is consumed by:**
- PM storage-driver consumption PM-NOTE
- Phase 3 BUILD-1 PLAN dispatch for Jarvis

**This artifact does NOT produce:** no ADR amendments, no Bible amendments.

---

## 16. Closure

Phase 3 storage-driver prior art confirms ADR-021's sequencing with three non-trivial extensions: the F_FLUSH-offered probe assertion is a correctness gate (not merely a best-practice), the `Weak<Thread>` substitution is required for exit-path correctness, and the aarch64 CI lane is the barrier-correctness net that x86-TSO otherwise masks. AHCI and NVMe are well-understood at BUILD-5 scope; the init sequences are documented with precise "must not skip" step-labeling based on production driver experience. The DMA abstraction is simple (identity-DMA collapses the problem), forward-compatible (slice API for scatter-gather), and grounded in the Redox `Dma<T>` pattern. The IoRendezvous model is sound, the Plan 9 invariant is satisfied, and the Linux bio-refcount discipline translated to Rust idiom handles the process-exit edge case cleanly.

---

**End of research artifact.**

*ADR-021 storage-driver sequencing confirmed. Key extension: probe must assert device offers F_FLUSH (spec Case 1 durability requires it). IoRendezvous extended: waiting_thread = Weak<Thread>; SwitchMutex held across park transition; DmaBuffer Arc'd to outlive exit. Doorbell-stride and CQ-phase-bit are top BUILD-5 implementation risks. aarch64 CI lane is a BUILD-1 acceptance gate.*
