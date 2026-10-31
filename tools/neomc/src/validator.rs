// SPDX-License-Identifier: MPL-2.0
//! ADR-028 D2 — `.neom` v1 self-validator.
//! Mirrors the kernel's load-time validation rules. Returns `Result`, never
//! panics on bad input.

use crate::format::{
    CONFIG_LAYER_COUNT_OFF, CONFIG_ROPE_DIMENSION_COUNT_OFF, Dtype, HEADER_SIZE, HEADER_SIZE_V1,
    HEADER_SIZE_V3, HEADER_SIZE_V4, IO_ENTRY_SIZE, LAYER_DESCRIPTOR_KIND_OFF,
    LAYER_DESCRIPTOR_RESERVED_OFF, LAYER_DESCRIPTOR_SIZE, LayerKind, NODE_ENTRY_SIZE, NeomError,
    NeomResult, OpCode, PAGE_SIZE, SCHEDULE_LAYER_COUNT_OFF, SCHEDULE_MAX_LAYERS, SCHEDULE_OFF_OFF,
    SCHEDULE_VERSION, SCHEDULE_VERSION_OFF, TENSOR_ENTRY_SIZE, read_u32_le, read_u64_le,
    shape_element_count,
};

const TABLE_ALIGN: usize = 8;

/// Validate a `.neom` v1 byte buffer against ADR-028 D2 rules.
pub fn validate(data: &[u8]) -> NeomResult<()> {
    validate_header(data)?;
    validate_table_bounds(data)?;
    validate_tensors(data)?;
    validate_nodes(data)?;
    validate_refs(data)?;
    validate_dag(data)?;
    validate_io(data)?;
    validate_schedule(data)?;
    Ok(())
}

// ── 1. Header ────────────────────────────────────────────────────────────────

fn validate_header(data: &[u8]) -> NeomResult<()> {
    if data.len() < HEADER_SIZE_V1 as usize {
        return Err(NeomError::InvalidMagic);
    }

    let magic = &data[0..4];
    if magic != b"NEOM" {
        return Err(NeomError::InvalidMagic);
    }

    let version = read_u32_le(data, 4);
    if version != 1 && version != 2 && version != 3 && version != 4 {
        return Err(NeomError::InvalidVersion(version));
    }

    let header_size = read_u32_le(data, 8);
    match (version, header_size) {
        (1, HEADER_SIZE_V1) => {}
        (2, HEADER_SIZE) => {}
        (3, HEADER_SIZE_V3) => {}
        (4, HEADER_SIZE_V4) => {}
        _ => return Err(NeomError::BadHeaderSize(header_size)),
    }

    if data.len() < header_size as usize {
        return Err(NeomError::InvalidMagic);
    }

    if version == 4 {
        let rope_dimensions = read_u32_le(data, CONFIG_ROPE_DIMENSION_COUNT_OFF);
        if rope_dimensions == 0 || !rope_dimensions.is_multiple_of(2) {
            return Err(NeomError::InvalidSchedule(
                "rope_dimension_count must be non-zero and even",
            ));
        }
    }

    let flags = read_u32_le(data, 12);
    if flags != 0 {
        return Err(NeomError::NonZeroFlags(flags));
    }

    let total_size = read_u64_le(data, 88);
    if total_size as usize != data.len() {
        return Err(NeomError::TotalSizeMismatch {
            header: total_size,
            file: data.len() as u64,
        });
    }

    Ok(())
}

// ── 2. Table bounds ─────────────────────────────────────────────────────────

fn validate_table_bounds(data: &[u8]) -> NeomResult<()> {
    let total_size = read_u64_le(data, 88) as usize;
    let tensor_count = read_u32_le(data, 16) as usize;
    let node_count = read_u32_le(data, 20) as usize;
    let input_count = read_u32_le(data, 24) as usize;
    let output_count = read_u32_le(data, 28) as usize;
    let io_count = input_count + output_count;

    let header_size = read_u32_le(data, 8) as usize;
    let tensor_table_off = read_u64_le(data, 32) as usize;
    let node_table_off = read_u64_le(data, 40) as usize;
    let io_table_off = read_u64_le(data, 48) as usize;
    let string_off = read_u64_le(data, 56) as usize;
    let string_len = read_u64_le(data, 64) as usize;
    let weight_blob_off = read_u64_le(data, 72);
    let weight_blob_len = read_u64_le(data, 80) as usize;

    // Tables must not overlap header
    if tensor_table_off < header_size {
        return Err(NeomError::TableOverflow {
            table: "tensor_table",
        });
    }
    if !tensor_table_off.is_multiple_of(TABLE_ALIGN)
        || !node_table_off.is_multiple_of(TABLE_ALIGN)
        || !io_table_off.is_multiple_of(TABLE_ALIGN)
        || !string_off.is_multiple_of(TABLE_ALIGN)
    {
        return Err(NeomError::TableOverflow {
            table: "unaligned_table",
        });
    }

    // Tensor table within file
    let Some(tensor_end) = tensor_table_off.checked_add(tensor_count * TENSOR_ENTRY_SIZE as usize)
    else {
        return Err(NeomError::TableOverflow {
            table: "tensor_table",
        });
    };
    if tensor_end > total_size {
        return Err(NeomError::TableOverflow {
            table: "tensor_table",
        });
    }

    // Node table within file
    let node_entry_len = node_count * NODE_ENTRY_SIZE as usize;
    // We need to account for value refs too, but the ref arrays are part of
    // the node table region. We check the node entries first.
    let Some(node_end) = node_table_off.checked_add(node_entry_len) else {
        return Err(NeomError::TableOverflow {
            table: "node_table",
        });
    };
    if node_end > total_size {
        return Err(NeomError::TableOverflow {
            table: "node_table",
        });
    }

    // IO table within file
    let Some(io_end) = io_table_off.checked_add(io_count * IO_ENTRY_SIZE as usize) else {
        return Err(NeomError::TableOverflow { table: "io_table" });
    };
    if io_end > total_size {
        return Err(NeomError::TableOverflow { table: "io_table" });
    }

    // String heap within file
    let Some(string_end) = string_off.checked_add(string_len) else {
        return Err(NeomError::TableOverflow {
            table: "string_heap",
        });
    };
    if string_end > total_size {
        return Err(NeomError::TableOverflow {
            table: "string_heap",
        });
    }

    // Weight blob page-aligned
    if !weight_blob_off.is_multiple_of(PAGE_SIZE as u64) {
        return Err(NeomError::WeightBlobNotPageAligned(weight_blob_off));
    }

    // Weight blob within file
    let Some(blob_end) = (weight_blob_off as usize).checked_add(weight_blob_len) else {
        return Err(NeomError::TableOverflow {
            table: "weight_blob",
        });
    };
    if blob_end > total_size {
        return Err(NeomError::TableOverflow {
            table: "weight_blob",
        });
    }

    Ok(())
}

// ── 3. Tensor integrity ─────────────────────────────────────────────────────

fn validate_tensors(data: &[u8]) -> NeomResult<()> {
    let tensor_count = read_u32_le(data, 16) as usize;
    let tensor_table_off = read_u64_le(data, 32) as usize;
    let weight_blob_off = read_u64_le(data, 72) as usize;
    let weight_blob_len = read_u64_le(data, 80) as usize;

    for i in 0..tensor_count {
        let base = tensor_table_off + i * TENSOR_ENTRY_SIZE as usize;

        let dtype_val = read_u32_le(data, base);
        Dtype::from_abi(dtype_val).map_err(|_| NeomError::InvalidDtype(dtype_val))?;
        let dtype = unsafe { std::mem::transmute::<u32, Dtype>(dtype_val) };

        let rank = read_u32_le(data, base + 4);
        if rank > 8 {
            return Err(NeomError::RankTooLarge(rank));
        }

        let mut dims = [0u64; 8];
        for (d, slot) in dims.iter_mut().enumerate() {
            *slot = read_u64_le(data, base + 8 + d * 8);
        }

        // Check dims non-zero
        for (d, &dim) in dims.iter().enumerate().take(rank as usize) {
            if dim == 0 {
                return Err(NeomError::ZeroDimension {
                    tensor_idx: i,
                    dim_idx: d,
                });
            }
        }

        let weight_off = read_u64_le(data, base + 72);
        let weight_len = read_u64_le(data, base + 80);

        // weight_len == product(dims) * dtype.size_bytes() (check before range)
        let expected = shape_element_count(rank as usize, &dims)? * dtype.size_bytes();
        if weight_len != expected {
            return Err(NeomError::WeightLenMismatch {
                tensor_idx: i,
                expected,
                actual: weight_len,
            });
        }

        // weight_off must be 64-aligned (absolute file offset)
        if !weight_off.is_multiple_of(64) {
            return Err(NeomError::WeightOffNot64Aligned {
                tensor_idx: i,
                off: weight_off,
            });
        }

        // weight range within blob
        let rel_off = (weight_off - weight_blob_off as u64) as usize;
        if (weight_off < weight_blob_off as u64)
            || (rel_off + weight_len as usize > weight_blob_len)
        {
            return Err(NeomError::WeightRangeOutsideBlob { tensor_idx: i });
        }

        // name offset into string heap
        let name_off = read_u32_le(data, base + 88);
        let name_len = read_u32_le(data, base + 92);
        check_string_range(data, name_off, name_len)?;
    }

    Ok(())
}

// ── 4. Op support ────────────────────────────────────────────────────────────

fn validate_nodes(data: &[u8]) -> NeomResult<()> {
    let node_count = read_u32_le(data, 20) as usize;
    let node_table_off = read_u64_le(data, 40) as usize;

    for i in 0..node_count {
        let base = node_table_off + i * NODE_ENTRY_SIZE as usize;

        let op_val = read_u32_le(data, base);
        OpCode::from_abi(op_val).map_err(|_| NeomError::UnknownOpCode(op_val))?;

        // Check reserved field is zero (offset 0x24, 12 bytes)
        for b in 0..12 {
            if data[base + 0x24 + b] != 0 {
                return Err(NeomError::NodeReservedNonZero { node_idx: i });
            }
        }

        // name/attr string offsets
        let attr_off = read_u32_le(data, base + 20);
        let attr_len = read_u32_le(data, base + 24);
        if attr_len > 0 {
            check_string_range(data, attr_off, attr_len)?;
        }

        let name_off = read_u32_le(data, base + 28);
        let name_len = read_u32_le(data, base + 32);
        check_string_range(data, name_off, name_len)?;
    }

    Ok(())
}

// ── 5. Ref validity ─────────────────────────────────────────────────────────

fn validate_refs(data: &[u8]) -> NeomResult<()> {
    let tensor_count = read_u32_le(data, 16);
    let node_count = read_u32_le(data, 20) as usize;
    let input_count = read_u32_le(data, 24) as usize;
    let output_count = read_u32_le(data, 28) as usize;
    let io_count = input_count + output_count;
    let node_table_off = read_u64_le(data, 40) as usize;
    let io_table_off = read_u64_le(data, 48) as usize;

    // Build set of valid graph values: IO-declared values + all node-produced values
    let mut declared_values: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for i in 0..io_count {
        let base = io_table_off + i * IO_ENTRY_SIZE as usize;
        let vi = read_u32_le(data, base);
        declared_values.insert(vi);
    }

    // First pass: collect all values produced by nodes
    for i in 0..node_count {
        let base = node_table_off + i * NODE_ENTRY_SIZE as usize;
        let n_output_count = read_u32_le(data, base + 8) as usize;
        let outputs_off = read_u32_le(data, base + 16) as usize;
        for j in 0..n_output_count {
            let val = read_u32_le(data, outputs_off + j * 4);
            declared_values.insert(val);
        }
    }

    // Check ref arrays and validate value refs
    let node_entry_end = node_table_off + node_count * NODE_ENTRY_SIZE as usize;
    let mut max_ref_end = node_entry_end;

    for i in 0..node_count {
        let base = node_table_off + i * NODE_ENTRY_SIZE as usize;
        let n_input_count = read_u32_le(data, base + 4) as usize;
        let n_output_count = read_u32_le(data, base + 8) as usize;

        let inputs_off = read_u32_le(data, base + 12) as usize;
        let outputs_off = read_u32_le(data, base + 16) as usize;

        for j in 0..n_input_count {
            if inputs_off + j * 4 + 4 > data.len() {
                return Err(NeomError::NodeIoRefOverflow { node_idx: i });
            }
            let val = read_u32_le(data, inputs_off + j * 4);
            check_value_ref(val, tensor_count, &declared_values)?;
        }
        if n_input_count > 0 {
            let end = inputs_off + n_input_count * 4;
            if end > max_ref_end {
                max_ref_end = end;
            }
        }

        for j in 0..n_output_count {
            if outputs_off + j * 4 + 4 > data.len() {
                return Err(NeomError::NodeIoRefOverflow { node_idx: i });
            }
            let val = read_u32_le(data, outputs_off + j * 4);
            check_value_ref(val, tensor_count, &declared_values)?;
        }
        if n_output_count > 0 {
            let end = outputs_off + n_output_count * 4;
            if end > max_ref_end {
                max_ref_end = end;
            }
        }
    }

    if max_ref_end > data.len() {
        return Err(NeomError::NodeIoRefOverflow { node_idx: 0 });
    }

    Ok(())
}

fn check_value_ref(
    val: u32,
    tensor_count: u32,
    declared: &std::collections::HashSet<u32>,
) -> NeomResult<()> {
    if val < tensor_count {
        // Weight tensor ref — always valid
        return Ok(());
    }
    // Must be a declared IO value or produced by some node
    if !declared.contains(&val) {
        return Err(NeomError::InputOutputRefOutOfRange { value_index: val });
    }
    Ok(())
}

// ── 6. DAG check (topological sort) ─────────────────────────────────────────

fn validate_dag(data: &[u8]) -> NeomResult<()> {
    let node_count = read_u32_le(data, 20) as usize;
    if node_count == 0 {
        return Ok(());
    }

    let node_table_off = read_u64_le(data, 40) as usize;

    // Build adjacency: for each node, its outputs are "produced" and its inputs
    // are "consumed". Edge: if node A produces value V and node B consumes V,
    // then A -> B.
    // We track in-degree for Kahn's algorithm.

    // Map value_index -> producing node index
    let mut producers: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    let mut node_inputs: Vec<Vec<u32>> = Vec::with_capacity(node_count);

    for i in 0..node_count {
        let base = node_table_off + i * NODE_ENTRY_SIZE as usize;
        let n_input_count = read_u32_le(data, base + 4) as usize;
        let n_output_count = read_u32_le(data, base + 8) as usize;
        let inputs_off = read_u32_le(data, base + 12) as usize;
        let outputs_off = read_u32_le(data, base + 16) as usize;

        let mut inputs = Vec::with_capacity(n_input_count);
        for j in 0..n_input_count {
            inputs.push(read_u32_le(data, inputs_off + j * 4));
        }
        node_inputs.push(inputs);

        for j in 0..n_output_count {
            let val = read_u32_le(data, outputs_off + j * 4);
            producers.insert(val, i);
        }
    }

    // Build in-degree and adjacency list
    let mut in_degree = vec![0usize; node_count];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); node_count];

    for (i, inputs) in node_inputs.iter().enumerate() {
        for &val in inputs {
            if let Some(&producer) = producers.get(&val)
                && producer != i
            {
                adj[producer].push(i);
                in_degree[i] += 1;
            }
        }
    }

    // Kahn's algorithm
    let mut queue: Vec<usize> = (0..node_count).filter(|&i| in_degree[i] == 0).collect();
    let mut visited = 0usize;

    while let Some(node) = queue.pop() {
        visited += 1;
        for &neighbor in &adj[node] {
            in_degree[neighbor] -= 1;
            if in_degree[neighbor] == 0 {
                queue.push(neighbor);
            }
        }
    }

    if visited != node_count {
        return Err(NeomError::CyclicGraph);
    }

    Ok(())
}

// ── 7. IO validation ────────────────────────────────────────────────────────

fn validate_io(data: &[u8]) -> NeomResult<()> {
    let input_count = read_u32_le(data, 24) as usize;
    let output_count = read_u32_le(data, 28) as usize;
    let io_count = input_count + output_count;
    let io_table_off = read_u64_le(data, 48) as usize;

    for i in 0..io_count {
        let base = io_table_off + i * IO_ENTRY_SIZE as usize;

        let value_index = read_u32_le(data, base);
        // value_index should be a valid graph value (>= tensor_count for intermediates/IO)
        let tensor_count = read_u32_le(data, 16);
        if value_index < tensor_count {
            return Err(NeomError::IoEntryValueIndexOutOfRange {
                entry_idx: i,
                value_index,
            });
        }

        let dtype_val = read_u32_le(data, base + 4);
        Dtype::from_abi(dtype_val).map_err(|_| NeomError::IoDtypeInvalid(dtype_val))?;

        let rank = read_u32_le(data, base + 8);
        if rank > 8 {
            return Err(NeomError::IoRankTooLarge(rank));
        }

        let is_output = read_u32_le(data, base + 12);
        if is_output != 0 && is_output != 1 {
            return Err(NeomError::IoIsOutputBadValue(is_output));
        }

        let mut dims = [0u64; 8];
        for (d, slot) in dims.iter_mut().enumerate() {
            *slot = read_u64_le(data, base + 16 + d * 8);
        }

        for &dim in dims.iter().take(rank as usize) {
            if dim == 0 {
                return Err(NeomError::IoZeroDimension { entry_idx: i });
            }
        }
    }

    Ok(())
}

// ── 8. v3 schedule validation ────────────────────────────────────────────────

fn validate_schedule(data: &[u8]) -> NeomResult<()> {
    let version = read_u32_le(data, 4);
    if version != 3 && version != 4 {
        return Ok(());
    }

    let layer_count = read_u32_le(data, SCHEDULE_LAYER_COUNT_OFF);
    if layer_count == 0 || layer_count > SCHEDULE_MAX_LAYERS {
        return Err(NeomError::InvalidSchedule("layer_count out of bounds"));
    }
    if layer_count != read_u32_le(data, CONFIG_LAYER_COUNT_OFF) {
        return Err(NeomError::InvalidSchedule(
            "schedule layer_count does not match config layer_count",
        ));
    }
    if read_u32_le(data, SCHEDULE_VERSION_OFF) != SCHEDULE_VERSION {
        return Err(NeomError::InvalidSchedule("unsupported schedule version"));
    }

    let table_off = read_u64_le(data, SCHEDULE_OFF_OFF) as usize;
    let total_size = read_u64_le(data, 88) as usize;
    let header_size = read_u32_le(data, 8) as usize;
    if table_off < header_size || !table_off.is_multiple_of(TABLE_ALIGN) {
        return Err(NeomError::InvalidSchedule(
            "schedule table offset is before header or unaligned",
        ));
    }
    let table_len = (layer_count as usize)
        .checked_mul(LAYER_DESCRIPTOR_SIZE as usize)
        .ok_or(NeomError::InvalidSchedule("schedule table length overflow"))?;
    let table_end = table_off
        .checked_add(table_len)
        .ok_or(NeomError::InvalidSchedule("schedule table end overflow"))?;
    if table_end > total_size {
        return Err(NeomError::TableOverflow { table: "schedule" });
    }

    for idx in 0..layer_count as usize {
        let base = table_off + idx * LAYER_DESCRIPTOR_SIZE as usize;
        let kind = LayerKind::from_abi(read_u32_le(data, base + LAYER_DESCRIPTOR_KIND_OFF))?;
        let config_ref = read_u32_le(data, base + LAYER_DESCRIPTOR_RESERVED_OFF);
        if kind == LayerKind::Qwen2AttentionFFN && config_ref != 0 {
            return Err(NeomError::InvalidSchedule(
                "Qwen2AttentionFFN config_ref must be 0",
            ));
        }
    }

    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn check_string_range(data: &[u8], off: u32, len: u32) -> NeomResult<()> {
    // Offsets are relative to string_off (ADR-028: "names are (off,len) into it")
    let _string_off = read_u64_le(data, 56);
    let string_len = read_u64_le(data, 64);

    if off as u64 + len as u64 > string_len {
        return Err(NeomError::StringOffsetOutOfRange { off, len });
    }

    Ok(())
}
