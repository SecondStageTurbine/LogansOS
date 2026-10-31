// SPDX-License-Identifier: MPL-2.0
//! ANSPEC-001 accelerator driver class (§9.1–§9.3; accounting/recency §9.5).
//!
//! The registry mirrors the block-device registry: probes register concrete
//! drivers, while AI execution and tensor migration look them up by DeviceId.

extern crate alloc;

pub mod accel_cpu;
#[cfg(feature = "accel-intel")]
pub mod accel_intel;
#[cfg(feature = "accel-intel")]
pub mod accel_intel_ids;
#[cfg(feature = "accel-sim")]
pub mod accel_sim;
pub mod accounting;
#[cfg(feature = "atrium-compositor-g1")]
pub mod gpu_compute;
#[cfg(feature = "accel-intel")]
pub mod intel_gen9_4a;
#[cfg(feature = "accel-intel")]
pub mod intel_gen9_execlist;
#[cfg(feature = "accel-intel")]
pub mod intel_gen9_ppgtt;
#[cfg(feature = "virtio-accel")]
pub mod virtio_accel;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::time::Duration;
use spin::Lazy;

use crate::ai::ops::{OpError, OpKind, TensorView};
use crate::ai::tensor::{Device, Dtype, HostTensorStorage, Shape, TensorError};
#[cfg(any(feature = "virtio-accel", feature = "accel-intel"))]
use crate::drivers::block::DmaBuffer;
use crate::sched::process::ProcessId;
use crate::serial_println;
use crate::sync::SwitchMutex;

pub type DeviceId = Device;

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum AccelError {
    NoDevice,
    Unsupported,
    InvalidRequest,
    OutOfMemory,
    Op(OpError),
}

pub type AccelResult<T> = Result<T, AccelError>;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum AccelMemoryKind {
    HostRam,
    DeviceArena,
    DmaBuffer,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum AccelDispatchKind {
    Synchronous,
    Asynchronous,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct AccelDescriptor {
    pub device: DeviceId,
    pub driver: &'static str,
    pub name: &'static str,
    pub memory: AccelMemoryKind,
    pub dispatch: AccelDispatchKind,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct DispatchToken {
    pub device: DeviceId,
    pub sequence: u64,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum DispatchStatus {
    Pending,
    Complete,
    Failed(AccelError),
}

/// OT-63 GPU P1 Slices 2–4 — a **manifest-only, kernel-internal** compute pipeline
/// submitted through [`AccelDriver::submit_compute_pipeline`]. This is NOT a
/// tensor [`OpKind`] (which is the syscall-reachable tensor-execution ABI) — the
/// compute pipeline is a *fixed, pinned* manifest with no userland-supplied
/// kernel, exactly per ADR-077 v2 amendment 4 (the pre-VT-d service is wholly
/// kernel-internal; there is no `create_pipeline(user_bytes)`).
///
/// Slice 2 introduced [`ComputePipeline::Noop`] for the submit/queue/fence
/// plumbing. Slice 4 binds real buffers to [`ComputePipeline::SeparableBlur`]:
/// `accel-sim` mirrors its Q16 output for QEMU wiring evidence, while
/// `accel-intel` submits the pinned ocloc blob through a 48-bit-PPGTT ELSP/LRC
/// context. Only the separate bare-metal gate can prove real EU execution.
#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum BlurDispatchMode {
    /// Dispatch Pass H only and retire a fence, allowing a non-perturbing
    /// `mid` readback in a separate diagnostic run.
    HorizontalOnly,
    /// Dispatch H -> PIPE_CONTROL -> V without a CPU touch between the passes.
    TwoPass,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ComputePipeline {
    /// The Slice-2 validation pipeline: submit → seqno fence → complete, with no
    /// EU work. Proves the compute submit path end to end without a real kernel.
    Noop,
    /// The Slice-3/4 blur pipeline (separable Gaussian over a linear XRGB8888
    /// buffer). Fields are checked pinned geometry; `mode` selects either the
    /// non-perturbing H-only diagnostic or the untouched H→barrier→V batch.
    SeparableBlur {
        width: u32,
        height: u32,
        stride: u32,
        radius: u32,
        mode: BlurDispatchMode,
    },
}

/// Host-visible storage bound to a kernel-internal compute manifest.
///
/// The manifest deliberately contains no user-controlled pointers.  The trusted
/// caller supplies ordinary kernel slices here; a hardware driver stages them in
/// its fenced GPU working set and copies `mid`/`dst` back only after the completion
/// fence retires.  Keeping the bindings next to the submit is also what makes the
/// Slice-4 result falsifiable: a successful backend must produce the bytes rather
/// than letting [`gpu_compute`](self::gpu_compute) overwrite them with its CPU
/// floor after the fact.
pub struct ComputeBindings<'a> {
    pub src: &'a [u8],
    pub mid: &'a mut [u8],
    pub dst: &'a mut [u8],
    pub weights: &'a [i32],
}

pub struct OpBatch<'a, 'out> {
    pub op: OpKind,
    pub inputs: &'a [TensorView<'a>],
    pub output_shape: &'a Shape,
    pub output: &'out mut [u8],
}

pub struct DeviceStorage {
    device: DeviceId,
    len: usize,
    /// The principal this device residency is accounted to (Phase 7 BUILD-1).
    /// Captured at alloc time so the matching free decrements the same principal
    /// regardless of which process drops the storage.
    principal: ProcessId,
    backing: DeviceStorageBacking,
}

enum DeviceStorageBacking {
    Host(HostTensorStorage),
    Arena(DeviceArenaStorage),
    #[cfg(any(feature = "virtio-accel", feature = "accel-intel"))]
    Dma(Arc<DmaBuffer>),
}

struct DeviceArenaStorage {
    offset: usize,
    release: fn(usize, usize),
}

impl DeviceStorage {
    /// Capture the owning principal and record `+len` residency for it. Called by
    /// every backing constructor so accounting is uniform across all backends.
    fn account_alloc(device: DeviceId, len: usize) -> ProcessId {
        let principal = accounting::current_principal(device);
        accounting::resident_add(device, principal, len as u64);
        principal
    }

    pub(crate) fn host(device: DeviceId, storage: HostTensorStorage, _align: usize) -> Self {
        let len = storage.len();
        Self {
            device,
            len,
            principal: Self::account_alloc(device, len),
            backing: DeviceStorageBacking::Host(storage),
        }
    }

    pub(crate) fn arena(
        device: DeviceId,
        offset: usize,
        len: usize,
        release: fn(usize, usize),
    ) -> Self {
        Self {
            device,
            len,
            principal: Self::account_alloc(device, len),
            backing: DeviceStorageBacking::Arena(DeviceArenaStorage { offset, release }),
        }
    }

    #[cfg(any(feature = "virtio-accel", feature = "accel-intel"))]
    pub(crate) fn dma(device: DeviceId, buffer: Arc<DmaBuffer>) -> Self {
        let len = buffer.len();
        Self {
            device,
            len,
            principal: Self::account_alloc(device, len),
            backing: DeviceStorageBacking::Dma(buffer),
        }
    }

    pub const fn device(&self) -> DeviceId {
        self.device
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        match &self.backing {
            DeviceStorageBacking::Host(storage) => storage.as_bytes(),
            DeviceStorageBacking::Arena(storage) => {
                #[cfg(feature = "accel-sim")]
                {
                    accel_sim::arena_slice(storage.offset, self.len)
                }
                #[cfg(not(feature = "accel-sim"))]
                {
                    let _ = storage;
                    &[]
                }
            }
            #[cfg(any(feature = "virtio-accel", feature = "accel-intel"))]
            DeviceStorageBacking::Dma(buffer) => unsafe { buffer.as_slice() },
        }
    }

    /// Exclusive mutable view of the backing (OT-70): `&mut self` replaces the
    /// former `&self -> &mut [u8]` accessor and its `mut_from_ref` allow.
    pub(crate) fn as_bytes_mut(&mut self) -> &mut [u8] {
        let ptr = self.as_mut_ptr();
        if ptr.is_null() {
            return &mut [];
        }
        // SAFETY: `ptr` addresses the `len` bytes this storage exclusively owns
        // (host frame, bounds-checked sim arena range, or owned DMA buffer), and
        // `&mut self` guarantees no other view of them is live.
        unsafe { core::slice::from_raw_parts_mut(ptr, self.len) }
    }

    /// Raw base pointer of the backing, or null when the backing has no
    /// addressable bytes (an arena storage without `accel-sim`, or an
    /// out-of-bounds arena range).
    pub(crate) fn as_mut_ptr(&self) -> *mut u8 {
        match &self.backing {
            DeviceStorageBacking::Host(storage) => storage.as_mut_ptr(),
            DeviceStorageBacking::Arena(storage) => {
                #[cfg(feature = "accel-sim")]
                {
                    accel_sim::arena_storage_ptr(storage.offset, self.len)
                }
                #[cfg(not(feature = "accel-sim"))]
                {
                    let _ = storage;
                    core::ptr::null_mut()
                }
            }
            #[cfg(any(feature = "virtio-accel", feature = "accel-intel"))]
            DeviceStorageBacking::Dma(buffer) => buffer.as_mut_ptr(),
        }
    }

    pub(crate) fn arena_offset(&self) -> Option<usize> {
        match &self.backing {
            DeviceStorageBacking::Host(_) => None,
            DeviceStorageBacking::Arena(storage) => Some(storage.offset),
            #[cfg(any(feature = "virtio-accel", feature = "accel-intel"))]
            DeviceStorageBacking::Dma(_) => None,
        }
    }
}

impl Drop for DeviceStorage {
    fn drop(&mut self) {
        // Residency `-len` for the owning principal, on free or implicit drop
        // (every backend's `free` just drops the storage — see accel_cpu/accel_sim).
        accounting::resident_sub(self.device, self.principal, self.len as u64);
        if let DeviceStorageBacking::Arena(storage) = &self.backing {
            (storage.release)(storage.offset, self.len);
        }
    }
}

#[allow(dead_code)]
pub trait AccelDriver: Send + Sync {
    fn descriptor(&self) -> AccelDescriptor;
    fn supported_ops(&self) -> &'static [OpKind];
    fn supported_dtypes(&self) -> &'static [Dtype];
    fn alloc(&self, size: usize, align: usize) -> AccelResult<DeviceStorage>;
    /// Total device-memory bytes available for resident tensor storage, when the
    /// device is capacity-bounded (e.g. accel_sim's fixed arena). `None` ⇒
    /// unbounded for placement purposes — eviction (Phase 7 BUILD-P2, ANSPEC-001
    /// §9.10) never engages and behavior is exactly the §9.9 path.
    fn capacity_bytes(&self) -> Option<u64> {
        None
    }
    fn free(&self, storage: DeviceStorage);
    fn copy_to_device(&self, dst: &mut DeviceStorage, src: &[u8]) -> AccelResult<()>;
    fn copy_from_device(&self, dst: &mut [u8], src: &DeviceStorage) -> AccelResult<()>;
    fn dispatch(&self, batch: &mut OpBatch<'_, '_>) -> AccelResult<DispatchToken>;
    /// OT-63 GPU P1 Slice 2 — submit a kernel-internal compute pipeline (§ADR-077
    /// v2 §2.2), returning the same [`DispatchToken`] seqno fence [`poll`]/[`wait`]
    /// consume. Completion is a **fence**, never a present: this path writes a
    /// destination buffer and stops — it does not, and must not, reach scanout
    /// (`present`/`arm_flip`/`PLANE_SURF`) — the compositor alone is capped for
    /// that (amendment 4 / the `GpuSubmit` present authority). Default: the
    /// driver does not implement compute (`Unsupported`); `accel_sim` implements
    /// both variants for QEMU wiring, while `accel_intel` implements the real
    /// Slice-4 EU dispatch.
    ///
    /// [`poll`]: AccelDriver::poll
    /// [`wait`]: AccelDriver::wait
    fn submit_compute_pipeline(
        &self,
        _pipeline: &ComputePipeline,
        _bindings: &mut ComputeBindings<'_>,
    ) -> AccelResult<DispatchToken> {
        Err(AccelError::Unsupported)
    }
    fn poll(&self, token: DispatchToken) -> DispatchStatus;
    fn wait(&self, token: DispatchToken, timeout: Option<Duration>) -> AccelResult<()>;
    fn preemptible(&self) -> bool;
    fn preempt(&self, token: DispatchToken) -> AccelResult<()>;
}

struct Registry {
    devices: Vec<Arc<dyn AccelDriver>>,
    next_sequence: u64,
}

static REGISTRY: Lazy<SwitchMutex<Registry>> = Lazy::new(|| {
    SwitchMutex::new(Registry {
        devices: Vec::new(),
        next_sequence: 1,
    })
});

pub fn init() {
    accel_cpu::init();
    #[cfg(feature = "accel-sim")]
    accel_sim::init();
    #[cfg(feature = "virtio-accel")]
    virtio_accel::init();
    #[cfg(feature = "accel-intel")]
    accel_intel::init();
    serial_println!(
        "[ kernel ] accel: init complete attached_devices={}",
        device_count()
    );
}

pub fn register_device(device: Arc<dyn AccelDriver>) -> usize {
    let mut registry = REGISTRY.lock();
    if let Some((idx, _)) = registry
        .devices
        .iter()
        .enumerate()
        .find(|(_, existing)| existing.descriptor().device == device.descriptor().device)
    {
        return idx;
    }
    let idx = registry.devices.len();
    let descriptor = device.descriptor();
    serial_println!(
        "[ kernel ] accel: registered device index={} driver={} name={} device=0x{:08x}",
        idx,
        descriptor.driver,
        descriptor.name,
        descriptor.device.as_abi()
    );
    registry.devices.push(device);
    idx
}

#[allow(dead_code)]
pub fn get_device(index: usize) -> Option<Arc<dyn AccelDriver>> {
    REGISTRY.lock().devices.get(index).cloned()
}

pub fn get_device_by_id(device: DeviceId) -> Option<Arc<dyn AccelDriver>> {
    REGISTRY
        .lock()
        .devices
        .iter()
        .find(|driver| driver.descriptor().device == device)
        .cloned()
}

pub fn find_device_by_driver(driver: &str) -> Option<(usize, Arc<dyn AccelDriver>)> {
    REGISTRY
        .lock()
        .devices
        .iter()
        .enumerate()
        .find(|(_, device)| device.descriptor().driver == driver)
        .map(|(index, device)| (index, device.clone()))
}

pub fn default_driver() -> Option<Arc<dyn AccelDriver>> {
    find_device_by_driver("accel_cpu").map(|(_, driver)| driver)
}

#[allow(dead_code)]
pub fn default_device() -> Option<DeviceId> {
    default_driver().map(|driver| driver.descriptor().device)
}

pub fn device_count() -> usize {
    REGISTRY.lock().devices.len()
}

pub(crate) fn next_dispatch_token(device: DeviceId) -> DispatchToken {
    let mut registry = REGISTRY.lock();
    let sequence = registry.next_sequence;
    registry.next_sequence = registry.next_sequence.saturating_add(1);
    DispatchToken { device, sequence }
}

impl From<TensorError> for AccelError {
    fn from(value: TensorError) -> Self {
        match value {
            TensorError::OutOfMemory => Self::OutOfMemory,
            TensorError::InvalidDevice => Self::NoDevice,
            TensorError::InvalidShape
            | TensorError::InvalidDtype
            | TensorError::InvalidLength
            | TensorError::PermissionDenied => Self::InvalidRequest,
        }
    }
}
