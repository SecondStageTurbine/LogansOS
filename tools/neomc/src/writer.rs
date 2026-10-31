// SPDX-License-Identifier: MPL-2.0
//! ADR-028 D1/D2 compliant `.neom` v1/v2 writer.
//! Programmatic builder that serializes to spec-compliant bytes with correct
//! offsets, page-aligned weight blob, 64-byte-aligned weights, and little-endian.

use std::path::Path;

use crate::format::{
    self, CONFIG_LAYER_COUNT_OFF, CONFIG_RMS_NORM_EPS_OFF, CONFIG_ROPE_THETA_OFF, Dtype,
    HEADER_SIZE, IO_ENTRY_SIZE, NEOM_MAGIC, NEOM_VERSION, NODE_ENTRY_SIZE, NeomError, NeomResult,
    OpCode, PAGE_SIZE, TENSOR_ENTRY_SIZE, WEIGHT_ALIGN, align_up, shape_element_count,
    write_f32_le, write_u32_le, write_u64_le,
};

// ── Builder types ────────────────────────────────────────────────────────────

/// Descriptor for a weight tensor to add to the model.
#[derive(Clone, Debug)]
pub struct TensorSpec {
    pub name: String,
    pub dtype: Dtype,
    pub dims: [u64; 8],
    pub rank: u32,
    pub data: Vec<u8>,
}

/// Descriptor for an op-graph node.
#[derive(Clone, Debug)]
pub struct NodeSpec {
    pub name: String,
    pub op: OpCode,
    /// Value refs for inputs.
    pub inputs: Vec<u32>,
    /// Value refs for outputs.
    pub outputs: Vec<u32>,
    /// Opaque attribute string (may be empty).
    pub attrs: String,
}

/// Descriptor for a model input or output declaration.
#[derive(Clone, Debug)]
pub struct IoSpec {
    /// The graph value index this entry declares.
    pub value_index: u32,
    pub dtype: Dtype,
    pub dims: [u64; 8],
    pub rank: u32,
    pub is_output: bool,
}

/// B2: per-layer schedule entry with config-ref.
#[derive(Copy, Clone, Debug)]
struct LayerEntry {
    kind: crate::format::LayerKind,
    config_ref: u32,
}

/// Public schedule entry for a v3 `.neom` layer descriptor.
#[derive(Copy, Clone, Debug)]
pub struct ScheduleEntrySpec {
    pub kind: crate::format::LayerKind,
    pub config_ref: u32,
}

/// High-level builder for a `.neom` v1/v2 file.
pub struct NeomBuilder {
    tensors: Vec<TensorSpec>,
    nodes: Vec<NodeSpec>,
    inputs: Vec<IoSpec>,
    outputs: Vec<IoSpec>,
    /// Optional v2 config-metadata extension fields. When present, the writer
    /// emits a v2 header (108 bytes) with these values; otherwise it falls back
    /// to v1 (96 bytes).
    layer_count: Option<u32>,
    rope_theta: Option<f32>,
    rms_norm_eps: Option<f32>,
    /// Optional v4 partial-RoPE geometry. Requires a v3+ schedule so older
    /// homogeneous Qwen2 files retain their byte-for-byte v1/v2 layout.
    rope_dimension_count: Option<u32>,
    /// Optional OT-16 v3 layer-kind schedule. When present, the writer emits a
    /// v3 header (124 bytes) + a LayerDescriptor table. Requires the v2 config
    /// block (a schedule describes a real model). Existing v1/v2 output is
    /// unchanged when this is `None`. B2: each entry carries a config-ref.
    schedule: Option<Vec<LayerEntry>>,
}

impl Default for NeomBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl NeomBuilder {
    pub fn new() -> Self {
        Self {
            tensors: Vec::new(),
            nodes: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            layer_count: None,
            rope_theta: None,
            rms_norm_eps: None,
            rope_dimension_count: None,
            schedule: None,
        }
    }

    /// Set the number of transformer blocks (layer count) for v2 config metadata.
    pub fn set_layer_count(mut self, value: u32) -> Self {
        self.layer_count = Some(value);
        self
    }

    /// Set the RoPE theta (frequency base) for v2 config metadata.
    pub fn set_rope_theta(mut self, value: f32) -> Self {
        self.rope_theta = Some(value);
        self
    }

    /// Set the RMS norm epsilon for v2 config metadata.
    pub fn set_rms_norm_eps(mut self, value: f32) -> Self {
        self.rms_norm_eps = Some(value);
        self
    }

    /// Set the per-head rotary dimension count. When combined with a schedule,
    /// this emits a v4 header; v1/v2/v3 output remains unchanged otherwise.
    pub fn set_rope_dimension_count(mut self, value: u32) -> Self {
        self.rope_dimension_count = Some(value);
        self
    }

    /// Attach an OT-16 v3 layer-kind schedule (one `LayerKind` per layer). Emits
    /// a v3 `.neom`. Requires the v2 config block to also be set (`build` errors
    /// otherwise). B2: each entry includes a `config_ref` (0 for Qwen2, GDN
    /// config table index for GatedDeltaNetLinear).
    pub fn set_schedule(mut self, kinds: Vec<crate::format::LayerKind>) -> Self {
        self.schedule = Some(
            kinds
                .iter()
                .map(|k| crate::writer::LayerEntry {
                    kind: *k,
                    config_ref: 0,
                })
                .collect(),
        );
        self
    }

    /// Attach an OT-16 v3 schedule with explicit per-layer config refs.
    pub fn set_schedule_entries(mut self, entries: Vec<ScheduleEntrySpec>) -> Self {
        self.schedule = Some(
            entries
                .iter()
                .map(|entry| crate::writer::LayerEntry {
                    kind: entry.kind,
                    config_ref: entry.config_ref,
                })
                .collect(),
        );
        self
    }

    pub fn add_tensor(mut self, spec: TensorSpec) -> Self {
        self.tensors.push(spec);
        self
    }

    pub fn add_node(mut self, spec: NodeSpec) -> Self {
        self.nodes.push(spec);
        self
    }

    pub fn add_input(mut self, spec: IoSpec) -> Self {
        self.inputs.push(spec);
        self
    }

    pub fn add_output(mut self, spec: IoSpec) -> Self {
        self.outputs.push(spec);
        self
    }

    /// Serialize the builder state to a valid `.neom` v1 byte buffer.
    pub fn build(self) -> NeomResult<Vec<u8>> {
        let tensor_count = self.tensors.len() as u32;
        let node_count = self.nodes.len() as u32;
        let input_count = self.inputs.len() as u32;
        let output_count = self.outputs.len() as u32;
        let io_count = input_count + output_count;

        // ── 1. Collect all strings for the string heap ────────────────────
        // Each entry: (byte_offset_in_heap, &str, len)
        let mut strings: Vec<(u32, &str, u32)> = Vec::new();
        let mut string_cursor: u32 = 0;

        let mut tensor_name_refs: Vec<(u32, u32)> = Vec::new();
        for t in &self.tensors {
            let len = t.name.len() as u32;
            tensor_name_refs.push((string_cursor, len));
            strings.push((string_cursor, &t.name, len));
            string_cursor += len;
        }

        let mut node_name_refs: Vec<(u32, u32)> = Vec::new();
        let mut node_attr_refs: Vec<(u32, u32)> = Vec::new();
        for n in &self.nodes {
            let nlen = n.name.len() as u32;
            node_name_refs.push((string_cursor, nlen));
            strings.push((string_cursor, &n.name, nlen));
            string_cursor += nlen;

            let alen = n.attrs.len() as u32;
            node_attr_refs.push((string_cursor, alen));
            if alen > 0 {
                strings.push((string_cursor, &n.attrs, alen));
                string_cursor += alen;
            }
        }

        let string_data: Vec<u8> = strings
            .iter()
            .flat_map(|(_, s, _)| s.as_bytes().to_vec())
            .collect();
        let string_len = string_data.len() as u64;

        // ── 2. Determine version & header size (before layout) ───────────
        let has_config_meta =
            self.layer_count.is_some() && self.rope_theta.is_some() && self.rms_norm_eps.is_some();
        let has_schedule = self.schedule.is_some();
        let has_rope_dimension_count = self.rope_dimension_count.is_some();
        // A v3 schedule describes a real model, so it requires the v2 config block.
        if has_schedule && !has_config_meta {
            return Err(NeomError::ScheduleRequiresConfig);
        }
        if has_rope_dimension_count && !has_schedule {
            return Err(NeomError::InvalidSchedule(
                "rope_dimension_count requires a layer schedule",
            ));
        }
        let (version, header_size): (u32, u32) = if has_rope_dimension_count {
            (
                crate::format::NEOM_VERSION_V4,
                crate::format::HEADER_SIZE_V4,
            )
        } else if has_schedule {
            (
                crate::format::NEOM_VERSION_V3,
                crate::format::HEADER_SIZE_V3,
            )
        } else if has_config_meta {
            (NEOM_VERSION, HEADER_SIZE)
        } else {
            (
                crate::format::NEOM_VERSION_V1,
                crate::format::HEADER_SIZE_V1,
            )
        };

        // ── 3. Compute layout offsets ─────────────────────────────────────
        // Tables start after header, 8-byte aligned. The v2 header is 108
        // bytes, so its first table begins at 112.
        let mut off: u64 = align_up(header_size as u64, 8);

        // Tensor table
        let tensor_table_off = off;
        let tensor_table_len = (tensor_count as u64) * TENSOR_ENTRY_SIZE as u64;
        off += tensor_table_len;
        off = align_up(off, 8);

        // Node table: NodeEntry array + value-ref arrays
        let node_entry_array_len = (node_count as u64) * NODE_ENTRY_SIZE as u64;

        // Compute total value refs for all nodes
        let total_inputs: u64 = self.nodes.iter().map(|n| n.inputs.len() as u64).sum();
        let total_outputs: u64 = self.nodes.iter().map(|n| n.outputs.len() as u64).sum();
        let total_value_refs = total_inputs + total_outputs;
        let value_ref_array_len = total_value_refs * 4; // u32 each

        // Pack input refs first, then output refs, contiguously after node entries
        let node_table_off = off;
        let node_table_end = off + node_entry_array_len + value_ref_array_len;
        off = node_table_end;
        off = align_up(off, 8);

        // IO table
        let io_table_off = off;
        let io_table_len = (io_count as u64) * IO_ENTRY_SIZE as u64;
        off += io_table_len;
        off = align_up(off, 8);

        // v3 schedule table (LayerDescriptor array), 8-aligned
        let schedule_layer_count = self.schedule.as_ref().map_or(0, |s| s.len() as u32);
        let schedule_table_off = off;
        let schedule_table_len =
            (schedule_layer_count as u64) * crate::format::LAYER_DESCRIPTOR_SIZE as u64;
        off += schedule_table_len;
        off = align_up(off, 8);

        // String heap
        let string_off = off;
        off += string_len;
        off = align_up(off, 8);

        // Weight blob — page-aligned
        let weight_blob_off = align_up(off, PAGE_SIZE as u64);

        // ── 3. Compute weight offsets within the blob ─────────────────────
        let mut weight_cursor: u64 = 0;
        let mut tensor_weight_refs: Vec<(u64, u64)> = Vec::new(); // (blob_rel_off, len)
        for t in &self.tensors {
            let expected_len =
                shape_element_count(t.rank as usize, &t.dims)? * t.dtype.size_bytes();
            if (t.data.len() as u64) != expected_len {
                return Err(NeomError::WeightLenMismatch {
                    tensor_idx: tensor_weight_refs.len(),
                    expected: expected_len,
                    actual: t.data.len() as u64,
                });
            }
            // 64-byte align within blob
            weight_cursor = align_up(weight_cursor, WEIGHT_ALIGN as u64);
            tensor_weight_refs.push((weight_cursor, expected_len));
            weight_cursor += expected_len;
        }
        let weight_blob_len = weight_cursor;

        // ── 4. Total file size ────────────────────────────────────────────
        let total_size = weight_blob_off + weight_blob_len;

        // ── 6. Allocate buffer ────────────────────────────────────────────
        let mut buf = vec![0u8; total_size as usize];

        // ── 6. Write header ──────────────────────────────────────────────
        buf[0..4].copy_from_slice(&NEOM_MAGIC);
        write_u32_le(&mut buf, 4, version);
        write_u32_le(&mut buf, 8, header_size);
        write_u32_le(&mut buf, 12, 0); // flags = 0
        write_u32_le(&mut buf, 16, tensor_count);
        write_u32_le(&mut buf, 20, node_count);
        write_u32_le(&mut buf, 24, input_count);
        write_u32_le(&mut buf, 28, output_count);
        write_u64_le(&mut buf, 32, tensor_table_off);
        write_u64_le(&mut buf, 40, node_table_off);
        write_u64_le(&mut buf, 48, io_table_off);
        write_u64_le(&mut buf, 56, string_off);
        write_u64_le(&mut buf, 64, string_len);
        write_u64_le(&mut buf, 72, weight_blob_off);
        write_u64_le(&mut buf, 80, weight_blob_len);
        write_u64_le(&mut buf, 88, total_size);

        // ── v2 config-metadata extension (offsets 96..108) ───────────────
        if has_config_meta {
            write_u32_le(&mut buf, CONFIG_LAYER_COUNT_OFF, self.layer_count.unwrap());
            write_f32_le(&mut buf, CONFIG_ROPE_THETA_OFF, self.rope_theta.unwrap());
            write_f32_le(
                &mut buf,
                CONFIG_RMS_NORM_EPS_OFF,
                self.rms_norm_eps.unwrap(),
            );
        }

        // ── v3 schedule-section descriptor (offsets 108..124) ────────────
        if has_schedule {
            write_u32_le(
                &mut buf,
                crate::format::SCHEDULE_LAYER_COUNT_OFF,
                schedule_layer_count,
            );
            write_u32_le(
                &mut buf,
                crate::format::SCHEDULE_VERSION_OFF,
                crate::format::SCHEDULE_VERSION,
            );
            write_u64_le(
                &mut buf,
                crate::format::SCHEDULE_OFF_OFF,
                schedule_table_off,
            );
        }

        // ── v4 attention-geometry extension (offset 124) ─────────────────
        if has_rope_dimension_count {
            write_u32_le(
                &mut buf,
                crate::format::CONFIG_ROPE_DIMENSION_COUNT_OFF,
                self.rope_dimension_count.unwrap(),
            );
        }

        // ── 7. Write tensor table ────────────────────────────────────────
        for (i, t) in self.tensors.iter().enumerate() {
            let base = tensor_table_off as usize + i * TENSOR_ENTRY_SIZE as usize;
            write_u32_le(&mut buf, base, t.dtype as u32);
            write_u32_le(&mut buf, base + 4, t.rank);
            // dims: 8 x u64
            for d in 0..8 {
                write_u64_le(&mut buf, base + 8 + d * 8, t.dims[d]);
            }
            let (w_off, w_len) = tensor_weight_refs[i];
            write_u64_le(&mut buf, base + 72, weight_blob_off + w_off);
            write_u64_le(&mut buf, base + 80, w_len);
            let (n_off, n_len) = tensor_name_refs[i];
            write_u32_le(&mut buf, base + 88, n_off);
            write_u32_le(&mut buf, base + 92, n_len);
        }

        // ── 8. Write node table ──────────────────────────────────────────
        // Compute per-node value ref offsets within the value-ref region
        let value_ref_base = node_table_off + node_entry_array_len;
        let mut input_ref_cursor: u64 = 0;
        let mut output_ref_cursor: u64 = total_inputs;

        for (i, n) in self.nodes.iter().enumerate() {
            let base = node_table_off as usize + i * NODE_ENTRY_SIZE as usize;
            write_u32_le(&mut buf, base, n.op as u32);
            write_u32_le(&mut buf, base + 4, n.inputs.len() as u32);
            write_u32_le(&mut buf, base + 8, n.outputs.len() as u32);
            // inputs_off: offset into the value-ref array (absolute file offset)
            write_u32_le(
                &mut buf,
                base + 12,
                (value_ref_base + input_ref_cursor * 4) as u32,
            );
            // outputs_off
            write_u32_le(
                &mut buf,
                base + 16,
                (value_ref_base + output_ref_cursor * 4) as u32,
            );
            let (a_off, a_len) = node_attr_refs[i];
            write_u32_le(&mut buf, base + 20, a_off);
            write_u32_le(&mut buf, base + 24, a_len);
            let (n_off, n_len) = node_name_refs[i];
            write_u32_le(&mut buf, base + 28, n_off);
            write_u32_le(&mut buf, base + 32, n_len);
            // reserved 12 bytes already zero

            // Write input refs
            for (j, &ref_val) in n.inputs.iter().enumerate() {
                let ref_off = (value_ref_base + (input_ref_cursor + j as u64) * 4) as usize;
                write_u32_le(&mut buf, ref_off, ref_val);
            }
            input_ref_cursor += n.inputs.len() as u64;

            // Write output refs
            for (j, &ref_val) in n.outputs.iter().enumerate() {
                let ref_off = (value_ref_base + (output_ref_cursor + j as u64) * 4) as usize;
                write_u32_le(&mut buf, ref_off, ref_val);
            }
            output_ref_cursor += n.outputs.len() as u64;
        }

        // ── 9. Write IO table ────────────────────────────────────────────
        let all_ios: Vec<&IoSpec> = self.inputs.iter().chain(self.outputs.iter()).collect();
        for (i, io) in all_ios.iter().enumerate() {
            let base = io_table_off as usize + i * IO_ENTRY_SIZE as usize;
            write_u32_le(&mut buf, base, io.value_index);
            write_u32_le(&mut buf, base + 4, io.dtype as u32);
            write_u32_le(&mut buf, base + 8, io.rank);
            write_u32_le(&mut buf, base + 12, if io.is_output { 1 } else { 0 });
            for d in 0..8 {
                write_u64_le(&mut buf, base + 16 + d * 8, io.dims[d]);
            }
        }

        // ── Write v3 schedule table (LayerDescriptor entries) ────────────
        if let Some(sched) = &self.schedule {
            for (i, entry) in sched.iter().enumerate() {
                let base =
                    schedule_table_off as usize + i * crate::format::LAYER_DESCRIPTOR_SIZE as usize;
                write_u32_le(
                    &mut buf,
                    base + crate::format::LAYER_DESCRIPTOR_KIND_OFF,
                    entry.kind as u32,
                );
                // B2: config-ref in the reserved slot (ADR-072 Decision 2)
                write_u32_le(
                    &mut buf,
                    base + crate::format::LAYER_DESCRIPTOR_RESERVED_OFF,
                    entry.config_ref,
                );
            }
        }

        // ── 10. Write string heap ────────────────────────────────────────
        buf[string_off as usize..(string_off + string_len) as usize].copy_from_slice(&string_data);

        // ── 11. Write weight blob ────────────────────────────────────────
        for (i, t) in self.tensors.iter().enumerate() {
            let (w_off, _) = tensor_weight_refs[i];
            let w_start = (weight_blob_off + w_off) as usize;
            buf[w_start..w_start + t.data.len()].copy_from_slice(&t.data);
        }

        Ok(buf)
    }

    /// Build and write to a file path.
    pub fn build_and_write(self, path: &Path) -> NeomResult<()> {
        let buf = self.build()?;
        format::write_file(&buf, path).map_err(|_| NeomError::IOError)
    }
}

#[cfg(test)]
mod schedule_byte_oracle {
    //! Hand-laid byte oracle for the OT-16 v3 layer-kind schedule section
    //! (ADR-028 v3 amendment). The expected bytes are hand-computed from the
    //! spec, NOT read back from the writer — round-trip alone is insufficient
    //! (a writer + reader built from the same off-spec understanding agree even
    //! when both are wrong; the ADR-028 `.neom` v1 lesson).
    use crate::format::{
        CONFIG_ROPE_DIMENSION_COUNT_OFF, Dtype, HEADER_SIZE_V3, HEADER_SIZE_V4,
        LAYER_DESCRIPTOR_SIZE, LayerKind, NEOM_VERSION_V3, NEOM_VERSION_V4,
        SCHEDULE_LAYER_COUNT_OFF, SCHEDULE_OFF_OFF, SCHEDULE_VERSION, SCHEDULE_VERSION_OFF,
        read_u32_le, read_u64_le,
    };
    use crate::writer::{NeomBuilder, ScheduleEntrySpec, TensorSpec};

    #[test]
    fn v3_schedule_section_matches_hand_laid_bytes() {
        // 2 layers with DISTINCT kinds so the oracle is discriminating (not
        // all-zero): layer 0 = Qwen2AttentionFFN (kind=0), layer 1 =
        // GatedDeltaNetLinear (kind=1). The writer serializes whatever kinds it
        // is given; rejecting GDN is the load-time walker's job, not the writer's.
        let neom = NeomBuilder::new()
            .set_layer_count(2)
            .set_rope_theta(1_000_000.0)
            .set_rms_norm_eps(1.0e-6)
            .set_schedule(vec![
                LayerKind::Qwen2AttentionFFN,
                LayerKind::GatedDeltaNetLinear,
            ])
            .add_tensor(TensorSpec {
                name: "w".to_string(),
                dtype: Dtype::F32,
                dims: [1, 0, 0, 0, 0, 0, 0, 0],
                rank: 1,
                data: 1.0f32.to_le_bytes().to_vec(),
            })
            .build()
            .expect("v3 build should succeed");

        // Container header: version=3, header_size=124.
        assert_eq!(read_u32_le(&neom, 4), NEOM_VERSION_V3, "container version");
        assert_eq!(read_u32_le(&neom, 8), HEADER_SIZE_V3, "v3 header size");

        // v3 schedule descriptor (offsets 108..124).
        assert_eq!(
            read_u32_le(&neom, SCHEDULE_LAYER_COUNT_OFF),
            2,
            "schedule layer_count"
        );
        assert_eq!(
            read_u32_le(&neom, SCHEDULE_VERSION_OFF),
            SCHEDULE_VERSION,
            "schedule version"
        );
        let sched_off = read_u64_le(&neom, SCHEDULE_OFF_OFF) as usize;
        assert_eq!(sched_off % 8, 0, "schedule table is 8-aligned");
        assert!(sched_off + 16 <= neom.len(), "schedule table in bounds");
        assert_eq!(LAYER_DESCRIPTOR_SIZE, 8, "descriptor size");

        // Hand-laid expected 16 bytes of the 2-entry LayerDescriptor table:
        //   entry 0: kind=0 (Qwen2AttentionFFN) LE u32, reserved=0 u32
        //   entry 1: kind=1 (GatedDeltaNetLinear) LE u32, reserved=0 u32
        let expected: [u8; 16] = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // entry 0: kind=0, reserved=0
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // entry 1: kind=1, reserved=0
        ];
        assert_eq!(
            &neom[sched_off..sched_off + 16],
            &expected,
            "hand-laid v3 schedule table bytes"
        );
    }

    #[test]
    fn v4_rope_dimension_count_matches_hand_laid_bytes() {
        let neom = NeomBuilder::new()
            .set_layer_count(1)
            .set_rope_theta(10_000_000.0)
            .set_rms_norm_eps(1.0e-6)
            .set_schedule(vec![LayerKind::Qwen2AttentionFFN])
            .set_rope_dimension_count(64)
            .add_tensor(TensorSpec {
                name: "w".to_string(),
                dtype: Dtype::F32,
                dims: [1, 0, 0, 0, 0, 0, 0, 0],
                rank: 1,
                data: 1.0f32.to_le_bytes().to_vec(),
            })
            .build()
            .expect("v4 build should succeed");

        assert_eq!(read_u32_le(&neom, 4), NEOM_VERSION_V4);
        assert_eq!(read_u32_le(&neom, 8), HEADER_SIZE_V4);
        assert_eq!(
            &neom[CONFIG_ROPE_DIMENSION_COUNT_OFF..CONFIG_ROPE_DIMENSION_COUNT_OFF + 4],
            &[0x40, 0x00, 0x00, 0x00],
            "hand-laid LE bytes for rope_dimension_count=64"
        );
        assert_eq!(read_u32_le(&neom, CONFIG_ROPE_DIMENSION_COUNT_OFF), 64);
    }

    #[test]
    fn v3_schedule_config_ref_bytes_are_discriminating() {
        let neom = NeomBuilder::new()
            .set_layer_count(2)
            .set_rope_theta(1_000_000.0)
            .set_rms_norm_eps(1.0e-6)
            .set_schedule_entries(vec![
                ScheduleEntrySpec {
                    kind: LayerKind::Qwen2AttentionFFN,
                    config_ref: 0,
                },
                ScheduleEntrySpec {
                    kind: LayerKind::GatedDeltaNetLinear,
                    config_ref: 7,
                },
            ])
            .add_tensor(TensorSpec {
                name: "w".to_string(),
                dtype: Dtype::F32,
                dims: [1, 0, 0, 0, 0, 0, 0, 0],
                rank: 1,
                data: 1.0f32.to_le_bytes().to_vec(),
            })
            .build()
            .expect("v3 build should succeed");

        let sched_off = read_u64_le(&neom, SCHEDULE_OFF_OFF) as usize;
        let expected: [u8; 16] = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x07, 0x00,
            0x00, 0x00,
        ];
        assert_eq!(
            &neom[sched_off..sched_off + 16],
            &expected,
            "hand-laid v3 config_ref schedule bytes"
        );
    }
}
