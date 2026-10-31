// SPDX-License-Identifier: MPL-2.0
//! Test fixture generation for `.neom` v1 files.
//! Produces one valid MLP model and a set of negative fixtures, each violating
//! exactly one ADR-028 D2 rule.

use std::path::{Path, PathBuf};

use crate::format::{self, Dtype, OpCode, read_u32_le, read_u64_le, write_u32_le, write_u64_le};
use crate::writer::{IoSpec, NeomBuilder, NodeSpec, TensorSpec};

/// Deterministic splitmix64 step yielding a uniform value in `[0, 1)`. Used to
/// fill the OT-28 KAT fixture weights so the bytes are fully reproducible (the
/// committed fixture is byte-oracle'd against this generator).
fn splitmix_unit(state: &mut u64) -> f32 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    // Top 24 bits → a float in [0, 1) with 24-bit resolution.
    ((z >> 40) as f32) / ((1u32 << 24) as f32)
}

/// Small bounded weights in roughly `[-0.08, 0.08]` — large enough to make the
/// forward's next-token choice depend on the accumulated context (so a broken KV
/// cache would diverge), small enough to keep activations/logits finite.
fn kat_weight_bytes(seed: u64, count: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(count * 4);
    let mut state = seed;
    for _ in 0..count {
        let value = (splitmix_unit(&mut state) - 0.5) * 0.16;
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

/// RMS-norm weights near `1.0` (`1.0 ± 0.05`) so the normalization neither zeros
/// nor blows up the hidden state.
fn kat_norm_bytes(seed: u64, count: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(count * 4);
    let mut state = seed;
    for _ in 0..count {
        let value = 1.0 + (splitmix_unit(&mut state) - 0.5) * 0.1;
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

/// OT-28 fast≡slow KAT fixture: a minimal but forward-complete Qwen2 **inventory**
/// model (no graph nodes/ios ⇒ `is_stage_b_inventory_model` in the kernel) at the
/// smallest legal geometry — `hidden = q_proj = 128`, `head_dim = 64`,
/// `q_heads = kv_heads = 2`, `intermediate = 64`, `vocab = 16`, one layer, all F32.
/// Deterministic weights make the bytes reproducible; the committed
/// `kernel/src/ai/fixtures/kv_persist_kat.neom` is byte-oracle'd against this
/// function. The kernel `sys_generate_kv_persist_equivalence` KAT loads these
/// bytes and asserts the persistent-KV path and the reprefill baseline produce
/// bit-identical token ids.
pub fn kv_persist_kat_inventory_bytes() -> crate::format::NeomResult<Vec<u8>> {
    const HIDDEN: u64 = 128;
    const VOCAB: u64 = 16;
    const INTER: u64 = 64;
    const Q_PROJ: u64 = 128; // q_heads(2) * head_dim(64)
    const KV_PROJ: u64 = 128; // kv_heads(2) * head_dim(64)

    let rank2 = |name: &str, d0: u64, d1: u64, seed: u64| TensorSpec {
        name: name.to_string(),
        dtype: Dtype::F32,
        dims: [d0, d1, 0, 0, 0, 0, 0, 0],
        rank: 2,
        data: kat_weight_bytes(seed, (d0 * d1) as usize),
    };
    let norm = |name: &str, d0: u64, seed: u64| TensorSpec {
        name: name.to_string(),
        dtype: Dtype::F32,
        dims: [d0, 0, 0, 0, 0, 0, 0, 0],
        rank: 1,
        data: kat_norm_bytes(seed, d0 as usize),
    };

    NeomBuilder::new()
        .set_layer_count(1)
        .set_rope_theta(1_000_000.0)
        .set_rms_norm_eps(1e-6)
        .add_tensor(rank2("token_embd.weight", HIDDEN, VOCAB, 0x1001))
        .add_tensor(norm("blk.0.attn_norm.weight", HIDDEN, 0x1002))
        .add_tensor(rank2("blk.0.attn_q.weight", HIDDEN, Q_PROJ, 0x1003))
        .add_tensor(rank2("blk.0.attn_k.weight", HIDDEN, KV_PROJ, 0x1004))
        .add_tensor(rank2("blk.0.attn_v.weight", HIDDEN, KV_PROJ, 0x1005))
        .add_tensor(rank2("blk.0.attn_output.weight", Q_PROJ, HIDDEN, 0x1006))
        .add_tensor(norm("blk.0.ffn_norm.weight", HIDDEN, 0x1007))
        .add_tensor(rank2("blk.0.ffn_gate.weight", HIDDEN, INTER, 0x1008))
        .add_tensor(rank2("blk.0.ffn_up.weight", HIDDEN, INTER, 0x1009))
        .add_tensor(rank2("blk.0.ffn_down.weight", INTER, HIDDEN, 0x100A))
        .add_tensor(norm("output_norm.weight", HIDDEN, 0x100B))
        .add_tensor(rank2("output.weight", HIDDEN, VOCAB, 0x100C))
        .build()
}

/// Generate the valid MLP fixture and all negative fixtures into `output_dir`.
/// Returns a map of fixture name -> path.
pub fn generate_all(
    output_dir: &Path,
) -> std::io::Result<std::collections::HashMap<&'static str, PathBuf>> {
    std::fs::create_dir_all(output_dir)?;
    let mut paths = std::collections::HashMap::new();

    let valid_path = output_dir.join("valid_mlp.neom");
    generate_valid_mlp(&valid_path)?;
    paths.insert("valid_mlp", valid_path);

    let bad_magic = output_dir.join("neg_bad_magic.neom");
    generate_neg_bad_magic(&bad_magic)?;
    paths.insert("neg_bad_magic", bad_magic);

    let bad_version = output_dir.join("neg_bad_version.neom");
    generate_neg_bad_version(&bad_version)?;
    paths.insert("neg_bad_version", bad_version);

    let bad_flags = output_dir.join("neg_bad_flags.neom");
    generate_neg_bad_flags(&bad_flags)?;
    paths.insert("neg_bad_flags", bad_flags);

    let bad_weight_len = output_dir.join("neg_weight_len_mismatch.neom");
    generate_neg_weight_len(&bad_weight_len)?;
    paths.insert("neg_weight_len_mismatch", bad_weight_len);

    let bad_offset = output_dir.join("neg_offset_overflow.neom");
    generate_neg_offset_overflow(&bad_offset)?;
    paths.insert("neg_offset_overflow", bad_offset);

    let bad_valref = output_dir.join("neg_value_ref_oor.neom");
    generate_neg_value_ref_oor(&bad_valref)?;
    paths.insert("neg_value_ref_oor", bad_valref);

    let cyclic = output_dir.join("neg_cyclic_graph.neom");
    generate_neg_cyclic(&cyclic)?;
    paths.insert("neg_cyclic_graph", cyclic);

    let unknown_op = output_dir.join("neg_unknown_opcode.neom");
    generate_neg_unknown_op(&unknown_op)?;
    paths.insert("neg_unknown_opcode", unknown_op);

    let transformer_path = output_dir.join("valid_transformer_block.neom");
    generate_valid_transformer_block(&transformer_path)?;
    paths.insert("valid_transformer_block", transformer_path);

    let embedding_path = output_dir.join("valid_embedding.neom");
    generate_valid_embedding(&embedding_path)?;
    paths.insert("valid_embedding", embedding_path);

    let unary_chain_path = output_dir.join("valid_unary_chain.neom");
    generate_valid_unary_chain(&unary_chain_path)?;
    paths.insert("valid_unary_chain", unary_chain_path);

    let pressure_chain_path = output_dir.join("valid_pressure_chain.neom");
    generate_valid_pressure_chain(&pressure_chain_path)?;
    paths.insert("valid_pressure_chain", pressure_chain_path);

    let retain_chain_path = output_dir.join("valid_retain_chain.neom");
    generate_valid_retain_chain(&retain_chain_path)?;
    paths.insert("valid_retain_chain", retain_chain_path);

    let weighted_chain_path = output_dir.join("valid_weighted_chain.neom");
    generate_valid_weighted_chain(&weighted_chain_path)?;
    paths.insert("valid_weighted_chain", weighted_chain_path);

    Ok(paths)
}

// ── Valid unary-chain fixture (Phase 7 BUILD-P1 placement gate) ───────────────
/// A weight-free chain of element-wise unary ops on a single input:
///   input(1,8) -> Gelu -> Relu -> Gelu -> Relu -> output(1,8).
/// The model has **no weight tensors**, so the input is the sole (dominant)
/// operand of every op. The placement gate migrates that input onto an
/// accelerator; residency-aware placement then keeps the whole chain there (each
/// op's operand is the prior op's accelerator-resident output), so the policy
/// incurs zero migration while a fixed-non-resident-device baseline bounces every
/// op host↔device. Value space: 0 = input (IO), 1..3 = intermediates, 4 = output.
fn generate_valid_unary_chain(path: &Path) -> std::io::Result<()> {
    let builder = NeomBuilder::new()
        // Node 0: Gelu(input=0) -> 1
        .add_node(NodeSpec {
            name: "gelu1".to_string(),
            op: OpCode::Gelu,
            inputs: vec![0],
            outputs: vec![1],
            attrs: String::new(),
        })
        // Node 1: Relu(1) -> 2
        .add_node(NodeSpec {
            name: "relu1".to_string(),
            op: OpCode::Relu,
            inputs: vec![1],
            outputs: vec![2],
            attrs: String::new(),
        })
        // Node 2: Gelu(2) -> 3
        .add_node(NodeSpec {
            name: "gelu2".to_string(),
            op: OpCode::Gelu,
            inputs: vec![2],
            outputs: vec![3],
            attrs: String::new(),
        })
        // Node 3: Relu(3) -> 4
        .add_node(NodeSpec {
            name: "relu2".to_string(),
            op: OpCode::Relu,
            inputs: vec![3],
            outputs: vec![4],
            attrs: String::new(),
        })
        // IO: input value 0
        .add_input(IoSpec {
            value_index: 0,
            dtype: Dtype::F32,
            dims: [1, 8, 0, 0, 0, 0, 0, 0],
            rank: 2,
            is_output: false,
        })
        // IO: output value 4
        .add_output(IoSpec {
            value_index: 4,
            dtype: Dtype::F32,
            dims: [1, 8, 0, 0, 0, 0, 0, 0],
            rank: 2,
            is_output: true,
        });

    let buf = builder
        .build()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    format::write_file(&buf, path)
}

// ── Valid pressure-chain fixture (Phase 7 BUILD-P2 eviction gate) ─────────────
/// The unary chain again — `input -> Gelu -> Relu -> Gelu -> Relu` — but with
/// **48 KiB tensors** (1×12288 F32), sized against accel_sim's 256 KiB arena so a
/// placed forward pass *cannot* keep everything resident: input + 4 outputs +
/// the driver's transient dispatch staging exceed capacity, forcing the kernel
/// to evict the LRU unpinned intermediates (BUILD-P2) while staying within the
/// §9.6 dispatch quota (4 ops). Weight-free, like `valid_unary_chain`, so the
/// resident set is exactly the input and the chain's intermediates.
fn generate_valid_pressure_chain(path: &Path) -> std::io::Result<()> {
    let builder = NeomBuilder::new()
        // Node 0: Gelu(input=0) -> 1
        .add_node(NodeSpec {
            name: "gelu1".to_string(),
            op: OpCode::Gelu,
            inputs: vec![0],
            outputs: vec![1],
            attrs: String::new(),
        })
        // Node 1: Relu(1) -> 2
        .add_node(NodeSpec {
            name: "relu1".to_string(),
            op: OpCode::Relu,
            inputs: vec![1],
            outputs: vec![2],
            attrs: String::new(),
        })
        // Node 2: Gelu(2) -> 3
        .add_node(NodeSpec {
            name: "gelu2".to_string(),
            op: OpCode::Gelu,
            inputs: vec![2],
            outputs: vec![3],
            attrs: String::new(),
        })
        // Node 3: Relu(3) -> 4
        .add_node(NodeSpec {
            name: "relu2".to_string(),
            op: OpCode::Relu,
            inputs: vec![3],
            outputs: vec![4],
            attrs: String::new(),
        })
        // IO: input value 0 — (1, 12288) F32 = 49152 bytes (48 KiB)
        .add_input(IoSpec {
            value_index: 0,
            dtype: Dtype::F32,
            dims: [1, 12288, 0, 0, 0, 0, 0, 0],
            rank: 2,
            is_output: false,
        })
        // IO: output value 4
        .add_output(IoSpec {
            value_index: 4,
            dtype: Dtype::F32,
            dims: [1, 12288, 0, 0, 0, 0, 0, 0],
            rank: 2,
            is_output: true,
        });

    let buf = builder
        .build()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    format::write_file(&buf, path)
}

// ── Valid retain-chain fixture (Phase 7 BUILD-P3 retention gate) ──────────────
/// A reuse graph sized against accel_sim's 256 KiB arena with **40 KiB tensors**
/// (1×10240 F32): `input -> Gelu -> a -> Relu -> b -> Gelu -> c -> Relu -> d`,
/// then `Add(a, d) -> e`. Op 5 reuses `a`, so at op 4's memory pressure the
/// LRU-coldest evictable tensor (`a`, last touched at op 2) is exactly the
/// tensor the next op needs — the net-negative eviction BUILD-P3's W=1
/// lookahead refuses (`residency: RETAINED`), evicting the colder `b` instead.
/// `b` is a **declared model output** (alongside `e`) so it has no remaining
/// *op* use yet is never proactively demoted (the output-protection rule),
/// which keeps the pre-pressure seams demote-free and lets the pressure arrive.
/// Value space: 0 = input, 1 = a, 2 = b (output), 3 = c, 4 = d, 5 = e (output).
fn generate_valid_retain_chain(path: &Path) -> std::io::Result<()> {
    let builder = NeomBuilder::new()
        // Node 0: Gelu(input=0) -> 1 (a)
        .add_node(NodeSpec {
            name: "gelu1".to_string(),
            op: OpCode::Gelu,
            inputs: vec![0],
            outputs: vec![1],
            attrs: String::new(),
        })
        // Node 1: Relu(1) -> 2 (b — declared output)
        .add_node(NodeSpec {
            name: "relu1".to_string(),
            op: OpCode::Relu,
            inputs: vec![1],
            outputs: vec![2],
            attrs: String::new(),
        })
        // Node 2: Gelu(2) -> 3 (c)
        .add_node(NodeSpec {
            name: "gelu2".to_string(),
            op: OpCode::Gelu,
            inputs: vec![2],
            outputs: vec![3],
            attrs: String::new(),
        })
        // Node 3: Relu(3) -> 4 (d) — the pressure op; a and b are evictable
        .add_node(NodeSpec {
            name: "relu2".to_string(),
            op: OpCode::Relu,
            inputs: vec![3],
            outputs: vec![4],
            attrs: String::new(),
        })
        // Node 4: Add(1, 4) -> 5 (e) — reuses a: the retention target
        .add_node(NodeSpec {
            name: "add_reuse".to_string(),
            op: OpCode::Add,
            inputs: vec![1, 4],
            outputs: vec![5],
            attrs: String::new(),
        })
        // IO: input value 0 — (1, 10240) F32 = 40960 bytes (40 KiB)
        .add_input(IoSpec {
            value_index: 0,
            dtype: Dtype::F32,
            dims: [1, 10240, 0, 0, 0, 0, 0, 0],
            rank: 2,
            is_output: false,
        })
        // IO: output value 2 (b)
        .add_output(IoSpec {
            value_index: 2,
            dtype: Dtype::F32,
            dims: [1, 10240, 0, 0, 0, 0, 0, 0],
            rank: 2,
            is_output: true,
        })
        // IO: output value 5 (e)
        .add_output(IoSpec {
            value_index: 5,
            dtype: Dtype::F32,
            dims: [1, 10240, 0, 0, 0, 0, 0, 0],
            rank: 2,
            is_output: true,
        });

    let buf = builder
        .build()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    format::write_file(&buf, path)
}

// ── Valid weighted-chain fixture (Phase 7 BUILD-O1b production-payoff gate) ──
/// An Add chain with four real weights: `out = in + W1 + W2 + W3 + W4` on
/// **4 KiB tensors** (1×1024 F32) — sized so the whole `.neom` file fits
/// NeoFS's 32 KiB direct-block cap (`DIRECT_BLOCKS = 8`). Loaded through the
/// slot arena (BUILD-O1b), the weights are the §9.12 daemon's population:
/// prefetched when published-hot, pinned while a pass mints them (I-9), and
/// demoted when idle under another tenant's arena pressure (the tenant
/// supplies the big residency — pressure needs no file). Weight contents
/// are distinct ramps so byte oracles stay unambiguous (the gate loads with
/// dedup off — the `None` path).
/// Value space: 0..3 = W1..W4 (weights), 4 = input, 5..7 = x1..x3, 8 = output.
fn generate_valid_weighted_chain(path: &Path) -> std::io::Result<()> {
    const ELEMENTS: usize = 1024; // (1, 1024) F32 = 4096 bytes (4 KiB)
    let weight_data = |seed: u32| -> Vec<u8> {
        (0..ELEMENTS)
            .flat_map(|i| (((i as u32 % 97) + seed) as f32 * 0.001).to_le_bytes())
            .collect()
    };
    let mut builder = NeomBuilder::new();
    for (idx, name) in ["W1", "W2", "W3", "W4"].iter().enumerate() {
        builder = builder.add_tensor(TensorSpec {
            name: (*name).to_string(),
            dtype: Dtype::F32,
            dims: [1, ELEMENTS as u64, 0, 0, 0, 0, 0, 0],
            rank: 2,
            data: weight_data(idx as u32 + 1),
        });
    }
    let chain = [(4u32, 0u32, 5u32), (5, 1, 6), (6, 2, 7), (7, 3, 8)];
    for (i, (lhs, weight, out)) in chain.iter().enumerate() {
        builder = builder.add_node(NodeSpec {
            name: format!("add{}", i + 1),
            op: OpCode::Add,
            inputs: vec![*lhs, *weight],
            outputs: vec![*out],
            attrs: String::new(),
        });
    }
    let builder = builder
        .add_input(IoSpec {
            value_index: 4,
            dtype: Dtype::F32,
            dims: [1, ELEMENTS as u64, 0, 0, 0, 0, 0, 0],
            rank: 2,
            is_output: false,
        })
        .add_output(IoSpec {
            value_index: 8,
            dtype: Dtype::F32,
            dims: [1, ELEMENTS as u64, 0, 0, 0, 0, 0, 0],
            rank: 2,
            is_output: true,
        });
    let buf = builder
        .build()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    format::write_file(&buf, path)
}

// ── Valid MLP fixture ────────────────────────────────────────────────────────
/// Small 2-layer MLP: input(1,4) -> MatMul(W1:4x8) -> Add(b1:8) -> Relu ->
///   MatMul(W2:8x3) -> Add(b2:3) -> output(1,3).
/// Value space:
///   0 = W1 (tensor), 1 = b1 (tensor), 2 = W2 (tensor), 3 = b2 (tensor)
///   4 = input (IO input), 5 = after W1 matmul, 6 = after b1 add,
///   7 = after relu, 8 = after W2 matmul, 9 = output (IO output)
fn generate_valid_mlp(path: &Path) -> std::io::Result<()> {
    // W1: 4x8 F32
    let w1_data: Vec<u8> = (0..32).flat_map(|i| (i as f32).to_le_bytes()).collect();
    // b1: 1x8 F32
    let b1_data: Vec<u8> = (0..8)
        .flat_map(|i| (i as f32 * 0.1).to_le_bytes())
        .collect();
    // W2: 8x3 F32
    let w2_data: Vec<u8> = (0..24)
        .flat_map(|i| (i as f32 * 0.01).to_le_bytes())
        .collect();
    // b2: 1x3 F32
    let b2_data: Vec<u8> = (0..3)
        .flat_map(|i| (i as f32 * 0.05).to_le_bytes())
        .collect();

    let builder = NeomBuilder::new()
        .add_tensor(TensorSpec {
            name: "W1".to_string(),
            dtype: Dtype::F32,
            dims: [4, 8, 0, 0, 0, 0, 0, 0],
            rank: 2,
            data: w1_data,
        })
        .add_tensor(TensorSpec {
            name: "b1".to_string(),
            dtype: Dtype::F32,
            dims: [1, 8, 0, 0, 0, 0, 0, 0],
            rank: 2,
            data: b1_data,
        })
        .add_tensor(TensorSpec {
            name: "W2".to_string(),
            dtype: Dtype::F32,
            dims: [8, 3, 0, 0, 0, 0, 0, 0],
            rank: 2,
            data: w2_data,
        })
        .add_tensor(TensorSpec {
            name: "b2".to_string(),
            dtype: Dtype::F32,
            dims: [1, 3, 0, 0, 0, 0, 0, 0],
            rank: 2,
            data: b2_data,
        })
        // Node 0: MatMul(input, W1) -> out5
        .add_node(NodeSpec {
            name: "mm1".to_string(),
            op: OpCode::MatMul,
            inputs: vec![4, 0],
            outputs: vec![5],
            attrs: String::new(),
        })
        // Node 1: Add(out5, b1) -> out6
        .add_node(NodeSpec {
            name: "add1".to_string(),
            op: OpCode::Add,
            inputs: vec![5, 1],
            outputs: vec![6],
            attrs: String::new(),
        })
        // Node 2: Relu(out6) -> out7
        .add_node(NodeSpec {
            name: "relu1".to_string(),
            op: OpCode::Relu,
            inputs: vec![6],
            outputs: vec![7],
            attrs: String::new(),
        })
        // Node 3: MatMul(out7, W2) -> out8
        .add_node(NodeSpec {
            name: "mm2".to_string(),
            op: OpCode::MatMul,
            inputs: vec![7, 2],
            outputs: vec![8],
            attrs: String::new(),
        })
        // Node 4: Add(out8, b2) -> out9
        .add_node(NodeSpec {
            name: "add2".to_string(),
            op: OpCode::Add,
            inputs: vec![8, 3],
            outputs: vec![9],
            attrs: String::new(),
        })
        // IO: input value 4
        .add_input(IoSpec {
            value_index: 4,
            dtype: Dtype::F32,
            dims: [1, 4, 0, 0, 0, 0, 0, 0],
            rank: 2,
            is_output: false,
        })
        // IO: output value 9
        .add_output(IoSpec {
            value_index: 9,
            dtype: Dtype::F32,
            dims: [1, 3, 0, 0, 0, 0, 0, 0],
            rank: 2,
            is_output: true,
        });

    let buf = builder
        .build()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    format::write_file(&buf, path)
}

// ── Negative fixtures ────────────────────────────────────────────────────────

fn generate_neg_bad_magic(path: &Path) -> std::io::Result<()> {
    let buf = build_valid_bytes()?;
    let mut data = buf;
    data[0..4].copy_from_slice(b"XXXX");
    format::write_file(&data, path)
}

fn generate_neg_bad_version(path: &Path) -> std::io::Result<()> {
    let buf = build_valid_bytes()?;
    let mut data = buf;
    write_u32_le(&mut data, 4, 99);
    format::write_file(&data, path)
}

fn generate_neg_bad_flags(path: &Path) -> std::io::Result<()> {
    let buf = build_valid_bytes()?;
    let mut data = buf;
    write_u32_le(&mut data, 12, 0x01);
    format::write_file(&data, path)
}

fn generate_neg_weight_len(path: &Path) -> std::io::Result<()> {
    let buf = build_valid_bytes()?;
    let mut data = buf;
    // Corrupt weight_len of first tensor (offset 80 in first TensorEntry)
    let tensor_table_off = read_u64_le(&data, 32) as usize;
    let wlen_off = tensor_table_off + 80;
    write_u64_le(&mut data, wlen_off, 999);
    // Also update total_size won't matter — validator catches weight_len first
    format::write_file(&data, path)
}

fn generate_neg_offset_overflow(path: &Path) -> std::io::Result<()> {
    let buf = build_valid_bytes()?;
    let mut data = buf;
    // Set tensor_table_off past total_size
    write_u64_le(&mut data, 32, u64::MAX);
    format::write_file(&data, path)
}

fn generate_neg_value_ref_oor(path: &Path) -> std::io::Result<()> {
    let buf = build_valid_bytes()?;
    let mut data = buf;
    // Corrupt the first input ref of the first node to a huge value
    let node_table_off = read_u64_le(&data, 40) as usize;
    let inputs_off = read_u32_le(&data, node_table_off + 12) as usize;
    write_u32_le(&mut data, inputs_off, 99999);
    format::write_file(&data, path)
}

fn generate_neg_cyclic(path: &Path) -> std::io::Result<()> {
    // Build a minimal model with an explicit cycle: A produces val 2 consumed by B,
    // B produces val 3 consumed by A.
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
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    format::write_file(&buf, path)
}

fn generate_neg_unknown_op(path: &Path) -> std::io::Result<()> {
    let buf = build_valid_bytes()?;
    let mut data = buf;
    let node_table_off = read_u64_le(&data, 40) as usize;
    write_u32_le(&mut data, node_table_off, 0xFF);
    format::write_file(&data, path)
}

// ── Valid transformer block fixture ──────────────────────────────────────────
/// Minimal transformer block: input(1,4) -> LayerNorm(g,b) -> MatMul(W:4x4) ->
///   Gelu -> RmsNorm(g2) -> Softmax -> output(1,4).
/// Value space:
///   0 = gamma (4), 1 = beta (4), 2 = W (4x4), 3 = gamma2 (4)
///   4 = input (IO), 5 = after LayerNorm, 6 = after MatMul,
///   7 = after Gelu, 8 = after RmsNorm, 9 = after Softmax/output (IO)
fn generate_valid_transformer_block(path: &Path) -> std::io::Result<()> {
    let gamma_data: Vec<u8> = (0..4).flat_map(|_| 1.0f32.to_le_bytes()).collect();
    let beta_data: Vec<u8> = (0..4).flat_map(|_| 0.0f32.to_le_bytes()).collect();
    let w_data: Vec<u8> = (0..16)
        .flat_map(|i| (i as f32 * 0.1).to_le_bytes())
        .collect();
    let gamma2_data: Vec<u8> = (0..4).flat_map(|_| 1.0f32.to_le_bytes()).collect();

    let builder = NeomBuilder::new()
        .add_tensor(TensorSpec {
            name: "gamma".to_string(),
            dtype: Dtype::F32,
            dims: [4, 0, 0, 0, 0, 0, 0, 0],
            rank: 1,
            data: gamma_data,
        })
        .add_tensor(TensorSpec {
            name: "beta".to_string(),
            dtype: Dtype::F32,
            dims: [4, 0, 0, 0, 0, 0, 0, 0],
            rank: 1,
            data: beta_data,
        })
        .add_tensor(TensorSpec {
            name: "W".to_string(),
            dtype: Dtype::F32,
            dims: [4, 4, 0, 0, 0, 0, 0, 0],
            rank: 2,
            data: w_data,
        })
        .add_tensor(TensorSpec {
            name: "gamma2".to_string(),
            dtype: Dtype::F32,
            dims: [4, 0, 0, 0, 0, 0, 0, 0],
            rank: 1,
            data: gamma2_data,
        })
        .add_node(NodeSpec {
            name: "ln1".to_string(),
            op: OpCode::LayerNorm,
            inputs: vec![4, 0, 1],
            outputs: vec![5],
            attrs: String::new(),
        })
        .add_node(NodeSpec {
            name: "mm1".to_string(),
            op: OpCode::MatMul,
            inputs: vec![5, 2],
            outputs: vec![6],
            attrs: String::new(),
        })
        .add_node(NodeSpec {
            name: "gelu1".to_string(),
            op: OpCode::Gelu,
            inputs: vec![6],
            outputs: vec![7],
            attrs: String::new(),
        })
        .add_node(NodeSpec {
            name: "rms1".to_string(),
            op: OpCode::RmsNorm,
            inputs: vec![7, 3],
            outputs: vec![8],
            attrs: String::new(),
        })
        .add_node(NodeSpec {
            name: "sm1".to_string(),
            op: OpCode::Softmax,
            inputs: vec![8],
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
            dims: [1, 4, 0, 0, 0, 0, 0, 0],
            rank: 2,
            is_output: true,
        });

    let buf = builder
        .build()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    format::write_file(&buf, path)
}

/// Minimal embedding gather:
///   indices I32[3] -> Embedding(table F32[4,3]) -> output F32[3,3].
/// Value space:
///   0 = table (tensor), 1 = indices (IO), 2 = output (IO)
fn generate_valid_embedding(path: &Path) -> std::io::Result<()> {
    let table_data: Vec<u8> = [
        0x3f80_0000,
        0x4000_0000,
        0x4040_0000,
        0x4080_0000,
        0x40a0_0000,
        0x40c0_0000,
        0xbf80_0000,
        0xc000_0000,
        0xc040_0000,
        0x3dcc_cccd,
        0x3e4c_cccd,
        0x3e99_999a,
    ]
    .into_iter()
    .flat_map(u32::to_le_bytes)
    .collect();

    let builder = NeomBuilder::new()
        .add_tensor(TensorSpec {
            name: "embedding_table".to_string(),
            dtype: Dtype::F32,
            dims: [4, 3, 0, 0, 0, 0, 0, 0],
            rank: 2,
            data: table_data,
        })
        .add_node(NodeSpec {
            name: "embedding".to_string(),
            op: OpCode::Embedding,
            inputs: vec![1, 0],
            outputs: vec![2],
            attrs: String::new(),
        })
        .add_input(IoSpec {
            value_index: 1,
            dtype: Dtype::I32,
            dims: [3, 0, 0, 0, 0, 0, 0, 0],
            rank: 1,
            is_output: false,
        })
        .add_output(IoSpec {
            value_index: 2,
            dtype: Dtype::F32,
            dims: [3, 3, 0, 0, 0, 0, 0, 0],
            rank: 2,
            is_output: true,
        });

    let buf = builder
        .build()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    format::write_file(&buf, path)
}

/// Build a valid MLP file to bytes (same as generate_valid_mlp but returns Vec).
fn build_valid_bytes() -> std::io::Result<Vec<u8>> {
    let tmp = PathBuf::from("/tmp/_neomc_tmp.neom");
    generate_valid_mlp(&tmp)?;
    let data = std::fs::read(&tmp)?;
    let _ = std::fs::remove_file(&tmp);
    Ok(data)
}
