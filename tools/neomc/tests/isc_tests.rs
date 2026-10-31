// SPDX-License-Identifier: MPL-2.0
//! Host unit tests for neomc writer, validator, alignment, round-trip,
//! and negative fixture rejection (ISC-5).

use std::path::PathBuf;

use std::collections::HashMap;

use neomc::format::{
    Dtype, HEADER_SIZE, HEADER_SIZE_V1, NeomError, OpCode, PAGE_SIZE, TENSOR_ENTRY_SIZE,
    WEIGHT_ALIGN, align_up, read_u32_le, read_u64_le, shape_element_count, write_u32_le,
    write_u64_le,
};
use neomc::validator;
use neomc::writer::{IoSpec, NeomBuilder, NodeSpec, TensorSpec};

// ── Helper: build a minimal valid model ──────────────────────────────────────

fn build_minimal_valid() -> Vec<u8> {
    // Single tensor 2x2 F32, no nodes, one input, one output
    let data: Vec<u8> = (0..4).flat_map(|i| (i as f32).to_le_bytes()).collect();

    NeomBuilder::new()
        .add_tensor(TensorSpec {
            name: "W".to_string(),
            dtype: Dtype::F32,
            dims: [2, 2, 0, 0, 0, 0, 0, 0],
            rank: 2,
            data,
        })
        .add_input(IoSpec {
            value_index: 1,
            dtype: Dtype::F32,
            dims: [1, 2, 0, 0, 0, 0, 0, 0],
            rank: 2,
            is_output: false,
        })
        .add_output(IoSpec {
            value_index: 2,
            dtype: Dtype::F32,
            dims: [1, 2, 0, 0, 0, 0, 0, 0],
            rank: 2,
            is_output: true,
        })
        .build()
        .expect("build minimal valid")
}

fn build_full_mlp() -> Vec<u8> {
    let w1: Vec<u8> = (0..32).flat_map(|i| (i as f32).to_le_bytes()).collect();
    let b1: Vec<u8> = (0..8)
        .flat_map(|i| (i as f32 * 0.1).to_le_bytes())
        .collect();
    let w2: Vec<u8> = (0..24)
        .flat_map(|i| (i as f32 * 0.01).to_le_bytes())
        .collect();
    let b2: Vec<u8> = (0..3)
        .flat_map(|i| (i as f32 * 0.05).to_le_bytes())
        .collect();

    NeomBuilder::new()
        .add_tensor(TensorSpec {
            name: "W1".to_string(),
            dtype: Dtype::F32,
            dims: [4, 8, 0, 0, 0, 0, 0, 0],
            rank: 2,
            data: w1,
        })
        .add_tensor(TensorSpec {
            name: "b1".to_string(),
            dtype: Dtype::F32,
            dims: [1, 8, 0, 0, 0, 0, 0, 0],
            rank: 2,
            data: b1,
        })
        .add_tensor(TensorSpec {
            name: "W2".to_string(),
            dtype: Dtype::F32,
            dims: [8, 3, 0, 0, 0, 0, 0, 0],
            rank: 2,
            data: w2,
        })
        .add_tensor(TensorSpec {
            name: "b2".to_string(),
            dtype: Dtype::F32,
            dims: [1, 3, 0, 0, 0, 0, 0, 0],
            rank: 2,
            data: b2,
        })
        .add_node(NodeSpec {
            name: "mm1".to_string(),
            op: OpCode::MatMul,
            inputs: vec![4, 0],
            outputs: vec![5],
            attrs: String::new(),
        })
        .add_node(NodeSpec {
            name: "add1".to_string(),
            op: OpCode::Add,
            inputs: vec![5, 1],
            outputs: vec![6],
            attrs: String::new(),
        })
        .add_node(NodeSpec {
            name: "relu1".to_string(),
            op: OpCode::Relu,
            inputs: vec![6],
            outputs: vec![7],
            attrs: String::new(),
        })
        .add_node(NodeSpec {
            name: "mm2".to_string(),
            op: OpCode::MatMul,
            inputs: vec![7, 2],
            outputs: vec![8],
            attrs: String::new(),
        })
        .add_node(NodeSpec {
            name: "add2".to_string(),
            op: OpCode::Add,
            inputs: vec![8, 3],
            outputs: vec![9],
            attrs: String::new(),
        })
        .add_input(IoSpec {
            value_index: 4,
            dtype: Dtype::F32,
            dims: [1, 4, 0, 0, 0, 0, 0, 0],
            rank: 2,
            is_output: false,
        })
        .add_output(IoSpec {
            value_index: 9,
            dtype: Dtype::F32,
            dims: [1, 3, 0, 0, 0, 0, 0, 0],
            rank: 2,
            is_output: true,
        })
        .build()
        .expect("build full MLP")
}

// ── ISC-1: Writer produces byte-exact ADR-028 D1 layout ─────────────────────

fn f32_payload(values: impl IntoIterator<Item = f32>) -> Vec<u8> {
    values
        .into_iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

#[test]
fn writer_magic_version_flags() {
    let buf = build_minimal_valid();
    assert_eq!(&buf[0..4], b"NEOM");
    assert_eq!(read_u32_le(&buf, 4), 1); // version
    assert_eq!(read_u32_le(&buf, 8), HEADER_SIZE_V1); // v1 — no config fields
    assert_eq!(read_u32_le(&buf, 12), 0); // flags
}

#[test]
fn writer_total_size_matches_file_length() {
    let buf = build_minimal_valid();
    let total_size = read_u64_le(&buf, 88) as usize;
    assert_eq!(total_size, buf.len());
}

#[test]
fn writer_counts_correct() {
    let buf = build_full_mlp();
    assert_eq!(read_u32_le(&buf, 16), 4); // tensor_count
    assert_eq!(read_u32_le(&buf, 20), 5); // node_count
    assert_eq!(read_u32_le(&buf, 24), 1); // input_count
    assert_eq!(read_u32_le(&buf, 28), 1); // output_count
}

// ── Alignment assertions ─────────────────────────────────────────────────────

#[test]
fn weight_blob_is_page_aligned() {
    let buf = build_minimal_valid();
    let weight_blob_off = read_u64_le(&buf, 72);
    assert_eq!(
        weight_blob_off % PAGE_SIZE as u64,
        0,
        "weight blob not page-aligned"
    );
}

#[test]
fn each_weight_is_64_byte_aligned() {
    let buf = build_full_mlp();
    let tensor_count = read_u32_le(&buf, 16) as usize;
    let tensor_table_off = read_u64_le(&buf, 32) as usize;

    for i in 0..tensor_count {
        let base = tensor_table_off + i * TENSOR_ENTRY_SIZE as usize;
        let weight_off = read_u64_le(&buf, base + 72);
        assert_eq!(
            weight_off % 64,
            0,
            "tensor[{}] weight_off 0x{:X} not 64-byte aligned",
            i,
            weight_off
        );
    }
}

#[test]
fn writer_places_payloads_at_declared_aligned_offsets() {
    let buf = build_full_mlp();
    let tensor_table_off = read_u64_le(&buf, 32) as usize;
    let expected = [
        f32_payload((0..32).map(|i| i as f32)),
        f32_payload((0..8).map(|i| i as f32 * 0.1)),
        f32_payload((0..24).map(|i| i as f32 * 0.01)),
        f32_payload((0..3).map(|i| i as f32 * 0.05)),
    ];

    for (idx, expected_payload) in expected.iter().enumerate() {
        let base = tensor_table_off + idx * TENSOR_ENTRY_SIZE as usize;
        let weight_off = read_u64_le(&buf, base + 72) as usize;
        let weight_len = read_u64_le(&buf, base + 80) as usize;
        assert_eq!(weight_len, expected_payload.len());
        assert_eq!(
            &buf[weight_off..weight_off + weight_len],
            expected_payload.as_slice(),
            "tensor[{idx}] payload did not match its declared weight offset"
        );
    }
}

#[test]
fn align_up_helper() {
    assert_eq!(align_up(0, PAGE_SIZE as u64), 0);
    assert_eq!(align_up(1, PAGE_SIZE as u64), PAGE_SIZE as u64);
    assert_eq!(
        align_up(PAGE_SIZE as u64, PAGE_SIZE as u64),
        PAGE_SIZE as u64
    );
    assert_eq!(
        align_up(PAGE_SIZE as u64 + 1, PAGE_SIZE as u64),
        (PAGE_SIZE * 2) as u64
    );
    assert_eq!(align_up(63, WEIGHT_ALIGN as u64), 64);
    assert_eq!(align_up(64, WEIGHT_ALIGN as u64), 64);
    assert_eq!(align_up(65, WEIGHT_ALIGN as u64), 128);
}

// ── ISC-2: Self-validator ────────────────────────────────────────────────────

#[test]
fn validator_accepts_valid_minimal() {
    let buf = build_minimal_valid();
    assert!(
        validator::validate(&buf).is_ok(),
        "minimal valid model should pass"
    );
}

#[test]
fn validator_accepts_full_mlp() {
    let buf = build_full_mlp();
    assert!(validator::validate(&buf).is_ok(), "full MLP should pass");
}

#[test]
fn validator_rejects_bad_magic() {
    let mut buf = build_minimal_valid();
    buf[0..4].copy_from_slice(b"XXXX");
    assert_eq!(validator::validate(&buf), Err(NeomError::InvalidMagic));
}

#[test]
fn validator_rejects_bad_version() {
    let mut buf = build_minimal_valid();
    write_u32_le(&mut buf, 4, 99);
    assert_eq!(
        validator::validate(&buf),
        Err(NeomError::InvalidVersion(99))
    );
}

#[test]
fn validator_rejects_nonzero_flags() {
    let mut buf = build_minimal_valid();
    write_u32_le(&mut buf, 12, 0x01);
    assert_eq!(validator::validate(&buf), Err(NeomError::NonZeroFlags(1)));
}

#[test]
fn validator_rejects_weight_len_mismatch() {
    let mut buf = build_minimal_valid();
    let tensor_table_off = read_u64_le(&buf, 32) as usize;
    write_u64_le(&mut buf, tensor_table_off + 80, 999);
    assert!(matches!(
        validator::validate(&buf),
        Err(NeomError::WeightLenMismatch { .. })
    ));
}

#[test]
fn validator_rejects_unknown_opcode() {
    let buf = build_full_mlp();
    let mut data = buf;
    let node_table_off = read_u64_le(&data, 40) as usize;
    write_u32_le(&mut data, node_table_off, 0xFF);
    assert_eq!(
        validator::validate(&data),
        Err(NeomError::UnknownOpCode(0xFF))
    );
}

#[test]
fn validator_rejects_cyclic_graph() {
    // Build a minimal cyclic graph: 2 nodes, A produces val 2 consumed by B,
    // B produces val 3 consumed by A. IO entries declare values so ref check passes.
    let buf = NeomBuilder::new()
        .add_node(NodeSpec {
            name: "a".to_string(),
            op: OpCode::Add,
            inputs: vec![3],
            outputs: vec![2],
            attrs: String::new(),
        })
        .add_node(NodeSpec {
            name: "b".to_string(),
            op: OpCode::Add,
            inputs: vec![2],
            outputs: vec![3],
            attrs: String::new(),
        })
        .add_input(IoSpec {
            value_index: 2,
            dtype: Dtype::F32,
            dims: [1, 1, 0, 0, 0, 0, 0, 0],
            rank: 2,
            is_output: false,
        })
        .add_output(IoSpec {
            value_index: 3,
            dtype: Dtype::F32,
            dims: [1, 1, 0, 0, 0, 0, 0, 0],
            rank: 2,
            is_output: true,
        })
        .build()
        .expect("build cyclic");

    assert_eq!(
        validator::validate(&buf),
        Err(NeomError::CyclicGraph),
        "cyclic graph should be detected"
    );
}

#[test]
fn validator_rejects_weight_blob_not_page_aligned() {
    let mut buf = build_minimal_valid();
    write_u64_le(&mut buf, 72, 1000); // not page-aligned
    // Need to also update total_size so we pass the header check
    let len = buf.len() as u64;
    write_u64_le(&mut buf, 88, len);
    assert_eq!(
        validator::validate(&buf),
        Err(NeomError::WeightBlobNotPageAligned(1000))
    );
}

#[test]
fn validator_never_panics_on_short_buffer() {
    let short = vec![0u8; 10];
    let result = validator::validate(&short);
    assert!(result.is_err());
}

// ── ISC-3: Round-trip (build → serialize → re-parse → fields match) ─────────

#[test]
fn roundtrip_header_fields() {
    let buf = build_full_mlp();
    // Re-read and verify header fields
    assert_eq!(&buf[0..4], b"NEOM");
    assert_eq!(read_u32_le(&buf, 4), 1);
    assert_eq!(read_u32_le(&buf, 8), HEADER_SIZE_V1); // v1 — no config fields
    assert_eq!(read_u32_le(&buf, 16), 4); // 4 tensors
    assert_eq!(read_u32_le(&buf, 20), 5); // 5 nodes
}

#[test]
fn roundtrip_tensor_data() {
    let buf = build_full_mlp();
    let tensor_table_off = read_u64_le(&buf, 32) as usize;
    let _weight_blob_off = read_u64_le(&buf, 72) as usize;

    // First tensor W1: dtype=F32(0), rank=2, dims=[4,8]
    let base = tensor_table_off;
    assert_eq!(read_u32_le(&buf, base), 0); // F32
    assert_eq!(read_u32_le(&buf, base + 4), 2); // rank
    assert_eq!(read_u64_le(&buf, base + 8), 4); // dims[0]
    assert_eq!(read_u64_le(&buf, base + 16), 8); // dims[1]

    let w_off = read_u64_le(&buf, base + 72) as usize;
    let w_len = read_u64_le(&buf, base + 80) as usize;
    // W1 should be 128 bytes (4*8*4)
    assert_eq!(w_len, 128);
    // First element should be 0.0f32
    assert_eq!(&buf[w_off..w_off + 4], &0.0f32.to_le_bytes());
    // Second element should be 1.0f32
    assert_eq!(&buf[w_off + 4..w_off + 8], &1.0f32.to_le_bytes());
}

#[test]
fn roundtrip_node_data() {
    let buf = build_full_mlp();
    let node_table_off = read_u64_le(&buf, 40) as usize;

    // First node: MatMul (op=3), 2 inputs, 1 output
    let base = node_table_off;
    assert_eq!(read_u32_le(&buf, base), 3); // MatMul
    assert_eq!(read_u32_le(&buf, base + 4), 2); // input_count
    assert_eq!(read_u32_le(&buf, base + 8), 1); // output_count
}

// ── ISC-4: Negative corpus rejection ─────────────────────────────────────────

#[test]
fn negative_fixtures_all_rejected() {
    let tmp_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("neomc_test_fixtures");
    let _ = std::fs::create_dir_all(&tmp_dir);

    let paths: HashMap<&str, PathBuf> =
        neomc::fixtures::generate_all(&tmp_dir).expect("generate fixtures");

    // Positive fixtures (keys prefixed `valid_`) must pass validation.
    let pos_keys: Vec<&&str> = paths.keys().filter(|k| k.starts_with("valid_")).collect();
    assert!(!pos_keys.is_empty(), "should have positive fixtures");
    for &key in &pos_keys {
        let path = paths.get(key).unwrap();
        let data = std::fs::read(path).expect("read valid fixture");
        assert!(
            validator::validate(&data).is_ok(),
            "positive fixture '{}' should pass validation",
            key
        );
    }

    // Each negative (keys prefixed `neg_`) should fail.
    let neg_keys: Vec<&&str> = paths.keys().filter(|k| k.starts_with("neg_")).collect();
    assert!(!neg_keys.is_empty(), "should have negative fixtures");

    for &key in &neg_keys {
        let path = paths.get(key).unwrap();
        let data = std::fs::read(path).expect("read neg fixture");
        let result = validator::validate(&data);
        assert!(
            result.is_err(),
            "negative fixture '{}' should have failed validation",
            key
        );
    }
}

// ── Dtype / OpCode ABI consistency ───────────────────────────────────────────

#[test]
fn dtype_abi_values_match_kernel() {
    assert_eq!(Dtype::F32 as u32, 0);
    assert_eq!(Dtype::F16 as u32, 1);
    assert_eq!(Dtype::BF16 as u32, 2);
    assert_eq!(Dtype::I32 as u32, 3);
    assert_eq!(Dtype::I8 as u32, 4);
    assert_eq!(Dtype::U8 as u32, 5);
    assert_eq!(Dtype::Bool as u32, 6);
    assert_eq!(Dtype::Q5K as u32, 7);
}

#[test]
fn dtype_size_bytes() {
    assert_eq!(Dtype::F32.size_bytes(), 4);
    assert_eq!(Dtype::F16.size_bytes(), 2);
    assert_eq!(Dtype::I32.size_bytes(), 4);
    assert_eq!(Dtype::I8.size_bytes(), 1);
    assert_eq!(Dtype::U8.size_bytes(), 1);
    assert_eq!(Dtype::Bool.size_bytes(), 1);
    assert_eq!(Dtype::Q5K.size_bytes(), 1);
}

#[test]
fn opcode_abi_values() {
    assert_eq!(OpCode::Identity as u32, 0);
    assert_eq!(OpCode::Add as u32, 1);
    assert_eq!(OpCode::Mul as u32, 2);
    assert_eq!(OpCode::MatMul as u32, 3);
    assert_eq!(OpCode::Relu as u32, 4);
    assert_eq!(OpCode::Gelu as u32, 5);
    assert_eq!(OpCode::Softmax as u32, 6);
    assert_eq!(OpCode::LayerNorm as u32, 7);
    assert_eq!(OpCode::RmsNorm as u32, 8);
    assert_eq!(OpCode::Embedding as u32, 9);
    assert_eq!(OpCode::Reshape as u32, 10);
    assert_eq!(OpCode::Transpose as u32, 11);
}

#[test]
fn shape_element_count_correct() {
    let dims = [4, 8, 0, 0, 0, 0, 0, 0];
    assert_eq!(shape_element_count(2, &dims).unwrap(), 32);

    let scalar = [0; 8];
    assert_eq!(shape_element_count(0, &scalar).unwrap(), 1);
}

// ── Builder rejects bad input (no unwrap/expect on user data) ────────────────

#[test]
fn builder_returns_result_on_weight_len_mismatch() {
    let result = NeomBuilder::new()
        .add_tensor(TensorSpec {
            name: "bad".to_string(),
            dtype: Dtype::F32,
            dims: [2, 2, 0, 0, 0, 0, 0, 0],
            rank: 2,
            data: vec![0u8; 10], // should be 16 bytes for 2x2 F32
        })
        .build();
    assert!(result.is_err());
}

#[test]
fn builder_empty_model_produces_valid_file() {
    let buf = NeomBuilder::new().build().expect("empty build");
    // Even an empty model should be structurally valid
    assert!(buf.len() >= HEADER_SIZE as usize);
    assert_eq!(&buf[0..4], b"NEOM");
}
