// SPDX-License-Identifier: MPL-2.0
//! Host tests for Phase 5 .neom model descriptor validation.
#![allow(dead_code)]

extern crate alloc;

#[path = "../../../kernel/src/ai/model_logic.rs"]
mod model_logic;

#[cfg(test)]
mod tests {
    use super::model_logic::{
        LayerKind, ModelFormatError, NEOM_CONFIG_LAYER_COUNT_OFF, NEOM_CONFIG_RMS_NORM_EPS_OFF,
        NEOM_CONFIG_ROPE_DIMENSION_COUNT_OFF, NEOM_CONFIG_ROPE_THETA_OFF, NeomDtype,
        neom_total_size_hint, resolve_schedule, validate_neom,
    };

    const HEADER_OFF: usize = 0;
    const TENSOR_OFF: usize = 96;
    const NODE_OFF: usize = 192;
    const IO_OFF: usize = 248;
    const STRING_OFF: usize = 408;
    const WEIGHT_OFF: usize = 4096;
    const TOTAL: usize = 4100;

    #[test]
    fn valid_neom_v1_header_and_graph_parse() {
        let bytes = valid_neom();
        let parsed = match validate_neom(&bytes) {
            Ok(parsed) => parsed,
            Err(err) => panic!("valid .neom rejected: {err:?}"),
        };
        assert_eq!(parsed.header.tensor_count, 1);
        assert_eq!(parsed.header.node_count, 1);
        assert_eq!(parsed.header.input_count, 1);
        assert_eq!(parsed.header.output_count, 1);
        assert_eq!(parsed.value_count, 3);
    }

    #[test]
    fn valid_neom_v2_header_config_offsets_parse() {
        let bytes = valid_neom_v2_config_header();
        assert_eq!(read_u32(&bytes, NEOM_CONFIG_LAYER_COUNT_OFF), 24);
        assert_eq!(read_f32(&bytes, NEOM_CONFIG_ROPE_THETA_OFF), 1_000_000.0);
        assert_eq!(read_f32(&bytes, NEOM_CONFIG_RMS_NORM_EPS_OFF), 1.0e-6);

        let parsed = validate_neom(&bytes).expect("valid v2 .neom rejected");
        assert_eq!(parsed.header.version, 2);
        assert_eq!(parsed.header.header_size, 108);
        let config = parsed.header.config.expect("v2 config metadata");
        assert_eq!(config.layer_count, 24);
        assert_eq!(config.rope_theta(), 1_000_000.0);
        assert_eq!(config.rms_norm_eps(), 1.0e-6);
    }

    #[test]
    fn rejects_bad_magic_and_bad_version() {
        let mut bad_magic = valid_neom();
        bad_magic[0] = b'X';
        assert_eq!(validate_neom(&bad_magic), Err(ModelFormatError::BadMagic));

        // v1/v2/v3 are the known versions (OT-16 added v3); use a genuinely
        // unknown version to exercise BadVersion. (A known version with the wrong
        // header_size is InvalidHeader instead — see rejects_known_version_wrong_size.)
        let mut bad_version = valid_neom();
        put_u32(&mut bad_version, HEADER_OFF + 4, 99);
        assert_eq!(
            validate_neom(&bad_version),
            Err(ModelFormatError::BadVersion)
        );

        // v3 version on a v1-sized (96-byte) header: known version, wrong size.
        let mut wrong_size = valid_neom();
        put_u32(&mut wrong_size, HEADER_OFF + 4, 3);
        assert_eq!(
            validate_neom(&wrong_size),
            Err(ModelFormatError::InvalidHeader)
        );
    }

    #[test]
    fn rejects_overflowing_weight_offset() {
        let mut bytes = valid_neom();
        put_u64(&mut bytes, TENSOR_OFF + 72, u64::MAX);
        assert_eq!(validate_neom(&bytes), Err(ModelFormatError::InvalidTensor));
    }

    #[test]
    fn rejects_weight_length_mismatch() {
        let mut bytes = valid_neom();
        put_u64(&mut bytes, TENSOR_OFF + 80, 8);
        assert_eq!(validate_neom(&bytes), Err(ModelFormatError::InvalidTensor));
    }

    #[test]
    fn q5_k_dtype_reader_accepts_literal_abi_seven() {
        let mut bytes = valid_neom();
        put_u32(&mut bytes, TENSOR_OFF, 7);
        put_u64(&mut bytes, TENSOR_OFF + 8, 4);

        assert_eq!(&bytes[TENSOR_OFF..TENSOR_OFF + 4], &[7, 0, 0, 0]);
        assert_eq!(NeomDtype::from_abi(7), Ok(NeomDtype::Q5K));
        validate_neom(&bytes).expect("literal Q5_K dtype tensor validates");
    }

    #[test]
    fn rejects_unknown_op() {
        let mut bytes = valid_neom();
        put_u32(&mut bytes, NODE_OFF, 99);
        assert_eq!(validate_neom(&bytes), Err(ModelFormatError::UnsupportedOp));
    }

    #[test]
    fn rejects_cyclic_or_future_value_reference() {
        let mut bytes = valid_neom();
        put_u32(&mut bytes, NODE_OFF + 48, 2);
        assert_eq!(validate_neom(&bytes), Err(ModelFormatError::InvalidGraph));
    }

    fn valid_neom() -> Vec<u8> {
        let mut bytes = vec![0u8; TOTAL];
        let strings = b"weightinputnodeoutput";
        bytes[STRING_OFF..STRING_OFF + strings.len()].copy_from_slice(strings);
        bytes[WEIGHT_OFF..WEIGHT_OFF + 4].copy_from_slice(&1.0f32.to_le_bytes());

        bytes[0..4].copy_from_slice(b"NEOM");
        put_u32(&mut bytes, HEADER_OFF + 4, 1);
        put_u32(&mut bytes, HEADER_OFF + 8, 96);
        put_u32(&mut bytes, HEADER_OFF + 16, 1);
        put_u32(&mut bytes, HEADER_OFF + 20, 1);
        put_u32(&mut bytes, HEADER_OFF + 24, 1);
        put_u32(&mut bytes, HEADER_OFF + 28, 1);
        put_u64(&mut bytes, HEADER_OFF + 32, TENSOR_OFF as u64);
        put_u64(&mut bytes, HEADER_OFF + 40, NODE_OFF as u64);
        put_u64(&mut bytes, HEADER_OFF + 48, IO_OFF as u64);
        put_u64(&mut bytes, HEADER_OFF + 56, STRING_OFF as u64);
        put_u64(&mut bytes, HEADER_OFF + 64, strings.len() as u64);
        put_u64(&mut bytes, HEADER_OFF + 72, WEIGHT_OFF as u64);
        put_u64(&mut bytes, HEADER_OFF + 80, 4);
        put_u64(&mut bytes, HEADER_OFF + 88, TOTAL as u64);

        put_u32(&mut bytes, TENSOR_OFF, 0);
        put_u32(&mut bytes, TENSOR_OFF + 4, 1);
        put_u64(&mut bytes, TENSOR_OFF + 8, 1);
        put_u64(&mut bytes, TENSOR_OFF + 72, WEIGHT_OFF as u64);
        put_u64(&mut bytes, TENSOR_OFF + 80, 4);
        put_u32(&mut bytes, TENSOR_OFF + 88, 0);
        put_u32(&mut bytes, TENSOR_OFF + 92, 6);

        put_u32(&mut bytes, NODE_OFF, 0);
        put_u32(&mut bytes, NODE_OFF + 4, 1);
        put_u32(&mut bytes, NODE_OFF + 8, 1);
        put_u32(&mut bytes, NODE_OFF + 12, (NODE_OFF + 48) as u32);
        put_u32(&mut bytes, NODE_OFF + 16, (NODE_OFF + 52) as u32);
        put_u32(&mut bytes, NODE_OFF + 28, 11);
        put_u32(&mut bytes, NODE_OFF + 32, 4);
        put_u32(&mut bytes, NODE_OFF + 48, 1);
        put_u32(&mut bytes, NODE_OFF + 52, 2);

        put_u32(&mut bytes, IO_OFF, 1);
        put_u32(&mut bytes, IO_OFF + 4, 0);
        put_u32(&mut bytes, IO_OFF + 8, 1);
        put_u64(&mut bytes, IO_OFF + 16, 1);
        put_u32(&mut bytes, IO_OFF + 80, 2);
        put_u32(&mut bytes, IO_OFF + 84, 0);
        put_u32(&mut bytes, IO_OFF + 88, 1);
        put_u32(&mut bytes, IO_OFF + 92, 1);
        put_u64(&mut bytes, IO_OFF + 96, 1);
        bytes
    }

    fn valid_neom_v2_config_header() -> Vec<u8> {
        const TENSOR_OFF_V2: usize = 112;
        const EMPTY_TABLES_OFF: usize = 208;

        let mut bytes = vec![0u8; TOTAL];
        bytes[WEIGHT_OFF..WEIGHT_OFF + 4].copy_from_slice(&1.0f32.to_le_bytes());

        bytes[0..4].copy_from_slice(b"NEOM");
        put_u32(&mut bytes, HEADER_OFF + 4, 2);
        put_u32(&mut bytes, HEADER_OFF + 8, 108);
        put_u32(&mut bytes, HEADER_OFF + 16, 1);
        put_u64(&mut bytes, HEADER_OFF + 32, TENSOR_OFF_V2 as u64);
        put_u64(&mut bytes, HEADER_OFF + 40, EMPTY_TABLES_OFF as u64);
        put_u64(&mut bytes, HEADER_OFF + 48, EMPTY_TABLES_OFF as u64);
        put_u64(&mut bytes, HEADER_OFF + 56, EMPTY_TABLES_OFF as u64);
        put_u64(&mut bytes, HEADER_OFF + 72, WEIGHT_OFF as u64);
        put_u64(&mut bytes, HEADER_OFF + 80, 4);
        put_u64(&mut bytes, HEADER_OFF + 88, TOTAL as u64);
        put_u32(&mut bytes, NEOM_CONFIG_LAYER_COUNT_OFF, 24);
        put_f32(&mut bytes, NEOM_CONFIG_ROPE_THETA_OFF, 1_000_000.0);
        put_f32(&mut bytes, NEOM_CONFIG_RMS_NORM_EPS_OFF, 1.0e-6);

        put_u32(&mut bytes, TENSOR_OFF_V2, 0);
        put_u32(&mut bytes, TENSOR_OFF_V2 + 4, 1);
        put_u64(&mut bytes, TENSOR_OFF_V2 + 8, 1);
        put_u64(&mut bytes, TENSOR_OFF_V2 + 72, WEIGHT_OFF as u64);
        put_u64(&mut bytes, TENSOR_OFF_V2 + 80, 4);
        bytes
    }

    fn put_u32(bytes: &mut [u8], off: usize, value: u32) {
        bytes[off..off + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u64(bytes: &mut [u8], off: usize, value: u64) {
        bytes[off..off + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn put_f32(bytes: &mut [u8], off: usize, value: f32) {
        bytes[off..off + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn read_u32(bytes: &[u8], off: usize) -> u32 {
        u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap())
    }

    fn read_f32(bytes: &[u8], off: usize) -> f32 {
        f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap())
    }

    // ── OT-16 v3 layer-kind schedule: reader-side byte-oracle + the 7 invariants ──

    /// Hand-laid minimal valid v3 `.neom`: the 124-byte v3 header + config + a
    /// layer-kind schedule table (`kinds`) + one tiny tensor. Layout:
    /// header(0..124) | schedule table(128..) | tensor table | weight blob.
    /// The config layer_count is set to match the schedule (the consistency
    /// invariant); tests override individual fields to exercise the negatives.
    fn valid_neom_v3_schedule(kinds: &[u32]) -> Vec<u8> {
        const WEIGHT_OFF: usize = 4096;
        const TOTAL: usize = 4100;
        const SCHEDULE_OFF: usize = 128; // 8-aligned, after the 124-byte v3 header
        let n = kinds.len();
        let tensor_off = (SCHEDULE_OFF + n * 8 + 7) & !7; // 8-align after the schedule table
        let empty_off = tensor_off + 96;
        let mut bytes = vec![0u8; TOTAL];
        bytes[WEIGHT_OFF..WEIGHT_OFF + 4].copy_from_slice(&1.0f32.to_le_bytes());
        bytes[0..4].copy_from_slice(b"NEOM");
        put_u32(&mut bytes, 4, 3); // version 3
        put_u32(&mut bytes, 8, 124); // header_size
        put_u32(&mut bytes, 16, 1); // tensor_count
        put_u64(&mut bytes, 32, tensor_off as u64);
        put_u64(&mut bytes, 40, empty_off as u64);
        put_u64(&mut bytes, 48, empty_off as u64);
        put_u64(&mut bytes, 56, empty_off as u64);
        put_u64(&mut bytes, 72, WEIGHT_OFF as u64);
        put_u64(&mut bytes, 80, 4);
        put_u64(&mut bytes, 88, TOTAL as u64);
        put_u32(&mut bytes, NEOM_CONFIG_LAYER_COUNT_OFF, n as u32); // must match schedule
        put_f32(&mut bytes, NEOM_CONFIG_ROPE_THETA_OFF, 1_000_000.0);
        put_f32(&mut bytes, NEOM_CONFIG_RMS_NORM_EPS_OFF, 1.0e-6);
        // v3 schedule descriptor (offsets 108/112/116)
        put_u32(&mut bytes, 108, n as u32); // schedule layer_count
        put_u32(&mut bytes, 112, 1); // schedule version
        put_u64(&mut bytes, 116, SCHEDULE_OFF as u64); // schedule table off
        // LayerDescriptor table: { kind: u32, reserved: u32 = 0 }
        for (i, &k) in kinds.iter().enumerate() {
            put_u32(&mut bytes, SCHEDULE_OFF + i * 8, k);
            put_u32(&mut bytes, SCHEDULE_OFF + i * 8 + 4, 0);
        }
        // one tiny tensor
        put_u32(&mut bytes, tensor_off, 0);
        put_u32(&mut bytes, tensor_off + 4, 1);
        put_u64(&mut bytes, tensor_off + 8, 1);
        put_u64(&mut bytes, tensor_off + 72, WEIGHT_OFF as u64);
        put_u64(&mut bytes, tensor_off + 80, 4);
        bytes
    }

    #[test]
    fn v3_schedule_reader_parses_hand_laid_bytes() {
        // Reader-side byte-oracle (independent of the writer): a hand-laid v3 with
        // 2x Qwen2AttentionFFN (kind=0) must parse to exactly that schedule.
        let neom = valid_neom_v3_schedule(&[0, 0]);
        let validated = validate_neom(&neom).expect("valid v3 .neom");
        assert_eq!(validated.header.version, 3);
        let sched = resolve_schedule(&neom, &validated.header, 2).expect("resolve schedule");
        assert_eq!(sched.version, 1);
        assert_eq!(sched.layers.len(), 2);
        assert_eq!(sched.layers[0].kind, LayerKind::Qwen2AttentionFFN);
        assert_eq!(sched.layers[1].kind, LayerKind::Qwen2AttentionFFN);
        assert_eq!(sched.layers[0].config_ref, 0);
        assert_eq!(sched.layers[1].config_ref, 0);
    }

    #[test]
    fn v4_rope_dimension_reader_parses_hand_laid_bytes() {
        let mut neom = valid_neom_v3_schedule(&[0, 0]);
        put_u32(&mut neom, 4, 4);
        put_u32(&mut neom, 8, 128);
        put_u32(&mut neom, NEOM_CONFIG_ROPE_DIMENSION_COUNT_OFF, 64);

        let validated = validate_neom(&neom).expect("valid hand-laid v4 .neom");
        assert_eq!(validated.header.version, 4);
        assert_eq!(validated.header.header_size, 128);
        assert_eq!(
            &neom[NEOM_CONFIG_ROPE_DIMENSION_COUNT_OFF..NEOM_CONFIG_ROPE_DIMENSION_COUNT_OFF + 4],
            &[0x40, 0x00, 0x00, 0x00]
        );
        assert_eq!(
            validated
                .header
                .config
                .expect("v4 config")
                .rope_dimension_count,
            Some(64)
        );
        let sched = resolve_schedule(&neom, &validated.header, 2).expect("v4 schedule");
        assert_eq!(sched.layers.len(), 2);
    }

    #[test]
    fn total_size_hint_accepts_complete_v1_through_v4_headers() {
        let v1 = valid_neom();
        assert_eq!(neom_total_size_hint(&v1[..96]), Ok(v1.len()));

        let v2 = valid_neom_v2_config_header();
        assert_eq!(neom_total_size_hint(&v2[..108]), Ok(v2.len()));

        let v3 = valid_neom_v3_schedule(&[0, 0]);
        assert_eq!(neom_total_size_hint(&v3[..124]), Ok(v3.len()));

        let mut v4 = v3;
        put_u32(&mut v4, 4, 4);
        put_u32(&mut v4, 8, 128);
        put_u32(&mut v4, NEOM_CONFIG_ROPE_DIMENSION_COUNT_OFF, 64);
        assert_eq!(neom_total_size_hint(&v4[..128]), Ok(v4.len()));
    }

    #[test]
    fn v2_synthesizes_homogeneous_qwen2_schedule() {
        // A v1/v2 file has no schedule section, so resolve synthesizes n x
        // Qwen2AttentionFFN — exactly the hardcoded 0..n loop the walker replaces.
        let neom = valid_neom_v2_config_header(); // config layer_count = 24
        let validated = validate_neom(&neom).expect("valid v2 .neom");
        assert!(validated.header.schedule.is_none());
        let sched = resolve_schedule(&neom, &validated.header, 24).expect("synthesize");
        assert_eq!(sched.layers.len(), 24);
        assert!(
            sched
                .layers
                .iter()
                .all(|e| e.kind == LayerKind::Qwen2AttentionFFN && e.config_ref == 0)
        );
    }

    #[test]
    fn v3_accepts_gateddeltanet_at_load() {
        // B2: GatedDeltaNetLinear (kind=1) is now accepted at load.
        let neom = valid_neom_v3_schedule(&[0, 1]);
        let validated = validate_neom(&neom).expect("valid v3 .neom with GDN");
        let sched = resolve_schedule(&neom, &validated.header, 2).expect("resolve schedule");
        assert_eq!(sched.layers[0].kind, LayerKind::Qwen2AttentionFFN);
        assert_eq!(sched.layers[1].kind, LayerKind::GatedDeltaNetLinear);
    }

    #[test]
    fn v3_rejects_unknown_kind() {
        let neom = valid_neom_v3_schedule(&[0, 99]);
        assert_eq!(
            validate_neom(&neom),
            Err(ModelFormatError::UnknownLayerKind)
        );
    }

    #[test]
    fn v3_rejects_unsupported_schedule_version() {
        let mut neom = valid_neom_v3_schedule(&[0, 0]);
        put_u32(&mut neom, 112, 2); // schedule version 2 (unsupported)
        assert_eq!(
            validate_neom(&neom),
            Err(ModelFormatError::UnsupportedScheduleVersion)
        );
    }

    #[test]
    fn v3_accepts_nonzero_config_ref() {
        // B2: the reserved field is the per-layer GDN config-ref (ADR-072 Decision 2).
        // Nonzero is valid — it indexes into the GDN config table.
        let mut neom = valid_neom_v3_schedule(&[0, 0]);
        put_u32(&mut neom, 128 + 4, 7); // descriptor 0 config_ref = 7
        let validated = validate_neom(&neom).expect("nonzero config_ref is valid in B2");
        let sched = resolve_schedule(&neom, &validated.header, 2).expect("resolve");
        assert_eq!(sched.layers[0].config_ref, 7);
    }

    #[test]
    fn v3_rejects_layer_count_mismatch_vs_config() {
        let mut neom = valid_neom_v3_schedule(&[0, 0]);
        put_u32(&mut neom, NEOM_CONFIG_LAYER_COUNT_OFF, 3); // config 3 != schedule 2
        assert_eq!(validate_neom(&neom), Err(ModelFormatError::InvalidSchedule));
    }

    #[test]
    fn v3_rejects_zero_layer_count() {
        let mut neom = valid_neom_v3_schedule(&[0, 0]);
        put_u32(&mut neom, 108, 0); // schedule layer_count 0 (bounds invariant)
        assert_eq!(validate_neom(&neom), Err(ModelFormatError::InvalidSchedule));
    }
}
