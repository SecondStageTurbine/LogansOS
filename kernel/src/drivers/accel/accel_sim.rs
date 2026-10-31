// SPDX-License-Identifier: MPL-2.0
//! In-kernel simulated accelerator with arena-backed memory and lazy async completion.

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::ptr;
#[cfg(feature = "neo-test")]
use core::sync::atomic::{AtomicBool, Ordering};
use core::time::Duration;
use spin::Lazy;

use crate::ai::ops::{self, OpKind, TensorView};
use crate::ai::tensor::{Device, Dtype, PAGE_SIZE, Shape};
use crate::sync::SwitchMutex;

use super::{
    AccelDescriptor, AccelDispatchKind, AccelDriver, AccelError, AccelMemoryKind, AccelResult,
    BlurDispatchMode, ComputeBindings, ComputePipeline, DeviceStorage, DispatchStatus,
    DispatchToken, OpBatch,
};

const DEVICE_ID: Device = Device::Accel(1);
const ARENA_SIZE: usize = 256 * 1024;
const MAX_ARENA_ALLOCS: usize = 128;
const MAX_PENDING: usize = 16;
const MAX_BATCH_INPUTS: usize = 4;

/// QEMU has no Gen9 EU, but it must still prove that the bound `mid` and `dst`
/// buffers are the outputs consumed by the service.  This deliberately small
/// simulator mirrors the pinned Q16 kernel arithmetic; it is not silicon
/// evidence (the Slice-4 bare-metal scenario says so in its PASS line).
fn simulate_separable_blur(
    bindings: &mut ComputeBindings<'_>,
    width: usize,
    height: usize,
    stride: usize,
    radius: usize,
    mode: BlurDispatchMode,
) -> AccelResult<()> {
    let row_bytes = width.checked_mul(4).ok_or(AccelError::InvalidRequest)?;
    let need = height
        .checked_sub(1)
        .and_then(|h| h.checked_mul(stride))
        .and_then(|body| body.checked_add(row_bytes))
        .ok_or(AccelError::InvalidRequest)?;
    if width == 0
        || height == 0
        || stride < row_bytes
        || bindings.weights.len() != radius.saturating_mul(2).saturating_add(1)
        || bindings.src.len() < need
        || bindings.mid.len() < need
        || bindings.dst.len() < need
    {
        return Err(AccelError::InvalidRequest);
    }

    fn clamp(center: usize, offset: isize, limit: usize) -> usize {
        let raw = center as isize + offset;
        if raw < 0 {
            0
        } else if raw as usize >= limit {
            limit - 1
        } else {
            raw as usize
        }
    }

    let pass_count = if mode == BlurDispatchMode::HorizontalOnly {
        1
    } else {
        2
    };
    for pass in 0..pass_count {
        let horizontal = pass == 0;
        let (src, dst): (&[u8], &mut [u8]) = if horizontal {
            (bindings.src, &mut *bindings.mid)
        } else {
            (&*bindings.mid, &mut *bindings.dst)
        };
        for y in 0..height {
            for x in 0..width {
                let out = y * stride + x * 4;
                for channel in 0..3 {
                    let mut acc = 0i32;
                    for (tap, weight) in bindings.weights.iter().copied().enumerate() {
                        let delta = tap as isize - radius as isize;
                        let sample = if horizontal {
                            y * stride + clamp(x, delta, width) * 4 + channel
                        } else {
                            clamp(y, delta, height) * stride + x * 4 + channel
                        };
                        acc += weight * src[sample] as i32;
                    }
                    dst[out + channel] = ((acc + 32_768) >> 16).clamp(0, 255) as u8;
                }
                dst[out + 3] = 0xff;
            }
        }
    }
    Ok(())
}

static SUPPORTED_OPS: &[OpKind] = &[
    OpKind::Identity,
    OpKind::Add,
    OpKind::Mul,
    OpKind::Matmul,
    OpKind::Relu,
    OpKind::Gelu,
    OpKind::Softmax,
    OpKind::LayerNorm,
    OpKind::RmsNorm,
    OpKind::Embedding,
    OpKind::Reshape,
    OpKind::Transpose,
];

static SUPPORTED_DTYPES: &[Dtype] = &[Dtype::F32, Dtype::I32];
static DRIVER: Lazy<Arc<SimAccelDriver>> = Lazy::new(|| Arc::new(SimAccelDriver));
static ARENA: SwitchMutex<ArenaState> = SwitchMutex::new(ArenaState::new());
static PENDING: SwitchMutex<PendingState> = SwitchMutex::new(PendingState::new());
#[cfg(feature = "neo-test")]
static FORCE_COMPUTE_REJECT: AtomicBool = AtomicBool::new(false);

static mut ARENA_BYTES: [u8; ARENA_SIZE] = [0; ARENA_SIZE];

pub fn init() {
    super::register_device(DRIVER.clone());
}

pub fn debug_arena_counts() -> (usize, usize) {
    let arena = ARENA.lock();
    (arena.allocs, arena.frees)
}

pub fn debug_pending_count() -> usize {
    PENDING.lock().live_count()
}

/// Scenario-only failpoint: reject a compute manifest before minting a token.
/// This proves the service's enabled-path graceful degradation without
/// pretending QEMU can reproduce a Gen9 EU hang.
#[cfg(feature = "neo-test")]
pub fn set_force_compute_reject(enabled: bool) {
    FORCE_COMPUTE_REJECT.store(enabled, Ordering::Release);
}

pub(crate) fn arena_slice(offset: usize, len: usize) -> &'static [u8] {
    if !arena_bounds_ok(offset, len) {
        return &[];
    }
    unsafe {
        // SAFETY: bounds were checked above. The simulated arena is static
        // device memory; callers only use this for debug/read views, while
        // mutating driver paths go through arena copy helpers.
        core::slice::from_raw_parts(arena_ptr().add(offset), len)
    }
}

/// Raw pointer to `len` arena bytes at `offset`, or null when out of bounds.
/// `DeviceStorage` owns the range; it forms `&mut` only from `&mut self`.
pub(crate) fn arena_storage_ptr(offset: usize, len: usize) -> *mut u8 {
    if !arena_bounds_ok(offset, len) {
        return core::ptr::null_mut();
    }
    // SAFETY: bounds were checked above, so the offset stays inside ARENA_BYTES.
    unsafe { arena_mut_ptr().add(offset) }
}

struct SimAccelDriver;

impl AccelDriver for SimAccelDriver {
    fn descriptor(&self) -> AccelDescriptor {
        AccelDescriptor {
            device: DEVICE_ID,
            driver: "accel_sim",
            name: "accel_sim0",
            memory: AccelMemoryKind::DeviceArena,
            dispatch: AccelDispatchKind::Asynchronous,
        }
    }

    fn supported_ops(&self) -> &'static [OpKind] {
        SUPPORTED_OPS
    }

    fn supported_dtypes(&self) -> &'static [Dtype] {
        SUPPORTED_DTYPES
    }

    fn alloc(&self, size: usize, align: usize) -> AccelResult<DeviceStorage> {
        if size == 0 || align == 0 || align > PAGE_SIZE || !align.is_power_of_two() {
            return Err(AccelError::InvalidRequest);
        }
        let offset = arena_alloc(size, align).ok_or(AccelError::OutOfMemory)?;
        Ok(DeviceStorage::arena(DEVICE_ID, offset, size, arena_release))
    }

    fn capacity_bytes(&self) -> Option<u64> {
        Some(ARENA_SIZE as u64)
    }

    fn free(&self, storage: DeviceStorage) {
        drop(storage);
    }

    fn copy_to_device(&self, dst: &mut DeviceStorage, src: &[u8]) -> AccelResult<()> {
        let offset = arena_storage_offset(dst)?;
        if dst.device() != DEVICE_ID || dst.len() != src.len() {
            return Err(AccelError::InvalidRequest);
        }
        arena_copy_from_host(offset, src);
        Ok(())
    }

    fn copy_from_device(&self, dst: &mut [u8], src: &DeviceStorage) -> AccelResult<()> {
        let offset = arena_storage_offset(src)?;
        if src.device() != DEVICE_ID || src.len() != dst.len() {
            return Err(AccelError::InvalidRequest);
        }
        arena_copy_to_host(dst, offset);
        Ok(())
    }

    fn dispatch(&self, batch: &mut OpBatch<'_, '_>) -> AccelResult<DispatchToken> {
        if !SUPPORTED_OPS.contains(&batch.op) || batch.inputs.len() > MAX_BATCH_INPUTS {
            return Err(AccelError::Unsupported);
        }
        for input in batch.inputs {
            if !SUPPORTED_DTYPES.contains(&input.dtype) {
                return Err(AccelError::Unsupported);
            }
        }
        let output_len = batch
            .output_shape
            .byte_len(output_dtype(batch.op, batch.inputs)?)
            .map_err(AccelError::from)?;
        if output_len != batch.output.len() {
            return Err(AccelError::InvalidRequest);
        }

        let token = super::next_dispatch_token(DEVICE_ID);
        let mut pending = PendingOp::new(
            token,
            batch.op,
            *batch.output_shape,
            batch.output.as_mut_ptr() as usize,
            batch.output.len(),
        );

        for (idx, input) in batch.inputs.iter().enumerate() {
            let offset = arena_alloc(input.bytes.len(), PAGE_SIZE).ok_or_else(|| {
                release_pending_allocs(&pending);
                AccelError::OutOfMemory
            })?;
            arena_copy_from_host(offset, input.bytes);
            pending.inputs[idx] = PendingInput {
                dtype: input.dtype,
                shape: *input.shape,
                offset,
                len: input.bytes.len(),
            };
            pending.input_count += 1;
        }

        pending.output_offset = arena_alloc(output_len, PAGE_SIZE).ok_or_else(|| {
            release_pending_allocs(&pending);
            AccelError::OutOfMemory
        })?;
        pending.output_allocated = true;

        let mut queue = PENDING.lock();
        let inserted = queue.insert(pending);
        if !inserted {
            release_pending_allocs(&pending);
            return Err(AccelError::OutOfMemory);
        }
        Ok(token)
    }

    fn submit_compute_pipeline(
        &self,
        pipeline: &ComputePipeline,
        bindings: &mut ComputeBindings<'_>,
    ) -> AccelResult<DispatchToken> {
        #[cfg(feature = "neo-test")]
        if FORCE_COMPUTE_REJECT.load(Ordering::Acquire) {
            return Err(AccelError::Unsupported);
        }
        // OT-63 GPU P1's QEMU substrate. Both pipelines exercise submit → seqno
        // token → poll/wait with no real EU work (QEMU has no Gen9 render engine).
        // `Noop` is the Slice-2 plumbing bisector. `SeparableBlur` validates the
        // pinned shape and writes the exact Q16 result into the bound buffers so
        // the service cannot pass by overwriting backend output with its floor.
        // Real batch/PPGTT/EU production remains the bare-metal Slice-4 gate.
        // Exact cross-thread/IDD/VFE/command bytes stay independently pinned by
        // the `intel_gen9_4a` host KATs; the simulator deliberately does not
        // pretend to interpret Gen9 commands.
        match pipeline {
            ComputePipeline::Noop => {
                let token = super::next_dispatch_token(DEVICE_ID);
                if !PENDING.lock().insert(PendingOp::noop(token)) {
                    return Err(AccelError::OutOfMemory);
                }
                Ok(token)
            }
            ComputePipeline::SeparableBlur {
                width,
                height,
                stride,
                radius,
                mode,
            } => {
                // ADR-077 v2 §1.4/§1.2 manifest-shape validation (the pinned r=8
                // kernel's geometry). A malformed manifest is rejected before the
                // fence — the submit cannot carry a shape the kernel's ABI can't
                // dispatch.
                // Pinned constants (ADR-077 v2 §1.2): r=8 max radius, 4 bytes per
                // XRGB8888 pixel. Inlined to keep the sim free of the atrium/blur
                // feature-gated module.
                const R8_MAX: u32 = 8;
                const BPP: u32 = 4;
                let ok = *width > 0
                    && *height > 0
                    && *radius <= R8_MAX
                    && *stride >= width.saturating_mul(BPP);
                if !ok {
                    return Err(AccelError::InvalidRequest);
                }
                simulate_separable_blur(
                    bindings,
                    *width as usize,
                    *height as usize,
                    *stride as usize,
                    *radius as usize,
                    *mode,
                )?;
                let token = super::next_dispatch_token(DEVICE_ID);
                if !PENDING.lock().insert(PendingOp::noop(token)) {
                    return Err(AccelError::OutOfMemory);
                }
                Ok(token)
            }
        }
    }

    fn poll(&self, token: DispatchToken) -> DispatchStatus {
        if token.device != DEVICE_ID {
            return DispatchStatus::Failed(AccelError::InvalidRequest);
        }
        {
            let mut queue = PENDING.lock();
            let Some(slot) = queue.find_mut(token) else {
                return DispatchStatus::Failed(AccelError::InvalidRequest);
            };
            if let Some(err) = slot.failed {
                return DispatchStatus::Failed(err);
            }
            if slot.completed {
                return DispatchStatus::Complete;
            }
            if !slot.pending_observed {
                slot.pending_observed = true;
                return DispatchStatus::Pending;
            }
            if slot.completion_started {
                return DispatchStatus::Pending;
            }
            slot.completion_started = true;
        }
        complete(token)
    }

    fn wait(&self, token: DispatchToken, _timeout: Option<Duration>) -> AccelResult<()> {
        loop {
            match self.poll(token) {
                DispatchStatus::Pending => continue,
                DispatchStatus::Complete => return Ok(()),
                DispatchStatus::Failed(err) => return Err(err),
            }
        }
    }

    fn preemptible(&self) -> bool {
        false
    }

    fn preempt(&self, _token: DispatchToken) -> AccelResult<()> {
        Err(AccelError::Unsupported)
    }
}

#[derive(Copy, Clone)]
struct ArenaSpan {
    offset: usize,
    len: usize,
    used: bool,
}

impl ArenaSpan {
    const FREE: Self = Self {
        offset: 0,
        len: 0,
        used: false,
    };

    const fn end(self) -> usize {
        self.offset + self.len
    }
}

struct ArenaState {
    spans: [ArenaSpan; MAX_ARENA_ALLOCS],
    allocs: usize,
    frees: usize,
}

impl ArenaState {
    const fn new() -> Self {
        Self {
            spans: [ArenaSpan::FREE; MAX_ARENA_ALLOCS],
            allocs: 0,
            frees: 0,
        }
    }

    fn alloc(&mut self, len: usize, align: usize) -> Option<usize> {
        let slot = self.spans.iter().position(|span| !span.used)?;
        let mut cursor = 0usize;
        loop {
            let aligned = align_up(cursor, align)?;
            let end = aligned.checked_add(len)?;
            if end > ARENA_SIZE {
                return None;
            }
            let mut advanced = false;
            for span in self.spans.iter().filter(|span| span.used) {
                if ranges_overlap(aligned, end, span.offset, span.end()) {
                    cursor = span.end();
                    advanced = true;
                    break;
                }
            }
            if !advanced {
                self.spans[slot] = ArenaSpan {
                    offset: aligned,
                    len,
                    used: true,
                };
                self.allocs = self.allocs.saturating_add(1);
                arena_zero(aligned, len);
                return Some(aligned);
            }
        }
    }

    fn free(&mut self, offset: usize, len: usize) {
        if let Some(span) = self
            .spans
            .iter_mut()
            .find(|span| span.used && span.offset == offset && span.len == len)
        {
            span.used = false;
            span.len = 0;
            self.frees = self.frees.saturating_add(1);
        }
    }
}

#[derive(Copy, Clone)]
struct PendingInput {
    dtype: Dtype,
    shape: Shape,
    offset: usize,
    len: usize,
}

impl PendingInput {
    const EMPTY: Self = Self {
        dtype: Dtype::F32,
        shape: Shape::EMPTY,
        offset: 0,
        len: 0,
    };
}

#[derive(Copy, Clone)]
struct PendingOp {
    token: DispatchToken,
    op: OpKind,
    inputs: [PendingInput; MAX_BATCH_INPUTS],
    input_count: usize,
    output_shape: Shape,
    output_offset: usize,
    output_len: usize,
    output_allocated: bool,
    output_ptr: usize,
    /// OT-63 GPU P1 Slice 2: a compute-pipeline `Noop` submit (no tensor op) —
    /// `complete_inner` retires it via the seqno path without running any op.
    noop: bool,
    pending_observed: bool,
    completion_started: bool,
    completed: bool,
    failed: Option<AccelError>,
}

impl PendingOp {
    const EMPTY: Self = Self {
        token: DispatchToken {
            device: DEVICE_ID,
            sequence: 0,
        },
        op: OpKind::Identity,
        inputs: [PendingInput::EMPTY; MAX_BATCH_INPUTS],
        input_count: 0,
        output_shape: Shape::EMPTY,
        output_offset: 0,
        output_len: 0,
        output_allocated: false,
        output_ptr: 0,
        noop: false,
        pending_observed: false,
        completion_started: false,
        completed: false,
        failed: None,
    };

    const fn new(
        token: DispatchToken,
        op: OpKind,
        output_shape: Shape,
        output_ptr: usize,
        output_len: usize,
    ) -> Self {
        Self {
            token,
            op,
            inputs: [PendingInput::EMPTY; MAX_BATCH_INPUTS],
            input_count: 0,
            output_shape,
            output_offset: 0,
            output_len,
            output_allocated: false,
            output_ptr,
            noop: false,
            pending_observed: false,
            completion_started: false,
            completed: false,
            failed: None,
        }
    }

    /// OT-63 GPU P1 Slice 2: a `ComputePipeline::Noop` pending entry — a seqno
    /// token that retires with no op and no buffers (the submit-path validation).
    const fn noop(token: DispatchToken) -> Self {
        Self {
            token,
            noop: true,
            ..Self::EMPTY
        }
    }
}

struct PendingState {
    slots: [PendingOp; MAX_PENDING],
    used: [bool; MAX_PENDING],
}

impl PendingState {
    const fn new() -> Self {
        Self {
            slots: [PendingOp::EMPTY; MAX_PENDING],
            used: [false; MAX_PENDING],
        }
    }

    fn insert(&mut self, pending: PendingOp) -> bool {
        let Some(idx) = self
            .used
            .iter()
            .zip(self.slots.iter())
            .position(|(used, slot)| !*used || slot.completed)
        else {
            return false;
        };
        self.slots[idx] = pending;
        self.used[idx] = true;
        true
    }

    fn find(&self, token: DispatchToken) -> Option<PendingOp> {
        self.used
            .iter()
            .zip(self.slots.iter())
            .find(|(used, slot)| **used && slot.token == token)
            .map(|(_, slot)| *slot)
    }

    fn find_mut(&mut self, token: DispatchToken) -> Option<&mut PendingOp> {
        self.used
            .iter()
            .zip(self.slots.iter_mut())
            .find(|(used, slot)| **used && slot.token == token)
            .map(|(_, slot)| slot)
    }

    fn live_count(&self) -> usize {
        self.used
            .iter()
            .zip(self.slots.iter())
            .filter(|(used, slot)| **used && !slot.completed)
            .count()
    }
}

fn complete(token: DispatchToken) -> DispatchStatus {
    let Some(pending) = PENDING.lock().find(token) else {
        return DispatchStatus::Failed(AccelError::InvalidRequest);
    };
    let result = complete_inner(pending);
    release_pending_allocs(&pending);

    let mut queue = PENDING.lock();
    let Some(slot) = queue.find_mut(token) else {
        return DispatchStatus::Failed(AccelError::InvalidRequest);
    };
    match result {
        Ok(()) => {
            slot.completed = true;
            slot.failed = None;
            DispatchStatus::Complete
        }
        Err(err) => {
            slot.completed = true;
            slot.failed = Some(err);
            DispatchStatus::Failed(err)
        }
    }
}

fn complete_inner(pending: PendingOp) -> AccelResult<()> {
    // OT-63 GPU P1 Slice 2: a compute-pipeline `Noop` retires with no op — the
    // seqno path (submit → poll → complete) is exactly what it validates.
    if pending.noop {
        return Ok(());
    }
    let mut input_bytes: [Vec<u8>; MAX_BATCH_INPUTS] = [(); MAX_BATCH_INPUTS].map(|()| Vec::new());
    for (idx, input) in pending.inputs[..pending.input_count].iter().enumerate() {
        let mut bytes = vec![0u8; input.len];
        arena_copy_to_host(&mut bytes, input.offset);
        input_bytes[idx] = bytes;
    }

    let mut views: [TensorView<'_>; MAX_BATCH_INPUTS] =
        [(); MAX_BATCH_INPUTS].map(|()| TensorView {
            dtype: Dtype::F32,
            shape: &Shape::EMPTY,
            bytes: &[],
        });
    for idx in 0..pending.input_count {
        views[idx] = TensorView {
            dtype: pending.inputs[idx].dtype,
            shape: &pending.inputs[idx].shape,
            bytes: &input_bytes[idx],
        };
    }

    let mut output = vec![0u8; pending.output_len];
    ops::dispatch(
        pending.op,
        &views[..pending.input_count],
        &pending.output_shape,
        &mut output,
    )
    .map_err(AccelError::Op)?;
    arena_copy_from_host(pending.output_offset, &output);
    unsafe {
        // SAFETY: `dispatch` records this pointer from the caller's output
        // slice. The AccelDriver contract requires that slice to remain live
        // and unaliased until `poll`/`wait` reports Complete or Failed; the
        // executor and tests call `wait` before the output buffer can drop.
        let dst =
            core::slice::from_raw_parts_mut(pending.output_ptr as *mut u8, pending.output_len);
        dst.copy_from_slice(&output);
    }
    Ok(())
}

fn release_pending_allocs(pending: &PendingOp) {
    for input in pending.inputs[..pending.input_count].iter() {
        arena_release(input.offset, input.len);
    }
    if pending.output_allocated {
        arena_release(pending.output_offset, pending.output_len);
    }
}

fn output_dtype(op: OpKind, inputs: &[TensorView<'_>]) -> AccelResult<Dtype> {
    match op {
        OpKind::Embedding => Ok(Dtype::F32),
        OpKind::Add | OpKind::Mul | OpKind::Matmul => Ok(Dtype::F32),
        OpKind::Identity
        | OpKind::Relu
        | OpKind::Gelu
        | OpKind::Softmax
        | OpKind::LayerNorm
        | OpKind::RmsNorm
        | OpKind::Reshape
        | OpKind::Transpose => inputs
            .first()
            .map(|input| input.dtype)
            .ok_or(AccelError::InvalidRequest),
        OpKind::Silu | OpKind::Swiglu | OpKind::Rope | OpKind::GqaAttention => {
            Err(AccelError::InvalidRequest)
        }
    }
}

fn arena_storage_offset(storage: &DeviceStorage) -> AccelResult<usize> {
    storage
        .arena_offset()
        .filter(|_| storage.device() == DEVICE_ID)
        .ok_or(AccelError::InvalidRequest)
}

fn arena_alloc(len: usize, align: usize) -> Option<usize> {
    ARENA.lock().alloc(len, align)
}

fn arena_release(offset: usize, len: usize) {
    ARENA.lock().free(offset, len);
}

fn arena_copy_from_host(offset: usize, src: &[u8]) {
    if src.is_empty() {
        return;
    }
    unsafe {
        // SAFETY: all callers use offsets returned by `arena_alloc` with
        // matching lengths, so the destination range is inside ARENA_BYTES.
        ptr::copy_nonoverlapping(src.as_ptr(), arena_mut_ptr().add(offset), src.len());
    }
}

fn arena_copy_to_host(dst: &mut [u8], offset: usize) {
    if dst.is_empty() {
        return;
    }
    unsafe {
        // SAFETY: all callers use offsets returned by `arena_alloc` with
        // matching lengths, so the source range is inside ARENA_BYTES.
        ptr::copy_nonoverlapping(arena_ptr().add(offset), dst.as_mut_ptr(), dst.len());
    }
}

fn arena_zero(offset: usize, len: usize) {
    if len == 0 {
        return;
    }
    unsafe {
        // SAFETY: `ArenaState::alloc` checks the allocated range is fully
        // inside ARENA_BYTES before calling this helper.
        ptr::write_bytes(arena_mut_ptr().add(offset), 0, len);
    }
}

fn arena_bounds_ok(offset: usize, len: usize) -> bool {
    offset.checked_add(len).is_some_and(|end| end <= ARENA_SIZE)
}

fn arena_ptr() -> *const u8 {
    core::ptr::addr_of!(ARENA_BYTES).cast::<u8>()
}

fn arena_mut_ptr() -> *mut u8 {
    core::ptr::addr_of_mut!(ARENA_BYTES).cast::<u8>()
}

fn align_up(value: usize, align: usize) -> Option<usize> {
    let mask = align.checked_sub(1)?;
    value.checked_add(mask).map(|value| value & !mask)
}

fn ranges_overlap(a_start: usize, a_end: usize, b_start: usize, b_end: usize) -> bool {
    a_start < b_end && b_start < a_end
}
