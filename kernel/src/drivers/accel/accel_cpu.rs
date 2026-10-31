// SPDX-License-Identifier: MPL-2.0
//! Synchronous host-RAM reference accelerator.

extern crate alloc;

use alloc::sync::Arc;
use core::time::Duration;
use spin::Lazy;

use crate::ai::ops::{self, OpKind};
use crate::ai::tensor::{Device, Dtype, HostTensorStorage, PAGE_SIZE};

use super::{
    AccelDescriptor, AccelDispatchKind, AccelDriver, AccelError, AccelMemoryKind, AccelResult,
    DeviceStorage, DispatchStatus, DispatchToken, OpBatch,
};

const DEVICE_ID: Device = Device::Accel(0);

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

static DRIVER: Lazy<Arc<CpuAccelDriver>> = Lazy::new(|| Arc::new(CpuAccelDriver));

pub fn init() {
    super::register_device(DRIVER.clone());
}

struct CpuAccelDriver;

impl AccelDriver for CpuAccelDriver {
    fn descriptor(&self) -> AccelDescriptor {
        AccelDescriptor {
            device: DEVICE_ID,
            driver: "accel_cpu",
            name: "accel_cpu0",
            memory: AccelMemoryKind::HostRam,
            dispatch: AccelDispatchKind::Synchronous,
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
        Ok(DeviceStorage::host(
            DEVICE_ID,
            HostTensorStorage::allocate_zeroed(size).map_err(AccelError::from)?,
            align,
        ))
    }

    fn free(&self, storage: DeviceStorage) {
        drop(storage);
    }

    fn copy_to_device(&self, dst: &mut DeviceStorage, src: &[u8]) -> AccelResult<()> {
        if dst.device() != DEVICE_ID || dst.len() != src.len() {
            return Err(AccelError::InvalidRequest);
        }
        dst.as_bytes_mut().copy_from_slice(src);
        Ok(())
    }

    fn copy_from_device(&self, dst: &mut [u8], src: &DeviceStorage) -> AccelResult<()> {
        if src.device() != DEVICE_ID || src.len() != dst.len() {
            return Err(AccelError::InvalidRequest);
        }
        dst.copy_from_slice(src.as_bytes());
        Ok(())
    }

    fn dispatch(&self, batch: &mut OpBatch<'_, '_>) -> AccelResult<DispatchToken> {
        dispatch_cpu_op(batch)?;
        Ok(super::next_dispatch_token(DEVICE_ID))
    }

    fn poll(&self, token: DispatchToken) -> DispatchStatus {
        if token.device == DEVICE_ID {
            DispatchStatus::Complete
        } else {
            DispatchStatus::Failed(AccelError::InvalidRequest)
        }
    }

    fn wait(&self, token: DispatchToken, _timeout: Option<Duration>) -> AccelResult<()> {
        match self.poll(token) {
            DispatchStatus::Complete => Ok(()),
            DispatchStatus::Pending => Err(AccelError::InvalidRequest),
            DispatchStatus::Failed(err) => Err(err),
        }
    }

    fn preemptible(&self) -> bool {
        false
    }

    fn preempt(&self, _token: DispatchToken) -> AccelResult<()> {
        Err(AccelError::Unsupported)
    }
}

#[cfg(feature = "inference-preemptible")]
fn dispatch_cpu_op(batch: &mut OpBatch<'_, '_>) -> AccelResult<()> {
    #[cfg(feature = "neo-test-inference-preemptible-liveness")]
    if !crate::arch::x86_64::syscall::inference_preemptible_dispatch_enabled_for_test() {
        ops::dispatch(batch.op, batch.inputs, batch.output_shape, batch.output)
            .map_err(AccelError::Op)?;
        return Ok(());
    }

    crate::arch::x86_64::with_interrupts_enabled(|| {
        #[cfg(feature = "neo-test-inference-preemptible-liveness")]
        {
            const MAX_LIVENESS_REPLAYS: usize = 100_000;
            let mut replay = 0usize;
            loop {
                let probe =
                    crate::arch::x86_64::syscall::begin_inference_preemptible_dispatch_probe();
                ops::dispatch(batch.op, batch.inputs, batch.output_shape, batch.output)
                    .map_err(AccelError::Op)?;
                if crate::arch::x86_64::syscall::finish_inference_preemptible_dispatch_probe(probe)
                    || replay >= MAX_LIVENESS_REPLAYS
                {
                    break;
                }
                replay += 1;
            }
            Ok(())
        }

        #[cfg(not(feature = "neo-test-inference-preemptible-liveness"))]
        {
            ops::dispatch(batch.op, batch.inputs, batch.output_shape, batch.output)
                .map_err(AccelError::Op)
        }
    })
}

#[cfg(not(feature = "inference-preemptible"))]
fn dispatch_cpu_op(batch: &mut OpBatch<'_, '_>) -> AccelResult<()> {
    ops::dispatch(batch.op, batch.inputs, batch.output_shape, batch.output).map_err(AccelError::Op)
}
