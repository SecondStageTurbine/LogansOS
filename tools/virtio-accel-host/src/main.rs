// SPDX-License-Identifier: MPL-2.0
//! ADR-031 BUILD-3 vhost-user backend for virtio-accel.

use std::env;
use std::fs;
use std::io::{Error, ErrorKind, Result};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use vhost::vhost_user::message::{VhostUserProtocolFeatures, VhostUserVirtioFeatures};
use vhost_user_backend::{VhostUserBackendMut, VhostUserDaemon, VringMutex, VringT};
use virtio_queue::QueueT;
use virtio_queue::desc::split::Descriptor;
use vm_memory::{Bytes, GuestAddress, GuestAddressSpace, GuestMemoryAtomic, GuestMemoryMmap};
use vmm_sys_util::epoll::EventSet;

#[allow(dead_code)]
#[path = "../../../kernel/src/ai/ops/mod.rs"]
mod ops;
#[allow(dead_code)]
#[path = "../../../kernel/src/ai/tensor_logic.rs"]
mod tensor_logic;

use ops::{OpError, OpKind, TensorView};
use tensor_logic::{Dtype, MAX_RANK, Shape, TensorError};

const VIRTIO_F_VERSION_1: u64 = 1 << 32;
const QUEUE_SIZE: usize = 8;
const MAX_INPUTS: usize = 3;
const MAX_TENSOR_BYTES: u64 = 1 << 20;
const DTYPE_F32: u32 = 0;
const DTYPE_I32: u32 = 5;
const STATUS_OK: u32 = 0;
const STATUS_UNSUPPORTED_OP: u32 = 1;
const STATUS_UNSUPPORTED_DTYPE: u32 = 2;
const STATUS_BAD_REQUEST: u32 = 3;
const STATUS_SHAPE_MISMATCH: u32 = 4;
const STATUS_INTERNAL: u32 = 5;

const REQ_HEADER_LEN: usize = 16;
const TENSOR_DESC_LEN: usize = 80;
const STATUS_LEN: usize = 16;
#[cfg(test)]
const IDENTITY_DESC_BLOCK_LEN: usize = REQ_HEADER_LEN + TENSOR_DESC_LEN * 2;
#[cfg(test)]
const ADD_DESC_BLOCK_LEN: usize = REQ_HEADER_LEN + TENSOR_DESC_LEN * 3;
#[cfg(test)]
const IDENTITY_INPUT_BYTES: [u8; 8] = [0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x00, 0x40];
#[cfg(test)]
const IDENTITY_STATUS_ORACLE: [u8; STATUS_LEN] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
#[cfg(test)]
const ADD_LHS_BYTES: [u8; 8] = [0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x00, 0x40];
#[cfg(test)]
const ADD_RHS_BYTES: [u8; 8] = [0x00, 0x00, 0x40, 0x40, 0x00, 0x00, 0x80, 0x40];
#[cfg(test)]
const ADD_OUTPUT_ORACLE: [u8; 8] = [0x00, 0x00, 0x80, 0x40, 0x00, 0x00, 0xc0, 0x40];
#[cfg(test)]
const ADD_STATUS_ORACLE: [u8; STATUS_LEN] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
#[cfg(test)]
const IDENTITY_DESC_BLOCK_ORACLE: [u8; IDENTITY_DESC_BLOCK_LEN] = [
    // VirtioAccelReqHeader { op_kind: Identity, input_count: 1, flags: 0, reserved: 0 }
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // input VirtioAccelTensorDesc { dtype: F32, rank: 1, dims: [2], byte_len: 8 }
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // output VirtioAccelTensorDesc { dtype: F32, rank: 1, dims: [2], byte_len: 8 }
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
#[cfg(test)]
const ADD_DESC_BLOCK_ORACLE: [u8; ADD_DESC_BLOCK_LEN] = [
    // VirtioAccelReqHeader { op_kind: Add, input_count: 2, flags: 0, reserved: 0 }
    0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // input0 VirtioAccelTensorDesc
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // input1 VirtioAccelTensorDesc
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // output VirtioAccelTensorDesc
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

type AtomicGuestMemory = GuestMemoryAtomic<GuestMemoryMmap>;

#[derive(Default)]
struct AccelBackend {
    mem: Option<AtomicGuestMemory>,
    event_idx: bool,
    acked_features: u64,
}

impl VhostUserBackendMut for AccelBackend {
    type Bitmap = ();
    type Vring = VringMutex;

    fn num_queues(&self) -> usize {
        1
    }

    fn max_queue_size(&self) -> usize {
        QUEUE_SIZE
    }

    fn features(&self) -> u64 {
        VIRTIO_F_VERSION_1 | VhostUserVirtioFeatures::PROTOCOL_FEATURES.bits()
    }

    fn acked_features(&mut self, features: u64) {
        self.acked_features = features;
    }

    fn protocol_features(&self) -> VhostUserProtocolFeatures {
        VhostUserProtocolFeatures::CONFIG
    }

    fn reset_device(&mut self) {
        self.event_idx = false;
        self.acked_features = 0;
    }

    fn set_event_idx(&mut self, enabled: bool) {
        self.event_idx = enabled;
    }

    fn get_config(&self, offset: u32, size: u32) -> Vec<u8> {
        let config = config_bytes();
        let start = offset as usize;
        let end = start.saturating_add(size as usize);
        if start >= config.len() || end > config.len() {
            return Vec::new();
        }
        config[start..end].to_vec()
    }

    fn update_memory(&mut self, mem: AtomicGuestMemory) -> Result<()> {
        self.mem = Some(mem);
        Ok(())
    }

    fn handle_event(
        &mut self,
        device_event: u16,
        _evset: EventSet,
        vrings: &[Self::Vring],
        _thread_id: usize,
    ) -> Result<()> {
        if device_event != 0 {
            return Ok(());
        }
        let Some(mem) = self.mem.as_ref() else {
            return Err(Error::new(
                ErrorKind::NotConnected,
                "guest memory not configured",
            ));
        };
        let mem = mem.memory();
        let mut vring_state = vrings[0].get_mut();

        while let Some(chain) = vring_state
            .get_queue_mut()
            .pop_descriptor_chain(mem.clone())
        {
            let head = chain.head_index();
            let status = process_chain(chain)?;
            vring_state
                .add_used(head, status.used_len)
                .map_err(|err| Error::new(ErrorKind::InvalidData, err))?;
            let notify = if self.event_idx {
                vring_state
                    .needs_notification()
                    .map_err(|err| Error::new(ErrorKind::InvalidData, err))?
            } else {
                true
            };
            if notify {
                vring_state.signal_used_queue()?;
            }
        }
        Ok(())
    }
}

struct ChainStatus {
    used_len: u32,
}

fn process_chain<M>(chain: virtio_queue::DescriptorChain<M>) -> Result<ChainStatus>
where
    M: Clone + Deref,
    M::Target: vm_memory::GuestMemory,
{
    let descs: Vec<_> = chain.clone().collect();
    let mem = chain.memory();
    if descs.len() < 4 || descs[0].is_write_only() {
        return Ok(ChainStatus { used_len: 0 });
    }
    let mut desc_block = vec![0u8; descs[0].len() as usize];
    mem.read_slice(&mut desc_block, descs[0].addr())
        .map_err(|err| Error::new(ErrorKind::InvalidData, err))?;

    let input_count = request_input_count(&desc_block);
    let output_desc = input_count.saturating_add(1);
    let status_desc = input_count.saturating_add(2);
    if input_count == 0
        || input_count > MAX_INPUTS
        || descs.len() != input_count + 3
        || output_desc >= descs.len()
        || status_desc >= descs.len()
        || descs[output_desc].len() as u64 > MAX_TENSOR_BYTES
        || (descs[status_desc].len() as usize) < STATUS_LEN
        || !descs[output_desc].is_write_only()
        || !descs[status_desc].is_write_only()
    {
        return write_chain_status(mem, &descs, status_desc, STATUS_BAD_REQUEST, 0);
    }
    for desc in descs.iter().take(input_count + 1).skip(1) {
        if desc.is_write_only() || desc.len() as u64 > MAX_TENSOR_BYTES {
            return write_chain_status(mem, &descs, status_desc, STATUS_BAD_REQUEST, 0);
        }
    }

    let mut inputs = Vec::with_capacity(input_count);
    for desc in descs.iter().take(input_count + 1).skip(1) {
        let mut input = vec![0u8; desc.len() as usize];
        mem.read_slice(&mut input, desc.addr())
            .map_err(|err| Error::new(ErrorKind::InvalidData, err))?;
        inputs.push(input);
    }

    let execution = execute_request(&desc_block, &inputs);
    if execution.status == STATUS_OK {
        if (descs[output_desc].len() as usize) < execution.output.len() {
            return write_chain_status(mem, &descs, status_desc, STATUS_BAD_REQUEST, 0);
        }
        mem.write_slice(&execution.output, descs[output_desc].addr())
            .map_err(|err| Error::new(ErrorKind::InvalidData, err))?;
    }
    write_chain_status(
        mem,
        &descs,
        status_desc,
        execution.status,
        execution.output.len() as u64,
    )
}

struct RequestExecution {
    status: u32,
    output: Vec<u8>,
}

#[derive(Copy, Clone)]
struct TensorDesc {
    dtype: Dtype,
    shape: Shape,
    byte_len: usize,
}

fn execute_request(desc_block: &[u8], input_buffers: &[Vec<u8>]) -> RequestExecution {
    let request = match parse_request(desc_block, input_buffers) {
        Ok(request) => request,
        Err(status) => {
            return RequestExecution {
                status,
                output: Vec::new(),
            };
        }
    };

    let mut output = vec![0u8; request.output.byte_len];
    let views: Vec<_> = request
        .inputs
        .iter()
        .zip(input_buffers.iter())
        .map(|(input, bytes)| TensorView {
            dtype: input.dtype,
            shape: &input.shape,
            bytes,
        })
        .collect();
    match ops::dispatch(request.op, &views, &request.output.shape, &mut output) {
        Ok(()) => RequestExecution {
            status: STATUS_OK,
            output,
        },
        Err(error) => RequestExecution {
            status: op_error_status(error),
            output: Vec::new(),
        },
    }
}

struct ParsedRequest {
    op: OpKind,
    inputs: Vec<TensorDesc>,
    output: TensorDesc,
}

fn parse_request(
    desc_block: &[u8],
    input_buffers: &[Vec<u8>],
) -> std::result::Result<ParsedRequest, u32> {
    if desc_block.len() < REQ_HEADER_LEN {
        return Err(STATUS_BAD_REQUEST);
    }
    let op_raw = read_le_u32(desc_block, 0).ok_or(STATUS_BAD_REQUEST)?;
    let input_count = read_le_u32(desc_block, 4).ok_or(STATUS_BAD_REQUEST)? as usize;
    let flags = read_le_u32(desc_block, 8).ok_or(STATUS_BAD_REQUEST)?;
    let reserved = read_le_u32(desc_block, 12).ok_or(STATUS_BAD_REQUEST)?;
    if flags != 0 || reserved != 0 || input_count == 0 || input_count > MAX_INPUTS {
        return Err(STATUS_BAD_REQUEST);
    }
    if input_buffers.len() != input_count
        || desc_block.len() != REQ_HEADER_LEN + TENSOR_DESC_LEN * (input_count + 1)
    {
        return Err(STATUS_BAD_REQUEST);
    }
    let op = OpKind::from_abi(op_raw).ok_or(STATUS_UNSUPPORTED_OP)?;
    if !supported_op(op) {
        return Err(STATUS_UNSUPPORTED_OP);
    }

    let mut inputs = Vec::with_capacity(input_count);
    for (idx, input_buffer) in input_buffers.iter().enumerate().take(input_count) {
        let tensor = parse_tensor_desc(desc_block, REQ_HEADER_LEN + idx * TENSOR_DESC_LEN)?;
        if tensor.byte_len as u64 > MAX_TENSOR_BYTES || input_buffer.len() != tensor.byte_len {
            return Err(STATUS_BAD_REQUEST);
        }
        inputs.push(tensor);
    }
    let output = parse_tensor_desc(desc_block, REQ_HEADER_LEN + input_count * TENSOR_DESC_LEN)?;
    if output.byte_len as u64 > MAX_TENSOR_BYTES {
        return Err(STATUS_BAD_REQUEST);
    }
    if output_dtype(op, &inputs)? != output.dtype {
        return Err(STATUS_BAD_REQUEST);
    }
    Ok(ParsedRequest { op, inputs, output })
}

fn parse_tensor_desc(desc_block: &[u8], base: usize) -> std::result::Result<TensorDesc, u32> {
    let dtype = dtype_from_wire(read_le_u32(desc_block, base).ok_or(STATUS_BAD_REQUEST)?)?;
    let rank = read_le_u32(desc_block, base + 4).ok_or(STATUS_BAD_REQUEST)?;
    let mut dims = [0u64; MAX_RANK];
    for (idx, dim) in dims.iter_mut().enumerate() {
        *dim = read_le_u64(desc_block, base + 8 + idx * 8).ok_or(STATUS_BAD_REQUEST)?;
    }
    let byte_len_u64 = read_le_u64(desc_block, base + 72).ok_or(STATUS_BAD_REQUEST)?;
    let byte_len = usize::try_from(byte_len_u64).map_err(|_| STATUS_BAD_REQUEST)?;
    let shape = Shape::new(rank, dims).map_err(tensor_error_status)?;
    let expected = shape.byte_len(dtype).map_err(tensor_error_status)?;
    if expected != byte_len {
        return Err(STATUS_BAD_REQUEST);
    }
    Ok(TensorDesc {
        dtype,
        shape,
        byte_len,
    })
}

fn request_input_count(desc_block: &[u8]) -> usize {
    if desc_block.len() < 8 {
        return 0;
    }
    read_le_u32(desc_block, 4).unwrap_or(0) as usize
}

fn write_chain_status<M>(
    mem: &M,
    descs: &[Descriptor],
    status_desc: usize,
    code: u32,
    output_len: u64,
) -> Result<ChainStatus>
where
    M: vm_memory::GuestMemory + ?Sized,
{
    if let Some(desc) = descs.get(status_desc).or_else(|| descs.last())
        && desc.is_write_only()
    {
        write_status(mem, desc.addr(), desc.len(), code, output_len)?;
    }
    let used_len = if code == STATUS_OK {
        output_len.saturating_add(STATUS_LEN as u64)
    } else {
        STATUS_LEN as u64
    };
    Ok(ChainStatus {
        used_len: used_len.min(u32::MAX as u64) as u32,
    })
}

fn write_status<M>(mem: &M, addr: GuestAddress, len: u32, code: u32, output_len: u64) -> Result<()>
where
    M: vm_memory::GuestMemory + ?Sized,
{
    if (len as usize) < STATUS_LEN {
        return Ok(());
    }
    let mut bytes = [0u8; STATUS_LEN];
    bytes[0..4].copy_from_slice(&code.to_le_bytes());
    bytes[8..16].copy_from_slice(&output_len.to_le_bytes());
    mem.write_slice(&bytes, addr)
        .map_err(|err| Error::new(ErrorKind::InvalidData, err))
}

fn supported_op(op: OpKind) -> bool {
    matches!(
        op,
        OpKind::Identity
            | OpKind::Add
            | OpKind::Mul
            | OpKind::Matmul
            | OpKind::Relu
            | OpKind::Gelu
            | OpKind::Softmax
            | OpKind::LayerNorm
            | OpKind::RmsNorm
            | OpKind::Embedding
            | OpKind::Reshape
            | OpKind::Transpose
            | OpKind::Silu
            | OpKind::Swiglu
            | OpKind::Rope
            | OpKind::GqaAttention
    )
}

fn supported_ops_mask() -> u32 {
    let mut mask = 0u32;
    for op in [
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
        OpKind::Silu,
        OpKind::Swiglu,
        OpKind::Rope,
        OpKind::GqaAttention,
    ] {
        mask |= 1u32 << (op as u32);
    }
    mask
}

fn dtype_from_wire(dtype: u32) -> std::result::Result<Dtype, u32> {
    match dtype {
        DTYPE_F32 => Ok(Dtype::F32),
        DTYPE_I32 => Ok(Dtype::I32),
        _ => Err(STATUS_UNSUPPORTED_DTYPE),
    }
}

fn output_dtype(op: OpKind, inputs: &[TensorDesc]) -> std::result::Result<Dtype, u32> {
    match op {
        OpKind::Identity
        | OpKind::Reshape
        | OpKind::Transpose
        | OpKind::Relu
        | OpKind::Gelu
        | OpKind::Softmax
        | OpKind::Silu => {
            if inputs.len() == 1 {
                Ok(inputs[0].dtype)
            } else {
                Err(STATUS_BAD_REQUEST)
            }
        }
        OpKind::Add | OpKind::Mul | OpKind::Matmul | OpKind::Embedding | OpKind::Swiglu => {
            if inputs.len() == 2 {
                Ok(Dtype::F32)
            } else {
                Err(STATUS_BAD_REQUEST)
            }
        }
        OpKind::Rope => {
            if inputs.len() == 2 {
                Ok(inputs[0].dtype)
            } else {
                Err(STATUS_BAD_REQUEST)
            }
        }
        OpKind::GqaAttention => {
            if inputs.len() == 4 {
                Ok(inputs[0].dtype)
            } else {
                Err(STATUS_BAD_REQUEST)
            }
        }
        OpKind::LayerNorm => {
            if inputs.len() == 1 || inputs.len() == 3 {
                Ok(inputs[0].dtype)
            } else {
                Err(STATUS_BAD_REQUEST)
            }
        }
        OpKind::RmsNorm => {
            if inputs.len() == 1 || inputs.len() == 2 {
                Ok(inputs[0].dtype)
            } else {
                Err(STATUS_BAD_REQUEST)
            }
        }
    }
}

fn op_error_status(error: OpError) -> u32 {
    match error {
        OpError::Dtype => STATUS_UNSUPPORTED_DTYPE,
        OpError::Shape | OpError::Output => STATUS_SHAPE_MISMATCH,
        OpError::Input | OpError::Index => STATUS_BAD_REQUEST,
    }
}

fn tensor_error_status(error: TensorError) -> u32 {
    match error {
        TensorError::InvalidDtype => STATUS_UNSUPPORTED_DTYPE,
        TensorError::InvalidShape | TensorError::InvalidLength => STATUS_BAD_REQUEST,
        TensorError::InvalidDevice | TensorError::PermissionDenied | TensorError::OutOfMemory => {
            STATUS_INTERNAL
        }
    }
}

fn read_le_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let value = bytes.get(offset..end)?;
    Some(u32::from_le_bytes(value.try_into().ok()?))
}

fn read_le_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let end = offset.checked_add(8)?;
    let value = bytes.get(offset..end)?;
    Some(u64::from_le_bytes(value.try_into().ok()?))
}

fn config_bytes() -> [u8; 24] {
    let mut bytes = [0u8; 24];
    let supported_ops = supported_ops_mask();
    let supported_dtypes = (1u32 << DTYPE_F32) | (1u32 << DTYPE_I32);
    bytes[0..4].copy_from_slice(&supported_ops.to_le_bytes());
    bytes[4..8].copy_from_slice(&supported_dtypes.to_le_bytes());
    bytes[8..12].copy_from_slice(&(MAX_INPUTS as u32).to_le_bytes());
    bytes[12..16].copy_from_slice(&(QUEUE_SIZE as u32).to_le_bytes());
    bytes[16..24].copy_from_slice(&MAX_TENSOR_BYTES.to_le_bytes());
    bytes
}

fn parse_socket_arg() -> Result<PathBuf> {
    let mut args = env::args().skip(1);
    let mut socket = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--socket" => {
                let Some(value) = args.next() else {
                    return Err(Error::new(ErrorKind::InvalidInput, "--socket needs a path"));
                };
                socket = Some(PathBuf::from(value));
            }
            "--help" | "-h" => {
                println!("usage: virtio-accel-host --socket <path>");
                std::process::exit(0);
            }
            other => {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!("unknown argument `{other}`"),
                ));
            }
        }
    }
    socket.ok_or_else(|| Error::new(ErrorKind::InvalidInput, "missing --socket <path>"))
}

fn remove_stale_socket(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn main() -> Result<()> {
    let socket = parse_socket_arg()?;
    if let Some(parent) = socket.parent() {
        fs::create_dir_all(parent)?;
    }
    remove_stale_socket(&socket)?;

    let mem = GuestMemoryAtomic::new(
        GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x1000)]).map_err(Error::other)?,
    );
    let backend = Arc::new(Mutex::new(AccelBackend::default()));
    let mut daemon = VhostUserDaemon::new("virtio-accel-host".to_owned(), backend, mem)
        .map_err(|err| Error::other(err.to_string()))?;
    daemon
        .serve(&socket)
        .map_err(|err| Error::other(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_request_oracle_is_byte_exact() {
        let expected: [u8; IDENTITY_DESC_BLOCK_LEN] = [
            0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        assert_eq!(IDENTITY_DESC_BLOCK_ORACLE, expected);
    }

    #[test]
    fn add_request_and_data_oracles_are_byte_exact() {
        assert_eq!(ADD_DESC_BLOCK_ORACLE.len(), 256);
        assert_eq!(
            ADD_DESC_BLOCK_ORACLE[0..16],
            [1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            ADD_LHS_BYTES,
            [0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x00, 0x40]
        );
        assert_eq!(
            ADD_RHS_BYTES,
            [0x00, 0x00, 0x40, 0x40, 0x00, 0x00, 0x80, 0x40]
        );
        assert_eq!(
            ADD_OUTPUT_ORACLE,
            [0x00, 0x00, 0x80, 0x40, 0x00, 0x00, 0xc0, 0x40]
        );
        assert_eq!(ADD_STATUS_ORACLE, IDENTITY_STATUS_ORACLE);
    }

    #[test]
    fn daemon_computes_add_with_shared_kernel_oracle() {
        let inputs = vec![ADD_LHS_BYTES.to_vec(), ADD_RHS_BYTES.to_vec()];
        let execution = execute_request(&ADD_DESC_BLOCK_ORACLE, &inputs);

        assert_eq!(execution.status, STATUS_OK);
        assert_eq!(execution.output, ADD_OUTPUT_ORACLE);
        assert_eq!(
            status_bytes(execution.status, execution.output.len() as u64),
            ADD_STATUS_ORACLE
        );
    }

    #[test]
    fn identity_buffer_and_status_oracles_are_byte_exact() {
        assert_eq!(
            IDENTITY_INPUT_BYTES,
            [0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x00, 0x40]
        );
        assert_eq!(
            IDENTITY_STATUS_ORACLE,
            [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00,
            ]
        );
    }

    #[test]
    fn config_bytes_advertise_full_3b_contract() {
        assert_eq!(
            config_bytes(),
            [
                0xff, 0xff, 0x00, 0x00, 0x21, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x08, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00,
            ]
        );
    }

    fn status_bytes(code: u32, output_len: u64) -> [u8; STATUS_LEN] {
        let mut bytes = [0u8; STATUS_LEN];
        bytes[0..4].copy_from_slice(&code.to_le_bytes());
        bytes[8..16].copy_from_slice(&output_len.to_le_bytes());
        bytes
    }
}
