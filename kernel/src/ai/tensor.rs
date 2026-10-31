// SPDX-License-Identifier: MPL-2.0
//! Capability-backed tensor object and buddy-frame storage.

extern crate alloc;

use core::ptr;
#[cfg(any(
    feature = "neo-test-tensor-create-query-drop",
    feature = "neo-test-model-load-cascade",
    feature = "neo-test-infer-mlp-forward",
    feature = "neo-test-infer-transformer-forward",
    feature = "neo-test-inference-preemptible-liveness",
    feature = "neo-test-infer-embedding-forward",
    feature = "neo-test-ai-generate-decode",
    feature = "neo-test-accel-cpu-forward-migrate",
    feature = "neo-test-accel-sim-forward-equiv",
    feature = "neo-test-accel-sim-migrate-roundtrip",
    feature = "neo-test-accel-sim-async-poll",
    feature = "neo-test-virtio-accel-forward-equiv",
    feature = "neo-test-virtio-accel-migrate-roundtrip",
    feature = "neo-test-accel-intel-add-bringup",
    feature = "neo-test-accel-evict-pressure",
    feature = "neo-test-accel-residency-retain",
    feature = "neo-test-accel-residency-demote",
    feature = "neo-test-accel-migration-reservation",
    feature = "neo-test-accel-residency-daemon",
    feature = "neo-test-accel-slot-arena",
    feature = "neo-test-accel-model-residency",
    feature = "neo-test-accel-dedup-movement",
    feature = "neo-test-accel-tensor-sharing"
))]
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use x86_64::structures::paging::{PhysFrame, Size4KiB};

pub use super::tensor_logic::{
    Device, Dtype, MAX_RANK, PAGE_SIZE, Shape, TensorError, order_for_len, validate_data_len,
};
use crate::drivers::accel::{self, DeviceStorage};

#[cfg(any(
    feature = "neo-test-tensor-create-query-drop",
    feature = "neo-test-model-load-cascade",
    feature = "neo-test-infer-mlp-forward",
    feature = "neo-test-infer-transformer-forward",
    feature = "neo-test-inference-preemptible-liveness",
    feature = "neo-test-infer-embedding-forward",
    feature = "neo-test-ai-generate-decode",
    feature = "neo-test-accel-cpu-forward-migrate",
    feature = "neo-test-accel-sim-forward-equiv",
    feature = "neo-test-accel-sim-migrate-roundtrip",
    feature = "neo-test-accel-sim-async-poll",
    feature = "neo-test-virtio-accel-forward-equiv",
    feature = "neo-test-virtio-accel-migrate-roundtrip",
    feature = "neo-test-accel-intel-add-bringup",
    feature = "neo-test-accel-evict-pressure",
    feature = "neo-test-accel-residency-retain",
    feature = "neo-test-accel-residency-demote",
    feature = "neo-test-accel-migration-reservation",
    feature = "neo-test-accel-residency-daemon",
    feature = "neo-test-accel-slot-arena",
    feature = "neo-test-accel-model-residency",
    feature = "neo-test-accel-dedup-movement",
    feature = "neo-test-accel-tensor-sharing"
))]
static STORAGE_ALLOCS: AtomicUsize = AtomicUsize::new(0);
#[cfg(any(
    feature = "neo-test-tensor-create-query-drop",
    feature = "neo-test-model-load-cascade",
    feature = "neo-test-infer-mlp-forward",
    feature = "neo-test-infer-transformer-forward",
    feature = "neo-test-inference-preemptible-liveness",
    feature = "neo-test-infer-embedding-forward",
    feature = "neo-test-ai-generate-decode",
    feature = "neo-test-accel-cpu-forward-migrate",
    feature = "neo-test-accel-sim-forward-equiv",
    feature = "neo-test-accel-sim-migrate-roundtrip",
    feature = "neo-test-accel-sim-async-poll",
    feature = "neo-test-virtio-accel-forward-equiv",
    feature = "neo-test-virtio-accel-migrate-roundtrip",
    feature = "neo-test-accel-intel-add-bringup",
    feature = "neo-test-accel-evict-pressure",
    feature = "neo-test-accel-residency-retain",
    feature = "neo-test-accel-residency-demote",
    feature = "neo-test-accel-migration-reservation",
    feature = "neo-test-accel-residency-daemon",
    feature = "neo-test-accel-slot-arena",
    feature = "neo-test-accel-model-residency",
    feature = "neo-test-accel-dedup-movement",
    feature = "neo-test-accel-tensor-sharing"
))]
static STORAGE_FREES: AtomicUsize = AtomicUsize::new(0);

#[repr(C)]
#[derive(Copy, Clone)]
pub struct TensorCreateDesc {
    pub rank: u32,
    pub dtype: u32,
    pub mutable: u32,
    pub _pad: u32,
    pub dims: [u64; MAX_RANK],
    pub data_ptr: u64,
    pub data_len: u64,
}

/// Immutable tensor identity cached in a `Handle::Tensor` (BUILD-O1b,
/// ADR-043): everything a handle can answer without minting its slot.
#[derive(Copy, Clone)]
#[allow(dead_code)] // Cached handle metadata is queried through scenario/syscall surfaces.
pub struct TensorMeta {
    pub id: u64,
    pub dtype: Dtype,
    pub shape: Shape,
    pub byte_len: u64,
    pub mutable: bool,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct TensorQueryDesc {
    pub rank: u32,
    pub dtype: u32,
    pub device: u32,
    pub mutable: u32,
    pub refcount: u64,
    pub dims: [u64; MAX_RANK],
    pub byte_len: u64,
}

/// Monotonic physical-tensor identity (Phase 7 BUILD-3). Every `Tensor` allocation
/// gets a unique id at construction; it is the stable key the sharing/dedup registry
/// (`ai::sharing`) and the cross-process reader scan use to recognise *one physical
/// copy* behind N capability handles. Never recycled (a freed tensor's id is gone).
static NEXT_TENSOR_ID: AtomicU64 = AtomicU64::new(1);

fn next_tensor_id() -> u64 {
    NEXT_TENSOR_ID.fetch_add(1, Ordering::Relaxed)
}

pub struct Tensor {
    /// Stable physical identity for sharing/dedup (Phase 7 BUILD-3).
    tensor_id: u64,
    /// Storage generation (Phase 7 BUILD-P4a, §9.12 discriminator #1):
    /// incremented on every storage swap (migration, either direction). An
    /// accessor observing the same generation before and after a read is
    /// guaranteed one consistent backing storage — a mid-read advance would be
    /// a use-after-migrate, which the reservation primitive makes impossible
    /// (a referenced tensor is never claimable).
    storage_generation: AtomicU64,
    /// Set once this physical tensor is entered into the `ai::sharing` registry
    /// (explicit RO grant or content-hash dedup). Gates the `Drop` deregistration so
    /// non-shared tensors never touch the sharing/accounting locks.
    shared: AtomicBool,
    shape: Shape,
    dtype: Dtype,
    /// Storage is resident on exactly one device. Migration replaces the
    /// backing storage instead of aliasing bytes across devices.
    device: Device,
    mutable: bool,
    storage: TensorStorage,
}

pub(crate) struct HostTensorStorage {
    frame: PhysFrame<Size4KiB>,
    order: usize,
    len: usize,
}

enum TensorStorage {
    Host(HostTensorStorage),
    Device(DeviceStorage),
}

impl Tensor {
    pub fn from_desc_and_bytes(desc: TensorCreateDesc, data: &[u8]) -> Result<Self, TensorError> {
        let dtype = Dtype::from_abi(desc.dtype)?;
        let shape = Shape::new(desc.rank, desc.dims)?;
        let expected_len = validate_data_len(shape, dtype, desc.data_len)?;
        if data.len() != expected_len {
            return Err(TensorError::InvalidLength);
        }
        let mutable = match desc.mutable {
            0 => false,
            1 => true,
            _ => return Err(TensorError::InvalidShape),
        };
        Ok(Self {
            tensor_id: next_tensor_id(),
            storage_generation: AtomicU64::new(0),
            shared: AtomicBool::new(false),
            shape,
            dtype,
            device: Device::Cpu,
            mutable,
            storage: TensorStorage::Host(HostTensorStorage::allocate_from_bytes(data)?),
        })
    }

    #[allow(dead_code)]
    pub fn allocate_writable(shape: Shape, dtype: Dtype) -> Result<Self, TensorError> {
        Self::allocate_writable_on(Device::Cpu, shape, dtype)
    }

    pub fn allocate_writable_on(
        device: Device,
        shape: Shape,
        dtype: Dtype,
    ) -> Result<Self, TensorError> {
        let len = shape.byte_len(dtype)?;
        let storage = match device {
            Device::Cpu => TensorStorage::Host(HostTensorStorage::allocate_zeroed(len)?),
            Device::Accel(_) => {
                #[cfg(feature = "mem-quota")]
                refuse_if_budgeted()?;
                let driver = accel::get_device_by_id(device).ok_or(TensorError::InvalidDevice)?;
                TensorStorage::Device(
                    driver
                        .alloc(len, PAGE_SIZE)
                        .map_err(accel_to_tensor_error)?,
                )
            }
        };
        let tensor_id = next_tensor_id();
        // BUILD-P2: device-resident from birth — seed the §9.5 recency entry so
        // an eviction consult never sees an untracked resident tensor.
        if matches!(device, Device::Accel(_)) {
            accel::accounting::recency_touch(device, tensor_id);
        }
        Ok(Self {
            tensor_id,
            storage_generation: AtomicU64::new(0),
            shared: AtomicBool::new(false),
            shape,
            dtype,
            device,
            mutable: true,
            storage,
        })
    }

    pub const fn shape(&self) -> Shape {
        self.shape
    }

    pub const fn shape_ref(&self) -> &Shape {
        &self.shape
    }

    pub const fn dtype(&self) -> Dtype {
        self.dtype
    }

    #[allow(dead_code)]
    pub const fn device(&self) -> Device {
        self.device
    }

    pub fn freeze(&mut self) {
        self.mutable = false;
    }

    /// Stable physical-tensor identity (Phase 7 BUILD-3). Shared by every capability
    /// handle that references this one physical copy; the sharing registry and the
    /// cross-process reader scan key on it.
    pub fn id(&self) -> u64 {
        self.tensor_id
    }

    /// Physical byte length of the backing storage — the residency this tensor
    /// contributes (counted *once* across all sharers, ISC-4).
    pub fn byte_len(&self) -> usize {
        self.storage.len()
    }

    /// Whether this tensor's backing storage is mutable (object-level immutability,
    /// independent of capability rights). Model weights are frozen (`false`).
    #[allow(dead_code)] // syscall/scenario surface — tensor mutability probe
    pub fn is_mutable(&self) -> bool {
        self.mutable
    }

    /// Mark this physical tensor as entered in the `ai::sharing` registry, so its
    /// `Drop` deregisters it (resident_sub + registry removal). Idempotent.
    pub fn mark_shared(&self) {
        self.shared.store(true, Ordering::Release);
    }

    /// Storage generation (§9.12 discriminator #1): advances on every storage
    /// swap. Equal reads before and after a byte access prove the access saw
    /// one consistent backing storage.
    #[allow(dead_code)] // Migration discriminator exercised by gated scenarios.
    pub fn storage_generation(&self) -> u64 {
        self.storage_generation.load(Ordering::Acquire)
    }

    pub fn bytes(&self) -> &[u8] {
        self.storage.as_bytes()
    }

    /// Exclusive mutable view (OT-70): requires `&mut self`, so it is only
    /// reachable by an owner that has not yet published the tensor behind an
    /// `Arc` (e.g. a freshly allocated op output). Writers holding a shared
    /// `Arc<Tensor>` use [`Tensor::write_bytes`] instead.
    pub fn bytes_mut(&mut self) -> Result<&mut [u8], TensorError> {
        if !self.mutable {
            return Err(TensorError::PermissionDenied);
        }
        Ok(self.storage.as_bytes_mut())
    }

    /// Copy `data` into the tensor at byte `offset` through a shared borrow
    /// (OT-70). Never materialises a `&mut [u8]`, so it cannot hand out an
    /// aliasing mutable reference the way the old `bytes_mut(&self)` did.
    /// `PermissionDenied` for a frozen tensor, `InvalidLength` when the range
    /// does not fit or `data` itself points into this tensor's storage.
    ///
    /// Not yet fully sound (OT-70 residual): a `bytes()` slice of this tensor
    /// that is still live elsewhere — on this thread or another — is
    /// invalidated by the write. Callers today pass kernel-owned buffers.
    pub fn write_bytes(&self, offset: usize, data: &[u8]) -> Result<(), TensorError> {
        if !self.mutable {
            return Err(TensorError::PermissionDenied);
        }
        self.storage.write_at(offset, data)
    }

    #[allow(dead_code)]
    pub fn migrate(&mut self, target: Device) -> Result<(), TensorError> {
        if self.device == target {
            return Ok(());
        }
        match target {
            Device::Cpu => self.migrate_to_cpu(),
            Device::Accel(_) => self.migrate_to_accel(target),
        }
    }

    /// The immutable identity a `Handle::Tensor` caches (BUILD-O1b): lets
    /// metadata paths (id scans, share/grant bookkeeping) answer without
    /// minting the slot. Live state (device, refcount) is NOT here — those
    /// queries mint.
    pub fn meta(&self) -> TensorMeta {
        TensorMeta {
            id: self.id(),
            dtype: self.dtype,
            shape: self.shape,
            byte_len: self.storage.len() as u64,
            mutable: self.mutable,
        }
    }

    pub fn query_desc(&self, refcount: u64) -> TensorQueryDesc {
        TensorQueryDesc {
            rank: self.shape.rank() as u32,
            dtype: self.dtype.as_abi(),
            device: self.device.as_abi(),
            mutable: u32::from(self.mutable),
            refcount,
            dims: self.shape.dims(),
            byte_len: self.storage.len() as u64,
        }
    }

    #[allow(dead_code)]
    fn migrate_to_cpu(&mut self) -> Result<(), TensorError> {
        let TensorStorage::Device(device_storage) = &self.storage else {
            return Err(TensorError::InvalidDevice);
        };
        let driver =
            accel::get_device_by_id(device_storage.device()).ok_or(TensorError::InvalidDevice)?;
        let mut host = HostTensorStorage::allocate_zeroed(self.storage.len())?;
        driver
            .copy_from_device(host.as_bytes_mut(), device_storage)
            .map_err(accel_to_tensor_error)?;
        let old = core::mem::replace(&mut self.storage, TensorStorage::Host(host));
        if let TensorStorage::Device(storage) = old {
            driver.free(storage);
        }
        self.device = Device::Cpu;
        // BUILD-P4a (§9.12 #1): the backing storage was swapped.
        self.storage_generation.fetch_add(1, Ordering::Release);
        // BUILD-P2: no longer device-resident — drop the §9.5 recency entry.
        accel::accounting::recency_forget(self.tensor_id);
        Ok(())
    }

    #[allow(dead_code)]
    fn migrate_to_accel(&mut self, target: Device) -> Result<(), TensorError> {
        #[cfg(feature = "mem-quota")]
        refuse_if_budgeted()?;
        let driver = accel::get_device_by_id(target).ok_or(TensorError::InvalidDevice)?;
        if matches!(self.device, Device::Accel(_)) {
            self.migrate_to_cpu()?;
        }
        let mut storage = driver
            .alloc(self.storage.len(), PAGE_SIZE)
            .map_err(accel_to_tensor_error)?;
        driver
            .copy_to_device(&mut storage, self.bytes())
            .map_err(accel_to_tensor_error)?;
        self.storage = TensorStorage::Device(storage);
        self.device = target;
        // BUILD-P4a (§9.12 #1): the backing storage was swapped.
        self.storage_generation.fetch_add(1, Ordering::Release);
        // BUILD-P2: arriving on a device is a use — seed the §9.5 recency entry.
        accel::accounting::recency_touch(target, self.tensor_id);
        Ok(())
    }
}

impl Drop for Tensor {
    fn drop(&mut self) {
        // Last capability handle to this physical tensor has dropped (Arc strong count
        // reached 0). If it was shared/deduped (ISC-1/3), deregister it from the sharing
        // registry — frees the §9.5 residency attributed to its owner and removes its
        // dedup entry (ISC-4: counted once, released once). No-op for non-shared tensors
        // (the common case never touches the sharing locks).
        if self.shared.load(Ordering::Acquire) {
            crate::ai::sharing::on_tensor_drop(self.tensor_id, self.storage.len() as u64);
        }
        // BUILD-P2: a device-resident tensor's recency entry dies with it.
        if matches!(self.device, Device::Accel(_)) {
            accel::accounting::recency_forget(self.tensor_id);
        }
    }
}

/// OT-61 C2.2 (I-18): a process with a memory budget may not allocate tensor
/// storage. Tensors are buddy blocks the account does not meter yet, and many
/// paths create them (create, model load, inference outputs, the KV cache,
/// migration). One check at the storage allocators covers every one of them
/// without a list of syscalls to keep current. Kernel threads (the residency
/// daemon) are never budgeted. Metering tensors instead of refusing them is a
/// later rung.
#[cfg(feature = "mem-quota")]
fn refuse_if_budgeted() -> Result<(), TensorError> {
    if crate::sched::scheduler::current_mem_budgeted() {
        Err(TensorError::PermissionDenied)
    } else {
        Ok(())
    }
}

impl HostTensorStorage {
    fn allocate_from_bytes(data: &[u8]) -> Result<Self, TensorError> {
        #[cfg(feature = "mem-quota")]
        refuse_if_budgeted()?;
        let order = order_for_len(data.len())?;
        let Some(frame) = crate::mm::with_frame_alloc(|alloc| alloc.allocate_blocks(order)) else {
            return Err(TensorError::OutOfMemory);
        };
        let storage = Self {
            frame,
            order,
            len: data.len(),
        };
        storage.copy_from(data);
        #[cfg(any(
            feature = "neo-test-tensor-create-query-drop",
            feature = "neo-test-model-load-cascade",
            feature = "neo-test-infer-mlp-forward",
            feature = "neo-test-infer-transformer-forward",
            feature = "neo-test-inference-preemptible-liveness",
            feature = "neo-test-infer-embedding-forward",
            feature = "neo-test-ai-generate-decode",
            feature = "neo-test-accel-cpu-forward-migrate",
            feature = "neo-test-accel-sim-forward-equiv",
            feature = "neo-test-accel-sim-migrate-roundtrip",
            feature = "neo-test-accel-sim-async-poll",
            feature = "neo-test-virtio-accel-forward-equiv",
            feature = "neo-test-virtio-accel-migrate-roundtrip",
            feature = "neo-test-accel-intel-add-bringup",
            feature = "neo-test-accel-evict-pressure",
            feature = "neo-test-accel-residency-retain",
            feature = "neo-test-accel-residency-demote",
            feature = "neo-test-accel-migration-reservation",
            feature = "neo-test-accel-residency-daemon",
            feature = "neo-test-accel-tensor-sharing"
        ))]
        STORAGE_ALLOCS.fetch_add(1, Ordering::AcqRel);
        Ok(storage)
    }

    pub(crate) fn allocate_zeroed(len: usize) -> Result<Self, TensorError> {
        #[cfg(feature = "mem-quota")]
        refuse_if_budgeted()?;
        let order = order_for_len(len)?;
        let Some(frame) = crate::mm::with_frame_alloc(|alloc| alloc.allocate_blocks(order)) else {
            return Err(TensorError::OutOfMemory);
        };
        let storage = Self { frame, order, len };
        storage.zero();
        #[cfg(any(
            feature = "neo-test-tensor-create-query-drop",
            feature = "neo-test-model-load-cascade",
            feature = "neo-test-infer-mlp-forward",
            feature = "neo-test-infer-transformer-forward",
            feature = "neo-test-inference-preemptible-liveness",
            feature = "neo-test-infer-embedding-forward",
            feature = "neo-test-ai-generate-decode",
            feature = "neo-test-accel-cpu-forward-migrate",
            feature = "neo-test-accel-sim-forward-equiv",
            feature = "neo-test-accel-sim-migrate-roundtrip",
            feature = "neo-test-accel-sim-async-poll",
            feature = "neo-test-virtio-accel-forward-equiv",
            feature = "neo-test-virtio-accel-migrate-roundtrip",
            feature = "neo-test-accel-intel-add-bringup",
            feature = "neo-test-accel-evict-pressure",
            feature = "neo-test-accel-residency-retain",
            feature = "neo-test-accel-residency-demote",
            feature = "neo-test-accel-migration-reservation",
            feature = "neo-test-accel-residency-daemon",
            feature = "neo-test-accel-tensor-sharing"
        ))]
        STORAGE_ALLOCS.fetch_add(1, Ordering::AcqRel);
        Ok(storage)
    }

    pub(crate) const fn len(&self) -> usize {
        self.len
    }

    fn copy_from(&self, data: &[u8]) {
        let dst = crate::mm::phys_to_virt(self.frame.start_address()).as_mut_ptr::<u8>();
        unsafe {
            ptr::copy_nonoverlapping(data.as_ptr(), dst, data.len());
        }
    }

    fn zero(&self) {
        let dst = crate::mm::phys_to_virt(self.frame.start_address()).as_mut_ptr::<u8>();
        unsafe {
            ptr::write_bytes(dst, 0, self.len);
        }
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        let ptr = crate::mm::phys_to_virt(self.frame.start_address()).as_ptr::<u8>();
        unsafe { core::slice::from_raw_parts(ptr, self.len) }
    }

    pub(crate) fn as_bytes_mut(&mut self) -> &mut [u8] {
        // SAFETY: the frame is exclusively owned by `self` for `len` bytes, and
        // `&mut self` guarantees no other view of it is live.
        unsafe { core::slice::from_raw_parts_mut(self.as_mut_ptr(), self.len) }
    }

    /// Raw base pointer of the owned frame. Producing it from `&self` is sound;
    /// writing through it is the caller's obligation (see `TensorStorage::write_at`).
    pub(crate) fn as_mut_ptr(&self) -> *mut u8 {
        crate::mm::phys_to_virt(self.frame.start_address()).as_mut_ptr::<u8>()
    }
}

impl Drop for HostTensorStorage {
    fn drop(&mut self) {
        #[cfg(any(
            feature = "neo-test-tensor-create-query-drop",
            feature = "neo-test-model-load-cascade",
            feature = "neo-test-infer-mlp-forward",
            feature = "neo-test-infer-transformer-forward",
            feature = "neo-test-inference-preemptible-liveness",
            feature = "neo-test-infer-embedding-forward",
            feature = "neo-test-ai-generate-decode",
            feature = "neo-test-accel-cpu-forward-migrate",
            feature = "neo-test-accel-sim-forward-equiv",
            feature = "neo-test-accel-sim-migrate-roundtrip",
            feature = "neo-test-accel-sim-async-poll",
            feature = "neo-test-virtio-accel-forward-equiv",
            feature = "neo-test-virtio-accel-migrate-roundtrip",
            feature = "neo-test-accel-intel-add-bringup",
            feature = "neo-test-accel-evict-pressure",
            feature = "neo-test-accel-residency-retain",
            feature = "neo-test-accel-residency-demote",
            feature = "neo-test-accel-migration-reservation",
            feature = "neo-test-accel-residency-daemon",
            feature = "neo-test-accel-tensor-sharing"
        ))]
        STORAGE_FREES.fetch_add(1, Ordering::AcqRel);
        crate::mm::with_frame_alloc(|alloc| alloc.deallocate_blocks(self.frame, self.order));
    }
}

impl TensorStorage {
    fn len(&self) -> usize {
        match self {
            Self::Host(storage) => storage.len(),
            Self::Device(storage) => storage.len(),
        }
    }

    fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Host(storage) => storage.as_bytes(),
            Self::Device(storage) => storage.as_bytes(),
        }
    }

    fn as_bytes_mut(&mut self) -> &mut [u8] {
        match self {
            Self::Host(storage) => storage.as_bytes_mut(),
            Self::Device(storage) => storage.as_bytes_mut(),
        }
    }

    fn write_at(&self, offset: usize, data: &[u8]) -> Result<(), TensorError> {
        let end = offset
            .checked_add(data.len())
            .ok_or(TensorError::InvalidLength)?;
        if end > self.len() {
            return Err(TensorError::InvalidLength);
        }
        if data.is_empty() {
            return Ok(());
        }
        let base = match self {
            Self::Host(storage) => storage.as_mut_ptr(),
            Self::Device(storage) => storage.as_mut_ptr(),
        };
        if base.is_null() {
            return Err(TensorError::InvalidDevice);
        }
        // `data` borrowed from this same storage (e.g. `t.bytes()[1..5]`) would
        // be overwritten under a live shared borrow — refuse it outright.
        let storage_start = base as usize;
        let data_start = data.as_ptr() as usize;
        if data_start < storage_start + self.len() && storage_start < data_start + data.len() {
            return Err(TensorError::InvalidLength);
        }
        // SAFETY: `offset..end` lies inside the `len()` bytes this storage owns
        // (checked above), and `data` was just shown not to overlap the storage,
        // so `copy_nonoverlapping`'s precondition holds. No `&mut [u8]` is
        // formed from `&self`. Any *other* live `bytes()` slice of a mutable
        // tensor is still invalidated by this write — the OT-70 residual
        // recorded in `docs/OPEN-THREADS.md`.
        unsafe {
            ptr::copy_nonoverlapping(data.as_ptr(), base.add(offset), data.len());
        }
        Ok(())
    }
}

fn accel_to_tensor_error(error: accel::AccelError) -> TensorError {
    match error {
        accel::AccelError::NoDevice | accel::AccelError::Unsupported => TensorError::InvalidDevice,
        accel::AccelError::OutOfMemory => TensorError::OutOfMemory,
        accel::AccelError::InvalidRequest | accel::AccelError::Op(_) => TensorError::InvalidLength,
    }
}

#[allow(dead_code)]
pub fn tensor_migrate(tensor: &mut Tensor, target: Device) -> Result<(), TensorError> {
    tensor.migrate(target)
}

#[cfg(any(
    feature = "neo-test-tensor-create-query-drop",
    feature = "neo-test-model-load-cascade",
    feature = "neo-test-infer-mlp-forward",
    feature = "neo-test-infer-transformer-forward",
    feature = "neo-test-inference-preemptible-liveness",
    feature = "neo-test-infer-embedding-forward",
    feature = "neo-test-ai-generate-decode",
    feature = "neo-test-accel-cpu-forward-migrate",
    feature = "neo-test-accel-sim-forward-equiv",
    feature = "neo-test-accel-sim-migrate-roundtrip",
    feature = "neo-test-accel-sim-async-poll",
    feature = "neo-test-virtio-accel-forward-equiv",
    feature = "neo-test-virtio-accel-migrate-roundtrip",
    feature = "neo-test-accel-intel-add-bringup",
    feature = "neo-test-accel-evict-pressure",
    feature = "neo-test-accel-residency-retain",
    feature = "neo-test-accel-residency-demote",
    feature = "neo-test-accel-migration-reservation",
    feature = "neo-test-accel-residency-daemon",
    feature = "neo-test-accel-slot-arena",
    feature = "neo-test-accel-model-residency",
    feature = "neo-test-accel-dedup-movement",
    feature = "neo-test-accel-tensor-sharing"
))]
pub fn debug_storage_counts() -> (usize, usize) {
    (
        STORAGE_ALLOCS.load(Ordering::Acquire),
        STORAGE_FREES.load(Ordering::Acquire),
    )
}
