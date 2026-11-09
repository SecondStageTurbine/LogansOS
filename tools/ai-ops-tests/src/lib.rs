// SPDX-License-Identifier: MPL-2.0
//! Host tests for Phase 5 CPU inference op kernels.
#![allow(
    dead_code,
    clippy::approx_constant,
    clippy::bool_assert_comparison,
    clippy::doc_lazy_continuation,
    clippy::excessive_precision,
    clippy::identity_op,
    clippy::manual_is_multiple_of,
    clippy::needless_range_loop,
    clippy::useless_vec
)]

#[path = "../../../kernel/src/drivers/accel/accounting_table.rs"]
mod accounting_table;
#[path = "../../../kernel/src/ai/admission_policy.rs"]
mod admission_policy;
#[path = "../../../userland/neo-arthur/src/agentic_action_logic.rs"]
mod agentic_action_logic;
#[path = "../../../kernel/src/ai/arena_logic.rs"]
mod arena_logic;
#[path = "../../../kernel/src/ai/decode_logic.rs"]
mod decode_logic;
#[path = "../../../kernel/src/drivers/dma_arena_logic.rs"]
mod dma_arena_logic;
#[path = "../../../kernel/src/acpi/dmar_logic.rs"]
mod dmar_logic;
#[path = "../../../kernel/src/ai/evict_policy.rs"]
mod evict_policy;
#[path = "../../../kernel/src/drivers/fb/console_logic.rs"]
mod fb_console_logic;
#[path = "../../../kernel/src/ai/forward_logic.rs"]
mod forward_logic;
#[path = "../../../kernel/src/ai/generate_logic.rs"]
mod generate_logic;
#[path = "../../../kernel/src/ai/gguf_dequant_logic.rs"]
mod gguf_dequant_logic;
#[path = "../../../kernel/src/ai/gguf_logic.rs"]
mod gguf_logic;
#[path = "../../../kernel/src/drivers/accel/intel_gen9_4a.rs"]
mod intel_gen9_4a;
#[path = "../../../kernel/src/drivers/accel/intel_gen9_execlist.rs"]
mod intel_gen9_execlist;
#[path = "../../../kernel/src/drivers/accel/intel_gen9_ppgtt.rs"]
mod intel_gen9_ppgtt;
#[path = "../../../kernel/src/ai/quant_timing_logic.rs"]
mod quant_timing_logic;
#[path = "../../../kernel/src/ai/quant_timing_prompts.rs"]
mod quant_timing_prompts;
#[cfg(test)]
mod quant_timing_tests;
// The Intel accel device-ID allowlist. QEMU has no Intel iGPU, so the 4-shard
// QEMU gate structurally cannot exercise an allowlist — every scenario reports
// `candidates=0` regardless of the table. That gap escaped to bare metal (the
// M710q's first Slice-4 run burned a boot cycle on a missing `0x1912`), so the
// check is promoted to a host KAT here.
#[path = "../../../kernel/src/drivers/accel/accel_intel_ids.rs"]
mod accel_intel_ids;
#[path = "../../../kernel/src/ai/kv_layout.rs"]
mod kv_layout;
#[path = "../../../kernel/src/ai/model_logic.rs"]
mod model_logic;
#[path = "../../../kernel/src/ai/prefill_perf_logic.rs"]
mod prefill_perf_logic;
mod qwen35_llama_cpp_golden {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/qwen35_llama_cpp_hello_golden.rs"
    ));
}
#[path = "../../../kernel/src/ai/ops/mod.rs"]
mod ops;
// OT-34 F1: the pure fork-join barrier-decision policy — host-KAT'd so the
// fail-stop rule (claimed+unfinished+past-limit ⇒ Halt, never a skip) is a
// regression, not just reasoning.
#[path = "../../../kernel/src/ai/par_matmul_logic.rs"]
mod par_matmul_logic;
// Arthur latency round 2: CPUID/MSR decoding for the APERF/MPERF clock and
// the HWP request.
#[path = "../../../kernel/src/arch/x86_64/cpufreq_logic.rs"]
mod cpufreq_logic;
#[path = "../../../kernel/src/ai/placement_policy.rs"]
mod placement_policy;
#[path = "../../../kernel/src/ai/pointer_window_logic.rs"]
mod pointer_window_logic;
#[path = "../../../kernel/src/ai/predictor.rs"]
mod predictor;
#[path = "../../../kernel/src/drivers/accel/recency_table.rs"]
mod recency_table;
#[path = "../../../kernel/src/ai/repetition_logic.rs"]
mod repetition_logic;
#[path = "../../../kernel/src/ai/reservation_logic.rs"]
mod reservation_logic;
#[path = "../../../kernel/src/ai/sharing_logic.rs"]
mod sharing_logic;
#[path = "../../../kernel/src/ai/tensor_logic.rs"]
mod tensor_logic;
#[path = "../../../kernel/src/ai/tokenizer_logic.rs"]
mod tokenizer_logic;
#[path = "../../../kernel/src/ai/vocab_bin_logic.rs"]
mod vocab_bin_logic;
// OT-37: Arthur's boot-time chat-template / real-vocab tokenizer, mounted so the
// host harness can run the EXACT `frame_turn_ids` the kernel uses at boot and
// assert it reproduces the prompt the coherence proof runs on (ISC-B1). Depends
// on `tokenizer_logic` above (resolved at this crate root).
#[path = "../../../kernel/src/atrium/input_logic.rs"]
mod atrium_input_logic;
#[path = "../../../kernel/src/ai/commit_slot_logic.rs"]
mod commit_slot_logic;
#[path = "../../../kernel/src/drivers/usb/hid_logic.rs"]
mod usb_hid_logic;
#[path = "../../../kernel/src/drivers/usb/usb_parse.rs"]
mod usb_parse;
#[path = "../../../userland/neo-arthur/src/vocab_sidecar.rs"]
mod vocab_sidecar;
#[path = "../../../kernel/src/drivers/iommu/vtd_logic.rs"]
mod vtd_logic;
#[path = "../../../kernel/src/ai/watermark.rs"]
mod watermark;
#[path = "../../../kernel/src/drivers/usb/xhci_ring_logic.rs"]
mod xhci_ring_logic;
// Phase 7.5 BUILD-W0: the WPA2/3 supplicant crypto library (path-mod'd as a
// module tree — wpa/mod.rs's submodules resolve relative to the kernel path).
#[path = "../../../kernel/src/net/wpa/mod.rs"]
mod wpa;
// Phase 7.5 BUILD-W1: the MT7601U driver's pure-logic cores (host-oracle'd —
// the on-wire/on-chip byte layouts the device-touching loader rides on, I-12).
// Declared flat so each module's `use super::{regs,fw_logic}` resolves to these
// crate-root siblings (same as the in-kernel `mt7601u` module layout).
#[path = "../../../kernel/src/drivers/usb/mt7601u/eeprom_logic.rs"]
mod eeprom_logic;
#[path = "../../../kernel/src/drivers/usb/mt7601u/fw_logic.rs"]
mod fw_logic;
#[path = "../../../kernel/src/drivers/usb/mt7601u/initvals.rs"]
mod initvals;
#[path = "../../../kernel/src/drivers/usb/mt7601u/linux_parity_logic.rs"]
mod linux_parity_logic;
#[path = "../../../kernel/src/drivers/usb/mt7601u/mcu_logic.rs"]
mod mcu_logic;
#[path = "../../../kernel/src/drivers/usb/mt7601u/regs.rs"]
mod regs;
#[path = "../../../kernel/src/drivers/usb/mt7601u/rx_logic.rs"]
mod rx_logic;
// Phase 7.5 BUILD-W2 ISC-1 (TX path): the MT7601U TXWI + DMA-wrap logic core.
#[path = "../../../kernel/src/drivers/usb/mt7601u/tx_logic.rs"]
mod tx_logic;
// Phase 7.5 BUILD-W2 ISC-2: the WCID key-install IV/ATTR encoding.
#[path = "../../../kernel/src/drivers/usb/mt7601u/key_logic.rs"]
mod key_logic;
#[path = "../../../kernel/src/drivers/usb/mt7601u/link_result_logic.rs"]
mod link_result_logic;
// Phase 7.5 OT-27a (cold firmware self-upload) ISC-1/ISC-3: the cold-upload
// fault classifier (the blind-hardware discriminator bank as pure logic).
#[path = "../../../kernel/src/drivers/usb/operation_budget_logic.rs"]
mod operation_budget_logic;
#[path = "../../../kernel/src/drivers/usb/mt7601u/post_fw_logic.rs"]
mod post_fw_logic;
#[path = "../../../kernel/src/drivers/usb/mt7601u/upload_logic.rs"]
mod upload_logic;
// OT-39: host-testable policy for first-targeted-tune CAL_BW deferral and the
// resident-firmware resync branch. Hardware timing remains silicon-only.
#[path = "../../../kernel/src/drivers/usb/mt7601u/wedge_logic.rs"]
mod wedge_logic;
// Phase 7.5 BUILD-W2 ISC-1: the MLME mgmt-frame build/parse logic core.
#[path = "../../../kernel/src/net/wifi/mlme_logic.rs"]
mod mlme_logic;

#[cfg(test)]
mod tests {
    use super::accel_intel_ids;
    use super::agentic_action_logic::{
        AgenticAction, AgenticActionKind, fallback_agentic_action, push_agentic_candidate_asts,
        render_agentic_ast,
    };
    use super::forward_logic::{
        self, ForwardLayerError, Qwen2KvCacheLayer, Qwen2KvCacheSet, Qwen2LayerConfig,
        Qwen2LayerDtypes, Qwen2LayerWeights, Qwen2OutputWeights, Qwen2QuantizedLayerWeights,
        QwenAttentionNormWeights, TensorDtype,
    };
    use super::generate_logic::{
        DecodeConfig, Pcg32, SamplingConfig, argmax_token, constrained_argmax_token,
        generate_tokens_with, logits_byte_range, masked_argmax_token, read_logits_at_position,
        sample_next_token,
    };
    use super::gguf_dequant_logic;
    use super::gguf_logic::{
        BLOCK_Q4_K_SIZE, BLOCK_Q8_0_SIZE, GGML_MAX_DIMS, GgmlType, GgufFile, GgufTensorInfo,
        GgufValue, K_SCALE_SIZE, QK_K, QK8_0, parse_gguf, parse_q4_k_blocks, parse_q8_0_blocks,
    };
    use super::intel_gen9_4a;
    use super::model_logic::{
        Header, HeaderConfig, ModelConfig, ModelFormatError, NEOM_MAX_FILE_SIZE, NEOM_MAX_TENSORS,
        NeomDtype, QWEN2_DEFAULT_CONTEXT, ShapeMeta, TensorMeta, ValidatedNeom,
        derive_model_config, validate_neom,
    };
    use super::ops::{self, OpKind, TensorView, soft_f32};
    use super::tensor_logic::{Dtype, Shape};
    use super::tokenizer_logic::{BpeMergeEntry, BpeTokenizer, BpeVocabEntry};

    // ── Phase 7.5 BUILD-W0: WPA2/3 crypto KATs (published IEEE 802.11 / RFC
    // known-answer vectors — I-12, NEVER round-trip) ─────────────────────────
    /// Decode a hex string to bytes — the KAT-vector helper.
    fn hex_bytes(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("valid hex"))
            .collect()
    }

    // ── Phase 8 BUILD-8.1: generation / sampler KATs ─────────────────────
    /// ISC-3: PCG32 XSH-RR sequence pinned against the published `pcg32-demo`
    /// fixed-seed output from pcg-random.org / pcg-c-basic:
    /// seed `(initstate=42, initseq=54)` first round, six 32-bit values.
    #[test]
    fn generate_isc3_pcg32_published_sequence_kat() {
        let mut rng = Pcg32::seeded(42, 54);
        let got = [
            rng.next_u32(),
            rng.next_u32(),
            rng.next_u32(),
            rng.next_u32(),
            rng.next_u32(),
            rng.next_u32(),
        ];
        assert_eq!(
            got,
            [
                0xa15c_02b7,
                0x7b47_f409,
                0xba1d_3330,
                0x83d2_f293,
                0xbfa4_784b,
                0xcbed_606e,
            ]
        );
    }

    /// ISC-2: hand-computed sampler outputs. Greedy keeps the lowest token on
    /// ties. Seeded sampling uses the PCG KAT's first draw, so the expected
    /// token is a fixed known answer, not a round-trip.
    #[test]
    fn generate_isc2_sampler_modes_kat() {
        let mut greedy_rng = Pcg32::seeded(42, 54);
        assert_eq!(
            sample_next_token(
                &[0.5, 2.0, 2.0, -1.0],
                SamplingConfig::greedy(),
                &mut greedy_rng,
            )
            .unwrap(),
            1
        );

        let mut temp_rng = Pcg32::seeded(42, 54);
        assert_eq!(
            sample_next_token(
                &[0.0, 1.0, 2.0],
                SamplingConfig::sample(0.5, None, 1.0),
                &mut temp_rng,
            )
            .unwrap(),
            2
        );

        let mut top_k_rng = Pcg32::seeded(42, 54);
        assert_eq!(
            sample_next_token(
                &[0.0, 1.0, 3.0, 4.0],
                SamplingConfig::sample(1.0, Some(2), 1.0),
                &mut top_k_rng,
            )
            .unwrap(),
            3
        );

        let mut top_p_rng = Pcg32::seeded(42, 54);
        assert_eq!(
            sample_next_token(
                &[4.0, 3.9, 0.0, 0.0],
                SamplingConfig::sample(1.0, None, 0.75),
                &mut top_p_rng,
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn agentic_constrained_argmax_masks_invalid_ast_tokens() {
        let read_ast = [10, 20, 30];
        let list_ast = [10, 21, 31];
        let clarify_ast = [11, 40];
        let noop_ast = [12];
        let candidates: [&[u32]; 4] = [&read_ast, &list_ast, &clarify_ast, &noop_ast];
        let mut logits = vec![-100.0; 64];

        // Step 0: token 63 is garbage with the highest score, but only
        // 10/11/12 can start one of the action ASTs.
        logits[63] = 10_000.0;
        logits[22] = 9_000.0;
        logits[10] = 3.0;
        logits[11] = 2.0;
        logits[12] = 1.0;
        assert_eq!(
            constrained_argmax_token(&logits, &[], &candidates).unwrap(),
            10
        );

        // Step 1 after the shared action prefix: a forbidden write-like token
        // cannot win even when it dominates the raw logits.
        logits.fill(-100.0);
        logits[22] = 10_000.0;
        logits[20] = 3.0;
        logits[21] = 4.0;
        assert_eq!(
            constrained_argmax_token(&logits, &[10], &candidates).unwrap(),
            21
        );

        logits.fill(-100.0);
        logits[63] = 10_000.0;
        logits[31] = 1.0;
        assert_eq!(
            constrained_argmax_token(&logits, &[10, 21], &candidates).unwrap(),
            31
        );
    }

    #[test]
    fn agentic_constrained_argmax_fails_closed_on_dead_prefix() {
        let read_ast = [10, 20, 30];
        let candidates: [&[u32]; 1] = [&read_ast];
        let logits = vec![1.0; 64];

        assert_eq!(
            constrained_argmax_token(&logits, &[10, 99], &candidates),
            Err(super::generate_logic::GenerateLogicError::InvalidProbability)
        );
    }

    #[test]
    fn agentic_masked_argmax_byte_oracle_rejects_invalid_tokens() {
        let logits = [0.2, 9.0, 0.4, 8.0, 3.0, 7.5, -2.0, 6.0, 5.0];
        let mask = [0b0010_1001, 0b0000_0001];

        assert_eq!(
            masked_argmax_token(&logits, Some(&mask), logits.len()).unwrap(),
            3
        );
        assert_eq!(masked_argmax_token(&logits, None, 0).unwrap(), 1);

        assert_eq!(
            masked_argmax_token(&logits, Some(&mask[..1]), logits.len()),
            Err(super::generate_logic::GenerateLogicError::InvalidMask)
        );

        let hostile_mask = [0b0010_1001, 0b1111_1111];
        assert_eq!(
            masked_argmax_token(&logits, Some(&hostile_mask), logits.len()),
            Err(super::generate_logic::GenerateLogicError::InvalidMask)
        );
    }

    #[test]
    fn agentic_candidate_set_lets_model_logits_drive_ast_choice() {
        let available = [AgenticActionKind::ReadFile, AgenticActionKind::ListDir];
        let hello_prompt = "read hello";
        let missing_prompt = "read missing";
        assert_eq!(
            fallback_agentic_action(hello_prompt, &available),
            AgenticAction::ReadFile { path: "/hello.txt" }
        );
        assert_eq!(
            fallback_agentic_action(missing_prompt, &available),
            AgenticAction::ReadFile { path: "/hello.txt" }
        );

        let mut asts = Vec::new();
        push_agentic_candidate_asts(hello_prompt, &available, &mut asts).unwrap();
        assert!(
            asts.len() > 1,
            "grammar candidate set must not collapse to the fallback AST"
        );

        let hello_ast = render_ast_for_test(AgenticAction::ReadFile { path: "/hello.txt" });
        let missing_ast = render_ast_for_test(AgenticAction::ReadFile {
            path: "/missing.txt",
        });
        assert!(asts.contains(&hello_ast));
        assert!(asts.contains(&missing_ast));

        let candidate_tokens: Vec<Vec<u32>> = asts.iter().map(|ast| byte_tokens(ast)).collect();
        let candidates: Vec<&[u32]> = candidate_tokens
            .iter()
            .map(|tokens| tokens.as_slice())
            .collect();
        let hello_tokens = byte_tokens(&hello_ast);
        let missing_tokens = byte_tokens(&missing_ast);

        assert_eq!(
            drive_masked_model_logits(&candidates, &hello_tokens),
            hello_tokens
        );
        assert_eq!(
            drive_masked_model_logits(&candidates, &missing_tokens),
            missing_tokens
        );
    }

    fn render_ast_for_test(action: AgenticAction<'_>) -> String {
        let mut ast = String::new();
        render_agentic_ast(action, &mut ast).unwrap();
        ast
    }

    fn byte_tokens(ast: &str) -> Vec<u32> {
        ast.bytes().map(u32::from).collect()
    }

    fn drive_masked_model_logits(candidates: &[&[u32]], target: &[u32]) -> Vec<u32> {
        assert!(
            candidates.contains(&target),
            "target AST must be a valid grammar candidate"
        );
        let mut generated = Vec::new();
        while !candidates.contains(&generated.as_slice()) {
            let mut logits = vec![-1_000.0; 256];
            for candidate in candidates {
                if candidate.len() <= generated.len() {
                    continue;
                }
                let (prefix, suffix) = candidate.split_at(generated.len());
                if prefix == generated.as_slice() {
                    logits[suffix[0] as usize] = 0.0;
                }
            }
            logits[target[generated.len()] as usize] = 10_000.0;
            let next = constrained_argmax_token(&logits, &generated, candidates).unwrap();
            generated.push(next);
        }
        generated
    }

    #[test]
    fn generate_logits_last_position_slice_kat() {
        let bytes: Vec<u8> = (0..12u32).flat_map(|v| (v as f32).to_le_bytes()).collect();
        let mut dims = [0u64; 8];
        dims[0] = 3;
        dims[1] = 4;
        assert_eq!(logits_byte_range(2, dims, 1).unwrap(), (16, 32));
        assert_eq!(
            read_logits_at_position(&bytes, 2, dims, 1).unwrap(),
            vec![4.0, 5.0, 6.0, 7.0]
        );
    }

    /// ISC-1/4 host loop oracle: each call sees the full growing context, and
    /// greedy decode stops after emitting EOS 4. The companion QEMU scenario
    /// runs the same sequence through a hand-laid `.neom` and `exec::run`.
    #[test]
    fn generate_isc4_decode_loop_recomputes_growing_context() {
        let mut calls = Vec::new();
        let mut rng = Pcg32::seeded(42, 54);
        let generated = generate_tokens_with(
            &[1],
            DecodeConfig {
                max_new_tokens: 4,
                eos_token_id: Some(4),
            },
            SamplingConfig::greedy(),
            &mut rng,
            |context, logits| {
                calls.push(context.to_vec());
                logits.clear();
                logits.resize(8, -8.0);
                let next = match context {
                    [1] => 2,
                    [1, 2] => 3,
                    [1, 2, 3] => 4,
                    _ => 7,
                };
                logits[next] = 8.0;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(generated, vec![2, 3, 4]);
        assert_eq!(calls, vec![vec![1], vec![1, 2], vec![1, 2, 3]]);
    }

    #[test]
    fn generate_isc4_tiny_neom_oracle_validates() {
        let bytes = generation_oracle_neom();
        let parsed = validate_neom(&bytes).expect("tiny generation oracle validates");
        assert_eq!(parsed.header.tensor_count, 1);
        assert_eq!(parsed.header.node_count, 1);
        assert_eq!(parsed.header.input_count, 1);
        assert_eq!(parsed.header.output_count, 1);
        assert_eq!(parsed.value_count, 3);
        assert_eq!(parsed.ios[0].shape.dims[0], 4);
        assert_eq!(parsed.ios[1].shape.dims[0], 4);
        assert_eq!(parsed.ios[1].shape.dims[1], 8);
    }

    #[test]
    fn neom_cap_lift_accepts_file_just_over_old_limit() {
        const OLD_LIMIT: usize = 64 * 1024 * 1024;
        const _: () = assert!(NEOM_MAX_FILE_SIZE == 2 * 1024 * 1024 * 1024);
        const _: () = assert!(usize::BITS >= 64);
        const _: () = assert!(NEOM_MAX_TENSORS > 340);

        let bytes = just_over_old_cap_neom();
        assert_eq!(bytes.len(), OLD_LIMIT + 1);
        let parsed = validate_neom(&bytes).expect("just-over-old-cap .neom validates");
        assert_eq!(parsed.header.total_size, bytes.len() as u64);
        assert_eq!(parsed.header.tensor_count, 1);
    }

    #[test]
    fn neom_config_05b_inventory_matches_hardcoded_qwen_consts() {
        let (bytes, model) = qwen_config_inventory(QwenInventory {
            hidden_size: forward_logic::QWEN25_05B_HIDDEN_SIZE,
            vocab: REAL_QWEN_VOCAB_SIZE,
            intermediate_size: forward_logic::QWEN25_05B_INTERMEDIATE_SIZE,
            layer_count: forward_logic::QWEN25_05B_LAYER_COUNT,
            q_projection: forward_logic::QWEN25_05B_Q_HEADS * forward_logic::QWEN25_05B_HEAD_DIM,
            kv_projection: forward_logic::QWEN25_05B_KV_HEADS * forward_logic::QWEN25_05B_HEAD_DIM,
            rope_theta: forward_logic::QWEN25_ROPE_THETA,
            rms_norm_eps: forward_logic::QWEN25_05B_RMS_NORM_EPS,
        });

        let config = derive_model_config(&bytes, &model)
            .expect("0.5B config derives")
            .expect("v2 header has config");
        assert_eq!(config.hidden_size, forward_logic::QWEN25_05B_HIDDEN_SIZE);
        assert_eq!(
            config.intermediate_size,
            forward_logic::QWEN25_05B_INTERMEDIATE_SIZE
        );
        assert_eq!(config.layer_count, forward_logic::QWEN25_05B_LAYER_COUNT);
        assert_eq!(config.q_heads, forward_logic::QWEN25_05B_Q_HEADS);
        assert_eq!(config.kv_heads, forward_logic::QWEN25_05B_KV_HEADS);
        assert_eq!(config.head_dim, forward_logic::QWEN25_05B_HEAD_DIM);
        assert_eq!(
            config.q_projection_size,
            forward_logic::QWEN25_05B_Q_HEADS * forward_logic::QWEN25_05B_HEAD_DIM
        );
        assert_eq!(config.context, forward_logic::QWEN25_05B_CONTEXT);
        assert_eq!(config.vocab, REAL_QWEN_VOCAB_SIZE);
        assert_eq!(config.rope_theta, forward_logic::QWEN25_ROPE_THETA);
        assert_eq!(config.rms_norm_eps, forward_logic::QWEN25_05B_RMS_NORM_EPS);
        assert_eq!(
            Qwen2LayerConfig::from_model_config(&config).unwrap(),
            Qwen2LayerConfig::qwen25_05b()
        );
    }

    #[test]
    fn neom_config_05b_raw_quant_inventory_derives_qwen_consts() {
        let (bytes, model) = qwen_raw_quant_config_inventory(QwenInventory {
            hidden_size: forward_logic::QWEN25_05B_HIDDEN_SIZE,
            vocab: REAL_QWEN_VOCAB_SIZE,
            intermediate_size: forward_logic::QWEN25_05B_INTERMEDIATE_SIZE,
            layer_count: forward_logic::QWEN25_05B_LAYER_COUNT,
            q_projection: forward_logic::QWEN25_05B_Q_HEADS * forward_logic::QWEN25_05B_HEAD_DIM,
            kv_projection: forward_logic::QWEN25_05B_KV_HEADS * forward_logic::QWEN25_05B_HEAD_DIM,
            rope_theta: forward_logic::QWEN25_ROPE_THETA,
            rms_norm_eps: forward_logic::QWEN25_05B_RMS_NORM_EPS,
        });

        let config = derive_model_config(&bytes, &model)
            .expect("raw quant config derives")
            .expect("v2 header has config");
        assert_eq!(config.hidden_size, forward_logic::QWEN25_05B_HIDDEN_SIZE);
        assert_eq!(
            config.intermediate_size,
            forward_logic::QWEN25_05B_INTERMEDIATE_SIZE
        );
        assert_eq!(config.layer_count, forward_logic::QWEN25_05B_LAYER_COUNT);
        assert_eq!(config.q_heads, forward_logic::QWEN25_05B_Q_HEADS);
        assert_eq!(config.kv_heads, forward_logic::QWEN25_05B_KV_HEADS);
        assert_eq!(config.head_dim, forward_logic::QWEN25_05B_HEAD_DIM);
        assert_eq!(
            config.q_projection_size,
            forward_logic::QWEN25_05B_Q_HEADS * forward_logic::QWEN25_05B_HEAD_DIM
        );
        assert_eq!(config.context, forward_logic::QWEN25_05B_CONTEXT);
        assert_eq!(config.vocab, REAL_QWEN_VOCAB_SIZE);
        assert_eq!(config.rope_theta, forward_logic::QWEN25_ROPE_THETA);
        assert_eq!(config.rms_norm_eps, forward_logic::QWEN25_05B_RMS_NORM_EPS);
    }

    #[test]
    fn neom_config_15b_inventory_derives_distinct_qwen_values() {
        let (bytes, model) = qwen_config_inventory(QwenInventory {
            hidden_size: 1536,
            vocab: REAL_QWEN_VOCAB_SIZE,
            intermediate_size: 8192,
            layer_count: 28,
            q_projection: 1536,
            kv_projection: 512,
            rope_theta: forward_logic::QWEN25_ROPE_THETA,
            rms_norm_eps: forward_logic::QWEN25_05B_RMS_NORM_EPS,
        });

        let config = derive_model_config(&bytes, &model)
            .expect("1.5B config derives")
            .expect("v2 header has config");
        assert_eq!(config.hidden_size, 1536);
        assert_eq!(config.intermediate_size, 8192);
        assert_eq!(config.layer_count, 28);
        assert_eq!(config.q_heads, 12);
        assert_eq!(config.kv_heads, 4);
        assert_eq!(config.head_dim, 128);
        assert_eq!(config.rope_dimension_count, 128);
        assert_eq!(config.context, QWEN2_DEFAULT_CONTEXT);
        assert_eq!(config.vocab, REAL_QWEN_VOCAB_SIZE);

        let layer = Qwen2LayerConfig::from_model_config(&config).unwrap();
        assert_eq!(layer.hidden_size, 1536);
        assert_eq!(layer.intermediate_size, 8192);
        assert_eq!(layer.q_heads, 12);
        assert_eq!(layer.kv_heads, 4);
        assert_eq!(layer.head_dim, 128);
        assert_eq!(layer.attn_q_projection_size, 1536);
        assert_eq!(
            layer.rope_theta_scale_bits,
            forward_logic::QWEN25_15B_ROPE_THETA_SCALE.to_bits()
        );
    }

    #[test]
    fn neom_config_qwen35_mixed_gdn_inventory_derives_values() {
        let (bytes, model) = qwen35_mixed_config_inventory();

        let config = derive_model_config(&bytes, &model)
            .expect("Qwen3.5 mixed config derives")
            .expect("v4 header has config");
        assert_eq!(config.hidden_size, 256);
        assert_eq!(config.intermediate_size, 512);
        assert_eq!(config.layer_count, 4);
        assert_eq!(config.q_heads, 2);
        assert_eq!(config.kv_heads, 1);
        assert_eq!(config.head_dim, 128);
        assert_eq!(config.rope_dimension_count, 64);
        assert_eq!(config.q_projection_size, 512);
        assert_eq!(config.vocab, 512);
        assert_eq!(config.rope_theta, 1_000_000.0);
    }

    #[test]
    fn qwen35_llama_cpp_greedy_32_golden_matches_committed_reference_bytes() {
        // The kernel/oracle identity comes from their shared include! of this
        // golden; this KAT independently pins the committed reference bytes.
        assert_eq!(
            super::qwen35_llama_cpp_golden::LLAMA_CPP_QWEN35_HELLO_GREEDY_32,
            [
                11, 353, 1044, 264, 5286, 506, 264, 11759, 13, 353, 1044, 4237, 264, 5392, 383,
                279, 8282, 314, 328, 760, 14955, 314, 279, 3130, 303, 279, 10499, 314, 279, 36317,
                3158, 353,
            ]
        );
    }

    #[test]
    fn neom_config_qwen35_rejects_zero_rope_dimension_count() {
        let (bytes, mut model) = qwen35_mixed_config_inventory();
        model
            .header
            .config
            .as_mut()
            .expect("v4 header config")
            .rope_dimension_count = Some(0);
        assert_eq!(
            derive_model_config(&bytes, &model),
            Err(ModelFormatError::InvalidConfig)
        );
    }

    #[test]
    fn neom_config_feature_off_selects_hardcoded_fixture_config() {
        let model_config = ModelConfig {
            hidden_size: 1536,
            intermediate_size: 8192,
            layer_count: 28,
            q_heads: 12,
            kv_heads: 4,
            head_dim: 128,
            rope_dimension_count: 128,
            q_projection_size: 1536,
            context: QWEN2_DEFAULT_CONTEXT,
            vocab: REAL_QWEN_VOCAB_SIZE,
            rope_theta: forward_logic::QWEN25_ROPE_THETA,
            rms_norm_eps: forward_logic::QWEN25_05B_RMS_NORM_EPS,
        };
        let selected = Qwen2LayerConfig::for_model_config(Some(model_config)).unwrap();
        #[cfg(feature = "phase9-data-driven-config")]
        assert_eq!(selected.hidden_size, 1536);
        #[cfg(not(feature = "phase9-data-driven-config"))]
        assert_eq!(selected, Qwen2LayerConfig::qwen25_05b());
    }

    fn just_over_old_cap_neom() -> Vec<u8> {
        const OLD_LIMIT: usize = 64 * 1024 * 1024;
        const TOTAL: usize = OLD_LIMIT + 1;
        const TENSOR_OFF: usize = 96;
        const EMPTY_TABLES_OFF: usize = 192;
        const WEIGHT_OFF: usize = 4096;
        const WEIGHT_LEN: usize = TOTAL - WEIGHT_OFF;

        let mut bytes = vec![0u8; TOTAL];
        bytes[0..4].copy_from_slice(b"NEOM");
        put_u32(&mut bytes, 4, 1);
        put_u32(&mut bytes, 8, 96);
        put_u32(&mut bytes, 16, 1);
        put_u64(&mut bytes, 32, TENSOR_OFF as u64);
        put_u64(&mut bytes, 40, EMPTY_TABLES_OFF as u64);
        put_u64(&mut bytes, 48, EMPTY_TABLES_OFF as u64);
        put_u64(&mut bytes, 56, EMPTY_TABLES_OFF as u64);
        put_u64(&mut bytes, 72, WEIGHT_OFF as u64);
        put_u64(&mut bytes, 80, WEIGHT_LEN as u64);
        put_u64(&mut bytes, 88, TOTAL as u64);

        put_u32(&mut bytes, TENSOR_OFF, 5);
        put_u32(&mut bytes, TENSOR_OFF + 4, 1);
        put_u64(&mut bytes, TENSOR_OFF + 8, WEIGHT_LEN as u64);
        put_u64(&mut bytes, TENSOR_OFF + 72, WEIGHT_OFF as u64);
        put_u64(&mut bytes, TENSOR_OFF + 80, WEIGHT_LEN as u64);
        bytes
    }

    struct QwenInventory {
        hidden_size: usize,
        vocab: usize,
        intermediate_size: usize,
        layer_count: usize,
        q_projection: usize,
        kv_projection: usize,
        rope_theta: f32,
        rms_norm_eps: f32,
    }

    fn qwen_config_inventory(spec: QwenInventory) -> (Vec<u8>, ValidatedNeom) {
        let mut strings = Vec::new();
        let tensors = vec![
            qwen_config_tensor(
                &mut strings,
                "token_embd.weight",
                spec.hidden_size,
                spec.vocab,
            ),
            qwen_config_tensor(
                &mut strings,
                "blk.0.attn_q.weight",
                spec.hidden_size,
                spec.q_projection,
            ),
            qwen_config_tensor(
                &mut strings,
                "blk.0.attn_k.weight",
                spec.hidden_size,
                spec.kv_projection,
            ),
            qwen_config_tensor(
                &mut strings,
                "blk.0.ffn_gate.weight",
                spec.hidden_size,
                spec.intermediate_size,
            ),
            qwen_config_tensor(
                &mut strings,
                "blk.0.ffn_up.weight",
                spec.hidden_size,
                spec.intermediate_size,
            ),
            qwen_config_tensor(
                &mut strings,
                "blk.0.ffn_down.weight",
                spec.intermediate_size,
                spec.hidden_size,
            ),
        ];
        let header = Header {
            version: 2,
            header_size: 108,
            tensor_count: tensors.len() as u32,
            node_count: 0,
            input_count: 0,
            output_count: 0,
            tensor_table_off: 0,
            node_table_off: 0,
            io_table_off: 0,
            string_off: 0,
            string_len: strings.len() as u64,
            weight_blob_off: 0,
            weight_blob_len: 0,
            total_size: strings.len() as u64,
            config: Some(HeaderConfig {
                layer_count: spec.layer_count as u32,
                rope_theta_bits: spec.rope_theta.to_bits(),
                rms_norm_eps_bits: spec.rms_norm_eps.to_bits(),
                rope_dimension_count: None,
            }),
            schedule: None,
        };
        (
            strings,
            ValidatedNeom {
                header,
                tensors,
                nodes: Vec::new(),
                ios: Vec::new(),
                value_count: 0,
            },
        )
    }

    fn qwen_raw_quant_config_inventory(spec: QwenInventory) -> (Vec<u8>, ValidatedNeom) {
        let mut strings = Vec::new();
        let tensors = vec![
            qwen_raw_quant_config_tensor(
                &mut strings,
                "token_embd.weight",
                q5_0_bytes(spec.hidden_size, spec.vocab),
            ),
            qwen_raw_quant_config_tensor(
                &mut strings,
                "blk.0.attn_q.weight",
                q5_0_bytes(spec.hidden_size, spec.q_projection),
            ),
            qwen_raw_quant_config_tensor(
                &mut strings,
                "blk.0.attn_k.weight",
                q5_0_bytes(spec.hidden_size, spec.kv_projection),
            ),
            qwen_raw_quant_config_tensor(
                &mut strings,
                "blk.0.ffn_gate.weight",
                q5_0_bytes(spec.hidden_size, spec.intermediate_size),
            ),
            qwen_raw_quant_config_tensor(
                &mut strings,
                "blk.0.ffn_up.weight",
                q5_0_bytes(spec.hidden_size, spec.intermediate_size),
            ),
            qwen_raw_quant_config_tensor(
                &mut strings,
                "blk.0.ffn_down.weight",
                q6_k_bytes(spec.intermediate_size, spec.hidden_size),
            ),
        ];
        let header = Header {
            version: 2,
            header_size: 108,
            tensor_count: tensors.len() as u32,
            node_count: 0,
            input_count: 0,
            output_count: 0,
            tensor_table_off: 0,
            node_table_off: 0,
            io_table_off: 0,
            string_off: 0,
            string_len: strings.len() as u64,
            weight_blob_off: 0,
            weight_blob_len: 0,
            total_size: strings.len() as u64,
            config: Some(HeaderConfig {
                layer_count: spec.layer_count as u32,
                rope_theta_bits: spec.rope_theta.to_bits(),
                rms_norm_eps_bits: spec.rms_norm_eps.to_bits(),
                rope_dimension_count: None,
            }),
            schedule: None,
        };
        (
            strings,
            ValidatedNeom {
                header,
                tensors,
                nodes: Vec::new(),
                ios: Vec::new(),
                value_count: 0,
            },
        )
    }

    fn qwen35_mixed_config_inventory() -> (Vec<u8>, ValidatedNeom) {
        let mut strings = Vec::new();
        let hidden = 256usize;
        let vocab = 512usize;
        let intermediate = 512usize;
        let tensors = vec![
            qwen_config_rank1_tensor(&mut strings, "output_norm.weight", hidden),
            qwen_raw_quant_config_tensor(
                &mut strings,
                "token_embd.weight",
                q4_k_bytes(hidden, vocab),
            ),
            qwen_config_tensor(&mut strings, "blk.0.ffn_gate.weight", hidden, intermediate),
            qwen_config_tensor(&mut strings, "blk.0.ffn_up.weight", hidden, intermediate),
            qwen_config_tensor(&mut strings, "blk.0.ffn_down.weight", intermediate, hidden),
            qwen_config_tensor(&mut strings, "blk.3.attn_q.weight", hidden, hidden * 2),
            qwen_config_tensor(&mut strings, "blk.3.attn_k.weight", hidden, 128),
            qwen_config_rank1_tensor(&mut strings, "blk.3.attn_q_norm.weight", 128),
        ];
        let header = Header {
            version: 4,
            header_size: 128,
            tensor_count: tensors.len() as u32,
            node_count: 0,
            input_count: 0,
            output_count: 0,
            tensor_table_off: 0,
            node_table_off: 0,
            io_table_off: 0,
            string_off: 0,
            string_len: strings.len() as u64,
            weight_blob_off: 0,
            weight_blob_len: 0,
            total_size: strings.len() as u64,
            config: Some(HeaderConfig {
                layer_count: 4,
                rope_theta_bits: 1_000_000.0f32.to_bits(),
                rms_norm_eps_bits: 1.0e-6f32.to_bits(),
                rope_dimension_count: Some(64),
            }),
            schedule: None,
        };
        (
            strings,
            ValidatedNeom {
                header,
                tensors,
                nodes: Vec::new(),
                ios: Vec::new(),
                value_count: 0,
            },
        )
    }

    fn qwen_raw_quant_config_tensor(
        strings: &mut Vec<u8>,
        name: &str,
        byte_len: usize,
    ) -> TensorMeta {
        let name_off = strings.len() as u32;
        strings.extend_from_slice(name.as_bytes());
        let mut dims = [0u64; 8];
        dims[0] = byte_len as u64;
        TensorMeta {
            dtype: NeomDtype::U8,
            shape: ShapeMeta { rank: 1, dims },
            weight_off: 0,
            weight_len: byte_len as u64,
            name_off,
            name_len: name.len() as u32,
        }
    }

    fn q4_k_bytes(rows: usize, cols: usize) -> usize {
        rows.checked_mul(cols).unwrap() / 256 * 144
    }

    fn q5_0_bytes(rows: usize, cols: usize) -> usize {
        rows.checked_mul(cols).unwrap() / 32 * 22
    }

    fn q6_k_bytes(rows: usize, cols: usize) -> usize {
        rows.checked_mul(cols).unwrap() / 256 * 210
    }

    fn qwen_config_tensor(
        strings: &mut Vec<u8>,
        name: &str,
        dim0: usize,
        dim1: usize,
    ) -> TensorMeta {
        let name_off = strings.len() as u32;
        strings.extend_from_slice(name.as_bytes());
        let mut dims = [0u64; 8];
        dims[0] = dim0 as u64;
        dims[1] = dim1 as u64;
        TensorMeta {
            dtype: NeomDtype::F32,
            shape: ShapeMeta { rank: 2, dims },
            weight_off: 0,
            weight_len: 0,
            name_off,
            name_len: name.len() as u32,
        }
    }

    fn qwen_config_rank1_tensor(strings: &mut Vec<u8>, name: &str, dim0: usize) -> TensorMeta {
        let name_off = strings.len() as u32;
        strings.extend_from_slice(name.as_bytes());
        let mut dims = [0u64; 8];
        dims[0] = dim0 as u64;
        TensorMeta {
            dtype: NeomDtype::F32,
            shape: ShapeMeta { rank: 1, dims },
            weight_off: 0,
            weight_len: 0,
            name_off,
            name_len: name.len() as u32,
        }
    }

    fn generation_oracle_neom() -> Vec<u8> {
        const HEADER_OFF: usize = 0;
        const TENSOR_OFF: usize = 96;
        const NODE_OFF: usize = 192;
        const NODE_REFS_OFF: usize = 240;
        const IO_OFF: usize = 256;
        const STRING_OFF: usize = 416;
        const WEIGHT_OFF: usize = 4096;
        const VOCAB: usize = 8;
        const CONTEXT: usize = 4;
        const WEIGHT_LEN: usize = VOCAB * VOCAB * 4;
        const TOTAL: usize = WEIGHT_OFF + WEIGHT_LEN;

        let mut bytes = vec![0u8; TOTAL];
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
        put_u64(&mut bytes, HEADER_OFF + 64, 0);
        put_u64(&mut bytes, HEADER_OFF + 72, WEIGHT_OFF as u64);
        put_u64(&mut bytes, HEADER_OFF + 80, WEIGHT_LEN as u64);
        put_u64(&mut bytes, HEADER_OFF + 88, TOTAL as u64);

        put_u32(&mut bytes, TENSOR_OFF, 0);
        put_u32(&mut bytes, TENSOR_OFF + 4, 2);
        put_u64(&mut bytes, TENSOR_OFF + 8, VOCAB as u64);
        put_u64(&mut bytes, TENSOR_OFF + 16, VOCAB as u64);
        put_u64(&mut bytes, TENSOR_OFF + 72, WEIGHT_OFF as u64);
        put_u64(&mut bytes, TENSOR_OFF + 80, WEIGHT_LEN as u64);

        put_u32(&mut bytes, NODE_OFF, 9);
        put_u32(&mut bytes, NODE_OFF + 4, 2);
        put_u32(&mut bytes, NODE_OFF + 8, 1);
        put_u32(&mut bytes, NODE_OFF + 12, NODE_REFS_OFF as u32);
        put_u32(&mut bytes, NODE_OFF + 16, (NODE_REFS_OFF + 8) as u32);
        put_u32(&mut bytes, NODE_REFS_OFF, 1);
        put_u32(&mut bytes, NODE_REFS_OFF + 4, 0);
        put_u32(&mut bytes, NODE_REFS_OFF + 8, 2);

        put_u32(&mut bytes, IO_OFF, 1);
        put_u32(&mut bytes, IO_OFF + 4, 3);
        put_u32(&mut bytes, IO_OFF + 8, 1);
        put_u64(&mut bytes, IO_OFF + 16, CONTEXT as u64);
        put_u32(&mut bytes, IO_OFF + 80, 2);
        put_u32(&mut bytes, IO_OFF + 84, 0);
        put_u32(&mut bytes, IO_OFF + 88, 2);
        put_u32(&mut bytes, IO_OFF + 92, 1);
        put_u64(&mut bytes, IO_OFF + 96, CONTEXT as u64);
        put_u64(&mut bytes, IO_OFF + 104, VOCAB as u64);

        for token in 0..VOCAB {
            let next = match token {
                1 => 2,
                2 => 3,
                3 => 4,
                _ => token,
            };
            for vocab in 0..VOCAB {
                let logit = if vocab == next { 8.0f32 } else { -8.0f32 };
                let off = WEIGHT_OFF + (token * VOCAB + vocab) * 4;
                bytes[off..off + 4].copy_from_slice(&logit.to_le_bytes());
            }
        }
        bytes
    }

    /// ISC-1: hand-laid GGUF v3 bytes. The fixture follows gguf.h's wire
    /// order directly: magic/version/counts, two KV entries, two tensor-info
    /// entries, then an aligned data blob. It is not generated by the parser.
    #[test]
    fn gguf_isc1_container_metadata_tensor_table_byte_oracle() {
        let bytes = tiny_gguf_fixture();
        let parsed = parse_gguf(&bytes).expect("hand-laid GGUF parses");
        assert_eq!(parsed.version, 3);
        assert_eq!(parsed.alignment, 32);
        assert_eq!(parsed.metadata.len(), 2);
        assert_eq!(
            parsed.metadata_value("general.architecture"),
            Some(&GgufValue::String("qwen2".to_string()))
        );
        assert_eq!(parsed.metadata_u32("general.alignment"), Some(32));

        assert_eq!(parsed.tensors.len(), 2);
        assert_eq!(parsed.tensors[0].name, "token_embd.weight");
        assert_eq!(parsed.tensors[0].ty, GgmlType::Q4K);
        assert_eq!(parsed.tensors[0].dims[0], QK_K as u64);
        assert_eq!(parsed.tensors[0].offset, 0);
        assert_eq!(parsed.tensors[0].byte_len, BLOCK_Q4_K_SIZE as u64);
        assert_eq!(parsed.tensors[0].data_start % 32, 0);

        assert_eq!(parsed.tensors[1].name, "output_norm.weight");
        assert_eq!(parsed.tensors[1].ty, GgmlType::Q8_0);
        assert_eq!(parsed.tensors[1].dims[0], QK8_0 as u64);
        assert_eq!(parsed.tensors[1].offset, 160);
        assert_eq!(parsed.tensors[1].byte_len, BLOCK_Q8_0_SIZE as u64);
        assert_eq!(
            parsed.tensors[1].data_start,
            parsed.tensors[0].data_start + 160
        );
    }

    /// ISC-2: raw block field extraction. This asserts the exact byte offsets
    /// from ggml-common.h: q4_K = d/dmin/scales/qs at 0/2/4/16, q8_0 =
    /// d/qs at 0/2. No dequantization is performed.
    #[test]
    fn gguf_isc2_quantized_block_extracts_field_offsets() {
        let bytes = tiny_gguf_fixture();
        let parsed = parse_gguf(&bytes).unwrap();
        let q4 = parsed
            .q4_k_blocks_for_tensor(&bytes, &parsed.tensors[0])
            .unwrap();
        assert_eq!(q4.len(), 1);
        assert_eq!(q4[0].d, 0x1234);
        assert_eq!(q4[0].dmin, 0x5678);
        assert_eq!(q4[0].scales[0], 0xa0);
        assert_eq!(q4[0].scales[K_SCALE_SIZE - 1], 0xab);
        assert_eq!(q4[0].qs[0], 0x10);
        assert_eq!(q4[0].qs[(QK_K / 2) - 1], 0x8f);

        let q8 = parsed
            .q8_0_blocks_for_tensor(&bytes, &parsed.tensors[1])
            .unwrap();
        assert_eq!(q8.len(), 1);
        assert_eq!(q8[0].d, 0x9abc);
        assert_eq!(q8[0].qs[0], -16);
        assert_eq!(q8[0].qs[QK8_0 - 1], 15);

        assert_eq!(
            parse_q4_k_blocks(&[0u8; 72]).unwrap_err(),
            super::gguf_logic::GgufError::InvalidTensor
        );
        assert_eq!(
            parse_q8_0_blocks(&[0u8; 33]).unwrap_err(),
            super::gguf_logic::GgufError::InvalidTensor
        );
    }

    /// ISC-3: Qwen2.5 BPE KAT from raw tokenizer assets:
    /// HuggingFace `Qwen/Qwen2.5-0.5B` `vocab.json` + `merges.txt`.
    /// The gap-analysis draft listed stale ids; the raw-source vocab maps
    /// "Hello, world!\n" to [9707, 11, 1879, 4894].
    #[test]
    fn gguf_isc3_qwen25_bpe_raw_source_kat() {
        let vocab = [
            BpeVocabEntry {
                token: b"Hello",
                id: 9707,
            },
            BpeVocabEntry {
                token: b",",
                id: 11,
            },
            BpeVocabEntry {
                token: b"\xc4\xa0world",
                id: 1879,
            },
            BpeVocabEntry {
                token: b"!\xc4\x8a",
                id: 4894,
            },
        ];
        let merges = [
            BpeMergeEntry {
                left: b"e",
                right: b"l",
                rank: 45,
            },
            BpeMergeEntry {
                left: b"l",
                right: b"o",
                rank: 129,
            },
            BpeMergeEntry {
                left: b"el",
                right: b"lo",
                rank: 4527,
            },
            BpeMergeEntry {
                left: b"H",
                right: b"ello",
                rank: 9434,
            },
            BpeMergeEntry {
                left: b"o",
                right: b"r",
                rank: 13,
            },
            BpeMergeEntry {
                left: b"\xc4\xa0",
                right: b"w",
                rank: 33,
            },
            BpeMergeEntry {
                left: b"l",
                right: b"d",
                rank: 251,
            },
            BpeMergeEntry {
                left: b"or",
                right: b"ld",
                rank: 1125,
            },
            BpeMergeEntry {
                left: b"\xc4\xa0w",
                right: b"orld",
                rank: 1618,
            },
            BpeMergeEntry {
                left: b"!",
                right: b"\xc4\x8a",
                rank: 4630,
            },
        ];
        let tokenizer = BpeTokenizer::from_slices(&vocab, &merges).unwrap();
        let ids = tokenizer.encode("Hello, world!\n").unwrap();
        assert_eq!(ids, vec![9707, 11, 1879, 4894]);
        assert_eq!(tokenizer.decode(&ids).unwrap(), "Hello, world!\n");
    }

    /// ISC-4: optional real-model gate. Set NEO_QWEN25_GGUF to the local
    /// Qwen2.5-0.5B-Q4_K_M.gguf path to prove the parser enumerates the real
    /// model. CI can run without the 491 MB artifact.
    #[test]
    fn gguf_isc4_real_qwen25_loads_when_available() {
        let Ok(path) = std::env::var("NEO_QWEN25_GGUF") else {
            eprintln!("SKIP gguf_isc4_real_qwen25_loads_when_available: NEO_QWEN25_GGUF unset");
            return;
        };
        let bytes = std::fs::read(&path).expect("read real Qwen2.5 GGUF");
        let parsed = parse_gguf(&bytes).expect("real Qwen2.5 GGUF parses");
        assert_eq!(parsed.version, 3);
        assert_eq!(
            parsed.metadata_string("general.architecture"),
            Some("qwen2")
        );
        assert!(parsed.tensors.len() > 100);
        assert!(
            parsed
                .tensors
                .iter()
                .any(|tensor| tensor.ty == GgmlType::Q4K)
        );
        assert!(
            parsed
                .tensor_named("token_embd.weight")
                .or_else(|| parsed.tensor_named("output.weight"))
                .is_some()
        );
        if let Some(tokens) = parsed.metadata_string_array("tokenizer.ggml.tokens") {
            assert!(tokens.len() > 100_000);
            assert_eq!(tokens.get(9707).map(String::as_str), Some("Hello"));
        }
        let first_q4 = parsed
            .tensors
            .iter()
            .find(|tensor| tensor.ty == GgmlType::Q4K)
            .expect("at least one q4_K tensor");
        assert_eq!(
            parsed.tensor_data(&bytes, first_q4).unwrap().len() % BLOCK_Q4_K_SIZE,
            0
        );
        eprintln!(
            "loaded_qwen25_gguf path={} tensors={} metadata={} first_q4={} bytes={}",
            path,
            parsed.tensors.len(),
            parsed.metadata.len(),
            first_q4.name,
            first_q4.byte_len
        );
    }

    fn tiny_gguf_fixture() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"GGUF");
        push_u32(&mut bytes, 3);
        push_i64(&mut bytes, 2);
        push_i64(&mut bytes, 2);

        push_gguf_string(&mut bytes, "general.architecture");
        push_i32(&mut bytes, 8);
        push_gguf_string(&mut bytes, "qwen2");

        push_gguf_string(&mut bytes, "general.alignment");
        push_i32(&mut bytes, 4);
        push_u32(&mut bytes, 32);

        push_gguf_string(&mut bytes, "token_embd.weight");
        push_u32(&mut bytes, 1);
        push_i64(&mut bytes, QK_K as i64);
        push_i32(&mut bytes, GgmlType::Q4K.as_abi());
        push_u64(&mut bytes, 0);

        push_gguf_string(&mut bytes, "output_norm.weight");
        push_u32(&mut bytes, 1);
        push_i64(&mut bytes, QK8_0 as i64);
        push_i32(&mut bytes, GgmlType::Q8_0.as_abi());
        push_u64(&mut bytes, 160);

        pad_vec(&mut bytes, 32);
        let data_start = bytes.len();
        let mut q4 = [0u8; BLOCK_Q4_K_SIZE];
        q4[0..2].copy_from_slice(&0x1234u16.to_le_bytes());
        q4[2..4].copy_from_slice(&0x5678u16.to_le_bytes());
        for idx in 0..K_SCALE_SIZE {
            q4[4 + idx] = 0xa0 + idx as u8;
        }
        for idx in 0..(QK_K / 2) {
            q4[16 + idx] = 0x10 + idx as u8;
        }
        bytes.extend_from_slice(&q4);
        while bytes.len() < data_start + 160 {
            bytes.push(0);
        }

        let mut q8 = [0u8; BLOCK_Q8_0_SIZE];
        q8[0..2].copy_from_slice(&0x9abcu16.to_le_bytes());
        for idx in 0..QK8_0 {
            q8[2 + idx] = ((idx as i8) - 16) as u8;
        }
        bytes.extend_from_slice(&q8);
        pad_vec(&mut bytes, 32);
        bytes
    }

    fn push_gguf_string(bytes: &mut Vec<u8>, value: &str) {
        push_u64(bytes, value.len() as u64);
        bytes.extend_from_slice(value.as_bytes());
    }

    fn push_i32(bytes: &mut Vec<u8>, value: i32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_i64(bytes: &mut Vec<u8>, value: i64) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u64(bytes: &mut Vec<u8>, value: u64) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn pad_vec(bytes: &mut Vec<u8>, alignment: usize) {
        while !bytes.len().is_multiple_of(alignment) {
            bytes.push(0);
        }
    }

    fn put_u32(bytes: &mut [u8], off: usize, value: u32) {
        bytes[off..off + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u64(bytes: &mut [u8], off: usize, value: u64) {
        bytes[off..off + 8].copy_from_slice(&value.to_le_bytes());
    }
    // ── Phase 7.5 BUILD-W1: MT7601U byte-oracles (I-12) ──────────────────────
    // Hand-laid byte fixtures, computed independently of the driver code, that
    // pin the firmware/MCU/eFUSE/RX byte layouts (CLAUDE.md byte-oracle rule:
    // a writer+reader pair from one spec agree even when both are wrong, so the
    // oracle's bytes are hand-derived from the raw struct definitions, never
    // from the parser they check).

    /// ISC-2: firmware header parse + image validation + chunk framing.
    #[test]
    fn mt7601u_w1_fw_header_and_chunk_oracle() {
        use super::fw_logic::*;
        // Hand-laid 32-byte mt76_fw_header: ilm_len=0x100, dlm_len=0x40,
        // build_ver=0x7640, fw_ver=0x0100 ("0.1.00"), build_time text.
        let mut hdr_bytes = [0u8; 32];
        hdr_bytes[0..4].copy_from_slice(&0x100u32.to_le_bytes());
        hdr_bytes[4..8].copy_from_slice(&0x40u32.to_le_bytes());
        hdr_bytes[8..10].copy_from_slice(&0x7640u16.to_le_bytes());
        hdr_bytes[10..12].copy_from_slice(&0x0100u16.to_le_bytes());
        hdr_bytes[16..32].copy_from_slice(b"2013020512000000");

        let hdr = parse_header(&hdr_bytes).expect("32 bytes parse");
        assert_eq!(hdr.ilm_len, 0x100);
        assert_eq!(hdr.dlm_len, 0x40);
        assert_eq!(hdr.build_ver, 0x7640);
        assert_eq!(hdr.fw_ver, 0x0100);
        assert_eq!(fw_version(&hdr), (0, 1, 0));

        // total = 32 + ilm_len(256) + dlm_len(64) = 352.
        let layout = validate(352, &hdr).expect("well-formed image");
        assert_eq!(layout.ivb_off, 32);
        assert_eq!(layout.ilm_data_len, 256 - 64); // 192
        assert_eq!(layout.ilm_off, 96);
        assert_eq!(layout.dlm_off, 96 + 192);
        assert_eq!(layout.dlm_len, 64);
        // Rejections — the three distinct -ENOENT causes.
        assert_eq!(validate(16, &hdr), Err(FwError::TooSmall));
        assert_eq!(validate(351, &hdr), Err(FwError::SizeMismatch));
        let short = FwHeader {
            ilm_len: 0x40,
            ..hdr
        };
        assert_eq!(
            validate(0x40 + 0x40 + 32, &short),
            Err(FwError::IlmTooShort)
        );

        // FCE TXINFO word: TYPE=DMA_PACKET(0) | D_PORT=CPU_TX_PORT(2)<<27 | LEN.
        assert_eq!(fce_txinfo(0x3800), 0x1000_3800);
        assert_eq!(fce_txinfo(100), 0x1000_0064);
        // FCE_DMA_LEN reg = roundup(len,4) << 16.
        assert_eq!(fce_dma_len_reg(100), 0x0064_0000);
        assert_eq!(fce_dma_len_reg(0x3800), 0x3800_0000);
        // build_chunk for a 3-byte payload: [4B txinfo][3B data][1B+4B zero].
        let mut buf = [0xEEu8; 32];
        let n = build_chunk(&mut buf, &[0xAA, 0xBB, 0xCC]).expect("fits");
        assert_eq!(n, 4 + 4 + 4); // DMA_HDR + roundup(3,4) + 4
        assert_eq!(
            &buf[..12],
            &[0x03, 0x00, 0x00, 0x10, 0xAA, 0xBB, 0xCC, 0x00, 0, 0, 0, 0]
        );
        // Chunk count: 0x3800-sized chunks.
        assert_eq!(chunk_count(0), 0);
        assert_eq!(chunk_count(0x3800), 1);
        assert_eq!(chunk_count(0x3801), 2);
    }

    /// ISC-3: MCU DMA command-wrap + reg-pair/burst payloads + response decode.
    #[test]
    fn mt7601u_w1_mcu_framing_oracle() {
        use super::mcu_logic::*;
        use super::regs;
        // dma_cmd_info(seq=3, cmd=RANDOM_WRITE=12, payload=16): hand-computed
        // 0x40000000(TYPE) | 0x10000000(D_PORT) | 0x00C00000(cmd) |
        // 0x00030000(seq) | 0x10(LEN) = 0x50C30010.
        assert_eq!(dma_cmd_info(3, regs::CMD_RANDOM_WRITE, 16), 0x50C3_0010);

        // Frame an 8-byte function-select payload (id=Q_SELECT, value=1).
        let mut payload = [0u8; 8];
        payload[0..4].copy_from_slice(&regs::FUNC_Q_SELECT.to_le_bytes());
        payload[4..8].copy_from_slice(&1u32.to_le_bytes());
        let mut frame = [0xEEu8; 32];
        let n = build_mcu_frame(&mut frame, &payload, 1, regs::CMD_FUN_SET_OP).expect("fits");
        assert_eq!(n, 16); // roundup(8+4,4) + 4
        // info = dma_cmd_info(1,1,8) = 0x50110008 → LE [08 00 11 50].
        assert_eq!(
            &frame[..16],
            &[
                0x08, 0x00, 0x11, 0x50, 0x01, 0, 0, 0, 0x01, 0, 0, 0, 0, 0, 0, 0
            ]
        );

        // RANDOM_WRITE payload for BBP base: [le32(base+reg)][le32(val)].
        let mut rp = [0u8; 16];
        let m = build_reg_pair_payload(&mut rp, regs::MCU_MEMMAP_BBP, &[(65, 0x2c), (66, 0x38)])
            .expect("fits");
        assert_eq!(m, 16);
        assert_eq!(
            &rp[..16],
            &[
                0x41, 0, 0, 0x40, 0x2c, 0, 0, 0, 0x42, 0, 0, 0x40, 0x38, 0, 0, 0
            ]
        );

        // BURST_WRITE payload: [le32(MEMMAP_WLAN+offset)][le32(data)...].
        let mut bw = [0u8; 12];
        let b = build_burst_payload(&mut bw, 0x10, &[0xffff_ffff, 0x00ff_ffff]).expect("fits");
        assert_eq!(b, 12);
        assert_eq!(
            &bw[..12],
            &[
                0x10, 0x00, 0x41, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00
            ]
        );

        // Response decode: seq=3 + CMD_DONE(0) matches; CMD_ERROR(1) does not.
        assert_eq!(
            decode_resp(0x0003_0000),
            McuResp {
                seq: 3,
                evt_type: 0
            }
        );
        assert!(resp_is_done(0x0003_0000, 3));
        assert!(!resp_is_done(0x0013_0000, 3)); // evt=CMD_ERROR
        assert!(!resp_is_done(0x0003_0000, 4)); // wrong seq

        // Persistent response-TD ownership: only the exact outstanding ring
        // index can consume the buffer; errors, impossible residue, and a
        // sub-RXFCE short transfer are rejected.
        assert_eq!(
            classify_response_transfer(7, 6, 13, 1024, 1008),
            ResponseTransfer::Unowned
        );
        assert_eq!(
            classify_response_transfer(7, 7, 6, 1024, 1008),
            ResponseTransfer::Invalid
        );
        assert_eq!(
            classify_response_transfer(7, 7, 13, 1024, 1021),
            ResponseTransfer::Invalid
        );
        assert_eq!(
            classify_response_transfer(7, 7, 13, 1024, 1008),
            ResponseTransfer::Read { actual_len: 16 }
        );
        assert_eq!(
            classify_response_transfer(7, 7, 1, 1024, 0),
            ResponseTransfer::Read { actual_len: 1024 }
        );

        // TX and response DMA spans are disjoint; adjacency is not overlap.
        assert!(!dma_spans_overlap(0x1000, 0x400, 0x1400, 0x400));
        assert!(dma_spans_overlap(0x1000, 0x401, 0x1400, 0x400));
        assert!(dma_spans_overlap(u64::MAX - 1, 4, 0x1000, 0x400));

        // Byte-level ZLP exclusion for the two implicated transfers. The
        // channel RF17-20 RANDOM_WRITE is 40 bytes, and a full 0x3800-byte
        // firmware payload carries the existing four-byte TXINFO plus four-byte
        // trailer for 14,344 bytes. Neither is a 512-byte max-packet multiple.
        // Historical framing bugs are hypothesis-class precedent only; these
        // lengths do not establish one as TEST 3's proximate defect.
        let mut rf_payload = [0u8; regs::INBAND_PACKET_MAX_LEN];
        let rf_payload_len = build_reg_pair_payload(
            &mut rf_payload,
            regs::MCU_MEMMAP_RF,
            &[(17, 1), (18, 2), (19, 3), (20, 4)],
        )
        .expect("four RF pairs fit");
        let mut rf_frame = [0u8; regs::INBAND_PACKET_MAX_LEN + 8];
        let rf_frame_len = build_mcu_frame(
            &mut rf_frame,
            &rf_payload[..rf_payload_len],
            1,
            regs::CMD_RANDOM_WRITE,
        )
        .expect("RF command fits");
        assert_eq!(rf_frame_len, 40);
        assert_ne!(rf_frame_len % 512, 0);

        let mut firmware_frame = [0u8; 0x3800 + 8];
        let firmware_transfer_len =
            super::fw_logic::build_chunk(&mut firmware_frame, &[0u8; 0x3800])
                .expect("full firmware chunk fits");
        assert_eq!(firmware_transfer_len, 14_344);
        assert_ne!(firmware_transfer_len % 512, 0);

        // Mainline chunk-response policy removes intermediate response pressure.
        assert!(!wait_response_for_chunk(0, 3));
        assert!(!wait_response_for_chunk(1, 3));
        assert!(wait_response_for_chunk(2, 3));
        assert!(!wait_response_for_chunk(0, 0));

        // Fourteen repeated hops preserve exact completion ownership even as
        // sequence numbers and transfer-ring indices advance.
        for hop in 1usize..=14 {
            let pending = (60 + hop) % 63; // exercise the ring wrap boundary
            let stale = (pending + 62) % 63;
            assert_eq!(
                classify_response_transfer(pending, stale, 13, 1024, 1008),
                ResponseTransfer::Unowned
            );
            assert_eq!(
                classify_response_transfer(pending, pending, 13, 1024, 1008),
                ResponseTransfer::Read { actual_len: 16 }
            );
            let seq = ((hop - 1) % 15 + 1) as u8;
            assert!(resp_is_done((seq as u32) << 16, seq));
        }
    }

    /// ISC-4: eFUSE control-word, usage-map HARD-FAIL, EEPROM field decode.
    /// The confirmed adapter's discriminator targets: ver 0x0d and its MAC
    /// (DISPATCH §2); the MAC here is a synthetic locally administered one.
    #[test]
    fn mt7601u_w1_eeprom_oracle() {
        use super::eeprom_logic::*;
        // efuse_ctrl_kick(cur=0, addr=0x10, mode=EE_READ): AIN(0x10)<<16 | KICK.
        assert_eq!(efuse_ctrl_kick(0, 0x10, super::regs::EE_READ), 0x4010_0000);
        assert!(efuse_out_of_map(0x3f)); // AOUT all-ones
        assert!(!efuse_out_of_map(0x10));

        // Hand-laid 256-byte EEPROM image: ver 0x0d, fae 0x00, the MAC, country.
        let mut ee = [0u8; super::regs::EEPROM_SIZE];
        ee[0x02] = 0x00;
        ee[0x03] = 0x0d;
        ee[0x04..0x0a].copy_from_slice(&[0x02, 0x00, 0x00, 0x00, 0x76, 0x01]);
        ee[0x39] = 0x01;
        assert_eq!(version(&ee), 0x0d);
        assert_eq!(version_fae(&ee), 0x00);
        assert_eq!(mac(&ee), [0x02, 0x00, 0x00, 0x00, 0x76, 0x01]);
        assert!(mac_is_valid(&mac(&ee)));
        assert_eq!(country_region(&ee), 0x01);
        assert!(!version_unsupported(&ee));
        let mut ee_new = ee;
        ee_new[0x03] = 0x0e;
        assert!(version_unsupported(&ee_new));

        // Usage-map HARD-FAIL: a fully-programmed map passes; a blank (all-zero,
        // or zeros spread end-to-end) fails — the blank-clone abort (T-011).
        let programmed = [0xffu8; super::regs::EFUSE_USAGE_MAP_SIZE];
        assert!(usage_map_ok(&programmed).is_ok());
        let mut clustered = [0xffu8; super::regs::EFUSE_USAGE_MAP_SIZE];
        clustered[0] = 0;
        clustered[1] = 0;
        clustered[2] = 0; // 3 contiguous free words → still programmed
        assert!(usage_map_ok(&clustered).is_ok());
        let blank = [0u8; super::regs::EFUSE_USAGE_MAP_SIZE];
        assert!(usage_map_ok(&blank).is_err());
        let mut spread = [0xffu8; super::regs::EFUSE_USAGE_MAP_SIZE];
        spread[0] = 0;
        spread[super::regs::EFUSE_USAGE_MAP_SIZE - 1] = 0; // free span end-to-end
        assert!(usage_map_ok(&spread).is_err());
    }

    /// ISC-5: RX DMA segment walk + RXWI strip + beacon → BSSID/SSID.
    #[test]
    fn mt7601u_w1_rx_beacon_oracle() {
        use super::rx_logic::*;
        // Build the 802.11 beacon frame (44 bytes, 4-aligned).
        let mut frame = Vec::new();
        frame.extend_from_slice(&[0x80, 0x00]); // FC: beacon
        frame.extend_from_slice(&[0x00, 0x00]); // duration
        frame.extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff]); // addr1 DA
        frame.extend_from_slice(&[0x00, 0x11, 0x22, 0x33, 0x44, 0x55]); // addr2 SA
        frame.extend_from_slice(&[0x00, 0x11, 0x22, 0x33, 0x44, 0x55]); // addr3 BSSID
        frame.extend_from_slice(&[0x00, 0x00]); // seq ctl
        frame.extend_from_slice(&[0; 8]); // timestamp
        frame.extend_from_slice(&[0x64, 0x00]); // beacon interval
        frame.extend_from_slice(&[0x01, 0x00]); // capability
        frame.extend_from_slice(&[0x00, 0x04, b'T', b'e', b's', b't']); // SSID IE
        frame.extend_from_slice(&[0x01, 0x00]); // (empty rates IE → 44 bytes)
        assert_eq!(frame.len(), 44);

        // Frame-level extraction.
        let bss = frame_bssid(&frame).expect("beacon");
        assert_eq!(bss.bssid, [0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        assert!(!bss.is_probe_resp);
        assert_eq!(frame_ssid(&frame), Some(&b"Test"[..]));
        // A data frame (FC byte0 not 0x80/0x50) yields no BSS.
        let mut data_frame = frame.clone();
        data_frame[0] = 0x08;
        assert_eq!(frame_bssid(&data_frame), None);

        // Wrap in a DMA segment: [4B DMA_HDR][28B RXWI][frame][4B FCE_INFO].
        // dma_len = frame_len(44) + RXWI(28) = 72 (4-aligned, ≥ MIN_SEG_LEN).
        let dma_len = 44 + RXWI_SIZE;
        let mut seg = Vec::new();
        seg.extend_from_slice(&(dma_len as u32).to_le_bytes()); // DMA_HDR (dma_len in low 16)
        let mut rxwi = [0u8; RXWI_SIZE];
        rxwi[4..8].copy_from_slice(&((44u32) << 16).to_le_bytes()); // ctl: MPDU_LEN=44
        seg.extend_from_slice(&rxwi);
        seg.extend_from_slice(&frame);
        seg.extend_from_slice(&[0u8; 4]); // FCE_INFO (TYPE=0 ⇒ data pkt)
        assert_eq!(seg.len(), 8 + dma_len);

        assert_eq!(next_seg_len(&seg), Some(8 + dma_len));
        assert_eq!(rxwi_mpdu_len(&seg), Some(44));
        assert!(rxwi_zero_ok(&seg));
        let parsed = segment_frame(&seg).expect("frame");
        assert_eq!(parsed, &frame[..]);
        assert_eq!(
            frame_bssid(parsed).unwrap().bssid,
            [0x00, 0x11, 0x22, 0x33, 0x44, 0x55]
        );

        // Malformed dma_len (not 4-aligned) → rejected.
        let mut bad = seg.clone();
        bad[0] = bad[0].wrapping_add(1);
        assert_eq!(next_seg_len(&bad), None);
    }

    // ── Phase 7.5 BUILD-W2 ISC-1: 802.11 mgmt-frame byte-oracles ─────────────
    // Hand-laid fixtures derived from IEEE 802.11 + the raw `ieee80211.h`/
    // `nl80211.h` constants, INDEPENDENT of mlme_logic.rs's builders. Each fixture
    // was triple-checked: raw header → mlme_logic, raw header → this fixture, and
    // raw header → an independent Forge derivation (all three agree). STA
    // aa:bb:cc:dd:ee:ff, AP 11:22:33:44:55:66 throughout.
    const W2_STA: [u8; 6] = [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff];
    const W2_AP: [u8; 6] = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];

    /// ISC-1: Open-System auth request (transaction 1), STA→AP — 30 bytes.
    #[test]
    fn mt7601u_w2_isc1_open_auth_oracle() {
        use super::mlme_logic::*;
        // FC=0x00B0(LE b0 00) | dur 0 | da=AP | sa=STA | bssid=AP | seq 0 |
        // auth_alg=OPEN(0) | transaction=1 | status=0.
        let want: [u8; 30] = [
            0xb0, 0x00, 0x00, 0x00, // FC, duration
            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, // da = AP
            0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, // sa = STA
            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, // bssid = AP
            0x00, 0x00, // seq_ctrl
            0x00, 0x00, // auth_alg = OPEN
            0x01, 0x00, // transaction = 1
            0x00, 0x00, // status = 0
        ];
        let mut buf = [0u8; 64];
        let n = build_open_auth(&mut buf, &W2_STA, &W2_AP).expect("build");
        assert_eq!(n, 30);
        assert_eq!(&buf[..n], &want[..]);
    }

    /// ISC-1: WPA2-PSK RSN IE — 22 bytes (EID 48, len 20), CCMP+PSK.
    #[test]
    fn mt7601u_w2_isc1_rsn_ie_oracle() {
        use super::mlme_logic::*;
        let want: [u8; 22] = [
            0x30, 0x14, // EID=48, len=20
            0x01, 0x00, // RSN version 1 (LE)
            0x00, 0x0f, 0xac, 0x04, // group cipher = CCMP
            0x01, 0x00, // pairwise count 1 (LE)
            0x00, 0x0f, 0xac, 0x04, // pairwise[0] = CCMP
            0x01, 0x00, // AKM count 1 (LE)
            0x00, 0x0f, 0xac, 0x02, // AKM[0] = PSK (type 2, NOT 1)
            0x00, 0x00, // RSN capabilities
        ];
        let mut buf = [0u8; 32];
        let n = build_rsn_ie_wpa2_psk(&mut buf, 0).expect("rsn");
        assert_eq!(n, 22);
        assert_eq!(&buf[..n], &want[..]);
    }

    /// ISC-1: Assoc Request with SSID/rates/RSN — 61 bytes.
    #[test]
    fn mt7601u_w2_isc1_assoc_req_oracle() {
        use super::mlme_logic::*;
        let want: [u8; 61] = [
            0x00, 0x00, 0x00, 0x00, // FC=ASSOC_REQ(0x0000), duration
            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, // da = AP
            0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, // sa = STA
            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, // bssid = AP
            0x00, 0x00, // seq_ctrl
            0x01, 0x00, // capab = ESS (LE)
            0x0a, 0x00, // listen_interval = 10 (LE)
            0x00, 0x03, 0x4e, 0x65, 0x6f, // SSID IE: id0 len3 "Neo"
            0x01, 0x04, 0x82, 0x84, 0x8b, 0x96, // SuppRates IE: id1 len4
            0x30, 0x14, 0x01, 0x00, 0x00, 0x0f, 0xac, 0x04, 0x01, 0x00, 0x00, 0x0f, 0xac, 0x04,
            0x01, 0x00, 0x00, 0x0f, 0xac, 0x02, 0x00, 0x00, // RSN IE
        ];
        let mut buf = [0u8; 128];
        let n = build_assoc_req(
            &mut buf,
            &W2_STA,
            &W2_AP,
            &AssocReqParams {
                ssid: b"Neo",
                rates: &[0x82, 0x84, 0x8b, 0x96],
                capab: CAP_ESS,
                listen_interval: 10,
                rsn: true,
            },
        )
        .expect("assoc");
        assert_eq!(n, 61);
        assert_eq!(&buf[..n], &want[..]);
    }

    /// ISC-1: parse an auth RESPONSE (alg=OPEN, transaction 2, status 0).
    #[test]
    fn mt7601u_w2_isc1_parse_auth() {
        use super::mlme_logic::*;
        let mut frame = [0u8; 30];
        frame[0] = 0xb0; // FC = mgmt|auth
        frame[24] = 0x00; // alg = OPEN (LE)
        frame[26] = 0x02; // transaction = 2 (LE)
        frame[28] = 0x00; // status = 0
        let a = parse_auth(&frame).expect("auth");
        assert_eq!(
            a,
            AuthFrame {
                alg: 0,
                transaction: 2,
                status: 0
            }
        );
        // A non-auth subtype is rejected.
        let mut bad = frame;
        bad[0] = 0x00; // assoc-req subtype
        assert_eq!(parse_auth(&bad), None);
        // Truncated → None.
        assert_eq!(parse_auth(&frame[..29]), None);
    }

    /// ISC-1: parse an assoc RESPONSE; AID masked to the non-S1G 0x07FF range.
    #[test]
    fn mt7601u_w2_isc1_parse_assoc_resp() {
        use super::mlme_logic::*;
        let mut frame = [0u8; 30];
        frame[0] = 0x10; // FC = mgmt|assoc_resp (0x0010)
        frame[24] = 0x11;
        frame[25] = 0x04; // capab = 0x0411 (LE)
        frame[26] = 0x00;
        frame[27] = 0x00; // status = 0
        frame[28] = 0x01;
        frame[29] = 0xc0; // aid field = 0xC001 → masked 0x0001
        let r = parse_assoc_resp(&frame).expect("assoc-resp");
        assert_eq!(
            r,
            AssocResp {
                capab: 0x0411,
                status: 0,
                aid: 0x0001
            }
        );
        // Reassoc-resp subtype (0x0030) shares the layout and is accepted.
        let mut re = frame;
        re[0] = 0x30;
        assert!(parse_assoc_resp(&re).is_some());
    }

    /// ISC-1: IE walker finds elements by ID and rejects truncation.
    #[test]
    fn mt7601u_w2_isc1_find_ie() {
        use super::mlme_logic::*;
        let mut buf = [0u8; 128];
        let n = build_assoc_req(
            &mut buf,
            &W2_STA,
            &W2_AP,
            &AssocReqParams {
                ssid: b"Neo",
                rates: &[0x82, 0x84, 0x8b, 0x96],
                capab: CAP_ESS,
                listen_interval: 10,
                rsn: true,
            },
        )
        .unwrap();
        // IEs begin after the 24B header + capab(2) + listen(2) = offset 28.
        let body = &buf[28..n];
        assert_eq!(find_ie(body, EID_SSID), Some(&b"Neo"[..]));
        assert_eq!(
            find_ie(body, EID_SUPP_RATES),
            Some(&[0x82, 0x84, 0x8b, 0x96][..])
        );
        assert!(find_ie(body, EID_RSN).is_some());
        assert_eq!(find_ie(body, 0x99), None);
        // A length that runs past the buffer → None (no panic).
        assert_eq!(find_ie(&[0x00, 0x05, 0x41], EID_SSID), None);
    }

    /// ISC-1: the MLME state machine — open short-circuit vs WPA2 handshake path.
    #[test]
    fn mt7601u_w2_isc1_mlme_state() {
        use super::mlme_logic::*;
        use MlmeEvent::*;
        use MlmeState::*;
        // Auth success advances to Associating; auth failure → Failed.
        assert_eq!(mlme_next(Authenticating, AuthResp(0), false), Associating);
        assert_eq!(mlme_next(Authenticating, AuthResp(1), false), Failed);
        // Assoc success: open → Connected directly; WPA2 → Handshaking.
        assert_eq!(mlme_next(Associating, AssocResp(0), true), Connected);
        assert_eq!(mlme_next(Associating, AssocResp(0), false), Handshaking);
        assert_eq!(mlme_next(Associating, AssocResp(1), false), Failed);
        // Handshake completion → Connected; a deauth/reset anywhere → Failed.
        assert_eq!(mlme_next(Handshaking, HandshakeDone, false), Connected);
        assert_eq!(mlme_next(Connected, Reset, false), Failed);
        // An irrelevant event leaves the state unchanged.
        assert_eq!(
            mlme_next(Authenticating, HandshakeDone, false),
            Authenticating
        );
    }

    // ── Phase 7.5 BUILD-W2 ISC-1 (TX path): TXWI + DMA-wrap byte-oracles ──────
    // Hand-laid fixtures from mac.h (struct mt76_txwi), tx.c (push_txwi), and
    // dma.h (the [TXINFO][TXWI][frame][pad][zero] wrap), independent of tx_logic.

    /// ISC-1: the 20-byte TXWI for a 30-byte mgmt frame, wcid 1, rate 0, ACK req.
    #[test]
    fn mt7601u_w2_isc1_txwi_oracle() {
        use super::tx_logic::*;
        // flags=0 | rate_ctl=0 | ack_ctl=ACK_REQ(0x01) | wcid=0x01 |
        // len_ctl=0x001e (byte_cnt=30, pktid=0) | iv=0 | eiv=0 | aid=0 |
        // txstream=0 | ctl=0.
        let want: [u8; 20] = [
            0x00, 0x00, // flags
            0x00, 0x00, // rate_ctl
            0x01, // ack_ctl = ACK_CTL_REQ
            0x01, // wcid
            0x1e, 0x00, // len_ctl = 30 (LE)
            0x00, 0x00, 0x00, 0x00, // iv
            0x00, 0x00, 0x00, 0x00, // eiv
            0x00, // aid
            0x00, // txstream
            0x00, 0x00, // ctl
        ];
        let mut buf = [0xffu8; 20];
        let n = build_txwi(
            &mut buf,
            &TxwiParams {
                mpdu_len: 30,
                wcid: 1,
                rate_ctl: 0,
                ack: true,
                pkt_id: 0,
            },
        )
        .expect("txwi");
        assert_eq!(n, 20);
        assert_eq!(&buf[..n], &want[..]);
        // The pkt-id packs into len_ctl bits 12-15; broadcast → NSEQ ack.
        let mut b2 = [0u8; 20];
        build_txwi(
            &mut b2,
            &TxwiParams {
                mpdu_len: 0x123,
                wcid: 0,
                rate_ctl: 0,
                ack: false,
                pkt_id: 0xa,
            },
        )
        .unwrap();
        assert_eq!(u16::from_le_bytes([b2[6], b2[7]]), 0x123 | (0xa << 12));
        assert_eq!(b2[4], TXWI_ACK_CTL_NSEQ);
    }

    /// ISC-1: the 4-byte TXINFO word — LEN=round_up(content,4), WLAN_PORT,
    /// DMA_PACKET, plus the QSEL/80211 flags.
    #[test]
    fn mt7601u_w2_isc1_txinfo_oracle() {
        use super::tx_logic::*;
        // content = 20 (TXWI) + 30 (frame) = 50 → round_up 52 = 0x34.
        // WLAN_PORT(0)<<27 | DMA_PACKET(0)<<30 = 0. flags=0 → 0x00000034.
        assert_eq!(dma_pkt_info(50, 0), 0x0000_0034);
        // Mgmt flags: QSEL_MGMT(0)<<25 | 80211(1<<19) = 0x0008_0000.
        assert_eq!(pkt_flags(QSEL_MGMT, true), 0x0008_0000);
        assert_eq!(dma_pkt_info(50, pkt_flags(QSEL_MGMT, true)), 0x0008_0034);
        // QSEL_EDCA(2)<<25 = 0x0400_0000; no 80211 bit.
        assert_eq!(pkt_flags(QSEL_EDCA, false), 0x0400_0000);
        // Unencrypted mgmt flags (dma.c): 80211(1<<19) | WIV(1<<24) | QSEL_EDCA.
        assert_eq!(mgmt_tx_flags(), 0x0008_0000 | 0x0100_0000 | 0x0400_0000);
        assert_eq!(mgmt_tx_flags(), 0x0508_0000);
    }

    /// ISC-1: the full USB TX buffer [TXINFO][TXWI][frame][pad][4B zero].
    #[test]
    fn mt7601u_w2_isc1_tx_frame_oracle() {
        use super::tx_logic::*;
        // 30-byte frame of 0xAA. content=50 → padded 52 → total = 4+52+4 = 60.
        let frame = [0xaau8; 30];
        let mut dst = [0x55u8; 80];
        let total = build_tx_frame(&mut dst, &frame, 1, 0, true, 0, pkt_flags(QSEL_MGMT, true))
            .expect("tx frame");
        assert_eq!(total, 60);
        // [0..4] TXINFO: LEN=52, WLAN/PACKET, mgmt flags → 0x0008_0034.
        assert_eq!(
            u32::from_le_bytes([dst[0], dst[1], dst[2], dst[3]]),
            0x0008_0034
        );
        // [4..24] TXWI matches a standalone build for the same params.
        let mut txwi = [0u8; 20];
        build_txwi(
            &mut txwi,
            &TxwiParams {
                mpdu_len: 30,
                wcid: 1,
                rate_ctl: 0,
                ack: true,
                pkt_id: 0,
            },
        )
        .unwrap();
        assert_eq!(&dst[4..24], &txwi[..]);
        // [24..54] the frame; [54..56] content-pad; [56..60] trailing zero.
        assert_eq!(&dst[24..54], &frame[..]);
        assert_eq!(&dst[54..60], &[0u8; 6][..]);
    }

    /// ISC-2: EAPOL-over-802.11-data encapsulation (the 4-way transport).
    #[test]
    fn mt7601u_w2_isc2_eapol_data_oracle() {
        use super::mlme_logic::*;
        // STA→AP (To-DS): FC=0x0108 | data hdr | LLC/SNAP EAPOL | payload.
        let eapol = [0x01u8, 0x02, 0x03, 0x04];
        let want: [u8; 36] = [
            0x08, 0x01, 0x00, 0x00, // FC=data|ToDS(0x0108), duration
            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, // addr1 = BSSID (AP)
            0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, // addr2 = STA
            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, // addr3 = AP
            0x00, 0x00, // seq
            0xaa, 0xaa, 0x03, 0x00, 0x00, 0x00, 0x88, 0x8e, // LLC/SNAP EAPOL
            0x01, 0x02, 0x03, 0x04, // EAPOL payload
        ];
        let mut buf = [0u8; 64];
        let n = build_eapol_data(&mut buf, &W2_STA, &W2_AP, &eapol).expect("eapol data");
        assert_eq!(n, 36);
        assert_eq!(&buf[..n], &want[..]);

        // RX (From-DS) round-trips back to the payload.
        let mut rx = want;
        rx[0] = 0x08;
        rx[1] = 0x02; // FC=data|FromDS(0x0208)
        assert_eq!(parse_eapol_data(&rx), Some(&eapol[..]));

        // A QoS data frame (subtype bit 0x80) has a 26-byte header before SNAP.
        let mut qos = [0u8; 38];
        qos[0] = 0x88; // FC=qos-data(0x0088) low byte
        qos[1] = 0x02; // From-DS
        qos[26..34].copy_from_slice(&LLC_SNAP_EAPOL);
        qos[34..38].copy_from_slice(&eapol);
        assert_eq!(parse_eapol_data(&qos), Some(&eapol[..]));

        // A non-EAPOL SNAP EtherType → rejected.
        let mut bad = want;
        bad[30] = 0x08;
        bad[31] = 0x00; // IPv4 EtherType, not EAPOL
        assert_eq!(parse_eapol_data(&bad), None);
    }

    /// ISC-4: 802.3↔802.11 data conversion (the NetDevice seam's encap).
    #[test]
    fn mt7601u_w2_isc4_data_frame_oracle() {
        use super::mlme_logic::*;
        let da = [0x12u8, 0x34, 0x56, 0x78, 0x9a, 0xbc];
        let payload = [0xdeu8, 0xad, 0xbe, 0xef];
        // To-DS: FC=0x0108 | addr1=BSSID | addr2=STA | addr3=DA | LLC/SNAP |
        // EtherType 0x0800 (IPv4, BE) | payload.
        let want: [u8; 36] = [
            0x08, 0x01, 0x00, 0x00, // FC=data|ToDS, duration
            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, // addr1 = BSSID (AP)
            0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, // addr2 = STA
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, // addr3 = DA
            0x00, 0x00, // seq
            0xaa, 0xaa, 0x03, 0x00, 0x00, 0x00, // LLC/SNAP prefix
            0x08, 0x00, // EtherType = IPv4 (BE)
            0xde, 0xad, 0xbe, 0xef, // payload
        ];
        let mut buf = [0u8; 64];
        let n = build_data_frame(&mut buf, &W2_STA, &W2_AP, &da, 0x0800, &payload).unwrap();
        assert_eq!(n, 36);
        assert_eq!(&buf[..n], &want[..]);
        // RX (From-DS) round-trips to (EtherType, payload).
        let mut rx = want;
        rx[1] = 0x02; // From-DS
        assert_eq!(parse_data_frame(&rx), Some((0x0800u16, &payload[..])));
        // A QoS data frame uses a 26-byte header before the SNAP (sized exactly
        // so the returned payload slice ends at the frame end).
        let mut qos = [0u8; 38];
        qos[0] = 0x88; // data | QoS subtype
        qos[1] = 0x02; // From-DS
        qos[26..32].copy_from_slice(&LLC_SNAP_PREFIX);
        qos[32..34].copy_from_slice(&0x0806u16.to_be_bytes()); // ARP
        qos[34..38].copy_from_slice(&payload);
        assert_eq!(parse_data_frame(&qos), Some((0x0806u16, &payload[..])));
        // A mgmt frame (not data) → None.
        let mut mgmt = want;
        mgmt[0] = 0x80; // beacon subtype, FTYPE_MGMT
        assert_eq!(parse_data_frame(&mgmt), None);
    }

    /// ISC-2: the WCID key-table install encoding (IV/PN seed + ATTR word).
    #[test]
    fn mt7601u_w2_isc2_wcid_key_oracle() {
        use super::key_logic::{build_wcid_iv, wcid_attr_word};
        use super::regs::{CIPHER_AES_CCMP, CIPHER_NONE};
        // CCMP (>= TKIP): iv[0]|=1 (PN starts at 1), iv[3] = keyidx<<6 | 0x20.
        assert_eq!(
            build_wcid_iv(0, CIPHER_AES_CCMP),
            [0x01, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00]
        );
        assert_eq!(
            build_wcid_iv(2, CIPHER_AES_CCMP),
            [0x01, 0x00, 0x00, 0xa0, 0x00, 0x00, 0x00, 0x00] // 2<<6=0x80 | 0x20
        );
        // CIPHER_NONE (< TKIP): no PN bits, just keyidx<<6.
        assert_eq!(build_wcid_iv(0, CIPHER_NONE), [0u8; 8]);
        // ATTR: PKEY_MODE = CCMP(4)<<1 = 0x08; EXT=0; PAIRWISE bit set.
        assert_eq!(wcid_attr_word(0, CIPHER_AES_CCMP, true), 0x09);
        assert_eq!(wcid_attr_word(0, CIPHER_AES_CCMP, false), 0x08);
        // Other ATTR bits (BSS index, etc.) are preserved; only mode+pairwise change.
        assert_eq!(
            wcid_attr_word(0xffff_ffff, CIPHER_AES_CCMP, false),
            0xffff_fbf8
        );
    }

    /// CB-1 ISC-3: the GTK-KDE byte oracle. A hand-laid-out GTK KDE — offsets
    /// confirmed against RAW hostap source (`w1.fi/cgit/hostap/plain/src/common/
    /// wpa_common.h`: `struct wpa_gtk_kde { u8 keyid; u8 reserved; u8 gtk[]; }`,
    /// `WPA_GTK_KDE_PREFIX_LEN = 2`, `RSN_KEY_DATA_GROUPKEY = RSN_SELECTOR(0x00,
    /// 0x0f,0xac,1)`) — asserted against `eapol::gtk_from_kde`, independent of any
    /// writer. An off-by-one in the keyid/reserved prefix or a wrong OUI/type check
    /// is caught here; a writer/reader round-trip alone would miss it.
    #[test]
    fn wpa_cb1_gtk_kde_byte_oracle() {
        use super::wpa::eapol::gtk_from_kde;
        // GTK KDE: DD | len | 00 0F AC 01 | keyid | reserved | GTK[16].
        //   keyid 0x02 → KeyID bits[1:0]=2, Tx bit2=0 ⇒ index 2; reserved 0x00.
        //   GTK = 0x11..=0x20 (a distinct 16-byte pattern); len 0x16 = 4+2+16 = 22.
        let gtk: [u8; 16] = [
            0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e,
            0x1f, 0x20,
        ];
        let mut kde = [0u8; 24];
        kde[..8].copy_from_slice(&[0xdd, 0x16, 0x00, 0x0f, 0xac, 0x01, 0x02, 0x00]);
        kde[8..].copy_from_slice(&gtk);
        let (out, idx) = gtk_from_kde(&kde).expect("GTK KDE parsed");
        assert_eq!(out, &gtk[..], "GTK = the 16 bytes after keyid+reserved");
        assert_eq!(idx, 2, "key index = keyid byte bits[1:0]");

        // Tx (bit 2) must not bleed into the index: keyid 0x05 = KeyID 1 | Tx.
        let mut kde_tx = kde;
        kde_tx[6] = 0x05;
        assert_eq!(
            gtk_from_kde(&kde_tx).unwrap().1,
            1,
            "Tx bit masked off the index"
        );

        // A preceding non-GTK KDE (RSN selector type 4 ≠ GROUPKEY) is walked past.
        let mut prefixed = [0u8; 8 + 24];
        prefixed[..8].copy_from_slice(&[0xdd, 0x06, 0x00, 0x0f, 0xac, 0x04, 0xaa, 0xbb]);
        prefixed[8..].copy_from_slice(&kde);
        let (out2, idx2) = gtk_from_kde(&prefixed).expect("GTK found after a leading non-GTK KDE");
        assert_eq!(out2, &gtk[..]);
        assert_eq!(idx2, 2);

        // Negatives → None: wrong OUI, wrong selector type, truncated GTK, empty.
        let mut bad_oui = kde;
        bad_oui[2..5].copy_from_slice(&[0x00, 0x00, 0x00]);
        assert!(gtk_from_kde(&bad_oui).is_none(), "wrong OUI rejected");
        let mut bad_type = kde;
        bad_type[5] = 0x02;
        assert!(
            gtk_from_kde(&bad_type).is_none(),
            "non-GROUPKEY selector rejected"
        );
        assert!(gtk_from_kde(&kde[..20]).is_none(), "truncated GTK rejected");
        assert!(gtk_from_kde(&[]).is_none());
    }

    /// CB-1 ISC-6: the group-rx kill-switch gate regression. OFF ≡ no GTK install
    /// (pairwise-only, the W2 association-only baseline) even with a valid GTK KDE
    /// present; ON defers to the parser. The falsifiable "disabled ≡ baseline" proof
    /// for the gate the 4-way M3 handler calls.
    #[test]
    fn wpa_cb1_group_rx_killswitch_gate() {
        use super::wpa::eapol::{group_key_to_install, gtk_from_kde};
        let gtk: [u8; 16] = [0x33; 16];
        let mut kde = [0u8; 24];
        kde[..8].copy_from_slice(&[0xdd, 0x16, 0x00, 0x0f, 0xac, 0x01, 0x00, 0x00]);
        kde[8..].copy_from_slice(&gtk);
        // OFF ⇒ None even with a valid GTK present ⇒ pairwise-only baseline.
        assert!(
            group_key_to_install(false, &kde).is_none(),
            "kill-switch off must not install a GTK"
        );
        // ON ⇒ exactly the parser's result (install proceeds).
        assert_eq!(group_key_to_install(true, &kde), gtk_from_kde(&kde));
        assert_eq!(group_key_to_install(true, &kde).unwrap().0, &gtk[..]);
        // ON but no GTK KDE ⇒ None (nothing to install).
        assert!(group_key_to_install(true, &[0u8; 8]).is_none());
    }

    /// OT-27a (ISC-1/ISC-3 + BUILD-2 C2): the cold-firmware-upload fault classifier
    /// — the blind-hardware discriminator bank as pure logic. Each captured-
    /// breadcrumb pattern must bucket into exactly one of FIVE causes so a blind
    /// Stage-B cold boot is conclusive on failure (not a stall), and a success
    /// returns `None`. BUILD-2 adds `DeviceUnresponsive` (bus drop) and proves it
    /// out-ranks the `EndpointStall`/`McuNoBoot` symptoms it also produces.
    #[test]
    fn mt7601u_ot27a_cold_upload_fault_classifier() {
        use super::upload_logic::{
            ColdUploadFault, ColdUploadProgress, classify_cold_upload_failure,
        };

        // A fully-successful upload classifies to nothing.
        let ok = ColdUploadProgress {
            blob_valid: true,
            chip_id_ok: true,
            bytes_total: 45_000,
            bytes_uploaded: 45_000,
            device_unresponsive: false,
            com_reg0_ever_nonzero: true,
            com_reg0_running: true,
        };
        assert_eq!(classify_cold_upload_failure(&ok), None);

        // Earliest failed precondition wins: a bad blob is a bad blob even if
        // every later breadcrumb is also empty (the root cause, not a symptom).
        let bad_blob = ColdUploadProgress {
            blob_valid: false,
            chip_id_ok: false,
            bytes_total: 0,
            bytes_uploaded: 0,
            device_unresponsive: false,
            com_reg0_ever_nonzero: false,
            com_reg0_running: false,
        };
        assert_eq!(
            classify_cold_upload_failure(&bad_blob),
            Some(ColdUploadFault::BlobMissingOrCorrupt)
        );

        // Right blob, wrong silicon (same USB-ID, different chip).
        let wrong_chip = ColdUploadProgress {
            blob_valid: true,
            chip_id_ok: false,
            bytes_total: 45_000,
            bytes_uploaded: 0,
            device_unresponsive: false,
            com_reg0_ever_nonzero: false,
            com_reg0_running: false,
        };
        assert_eq!(
            classify_cold_upload_failure(&wrong_chip),
            Some(ColdUploadFault::WrongVariant)
        );

        // BUILD-2: the device fell off the bus (all-ones / control-no-event). This
        // is the BUILD-1-on-silicon signature. It OUT-RANKS the symptoms it also
        // produces: an unresponsive device whose upload looks "incomplete" is
        // DeviceUnresponsive, NOT EndpointStall...
        let unresp_midupload = ColdUploadProgress {
            blob_valid: true,
            chip_id_ok: true,
            bytes_total: 45_000,
            bytes_uploaded: 20_000,
            device_unresponsive: true,
            com_reg0_ever_nonzero: false,
            com_reg0_running: false,
        };
        assert_eq!(
            classify_cold_upload_failure(&unresp_midupload),
            Some(ColdUploadFault::DeviceUnresponsive)
        );
        // ...and one whose (false-passed) upload looked "complete" is
        // DeviceUnresponsive, NOT McuNoBoot (the exact BUILD-1 mislabel).
        let unresp_complete = ColdUploadProgress {
            bytes_uploaded: 45_000,
            ..unresp_midupload
        };
        assert_eq!(
            classify_cold_upload_failure(&unresp_complete),
            Some(ColdUploadFault::DeviceUnresponsive)
        );

        // Right blob + chip, RESPONSIVE, but a transfer stalled partway — endpoint
        // stall (incomplete WITHOUT a bus drop).
        let stalled = ColdUploadProgress {
            blob_valid: true,
            chip_id_ok: true,
            bytes_total: 45_000,
            bytes_uploaded: 20_000,
            device_unresponsive: false,
            com_reg0_ever_nonzero: false,
            com_reg0_running: false,
        };
        assert_eq!(
            classify_cold_upload_failure(&stalled),
            Some(ColdUploadFault::EndpointStall)
        );

        // Everything egressed, device RESPONSIVE, but COM_REG0 never reached 1 —
        // an HONEST no-boot (not a bus drop). Both "never stirred" and "stirred but
        // not 1" land here.
        let no_boot_quiet = ColdUploadProgress {
            blob_valid: true,
            chip_id_ok: true,
            bytes_total: 45_000,
            bytes_uploaded: 45_000,
            device_unresponsive: false,
            com_reg0_ever_nonzero: false,
            com_reg0_running: false,
        };
        let no_boot_stirred = ColdUploadProgress {
            com_reg0_ever_nonzero: true,
            ..no_boot_quiet
        };
        assert_eq!(
            classify_cold_upload_failure(&no_boot_quiet),
            Some(ColdUploadFault::McuNoBoot)
        );
        assert_eq!(
            classify_cold_upload_failure(&no_boot_stirred),
            Some(ColdUploadFault::McuNoBoot)
        );

        // `upload_complete` guards the empty-upload edge (0/0 is not "complete").
        assert!(!bad_blob.upload_complete());
        assert!(ok.upload_complete());
    }

    /// BUILD-2 C1: the all-ones-aware poll predicate. This is the fix for the
    /// BUILD-1 silicon wedge being masked — `0xffffffff & BIT(31) == BIT(31)` used
    /// to read as "DMA done", so a dead device looked like a completed upload.
    #[test]
    fn mt7601u_build2_poll_step_predicate() {
        use super::upload_logic::{BUS_DROP_SENTINEL, PollStep, poll_step};

        // All-ones ⇒ Unresponsive — even for a mask whose bits are a subset (the
        // exact BUILD-1 false-pass on the COM_REG1 DMA-done poll).
        assert_eq!(
            poll_step(0xffff_ffff, 1 << 31, 1 << 31),
            PollStep::Unresponsive
        );
        assert_eq!(
            poll_step(BUS_DROP_SENTINEL, u32::MAX, 1),
            PollStep::Unresponsive
        );

        // The awaited condition ⇒ Done.
        assert_eq!(poll_step(1 << 31, 1 << 31, 1 << 31), PollStep::Done); // COM_REG1 DMA-done
        assert_eq!(poll_step(1, u32::MAX, 1), PollStep::Done); // COM_REG0 == 1
        // A real DMA-done word with other status bits set (NOT all-ones) ⇒ Done.
        assert_eq!(poll_step(0x8000_00a5, 1 << 31, 1 << 31), PollStep::Done);

        // Not-yet ⇒ Retry (keep polling until timeout).
        assert_eq!(poll_step(0, u32::MAX, 1), PollStep::Retry); // COM_REG0 == 0
        assert_eq!(poll_step(0x0000_00a5, 1 << 31, 1 << 31), PollStep::Retry); // bit31 clear
    }

    /// Warm post-firmware repair: prove the exact three-way WPDMA decision used
    /// by the device-touching poll. The all-ones negative control is
    /// load-bearing: the old mask-only predicate would call it busy and repeat
    /// the expensive nested EP0 retry; the new predicate must terminate it as
    /// device-unresponsive before considering the busy bits.
    #[test]
    fn mt7601u_warm_postfw_wpdma_decision_oracle() {
        use super::post_fw_logic::{DEVICE_UNRESPONSIVE, WpdmaStep, classify_wpdma};

        let busy = (1 << 1) | (1 << 3);

        assert_ne!(
            DEVICE_UNRESPONSIVE & busy,
            0,
            "negative control: the legacy mask-only check must see all-ones as busy"
        );
        assert_eq!(
            classify_wpdma(DEVICE_UNRESPONSIVE, busy),
            WpdmaStep::DeviceUnresponsive
        );

        assert_eq!(classify_wpdma(0, busy), WpdmaStep::Idle);
        assert_eq!(classify_wpdma(0x0000_0100, busy), WpdmaStep::Idle);

        assert_eq!(classify_wpdma(1 << 1, busy), WpdmaStep::Busy);
        assert_eq!(classify_wpdma(1 << 3, busy), WpdmaStep::Busy);
        assert_eq!(classify_wpdma(busy, busy), WpdmaStep::Busy);
    }

    /// AAG fix-forward: scripted model of the exact non-replenishing accounting
    /// core charged by xHCI + the MT7601U retry/poll layers. Every row pins both
    /// the terminal discriminator and the complete nested counter bank.
    #[test]
    fn mt7601u_postfw_operation_wide_budget_oracle() {
        use super::operation_budget_logic::{BudgetModel, ChargeKind};
        use super::post_fw_logic::{DEVICE_UNRESPONSIVE, WpdmaStep, classify_wpdma};

        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        enum Terminal {
            Idle,
            Unresponsive,
            NoEvent,
            OperationBudget,
        }

        fn immediate_read(
            budget: &mut BudgetModel,
            value: u32,
            busy_mask: u32,
        ) -> Result<WpdmaStep, Terminal> {
            for charge in [
                ChargeKind::Observation,
                ChargeKind::VendorAttempt,
                ChargeKind::WaitCall,
            ] {
                if !budget.charge(charge) {
                    return Err(Terminal::OperationBudget);
                }
            }
            Ok(classify_wpdma(value, busy_mask))
        }

        let busy_mask = (1 << 1) | (1 << 3);

        // 1. Immediate idle.
        let mut idle = BudgetModel::new(16);
        assert_eq!(immediate_read(&mut idle, 0, busy_mask), Ok(WpdmaStep::Idle));
        assert_eq!(idle.counters().observations, 1);
        assert_eq!(idle.counters().vendor_attempts, 1);
        assert_eq!(idle.counters().wait_calls, 1);
        assert_eq!(idle.counters().charged_total(), 3);
        assert!(!idle.exhausted());

        // 2. Immediate all-ones after the Linux-shaped request exhausts ten
        // immediately failing transfers. The outer observation never repeats.
        let mut all_ones = BudgetModel::new(64);
        assert!(all_ones.charge(ChargeKind::Observation));
        for attempt in 0..10 {
            assert!(all_ones.charge(ChargeKind::VendorAttempt));
            assert!(all_ones.charge(ChargeKind::WaitCall)); // immediate error event
            if attempt != 9 {
                assert!(all_ones.charge(ChargeKind::VendorRetryGap));
            }
        }
        all_ones.note_vendor_request_exhaustion();
        assert_eq!(
            classify_wpdma(DEVICE_UNRESPONSIVE, busy_mask),
            WpdmaStep::DeviceUnresponsive
        );
        assert_eq!(all_ones.counters().charged_total(), 30);
        assert_eq!(all_ones.counters().observations, 1);
        assert_eq!(all_ones.counters().vendor_attempts, 10);
        assert_eq!(all_ones.counters().vendor_retry_gaps, 9);
        assert_eq!(all_ones.counters().vendor_request_exhaustions, 1);

        // 3. Persistent responsive busy. Four charges/read mean an exact
        // 32-unit ledger permits eight reads and no ninth observation.
        let mut persistent_busy = BudgetModel::new(32);
        let busy_terminal = loop {
            match immediate_read(&mut persistent_busy, busy_mask, busy_mask) {
                Ok(WpdmaStep::Busy) => {
                    if !persistent_busy.charge(ChargeKind::PollGap) {
                        break Terminal::OperationBudget;
                    }
                }
                Err(terminal) => break terminal,
                other => panic!("unexpected persistent-busy result: {other:?}"),
            }
        };
        assert_eq!(busy_terminal, Terminal::OperationBudget);
        assert_eq!(persistent_busy.counters().observations, 8);
        assert_eq!(persistent_busy.counters().vendor_attempts, 8);
        assert_eq!(persistent_busy.counters().wait_calls, 8);
        assert_eq!(persistent_busy.counters().poll_gaps, 8);
        assert_eq!(persistent_busy.counters().charged_total(), 32);
        assert!(persistent_busy.exhausted());

        // 4. Repeated missing events consume the same ledger. The failed wait
        // cannot replenish itself or advance to a vendor retry.
        let mut missing = BudgetModel::new(12);
        assert!(missing.charge(ChargeKind::Observation));
        assert!(missing.charge(ChargeKind::VendorAttempt));
        assert!(missing.charge(ChargeKind::WaitCall));
        while missing.charge(ChargeKind::WaitIteration) {}
        missing.note_no_event_exhaustion();
        let missing_terminal = Terminal::NoEvent;
        assert_eq!(missing_terminal, Terminal::NoEvent);
        assert_eq!(missing.counters().wait_iterations, 9);
        assert_eq!(missing.counters().no_event_exhaustions, 1);
        assert_eq!(missing.counters().vendor_attempts, 1);
        assert_eq!(missing.counters().charged_total(), 12);
        assert!(missing.exhausted());

        // 5. Seven tolerated late/non-status events followed by the status TRB.
        let mut late_then_status = BudgetModel::new(16);
        assert!(late_then_status.charge(ChargeKind::Observation));
        assert!(late_then_status.charge(ChargeKind::VendorAttempt));
        for _ in 0..7 {
            assert!(late_then_status.charge(ChargeKind::WaitCall));
            late_then_status.note_late_event();
        }
        assert!(late_then_status.charge(ChargeKind::WaitCall));
        assert_eq!(classify_wpdma(0, busy_mask), WpdmaStep::Idle);
        assert_eq!(late_then_status.counters().wait_calls, 8);
        assert_eq!(late_then_status.counters().late_events, 7);
        assert_eq!(late_then_status.counters().charged_total(), 10);

        // 6. A late event, then attempt failure; retry succeeds but reports
        // busy; the next observation succeeds idle. Counters cross every layer.
        let mut retry_then_busy = BudgetModel::new(24);
        assert!(retry_then_busy.charge(ChargeKind::Observation));
        assert!(retry_then_busy.charge(ChargeKind::VendorAttempt));
        assert!(retry_then_busy.charge(ChargeKind::WaitCall));
        retry_then_busy.note_late_event();
        assert!(retry_then_busy.charge(ChargeKind::WaitCall)); // terminal error event
        assert!(retry_then_busy.charge(ChargeKind::VendorRetryGap));
        assert!(retry_then_busy.charge(ChargeKind::VendorAttempt));
        assert!(retry_then_busy.charge(ChargeKind::WaitCall)); // status
        assert_eq!(classify_wpdma(busy_mask, busy_mask), WpdmaStep::Busy);
        assert!(retry_then_busy.charge(ChargeKind::PollGap));
        assert_eq!(
            immediate_read(&mut retry_then_busy, 0, busy_mask),
            Ok(WpdmaStep::Idle)
        );
        assert_eq!(retry_then_busy.counters().observations, 2);
        assert_eq!(retry_then_busy.counters().vendor_attempts, 3);
        assert_eq!(retry_then_busy.counters().vendor_retry_gaps, 1);
        assert_eq!(retry_then_busy.counters().wait_calls, 4);
        assert_eq!(retry_then_busy.counters().late_events, 1);
        assert_eq!(retry_then_busy.counters().poll_gaps, 1);
        assert_eq!(retry_then_busy.counters().charged_total(), 11);

        // 7. Alternating failed-attempt/busy observations used to replenish the
        // rejected per-wait/per-observation composition. The shared 24-unit
        // ledger stops during the fourth read's retry gap, exactly at 24.
        let mut alternating = BudgetModel::new(24);
        let alternating_terminal = 'outer: loop {
            for charge in [
                ChargeKind::Observation,
                ChargeKind::VendorAttempt,
                ChargeKind::WaitCall,
                ChargeKind::VendorRetryGap,
                ChargeKind::VendorAttempt,
                ChargeKind::WaitCall,
            ] {
                if !alternating.charge(charge) {
                    break 'outer Terminal::OperationBudget;
                }
            }
            assert_eq!(classify_wpdma(busy_mask, busy_mask), WpdmaStep::Busy);
            if !alternating.charge(ChargeKind::PollGap) {
                break Terminal::OperationBudget;
            }
        };
        assert_eq!(alternating_terminal, Terminal::OperationBudget);
        assert_eq!(alternating.counters().observations, 4);
        assert_eq!(alternating.counters().vendor_attempts, 7);
        assert_eq!(alternating.counters().vendor_retry_gaps, 3);
        assert_eq!(alternating.counters().wait_calls, 7);
        assert_eq!(alternating.counters().poll_gaps, 3);
        assert_eq!(alternating.counters().charged_total(), 24);
        assert!(alternating.exhausted());

        // Negative control: the rejected reset-at-each-layer composition admits
        // far more than the operation ceiling.
        let rejected_nested = 10_001u64 * 10u64 * 8u64 * 50_000u64 + 10_001 * 9 + 10_000;
        assert!(rejected_nested > 24);
        assert!(rejected_nested > 50_000);
    }

    /// AAG fix-forward: terminal PASS is a function of the durable lease/link
    /// evidence, never of association alone.
    #[test]
    fn mt7601u_dhcp_link_up_truth_oracle() {
        use super::link_result_logic::{
            DhcpTerminal, LinkEvidence, ack_terminal, pass_eligible, timeout_terminal,
        };

        let expected_ip = [192, 168, 1, 8];
        let ack = ack_terminal(expected_ip, true);
        assert_eq!(ack, DhcpTerminal::BoundLinkUp);
        assert!(pass_eligible(LinkEvidence {
            bound: true,
            connected: true,
            gtk_installed: true,
            dhcp: ack,
            assigned_ip: expected_ip,
            link_up: true,
        }));

        let no_offer = timeout_terminal(false);
        assert_eq!(no_offer, DhcpTerminal::TimeoutSelecting);
        assert!(!pass_eligible(LinkEvidence {
            bound: true,
            connected: true,
            gtk_installed: true,
            dhcp: no_offer,
            assigned_ip: [0; 4],
            link_up: false,
        }));

        let offer_no_ack = timeout_terminal(true);
        assert_eq!(offer_no_ack, DhcpTerminal::TimeoutRequesting);
        assert!(!pass_eligible(LinkEvidence {
            bound: true,
            connected: true,
            gtk_installed: true,
            dhcp: offer_no_ack,
            assigned_ip: [0; 4],
            link_up: false,
        }));

        // Negative control: the rejected harness predicate was effectively
        // bound && connected, so this row would have passed without DORA.
        let rejected_association_only = true;
        assert!(rejected_association_only);
        assert!(!pass_eligible(LinkEvidence {
            bound: true,
            connected: true,
            gtk_installed: true,
            dhcp: DhcpTerminal::NotRun,
            assigned_ip: [0; 4],
            link_up: false,
        }));

        assert_eq!(
            ack_terminal([0; 4], true),
            DhcpTerminal::InvalidLease,
            "ACK with zero yiaddr is never LINK-UP"
        );
    }

    /// OT-39 Fix B: CAL_BW is deferred only for the first successful targeted
    /// operating-channel tune, and a failed first tune does not consume the
    /// one-shot. The targeted path remains unchanged by the scan-path fix.
    #[test]
    fn mt7601u_ot39_first_targeted_tune_cal_bw_policy() {
        use super::wedge_logic::{TunePath, inline_cal_bw, note_targeted_tune_success};

        let completed = 0;
        assert!(!inline_cal_bw(TunePath::Targeted, completed));
        // A failed attempt leaves `completed` unchanged. This scheduling rule is
        // not the TEST-3 repair: RF17–20 precedes CAL_BW.
        assert!(!inline_cal_bw(TunePath::Targeted, completed));

        let completed = note_targeted_tune_success(completed);
        assert_eq!(completed, 1);
        assert!(inline_cal_bw(TunePath::Targeted, completed));
        assert_eq!(note_targeted_tune_success(u8::MAX), u8::MAX);
    }

    /// Scan-path CAL_BW parity: CHANNEL=0 selects the full survey, and raw
    /// upstream runs BW filter calibration inside every scan-channel tune before
    /// its scanning-state early return. A non-zero channel still selects the
    /// unchanged targeted fast path.
    #[test]
    fn mt7601u_scan_path_cal_bw_parity_and_channel_zero_route() {
        use super::wedge_logic::{
            TunePath, deferred_cal_pending_after_tune, inline_cal_bw, scan_tune_path,
        };

        assert_eq!(scan_tune_path(0), TunePath::Survey);
        assert_eq!(scan_tune_path(1), TunePath::Targeted);
        assert_eq!(scan_tune_path(14), TunePath::Targeted);

        assert!(inline_cal_bw(TunePath::Survey, 0));
        assert!(inline_cal_bw(TunePath::Survey, u8::MAX));
        assert!(deferred_cal_pending_after_tune(false));
        assert!(!deferred_cal_pending_after_tune(true));
    }

    /// Linux parity: resident firmware returns after USB-DMA enable with zero
    /// cold-preload/reset/FCE mutations. Cold firmware still selects the
    /// existing verified upload path.
    #[test]
    fn mt7601u_resident_firmware_linux_early_return_policy() {
        use super::linux_parity_logic::{
            FirmwareLoadAction, FirmwareLoadEffects, firmware_load_action, firmware_load_effects,
        };

        assert_eq!(firmware_load_action(true), FirmwareLoadAction::UseResident);
        assert_eq!(firmware_load_action(false), FirmwareLoadAction::UploadCold);
        assert_eq!(
            firmware_load_effects(FirmwareLoadAction::UseResident),
            FirmwareLoadEffects {
                preload_runs: 0,
                vendor_resets: 0,
                fce_pbf_writes: 0,
                cold_uploads: 0,
            }
        );
        assert_eq!(
            firmware_load_effects(FirmwareLoadAction::UploadCold),
            FirmwareLoadEffects {
                preload_runs: 1,
                vendor_resets: 1,
                fce_pbf_writes: 1,
                cold_uploads: 1,
            }
        );
    }

    #[test]
    fn mt7601u_linux_post_qselect_order_and_negative_control() {
        use super::linux_parity_logic::{
            FirmwareLoadAction, PacketRxAction, PostQselectTrace, firmware_load_effects,
            packet_rx_action, parity_violation, post_qselect_parity_violations, ready_for_mac_init,
        };

        let accepted = PostQselectTrace {
            resident_effects: firmware_load_effects(FirmwareLoadAction::UseResident),
            command_response_owned: true,
            packet_rx_posted: 16,
            packet_rx_required: 16,
        };
        assert_eq!(post_qselect_parity_violations(accepted), 0);
        assert!(ready_for_mac_init(true, 16, 16));
        assert_eq!(
            packet_rx_action(0, 16),
            PacketRxAction::PostMissing { count: 16 }
        );
        assert_eq!(packet_rx_action(16, 16), PacketRxAction::ReusePosted);

        let rejected = PostQselectTrace {
            resident_effects: super::linux_parity_logic::FirmwareLoadEffects {
                preload_runs: 1,
                vendor_resets: 1,
                fce_pbf_writes: 1,
                cold_uploads: 1,
            },
            command_response_owned: false,
            packet_rx_posted: 0,
            packet_rx_required: 16,
        };
        assert_eq!(
            post_qselect_parity_violations(rejected),
            parity_violation::RESIDENT_PRELOAD
                | parity_violation::RESIDENT_VENDOR_RESET
                | parity_violation::RESIDENT_FCE_PBF_WRITE
                | parity_violation::RESIDENT_COLD_UPLOAD
                | parity_violation::RESPONSE_NOT_OWNED
                | parity_violation::PACKET_RX_NOT_READY
        );
        assert!(!ready_for_mac_init(false, 0, 16));
        assert_eq!(packet_rx_action(17, 16), PacketRxAction::InvalidOwnership);
    }

    #[test]
    fn mt7601u_linux_mac_random_write_frames_and_response_policy() {
        use super::{initvals, mcu_logic, regs};

        let common_chunks = initvals::MAC_COMMON_VALS
            .len()
            .div_ceil(mcu_logic::MAX_REG_PAIRS_PER_CMD);
        assert_eq!(common_chunks, 2);
        assert!(!mcu_logic::wait_response_for_chunk(0, common_chunks));
        assert!(mcu_logic::wait_response_for_chunk(1, common_chunks));

        let mut payload = [0u8; regs::INBAND_PACKET_MAX_LEN];
        let first = &initvals::MAC_COMMON_VALS[..mcu_logic::MAX_REG_PAIRS_PER_CMD];
        let payload_len =
            mcu_logic::build_reg_pair_payload(&mut payload, regs::MCU_MEMMAP_WLAN, first)
                .expect("first MAC payload");
        assert_eq!(payload_len, 192);
        assert_eq!(&payload[..4], &0x0041_1408u32.to_le_bytes());
        assert_eq!(&payload[4..8], &0x0000_013fu32.to_le_bytes());
        let mut frame = [0u8; regs::INBAND_PACKET_MAX_LEN + 8];
        let frame_len = mcu_logic::build_mcu_frame(
            &mut frame,
            &payload[..payload_len],
            0,
            regs::CMD_RANDOM_WRITE,
        )
        .expect("first MAC frame");
        assert_eq!(frame_len, 200);
        assert_eq!(&frame[..4], &0x50c0_00c0u32.to_le_bytes());

        let final_common = &initvals::MAC_COMMON_VALS[mcu_logic::MAX_REG_PAIRS_PER_CMD..];
        let payload_len =
            mcu_logic::build_reg_pair_payload(&mut payload, regs::MCU_MEMMAP_WLAN, final_common)
                .expect("final common payload");
        assert_eq!(payload_len, 24);
        let frame_len = mcu_logic::build_mcu_frame(
            &mut frame,
            &payload[..payload_len],
            1,
            regs::CMD_RANDOM_WRITE,
        )
        .expect("final common frame");
        assert_eq!(frame_len, 32);
        assert_eq!(&frame[..4], &0x50c1_0018u32.to_le_bytes());

        assert_eq!(initvals::MAC_CHIP_VALS.len(), 17);
        assert!(mcu_logic::wait_response_for_chunk(0, 1));
        let payload_len = mcu_logic::build_reg_pair_payload(
            &mut payload,
            regs::MCU_MEMMAP_WLAN,
            initvals::MAC_CHIP_VALS,
        )
        .expect("chip payload");
        assert_eq!(payload_len, 136);
        let frame_len = mcu_logic::build_mcu_frame(
            &mut frame,
            &payload[..payload_len],
            2,
            regs::CMD_RANDOM_WRITE,
        )
        .expect("chip frame");
        assert_eq!(frame_len, 144);
        assert_eq!(&frame[..4], &0x50c2_0088u32.to_le_bytes());
    }

    #[test]
    fn mt7601u_post_qselect_timeout_is_bounded_terminal_and_flushed() {
        use super::linux_parity_logic::{
            POST_QSELECT_OPERATION_BUDGET, PostQselectFailureClass, PostQselectTrafficAction,
            classify_post_qselect_failure, post_qselect_failure_disposition,
            post_qselect_traffic_action,
        };
        use super::operation_budget_logic::{BudgetModel, ChargeKind};

        let mut budget = BudgetModel::new(POST_QSELECT_OPERATION_BUDGET);
        for _ in 0..POST_QSELECT_OPERATION_BUDGET {
            assert!(budget.charge(ChargeKind::BulkPoll));
        }
        assert!(!budget.charge(ChargeKind::BulkPoll));
        assert!(budget.exhausted());
        assert_eq!(budget.remaining(), 0);
        assert_eq!(
            classify_post_qselect_failure("inband bulk-out timeout"),
            PostQselectFailureClass::BulkOutTimeout
        );
        assert_eq!(
            classify_post_qselect_failure("mcu response timed out"),
            PostQselectFailureClass::ResponseTimeout
        );
        let disposition = post_qselect_failure_disposition();
        assert!(disposition.stop);
        assert!(!disposition.attempt_firmware_reload);
        assert!(!disposition.allow_later_device_traffic);
        assert!(disposition.flush_serial_and_sink);
        assert_eq!(
            post_qselect_traffic_action(false),
            PostQselectTrafficAction::Allow
        );
        assert_eq!(
            post_qselect_traffic_action(true),
            PostQselectTrafficAction::Block
        );
    }

    /// TEST-3 hostile oracle: channels 1–6 completed, the initial RF17–20
    /// RANDOM_WRITE timed out on channel 7 before CAL_BW,
    /// channel 7, both host endpoints recovered, both device halts cleared,
    /// and the forced reload transferred 0/45380 bytes. That trace is evidence
    /// against recovery. The accepted disposition is an evidence-neutral
    /// terminal command-path classification with every later stage fenced.
    /// Q-B non-destructive recovery: the drain/proof policy that replaces
    /// "endpoint reset, then re-issue the identical RF17-20 write".
    ///
    /// The `cea89e6a` silicon trace is the negative control throughout: endpoint
    /// recovery reported OK, the MCU command path was still latched, and the
    /// retry hung. Every assertion below exists to make that specific sequence
    /// unreachable.
    #[test]
    fn mt7601u_qb_non_destructive_recovery_policy() {
        use super::wedge_logic::{
            CHANNEL_RECOVERY_SEQUENCE, ChannelRecoveryObservation, ChannelRecoveryStep,
            ChannelRecoveryVerdict, ChannelRetryAction, MAX_MCU_DRAIN_COMMANDS, RecoveryBudgetPlan,
            channel_retry_action, classify_channel_recovery, drain_command_is_idempotent_noop,
            mcu_drain_depth, recovery_reserve_covers, recovery_retry_observation,
            recovery_worst_case_cost,
        };

        // ── The sequence is endpoint → drain → proof, and cold reload is not a
        // step at any position. The enum has no reload variant, so no future
        // reordering can reach one.
        assert_eq!(
            CHANNEL_RECOVERY_SEQUENCE,
            [
                ChannelRecoveryStep::EndpointRecovery,
                ChannelRecoveryStep::McuQueueDrain,
                ChannelRecoveryStep::CommandPathProbe,
            ]
        );

        // ── Drain depth is one no-op per prior hop, clamped to a survey span.
        assert_eq!(mcu_drain_depth(0), 0, "no prior hops ⇒ nothing accumulated");
        assert_eq!(mcu_drain_depth(6), 6, "the cea89e6a ch7 case: 6 prior hops");
        assert_eq!(
            mcu_drain_depth(MAX_MCU_DRAIN_COMMANDS),
            MAX_MCU_DRAIN_COMMANDS
        );
        assert_eq!(mcu_drain_depth(14), MAX_MCU_DRAIN_COMMANDS);
        assert_eq!(mcu_drain_depth(u8::MAX), MAX_MCU_DRAIN_COMMANDS);

        // ── The drain no-op must be an operation the device already accepted,
        // re-issued with identical arguments. `FUNC_Q_SELECT` / value 1 is the
        // post-firmware function-select; anything else could mutate state.
        const ACCEPTED_FUNC: u32 = 1; // regs::FUNC_Q_SELECT
        const ACCEPTED_VAL: u32 = 1; // POST_FW_Q_SELECT_VALUE
        assert!(drain_command_is_idempotent_noop(
            ACCEPTED_FUNC,
            ACCEPTED_VAL,
            ACCEPTED_FUNC,
            ACCEPTED_VAL
        ));
        assert!(
            !drain_command_is_idempotent_noop(2, ACCEPTED_VAL, ACCEPTED_FUNC, ACCEPTED_VAL),
            "a different function id is not a no-op"
        );
        assert!(
            !drain_command_is_idempotent_noop(ACCEPTED_FUNC, 0, ACCEPTED_FUNC, ACCEPTED_VAL),
            "the same function with a different value is not a no-op"
        );

        // ── Only an observed CMD_DONE proves the command path. This is the
        // whole point of Q-B step 3, so it is asserted against every near-miss.
        let reproven = ChannelRecoveryObservation {
            endpoint_probe_reported_ok: true,
            drain_commands_issued: 6,
            drain_cmd_done_observed: 6,
            probe_command_completed: true,
            probe_cmd_done_observed: true,
        };
        assert_eq!(
            classify_channel_recovery(reproven),
            ChannelRecoveryVerdict::CommandPathReproven
        );

        // The exact cea89e6a shape: transport healthy, MCU silent. Must NOT
        // authorize a retry — this is the assertion that forbids the hang.
        let cea89e6a = ChannelRecoveryObservation {
            endpoint_probe_reported_ok: true,
            drain_commands_issued: 6,
            drain_cmd_done_observed: 0,
            probe_command_completed: false,
            probe_cmd_done_observed: false,
        };
        assert_eq!(
            classify_channel_recovery(cea89e6a),
            ChannelRecoveryVerdict::CommandPathStillLatched
        );
        assert_eq!(
            channel_retry_action(recovery_retry_observation(cea89e6a)),
            ChannelRetryAction::ClassifyCommandPathUnusable,
            "endpoint-OK without an MCU CMD_DONE must never re-issue RF17-20"
        );

        // A completed transfer that never decoded CMD_DONE is not proof — the
        // same category error the endpoint probe made one layer up.
        assert_eq!(
            classify_channel_recovery(ChannelRecoveryObservation {
                probe_command_completed: true,
                probe_cmd_done_observed: false,
                ..reproven
            }),
            ChannelRecoveryVerdict::CommandPathStillLatched
        );
        // A successful drain is diagnostic, never authorization: if the final
        // proof fails, the drain's CMD_DONEs do not rescue the retry.
        assert_eq!(
            classify_channel_recovery(ChannelRecoveryObservation {
                drain_cmd_done_observed: 6,
                probe_command_completed: false,
                probe_cmd_done_observed: false,
                ..reproven
            }),
            ChannelRecoveryVerdict::CommandPathStillLatched
        );
        // And a dead transport is not rescued by a proof that somehow reported.
        assert_eq!(
            classify_channel_recovery(ChannelRecoveryObservation {
                endpoint_probe_reported_ok: false,
                ..reproven
            }),
            ChannelRecoveryVerdict::CommandPathStillLatched
        );

        // Only the fully-proven path retries.
        assert_eq!(
            channel_retry_action(recovery_retry_observation(reproven)),
            ChannelRetryAction::Retry
        );

        // ── The budget relationship, mirroring the driver constants in
        // `mt7601u/mod.rs`. The driver asserts these at compile time; this test
        // pins the arithmetic itself so the formula cannot drift silently.
        const MCU_RESPONSE_ATTEMPTS: u32 = 5;
        const CHANNEL_DRAIN_BULK_BUDGET: u32 = 2_000;
        const CHANNEL_PROBE_BULK_BUDGET: u32 = 20_000;
        const CHANNEL_RETRY_RESERVE: u64 = 525_000;
        let plan = RecoveryBudgetPlan {
            drain_commands: MAX_MCU_DRAIN_COMMANDS,
            attempts_per_command: MCU_RESPONSE_ATTEMPTS,
            drain_polls_per_attempt: CHANNEL_DRAIN_BULK_BUDGET,
            probe_polls_per_attempt: CHANNEL_PROBE_BULK_BUDGET,
        };
        // 13 × 5 × 2001 drain + 5 × 20001 probe.
        assert_eq!(recovery_worst_case_cost(plan), 130_065 + 100_005);
        let reserve = CHANNEL_RETRY_RESERVE + recovery_worst_case_cost(plan);
        assert!(
            recovery_reserve_covers(plan, reserve, CHANNEL_RETRY_RESERVE),
            "the reserve that authorizes a retry must fund the recovery AND the retry"
        );
        assert!(
            !recovery_reserve_covers(plan, reserve - 1, CHANNEL_RETRY_RESERVE),
            "the reserve check has teeth: one unit short must fail"
        );
        // A recovery with nothing to drain still has to fund its proof.
        assert_eq!(
            recovery_worst_case_cost(RecoveryBudgetPlan {
                drain_commands: 0,
                ..plan
            }),
            100_005
        );
    }

    #[test]
    fn mt7601u_test3_channel7_command_path_terminal_oracle() {
        use super::wedge_logic::{
            ChannelCommandPathState, ChannelFailureAction, ChannelFailureObservation,
            ChannelFailurePhase, ChannelRetryAction, ChannelRetryObservation, ChannelSwitchTrap,
            ChannelTerminal, RF_FREQPLAN_TIMEOUT_TRAPS, RejectedRecoveryTrace,
            RejectedRecoveryVerdict, channel_contract_violation, channel_contract_violations,
            channel_failure_action, channel_fault_disposition, channel_operation_evidence,
            channel_retry_action, classify_rejected_recovery,
            mcu_response_budget_has_upstream_margin, rejected_reload_oracle,
            terminal_channel_failure_oracle,
        };

        let rejected = RejectedRecoveryTrace {
            channels_completed: 6,
            failed_channel: 7,
            failed_phase: ChannelFailurePhase::RfFreqplanRf17To20RandomWrite,
            cal_bw_reached: false,
            host_inband_recovered: true,
            host_response_recovered: true,
            device_inband_clear_halt: true,
            device_response_clear_halt: true,
            reload_uploaded_bytes: 0,
            reload_expected_bytes: 45_380,
        };
        assert_eq!(
            classify_rejected_recovery(rejected),
            RejectedRecoveryVerdict::ReloadFailedAtByteZero
        );
        assert_eq!(
            RF_FREQPLAN_TIMEOUT_TRAPS,
            [
                ChannelSwitchTrap::AccumulatedMcuQueueDepth,
                ChannelSwitchTrap::RfFreqplanReadyPollWindow,
                ChannelSwitchTrap::DestructiveColdReloadRejected,
            ]
        );
        assert!(mcu_response_budget_has_upstream_margin(100_000, 100_000));
        assert!(!mcu_response_budget_has_upstream_margin(20_000, 100_000));

        let disposition = channel_fault_disposition();
        assert_eq!(disposition.command_path, ChannelCommandPathState::Unusable);
        assert_eq!(
            disposition.terminal,
            ChannelTerminal::ChannelCommandPathUnusable
        );
        assert!(!disposition.attempt_destructive_reload);
        assert!(!disposition.continue_survey);
        assert!(!disposition.allow_association);
        assert!(!disposition.allow_dhcp);
        assert!(!disposition.allow_link_up);

        // The production decision seam permits one non-destructive endpoint
        // probe only while the same operation ledger can fund the retry. Reported
        // endpoint success is not enough: a second failure terminates.
        assert_eq!(
            channel_failure_action(ChannelFailureObservation {
                endpoint_probes_used: 0,
                operation_remaining: 60_000,
                probe_and_retry_reserve: 45_000,
            }),
            ChannelFailureAction::ProbeEndpointsAndRetry
        );
        assert_eq!(
            channel_failure_action(ChannelFailureObservation {
                endpoint_probes_used: 1,
                operation_remaining: 60_000,
                probe_and_retry_reserve: 45_000,
            }),
            ChannelFailureAction::ClassifyCommandPathUnusable
        );
        assert_eq!(
            channel_failure_action(ChannelFailureObservation {
                endpoint_probes_used: 0,
                operation_remaining: 44_999,
                probe_and_retry_reserve: 45_000,
            }),
            ChannelFailureAction::ClassifyCommandPathUnusable
        );
        assert_eq!(
            channel_retry_action(ChannelRetryObservation {
                endpoint_probe_reported_ok: true,
                command_path_proven_usable: false,
            }),
            ChannelRetryAction::ClassifyCommandPathUnusable
        );
        assert_eq!(
            channel_retry_action(ChannelRetryObservation {
                endpoint_probe_reported_ok: true,
                command_path_proven_usable: true,
            }),
            ChannelRetryAction::Retry
        );

        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        struct SurveyResult {
            channels_completed: u8,
            endpoint_probes: u8,
            terminal_count: u8,
            destructive_reloads: u8,
        }

        fn run_survey_control(
            first_attempt_failure_channel: Option<u8>,
            command_path_reproven: bool,
        ) -> SurveyResult {
            let mut result = SurveyResult {
                channels_completed: 0,
                endpoint_probes: 0,
                terminal_count: 0,
                destructive_reloads: 0,
            };

            for channel in 1..=14 {
                if first_attempt_failure_channel != Some(channel) {
                    result.channels_completed += 1;
                    continue;
                }

                assert_eq!(
                    channel_failure_action(ChannelFailureObservation {
                        endpoint_probes_used: result.endpoint_probes,
                        operation_remaining: 60_000,
                        probe_and_retry_reserve: 45_000,
                    }),
                    ChannelFailureAction::ProbeEndpointsAndRetry
                );
                result.endpoint_probes += 1;
                if channel_retry_action(ChannelRetryObservation {
                    endpoint_probe_reported_ok: true,
                    command_path_proven_usable: command_path_reproven,
                }) == ChannelRetryAction::Retry
                {
                    result.channels_completed += 1;
                    continue;
                }

                assert_eq!(
                    channel_failure_action(ChannelFailureObservation {
                        endpoint_probes_used: result.endpoint_probes,
                        operation_remaining: 30_000,
                        probe_and_retry_reserve: 45_000,
                    }),
                    ChannelFailureAction::ClassifyCommandPathUnusable
                );
                result.terminal_count += 1;
                break;
            }

            result
        }

        // Candidate acceptance requires the genuine CHANNEL=0 survey to finish.
        // A healthy run finishes all fourteen hops. A single transient
        // command-path failure may continue only after an explicit MCU command
        // proof; endpoint-only recovery terminalizes after channels 1-6 with no
        // destructive firmware reload.
        assert_eq!(
            run_survey_control(None, false),
            SurveyResult {
                channels_completed: 14,
                endpoint_probes: 0,
                terminal_count: 0,
                destructive_reloads: 0,
            }
        );
        assert_eq!(
            run_survey_control(Some(7), true),
            SurveyResult {
                channels_completed: 14,
                endpoint_probes: 1,
                terminal_count: 0,
                destructive_reloads: 0,
            }
        );
        assert_eq!(
            run_survey_control(Some(7), false),
            SurveyResult {
                channels_completed: 6,
                endpoint_probes: 1,
                terminal_count: 1,
                destructive_reloads: 0,
            }
        );

        let operation_evidence = channel_operation_evidence(100_000, 9_876, false, 0);
        assert_eq!(operation_evidence.consumed, 90_124);
        let terminal = terminal_channel_failure_oracle(rejected, operation_evidence);
        assert!(terminal.bounded_completion);
        assert_eq!(terminal.terminal_count, 1);
        assert!(!terminal.attempted_destructive_reload);
        assert_eq!(terminal.commands_after_terminal, 0);
        assert!(!terminal.association_reached);
        assert!(!terminal.dhcp_reached);
        assert!(!terminal.link_up_reached);
        assert!(!terminal.pass_reached);
        assert!(terminal.terminal_sink_available);
        assert!(terminal.budget_summary_available);
        assert!(terminal.fence_summary_available);
        assert_eq!(terminal.operation_evidence, operation_evidence);
        assert_eq!(channel_contract_violations(terminal), 0);

        let removed_behavior = rejected_reload_oracle(rejected, operation_evidence);
        assert!(removed_behavior.attempted_destructive_reload);
        assert_eq!(removed_behavior.terminal_count, 0);
        assert_eq!(removed_behavior.commands_after_terminal, 1);
        assert!(!removed_behavior.terminal_sink_available);
        assert!(!removed_behavior.budget_summary_available);
        assert!(!removed_behavior.fence_summary_available);
        assert_eq!(
            channel_contract_violations(removed_behavior),
            channel_contract_violation::TERMINAL_COUNT
                | channel_contract_violation::DESTRUCTIVE_RELOAD
                | channel_contract_violation::COMMAND_AFTER_TERMINAL
                | channel_contract_violation::TERMINAL_SINK_MISSING
                | channel_contract_violation::BUDGET_SUMMARY_MISSING
                | channel_contract_violation::FENCE_SUMMARY_MISSING
        );

        // Mutation matrix: every contract clause has independent teeth.
        let mutations = [
            (
                super::wedge_logic::ChannelFailureOracle {
                    bounded_completion: false,
                    ..terminal
                },
                channel_contract_violation::UNBOUNDED,
            ),
            (
                super::wedge_logic::ChannelFailureOracle {
                    terminal_count: 2,
                    ..terminal
                },
                channel_contract_violation::TERMINAL_COUNT,
            ),
            (
                super::wedge_logic::ChannelFailureOracle {
                    attempted_destructive_reload: true,
                    ..terminal
                },
                channel_contract_violation::DESTRUCTIVE_RELOAD,
            ),
            (
                super::wedge_logic::ChannelFailureOracle {
                    commands_after_terminal: 1,
                    ..terminal
                },
                channel_contract_violation::COMMAND_AFTER_TERMINAL,
            ),
            (
                super::wedge_logic::ChannelFailureOracle {
                    association_reached: true,
                    ..terminal
                },
                channel_contract_violation::LATER_NETWORK_STAGE,
            ),
            (
                super::wedge_logic::ChannelFailureOracle {
                    dhcp_reached: true,
                    ..terminal
                },
                channel_contract_violation::LATER_NETWORK_STAGE,
            ),
            (
                super::wedge_logic::ChannelFailureOracle {
                    link_up_reached: true,
                    ..terminal
                },
                channel_contract_violation::LATER_NETWORK_STAGE,
            ),
            (
                super::wedge_logic::ChannelFailureOracle {
                    pass_reached: true,
                    ..terminal
                },
                channel_contract_violation::LATER_NETWORK_STAGE,
            ),
            (
                super::wedge_logic::ChannelFailureOracle {
                    terminal_sink_available: false,
                    ..terminal
                },
                channel_contract_violation::TERMINAL_SINK_MISSING,
            ),
            (
                super::wedge_logic::ChannelFailureOracle {
                    budget_summary_available: false,
                    ..terminal
                },
                channel_contract_violation::BUDGET_SUMMARY_MISSING,
            ),
            (
                super::wedge_logic::ChannelFailureOracle {
                    fence_summary_available: false,
                    ..terminal
                },
                channel_contract_violation::FENCE_SUMMARY_MISSING,
            ),
            (
                super::wedge_logic::ChannelFailureOracle {
                    operation_evidence: channel_operation_evidence(100_000, 100_000, false, 0),
                    ..terminal
                },
                channel_contract_violation::INVALID_OPERATION_ACCOUNTING,
            ),
        ];
        for (mutated, expected) in mutations {
            assert_eq!(channel_contract_violations(mutated), expected);
        }

        // A completed reload is a distinct trace and must not be used to
        // "prove" either the terminal path or the negative control.
        assert_eq!(
            classify_rejected_recovery(RejectedRecoveryTrace {
                reload_uploaded_bytes: 45_380,
                ..rejected
            }),
            RejectedRecoveryVerdict::DifferentTrace
        );
        assert_eq!(
            classify_rejected_recovery(RejectedRecoveryTrace {
                failed_phase: ChannelFailurePhase::Other,
                ..rejected
            }),
            RejectedRecoveryVerdict::DifferentTrace
        );
        assert_eq!(
            classify_rejected_recovery(RejectedRecoveryTrace {
                cal_bw_reached: true,
                ..rejected
            }),
            RejectedRecoveryVerdict::DifferentTrace
        );
        assert_eq!(
            terminal_channel_failure_oracle(
                RejectedRecoveryTrace {
                    reload_uploaded_bytes: 45_380,
                    ..rejected
                },
                operation_evidence,
            )
            .terminal_count,
            0
        );

        // Fence accounting is observed, not defaulted: a refusal survives the
        // same production helper used by the terminal breadcrumb.
        assert_eq!(
            channel_operation_evidence(100_000, 9_876, false, 3).fence_refusals,
            3
        );
    }

    /// ISC-1: WPA2 PMK = PBKDF2-HMAC-SHA1(passphrase, SSID, 4096, 256 bits).
    /// The published IEEE 802.11i vector (Annex J.4 / the wpa_supplicant test
    /// suite): passphrase "password", SSID "IEEE".
    #[test]
    fn wpa_isc1_pmk_ieee80211i_kat() {
        let pmk = super::wpa::kdf::pmk_from_passphrase(b"password", b"IEEE");
        assert_eq!(
            pmk.as_slice(),
            hex_bytes("f42c6fc52df0ebef9ebb4b90b38a5f902e83fe1b135a70e23aed762e9710a12e")
                .as_slice()
        );
    }

    /// ISC-2: the IEEE 802.11 PRF over HMAC-SHA1. The three published PRF-512
    /// known-answer vectors (verbatim from the IWD `unit/test-prf-sha1.c` suite,
    /// Intel — the RFC-2202-style inputs with prefix "prefix"). These pin the
    /// PRF construction (label ‖ 0x00 ‖ data ‖ counter, concatenated/truncated)
    /// that PTK/GTK ride on.
    #[test]
    fn wpa_isc2_prf_sha1_kats() {
        use super::wpa::kdf::prf_sha1;
        assert_eq!(
            prf_sha1(&[0x0b; 20], b"prefix", b"Hi There", 512),
            hex_bytes(
                "bcd4c650b30b9684951829e0d75f9d54b862175ed9f00606e17d8da35402ffee\
                 75df78c3d31e0f889f012120c0862beb67753e7439ae242edb8373698356cf5a"
            )
        );
        assert_eq!(
            prf_sha1(b"Jefe", b"prefix", b"what do ya want for nothing?", 512),
            hex_bytes(
                "51f4de5b33f249adf81aeb713a3c20f4fe631446fabdfa58244759ae58ef9009\
                 a99abf4eac2ca5fa87e692c440eb40023e7babb206d61de7b92f41529092b8fc"
            )
        );
        assert_eq!(
            prf_sha1(&[0xaa; 20], b"prefix", &[0xdd; 50], 512),
            hex_bytes(
                "e1ac546ec4cb636f9976487be5c86be17a0252ca5d8d8df12cfb0473525249ce\
                 9dd8d177ead710bc9b590547239107aef7b4abd43d87f0a68f1cbd9e2b6f7607"
            )
        );
    }

    /// ISC-1: a second published PMK vector (IWD `eapol_4way_test`):
    /// passphrase "EasilyGuessedPassword", SSID "TestWPA".
    #[test]
    fn wpa_isc1_pmk_iwd_kat() {
        let pmk = super::wpa::kdf::pmk_from_passphrase(b"EasilyGuessedPassword", b"TestWPA");
        assert_eq!(
            pmk.as_slice(),
            hex_bytes("bf9aa3155300125e7a5ebb2a549f8cd4edab8ee12e94bfc24b3357ad049665d9")
                .as_slice()
        );
    }

    /// ISC-3: EAPOL-Key MIC, v2 (HMAC-SHA1-128) over the frame with the MIC
    /// field zeroed. Published vector from IWD `unit/test-eapol.c`
    /// (`eapol_key_mic_data_1` + `kck_data_1` → `eapol_key_mic_2`). The frame's
    /// MIC field (offset 81) is pre-zeroed in the source. Exercises the
    /// version-selection (key-info low bits = 2 → HMAC-SHA1) and the
    /// zero-before-compute install/verify paths.
    #[test]
    fn wpa_isc3_eapol_mic_v2_kat() {
        use super::wpa::eapol::{self, KeyDescriptorVersion};
        let kck = hex_bytes("9a75ef0bde7c209ccae13f54b1b33ea3");
        let mic2 = hex_bytes("6f0489cf7406acf0ae8fcb32bce57c37");
        // eapol_key_mic_data_1 (121 octets), MIC field pre-zeroed at offset 81.
        let frame = hex_bytes(concat!(
            "0103007502010a00",
            "1000000000000000",
            "0159168bc3a5df18",
            "d71efb6423f34008",
            "8dab9e1ba2bbc586",
            "59e07b3764b0de85",
            "7000000000000000",
            "0000000000000000",
            "0000000000000000",
            "0000000000000000",
            "0000000000000000",
            "0000000000000000",
            "0000163014010000",
            "0fac040100000fac",
            "040100000fac0201",
            "00",
        ));

        let kf = eapol::KeyFrame::parse(&frame).expect("parse EAPOL-Key");
        assert_eq!(kf.version(), KeyDescriptorVersion::HmacSha1Aes);

        // Direct compute over the (pre-zeroed-MIC) frame matches the published MIC.
        let mic = eapol::compute_mic(KeyDescriptorVersion::HmacSha1Aes, &kck, &frame).unwrap();
        assert_eq!(mic.as_slice(), mic2.as_slice());

        // install_mic (zero-before-compute) writes the same MIC, and verify_mic
        // accepts it; a tampered MIC is rejected.
        let mut f = frame.clone();
        assert!(eapol::install_mic(
            KeyDescriptorVersion::HmacSha1Aes,
            &kck,
            &mut f
        ));
        assert_eq!(
            &f[eapol::KEY_MIC_OFFSET..eapol::KEY_MIC_OFFSET + 16],
            mic2.as_slice()
        );
        assert!(eapol::verify_mic(
            KeyDescriptorVersion::HmacSha1Aes,
            &kck,
            &f
        ));
        f[eapol::KEY_MIC_OFFSET] ^= 0xff;
        assert!(!eapol::verify_mic(
            KeyDescriptorVersion::HmacSha1Aes,
            &kck,
            &f
        ));

        // Version selection: low 3 bits of key-info → algorithm.
        assert_eq!(
            eapol::descriptor_version(0x000a),
            KeyDescriptorVersion::HmacSha1Aes
        );
        assert_eq!(
            eapol::descriptor_version(0x008b),
            KeyDescriptorVersion::AesCmac
        );
    }

    /// ISC-2: the PTK Min/Max ordering trap, verified TRANSITIVELY (IWD
    /// `eapol_4way_test`). Derive the PTK from (PMK, AA, SPA, ANonce, SNonce),
    /// take its KCK, and verify the published MIC on the real frame-2
    /// (`eapol_key_data_4`) — a wrong Min/Max ordering yields a wrong KCK so the
    /// MIC fails. A pure unit KAT of the PTK bytes isn't published; this is the
    /// standard's own end-to-end answer (the supplicant's MIC).
    #[test]
    fn wpa_isc2_ptk_minmax_transitive_kat() {
        use super::wpa::eapol::{self, KeyDescriptorVersion};
        use super::wpa::kdf;

        let pmk = kdf::pmk_from_passphrase(b"EasilyGuessedPassword", b"TestWPA");
        let aa = [0x24, 0xa2, 0xe1, 0xec, 0x17, 0x04];
        let spa = [0xa0, 0xa8, 0xcd, 0x1c, 0x7e, 0xc9];
        let anonce: [u8; 32] =
            hex_bytes("c2bb57ab588f92ebbd44e811094f601c087986030c3ac749cc61d63e33832e50")
                .try_into()
                .unwrap();
        let snonce: [u8; 32] =
            hex_bytes("3289e91565094f329a9cd54a4a090d2cf4344683bf50efee3608b64856800e84")
                .try_into()
                .unwrap();

        let ptk = kdf::ptk_ccmp(&pmk, &aa, &spa, &anonce, &snonce);

        // frame-2 (eapol_key_data_4) carries the supplicant's MIC computed with
        // this PTK's KCK (MIC field at offset 81 = 01c31b82...f768).
        let frame2 = hex_bytes(concat!(
            "0103007502010a0000000000",
            "00000000003289e91565094f",
            "329a9cd54a4a090d2cf43446",
            "83bf50efee3608b64856800e",
            "840000000000000000000000",
            "000000000000000000000000",
            "00000000000000000001c31b",
            "82ff62a379b08dd1fc82c2f7",
            "68001630140100000fac0401",
            "00000fac040100000fac0200",
            "00",
        ));
        assert!(eapol::verify_mic(
            KeyDescriptorVersion::HmacSha1Aes,
            &ptk.kck,
            &frame2
        ));

        // Min/Max is argument-order-independent (swapping both addrs + both
        // nonces yields the identical PTK) — a property the trap would break.
        assert_eq!(ptk, kdf::ptk_ccmp(&pmk, &spa, &aa, &snonce, &anonce));
    }

    /// OT-2 (CLOSED — W2 becomes a security boundary): the 4-way handshake
    /// derives a *correct, SNonce-sensitive* PTK from an arbitrary (CSPRNG-style,
    /// non-counter) SNonce. This is the property that makes swapping `gen_snonce`
    /// to the kernel ChaCha20 DRBG a real security upgrade and not cosmetic — a
    /// random SNonce must yield a fresh, unpredictable, *valid* key. (That the
    /// SNonce *source* is now a CSPRNG is proven separately by the kernel-rng
    /// self-test KATs; that the *handshake* is SNonce-value-agnostic and still
    /// correct is proven here. The `..minmax..` KAT above pins that the published
    /// vector SNonce still derives the published key.) Reuses the IWD
    /// `eapol_4way_test` vector.
    #[test]
    fn wpa_ot2_ptk_snonce_agnostic_kat() {
        use super::wpa::eapol::{self, KeyDescriptorVersion};
        use super::wpa::kdf;

        let pmk = kdf::pmk_from_passphrase(b"EasilyGuessedPassword", b"TestWPA");
        let aa = [0x24, 0xa2, 0xe1, 0xec, 0x17, 0x04];
        let spa = [0xa0, 0xa8, 0xcd, 0x1c, 0x7e, 0xc9];
        let anonce: [u8; 32] =
            hex_bytes("c2bb57ab588f92ebbd44e811094f601c087986030c3ac749cc61d63e33832e50")
                .try_into()
                .unwrap();
        let snonce_vec: [u8; 32] =
            hex_bytes("3289e91565094f329a9cd54a4a090d2cf4344683bf50efee3608b64856800e84")
                .try_into()
                .unwrap();
        // An arbitrary, non-counter SNonce — what a CSPRNG draw looks like to the
        // KDF (no relation to the chip MAC or a per-call counter).
        let snonce_rand: [u8; 32] =
            hex_bytes("a1b2c3d4e5f60718293a4b5c6d7e8f90112233445566778899aabbccddeeff00")
                .try_into()
                .unwrap();

        let ptk_vec = kdf::ptk_ccmp(&pmk, &aa, &spa, &anonce, &snonce_vec);
        let ptk_rand = kdf::ptk_ccmp(&pmk, &aa, &spa, &anonce, &snonce_rand);

        // (1) The SNonce feeds the key: a different nonce ⇒ a different PTK. So a
        // CSPRNG SNonce yields a fresh key (not the predictable counter's key).
        assert_ne!(ptk_vec, ptk_rand);

        // (2) Build M2 with the random SNonce and round-trip the MIC under the
        // matching PTK; the vector PTK must REJECT it — the MIC is key-specific,
        // so derivation from a random SNonce is correct, not a no-op.
        let key_info = 2 | eapol::KEY_INFO_KEY_TYPE | eapol::KEY_INFO_MIC;
        let replay = [0u8, 0, 0, 0, 0, 0, 0, 1];
        let mut m2 = [0u8; 128];
        let m2n = eapol::build_key_frame(&mut m2, 1, key_info, 16, &replay, &snonce_rand, &[])
            .expect("M2 build");
        assert!(eapol::install_mic(
            KeyDescriptorVersion::HmacSha1Aes,
            &ptk_rand.kck,
            &mut m2[..m2n]
        ));
        assert!(eapol::verify_mic(
            KeyDescriptorVersion::HmacSha1Aes,
            &ptk_rand.kck,
            &m2[..m2n]
        ));
        assert!(!eapol::verify_mic(
            KeyDescriptorVersion::HmacSha1Aes,
            &ptk_vec.kck,
            &m2[..m2n]
        ));
    }

    /// ISC-3 (v3): the AES-128-CMAC MIC algorithm, pinned against the RFC 4493 /
    /// NIST SP 800-38B vectors (verbatim from IWD `unit/test-cmac-aes.c`). v3
    /// EAPOL-Key MICs use AES-CMAC, and `compute_mic(AesCmac, ..)` is exactly
    /// CMAC(kck, frame)[..16] — so these pin the algorithm through the MIC path
    /// (the v2 frame above pins the EAPOL framing end-to-end).
    #[test]
    fn wpa_isc3_aes_cmac_rfc4493_kats() {
        use super::wpa::eapol::{KeyDescriptorVersion::AesCmac, compute_mic};
        let key = hex_bytes("2b7e151628aed2a6abf7158809cf4f3c");
        let msg = hex_bytes(concat!(
            "6bc1bee22e409f96e93d7e117393172a",
            "ae2d8a571e03ac9c9eb76fac45af8e51",
            "30c81c46a35ce411e5fbc1191a0a52ef",
            "f69f2445df4f9b17ad2b417be66c3710",
        ));
        for (len, tag) in [
            (0usize, "bb1d6929e95937287fa37d129b756746"),
            (16, "070a16b46b4d4144f79bdd9dd04a287c"),
            (40, "dfa66747de9ae63030ca32611497c827"),
            (64, "51f0bebf7e3b9d92fc49741779363cfe"),
        ] {
            let mic = compute_mic(AesCmac, &key, &msg[..len]).unwrap();
            assert_eq!(&mic[..], hex_bytes(tag).as_slice(), "CMAC len={len}");
        }
    }

    /// ISC-4 (CCM core): the RFC 3610 §8 packet vectors — AES-128, M=8, L=2
    /// (13-octet nonce): *exactly* CCMP's CCM configuration. These pin the
    /// `aes`+`ccm` core (encrypt, authenticated decrypt, and MIC rejection on
    /// tamper) that `ccmp_encrypt`/`ccmp_decrypt` delegate to.
    #[test]
    fn wpa_isc4_aes_ccm_rfc3610_kats() {
        use super::wpa::ccmp::{aes_ccm_ad, aes_ccm_ae};
        // (key, nonce, aad, plaintext, ciphertext‖MIC) — verbatim RFC 3610 §8.
        struct V {
            key: &'static str,
            nonce: &'static str,
            aad: &'static str,
            pt: &'static str,
            ct: &'static str,
        }
        let vs = [
            // Packet Vector #1
            V {
                key: "C0C1C2C3C4C5C6C7C8C9CACBCCCDCECF",
                nonce: "00000003020100A0A1A2A3A4A5",
                aad: "0001020304050607",
                pt: "08090A0B0C0D0E0F101112131415161718191A1B1C1D1E",
                ct: "588C979A61C663D2F066D0C2C0F989806D5F6B61DAC38417E8D12CFDF926E0",
            },
            // Packet Vector #2
            V {
                key: "C0C1C2C3C4C5C6C7C8C9CACBCCCDCECF",
                nonce: "00000004030201A0A1A2A3A4A5",
                aad: "0001020304050607",
                pt: "08090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F",
                ct: "72C91A36E135F8CF291CA894085C87E3CC15C439C9E43A3BA091D56E10400916",
            },
            // Packet Vector #3
            V {
                key: "C0C1C2C3C4C5C6C7C8C9CACBCCCDCECF",
                nonce: "00000005040302A0A1A2A3A4A5",
                aad: "0001020304050607",
                pt: "08090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F20",
                ct: "51B1E5F44A197D1DA46B0F8E2D282AE871E838BB64DA8596574ADAA76FBD9FB0C5",
            },
        ];
        for (i, v) in vs.iter().enumerate() {
            let key: [u8; 16] = hex_bytes(v.key).try_into().unwrap();
            let nonce: [u8; 13] = hex_bytes(v.nonce).try_into().unwrap();
            let aad = hex_bytes(v.aad);
            let pt = hex_bytes(v.pt);
            let expect = hex_bytes(v.ct);
            let got = aes_ccm_ae(&key, &nonce, &aad, &pt).expect("ccm ae");
            assert_eq!(got, expect, "RFC3610 PV#{} encrypt", i + 1);
            let back = aes_ccm_ad(&key, &nonce, &aad, &expect).expect("ccm ad");
            assert_eq!(back, pt, "RFC3610 PV#{} decrypt", i + 1);
            // Flip the last MIC octet → authentication must fail.
            let mut bad = expect.clone();
            let last = bad.len() - 1;
            bad[last] ^= 0x01;
            assert!(
                aes_ccm_ad(&key, &nonce, &aad, &bad).is_none(),
                "RFC3610 PV#{} tamper",
                i + 1
            );
        }
    }

    /// ISC-4 (802.11 construction): an independent hand-laid byte oracle for the
    /// CCM nonce/AAD layout (IEEE 802.11 §12.5.3.3.{3,4}), per the repo's
    /// "format/ABI specs require an independent byte oracle" rule. The expected
    /// bytes are computed by hand from the spec + the hostap `ccmp_aad_nonce`
    /// reference — NOT from this code — so they catch a writer/reader that agree
    /// with each other but disagree with the standard.
    #[test]
    fn wpa_isc4_ccmp_construction_byte_oracle() {
        use super::wpa::ccmp::{build_aad, build_nonce, ccmp_header, nonce_priority};

        // Non-QoS Data, FromDS. FC=0x0208; A1=000102030405 A2=060708090A0B
        // A3=0C0D0E0F1011; SeqCtrl=0x1234 (Seq#=0x123, Frag=4).
        let hdr = hex_bytes(concat!(
            "0802",
            "0000",
            "000102030405",
            "060708090a0b",
            "0c0d0e0f1011",
            "3412",
        ));
        // FC masked → mutable bits cleared, Protected set → 0x4208 (LE 08 42);
        // SeqCtrl masked to the Fragment number only → 0x0004 (LE 04 00).
        assert_eq!(
            build_aad(&hdr).unwrap(),
            hex_bytes(concat!(
                "0842",
                "000102030405",
                "060708090a0b",
                "0c0d0e0f1011",
                "0400",
            )),
            "non-QoS data AAD"
        );
        assert_eq!(nonce_priority(&hdr), 0x00);

        // Nonce: priority(0) ‖ A2 ‖ PN big-endian. PN = 0x0000FEDCBA98.
        let pn = [0x98u8, 0xba, 0xdc, 0xfe, 0x00, 0x00]; // [PN0..PN5]
        let a2 = [0x06u8, 0x07, 0x08, 0x09, 0x0a, 0x0b];
        assert_eq!(
            build_nonce(0x00, &a2, &pn).as_slice(),
            hex_bytes("00060708090a0b0000fedcba98").as_slice()
        );
        // CCMP header: PN0 PN1 Rsvd (0x20|KeyID<<6) PN2 PN3 PN4 PN5.
        assert_eq!(
            ccmp_header(&pn, 0).as_slice(),
            hex_bytes("98ba0020dcfe0000").as_slice()
        );
        assert_eq!(
            ccmp_header(&pn, 2).as_slice(),
            hex_bytes("98ba00a0dcfe0000").as_slice(), // KeyID 2 → 0x20|0x80 = 0xA0
        );

        // QoS Data, FromDS, TID 6. FC=0x0288; QoS Control = 0x0006.
        let qhdr = hex_bytes(concat!(
            "8802",
            "0000",
            "000102030405",
            "060708090a0b",
            "0c0d0e0f1011",
            "3412",
            "0600",
        ));
        assert_eq!(
            build_aad(&qhdr).unwrap(),
            hex_bytes(concat!(
                "8842",
                "000102030405",
                "060708090a0b",
                "0c0d0e0f1011",
                "0400",
                "0600",
            )),
            "QoS data AAD"
        );
        assert_eq!(nonce_priority(&qhdr), 0x06);
    }

    /// ISC-4 (encap/decap wiring): every output byte of `ccmp_encrypt` traced to
    /// a proven sub-oracle — the header (Protected set) ‖ the CCMP header (PN
    /// oracle) ‖ the RFC-3610-proven core run over the hand-laid-oracle
    /// nonce/AAD. Then `ccmp_decrypt` inverts it and rejects a flipped MIC. This
    /// is composition over proven parts, not a self-referential round-trip.
    #[test]
    fn wpa_isc4_ccmp_encap_decap() {
        use super::wpa::ccmp::{
            aes_ccm_ae, build_aad, build_nonce, ccmp_decrypt, ccmp_encrypt, ccmp_header,
            nonce_priority,
        };
        let tk: [u8; 16] = hex_bytes("c97c1f67ce371185514a8a19f2bdd52f")
            .try_into()
            .unwrap();
        let hdr = hex_bytes(concat!(
            "0802",
            "0000",
            "000102030405",
            "060708090a0b",
            "0c0d0e0f1011",
            "0000",
        ));
        let pn = [0x01u8, 0x00, 0x00, 0x00, 0x00, 0x00];
        let pt = b"NeoPlutarch CCMP";
        let hdrlen = hdr.len();

        let mpdu = ccmp_encrypt(&tk, &hdr, &pn, 0, pt).expect("encap");
        // Protected bit forced on; CCMP header carries the PN.
        assert_eq!(u16::from_le_bytes([mpdu[0], mpdu[1]]) & 0x4000, 0x4000);
        assert_eq!(&mpdu[hdrlen..hdrlen + 8], ccmp_header(&pn, 0).as_slice());
        // ciphertext‖MIC == the proven core over the proven construction.
        let a2 = [0x06u8, 0x07, 0x08, 0x09, 0x0a, 0x0b];
        let aad = build_aad(&hdr).unwrap();
        let nonce = build_nonce(nonce_priority(&hdr), &a2, &pn);
        assert_eq!(
            &mpdu[hdrlen + 8..],
            aes_ccm_ae(&tk, &nonce, &aad, pt).unwrap().as_slice()
        );
        // decap authenticates and recovers the plaintext.
        assert_eq!(ccmp_decrypt(&tk, &mpdu, hdrlen).unwrap().as_slice(), pt);
        // A flipped MIC octet → rejected.
        let mut bad = mpdu.clone();
        let last = bad.len() - 1;
        bad[last] ^= 0x80;
        assert!(ccmp_decrypt(&tk, &bad, hdrlen).is_none());
    }

    /// ISC-2 (PTK, direct): a published CCMP PTK→TK known-answer (IWD
    /// `unit/test-crypto.c` `ptk_test_1`). Complements the transitive Min/Max
    /// KAT above with a direct assertion on the derived 128-bit TK — the third
    /// 128-bit slice of the 384-bit CCMP PTK.
    #[test]
    fn wpa_isc2_ptk_ccmp_tk_kat() {
        use super::wpa::kdf;
        let pmk = hex_bytes("0dc0d6eb90555ed6419756b9a15ec3e3209b63df707dd508d14581f8982721af");
        let aa = [0xa0, 0xa1, 0xa1, 0xa3, 0xa4, 0xa5];
        let spa = [0xb0, 0xb1, 0xb2, 0xb3, 0xb4, 0xb5];
        let snonce: [u8; 32] =
            hex_bytes("c0c1c2c3c4c5c6c7c8c9d0d1d2d3d4d5d6d7d8d9dadbdcdddedfe0e1e2e3e4e5")
                .try_into()
                .unwrap();
        let anonce: [u8; 32] =
            hex_bytes("e0e1e2e3e4e5e6e7e8e9f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff000102030405")
                .try_into()
                .unwrap();
        let ptk = kdf::ptk_ccmp(&pmk, &aa, &spa, &anonce, &snonce);
        assert_eq!(
            ptk.tk.as_slice(),
            hex_bytes("b2360c79e9710fdd58bea93deaf06599").as_slice()
        );
    }

    /// ISC-2: AES Key Unwrap (RFC 3394) — the 4-way M3 GTK unwrap. The published
    /// §4.1 vector (128-bit KEK, 128-bit key data): unwrapping the wrapped
    /// ciphertext recovers the key data AND passes the A6A6.. integrity check;
    /// a one-bit corruption fails it. Vector hex from the raw RFC 3394 text.
    #[test]
    fn wpa_isc2_aes_keyunwrap_rfc3394_kat() {
        use super::wpa::keywrap::aes_unwrap;
        let kek_v = hex_bytes("000102030405060708090a0b0c0d0e0f");
        let mut kek = [0u8; 16];
        kek.copy_from_slice(&kek_v);
        let ct = hex_bytes("1fa68b0a8112b447aef34bd8fb5a7b829d3e862371d2cfe5");
        let pt = hex_bytes("00112233445566778899aabbccddeeff");
        let mut out = [0u8; 32];
        let n = aes_unwrap(&ct, &kek, &mut out).expect("unwrap");
        assert_eq!(&out[..n], pt.as_slice());
        // A corrupted wrapped block fails the integrity check (no plaintext).
        let mut bad = ct.clone();
        bad[0] ^= 0x01;
        assert!(aes_unwrap(&bad, &kek, &mut out).is_none());
        // Malformed length (not a multiple of 8, or < 16) → None.
        assert!(aes_unwrap(&ct[..15], &kek, &mut out).is_none());
    }

    /// CB-5a: AES Key Wrap (RFC 3394 §2.2.1) — the inverse `aes_wrap`, added so the
    /// hermetic WPA3-SAE gate's in-process AP can PRODUCE the wrapped GTK/IGTK Key
    /// Data the STA unwraps. FORWARD oracle (not round-trip): wrapping the published
    /// §4.1 plaintext under the §4.1 KEK MUST equal the published §4.1 ciphertext,
    /// so a wrong wrap can't hide behind a matching unwrap. Then wrap→unwrap agrees,
    /// and a 7-block (56-byte, the gate's padded KDE size) wrap round-trips.
    #[test]
    fn wpa_cb5a_aes_keywrap_rfc3394_kat() {
        use super::wpa::keywrap::{aes_unwrap, aes_wrap};
        let kek_v = hex_bytes("000102030405060708090a0b0c0d0e0f");
        let mut kek = [0u8; 16];
        kek.copy_from_slice(&kek_v);
        let pt = hex_bytes("00112233445566778899aabbccddeeff");
        let ct = hex_bytes("1fa68b0a8112b447aef34bd8fb5a7b829d3e862371d2cfe5");
        // Forward: wrap(KEK, pt) == the published ciphertext (the oracle).
        let mut wrapped = [0u8; 24];
        let wn = aes_wrap(&pt, &kek, &mut wrapped).expect("wrap");
        assert_eq!(&wrapped[..wn], ct.as_slice());
        // wrap→unwrap agrees (closes the pair the gate depends on).
        let mut back = [0u8; 16];
        let un = aes_unwrap(&wrapped[..wn], &kek, &mut back).expect("unwrap");
        assert_eq!(&back[..un], pt.as_slice());
        // A 56-byte (7-block) key-data wrap — the gate's IGTK+GTK+pad size — is a
        // 64-byte wrapped blob that unwraps back to the same 56 bytes.
        let big: Vec<u8> = (0..56u8).collect();
        let mut wbig = [0u8; 64];
        let bn = aes_wrap(&big, &kek, &mut wbig).expect("wrap56");
        assert_eq!(bn, 64);
        let mut ubig = [0u8; 56];
        let bu = aes_unwrap(&wbig[..bn], &kek, &mut ubig).expect("unwrap56");
        assert_eq!(&ubig[..bu], big.as_slice());
        // Invalid length (not a multiple of 8) → None.
        assert!(aes_wrap(&pt[..15], &kek, &mut wrapped).is_none());
    }

    /// ISC-3: CCMP header PN extraction (inverse of ccmp_header) + the replay
    /// window (strictly-increasing accept; replays/old PNs dropped).
    #[test]
    fn wpa_isc3_ccmp_pn_replay() {
        use super::wpa::ccmp::{ReplayWindow, ccmp_header, pn_from_header};
        // ccmp_header round-trips through pn_from_header.
        let hdr = ccmp_header(&[1, 2, 3, 4, 5, 6], 2);
        assert_eq!(hdr, [0x01, 0x02, 0x00, 0xa0, 0x03, 0x04, 0x05, 0x06]);
        assert_eq!(pn_from_header(&hdr), Some(0x0605_0403_0201));
        assert_eq!(pn_from_header(&hdr[..7]), None); // short header
        // Replay window: strictly-increasing accept; replay + out-of-order drop.
        let mut rw = ReplayWindow::new();
        assert!(rw.accept(1));
        assert!(rw.accept(2));
        assert!(!rw.accept(2)); // replay
        assert!(!rw.accept(1)); // old / out-of-order
        assert!(rw.accept(3));
        assert_eq!(rw.last(), 3);
    }

    /// ISC-2: the EAPOL-Key frame builder (the supplicant's M2). Field-exact +
    /// round-trips through W0's parser, and install/verify MIC closes the loop.
    #[test]
    fn wpa_isc2_eapol_key_build_m2() {
        use super::wpa::eapol::*;
        let snonce = [0xaau8; 32];
        let replay = [0u8, 0, 0, 0, 0, 0, 0, 1];
        // M2: descriptor version 2 | pairwise key-type | MIC present = 0x010a.
        let key_info = 2 | KEY_INFO_KEY_TYPE | KEY_INFO_MIC;
        let mut buf = [0u8; 128];
        let n = build_key_frame(&mut buf, 2, key_info, 16, &replay, &snonce, &[]).unwrap();
        assert_eq!(n, 99); // fixed descriptor, no key data
        assert_eq!(buf[0], 2); // EAPOL version
        assert_eq!(buf[1], 3); // EAPOL-Key type
        assert_eq!(&buf[2..4], &[0x00, 0x5f]); // body length = 95 (BE)
        assert_eq!(buf[4], 2); // RSN descriptor type
        assert_eq!(&buf[5..7], &[0x01, 0x0a]); // Key Info (BE)
        assert_eq!(&buf[7..9], &[0x00, 0x10]); // Key Length = 16
        assert_eq!(&buf[9..17], &replay);
        assert_eq!(&buf[17..49], &snonce);
        assert_eq!(&buf[81..97], &[0u8; 16]); // MIC zeroed pre-install
        assert_eq!(&buf[97..99], &[0u8, 0]); // Key Data Length = 0
        // Parses back (W0's KeyFrame) and the version selects HMAC-SHA1.
        let kf = KeyFrame::parse(&buf[..n]).unwrap();
        assert_eq!(kf.key_info(), key_info);
        assert_eq!(kf.nonce(), &snonce[..]);
        assert_eq!(kf.version(), KeyDescriptorVersion::HmacSha1Aes);
        // install_mic then verify_mic round-trip (KCK is a test key).
        let kck = [0x11u8; 16];
        assert!(install_mic(
            KeyDescriptorVersion::HmacSha1Aes,
            &kck,
            &mut buf[..n]
        ));
        assert!(verify_mic(
            KeyDescriptorVersion::HmacSha1Aes,
            &kck,
            &buf[..n]
        ));
    }

    /// W2 silicon fix: a received M3 arrives inside an 802.11 DATA frame that
    /// carries trailing bytes (the FCS / 802.11 padding) AFTER the EAPOL-Key PDU,
    /// and the extraction hands those back too. `verify_mic` MUST bound the MIC to
    /// the EAPOL header's declared body length (4 + body_len), NOT the buffer
    /// length — the AP MIC'd only the PDU. Pre-fix, the MIC covered the slop and
    /// failed; the silicon discriminator was "M3 rx buf_len=156 eapol_pdu_len=155
    /// (trailing=1)". This pins the bound so the regression can't return.
    #[test]
    fn wpa_eapol_verify_mic_ignores_trailing_bytes() {
        use super::wpa::eapol::*;
        let kck = [0x11u8; 16];
        let replay = [0u8, 0, 0, 0, 0, 0, 0, 1];
        let nonce = [0x22u8; 32];
        let key_info = 2 | KEY_INFO_KEY_TYPE | KEY_INFO_MIC;
        let mut buf = [0u8; 160];
        let n = build_key_frame(&mut buf, 2, key_info, 16, &replay, &nonce, &[]).unwrap();
        assert!(install_mic(
            KeyDescriptorVersion::HmacSha1Aes,
            &kck,
            &mut buf[..n]
        ));
        // Exact-length PDU verifies (the baseline).
        assert!(verify_mic(
            KeyDescriptorVersion::HmacSha1Aes,
            &kck,
            &buf[..n]
        ));
        // The SAME PDU with trailing garbage appended (the FCS/padding the wire
        // carries) MUST still verify — the fix bounds the MIC to 4 + body_len.
        buf[n] = 0xde;
        buf[n + 1] = 0xad;
        buf[n + 2] = 0xbe;
        buf[n + 3] = 0xef;
        assert!(verify_mic(
            KeyDescriptorVersion::HmacSha1Aes,
            &kck,
            &buf[..n + 4]
        ));
        // The bound still COVERS the PDU: tampering a byte INSIDE it (a nonce byte)
        // must fail even with the trailing bytes present — so the fix isn't just
        // "ignore everything past 81".
        let mut tampered = buf;
        tampered[20] ^= 0xff; // inside the Key Nonce (offset 17..49)
        assert!(!verify_mic(
            KeyDescriptorVersion::HmacSha1Aes,
            &kck,
            &tampered[..n + 4]
        ));
    }

    /// ISC-5: the RSN PMKID = Truncate-128(HMAC-SHA1(PMK, "PMK Name"‖AA‖SPA)).
    /// Pinned against the canonical hashcat WPA-PMKID example (mode 16800,
    /// hashcat.net/wiki example_hashes): the published end-to-end answer is the
    /// PMKID `2582a828…`, so deriving PMK from the example's passphrase+ESSID
    /// (ISC-1) then computing the PMKID reproduces it iff every step — PBKDF2,
    /// the "PMK Name" label, the AA-before-SPA order, the 128-bit truncation —
    /// is correct (HMAC's preimage resistance rules out a coincidental match).
    #[test]
    fn wpa_isc5_pmkid_hashcat_kat() {
        use super::wpa::kdf;
        // Example: PMKID*MAC_AP*MAC_STA*ESSID(hex), password "hashcat!".
        let essid = hex_bytes("ed487162465a774bfba60eb603a39f3a");
        let aa = [0x46, 0x04, 0xba, 0x73, 0x4d, 0x4e];
        let spa = [0x89, 0xac, 0xf0, 0xe7, 0x61, 0xf4];
        let pmk = kdf::pmk_from_passphrase(b"hashcat!", &essid);
        assert_eq!(
            kdf::pmkid(&pmk, &aa, &spa).as_slice(),
            hex_bytes("2582a8281bf9d4308d6f5731d0e61c61").as_slice()
        );
    }

    // ── Phase 7.5 BUILD-W0b: WPA3 SAE (dragonfly / P-256, H2E) KATs. Published
    // cross-source vectors (hostap common_module_tests.c IEEE 802.11-2020 §J.10
    // + iwd unit/test-sae.c), verbatim from the PM-verified
    // Plans/RESEARCH-2026-06-13-sae-vector-sourcing-FINDINGS.md. I-12: every
    // assert is a published known-answer (the PWE is cross-validated by two
    // independent impls + on-curve); the confirm (ISC-6) is the one reconstructed
    // value — an INDEPENDENT HMAC-SHA256 over published inputs, NOT a round-trip.

    // Shared inputs (both sources): SSID/password/pwid, the PWE-pair MACs, and
    // the handshake MACs.
    const SAE_SSID: &[u8] = b"byteme";
    const SAE_PW: &[u8] = b"mekmitasdigoat";
    const SAE_PWID: &[u8] = b"psk4internet";

    /// ISC-2: the H2E PT pipeline (iwd `test-sae.c`) — pwd-seed → u1/u2 →
    /// SSWU P1/P2 → PT, each intermediate KAT'd.
    #[test]
    fn sae_isc2_pt_pipeline_kat() {
        use super::wpa::sae;
        let seed = sae::pwd_seed(SAE_SSID, SAE_PW, SAE_PWID);
        assert_eq!(
            seed.as_slice(),
            hex_bytes("3bd53fe9223dc0280fbfce17d7a3564064e20f48c6ec72246ce367b5569a22af")
                .as_slice(),
            "pwd-seed"
        );

        let pv1 = sae::pwd_value(&seed, b"SAE Hash to Element u1 P1");
        assert_eq!(
            pv1.as_slice(),
            hex_bytes(concat!(
                "a5044469ab16f25b6abf1e0e37a36b56f50be73369053df8",
                "db87989a6b66fd1a491f1cdacbd07931620f83008ffc0ecc"
            ))
            .as_slice(),
            "okm1"
        );
        assert_eq!(
            sae::field_from_okm(&pv1).as_slice(),
            hex_bytes("dc941bc3c6a2b4948b6c61d55590ecb1f0c51c4b1bebaff677e593698d5a53c6")
                .as_slice(),
            "u1"
        );
        let (p1x, p1y) = sae::point_xy(&sae::sswu_point(&pv1)).unwrap();
        assert_eq!(
            p1x.as_slice(),
            hex_bytes("a07c260764a13445ff8cd97c5acc644e7119bde51bad42583eed6f4109639e6b")
                .as_slice(),
            "p1x"
        );
        assert_eq!(
            p1y.as_slice(),
            hex_bytes("3bdc8df0d32337936c74df604933a454142251c53c576c0351b28deaf9428d7e")
                .as_slice(),
            "p1y"
        );

        let pv2 = sae::pwd_value(&seed, b"SAE Hash to Element u2 P2");
        assert_eq!(
            pv2.as_slice(),
            hex_bytes(concat!(
                "9b4e0d5b1879f253c5319615099b05aec5b06fa5e788bcfd",
                "1e9ea60d33436927190814c322a62585c93c577bbaa3d307"
            ))
            .as_slice(),
            "okm2"
        );
        assert_eq!(
            sae::field_from_okm(&pv2).as_slice(),
            hex_bytes("1b8375a518bc21396ad6a65e5597e0bf80d793b6d66e2534a6e7dfe3ee22616f")
                .as_slice(),
            "u2"
        );
        let (p2x, p2y) = sae::point_xy(&sae::sswu_point(&pv2)).unwrap();
        assert_eq!(
            p2x.as_slice(),
            hex_bytes("72cd2a967a837fea5051f0133db46227775ba09f7b6dfb99ae7a8ef22c7d34a0")
                .as_slice(),
            "p2x"
        );
        assert_eq!(
            p2y.as_slice(),
            hex_bytes("864390d797d352b368d311af515bde116fe54459fec867ee18a8a1619ca3ff59")
                .as_slice(),
            "p2y"
        );

        let (ptx, pty) = sae::point_xy(&sae::derive_pt(SAE_SSID, SAE_PW, SAE_PWID)).unwrap();
        assert_eq!(
            ptx.as_slice(),
            hex_bytes("b6e38c98750c684b5d17c3d8c9a4100b39931279187ca6cced5f37ef46ddfa97")
                .as_slice(),
            "ptx"
        );
        assert_eq!(
            pty.as_slice(),
            hex_bytes("5687e972e50f73e3898861e7edad21bea7d5f622df88243bb804920ae8e647fa")
                .as_slice(),
            "pty"
        );
    }

    /// ISC-3: val (sorted-MAC HKDF-Extract, the MAC-order trap) and PWE = val·PT
    /// — the timing-trap stage, cross-validated byte-identical across hostap+iwd.
    #[test]
    fn sae_isc3_pwe_kat() {
        use super::wpa::sae;
        // PWE-pair MACs (hostap addr1b/addr2b, line 389-390). The FINDINGS rendered
        // iwd's mac2[5] as "46" (decimal 0x2e); the real byte is 0x46 — confirmed by
        // val_data matching only with 0x46, and by hostap's `0x46` literal.
        let mac1 = [0x00, 0x09, 0x5b, 0x66, 0xec, 0x1e];
        let mac2 = [0x00, 0x0b, 0x6b, 0xd9, 0x02, 0x46];

        assert_eq!(
            sae::pwe_val(&mac1, &mac2).to_bytes().as_slice(),
            hex_bytes("bb7f9cac5aa8b72c02b5daacc2771abe74e72604612295eca2ce18363ae9a927")
                .as_slice(),
            "val"
        );

        let pt = sae::derive_pt(SAE_SSID, SAE_PW, SAE_PWID);
        let (pwex, pwey) = sae::point_xy(&sae::derive_pwe(&pt, &mac1, &mac2)).unwrap();
        assert_eq!(
            pwex.as_slice(),
            hex_bytes("c93049b9e64000f848201649e999f2b5c22dea69b5632c9df4d633b8aa1f6c1e")
                .as_slice(),
            "PWE.x"
        );
        assert_eq!(
            pwey.as_slice(),
            hex_bytes("73634e94b53d82e7383a8d258199d9dc1a5ee8269d060382ccbf33e614ff59a0")
                .as_slice(),
            "PWE.y"
        );
    }

    // Handshake test vector (hostap `sae_tests`, IEEE 802.11-2020 §J.10). NOTE:
    // hostap derives this handshake's PWE by HUNTING-AND-PECKING (`sae_prepare_
    // commit`, line 396) — NOT H2E. We implement H2E only (per scope), so we KAT
    // the PWE-AGNOSTIC handshake math (commit serialization, the key schedule,
    // confirm) against these published values, obtaining the handshake's PWE
    // algebraically from the published commit (PWE = mask⁻¹·(−element)) rather
    // than by implementing hunting-and-pecking. The oracle stays the published
    // KCK/PMK/PMKID — published-answer, not round-trip. (The H2E PWE itself is
    // KAT'd directly in ISC-3.) See ADR-051 / the close report.
    const SAE_LOCAL_COMMIT: &str = concat!(
        "13002e2c0f0db52440ad146d967114ce005ce1eab0aa2c2e",
        "5c2871b774f6c2575c65d5ad9e00829707aa36ba8b859738",
        "fc961d08243505f47c035376d7ac4bc8d7b95083bf43827d",
        "0fc31ed778dd3671fd21a46d1091d64b6f9a1e1272621325",
        "dbe1"
    );
    const SAE_PEER_COMMIT: &str = concat!(
        "1300591b96f3397fb945100848e7b550543b6720d88337ee",
        "93fc49fd6df7e08b5223e71b9bb048d3873f20556953a96c",
        "91536fd8ee6ca9b4a68a148b056a909be03e83ae208f60f8",
        "ef5537858074db06687032399862999b511e0a1552a5fea3",
        "17c2"
    );
    const SAE_LOCAL_RAND: &str = "992465fd3daa3c60aa6565b7f62a2a7f2e12dd12f198faf4fbed89d7ff1ace94";
    const SAE_LOCAL_MASK: &str = "9507a90f777a044d6a0830b91ea3d5dd70bece44e1acffb86983b5e1bf9fb322";
    const SAE_KCK: &str = "1e733f6d9bd53256287304338831b09a39406d121017073a5c30db36f36cb81a";

    /// ISC-4: commit construct/parse + the 98-byte wire serialization. The
    /// commit-SCALAR = (rand+mask) mod q is PWE-independent and KAT'd directly
    /// against the published `local_commit[2..34]`; the full 98-byte layout
    /// (group LE ‖ scalar BE ‖ element x‖y) is KAT'd byte-for-byte by parsing the
    /// published 98-byte `local_commit`/`peer_commit` (group id + on-curve
    /// element validated) and serializing them back identically.
    #[test]
    fn sae_isc4_commit_kat() {
        use super::wpa::sae;
        let local_rand: [u8; 32] = hex_bytes(SAE_LOCAL_RAND).try_into().unwrap();
        let local_mask: [u8; 32] = hex_bytes(SAE_LOCAL_MASK).try_into().unwrap();
        let local_commit = hex_bytes(SAE_LOCAL_COMMIT);
        let peer_commit = hex_bytes(SAE_PEER_COMMIT);

        // commit-scalar (PWE-independent) matches the published wire scalar.
        let scalar = sae::commit_scalar(&local_rand, &local_mask).unwrap();
        assert_eq!(
            scalar.to_bytes().as_slice(),
            &local_commit[2..34],
            "commit-scalar"
        );

        // The 98-byte wire format round-trips for both published messages:
        // parse (validates group=19 + on-curve element) then serialize back.
        for (name, wire) in [("local", &local_commit), ("peer", &peer_commit)] {
            let c = sae::parse_commit(wire).unwrap_or_else(|| panic!("{name} parses + on-curve"));
            assert_eq!(c.group, sae::GROUP_P256);
            assert_eq!(
                sae::serialize_commit(c.group, &c.scalar, &c.element)
                    .unwrap()
                    .as_slice(),
                wire.as_slice(),
                "{name} commit round-trip"
            );
        }

        // Negative: a non-P256 group id and a truncated buffer are rejected.
        let mut bad_group = local_commit.clone();
        bad_group[0] = 0x14; // group 20, not present
        assert!(sae::parse_commit(&bad_group).is_none());
        assert!(sae::parse_commit(&local_commit[..97]).is_none());
    }

    /// ISC-5: the shared-secret → KCK/PMK/PMKID key schedule, KAT'd against the
    /// published hostap values. Because this handshake's PWE is hunting-and-
    /// pecking-derived (and we implement H2E only), we recover that PWE from the
    /// published commit (`PWE = mask⁻¹·(−element)`, pure published-byte algebra,
    /// no hunting-and-pecking) and drive our H2E-identical key schedule with it.
    /// The published KCK/PMK/PMKID are the oracle; `k` is pinned transitively.
    #[test]
    fn sae_isc5_keys_kat() {
        use super::wpa::sae;
        use p256::Scalar;
        use p256::elliptic_curve::ff::PrimeField;
        use p256::elliptic_curve::generic_array::GenericArray;

        let local_rand: [u8; 32] = hex_bytes(SAE_LOCAL_RAND).try_into().unwrap();
        let local_mask: [u8; 32] = hex_bytes(SAE_LOCAL_MASK).try_into().unwrap();
        let local_commit = hex_bytes(SAE_LOCAL_COMMIT);
        let peer_commit = hex_bytes(SAE_PEER_COMMIT);

        let own = sae::parse_commit(&local_commit).unwrap();
        let peer = sae::parse_commit(&peer_commit).unwrap();
        let own_scalar = sae::commit_scalar(&local_rand, &local_mask).unwrap();
        assert_eq!(
            own_scalar.to_bytes().as_slice(),
            own.scalar.to_bytes().as_slice()
        );

        // Recover the handshake PWE from the published commit element + mask:
        // own.element = −(mask·PWE) ⇒ PWE = mask⁻¹·(−element). Pure algebra over
        // published bytes — no hunting-and-pecking implemented.
        let mask = Scalar::from_repr(*GenericArray::from_slice(&local_mask)).unwrap();
        let pwe = (-own.element) * mask.invert().unwrap();

        let k = sae::derive_k(&pwe, &peer.scalar, &peer.element, &local_rand).unwrap();
        let keys = sae::derive_keys(&k, &own_scalar, &peer.scalar);
        assert_eq!(keys.kck.as_slice(), hex_bytes(SAE_KCK).as_slice(), "KCK");
        assert_eq!(
            keys.pmk.as_slice(),
            hex_bytes("4e4dfab1a2dd8ac1a91790f953faaa452ae5c6873ab75b63605ba663f8a7fe59")
                .as_slice(),
            "PMK"
        );
        assert_eq!(
            keys.pmkid.as_slice(),
            hex_bytes("8747a600eea3f9f22475df58ca1e5498").as_slice(),
            "PMKID"
        );
    }

    /// ISC-6: confirm. Neither source publishes a confirm vector, so the expected
    /// value is computed by an INDEPENDENT HMAC-SHA256 over PUBLISHED inputs (the
    /// published KCK + the published commit wire bytes + a known send-confirm) —
    /// a known-answer test (a trusted primitive over published bytes), NOT a
    /// round-trip of our SAE code against itself. Our `sae::confirm`, fed the
    /// scalars/elements parsed from the published commits, must reproduce it.
    #[test]
    fn sae_isc6_confirm_kat() {
        use super::wpa::sae;
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        let kck = hex_bytes(SAE_KCK);
        let local_commit = hex_bytes(SAE_LOCAL_COMMIT);
        let peer_commit = hex_bytes(SAE_PEER_COMMIT);
        let send_confirm: u16 = 1;

        // Independent expected: HMAC-SHA256(KCK, sc ‖ local[2..98] ‖ peer[2..98])
        // over the *published* commit bytes (scalar‖x‖y for each).
        let mut m = Hmac::<Sha256>::new_from_slice(&kck).unwrap();
        m.update(&send_confirm.to_le_bytes());
        m.update(&local_commit[2..98]);
        m.update(&peer_commit[2..98]);
        let expected = m.finalize().into_bytes();

        // Our SAE confirm over the scalars/elements parsed from the published
        // commits (so the inputs provably come from the published vector, not the
        // code under test).
        let own = sae::parse_commit(&local_commit).unwrap();
        let peer = sae::parse_commit(&peer_commit).unwrap();
        let got = sae::confirm(
            &kck,
            send_confirm,
            &own.scalar,
            &own.element,
            &peer.scalar,
            &peer.element,
        )
        .unwrap();

        assert_eq!(got.as_slice(), expected.as_slice(), "confirm");
    }

    // ── CB-5a ISC-1: SAE on-air state machine + Auth frame wire ──────────────

    /// CB-5a ISC-1: the SAE finite-state machine drives two peers (STA + AP role)
    /// to a shared PMK end-to-end — Commit ↔ Commit, Confirm ↔ Confirm, both
    /// Accepted. H2E is MAC-symmetric so both compute the same PWE, k, and keys;
    /// the confirm cross-verify (scalars swapped) is the proof the exchange is
    /// wired correctly. (The crypto primitives are vector-KAT'd above; this KATs
    /// the orchestration.)
    #[test]
    fn sae_sm_isc1_roundtrip() {
        use super::wpa::sae_sm::{SaeError, SaeSession, SaeState};

        let ssid = b"byteme";
        let pw = b"mekmitasdigoat";
        let id = b"";
        let sta = [2u8, 0, 0, 0, 0, 0];
        let ap = [2u8, 0, 0, 0, 0, 1];
        // Per-commit scalars, all < q.
        let mut s = SaeSession::new(ssid, pw, id, &sta, &ap, &[0x11; 32], &[0x22; 32]).unwrap();
        let mut a = SaeSession::new(ssid, pw, id, &ap, &sta, &[0x33; 32], &[0x44; 32]).unwrap();

        // Commit exchange.
        let s_commit = s.commit_body().unwrap();
        let a_commit = a.commit_body().unwrap();
        assert_eq!(
            s_commit.len(),
            98,
            "H2E commit body is the 98-byte serialize"
        );
        a.on_peer_commit(&s_commit).unwrap();
        s.on_peer_commit(&a_commit).unwrap();
        assert_eq!(s.state(), SaeState::Committed);
        assert_eq!(a.state(), SaeState::Committed);

        // Confirm exchange.
        let s_confirm = s.confirm_body().unwrap();
        let a_confirm = a.confirm_body().unwrap();
        assert_eq!(s_confirm.len(), 34, "confirm = send-confirm(2) + CN(32)");
        a.on_peer_confirm(&s_confirm).unwrap();
        s.on_peer_confirm(&a_confirm).unwrap();
        assert_eq!(s.state(), SaeState::Accepted);
        assert_eq!(a.state(), SaeState::Accepted);

        // The whole point: both sides derive the SAME PMK.
        let pmk_s = s.pmk().expect("STA PMK");
        let pmk_a = a.pmk().expect("AP PMK");
        assert_eq!(pmk_s, pmk_a, "both peers agree on the PMK");
        assert_ne!(pmk_s, [0u8; 32], "PMK is non-trivial");

        // Negative: a tampered confirm hash is rejected (constant-time compare).
        let mut s2 = SaeSession::new(ssid, pw, id, &sta, &ap, &[0x11; 32], &[0x22; 32]).unwrap();
        s2.on_peer_commit(&a_commit).unwrap();
        let mut bad = a.confirm_body().unwrap();
        bad[10] ^= 0xff;
        assert_eq!(s2.on_peer_confirm(&bad), Err(SaeError::ConfirmMismatch));
    }

    /// CB-5a ISC-1 byte-oracle: the SAE Authentication-frame wire (T-049/T-050).
    /// `alg=SAE(3) ‖ transaction ‖ status ‖ body`, at the fixed offsets past the
    /// 24-byte mgmt header, and `sae_auth_body` recovers exactly the body.
    #[test]
    fn sae_sm_isc1_auth_frame_byte_oracle() {
        use super::mlme_logic;
        use super::wpa::sae_sm::{SAE_HASH_TO_ELEMENT, SaeSession};

        let sta = [2u8, 0, 0, 0, 0, 0];
        let ap = [2u8, 0, 0, 0, 0, 1];
        let s =
            SaeSession::new(b"byteme", b"pw", b"", &sta, &ap, &[0x11; 32], &[0x22; 32]).unwrap();
        let commit = s.commit_body().unwrap();

        let mut buf = [0u8; 128];
        // Commit: transaction 1, status 126 (H2E).
        let n = mlme_logic::build_sae_auth(&mut buf, &sta, &ap, 1, SAE_HASH_TO_ELEMENT, &commit)
            .unwrap();
        assert_eq!(n, 24 + 6 + commit.len());
        assert_eq!(
            u16::from_le_bytes([buf[24], buf[25]]),
            3,
            "auth_alg = SAE(3)"
        );
        assert_eq!(u16::from_le_bytes([buf[26], buf[27]]), 1, "transaction 1");
        assert_eq!(
            u16::from_le_bytes([buf[28], buf[29]]),
            126,
            "status = H2E(126)"
        );
        assert_eq!(&buf[30..30 + commit.len()], commit.as_slice(), "SAE body");
        assert_eq!(
            mlme_logic::sae_auth_body(&buf[..n]).unwrap(),
            commit.as_slice()
        );

        // Confirm: transaction 2, status 0.
        let mut s2 =
            SaeSession::new(b"byteme", b"pw", b"", &sta, &ap, &[0x11; 32], &[0x22; 32]).unwrap();
        let a =
            SaeSession::new(b"byteme", b"pw", b"", &ap, &sta, &[0x33; 32], &[0x44; 32]).unwrap();
        s2.on_peer_commit(&a.commit_body().unwrap()).unwrap();
        let confirm = s2.confirm_body().unwrap();
        let mut buf2 = [0u8; 128];
        let n2 = mlme_logic::build_sae_auth(&mut buf2, &sta, &ap, 2, 0, &confirm).unwrap();
        assert_eq!(u16::from_le_bytes([buf2[26], buf2[27]]), 2, "transaction 2");
        assert_eq!(u16::from_le_bytes([buf2[28], buf2[29]]), 0, "status 0");
        assert_eq!(
            mlme_logic::sae_auth_body(&buf2[..n2]).unwrap(),
            confirm.as_slice()
        );
    }

    /// CB-5a ISC-1: anti-clogging (H2E container, T-051). An AP reply with the
    /// group + IE-255/ext-id-93 token is absorbed, and the NEXT Commit body
    /// carries that container appended after the 98-byte commit.
    #[test]
    fn sae_sm_isc1_anti_clogging() {
        use super::wpa::sae_sm::{SaeError, SaeSession};

        let sta = [2u8, 0, 0, 0, 0, 0];
        let ap = [2u8, 0, 0, 0, 0, 1];
        let mut s =
            SaeSession::new(b"byteme", b"pw", b"", &sta, &ap, &[0x11; 32], &[0x22; 32]).unwrap();

        let token = [0xdeu8; 8];
        // AP anti-clogging body: group(19 LE) ‖ [255][token_len+1][93][token].
        let mut body = vec![0x13u8, 0x00, 255, (token.len() + 1) as u8, 93];
        body.extend_from_slice(&token);
        s.on_anti_clogging(&body).unwrap();

        let commit = s.commit_body().unwrap();
        assert_eq!(commit.len(), 98 + 3 + token.len(), "commit + container");
        assert_eq!(&commit[98..101], &[255, (token.len() + 1) as u8, 93]);
        assert_eq!(&commit[101..], &token);

        // Negative: a bad ext-id is rejected.
        let mut bad =
            SaeSession::new(b"byteme", b"pw", b"", &sta, &ap, &[0x11; 32], &[0x22; 32]).unwrap();
        let bad_body = vec![0x13u8, 0x00, 255, 2, 92, 0xaa]; // ext-id 92, not 93
        assert_eq!(
            bad.on_anti_clogging(&bad_body),
            Err(SaeError::BadAntiClogging)
        );
    }

    /// CB-5a ISC-2 byte-oracle: the WPA3-SAE RSN IE — hand-laid per IEEE
    /// 802.11-2020 §9.4.2.24 (AKM SAE=8) + trap-mine T-059 (MFPC+MFPR). The 22
    /// bytes must match exactly, independent of the builder.
    #[test]
    fn rsn_ie_wpa3_sae_byte_oracle() {
        use super::mlme_logic;
        let expected = hex_bytes(concat!(
            "3014",         // EID_RSN(48), len 20
            "0100",         // version 1
            "000fac04",     // group cipher CCMP
            "0100000fac04", // 1 pairwise: CCMP
            "0100000fac08", // 1 AKM: SAE(8)
            "c000",         // RSN caps: MFPC|MFPR = 0x00C0 (LE)
        ));
        let mut buf = [0u8; 40];
        let n = mlme_logic::build_rsn_ie_wpa3_sae(&mut buf, 0).unwrap();
        assert_eq!(n, 22);
        assert_eq!(&buf[..n], expected.as_slice());
        let caps = u16::from_le_bytes([buf[20], buf[21]]);
        assert_ne!(caps & mlme_logic::RSN_CAP_MFPR, 0, "MFPR set");
        assert_ne!(caps & mlme_logic::RSN_CAP_MFPC, 0, "MFPC set");
    }

    // ── CB-5a ISC-3: BIP-CMAC-128 / 802.11w software-verify ──────────────────

    /// A minimal deauth management frame (24-byte header + 2-byte reason). FC has
    /// the Retry bit set so the AAD-masking (T-055) is actually exercised.
    fn bip_test_frame(buf: &mut [u8]) -> usize {
        let fc = 0x00c0u16 | 0x0800; // deauth | retry
        buf[0..2].copy_from_slice(&fc.to_le_bytes());
        buf[2..4].copy_from_slice(&[0, 0]); // duration
        buf[4..10].copy_from_slice(&[0x02, 0, 0, 0, 0, 0x11]); // A1
        buf[10..16].copy_from_slice(&[0x02, 0, 0, 0, 0, 0x22]); // A2
        buf[16..22].copy_from_slice(&[0x02, 0, 0, 0, 0, 0x33]); // A3
        buf[22..24].copy_from_slice(&[0, 0]); // seq
        buf[24..26].copy_from_slice(&[0x03, 0x00]); // reason code (body)
        26
    }

    /// CB-5a ISC-3 byte-oracle: the MMIE layout (T-054) — `eid 76 · len 16 ·
    /// key_id LE · IPN 6 · MIC 8`, appended by `protect_mgmt` and accepted by
    /// `verify_mmie`.
    #[test]
    fn bip_mmie_byte_oracle() {
        use super::wpa::bip::{self, Igtk};
        let key = [0xa5u8; 16];
        let ipn = [7u8, 0, 0, 0, 0, 0];
        let mut buf = [0u8; 64];
        let body_end = bip_test_frame(&mut buf);
        let end = bip::protect_mgmt(&mut buf, body_end, &key, 4, &ipn).unwrap();
        assert_eq!(end, body_end + 18, "MMIE is 18 bytes");
        assert_eq!(buf[body_end], 76, "MMIE element_id = WLAN_EID_MMIE");
        assert_eq!(buf[body_end + 1], 16, "MMIE length = 16 (CMAC-128)");
        assert_eq!(
            u16::from_le_bytes([buf[body_end + 2], buf[body_end + 3]]),
            4,
            "key_id LE"
        );
        assert_eq!(&buf[body_end + 4..body_end + 10], &ipn, "IPN");
        assert_ne!(
            &buf[body_end + 10..body_end + 18],
            &[0u8; 8],
            "MIC computed"
        );
        let mut igtk = Igtk::new(4, key, [0u8; 6]);
        assert!(bip::verify_mmie(&mut igtk, &buf[..end]).is_ok());
    }

    /// CB-5a ISC-3 KAT: `bip_cmac_mic` reproduces an INDEPENDENT AES-CMAC-128 over
    /// the T-055 input (`masked-FC ‖ A1‖A2‖A3 ‖ body+MMIE-with-MIC-zeroed`),
    /// truncated to 8 — trusted RustCrypto primitive over hand-laid bytes, not a
    /// round-trip. The Retry bit in the FC proves the masking.
    #[test]
    fn bip_mic_kat() {
        use super::wpa::bip;
        use aes::Aes128;
        use cmac::{Cmac, Mac};
        let key = [0x5au8; 16];
        let ipn = [9u8, 0, 0, 0, 0, 0];
        let mut buf = [0u8; 64];
        let body_end = bip_test_frame(&mut buf);
        let end = bip::protect_mgmt(&mut buf, body_end, &key, 5, &ipn).unwrap();

        // Independent expected MIC per T-055.
        let fc = u16::from_le_bytes([buf[0], buf[1]]);
        let masked = fc & !(0x0800u16 | 0x1000 | 0x2000);
        let mut aad = [0u8; 20];
        aad[0..2].copy_from_slice(&masked.to_le_bytes());
        aad[2..20].copy_from_slice(&buf[4..22]);
        let mut m = Cmac::<Aes128>::new_from_slice(&key).unwrap();
        m.update(&aad);
        m.update(&buf[24..end - 8]); // body + MMIE header (MIC excluded)
        m.update(&[0u8; 8]);
        let expected = m.finalize().into_bytes();
        assert_eq!(
            &buf[end - 8..end],
            &expected[..8],
            "BIP MIC = CMAC(IGTK, T-055 input)[..8]"
        );

        // Negative: a flipped body byte fails verification.
        let mut tampered = buf;
        tampered[24] ^= 0xff;
        let mut igtk = bip::Igtk::new(5, key, [0u8; 6]);
        assert_eq!(
            bip::verify_mmie(&mut igtk, &tampered[..end]),
            Err(bip::BipError::MicFail)
        );
    }

    /// CB-5a ISC-3 KAT: IPN replay (T-056). Received IPN must be strictly greater;
    /// `rx_pn` advances ONLY after the MIC verifies (a bad-MIC forgery cannot
    /// poison the window).
    #[test]
    fn bip_ipn_replay_kat() {
        use super::wpa::bip::{self, BipError, Igtk};
        let key = [0x3cu8; 16];
        let mut igtk = Igtk::new(4, key, [0u8; 6]);

        let mk = |ipn: [u8; 6]| {
            let mut b = [0u8; 64];
            let e = bip_test_frame(&mut b);
            let end = bip::protect_mgmt(&mut b, e, &key, 4, &ipn).unwrap();
            (b, end)
        };

        let (f5, e5) = mk([5, 0, 0, 0, 0, 0]);
        assert!(bip::verify_mmie(&mut igtk, &f5[..e5]).is_ok());
        assert_eq!(igtk.rx_pn, [5, 0, 0, 0, 0, 0]);
        // Same IPN → replay; lower IPN → replay.
        assert_eq!(
            bip::verify_mmie(&mut igtk, &f5[..e5]),
            Err(BipError::Replay)
        );
        let (f3, e3) = mk([3, 0, 0, 0, 0, 0]);
        assert_eq!(
            bip::verify_mmie(&mut igtk, &f3[..e3]),
            Err(BipError::Replay)
        );

        // Higher IPN but corrupt MIC → MicFail, and rx_pn is NOT advanced.
        let (mut f6, e6) = mk([6, 0, 0, 0, 0, 0]);
        f6[e6 - 1] ^= 0xff;
        assert_eq!(
            bip::verify_mmie(&mut igtk, &f6[..e6]),
            Err(BipError::MicFail)
        );
        assert_eq!(
            igtk.rx_pn,
            [5, 0, 0, 0, 0, 0],
            "bad MIC must not advance rx_pn"
        );
        // A valid higher IPN is then accepted.
        let (g6, g6e) = mk([6, 0, 0, 0, 0, 0]);
        assert!(bip::verify_mmie(&mut igtk, &g6[..g6e]).is_ok());
        assert_eq!(igtk.rx_pn, [6, 0, 0, 0, 0, 0]);
    }

    /// CB-5a ISC-3 byte-oracle: the IGTK KDE (T-057) — `KeyID(2 LE) ‖ IPN(6) ‖
    /// IGTK(16)` in a `DD ‖ len ‖ 00 0F AC 09` wrapper — parses, and the 1-byte-
    /// keyidx GTK KDE (type 1) is NOT mistaken for it.
    #[test]
    fn bip_igtk_kde_parse() {
        use super::wpa::bip;
        // DD | len(28) | 00 0F AC 09 | keyid(04 00) | ipn(6) | igtk(16)
        let mut kde = vec![0xDDu8, 28, 0x00, 0x0f, 0xac, 9, 0x04, 0x00];
        let ipn = [1u8, 2, 3, 4, 5, 6];
        kde.extend_from_slice(&ipn);
        let igtk_key = [0x77u8; 16];
        kde.extend_from_slice(&igtk_key);
        let parsed = bip::find_igtk_kde(&kde).expect("IGTK KDE parses");
        assert_eq!(parsed.key_id, 4);
        assert_eq!(parsed.rx_pn, ipn);
        assert_eq!(parsed.key, igtk_key);

        // A GTK KDE (type 1) must NOT be parsed as an IGTK.
        let gtk_kde = vec![0xDDu8, 22, 0x00, 0x0f, 0xac, 1, 0x00, 0x00];
        assert!(bip::find_igtk_kde(&gtk_kde).is_none());
    }

    /// CB-5a ISC-3: the protected-mgmt decision table (T-058) — the headline PMF
    /// property (forged unprotected deauth ignored post-key) AND the pre-key
    /// `RX_CONTINUE` exception.
    #[test]
    fn bip_mgmt_action() {
        use super::wpa::bip::{MgmtAction, mgmt_action};
        let deauth = 0x00c0u16;
        let assoc_resp = 0x0010u16; // not robust
        // Unprotected deauth, MFP on, key installed → dropped (the PASS property).
        assert_eq!(
            mgmt_action(deauth, true, true, false),
            MgmtAction::DropUnprotected
        );
        // Unprotected deauth, MFP on, no key yet → permitted (mid-handshake).
        assert_eq!(
            mgmt_action(deauth, true, false, false),
            MgmtAction::ProcessPreKey
        );
        // Protected (MMIE-verified) deauth → processed.
        assert_eq!(
            mgmt_action(deauth, true, true, true),
            MgmtAction::ProcessProtected
        );
        // MFP off → always processed.
        assert_eq!(mgmt_action(deauth, false, true, false), MgmtAction::Process);
        // Non-robust frame → processed regardless.
        assert_eq!(
            mgmt_action(assoc_resp, true, true, false),
            MgmtAction::Process
        );
    }

    /// T-039 oracle: GKH Message 1 GTK-KDE byte fixture — hand-laid per
    /// wpa_supplicant-2.11/src/rsn_supp/wpa.c:3170–3216 (the RSN GKH handler).
    /// Validates the GTK KDE layout that CB-5's bare-metal SAE run depends on:
    /// keyidx/tx prefix at GTK[0..2], raw GTK bytes at GTK[2..].
    #[test]
    fn wpa_gkh_message1_gtk_kde_byte_fixture() {
        use super::wpa::eapol::*;

        // ── Hand-laid GKH Message 1 (AP → STA) — the oracle fixture.
        // Key Info: group(0) | ack(0x80) | mic(0x100) | secure(0x200) |
        //          encr_key_data(0x1000) | version(v3=0x03) = 0x1383. INSTALL is
        //          CLEAR — CB-5a trap-mine T-060 corrected the earlier 0x13C3:
        //          Install=1 is a 4-way-M3-only bit, and iwd's
        //          `eapol_verify_gtk_1_of_2` rejects Install=1 on a GKH frame.
        let gkh_key_info: u16 = GKH_M1_KEY_INFO; // 0x1383
        assert_eq!(gkh_key_info, 0x1383);
        assert_eq!(gkh_key_info & KEY_INFO_KEY_TYPE, 0); // group key (bit 3 clear)
        assert_eq!(gkh_key_info & KEY_INFO_INSTALL, 0); // install CLEAR (T-060)
        assert_ne!(gkh_key_info & KEY_INFO_ACK, 0); // ack bit set
        assert_ne!(gkh_key_info & KEY_INFO_MIC, 0); // mic present
        assert_ne!(gkh_key_info & KEY_INFO_SECURE, 0); // secure
        assert_ne!(gkh_key_info & KEY_INFO_ENCRYPTED, 0); // encrypted key data

        let kck = [0xABu8; 16]; // test KCK (SAE-derived, not real)
        let replay_counter = [0u8, 0, 0, 0, 0, 0, 0, 5]; // AP's GTK retransmission counter
        let ap_nonce = [0u8; 32]; // GKH M1: AP nonce may be all zeros per IEEE §12.7.5

        // ── GTK KDE payload (the encrypted Key Data content) — hand-laid per
        // wpa_supplicant-2.11/src/rsn_supp/wpa.c:3213–3216.
        // The GTK KDE is wrapped as a vendor-specific element (0xDD):
        //   Element ID = 0xDD, Length = 4(OUI-Type) + 2(prefix) + 16(GTK) = 22
        //   OUI-Type = RSN_KEY_DATA_GROUPKEY = 0x000fac01
        //   Prefix[0] = keyidx (low 2 bits) | tx_flag (bit 2)
        //   Prefix[1] = reserved
        //   GTK bytes = 16B for CCMP-128

        let gtk_keyidx: u8 = 2; // GTK slot 2
        let gtk_tx: bool = false; // STA receives, doesn't transmit with GTK
        let gtk_prefix0 = (gtk_keyidx & 0x03) | (if gtk_tx { 1 << 2 } else { 0 });
        let gtk_prefix1 = 0x00;

        // The raw GTK — a known test key (not derived from any real handshake).
        let gtk_bytes: [u8; 16] = [
            0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54,
            0x32, 0x10,
        ];

        // Build the GTK KDE: vendor-specific element wrapper.
        let mut gtk_kde = [0u8; 24]; // 1(elem_id) + 1(len) + 4(oui_type) + 2(prefix) + 16(gtk)
        gtk_kde[0] = 0xDD; // WLAN_EID_VENDOR_SPECIFIC
        gtk_kde[1] = 22; // length (everything after this byte)
        gtk_kde[2..6].copy_from_slice(&0x000F_AC01u32.to_be_bytes()); // RSN_KEY_DATA_GROUPKEY
        gtk_kde[6] = gtk_prefix0; // keyidx + tx flag
        gtk_kde[7] = gtk_prefix1; // reserved
        gtk_kde[8..24].copy_from_slice(&gtk_bytes); // the GTK

        // ── Oracle assertions: re-parse the KDE and recover the exact fields.
        // This is what wpa_supplicant does at lines 3213–3216.
        assert_eq!(gtk_kde[0], 0xDD); // vendor-specific element
        assert_eq!(gtk_kde[6] & 0x03, gtk_keyidx); // keyidx = 2 (line 3213)
        assert_eq!((gtk_kde[6] >> 2) & 1, u8::from(gtk_tx)); // tx flag (line 3214-3215)
        assert_eq!(&gtk_kde[8..24], &gtk_bytes); // GTK at offset 2 within prefix+GTK (line 3216)

        // ── Build the GKH Message 1 EAPOL-Key frame with the GTK KDE as Key Data.
        let mut buf = [0u8; 256];
        let n = build_key_frame(
            &mut buf,
            2, // EAPOL version
            gkh_key_info,
            16, // CCMP-128 key length
            &replay_counter,
            &ap_nonce,
            &gtk_kde[..24],
        )
        .unwrap();

        // Frame size: 99 (fixed descriptor through Key Data start) + 24 (GTK KDE) = 123
        assert_eq!(n, 123);

        // Parse back and verify the structure.
        let kf = KeyFrame::parse(&buf[..n]).unwrap();
        assert_eq!(kf.key_info(), gkh_key_info);
        assert_eq!(kf.version(), KeyDescriptorVersion::AesCmac); // v3 → AES-CMAC
        assert_eq!(kf.key_data_len() as usize, 24);

        // CB-5a ISC-4 (T-060): the GKH M1 verifier accepts the corrected
        // (Install=0) frame and REJECTS a 4-way-M3-style Install=1 frame.
        assert!(verify_gkh_m1(&kf), "GKH M1 (Install=0) verifies");
        let mut m3ish = buf;
        m3ish[KEY_INFO_OFFSET..KEY_INFO_OFFSET + 2]
            .copy_from_slice(&(gkh_key_info | KEY_INFO_INSTALL).to_be_bytes());
        let kf_bad = KeyFrame::parse(&m3ish[..n]).unwrap();
        assert!(!verify_gkh_m1(&kf_bad), "Install=1 rejected on GKH (T-060)");

        // The GTK KDE in the encrypted Key Data parses (reused from the 4-way).
        let (gtk, keyidx) = gtk_from_kde(&gtk_kde[..24]).expect("GTK KDE");
        assert_eq!(keyidx, gtk_keyidx);
        assert_eq!(gtk, &gtk_bytes);

        // MIC install + verify round-trip (the "zero-before-compute" invariant).
        assert!(install_mic(
            KeyDescriptorVersion::AesCmac,
            &kck,
            &mut buf[..n]
        ));
        assert!(verify_mic(KeyDescriptorVersion::AesCmac, &kck, &buf[..n]));

        // ── GKH Message 2 (STA → AP acknowledgment) — empty Key Data.
        // Key Info: group(0) | mic(0x100) | secure(0x200) | version(v3=0x03) = 0x0303
        let gkh_ack_key_info: u16 = 0x0303; // group + mic + secure + v3
        assert_eq!(gkh_ack_key_info & KEY_INFO_KEY_TYPE, 0); // still group key
        assert_eq!(gkh_ack_key_info & KEY_INFO_INSTALL, 0); // no install on ack
        assert_eq!(gkh_ack_key_info & KEY_INFO_ACK, 0); // no ack on ack (per IEEE §12.7.5)

        let mut ack_buf = [0u8; 128];
        let ack_n = build_key_frame(
            &mut ack_buf,
            2,
            gkh_ack_key_info,
            16,
            &replay_counter, // echoes AP's replay counter
            &ap_nonce,       // zeroed in M2 per IEEE §12.7.5
            &[],             // empty Key Data
        )
        .unwrap();

        assert_eq!(ack_n, 99); // fixed descriptor, no key data
        let ack_kf = KeyFrame::parse(&ack_buf[..ack_n]).unwrap();
        assert_eq!(ack_kf.key_data_len(), 0); // no KDE in acknowledgment
        assert!(install_mic(
            KeyDescriptorVersion::AesCmac,
            &kck,
            &mut ack_buf[..ack_n]
        ));
        assert!(verify_mic(
            KeyDescriptorVersion::AesCmac,
            &kck,
            &ack_buf[..ack_n]
        ));

        // CB-5a ISC-4: build_gkh_m2 produces the same ACK (key_info 0x0303,
        // key_len 0, empty Key Data) as the hand-laid oracle above.
        let mut m2 = [0u8; 128];
        let m2n = build_gkh_m2(&mut m2, 2, &replay_counter).unwrap();
        let m2kf = KeyFrame::parse(&m2[..m2n]).unwrap();
        assert_eq!(m2kf.key_info(), GKH_M2_KEY_INFO);
        assert_eq!(GKH_M2_KEY_INFO, gkh_ack_key_info); // 0x0303
        assert_eq!(m2kf.key_data_len(), 0);

        // ── Precondition gate: msg_3_of_4_ok must be set before GKH.
        // wpa_supplicant line 3139 rejects GKH if the 4-way hasn't completed.
        // This is a structural invariant, not testable in isolation here, but the
        // fixture documents the gate for CB-5's bare-metal SAE run.
    }

    /// Phase 7.5 BUILD-A (ISC-2/C-6): the xHCI ring arithmetic — the F4/F5
    /// discriminator logic under a truth table BEFORE any blind run. Cycle
    /// protocol, Link-TRB wrap, event-ring CCS flip.
    #[test]
    fn xhci_ring_logic_truth_tables() {
        use super::xhci_ring_logic::{
            ConsumerRing, ProducerRing, TRB_CYCLE, Trb, TrbType, control_for,
        };

        // Producer: 4 entries = 3 payload + 1 Link. Walk two full laps.
        let mut ring = ProducerRing::new(4);
        // Lap 1, cycle=true.
        let s0 = ring.enqueue_step();
        assert_eq!((s0.index, s0.cycle, s0.rearm_link_cycle), (0, true, None));
        let s1 = ring.enqueue_step();
        assert_eq!((s1.index, s1.cycle, s1.rearm_link_cycle), (1, true, None));
        // The third payload lands on the last payload slot: the Link re-arms
        // with the CURRENT cycle (true) and the producer flips for lap 2.
        let s2 = ring.enqueue_step();
        assert_eq!(
            (s2.index, s2.cycle, s2.rearm_link_cycle),
            (2, true, Some(true))
        );
        // Lap 2, cycle=false.
        let s3 = ring.enqueue_step();
        assert_eq!((s3.index, s3.cycle, s3.rearm_link_cycle), (0, false, None));
        let s4 = ring.enqueue_step();
        assert_eq!((s4.index, s4.cycle, s4.rearm_link_cycle), (1, false, None));
        let s5 = ring.enqueue_step();
        assert_eq!(
            (s5.index, s5.cycle, s5.rearm_link_cycle),
            (2, false, Some(false))
        );
        // Lap 3 returns to cycle=true — the toggle is an involution.
        assert!(ring.enqueue_step().cycle);

        // Consumer: 3-entry event ring, two laps of ownership + ERDP math.
        let mut ev = ConsumerRing::new(3);
        let owned = control_for(TrbType::CmdCompletionEvent, true, 0);
        let foreign = control_for(TrbType::CmdCompletionEvent, false, 0);
        assert!(ev.owns(owned));
        assert!(!ev.owns(foreign)); // stale TRB from the previous lap
        assert_eq!(ev.advance(), 1);
        assert_eq!(ev.advance(), 2);
        assert_eq!(ev.advance(), 0); // segment end: wrap + CCS flip
        assert!(!ev.owns(owned)); // lap 2: the OLD cycle no longer owns
        assert!(ev.owns(foreign));

        // TRB field extraction (the §6.4 layout, byte-oracle style).
        let trb = Trb {
            parameter: 0xDEAD_BEEF_0000_1000,
            status: 0x01 << 24, // completion code = Success
            control: control_for(TrbType::CmdCompletionEvent, true, 0) | (5 << 24),
        };
        assert_eq!(trb.trb_type(), TrbType::CmdCompletionEvent as u32);
        assert_eq!(trb.completion_code(), 1);
        assert_eq!(trb.slot_id(), 5);
        assert!(trb.cycle());
        assert_eq!(trb.control & TRB_CYCLE, 1);
    }

    /// Phase 7.5 BUILD-0 (ISC-2): the fenced-DMA-arena pure logic — the
    /// I-14(c1) validation truth table and the span allocator.
    #[test]
    fn dma_arena_logic_truth_tables() {
        use super::dma_arena_logic::{DmaFault, SpanTable, validate};

        const BASE: u64 = 0x10_0000;
        const LEN: u64 = 0x4_0000; // 256 KiB fence

        assert_eq!(validate(BASE, LEN, BASE, LEN), Ok(()));
        assert_eq!(validate(BASE + 0x1000, 0x1000, BASE, LEN), Ok(()));
        assert_eq!(validate(BASE + LEN - 0x1000, 0x1000, BASE, LEN), Ok(()));
        assert_eq!(
            validate(BASE - 0x1000, 0x2000, BASE, LEN),
            Err(DmaFault::OutsideArena)
        );
        assert_eq!(
            validate(BASE + LEN - 0x1000, 0x2000, BASE, LEN),
            Err(DmaFault::OutsideArena)
        );
        assert_eq!(
            validate(BASE + LEN, 0x1000, BASE, LEN),
            Err(DmaFault::OutsideArena)
        );
        assert_eq!(validate(0, 0x1000, BASE, LEN), Err(DmaFault::OutsideArena));
        assert_eq!(validate(BASE, 0, BASE, LEN), Err(DmaFault::ZeroLength));
        assert_eq!(validate(BASE, 0x1000, BASE, 0), Err(DmaFault::ArenaUnset));
        assert_eq!(
            validate(u64::MAX - 0x10, 0x100, BASE, LEN),
            Err(DmaFault::Overflow)
        );
        assert_eq!(
            validate(BASE, LEN, u64::MAX - 0x10, u64::MAX),
            Err(DmaFault::Overflow)
        );

        let mut table = SpanTable::new(64);
        let a = table.alloc(16).expect("a");
        let b = table.alloc(16).expect("b");
        let c = table.alloc(32).expect("c");
        assert_eq!((a, b, c), (0, 16, 32));
        assert_eq!(table.alloc(1), None);
        assert_eq!(table.used_pages(), 64);
        assert!(table.free(b));
        assert!(!table.free(b));
        assert_eq!(table.alloc(32), None);
        assert_eq!(table.alloc(16), Some(16));
        assert!(table.free(a));
        assert!(table.free(16));
        assert!(table.free(c));
        assert_eq!(table.used_pages(), 0);
        assert_eq!(table.alloc(64), Some(0));
        assert!(table.free(0));
        assert!(!table.free(7));
    }

    /// Phase 7 BUILD-O1a (ISC-1/ISC-3): the pure slot-arena logic —
    /// generations, key matching, free-list reuse, the Rule-1 kill-switch,
    /// and the handle-count lifecycle.
    #[test]
    fn accel_arena_logic_truth_tables() {
        use super::arena_logic::{
            AllocPick, SlotKey, alloc_pick, bump_generation, key_matches, recycle_slot,
            release_count, retain_count,
        };

        // Key matching: the I-13 gate. The live generation matches; one free
        // (bump) makes the old key stale; a wrong index never matches.
        let key = SlotKey {
            index: 3,
            generation: 7,
        };
        assert!(key_matches(7, key, 3));
        assert!(!key_matches(bump_generation(7), key, 3));
        assert!(!key_matches(7, key, 4));

        // Generation wrap is benign arithmetic: the bump is total, and a
        // stale key mismatches every generation except its own — aliasing
        // would need exactly 2^32 frees of one index while the key is held
        // (the documented ADR-042 bound).
        assert_eq!(bump_generation(u32::MAX), 0);
        let old = SlotKey {
            index: 0,
            generation: u32::MAX,
        };
        assert!(!key_matches(0, old, 0));

        // Free list: LIFO reuse (deliberately adversarial for I-13 — stale
        // keys collide with hot indices), grow when empty; the kill-switch
        // (reuse off) never recycles → append-only, the pre-O1a registry
        // shape bit-for-bit (the ISC-2 equivalence at the pure level).
        let mut free = vec![];
        assert_eq!(alloc_pick(&mut free, 2), AllocPick::Grow(2));
        recycle_slot(&mut free, 1, true);
        recycle_slot(&mut free, 0, true);
        assert_eq!(alloc_pick(&mut free, 2), AllocPick::Reuse(0));
        assert_eq!(alloc_pick(&mut free, 2), AllocPick::Reuse(1));
        assert_eq!(alloc_pick(&mut free, 2), AllocPick::Grow(2));
        let mut append_only = vec![];
        recycle_slot(&mut append_only, 0, false);
        assert!(append_only.is_empty());
        assert_eq!(alloc_pick(&mut append_only, 3), AllocPick::Grow(3));

        // Handle counts: release frees exactly once at the zero transition;
        // a double release and a retain overflow are refused, never wrapped.
        assert_eq!(retain_count(1), Some(2));
        assert_eq!(retain_count(u32::MAX), None);
        assert_eq!(release_count(2), Some((1, false)));
        assert_eq!(release_count(1), Some((0, true)));
        assert_eq!(release_count(0), None);
    }

    /// Phase 7 BUILD-2 (ISC-5): host unit tests for the pure admission policy —
    /// reservation / device-busy deferral, and priority → quota → FIFO next-choice.
    #[test]
    fn accel_admission_policy_decisions() {
        use super::admission_policy::{
            Decision, DeferReason, DeviceState, Request, admit_arrival, choose_next, losing_reason,
        };
        let req = |principal, priority, seq, reserved| Request {
            principal,
            priority,
            submit_seq: seq,
            reserved,
        };

        // Arrival: free device → admit.
        assert_eq!(
            admit_arrival(&req(7, 5, 1, false), &DeviceState::default()),
            Decision::Admit
        );
        // Arrival: another principal holds a reservation → defer for the holder
        // (reservation outranks plain busy — the explicit cross-tenant claim).
        let reserved_by_7 = DeviceState {
            running: Some(7),
            reservation: Some(7),
        };
        assert_eq!(
            admit_arrival(&req(12, 5, 2, false), &reserved_by_7),
            Decision::Defer(DeferReason::Reservation { holder: 7 })
        );
        // The reservation holder's OWN request is not blocked by its own reservation.
        assert_eq!(
            admit_arrival(&req(7, 5, 3, false), &reserved_by_7),
            Decision::Admit
        );
        // Arrival: device busy with another principal, no reservation → defer (busy).
        let busy_with_9 = DeviceState {
            running: Some(9),
            reservation: None,
        };
        assert_eq!(
            admit_arrival(&req(12, 5, 4, false), &busy_with_9),
            Decision::Defer(DeferReason::DeviceBusy { running: 9 })
        );

        // choose_next: higher priority wins regardless of FIFO.
        let pending = [req(7, 9, 1, false), req(12, 5, 2, false)];
        assert_eq!(choose_next(&pending, |_| false), Some(0)); // PID7 (priority 9)

        // choose_next: same priority → under-quota beats over-quota even if it
        // submitted LATER (quota grounds in §9.5, read via the predicate).
        let pending = [req(7, 5, 1, false), req(12, 5, 2, false)];
        let over_quota = |p: u32| p == 7; // PID7 over quota
        assert_eq!(choose_next(&pending, over_quota), Some(1)); // PID12 (under quota)
        assert_eq!(
            losing_reason(&pending[1], &pending[0], over_quota),
            DeferReason::OverQuota { admitted: 12 }
        );

        // choose_next: same priority, both under quota → earliest FIFO wins.
        let pending = [req(7, 5, 2, false), req(12, 5, 1, false)];
        assert_eq!(choose_next(&pending, |_| false), Some(1)); // PID12 (submit_seq 1)

        // Empty queue → None.
        assert_eq!(choose_next(&[], |_| false), None);
    }

    /// Phase 7 BUILD-3 (ISC-4/6): host unit tests for the pure tensor-sharing
    /// attribution rule — a shared physical copy's bytes are counted once (to the
    /// owner); readers contribute zero, so total residency never inflates with the
    /// number of references (the no-double-count invariant).
    #[test]
    fn tensor_sharing_no_double_count() {
        use super::sharing_logic::{SharedRecord, attributed_resident_bytes, total_resident_bytes};

        let record = SharedRecord {
            owner: 7,
            bytes: 4096,
        };

        // The owner carries the full physical bytes; every other principal carries zero.
        assert_eq!(attributed_resident_bytes(&record, 7), 4096);
        assert_eq!(attributed_resident_bytes(&record, 12), 0);
        assert_eq!(attributed_resident_bytes(&record, 99), 0);

        // Total across the reader set equals the physical copy exactly once, regardless
        // of how many principals reference it.
        assert_eq!(total_resident_bytes(&record, &[7]), 4096); // owner only
        assert_eq!(total_resident_bytes(&record, &[7, 12]), 4096); // owner + 1 reader
        assert_eq!(total_resident_bytes(&record, &[7, 12, 99, 4]), 4096); // owner + 3 readers
        assert_eq!(total_resident_bytes(&record, &[12, 99]), 0); // readers only (owner absent)
        assert_eq!(total_resident_bytes(&record, &[]), 0); // no references

        // A different owner carries its own copy's bytes once; an unrelated principal
        // sharing the same id contributes nothing — sharing never double-counts.
        // Callers pass a de-duplicated reader set (the kernel's `readers()` scan
        // returns one); over such a set a different owner's copy is still counted once.
        let other = SharedRecord {
            owner: 3,
            bytes: 256,
        };
        assert_eq!(total_resident_bytes(&other, &[3]), 256);
        assert_eq!(total_resident_bytes(&other, &[3, 8, 9]), 256);
    }

    /// Phase 7 BUILD-P1 (ISC-1): host unit tests for the pure residency-aware
    /// placement policy — the §1 cases (resident / split / cpu-fallback /
    /// over-quota / uncovered) plus the deterministic lowest-id tiebreak.
    #[test]
    fn accel_placement_policy_decisions() {
        use super::placement_policy::{DeviceCandidate, Placement, PlacementReason, decide};
        const CPU: u32 = 0x8000_0000; // accel_cpu = Device::Accel(0).as_abi()
        const SIM: u32 = 0x8000_0001; // accel_sim = Device::Accel(1).as_abi()
        const VIO: u32 = 0x8000_0002; // virtio    = Device::Accel(2).as_abi()
        let cand = |abi, resident, covers, over, fallback| DeviceCandidate {
            device_abi: abi,
            resident_bytes: resident,
            covers_op: covers,
            over_quota: over,
            is_cpu_fallback: fallback,
        };
        let cpu = |resident| cand(CPU, resident, true, false, true);

        // 1. All operands resident on one covering, under-quota accelerator → place
        //    there, zero migration.
        assert_eq!(
            decide(&[cpu(0), cand(SIM, 64, true, false, false)], 64),
            Placement {
                device_abi: SIM,
                reason: PlacementReason::OperandsResident,
                migrated_bytes: 0,
            }
        );

        // 2. Operands split across devices → place on the one holding the most bytes;
        //    the remainder is the migration.
        assert_eq!(
            decide(
                &[
                    cpu(0),
                    cand(SIM, 40, true, false, false),
                    cand(VIO, 24, true, false, false)
                ],
                64
            ),
            Placement {
                device_abi: SIM,
                reason: PlacementReason::MinMigration,
                migrated_bytes: 24,
            }
        );

        // 3. Tie on resident bytes → deterministic lowest device id (SIM < VIO).
        assert_eq!(
            decide(
                &[
                    cpu(0),
                    cand(SIM, 32, true, false, false),
                    cand(VIO, 32, true, false, false)
                ],
                64
            )
            .device_abi,
            SIM
        );

        // 4. Operands host-resident, an accelerator covers the op but holds nothing →
        //    stay on the host (do NOT thrash to the empty accelerator).
        assert_eq!(
            decide(&[cpu(64), cand(SIM, 0, true, false, false)], 64),
            Placement {
                device_abi: CPU,
                reason: PlacementReason::CpuFallback,
                migrated_bytes: 0,
            }
        );

        // 5. Operands on a covering accelerator but the principal is over quota there
        //    → spill to the host fallback, reason over-quota.
        assert_eq!(
            decide(&[cpu(0), cand(SIM, 64, true, true, false)], 64),
            Placement {
                device_abi: CPU,
                reason: PlacementReason::OverQuota,
                migrated_bytes: 64,
            }
        );

        // 6. The only accelerator holding operands does not cover the op → host
        //    fallback (plain cpu-fallback, not over-quota).
        assert_eq!(
            decide(&[cpu(0), cand(SIM, 64, false, false, false)], 64),
            Placement {
                device_abi: CPU,
                reason: PlacementReason::CpuFallback,
                migrated_bytes: 64,
            }
        );
    }

    /// Phase 7 BUILD-1 (ISC-5): host unit tests for the pure accounting-table
    /// arithmetic — increment/decrement/attribution edge cases incl. the
    /// system-principal (KERNEL_PID) path and saturating underflow.
    #[test]
    fn accel_accounting_table_arithmetic() {
        use super::accounting_table::{AccelUsage, AccountingTable};
        const SIM: u32 = 0x8000_0001; // Device::Accel(1).as_abi()
        const CPU: u32 = 0; // Device::Cpu.as_abi()
        const KERNEL: u32 = 0; // KERNEL_PID (the system principal) — never special-cased
        const P7: u32 = 7;
        const P8: u32 = 8;

        let mut t = AccountingTable::new();
        assert!(t.is_empty());

        // Residency add, distinct principals on the same device → distinct keys.
        t.add_resident(SIM, P7, 16);
        t.add_resident(SIM, P8, 32);
        assert_eq!(t.usage(SIM, P7).resident_bytes, 16);
        assert_eq!(t.usage(SIM, P8).resident_bytes, 32);
        assert_eq!(t.len(), 2);

        // Dispatch count + monotonic time accumulation.
        t.record_dispatch(SIM, P7, 3);
        t.record_dispatch(SIM, P7, 2);
        assert_eq!(t.usage(SIM, P7).dispatch_count, 2);
        assert_eq!(t.usage(SIM, P7).dispatch_time_ticks, 5);

        // Residency sub balances to 0; over-subtract saturates (never wraps).
        t.sub_resident(SIM, P7, 16);
        assert_eq!(t.usage(SIM, P7).resident_bytes, 0);
        t.sub_resident(SIM, P7, 99);
        assert_eq!(t.usage(SIM, P7).resident_bytes, 0);

        // Same principal, different device → distinct key; SIM unaffected.
        t.add_resident(CPU, P7, 8);
        assert_eq!(t.usage(CPU, P7).resident_bytes, 8);
        assert_eq!(t.usage(SIM, P7).resident_bytes, 0);

        // System-principal (KERNEL_PID = 0) is just another key — no panic, no special case.
        t.record_dispatch(SIM, KERNEL, 1);
        assert_eq!(t.usage(SIM, KERNEL).dispatch_count, 1);

        // Unknown key → all-zero default, no panic.
        assert_eq!(t.usage(0xdead, 0xbeef), AccelUsage::default());

        // forget_principal drops a principal's entries across all devices; others remain.
        t.forget_principal(P7);
        assert_eq!(t.usage(SIM, P7), AccelUsage::default());
        assert_eq!(t.usage(CPU, P7), AccelUsage::default());
        assert_eq!(t.usage(SIM, P8).resident_bytes, 32);
        assert_eq!(t.usage(SIM, KERNEL).dispatch_count, 1);
    }

    /// Phase 7 BUILD-P2 (ISC-2): host unit tests for the pure LRU eviction
    /// policy — no-eviction, single-LRU-victim, multi-victim, pinned-protected,
    /// and no-room-fallback, plus the deterministic equal-tick tiebreak.
    #[test]
    fn accel_evict_policy_decisions() {
        use super::evict_policy::{EvictCandidate, EvictReason, select_victims};
        let cand = |tensor_id, bytes, last_use_tick, evictable| EvictCandidate {
            tensor_id,
            bytes,
            last_use_tick,
            evictable,
        };

        // No eviction: resident + need fits — no victims, even with candidates.
        let set = [cand(1, 64, 1, true), cand(2, 64, 2, true)];
        assert_eq!(
            select_victims(256, 128, 64, &set),
            (vec![], EvictReason::NoEviction)
        );

        // Single LRU victim: the lowest-tick unpinned candidate, and only it.
        // resident 192 + need 128 > 256 → shortfall 64; tensor 1 (tick 5) is
        // older than tensor 2 (tick 9) and suffices.
        let set = [
            cand(1, 64, 5, true),
            cand(2, 64, 9, true),
            cand(3, 64, 7, false),
        ];
        assert_eq!(
            select_victims(256, 192, 128, &set),
            (vec![1], EvictReason::Lru)
        );

        // Multi-victim, ascending tick order, stop as soon as room is reached:
        // shortfall 128 → the two coldest (ticks 3 then 6), never the hottest.
        let set = [
            cand(10, 64, 6, true),
            cand(11, 64, 3, true),
            cand(12, 64, 8, true),
        ];
        assert_eq!(
            select_victims(256, 192, 192, &set),
            (vec![11, 10], EvictReason::Lru)
        );

        // Pinned protection: the LRU tensor is pinned (evictable=false) — the
        // policy must skip it and take the next-coldest, never the pinned one.
        let set = [cand(20, 64, 1, false), cand(21, 64, 4, true)];
        assert_eq!(
            select_victims(256, 192, 128, &set),
            (vec![21], EvictReason::Lru)
        );

        // No room: evicting every evictable tensor still cannot fit `need` —
        // CPU fallback with ZERO victims (no partial eviction, never thrash).
        let set = [cand(30, 64, 1, true), cand(31, 64, 2, false)];
        assert_eq!(
            select_victims(256, 192, 256, &set),
            (vec![], EvictReason::NoRoomCpuFallback)
        );

        // Need alone exceeds capacity → fallback regardless of candidates.
        assert_eq!(
            select_victims(256, 0, 512, &[cand(40, 256, 1, true)]),
            (vec![], EvictReason::NoRoomCpuFallback)
        );

        // Deterministic tiebreak: equal ticks → lower tensor_id first.
        let set = [cand(52, 64, 7, true), cand(51, 64, 7, true)];
        assert_eq!(
            select_victims(256, 192, 128, &set),
            (vec![51], EvictReason::Lru)
        );

        // Never-used (tick 0 via the glue's `unwrap_or(0)`) is coldest of all.
        let set = [cand(60, 64, 0, true), cand(61, 64, 1, true)];
        assert_eq!(
            select_victims(256, 192, 128, &set),
            (vec![60], EvictReason::Lru)
        );
    }

    /// Phase 7 BUILD-P3 (ISC-1): host unit tests for the pure structural reuse
    /// predictor — operand-of-next-op is hot at W=1, beyond-W is not, W=0 is
    /// empty, and the future-use set unions every remaining op.
    #[test]
    fn accel_reuse_predictor() {
        use super::predictor::{future_use_values, hot_values};

        // Remaining ops after the one being placed: op+1 consumes {1},
        // op+2 consumes {1, 4}, op+3 consumes {7}.
        let remaining: [&[u32]; 3] = [&[1], &[1, 4], &[7]];

        // W=1: only the next op's operands are hot.
        let hot = hot_values(&remaining, 1);
        assert!(hot.contains(&1));
        assert!(!hot.contains(&4)); // used at op+2 — beyond the window
        assert!(!hot.contains(&7));

        // W=2 widens the window.
        let hot = hot_values(&remaining, 2);
        assert!(hot.contains(&1) && hot.contains(&4));
        assert!(!hot.contains(&7));

        // W=0: the reactive path — nothing is predicted-hot.
        assert!(hot_values(&remaining, 0).is_empty());

        // W beyond the remaining sequence saturates harmlessly.
        let hot = hot_values(&remaining, 99);
        assert!(hot.contains(&7));

        // Future-use is the union over all remaining ops; absent values
        // (e.g. an output value not yet produced, or a dead intermediate)
        // are simply not in the set.
        let future = future_use_values(&remaining);
        assert!(future.contains(&1) && future.contains(&4) && future.contains(&7));
        assert!(!future.contains(&0) && !future.contains(&2));

        // Empty remaining sequence (the last op): nothing hot, nothing used.
        assert!(hot_values(&[], 1).is_empty());
        assert!(future_use_values(&[]).is_empty());
    }

    /// Phase 7 BUILD-P3 (ISC-2): host unit tests for the retain-aware eviction
    /// decision and the proactive cold-demote selector.
    #[test]
    fn accel_residency_policy_decisions() {
        use super::evict_policy::{
            EvictReason, ResidencyCandidate, ResidencyReason, decide_eviction, select_demotion,
            select_victims,
        };
        let cand = |tensor_id, bytes, tick, evictable, hot, demotable| ResidencyCandidate {
            tensor_id,
            bytes,
            last_use_tick: tick,
            evictable,
            predicted_hot: hot,
            demote_eligible: demotable,
        };

        // Retain-hot: the LRU-coldest evictable (id 1, tick 2) is predicted-hot;
        // the colder-by-policy victim is the non-hot id 2 (tick 5). The hot
        // tensor is reported retained and the reason is RetainHot.
        let set = [
            cand(1, 64, 2, true, true, false),
            cand(2, 64, 5, true, false, false),
        ];
        let d = decide_eviction(256, 192, 128, &set);
        assert_eq!(d.victims, vec![2]);
        assert_eq!(d.retained, vec![(1, 64)]);
        assert_eq!(d.reason, ResidencyReason::RetainHot);

        // Plain LRU when retention changes nothing: the chosen victim is
        // already older than every hot tensor — nothing was saved.
        let set = [
            cand(1, 64, 9, true, true, false),
            cand(2, 64, 5, true, false, false),
        ];
        let d = decide_eviction(256, 192, 128, &set);
        assert_eq!(d.victims, vec![2]);
        assert!(d.retained.is_empty());
        assert_eq!(d.reason, ResidencyReason::Lru);

        // Fall-through: retaining every hot tensor leaves no way to make room
        // → §9.10-exact LRU among ALL evictable (hot included), no retention.
        let set = [
            cand(1, 64, 2, true, true, false),
            cand(2, 64, 5, true, true, false),
        ];
        let d = decide_eviction(256, 192, 128, &set);
        assert_eq!(d.victims, vec![1]); // coldest overall
        assert!(d.retained.is_empty());
        assert_eq!(d.reason, ResidencyReason::Lru);

        // No room even falling through → CPU fallback with zero victims.
        let set = [cand(1, 16, 2, true, true, false)];
        let d = decide_eviction(256, 250, 128, &set);
        assert!(d.victims.is_empty());
        assert_eq!(d.reason, ResidencyReason::NoRoomCpuFallback);

        // Strict superset: with no prediction flags set, decide_eviction
        // reproduces select_victims exactly across the P2 case matrix.
        let p2 = |id, bytes, tick, evictable| cand(id, bytes, tick, evictable, false, false);
        let matrices: [&[ResidencyCandidate]; 4] = [
            &[p2(1, 64, 1, true), p2(2, 64, 2, true)],
            &[p2(1, 64, 5, true), p2(2, 64, 9, true), p2(3, 64, 7, false)],
            &[
                p2(10, 64, 6, true),
                p2(11, 64, 3, true),
                p2(12, 64, 8, true),
            ],
            &[p2(30, 64, 1, true), p2(31, 64, 2, false)],
        ];
        let cases = [
            (256u64, 128u64, 64u64),
            (256, 192, 128),
            (256, 192, 192),
            (256, 192, 256),
        ];
        for (candidates, (capacity, resident, need)) in matrices.iter().zip(cases) {
            let legacy: Vec<super::evict_policy::EvictCandidate> = candidates
                .iter()
                .map(|c| super::evict_policy::EvictCandidate {
                    tensor_id: c.tensor_id,
                    bytes: c.bytes,
                    last_use_tick: c.last_use_tick,
                    evictable: c.evictable,
                })
                .collect();
            let (victims, reason) = select_victims(capacity, resident, need, &legacy);
            let d = decide_eviction(capacity, resident, need, candidates);
            assert_eq!(d.victims, victims);
            assert!(d.retained.is_empty());
            let expected = match reason {
                EvictReason::NoEviction => ResidencyReason::NoEviction,
                EvictReason::Lru => ResidencyReason::Lru,
                EvictReason::NoRoomCpuFallback => ResidencyReason::NoRoomCpuFallback,
            };
            assert_eq!(d.reason, expected);
        }

        // Cold-demote: below the threshold → None, regardless of candidates.
        let set = [cand(1, 64, 1, true, false, true)];
        assert_eq!(select_demotion(256, 100, 64, &set), None);

        // Above the threshold → the coldest eligible tensor, exactly one.
        let set = [
            cand(1, 64, 4, true, false, true),
            cand(2, 64, 2, true, false, true),
        ];
        assert_eq!(select_demotion(256, 200, 64, &set), Some(2));

        // Never a predicted-hot tensor; never an ineligible (future-used or
        // output) tensor — even when they are the coldest.
        let set = [
            cand(1, 64, 1, true, true, true),   // hot — protected
            cand(2, 64, 2, true, false, false), // future-used/output — protected
            cand(3, 64, 9, true, false, true),  // the only safe candidate
        ];
        assert_eq!(select_demotion(256, 200, 64, &set), Some(3));

        // Thrash guard: above threshold but nothing safely demotable →
        // refuse (None), never demote something the pass still needs.
        let set = [
            cand(1, 64, 1, true, true, true),
            cand(2, 64, 2, true, false, false),
            cand(3, 64, 3, false, false, true), // externally held
        ];
        assert_eq!(select_demotion(256, 200, 64, &set), None);
    }

    /// Phase 7 BUILD-P4a (ISC-6): host unit tests for the pure claim-decision
    /// truth table - the S9.10 evictability gate plus contention, contention
    /// checked first (a reserved tensor's counts belong to its owner).
    #[test]
    fn accel_reservation_claim_decisions() {
        use super::reservation_logic::{ClaimDecision, decide_claim};

        // Unreferenced + unreserved -> claimable.
        assert_eq!(decide_claim(false, 1, 0), ClaimDecision::Admit);

        // Any strong reference beyond the container's own -> referenced.
        assert_eq!(decide_claim(false, 2, 0), ClaimDecision::SkippedReferenced);
        assert_eq!(decide_claim(false, 7, 0), ClaimDecision::SkippedReferenced);

        // Any Weak -> referenced: a Weak::upgrade could mint a reference
        // without the container lock (the dedup-registry case), so a
        // Weak-bearing tensor is never claimable.
        assert_eq!(decide_claim(false, 1, 1), ClaimDecision::SkippedReferenced);

        // Reserved wins over everything - checked first, never blocked.
        assert_eq!(decide_claim(true, 1, 0), ClaimDecision::SkippedContended);
        assert_eq!(decide_claim(true, 2, 3), ClaimDecision::SkippedContended);
    }
    /// Phase 7 BUILD-P4b (ISC-2): host unit tests for the watermark hysteresis
    /// and the prefetch thrash-guard - no single-byte oscillation, the band
    /// holds state, the degenerate tiny-arena case documented.
    #[test]
    fn accel_daemon_watermarks() {
        use super::watermark::{WatermarkState, next_state, prefetch_admissible, watermarks};
        use WatermarkState::{Reclaiming, Sleeping};

        // Realistic band: C=1000, H=100 -> low=100, high=200.
        assert_eq!(watermarks(1000, 100), (100, 200));
        // Wake strictly below low; hold at low.
        assert_eq!(next_state(Sleeping, 99, 100, 200), Reclaiming);
        assert_eq!(next_state(Sleeping, 100, 100, 200), Sleeping);
        // Sleep at-or-above high; hold below it.
        assert_eq!(next_state(Reclaiming, 200, 100, 200), Sleeping);
        assert_eq!(next_state(Reclaiming, 199, 100, 200), Reclaiming);
        // The anti-oscillation property: byte-by-byte sweep through the band
        // never flips either state.
        for free in 100..200 {
            assert_eq!(next_state(Sleeping, free, 100, 200), Sleeping);
            assert_eq!(next_state(Reclaiming, free, 100, 200), Reclaiming);
        }

        // Degenerate tiny arena (accel_sim shape): 2H >= C -> high == C, the
        // daemon drains everything cold before sleeping. low never exceeds C.
        assert_eq!(watermarks(262144, 147456), (147456, 262144));
        assert_eq!(watermarks(100, 400), (100, 100));
        // Untouched device: no placement has noted a need -> never reclaim.
        assert_eq!(watermarks(262144, 0), (0, 0));

        // BUILD-C1, the decaying-H kill-switch (Rule 1): decay DISABLED is the
        // identity - an arbitrary interleaving of latches and decay steps
        // reproduces the original monotonic latch bit-for-bit.
        use super::watermark::{decay_step, latch};
        let needs = [147456u64, 1024, 999999, 0, 32768, 147456];
        let mut latched = 0u64;
        let mut switched = 0u64;
        for need in needs {
            latched = latch(latched, need);
            // the production composition: decay-then-latch per note —
            // disabled, the decay is the identity and this IS the latch
            switched = latch(decay_step(switched, false), need);
            assert_eq!(switched, latched);
        }
        // Decay ENABLED (demand-driven: one step per note): a steady workload
        // holds its band; an outlier erodes toward the recent max over notes.
        let mut hint = 999_999u64; // the outlier latched once
        for _ in 0..40 {
            hint = latch(decay_step(hint, true), 147_456); // steady demand
        }
        assert_eq!(hint, 147_456); // the latch hazard, fixed by traffic itself
        // And absent demand the raw step is strictly decreasing toward zero.
        let mut hint = 147456u64;
        let once = decay_step(hint, true);
        assert!(once < hint && once == 147456 - 147456 / 8);
        for _ in 0..200 {
            hint = decay_step(hint, true);
        }
        assert!(hint < 1024); // decays away without demand
        assert_eq!(latch(hint, 147456), 147456); // demand restores immediately

        // Prefetch thrash guard: admitted only when free stays >= low after.
        assert!(prefetch_admissible(300, 100, 200));
        assert!(!prefetch_admissible(299, 100, 200)); // would dip below low
        assert!(!prefetch_admissible(50, 100, 200)); // cannot even fit
        assert!(prefetch_admissible(200, 0, 200)); // zero-byte edge holds
    }
    /// Phase 7 BUILD-P2 (ISC-1): host unit tests for the pure recency table —
    /// strictly increasing ticks, re-touch moves to MRU, forget removes,
    /// per-device reads, boundedness.
    #[test]
    fn accel_recency_table_ticks() {
        use super::recency_table::RecencyTable;
        const SIM: u32 = 0x8000_0001;
        const GPU: u32 = 0x8000_0003;

        let mut t = RecencyTable::new();
        assert!(t.is_empty());

        // Strictly increasing ticks across touches.
        let t1 = t.touch(SIM, 100);
        let t2 = t.touch(SIM, 101);
        let t3 = t.touch(SIM, 102);
        assert!(t1 < t2 && t2 < t3);
        assert_eq!(t.tick_of(SIM, 100), Some(t1));

        // Re-touch updates to a strictly newer tick (LRU → MRU).
        let t4 = t.touch(SIM, 100);
        assert!(t4 > t3);
        assert_eq!(t.tick_of(SIM, 100), Some(t4));
        assert_eq!(t.len(), 3); // re-touch is an update, not a new entry

        // A tensor lives on exactly one device: touching it on another device
        // moves the entry — the old device no longer reports it.
        let t5 = t.touch(GPU, 101);
        assert_eq!(t.tick_of(GPU, 101), Some(t5));
        assert_eq!(t.tick_of(SIM, 101), None);

        // Unknown tensor / wrong device → None (the glue treats it as coldest).
        assert_eq!(t.tick_of(SIM, 999), None);

        // Forget removes the entry; idempotent; others unaffected.
        t.forget(100);
        t.forget(100);
        assert_eq!(t.tick_of(SIM, 100), None);
        assert_eq!(t.tick_of(SIM, 102), Some(t3));
        assert_eq!(t.len(), 2);
    }

    /// Phase 7 BUILD-P2 (ISC-19): per-device residency totals + the high-water
    /// mark — the eviction policy's `R` and the gate's `R ≤ C always` proof —
    /// stay in lockstep with the per-principal §9.5 entries.
    #[test]
    fn accel_accounting_device_residency_peak() {
        use super::accounting_table::AccountingTable;
        const SIM: u32 = 0x8000_0001;
        const CPU: u32 = 0x8000_0000;
        const P7: u32 = 7;
        const P8: u32 = 8;

        let mut t = AccountingTable::new();
        assert_eq!(t.device_residency(SIM).current, 0);
        assert_eq!(t.device_residency(SIM).peak, 0);

        // Totals aggregate across principals; peak tracks the high water.
        t.add_resident(SIM, P7, 100);
        t.add_resident(SIM, P8, 50);
        assert_eq!(t.device_residency(SIM).current, 150);
        assert_eq!(t.device_residency(SIM).peak, 150);

        // Sub lowers current; peak holds.
        t.sub_resident(SIM, P7, 60);
        assert_eq!(t.device_residency(SIM).current, 90);
        assert_eq!(t.device_residency(SIM).peak, 150);

        // Climb past the old peak → peak advances.
        t.add_resident(SIM, P7, 120);
        assert_eq!(t.device_residency(SIM).current, 210);
        assert_eq!(t.device_residency(SIM).peak, 210);

        // Lockstep rule: a sub for a never-seen (device, principal) moves nothing.
        t.sub_resident(SIM, 99, 1000);
        assert_eq!(t.device_residency(SIM).current, 210);

        // Lockstep rule under over-subtract: a sub larger than the principal's
        // holding sheds only what that principal actually held — the device
        // aggregate stays equal to the per-principal sum (P7's 160 remains),
        // never undercounting (an undercounted `R` would let a room check
        // place onto a full device).
        t.sub_resident(SIM, P8, 9999);
        assert_eq!(t.device_residency(SIM).current, 160); // P7's bytes intact
        assert_eq!(t.device_residency(SIM).peak, 210);
        t.add_resident(SIM, P8, 50); // restore for the forget_principal check

        // Devices are independent.
        t.add_resident(CPU, P7, 10);
        assert_eq!(t.device_residency(CPU).current, 10);
        assert_eq!(t.device_residency(CPU).peak, 10);
        assert_eq!(t.device_residency(SIM).current, 210);

        // forget_principal sheds that principal's bytes from current — never peak.
        t.forget_principal(P7);
        assert_eq!(t.device_residency(SIM).current, 50); // P8's bytes remain
        assert_eq!(t.device_residency(SIM).peak, 210);
        assert_eq!(t.device_residency(CPU).current, 0);
    }

    #[test]
    fn soft_f32_known_bit_patterns() {
        assert_eq!(soft_f32::add(0x3fc0_0000, 0x4010_0000), 0x4070_0000);
        assert_eq!(soft_f32::mul(0x3fc0_0000, 0x4010_0000), 0x4058_0000);
        assert_eq!(soft_f32::add(0xc000_0000, 0x3f80_0000), 0xbf80_0000);
        assert_eq!(soft_f32::relu(0xbf80_0000), 0);
        assert_eq!(soft_f32::relu(0x4000_0000), 0x4000_0000);
    }

    #[test]
    fn matmul_2x2_matches_hand_oracle() {
        let lhs_bits = [0x3f80_0000, 0x4000_0000, 0x4040_0000, 0x4080_0000];
        let rhs_bits = [0x40a0_0000, 0x40c0_0000, 0x40e0_0000, 0x4100_0000];
        let expected = [0x4198_0000, 0x41b0_0000, 0x422c_0000, 0x4248_0000];
        let lhs = bytes_from_bits(&lhs_bits);
        let rhs = bytes_from_bits(&rhs_bits);
        let mut out = vec![0u8; expected.len() * 4];
        let lhs_shape = shape2(2, 2);
        let rhs_shape = shape2(2, 2);

        ops::matmul(&view(&lhs_shape, &lhs), &view(&rhs_shape, &rhs), &mut out).unwrap();

        assert_eq!(bits_from_bytes(&out), expected);
    }

    #[test]
    fn dispatch_matmul_matches_hand_oracle() {
        let lhs_bits = [0x3f80_0000, 0x4000_0000];
        let rhs_bits = [0x4040_0000, 0x4080_0000, 0x40a0_0000, 0x40c0_0000];
        let expected = [0x4150_0000, 0x4180_0000];
        let lhs = bytes_from_bits(&lhs_bits);
        let rhs = bytes_from_bits(&rhs_bits);
        let mut out = vec![0u8; expected.len() * 4];
        let lhs_shape = shape2(1, 2);
        let rhs_shape = shape2(2, 2);
        let output_shape = shape2(1, 2);
        let inputs = [view(&lhs_shape, &lhs), view(&rhs_shape, &rhs)];

        ops::dispatch(OpKind::Matmul, &inputs, &output_shape, &mut out).unwrap();

        assert_eq!(bits_from_bytes(&out), expected);
    }

    /// Phase 8 BUILD-8.4a ISC-1: SiLU / Swish, pinned against the raw-source
    /// formula `x / (1 + exp(-x))` from ggml-cpu/vec.h:1046-1047. Expected
    /// values are host-computed constants, independent of `ops::silu`.
    #[test]
    fn silu_isc1_matches_hand_reference_kat() {
        let input = bytes_from_f32(&[-3.0, -1.0, 0.0, 1.0, 3.0]);
        let expected = [-0.14227761, -0.26894143, 0.0, 0.7310586, 2.8577223];
        let shape = shape1(expected.len() as u64);
        let mut direct = vec![0u8; expected.len() * 4];
        let mut dispatched = vec![0u8; expected.len() * 4];

        ops::silu(&view(&shape, &input), &mut direct).unwrap();
        ops::dispatch(
            OpKind::Silu,
            &[view(&shape, &input)],
            &shape,
            &mut dispatched,
        )
        .unwrap();

        assert_f32_bytes_close("silu direct", &direct, &expected, REL_EPS);
        assert_eq!(direct, dispatched);
    }

    /// Phase 8 BUILD-8.4a ISC-2: SwiGLU activation
    /// `silu(gate) * up`, matching ggml_vec_swiglu_f32's scalar tail. This is
    /// the activation only; the FFN matmuls stay out of this build.
    #[test]
    fn swiglu_isc2_matches_hand_reference_kat() {
        let gate = bytes_from_f32(&[-3.0, -1.0, 0.0, 1.0, 3.0]);
        let up = bytes_from_f32(&[0.5, -2.0, 4.0, -0.25, 0.125]);
        let expected = [-0.07113881, 0.53788286, 0.0, -0.18276465, 0.3572153];
        let shape = shape1(expected.len() as u64);
        let mut direct = vec![0u8; expected.len() * 4];
        let mut dispatched = vec![0u8; expected.len() * 4];

        ops::swiglu(&view(&shape, &gate), &view(&shape, &up), &mut direct).unwrap();
        ops::dispatch(
            OpKind::Swiglu,
            &[view(&shape, &gate), view(&shape, &up)],
            &shape,
            &mut dispatched,
        )
        .unwrap();

        assert_f32_bytes_close("swiglu direct", &direct, &expected, REL_EPS);
        assert_eq!(direct, dispatched);
    }

    /// Phase 8 BUILD-8.4a ISC-3: Qwen2.5-style RoPE, `theta=1e6`, NEOX
    /// split-half pair layout. Raw-source convention: ggml's NEOX path calls
    /// `rotate_pairs(n_dims, n_dims/2, ...)`, pairing dim 0 with n_dims/2, not
    /// adjacent dims. Expected values are fixed from the formula.
    #[test]
    fn rope_isc3_qwen_neox_split_half_theta_1e6_kat() {
        let input = bytes_from_f32(&[0.25, -0.5, 0.75, -1.0, 1.25, -1.5, 1.75, -2.0]);
        let expected = [
            -0.42389813,
            -0.35566255,
            0.7447466,
            -0.9998103,
            -1.2022107,
            -1.5406181,
            1.7522421,
            -2.000095,
        ];
        let shape = shape1(expected.len() as u64);
        let params = ops::RopeYarnParams {
            position: 3,
            n_dims: 8,
            theta_scale: f32_to_bits(1_000_000.0_f32.powf(-2.0 / 8.0)),
            freq_scale: f32_to_bits(1.0),
            ext_factor: f32_to_bits(0.0),
            attn_factor: f32_to_bits(1.0),
            corr_dim_low: f32_to_bits(0.0),
            corr_dim_high: f32_to_bits(7.0),
        };
        let mut direct = vec![0u8; expected.len() * 4];
        let mut dispatched = vec![0u8; expected.len() * 4];
        let param_shape = shape1(ops::ROPE_YARN_PARAM_COUNT as u64);
        let param_bytes = bytes_from_f32(&[
            3.0,
            8.0,
            1_000_000.0_f32.powf(-2.0 / 8.0),
            1.0,
            0.0,
            1.0,
            0.0,
            7.0,
        ]);

        ops::rope_neox_yarn(&view(&shape, &input), params, &mut direct).unwrap();
        ops::dispatch(
            OpKind::Rope,
            &[view(&shape, &input), view(&param_shape, &param_bytes)],
            &shape,
            &mut dispatched,
        )
        .unwrap();

        assert_f32_bytes_close("rope qwen split-half", &direct, &expected, ROPE_REL_EPS);
        assert_eq!(direct, dispatched);
    }

    /// Phase 8 BUILD-8.4a ISC-3: nonzero YaRN ramp/scale path. This pins the
    /// ggml rope_yarn mix: theta interpolates/extrapolates by the correction
    /// ramp and mscale includes `1 + 0.1 * ln(1 / freq_scale)`.
    #[test]
    fn rope_isc3_yarn_scaling_ramp_kat() {
        let input = bytes_from_f32(&[1.0, 2.0, 3.0, 4.0, -1.0, -2.0, -3.0, -4.0]);
        let expected = [
            -1.5082108,
            1.0546664,
            4.533876,
            5.2025,
            -0.1103093,
            2.834634,
            -0.16055796,
            -3.0860784,
        ];
        let shape = shape1(expected.len() as u64);
        let params = ops::RopeYarnParams {
            position: 4,
            n_dims: 8,
            theta_scale: f32_to_bits(0.5),
            freq_scale: f32_to_bits(0.5),
            ext_factor: f32_to_bits(1.0),
            attn_factor: f32_to_bits(1.0),
            corr_dim_low: f32_to_bits(1.0),
            corr_dim_high: f32_to_bits(3.0),
        };
        let mut out = vec![0u8; expected.len() * 4];

        ops::rope_neox_yarn(&view(&shape, &input), params, &mut out).unwrap();

        assert_f32_bytes_close("rope yarn", &out, &expected, ROPE_REL_EPS);
    }

    /// Phase 8 BUILD-8.4 ISC-3: the decoder-layer scaffold runs the two
    /// supported halves and leaves attention as an explicit unsupported seam
    /// for BUILD-8.5.
    #[test]
    fn forward_isc3_layer_scaffold_stops_at_attention_seam() {
        let config = Qwen2LayerConfig {
            hidden_size: 4,
            intermediate_size: 3,
            q_heads: 2,
            kv_heads: 1,
            head_dim: 2,
            rope_dimension_count: 2,
            attn_q_projection_size: 4,
            rope_theta_scale_bits: f32_to_bits(0.5),
            rms_norm_eps_bits: f32_to_bits(RMS_NORM_EPS),
        };
        let input = [0.25, -0.5, 0.75, -1.0];
        let attn_norm = [1.0, 1.0, 1.0, 1.0];
        let attn_q = [
            0.25, -0.5, 0.75, -1.0, //
            -0.125, 0.25, -0.375, 0.5, //
            0.6, 0.4, 0.2, 0.0, //
            0.1, -0.2, 0.3, -0.4,
        ];
        let attn_k = [
            0.5, -0.25, //
            -0.625, 0.75, //
            -0.2, -0.4, //
            0.5, -0.6,
        ];
        let attn_v = [
            1.0, -0.75, //
            -0.875, 1.0, //
            -0.6, -0.8, //
            0.7, -0.8,
        ];
        let attn_o = [
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ];
        let ffn_norm = [1.0, 1.0, 1.0, 1.0];
        let ffn_gate = [
            0.25, -0.25, 0.5, //
            -0.5, 0.75, -0.75, //
            1.0, -1.0, 1.25, //
            -1.25, 1.5, -1.5,
        ];
        let ffn_up = [
            0.5, 0.25, -0.5, //
            -0.25, 0.75, 0.5, //
            1.0, -0.75, 0.25, //
            -1.0, 1.25, -1.25,
        ];
        let ffn_down = [
            0.25, -0.5, 0.75, -1.0, //
            -0.125, 0.25, -0.375, 0.5, //
            0.6, -0.4, 0.2, -0.8,
        ];
        let weights = Qwen2LayerWeights {
            attn_norm: &attn_norm,
            attn_q: &attn_q,
            attn_k: &attn_k,
            attn_v: &attn_v,
            attn_q_bias: None,
            attn_k_bias: None,
            attn_v_bias: None,
            attn_o: &attn_o,
            ffn_norm: &ffn_norm,
            ffn_gate: &ffn_gate,
            ffn_up: &ffn_up,
            ffn_down: &ffn_down,
            dtypes: Qwen2LayerDtypes::F32,
            quantized: Qwen2QuantizedLayerWeights::NONE,
        };

        let attention = forward_logic::attention_seam(&input, weights, config, 2).unwrap();
        assert_eq!(attention.q.len(), 4);
        assert_eq!(attention.k.len(), 2);
        assert_eq!(attention.v.len(), 2);
        assert_eq!(attention.q_rope.len(), 4);
        assert_eq!(attention.k_rope.len(), 2);

        let unsupported =
            forward_logic::forward_layer_with_attention_output(&input, weights, config, 2, None)
                .unwrap_err();
        assert_eq!(unsupported, ForwardLayerError::AttentionUnsupported);

        let layer = forward_logic::forward_layer_with_attention_output(
            &input,
            weights,
            config,
            2,
            Some(&[0.1, -0.2, 0.3, -0.4]),
        )
        .unwrap();
        assert_eq!(layer.ffn_input.len(), 4);
        assert_eq!(layer.ffn.swiglu.len(), 3);
        assert_eq!(layer.ffn.ffn_out.len(), 4);
        assert_eq!(layer.ffn.residual.len(), 4);
    }

    /// Phase 8 BUILD-8.5 ISC-1/5: the forward layer consumes the GQA context,
    /// applies the attention output projection, and the cache kill-switch path
    /// matches the cached path for the same one-token prefix.
    #[test]
    fn ot16_walker_forward_is_byte_identical_to_hardcoded_loop() {
        // KS2 — the load-bearing kill-switch proof, EXECUTED (B1a). Walk a Qwen2
        // schedule through the REAL dispatch — `crate::model_logic::walk_qwen2_schedule`,
        // the exact function the ring-0 walker (qwen_inventory) calls — with
        // DISTINCT per-layer weights, and require BYTE-identity to the hardcoded
        // 0..N loop. The distinct weights make it non-tautological: a
        // layer_idx/order mis-map in the walk would apply the wrong layer's
        // weights and FAIL (unlike identical weights, where order is invisible —
        // the prior tautology the AAG caught). The reversed-order guard proves the
        // weights are genuinely distinct, so the byte-identity is a real proof.
        struct Layer {
            attn_norm: Vec<f32>,
            attn_q: Vec<f32>,
            attn_k: Vec<f32>,
            attn_v: Vec<f32>,
            attn_o: Vec<f32>,
            ffn_norm: Vec<f32>,
            ffn_gate: Vec<f32>,
            ffn_up: Vec<f32>,
            ffn_down: Vec<f32>,
        }
        impl Layer {
            fn view(&self) -> Qwen2LayerWeights<'_> {
                Qwen2LayerWeights {
                    attn_norm: &self.attn_norm,
                    attn_q: &self.attn_q,
                    attn_k: &self.attn_k,
                    attn_v: &self.attn_v,
                    attn_q_bias: None,
                    attn_k_bias: None,
                    attn_v_bias: None,
                    attn_o: &self.attn_o,
                    ffn_norm: &self.ffn_norm,
                    ffn_gate: &self.ffn_gate,
                    ffn_up: &self.ffn_up,
                    ffn_down: &self.ffn_down,
                    dtypes: Qwen2LayerDtypes::F32,
                    quantized: Qwen2QuantizedLayerWeights::NONE,
                }
            }
        }
        // Base weights scaled by a DISTINCT factor per layer → each layer is a
        // different transform, so composition order is observable.
        fn scaled(scale: f32) -> Layer {
            let s = |v: &[f32]| v.iter().map(|x| x * scale).collect::<Vec<f32>>();
            Layer {
                attn_norm: s(&[1.0, 1.0, 1.0, 1.0]),
                attn_q: s(&[
                    0.25, -0.5, 0.75, -1.0, -0.125, 0.25, -0.375, 0.5, 0.6, 0.4, 0.2, 0.0, 0.1,
                    -0.2, 0.3, -0.4,
                ]),
                attn_k: s(&[0.5, -0.25, -0.625, 0.75, -0.2, -0.4, 0.5, -0.6]),
                attn_v: s(&[1.0, -0.75, -0.875, 1.0, -0.6, -0.8, 0.7, -0.8]),
                attn_o: s(&[
                    0.25, -0.5, 0.75, -1.0, -0.125, 0.25, -0.375, 0.5, 0.6, -0.4, 0.2, -0.8, 0.5,
                    0.25, -0.5, -0.25,
                ]),
                ffn_norm: s(&[1.0, 1.0, 1.0, 1.0]),
                ffn_gate: s(&[
                    0.25, -0.25, 0.5, -0.5, 0.75, -0.75, 1.0, -1.0, 1.25, -1.25, 1.5, -1.5,
                ]),
                ffn_up: s(&[
                    0.5, 0.25, -0.5, -0.25, 0.75, 0.5, 1.0, -0.75, 0.25, -1.0, 1.25, -1.25,
                ]),
                ffn_down: s(&[
                    0.25, -0.5, 0.75, -1.0, -0.125, 0.25, -0.375, 0.5, 0.6, -0.4, 0.2, -0.8,
                ]),
            }
        }

        let config = Qwen2LayerConfig {
            hidden_size: 4,
            intermediate_size: 3,
            q_heads: 2,
            kv_heads: 1,
            head_dim: 2,
            rope_dimension_count: 2,
            attn_q_projection_size: 4,
            rope_theta_scale_bits: f32_to_bits(0.5),
            rms_norm_eps_bits: f32_to_bits(RMS_NORM_EPS),
        };
        let input = [0.25f32, -0.5, 0.75, -1.0];
        const MAX_CTX: usize = 4;
        // DISTINCT per-layer scales.
        let layers = [scaled(1.0), scaled(0.5), scaled(1.5)];
        let n = layers.len();

        // The hardcoded 0..N loop over a given layer ORDER.
        fn run_hardcoded(
            layers: &[Layer],
            config: Qwen2LayerConfig,
            input: &[f32],
            order: &[usize],
            max_ctx: usize,
        ) -> Vec<f32> {
            let mut hidden = input.to_vec();
            let mut caches: Vec<Qwen2KvCacheLayer> = (0..order.len())
                .map(|_| Qwen2KvCacheLayer::new(config, max_ctx).unwrap())
                .collect();
            for (slot, &li) in order.iter().enumerate() {
                let out = forward_logic::forward_layer_with_gqa_attention(
                    &hidden,
                    layers[li].view(),
                    config,
                    0,
                    true,
                    Some(&mut caches[slot]),
                    None,
                )
                .unwrap();
                hidden = out.ffn.residual;
            }
            hidden
        }

        // Path A: hardcoded loop, forward order 0..n.
        let forward_order: Vec<usize> = (0..n).collect();
        let hidden_hardcoded = run_hardcoded(&layers, config, &input, &forward_order, MAX_CTX);

        // Path B: the REAL walk dispatch (kernel path) over a homogeneous Qwen2
        // schedule, applying layers[layer_idx] for each layer_idx.
        let schedule = crate::model_logic::LayerSchedule {
            version: 1,
            layers: (0..n)
                .map(|_| crate::model_logic::LayerEntry {
                    kind: crate::model_logic::LayerKind::Qwen2AttentionFFN,
                    config_ref: 0,
                })
                .collect(),
        };
        let mut caches_w: Vec<Qwen2KvCacheLayer> = (0..n)
            .map(|_| Qwen2KvCacheLayer::new(config, MAX_CTX).unwrap())
            .collect();
        let hidden_walker = crate::model_logic::walk_qwen2_schedule(
            &schedule,
            input.to_vec(),
            |layer_idx, h| {
                let out = forward_logic::forward_layer_with_gqa_attention(
                    h,
                    layers[layer_idx].view(),
                    config,
                    0,
                    true,
                    Some(&mut caches_w[layer_idx]),
                    None,
                )
                .map_err(|_| ())?;
                Ok(out.ffn.residual)
            },
            |_layer_idx, _config_ref, _h| Err(()),
        )
        .unwrap();

        // (1) Real walk == hardcoded loop, BYTE-identical (exact bits, not epsilon).
        assert_eq!(hidden_hardcoded.len(), hidden_walker.len());
        for (idx, (a, b)) in hidden_hardcoded
            .iter()
            .zip(hidden_walker.iter())
            .enumerate()
        {
            assert_eq!(
                a.to_bits(),
                b.to_bits(),
                "KS2: walker output byte {idx} differs from the hardcoded loop"
            );
        }

        // (2) Discriminating guard: the per-layer weights are genuinely distinct,
        // so ORDER matters — a reversed order yields DIFFERENT output. This is what
        // makes (1) a real proof rather than f(x)==f(x): had the walk mis-mapped
        // layer_idx, (1) would have failed.
        let reversed_order: Vec<usize> = (0..n).rev().collect();
        let hidden_reversed = run_hardcoded(&layers, config, &input, &reversed_order, MAX_CTX);
        let differs = hidden_hardcoded
            .iter()
            .zip(hidden_reversed.iter())
            .any(|(a, b)| a.to_bits() != b.to_bits());
        assert!(
            differs,
            "non-discriminating: per-layer weights must be distinct so a mis-map would fail"
        );
    }

    #[test]
    fn ot16_gdn_walker_executes_gdn_and_carries_recurrent_state() {
        let config = ops::gdn::GdnLayerConfig {
            hidden_size: 4,
            num_k_heads: 1,
            num_v_heads: 1,
            head_k_dim: 2,
            head_v_dim: 2,
            conv_kernel_size: 2,
            rms_norm_eps: 1.0e-6,
        };
        let in_proj_qkv = [
            1.0, 0.0, 0.5, 0.0, 1.0, 0.0, //
            0.0, 1.0, 0.0, 0.5, 0.0, 1.0, //
            0.25, 0.0, -0.25, 0.0, 0.5, 0.0, //
            0.0, -0.25, 0.0, 0.25, 0.0, 0.5,
        ];
        let in_proj_z = [
            0.5, 0.0, //
            0.0, 0.5, //
            0.25, 0.0, //
            0.0, 0.25,
        ];
        let in_proj_b = [0.25, -0.25, 0.125, 0.375];
        let in_proj_a = [0.0, 0.0, 0.0, 0.0];
        let conv1d_weight = [
            0.25, 1.0, //
            0.25, 1.0, //
            0.25, 1.0, //
            0.25, 1.0, //
            0.25, 1.0, //
            0.25, 1.0,
        ];
        let a_log = [0.0];
        let dt_bias = [0.0];
        let norm_weight = [1.0, 1.0];
        let out_proj = [
            0.5, 0.0, 0.25, 0.0, //
            0.0, 0.5, 0.0, 0.25,
        ];
        let weights = ops::gdn::GdnLayerWeights {
            in_proj_qkv: &in_proj_qkv,
            in_proj_z: &in_proj_z,
            in_proj_b: &in_proj_b,
            in_proj_a: &in_proj_a,
            conv1d_weight: &conv1d_weight,
            a_log: &a_log,
            dt_bias: &dt_bias,
            norm_weight: &norm_weight,
            out_proj: &out_proj,
        };
        let projection_input = [0.5, -0.25, 0.75, -1.0];
        let mut projected_recurrent_state = vec![0.0f32; config.state_size()];
        let mut projected_conv_state = vec![0.0f32; config.conv_state_size().unwrap()];
        let projected = ops::gdn::gdn_layer_projected(
            &projection_input,
            weights,
            config,
            &mut projected_recurrent_state,
            &mut projected_conv_state,
        )
        .unwrap();
        let qkv = soft_matvec(
            &projection_input,
            config.hidden_size,
            &in_proj_qkv,
            config.conv_dim(),
        );
        let z = soft_matvec(
            &projection_input,
            config.hidden_size,
            &in_proj_z,
            config.value_dim(),
        );
        let beta = soft_matvec(
            &projection_input,
            config.hidden_size,
            &in_proj_b,
            config.num_v_heads,
        );
        let alpha = soft_matvec(
            &projection_input,
            config.hidden_size,
            &in_proj_a,
            config.num_v_heads,
        );
        let mut core_recurrent_state = vec![0.0f32; config.state_size()];
        let mut core_conv_state = vec![0.0f32; config.conv_state_size().unwrap()];
        let core = ops::gdn::gdn_layer_core_projected(
            ops::gdn::GdnProjectedInputs {
                qkv: &qkv,
                z: &z,
                beta_raw: &beta,
                a_raw: &alpha,
            },
            weights,
            config,
            &mut core_recurrent_state,
            &mut core_conv_state,
        )
        .unwrap();
        let core_projected = soft_matvec(&core, config.value_dim(), &out_proj, config.hidden_size);
        assert_eq!(
            projected
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            core_projected
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            "storage-independent GDN core must match the f32 wrapper"
        );
        let mut forward_recurrent_state = vec![0.0f32; config.state_size()];
        let mut forward_conv_state = vec![0.0f32; config.conv_state_size().unwrap()];
        let forward = ops::gdn::gdn_layer_forward(
            &projection_input,
            weights,
            config,
            &mut forward_recurrent_state,
            &mut forward_conv_state,
        )
        .unwrap();
        let expected_forward = projection_input
            .iter()
            .zip(projected.iter())
            .map(|(&hidden, &projected)| {
                f32::from_bits(ops::soft_f32::add(hidden.to_bits(), projected.to_bits()))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            forward.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            expected_forward
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            "residual GDN forward must remain hidden + projection"
        );
        assert_eq!(
            projected_recurrent_state
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            forward_recurrent_state
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            "projection-only and residual GDN paths must update recurrent state identically"
        );
        assert_eq!(
            projected_conv_state
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            forward_conv_state
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            "projection-only and residual GDN paths must update conv state identically"
        );
        let schedule = crate::model_logic::LayerSchedule {
            version: 1,
            layers: vec![
                crate::model_logic::LayerEntry {
                    kind: crate::model_logic::LayerKind::Qwen2AttentionFFN,
                    config_ref: 0,
                },
                crate::model_logic::LayerEntry {
                    kind: crate::model_logic::LayerKind::GatedDeltaNetLinear,
                    config_ref: 7,
                },
            ],
        };

        struct WalkCtx<'a> {
            schedule: &'a crate::model_logic::LayerSchedule,
            weights: ops::gdn::GdnLayerWeights<'a>,
            config: ops::gdn::GdnLayerConfig,
            recurrent_state: &'a mut [f32],
            conv_state: &'a mut [f32],
            qwen_calls: &'a mut usize,
            gdn_calls: &'a mut usize,
        }

        fn walk_one(ctx: WalkCtx<'_>, input: Vec<f32>) -> Vec<f32> {
            let WalkCtx {
                schedule,
                weights,
                config,
                recurrent_state,
                conv_state,
                qwen_calls,
                gdn_calls,
            } = ctx;
            crate::model_logic::walk_qwen2_schedule(
                schedule,
                input,
                |layer_idx, h| {
                    *qwen_calls += 1;
                    assert_eq!(layer_idx, 0);
                    Ok::<Vec<f32>, ()>(h.iter().map(|v| v + 0.125).collect())
                },
                |layer_idx, config_ref, h| {
                    *gdn_calls += 1;
                    assert_eq!(layer_idx, 1);
                    assert_eq!(config_ref, 7);
                    ops::gdn::gdn_layer_forward(h, weights, config, recurrent_state, conv_state)
                        .map_err(|_| ())
                },
            )
            .unwrap()
        }

        let mut recurrent_state = vec![0.0f32; config.state_size()];
        let mut conv_state = vec![0.0f32; config.conv_state_size().unwrap()];
        let mut qwen_calls = 0usize;
        let mut gdn_calls = 0usize;

        let _out0 = walk_one(
            WalkCtx {
                schedule: &schedule,
                weights,
                config,
                recurrent_state: &mut recurrent_state,
                conv_state: &mut conv_state,
                qwen_calls: &mut qwen_calls,
                gdn_calls: &mut gdn_calls,
            },
            vec![0.5, -0.25, 0.75, -1.0],
        );
        let state_after0 = recurrent_state.clone();
        assert!(
            state_after0.iter().any(|v| v.to_bits() != 0),
            "token 0 must update recurrent state"
        );

        let out1_carried = walk_one(
            WalkCtx {
                schedule: &schedule,
                weights,
                config,
                recurrent_state: &mut recurrent_state,
                conv_state: &mut conv_state,
                qwen_calls: &mut qwen_calls,
                gdn_calls: &mut gdn_calls,
            },
            vec![-0.125, 0.625, -0.5, 0.25],
        );
        assert_ne!(
            state_after0.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            recurrent_state
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            "token 1 must carry and then mutate token 0 state"
        );

        let mut fresh_recurrent_state = vec![0.0f32; config.state_size()];
        let mut fresh_conv_state = vec![0.0f32; config.conv_state_size().unwrap()];
        let out1_fresh = walk_one(
            WalkCtx {
                schedule: &schedule,
                weights,
                config,
                recurrent_state: &mut fresh_recurrent_state,
                conv_state: &mut fresh_conv_state,
                qwen_calls: &mut qwen_calls,
                gdn_calls: &mut gdn_calls,
            },
            vec![-0.125, 0.625, -0.5, 0.25],
        );
        assert!(
            out1_carried
                .iter()
                .zip(out1_fresh.iter())
                .any(|(a, b)| a.to_bits() != b.to_bits()),
            "token 1 output must observe carried recurrent/conv state"
        );
        assert_eq!(qwen_calls, 3);
        assert_eq!(gdn_calls, 3);
    }

    #[test]
    fn ot16_gdn_layer_repeats_k_heads_for_grouped_value_heads() {
        let config = ops::gdn::GdnLayerConfig {
            hidden_size: 2,
            num_k_heads: 1,
            num_v_heads: 2,
            head_k_dim: 1,
            head_v_dim: 1,
            conv_kernel_size: 2,
            rms_norm_eps: 1.0e-6,
        };
        let hidden = [1.0f32, 0.5f32];
        let in_proj_qkv = [
            0.25, -0.5, 0.75, -1.0, //
            0.5, 0.25, -0.25, 0.5,
        ];
        let in_proj_z = [
            0.5, -0.25, //
            -0.75, 0.5,
        ];
        let in_proj_b = [
            0.25, -0.5, //
            -0.25, 0.75,
        ];
        let in_proj_a = [
            0.0, 0.25, //
            0.125, -0.125,
        ];
        let conv1d_weight = [
            0.0, 1.0, //
            0.0, 1.0, //
            0.0, 1.0, //
            0.0, 1.0,
        ];
        let a_log = [0.0, 0.125];
        let dt_bias = [0.0, -0.25];
        let norm_weight = [1.25];
        let out_proj = [
            1.0, 0.0, //
            0.0, 1.0,
        ];
        let weights = ops::gdn::GdnLayerWeights {
            in_proj_qkv: &in_proj_qkv,
            in_proj_z: &in_proj_z,
            in_proj_b: &in_proj_b,
            in_proj_a: &in_proj_a,
            conv1d_weight: &conv1d_weight,
            a_log: &a_log,
            dt_bias: &dt_bias,
            norm_weight: &norm_weight,
            out_proj: &out_proj,
        };

        let mut recurrent_state = vec![0.0f32; config.state_size()];
        let mut conv_state = vec![0.0f32; config.conv_state_size().unwrap()];
        let actual = ops::gdn::gdn_layer_forward(
            &hidden,
            weights,
            config,
            &mut recurrent_state,
            &mut conv_state,
        )
        .expect("grouped-head GDN layer should execute");

        let qkv = soft_matvec(&hidden, config.hidden_size, &in_proj_qkv, config.conv_dim());
        let mut reference_conv_state = vec![0.0f32; config.conv_state_size().unwrap()];
        let mut conv_out = vec![0.0f32; config.conv_dim()];
        ops::gdn::gdn_causal_conv1d_update(
            &qkv,
            &mut reference_conv_state,
            &conv1d_weight,
            &mut conv_out,
        )
        .unwrap();

        let mut q_norm = [0.0f32; 1];
        let mut k_norm = [0.0f32; 1];
        ops::gdn::gdn_l2norm(&conv_out[0..1], &mut q_norm, config.rms_norm_eps).unwrap();
        ops::gdn::gdn_l2norm(&conv_out[1..2], &mut k_norm, config.rms_norm_eps).unwrap();
        let q_scale = f32::from_bits(ops::soft_f32::div(
            ops::soft_f32::int_to_f32(1),
            ops::soft_f32::sqrt(ops::soft_f32::int_to_f32(config.head_k_dim as i32)),
        ));
        let q = [q_norm[0] * q_scale, q_norm[0] * q_scale];
        let k = [k_norm[0], k_norm[0]];
        let v = [conv_out[2], conv_out[3]];

        let beta_raw = soft_matvec(&hidden, config.hidden_size, &in_proj_b, config.num_v_heads);
        let a_raw = soft_matvec(&hidden, config.hidden_size, &in_proj_a, config.num_v_heads);
        let mut beta = vec![0.0f32; config.num_v_heads];
        let mut gate = vec![0.0f32; config.num_v_heads];
        for head in 0..config.num_v_heads {
            beta[head] = sigmoid_via_soft(beta_raw[head]);
            let a_plus_dt = f32::from_bits(ops::soft_f32::add(
                a_raw[head].to_bits(),
                dt_bias[head].to_bits(),
            ));
            let a_softplus = softplus_via_soft(a_plus_dt);
            gate[head] = f32::from_bits(ops::soft_f32::exp(ops::soft_f32::mul(
                a_softplus.to_bits(),
                a_log[head].to_bits(),
            )));
        }

        let mut reference_recurrent_state = vec![0.0f32; config.state_size()];
        let mut recurrent_out = vec![0.0f32; config.value_dim()];
        ops::gdn::gdn_recurrent_step(
            &q,
            &k,
            &v,
            &gate,
            &beta,
            &mut reference_recurrent_state,
            &mut recurrent_out,
        )
        .unwrap();

        let z = soft_matvec(&hidden, config.hidden_size, &in_proj_z, config.value_dim());
        let mut normed = vec![0.0f32; config.value_dim()];
        for head in 0..config.num_v_heads {
            ops::gdn::gdn_rmsnorm_gated(
                &recurrent_out[head..head + 1],
                &norm_weight,
                &z[head..head + 1],
                config.rms_norm_eps,
                &mut normed[head..head + 1],
            )
            .unwrap();
        }
        let projected = soft_matvec(&normed, config.value_dim(), &out_proj, config.hidden_size);
        let expected = [
            f32::from_bits(ops::soft_f32::add(
                hidden[0].to_bits(),
                projected[0].to_bits(),
            )),
            f32::from_bits(ops::soft_f32::add(
                hidden[1].to_bits(),
                projected[1].to_bits(),
            )),
        ];

        assert_eq!(
            actual.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            "grouped-head GDN output must repeat K heads and norm per value head"
        );
        assert_eq!(
            recurrent_state
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            reference_recurrent_state
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            "grouped-head recurrent state must match the decomposed reference"
        );
        assert_ne!(
            recurrent_state[0].to_bits(),
            recurrent_state[1].to_bits(),
            "both value heads must use the repeated K head but keep distinct V state"
        );
    }

    #[test]
    fn forward_isc1_gqa_cache_fills_attention_seam_kat() {
        let config = Qwen2LayerConfig {
            hidden_size: 4,
            intermediate_size: 3,
            q_heads: 2,
            kv_heads: 1,
            head_dim: 2,
            rope_dimension_count: 2,
            attn_q_projection_size: 4,
            rope_theta_scale_bits: f32_to_bits(0.5),
            rms_norm_eps_bits: f32_to_bits(RMS_NORM_EPS),
        };
        let input = [0.25, -0.5, 0.75, -1.0];
        let attn_norm = [1.0, 1.0, 1.0, 1.0];
        let attn_q = [
            0.25, -0.5, 0.75, -1.0, //
            -0.125, 0.25, -0.375, 0.5, //
            0.6, 0.4, 0.2, 0.0, //
            0.1, -0.2, 0.3, -0.4,
        ];
        let attn_k = [
            0.5, -0.25, //
            -0.625, 0.75, //
            -0.2, -0.4, //
            0.5, -0.6,
        ];
        let attn_v = [
            1.0, -0.75, //
            -0.875, 1.0, //
            -0.6, -0.8, //
            0.7, -0.8,
        ];
        let attn_o = [
            0.25, -0.5, 0.75, -1.0, //
            -0.125, 0.25, -0.375, 0.5, //
            0.6, -0.4, 0.2, -0.8, //
            0.5, 0.25, -0.5, -0.25,
        ];
        let ffn_norm = [1.0, 1.0, 1.0, 1.0];
        let ffn_gate = [
            0.25, -0.25, 0.5, //
            -0.5, 0.75, -0.75, //
            1.0, -1.0, 1.25, //
            -1.25, 1.5, -1.5,
        ];
        let ffn_up = [
            0.5, 0.25, -0.5, //
            -0.25, 0.75, 0.5, //
            1.0, -0.75, 0.25, //
            -1.0, 1.25, -1.25,
        ];
        let ffn_down = [
            0.25, -0.5, 0.75, -1.0, //
            -0.125, 0.25, -0.375, 0.5, //
            0.6, -0.4, 0.2, -0.8,
        ];
        let weights = Qwen2LayerWeights {
            attn_norm: &attn_norm,
            attn_q: &attn_q,
            attn_k: &attn_k,
            attn_v: &attn_v,
            attn_q_bias: None,
            attn_k_bias: None,
            attn_v_bias: None,
            attn_o: &attn_o,
            ffn_norm: &ffn_norm,
            ffn_gate: &ffn_gate,
            ffn_up: &ffn_up,
            ffn_down: &ffn_down,
            dtypes: Qwen2LayerDtypes::F32,
            quantized: Qwen2QuantizedLayerWeights::NONE,
        };
        let seam = forward_logic::attention_seam(&input, weights, config, 0).unwrap();
        let uncached = forward_logic::Qwen2UncachedAttention {
            q_rope_tokens: &seam.q_rope,
            k_rope_tokens: &seam.k_rope,
            value_tokens: &seam.v,
            seq_len: 1,
        };

        let mut cache = Qwen2KvCacheLayer::new(config, 4).unwrap();
        let cached = forward_logic::forward_layer_with_gqa_attention(
            &input,
            weights,
            config,
            0,
            true,
            Some(&mut cache),
            None,
        )
        .unwrap();
        let disabled = forward_logic::forward_layer_with_gqa_attention(
            &input,
            weights,
            config,
            0,
            false,
            None,
            Some(uncached),
        )
        .unwrap();

        assert_eq!(cache.used_len(), 1);
        assert_f32_slice_close(
            "forward gqa cache kill switch ffn input",
            &cached.ffn_input,
            &disabled.ffn_input,
            0.0,
        );
        assert_f32_slice_close(
            "forward gqa cache kill switch residual",
            &cached.ffn.residual,
            &disabled.ffn.residual,
            0.0,
        );
    }

    #[test]
    fn qwen35_attention_seam_splits_gate_and_norms_qk_kat() {
        let config = Qwen2LayerConfig {
            hidden_size: 2,
            intermediate_size: 3,
            q_heads: 2,
            kv_heads: 1,
            head_dim: 2,
            rope_dimension_count: 2,
            attn_q_projection_size: 8,
            rope_theta_scale_bits: f32_to_bits(0.5),
            rms_norm_eps_bits: f32_to_bits(RMS_NORM_EPS),
        };
        assert_eq!(config.q_len(), 4);
        assert!(config.has_attention_gate());

        let input = [0.5, -0.25];
        let attn_norm = [1.25, 0.75];
        let attn_q = [
            0.2, -0.4, 0.6, -0.8, 1.0, -1.2, 1.4, -1.6, //
            -0.1, 0.3, -0.5, 0.7, -0.9, 1.1, -1.3, 1.5,
        ];
        let attn_k = [
            0.25, -0.5, //
            0.75, -1.0,
        ];
        let attn_v = [
            -0.3, 0.4, //
            0.5, -0.6,
        ];
        let attn_q_bias = [0.01, -0.02, 0.03, -0.04, 0.05, -0.06, 0.07, -0.08];
        let attn_k_bias = [0.125, -0.25];
        let attn_v_bias = [-0.0625, 0.03125];
        let q_norm = [1.5, 0.75];
        let k_norm = [0.5, 1.25];
        let weights = Qwen2LayerWeights {
            attn_norm: &attn_norm,
            attn_q: &attn_q,
            attn_k: &attn_k,
            attn_v: &attn_v,
            attn_q_bias: Some(&attn_q_bias),
            attn_k_bias: Some(&attn_k_bias),
            attn_v_bias: Some(&attn_v_bias),
            attn_o: &[],
            ffn_norm: &[],
            ffn_gate: &[],
            ffn_up: &[],
            ffn_down: &[],
            dtypes: Qwen2LayerDtypes::F32,
            quantized: Qwen2QuantizedLayerWeights::NONE,
        };

        let got = forward_logic::attention_seam_with_norms(
            &input,
            weights,
            config,
            0,
            QwenAttentionNormWeights {
                q_norm: Some(&q_norm),
                k_norm: Some(&k_norm),
            },
        )
        .expect("Qwen3.5 gated attention seam should execute");

        let ref_norm =
            reference_rms_norm(&input, &attn_norm, bits_to_f32(config.rms_norm_eps_bits));
        let ref_q_full = reference_add(
            &reference_matmul(
                &ref_norm,
                1,
                config.hidden_size,
                &attn_q,
                config.attn_q_projection_size,
            ),
            &attn_q_bias,
        );
        let mut ref_q_pre_norm = vec![0.0f32; config.q_len()];
        let mut ref_gate = vec![0.0f32; config.q_len()];
        for head in 0..config.q_heads {
            let src = head * config.head_dim * 2;
            let dst = head * config.head_dim;
            ref_q_pre_norm[dst..dst + config.head_dim]
                .copy_from_slice(&ref_q_full[src..src + config.head_dim]);
            ref_gate[dst..dst + config.head_dim]
                .copy_from_slice(&ref_q_full[src + config.head_dim..src + config.head_dim * 2]);
        }
        let ref_q = reference_rms_norm_heads(
            &ref_q_pre_norm,
            &q_norm,
            config.q_heads,
            config.head_dim,
            bits_to_f32(config.rms_norm_eps_bits),
        );
        let ref_k_raw = reference_add(
            &reference_matmul(&ref_norm, 1, config.hidden_size, &attn_k, config.kv_len()),
            &attn_k_bias,
        );
        let ref_k = reference_rms_norm_heads(
            &ref_k_raw,
            &k_norm,
            config.kv_heads,
            config.head_dim,
            bits_to_f32(config.rms_norm_eps_bits),
        );
        let ref_v = reference_add(
            &reference_matmul(&ref_norm, 1, config.hidden_size, &attn_v, config.kv_len()),
            &attn_v_bias,
        );
        let ref_q_rope = reference_rope_neox(
            &ref_q,
            config.q_heads,
            config.head_dim,
            config.rope_dimension_count,
            0,
            bits_to_f32(config.rope_theta_scale_bits),
        );
        let ref_k_rope = reference_rope_neox(
            &ref_k,
            config.kv_heads,
            config.head_dim,
            config.rope_dimension_count,
            0,
            bits_to_f32(config.rope_theta_scale_bits),
        );

        assert_eq!(got.q.len(), config.q_len());
        assert_eq!(got.gate.as_ref().unwrap().len(), config.q_len());
        assert_eq!(got.k.len(), config.kv_len());
        assert_eq!(got.v.len(), config.kv_len());
        assert_f32_slice_close("qwen35 projected q", &got.q, &ref_q, REL_EPS);
        assert_f32_slice_close(
            "qwen35 attention gate",
            got.gate.as_deref().unwrap(),
            &ref_gate,
            REL_EPS,
        );
        assert_f32_slice_close("qwen35 projected k", &got.k, &ref_k, REL_EPS);
        assert_f32_slice_close("qwen35 projected v", &got.v, &ref_v, REL_EPS);
        assert_f32_slice_close("qwen35 q rope", &got.q_rope, &ref_q_rope, REL_EPS);
        assert_f32_slice_close("qwen35 k rope", &got.k_rope, &ref_k_rope, REL_EPS);
    }

    #[test]
    fn qwen35_attention_partial_rope_leaves_head_tail_unchanged_kat() {
        let config = Qwen2LayerConfig {
            hidden_size: 4,
            intermediate_size: 3,
            q_heads: 1,
            kv_heads: 1,
            head_dim: 4,
            rope_dimension_count: 2,
            attn_q_projection_size: 8,
            rope_theta_scale_bits: f32_to_bits(0.5),
            rms_norm_eps_bits: f32_to_bits(RMS_NORM_EPS),
        };
        let input = [0.5, -0.25, 0.75, -1.0];
        let attn_norm = [1.0, 0.75, 1.25, 0.5];
        let attn_q: Vec<f32> = (0..config.hidden_size * config.attn_q_projection_size)
            .map(|idx| (idx as f32 - 11.0) * 0.03125)
            .collect();
        let attn_k: Vec<f32> = (0..config.hidden_size * config.kv_len())
            .map(|idx| (7.0 - idx as f32) * 0.0625)
            .collect();
        let attn_v: Vec<f32> = (0..config.hidden_size * config.kv_len())
            .map(|idx| (idx as f32 - 3.0) * 0.046875)
            .collect();
        let q_norm = [1.0, 0.875, 1.125, 0.75];
        let k_norm = [0.75, 1.25, 0.625, 1.5];
        let weights = Qwen2LayerWeights {
            attn_norm: &attn_norm,
            attn_q: &attn_q,
            attn_k: &attn_k,
            attn_v: &attn_v,
            attn_q_bias: None,
            attn_k_bias: None,
            attn_v_bias: None,
            attn_o: &[],
            ffn_norm: &[],
            ffn_gate: &[],
            ffn_up: &[],
            ffn_down: &[],
            dtypes: Qwen2LayerDtypes::F32,
            quantized: Qwen2QuantizedLayerWeights::NONE,
        };

        let got = forward_logic::attention_seam_with_norms(
            &input,
            weights,
            config,
            3,
            QwenAttentionNormWeights {
                q_norm: Some(&q_norm),
                k_norm: Some(&k_norm),
            },
        )
        .expect("partial-RoPE Qwen3.5 attention seam");

        let expected_q = reference_rope_neox(
            &got.q,
            config.q_heads,
            config.head_dim,
            config.rope_dimension_count,
            3,
            bits_to_f32(config.rope_theta_scale_bits),
        );
        let expected_k = reference_rope_neox(
            &got.k,
            config.kv_heads,
            config.head_dim,
            config.rope_dimension_count,
            3,
            bits_to_f32(config.rope_theta_scale_bits),
        );
        assert_f32_slice_close(
            "qwen35 partial q rope",
            &got.q_rope,
            &expected_q,
            ROPE_REL_EPS,
        );
        assert_f32_slice_close(
            "qwen35 partial k rope",
            &got.k_rope,
            &expected_k,
            ROPE_REL_EPS,
        );
        assert_eq!(&got.q_rope[2..4], &got.q[2..4]);
        assert_eq!(&got.k_rope[2..4], &got.k[2..4]);
        assert_ne!(got.q_rope[0].to_bits(), got.q[0].to_bits());
        assert_ne!(got.k_rope[0].to_bits(), got.k[0].to_bits());
    }

    /// Phase 8 BUILD-8.4 ISC-1: optional real-model FFN oracle. Set
    /// NEO_QWEN25_GGUF to Qwen2.5-0.5B-Q4_K_M. The implementation path uses
    /// kernel GGUF dequant + existing matmul/SwiGLU; the oracle uses the
    /// raw-source llama.cpp Q4_K transcription already used by BUILD-8.3 plus
    /// native f32 matmul/SwiGLU over the same real tensor bytes.
    #[test]
    fn forward_isc1_real_qwen_swiglu_ffn_matches_llama_reference_when_available() {
        let Some((bytes, parsed)) = read_real_qwen_gguf_or_skip(
            "forward_isc1_real_qwen_swiglu_ffn_matches_llama_reference_when_available",
        ) else {
            return;
        };
        let config = Qwen2LayerConfig::qwen25_05b();
        let input = qwen_fixed_hidden_input();

        assert_real_tensor(&parsed, "blk.11.ffn_norm.weight", &[896]);
        assert_real_tensor(&parsed, "blk.11.ffn_gate.weight", &[896, 4864]);
        assert_real_tensor(&parsed, "blk.11.ffn_up.weight", &[896, 4864]);
        assert_real_tensor(&parsed, "blk.11.ffn_down.weight", &[4864, 896]);

        let ffn_norm = real_tensor_to_f32(&parsed, &bytes, "blk.11.ffn_norm.weight");
        let gate = real_tensor_to_f32(&parsed, &bytes, "blk.11.ffn_gate.weight");
        let up = real_tensor_to_f32(&parsed, &bytes, "blk.11.ffn_up.weight");
        let down = real_tensor_to_f32(&parsed, &bytes, "blk.11.ffn_down.weight");
        let ref_gate = real_qwen_quant_reference(&parsed, &bytes, "blk.11.ffn_gate.weight");
        let ref_up = real_qwen_quant_reference(&parsed, &bytes, "blk.11.ffn_up.weight");
        let ref_down = real_qwen_quant_reference(&parsed, &bytes, "blk.11.ffn_down.weight");
        assert_f32_slice_close("real qwen ffn_gate dequant", &gate, &ref_gate, 0.0);
        assert_f32_slice_close("real qwen ffn_up dequant", &up, &ref_up, 0.0);
        assert_f32_slice_close("real qwen ffn_down dequant", &down, &ref_down, 0.0);

        let weights = Qwen2LayerWeights {
            attn_norm: &[],
            attn_q: &[],
            attn_k: &[],
            attn_v: &[],
            attn_q_bias: None,
            attn_k_bias: None,
            attn_v_bias: None,
            attn_o: &[],
            ffn_norm: &ffn_norm,
            ffn_gate: &gate,
            ffn_up: &up,
            ffn_down: &down,
            dtypes: Qwen2LayerDtypes::F32,
            quantized: Qwen2QuantizedLayerWeights::NONE,
        };
        let got = forward_logic::swiglu_ffn(&input, weights, config).unwrap();

        let ref_norm = reference_rms_norm(&input, &ffn_norm, bits_to_f32(config.rms_norm_eps_bits));
        let ref_gate_out = reference_matmul(
            &ref_norm,
            1,
            config.hidden_size,
            &ref_gate,
            config.intermediate_size,
        );
        let ref_up_out = reference_matmul(
            &ref_norm,
            1,
            config.hidden_size,
            &ref_up,
            config.intermediate_size,
        );
        let ref_swiglu = reference_swiglu(&ref_gate_out, &ref_up_out);
        let ref_ffn_out = reference_matmul(
            &ref_swiglu,
            1,
            config.intermediate_size,
            &ref_down,
            config.hidden_size,
        );
        let ref_residual = reference_add(&input, &ref_ffn_out);

        assert_f32_slice_close(
            "real qwen swiglu activation",
            &got.swiglu,
            &ref_swiglu,
            REAL_FORWARD_REL_EPS,
        );
        assert_f32_slice_close(
            "real qwen swiglu ffn out",
            &got.ffn_out,
            &ref_ffn_out,
            REAL_FORWARD_REL_EPS,
        );
        assert_f32_slice_close(
            "real qwen swiglu ffn residual",
            &got.residual,
            &ref_residual,
            REAL_FORWARD_REL_EPS,
        );
    }

    /// Phase 8 BUILD-8.4 ISC-2: optional real-model RoPE oracle over projected
    /// Q/K. This pins the Qwen2.5 GGUF fused-QKV shape and the llama.cpp NEOX
    /// split-half pair convention against real projected values.
    #[test]
    fn forward_isc2_real_qwen_rope_qk_matches_llama_reference_when_available() {
        let Some((bytes, parsed)) = read_real_qwen_gguf_or_skip(
            "forward_isc2_real_qwen_rope_qk_matches_llama_reference_when_available",
        ) else {
            return;
        };
        let config = Qwen2LayerConfig::qwen25_05b();
        let input = qwen_fixed_hidden_input();
        let position = 7;

        assert_real_tensor(&parsed, "blk.0.attn_norm.weight", &[896]);
        assert_real_tensor(&parsed, "blk.0.attn_q.weight", &[896, 896]);
        assert_real_tensor(&parsed, "blk.0.attn_k.weight", &[896, 128]);
        assert_real_tensor(&parsed, "blk.0.attn_v.weight", &[896, 128]);
        assert_real_tensor(&parsed, "blk.0.attn_q.bias", &[896]);
        assert_real_tensor(&parsed, "blk.0.attn_k.bias", &[128]);
        assert_real_tensor(&parsed, "blk.0.attn_v.bias", &[128]);

        let attn_norm = real_tensor_to_f32(&parsed, &bytes, "blk.0.attn_norm.weight");
        let attn_q = real_tensor_to_f32(&parsed, &bytes, "blk.0.attn_q.weight");
        let attn_k = real_tensor_to_f32(&parsed, &bytes, "blk.0.attn_k.weight");
        let attn_v = real_tensor_to_f32(&parsed, &bytes, "blk.0.attn_v.weight");
        let attn_q_bias = real_tensor_to_f32(&parsed, &bytes, "blk.0.attn_q.bias");
        let attn_k_bias = real_tensor_to_f32(&parsed, &bytes, "blk.0.attn_k.bias");
        let attn_v_bias = real_tensor_to_f32(&parsed, &bytes, "blk.0.attn_v.bias");
        let ref_attn_q = real_qwen_quant_reference(&parsed, &bytes, "blk.0.attn_q.weight");
        let ref_attn_k = real_qwen_quant_reference(&parsed, &bytes, "blk.0.attn_k.weight");
        let ref_attn_v = real_qwen_quant_reference(&parsed, &bytes, "blk.0.attn_v.weight");
        assert_f32_slice_close("real qwen attn_q dequant", &attn_q, &ref_attn_q, 0.0);
        assert_f32_slice_close("real qwen attn_k dequant", &attn_k, &ref_attn_k, 0.0);
        assert_f32_slice_close("real qwen attn_v dequant", &attn_v, &ref_attn_v, 0.0);

        let weights = Qwen2LayerWeights {
            attn_norm: &attn_norm,
            attn_q: &attn_q,
            attn_k: &attn_k,
            attn_v: &attn_v,
            attn_q_bias: Some(&attn_q_bias),
            attn_k_bias: Some(&attn_k_bias),
            attn_v_bias: Some(&attn_v_bias),
            attn_o: &[],
            ffn_norm: &[],
            ffn_gate: &[],
            ffn_up: &[],
            ffn_down: &[],
            dtypes: Qwen2LayerDtypes::F32,
            quantized: Qwen2QuantizedLayerWeights::NONE,
        };
        let got = forward_logic::attention_seam(&input, weights, config, position).unwrap();

        let ref_norm =
            reference_rms_norm(&input, &attn_norm, bits_to_f32(config.rms_norm_eps_bits));
        let ref_q = reference_add(
            &reference_matmul(
                &ref_norm,
                1,
                config.hidden_size,
                &ref_attn_q,
                config.q_len(),
            ),
            &attn_q_bias,
        );
        let ref_k = reference_add(
            &reference_matmul(
                &ref_norm,
                1,
                config.hidden_size,
                &ref_attn_k,
                config.kv_len(),
            ),
            &attn_k_bias,
        );
        let ref_q_rope = reference_rope_neox(
            &ref_q,
            config.q_heads,
            config.head_dim,
            config.rope_dimension_count,
            position,
            bits_to_f32(config.rope_theta_scale_bits),
        );
        let ref_k_rope = reference_rope_neox(
            &ref_k,
            config.kv_heads,
            config.head_dim,
            config.rope_dimension_count,
            position,
            bits_to_f32(config.rope_theta_scale_bits),
        );

        assert_f32_slice_close(
            "real qwen projected q",
            &got.q,
            &ref_q,
            REAL_FORWARD_REL_EPS,
        );
        assert_f32_slice_close(
            "real qwen projected k",
            &got.k,
            &ref_k,
            REAL_FORWARD_REL_EPS,
        );
        assert_f32_slice_close(
            "real qwen q rope split-half",
            &got.q_rope,
            &ref_q_rope,
            REAL_FORWARD_REL_EPS,
        );
        assert_f32_slice_close(
            "real qwen k rope split-half",
            &got.k_rope,
            &ref_k_rope,
            REAL_FORWARD_REL_EPS,
        );
    }

    /// Phase 8 BUILD-8.5a ISC-1: single-head causal scaled-dot-product
    /// attention. Scores are `Q * K^T / sqrt(head_dim)`, future keys are absent
    /// from the softmax, and the expected values are fixed host constants.
    #[test]
    fn gqa_attention_isc1_single_head_causal_sdpa_kat() {
        let params = ops::GqaAttentionParams {
            seq_len: 3,
            n_q_heads: 1,
            n_kv_heads: 1,
            head_dim: 4,
        };
        let query = [
            1.0, 0.0, 0.0, 0.0, //
            0.0, 2.0, 0.0, 0.0, //
            1.0, 1.0, 0.0, 0.0,
        ];
        let key = [
            2.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            1.0, 1.0, 0.0, 0.0,
        ];
        let value = [
            1.0, 2.0, 3.0, 4.0, //
            10.0, 20.0, 30.0, 40.0, //
            -1.0, -2.0, -3.0, -4.0,
        ];
        let expected = [
            1.0, 2.0, 3.0, 4.0, //
            7.5795274, 15.159055, 22.73858, 30.31811, //
            2.3269653, 4.6539307, 6.980896, 9.307861,
        ];

        assert_gqa_attention_kat(
            "gqa single-head causal",
            params,
            &query,
            &key,
            &value,
            &expected,
        );
    }

    /// Phase 8 BUILD-8.5a ISC-2: GQA q-to-kv grouping trap. Raw-source
    /// convention from ggml flash attention: `rk2 = q_heads / kv_heads`,
    /// then `kv_head = q_head / rk2`, not modulo.
    #[test]
    fn gqa_attention_isc2_q_to_kv_integer_grouping_kat() {
        let params = ops::GqaAttentionParams {
            seq_len: 1,
            n_q_heads: 4,
            n_kv_heads: 2,
            head_dim: 1,
        };
        let query = [0.0, 0.0, 0.0, 0.0];
        let key = [0.0, 0.0];
        let value = [11.0, 29.0];
        let expected = [11.0, 11.0, 29.0, 29.0];

        assert_gqa_attention_kat("gqa q-kv grouping", params, &query, &key, &value, &expected);
    }

    /// Phase 8 BUILD-8.5a ISC-3: causal mask trap. Equal logits make the
    /// allowed softmax uniform; position 1 must include its diagonal value 20
    /// and must not see the future value 1000.
    #[test]
    fn gqa_attention_isc3_causal_mask_diagonal_kat() {
        let params = ops::GqaAttentionParams {
            seq_len: 3,
            n_q_heads: 1,
            n_kv_heads: 1,
            head_dim: 1,
        };
        let query = [0.0, 0.0, 0.0];
        let key = [0.0, 0.0, 0.0];
        let value = [10.0, 20.0, 1000.0];
        let expected = [10.0, 15.0, 343.33334];

        assert_gqa_attention_kat("gqa causal mask", params, &query, &key, &value, &expected);
    }

    #[test]
    fn gqa_attention_last_matches_generic_output_byte_for_byte() {
        let params = ops::GqaAttentionParams {
            seq_len: 4,
            n_q_heads: 4,
            n_kv_heads: 2,
            head_dim: 8,
        };
        let query = (0..128)
            .map(|idx| ((idx * 17 % 41) as f32 - 20.0) / 13.0)
            .collect::<Vec<_>>();
        let key = (0..64)
            .map(|idx| ((idx * 11 % 37) as f32 - 18.0) / 9.0)
            .collect::<Vec<_>>();
        let value = (0..64)
            .map(|idx| ((idx * 7 % 29) as f32 - 14.0) / 5.0)
            .collect::<Vec<_>>();
        let query_shape = shape3(params.n_q_heads, params.seq_len, params.head_dim);
        let kv_shape = shape3(params.n_kv_heads, params.seq_len, params.head_dim);
        let query_bytes = bytes_from_f32(&query);
        let key_bytes = bytes_from_f32(&key);
        let value_bytes = bytes_from_f32(&value);
        let mut generic = vec![0u8; query_bytes.len()];
        let mut last = vec![0u8; params.n_q_heads as usize * params.head_dim as usize * 4];

        ops::gqa_causal_attention(
            &view(&query_shape, &query_bytes),
            &view(&kv_shape, &key_bytes),
            &view(&kv_shape, &value_bytes),
            params,
            &mut generic,
        )
        .unwrap();
        ops::gqa_causal_attention_last(
            &view(&query_shape, &query_bytes),
            &view(&kv_shape, &key_bytes),
            &view(&kv_shape, &value_bytes),
            params,
            &mut last,
        )
        .unwrap();

        let seq_len = params.seq_len as usize;
        let head_dim = params.head_dim as usize;
        let mut expected = Vec::with_capacity(last.len());
        for head in 0..params.n_q_heads as usize {
            let start = ((head * seq_len + seq_len - 1) * head_dim) * 4;
            expected.extend_from_slice(&generic[start..start + head_dim * 4]);
        }
        assert_eq!(last, expected);
    }

    /// Phase 8 BUILD-8.5 ISC-2: fixed-capacity KV cache append/read. K is
    /// already RoPE-rotated at write time; reads return the raw cached prefix
    /// and never re-apply RoPE. The position assertion traps off-by-one writes.
    #[test]
    fn kv_cache_isc2_prereserved_append_reads_rope_at_write_kat() {
        let config = Qwen2LayerConfig {
            hidden_size: 4,
            intermediate_size: 3,
            q_heads: 2,
            kv_heads: 1,
            head_dim: 2,
            rope_dimension_count: 2,
            attn_q_projection_size: 4,
            rope_theta_scale_bits: f32_to_bits(0.5),
            rms_norm_eps_bits: f32_to_bits(RMS_NORM_EPS),
        };
        let mut cache = Qwen2KvCacheLayer::new(config, 3).unwrap();

        assert_eq!(
            cache
                .append_rope_kv(1, &[99.0, 100.0], &[199.0, 200.0])
                .unwrap_err(),
            ForwardLayerError::CachePosition
        );
        cache
            .append_rope_kv(0, &[1.25, -2.5], &[10.0, 20.0])
            .unwrap();
        cache
            .append_rope_kv(1, &[3.5, -4.75], &[30.0, 40.0])
            .unwrap();

        assert_eq!(cache.used_len(), 2);
        assert_eq!(cache.max_context(), 3);
        assert_eq!(cache.active_key().unwrap(), vec![1.25, -2.5, 3.5, -4.75]);
        assert_eq!(cache.active_value().unwrap(), vec![10.0, 20.0, 30.0, 40.0]);
    }

    /// Phase 8 BUILD-8.5 ISC-4/5: cached decode equals uncached recompute-all
    /// at each generation step, and the `kv_cache_enabled=false` path produces
    /// the same logits without reading or writing the cache.
    #[test]
    fn kv_cache_isc4_cached_equals_uncached_generation_kat() {
        let config = tiny_kv_config();
        let prompt = [1u32];
        let decode = DecodeConfig {
            max_new_tokens: 3,
            eos_token_id: None,
        };

        let mut uncached_trace = Vec::new();
        let mut uncached_rng = Pcg32::seeded(42, 54);
        let uncached = generate_tokens_with(
            &prompt,
            decode,
            SamplingConfig::greedy(),
            &mut uncached_rng,
            |context, logits| {
                let next_logits = tiny_uncached_logits(context, config);
                uncached_trace.push(next_logits.clone());
                *logits = next_logits;
                Ok(())
            },
        )
        .unwrap();

        let mut cached_trace = Vec::new();
        let mut cached_rng = Pcg32::seeded(42, 54);
        let mut cache = Qwen2KvCacheLayer::new(config, 8).unwrap();
        let mut cached_context = Vec::new();
        let cached = generate_tokens_with(
            &prompt,
            decode,
            SamplingConfig::greedy(),
            &mut cached_rng,
            |context, logits| {
                assert_eq!(context.len(), cached_context.len() + 1);
                assert_eq!(&context[..cached_context.len()], cached_context.as_slice());
                let pos = context.len() - 1;
                let token = context[pos];
                cached_context.push(token);
                let next_logits = tiny_cached_logits(token, pos, config, &mut cache);
                cached_trace.push(next_logits.clone());
                *logits = next_logits;
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(uncached, cached);
        assert_eq!(uncached, vec![5, 0, 5]);
        assert_eq!(uncached_trace.len(), cached_trace.len());
        for (idx, (uncached_logits, cached_logits)) in
            uncached_trace.iter().zip(cached_trace.iter()).enumerate()
        {
            assert_f32_slice_close(
                &format!("cached generation logits step {idx}"),
                cached_logits,
                uncached_logits,
                0.0,
            );
        }
    }

    /// Phase 8 BUILD-8.5 ISC-6: real Qwen2.5-0.5B-Q4_K_M one-token greedy
    /// generation through the cached GQA path. The expected token is raw-sourced
    /// from local llama.cpp `5f04dc7` using `/completion` with `n_probs=5`:
    /// prompt token `9707` ("Hello") => top-1 token `271` ("\n\n").
    #[test]
    fn forward_isc6_real_qwen_cached_top1_matches_llama_cpp_when_available() {
        let Some((bytes, parsed)) = read_real_qwen_gguf_or_skip(
            "forward_isc6_real_qwen_cached_top1_matches_llama_cpp_when_available",
        ) else {
            return;
        };
        const PROMPT: [u32; 1] = [REAL_QWEN_HELLO_TOKEN];
        let decode = DecodeConfig {
            max_new_tokens: 1,
            eos_token_id: None,
        };
        let mut rng = Pcg32::seeded(42, 54);

        let generated = generate_tokens_with(
            &PROMPT,
            decode,
            SamplingConfig::greedy(),
            &mut rng,
            |context, logits| {
                assert_eq!(context, PROMPT);
                *logits = real_qwen_cached_logits(&parsed, &bytes, context);
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(generated, vec![REAL_QWEN_LLAMA_CPP_HELLO_TOP1]);

        #[cfg(feature = "phase9-quantized-matmul")]
        {
            let mut f32_rng = Pcg32::seeded(42, 54);
            let f32_generated = generate_tokens_with(
                &PROMPT,
                decode,
                SamplingConfig::greedy(),
                &mut f32_rng,
                |context, logits| {
                    assert_eq!(context, PROMPT);
                    *logits =
                        real_qwen_cached_logits_with_quantized(&parsed, &bytes, context, false);
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(generated, f32_generated);
        }
    }

    /// OT-28 Stage-A rehearsal: optional real-model gate over the imported
    /// 0.5B `.neom`. Set `NEO_REAL_NEOM` to the imported artifact and
    /// `NEO_REAL_LLAMA_TOKENS` to the raw-sourced llama.cpp greedy output token
    /// IDs for `REAL_QWEN_STAGE_A_PROMPT`.
    #[test]
    fn real_model_stage_a_forward_validation_when_available() {
        let Some(model) =
            read_real_neom_or_skip("real_model_stage_a_forward_validation_when_available")
        else {
            return;
        };
        require_phase9_real_quant_feature();
        let prompt = real_neom_stage_a_prompt(&model);
        let expected = expected_llama_tokens_from_env();
        assert!(
            (16..=32).contains(&expected.len()),
            "NEO_REAL_LLAMA_TOKENS must contain 16..=32 generated token IDs, got {}",
            expected.len()
        );

        let unique_counts = log_real_neom_layer_dtype_inventory(&model);
        assert!(
            unique_counts.total_quant() > 0,
            "real .neom is not quantized (all-f32?): {}",
            unique_counts.summary()
        );

        let mut trace = RealForwardTrace::default();
        let report =
            real_neom_greedy_decode(&model, &prompt, expected.len(), &mut trace, Some(&expected));
        assert!(
            trace.dtype_counts.covers(&unique_counts),
            "F1 mixed-quant matmul skipped a quant path present in the model: model={} matmul={}",
            unique_counts.summary(),
            trace.dtype_counts.summary()
        );

        let first_divergence = first_token_divergence(&report.tokens, &expected);
        if let Some(idx) = first_divergence {
            let step = &report.steps[idx];
            eprintln!(
                "REAL_MODEL_STAGE_A prompt={:?} generated={:?} expected={:?} first_divergence={:?} finite=ok deterministic=not-run unique_dtypes={} matmul_dtypes={} latency_ms={} per_token_ms={:.3}",
                prompt,
                report.tokens,
                expected,
                first_divergence,
                unique_counts.summary(),
                trace.dtype_counts.summary(),
                report.elapsed.as_millis(),
                report.elapsed.as_secs_f64() * 1000.0 / report.tokens.len() as f64,
            );
            panic!(
                "real .neom top-1 diverged from llama.cpp at generated index {idx}: got {} expected {}; our_top1_logit={:.6} runner_up={} runner_up_logit={:.6} margin={:.6}; generated={:?} expected={:?}",
                step.token,
                expected[idx],
                step.logit,
                step.runner_up_token,
                step.runner_up_logit,
                step.logit - step.runner_up_logit,
                report.tokens,
                expected
            );
        }

        let mut determinism_trace = RealForwardTrace::default();
        let deterministic = real_neom_greedy_decode(
            &model,
            &prompt,
            expected.len(),
            &mut determinism_trace,
            None,
        );
        assert_eq!(
            report.tokens, deterministic.tokens,
            "real .neom greedy decode is not deterministic"
        );
        eprintln!(
            "REAL_MODEL_STAGE_A prompt={:?} generated={:?} expected={:?} first_divergence={:?} finite=ok deterministic=ok unique_dtypes={} matmul_dtypes={} latency_ms={} per_token_ms={:.3}",
            prompt,
            report.tokens,
            expected,
            first_divergence,
            unique_counts.summary(),
            trace.dtype_counts.summary(),
            report.elapsed.as_millis(),
            report.elapsed.as_secs_f64() * 1000.0 / expected.len() as f64,
        );
    }

    /// OT-30 refinement (PL-ratified): near-tie-aware teacher-forced Stage-A
    /// parity for the 1.5B. Strict free-running greedy top-1 is the wrong
    /// instrument when the reference's own top-1/top-2 are near-equiprobable
    /// (the 1.5B's bare-"Hello" token 0 is a 0.0242-logprob coin-flip between
    /// ` gorgeous`/` beautiful`): one such flip cascades the whole sequence.
    /// This teacher-forces the reference prefix, compares the kernel's argmax
    /// PER POSITION, and tolerates a flip ONLY when the reference itself ranks
    /// the two tokens within `NEAR_TIE_EPS_LOGPROB` (same top-2, swapped).
    /// GROSS divergences (a real forward error) must be zero. Strict matches
    /// always pass — a superset of the old oracle, so the 0.5B stays green.
    #[test]
    fn real_model_stage_a_teacher_forced_parity_when_available() {
        let Some(model) =
            read_real_neom_or_skip("real_model_stage_a_teacher_forced_parity_when_available")
        else {
            return;
        };
        require_phase9_real_quant_feature();
        let Some(ref_topk) =
            ref_topk_from_env_or_skip("real_model_stage_a_teacher_forced_parity_when_available")
        else {
            return;
        };
        let prompt = real_neom_stage_a_prompt(&model);
        assert!(
            (16..=32).contains(&ref_topk.len()),
            "NEO_REAL_LLAMA_TOPK must contain 16..=32 positions, got {}",
            ref_topk.len()
        );

        let unique_counts = log_real_neom_layer_dtype_inventory(&model);
        assert!(
            unique_counts.total_quant() > 0,
            "real .neom is not quantized (all-f32?): {}",
            unique_counts.summary()
        );

        let mut trace = RealForwardTrace::default();
        let result = real_neom_teacher_forced_parity(
            &model,
            &prompt,
            &ref_topk,
            NEAR_TIE_EPS_LOGPROB,
            &mut trace,
        );
        assert!(
            trace.dtype_counts.covers(&unique_counts),
            "F1 mixed-quant matmul skipped a quant path present in the model: model={} matmul={}",
            unique_counts.summary(),
            trace.dtype_counts.summary()
        );

        eprintln!(
            "TEACHER_FORCED_PARITY positions={} strict={} near_tie={} gross={} eps_logprob={} per_token_ms={:.3}",
            ref_topk.len(),
            result.strict,
            result.near_tie,
            result.gross,
            NEAR_TIE_EPS_LOGPROB,
            result.per_token_ms
        );
        for note in &result.notes {
            eprintln!("  {note}");
        }
        assert_eq!(
            result.strict + result.near_tie + result.gross,
            ref_topk.len(),
            "every position must be classified"
        );
        assert_eq!(
            result.gross,
            0,
            "teacher-forced parity has {} gross divergence(s) (not near-ties) of {} positions",
            result.gross,
            ref_topk.len()
        );

        // Determinism: identical classification on re-run.
        let mut trace2 = RealForwardTrace::default();
        let again = real_neom_teacher_forced_parity(
            &model,
            &prompt,
            &ref_topk,
            NEAR_TIE_EPS_LOGPROB,
            &mut trace2,
        );
        assert_eq!(
            (again.strict, again.near_tie, again.gross),
            (result.strict, result.near_tie, result.gross),
            "teacher-forced parity is not deterministic"
        );
    }

    /// OT-16 B3 localizer scaffold: optional per-layer fingerprint comparison.
    /// Set `NEO_REAL_LAYER_TRACE` to semicolon/newline-separated entries:
    ///
    /// `pos=0,layer=3,stage=l_out,len=896,sum=1.25,absmax=0.5,hash=0x...`
    ///
    /// `layer=none` is used for global stages such as `model.input_embed`,
    /// `result_norm`, and `result_output`. `len`, `sum`, `absmax`, and `hash`
    /// are optional, but at least one value must be present per entry. Set
    /// `NEO_REAL_LAYER_TRACE_TOKENS` to the reference-forced token IDs when the
    /// trace extends beyond the prompt; otherwise the Stage-A prompt is used.
    /// Set `NEO_REAL_LAYER_TRACE_DUMP=1` to print every actual fingerprint; a
    /// reference is optional in dump mode so a single run can seed comparison.
    #[test]
    fn real_model_stage_a_layer_localizer_when_available() {
        let Some(model) =
            read_real_neom_or_skip("real_model_stage_a_layer_localizer_when_available")
        else {
            return;
        };
        require_phase9_real_quant_feature();
        let dump = std::env::var_os("NEO_REAL_LAYER_TRACE_DUMP").is_some();
        let reference = std::env::var("NEO_REAL_LAYER_TRACE")
            .ok()
            .map(|value| parse_layer_trace_reference(&value).expect("parse NEO_REAL_LAYER_TRACE"));
        if reference.is_none() && !dump {
            eprintln!(
                "SKIP real_model_stage_a_layer_localizer_when_available: NEO_REAL_LAYER_TRACE unset"
            );
            return;
        }
        let tokens =
            layer_trace_tokens_from_env().unwrap_or_else(|| real_neom_stage_a_prompt(&model));
        assert!(
            !tokens.is_empty(),
            "NEO_REAL_LAYER_TRACE_TOKENS must not be empty"
        );

        let mut trace = RealForwardTrace::default();
        let logits = real_neom_trace_token_stream(&model, &tokens, &mut trace);
        if dump {
            let top = top_two_logits(&logits);
            eprintln!(
                "REAL_LAYER_TRACE_TOP1 token={} logit={:.9} runner_up={} runner_up_logit={:.9} margin={:.9}",
                top.token,
                top.logit,
                top.runner_up_token,
                top.runner_up_logit,
                top.logit - top.runner_up_logit
            );
            for entry in &trace.layer_trace {
                eprintln!(
                    "REAL_LAYER_TRACE pos={} layer={} stage={} len={} sum={:.9} absmax={:.9} hash=0x{:016x}",
                    entry.position,
                    layer_label(entry.layer),
                    entry.stage,
                    entry.len,
                    entry.sum,
                    entry.abs_max,
                    entry.hash64
                );
            }
        }
        let compared = reference.map_or(0, |reference| {
            compare_layer_trace(&trace.layer_trace, &reference).unwrap_or_else(|err| {
                panic!("REAL_LAYER_LOCALIZER {err}");
            })
        });
        eprintln!(
            "REAL_LAYER_LOCALIZER compared={} actual_entries={} tokens={:?}",
            compared,
            trace.layer_trace.len(),
            tokens
        );
    }

    /// Negative control (PL-ratified, AAG focus): the near-tie tolerance must
    /// NOT make the oracle soft. A deliberately-broken forward — `rope_theta`
    /// corrupted from the real 1e6 to the Qwen1-era 1e4 — scrambles the
    /// positional encoding at every position past 0, so the kernel emits tokens
    /// far from the reference. The refined oracle must STILL FAIL (gross > 0);
    /// if a broken forward passed clean, the tolerance would be too loose.
    #[test]
    fn teacher_forced_parity_rejects_broken_rope_theta() {
        let Some(mut model) =
            read_real_neom_or_skip("teacher_forced_parity_rejects_broken_rope_theta")
        else {
            return;
        };
        require_phase9_real_quant_feature();
        let Some(ref_topk) =
            ref_topk_from_env_or_skip("teacher_forced_parity_rejects_broken_rope_theta")
        else {
            return;
        };
        let prompt = real_neom_stage_a_prompt(&model);

        let mut broken = model
            .model_config
            .expect("data-driven config required for the negative control");
        assert!(
            (broken.rope_theta - 10_000.0).abs() > 1.0,
            "real rope_theta must differ from the broken 1e4 value"
        );
        broken.rope_theta = 10_000.0;
        model.model_config = Some(broken);
        model.layer_config =
            Qwen2LayerConfig::for_model_config(Some(broken)).expect("broken config selects");

        let mut trace = RealForwardTrace::default();
        let result = real_neom_teacher_forced_parity(
            &model,
            &prompt,
            &ref_topk,
            NEAR_TIE_EPS_LOGPROB,
            &mut trace,
        );
        eprintln!(
            "NEGATIVE_CONTROL rope_theta=1e4 strict={} near_tie={} gross={} (gross MUST be > 0)",
            result.strict, result.near_tie, result.gross
        );
        assert!(
            result.gross > 0,
            "negative control failed: a broken (rope_theta=1e4) forward produced 0 gross \
             divergences — the near-tie tolerance is too loose"
        );
    }

    /// OT-37 ISC-3 (host-harness coherence + first-K parity proof). Feeds the
    /// EXACT chat-templated prompt the kernel builds at boot — captured verbatim
    /// from the QEMU serial `prompt_ids=21 [...]`: the Qwen2.5 chat template with
    /// ATOMIC `<|im_start|>`(151644)/`<|im_end|>`(151645) control tokens — to the
    /// real .neom forward + greedy decode. The kernel's `sys_generate` calls the
    /// identical forward (OT-30 parity-frozen) + the same greedy argmax over the
    /// same prompt, so the token stream here IS what the booted brain emits; the
    /// in-kernel boot simply cannot finish even one 485 MB forward inside the
    /// 3600s QEMU-TCG gate (TCG has no SIMD JIT / KVM), so this native run is the
    /// local full-485 MB coherence proof the dispatch scopes as "local/bare-metal".
    /// Prints the reply token IDs (decode to text via the `.vocab` sidecar). Gated
    /// on NEO_REAL_NEOM (CI has no model) — skips otherwise.
    #[cfg(feature = "phase9-real-quant")]
    #[test]
    fn ot37_real_brain_chat_reply_when_available() {
        let Some(model) = read_real_neom_or_skip("ot37_real_brain_chat_reply_when_available")
        else {
            return;
        };
        require_phase9_real_quant_feature();

        // Raw-captured from the kernel boot serial `prompt_ids=21 [...]` breadcrumb
        // (run target/neo-test/arthur_real_brain_boot.attempt1.serial.log:111,
        // template-fix run). Decodes to:
        //   <|im_start|>system\nYou are a helpful assistant.<|im_end|>\n
        //   <|im_start|>user\nhello world<|im_end|>\n<|im_start|>assistant\n
        // ISC-B1 below proves this equals what the kernel's REAL tokenizer builds
        // (frame_turn_ids over the real 151936-entry BPE), so the proof is no
        // longer hostage to a human transcription (byte-oracle-acquisition rule).
        const PROMPT: [u32; 21] = [
            151644, 8948, 198, 2610, 525, 264, 10950, 17847, 13, 151645, 198, 151644, 872, 198,
            14990, 1879, 151645, 198, 151644, 77091, 198,
        ];
        const QWEN_EOS: u32 = 151645;

        // ISC-B1 (byte-oracle joint): run the kernel's EXACT boot tokenizer path —
        // the real `frame_turn_ids` over the real BPE built from this model's
        // `.vocab` sidecar — and assert it reproduces PROMPT id-for-id. This ties
        // the two ends of the coherence proof (the kernel builds this prompt; the
        // forward answers it) with a gate, and (ISC-B2) exercises the REAL encoder
        // on the text segments — not the mock byte-identity encoder of the framing
        // unit oracle.
        {
            let neom_path = std::env::var("NEO_REAL_NEOM").expect("NEO_REAL_NEOM set");
            let vocab_path = std::path::Path::new(&neom_path).with_extension("vocab");
            let sidecar = std::fs::read(&vocab_path)
                .unwrap_or_else(|e| panic!("read .vocab sidecar {}: {e}", vocab_path.display()));
            let real_vocab = crate::vocab_sidecar::try_build_real_vocab(&sidecar)
                .expect("build real vocab from sidecar");
            let mut built = Vec::new();
            crate::vocab_sidecar::QwenTemplate::new()
                .frame_turn_ids(
                    "hello world",
                    |s| real_vocab.tokenizer.encode(s).ok(),
                    &mut built,
                )
                .expect("frame_turn_ids over real BPE");
            assert_eq!(
                built.as_slice(),
                &PROMPT[..],
                "ISC-B1: kernel real-tokenizer boot prompt must equal the raw-captured PROMPT"
            );
            eprintln!("OT37_FRAME_MATCH=ok n={}", built.len());
        }

        let mut trace = RealForwardTrace::default();
        let report = real_neom_greedy_decode(&model, &PROMPT, 48, &mut trace, None);

        // Coherence preconditions: a non-EOS first token and a finite turn.
        assert_ne!(
            report.tokens.first().copied(),
            Some(QWEN_EOS),
            "OT-37: the real brain must NOT emit immediate EOS (the bug)"
        );
        let reply: Vec<u32> = match report.tokens.iter().position(|&t| t == QWEN_EOS) {
            Some(i) => report.tokens[..i].to_vec(),
            None => report.tokens.clone(),
        };
        assert!(
            !reply.is_empty(),
            "OT-37: reply must contain at least one non-EOS token"
        );

        // Determinism by construction (greedy argmax, no RNG) — re-run is identical.
        let report2 =
            real_neom_greedy_decode(&model, &PROMPT, 48, &mut RealForwardTrace::default(), None);
        assert_eq!(
            report.tokens, report2.tokens,
            "OT-37: greedy decode must be deterministic"
        );

        eprintln!("OT37_PROMPT_IDS={:?}", PROMPT);
        eprintln!("OT37_REPLY_IDS={:?}", reply);
        eprintln!("OT37_FULL_IDS={:?}", report.tokens);
        eprintln!(
            "OT37_FIRSTK_TOP1={:?}",
            report.tokens.iter().take(8).collect::<Vec<_>>()
        );
        eprintln!("OT37_DECODE_MS={}", report.elapsed.as_millis());
    }

    #[cfg(feature = "phase9-real-quant")]
    #[test]
    fn forward_f4_q5_0_native_perf_probe_when_requested() {
        if std::env::var("NEO_F4_Q5_PERF").ok().as_deref() != Some("1") {
            eprintln!(
                "SKIP forward_f4_q5_0_native_perf_probe_when_requested: set NEO_F4_Q5_PERF=1"
            );
            return;
        }
        let Some(model) =
            read_real_neom_or_skip("forward_f4_q5_0_native_perf_probe_when_requested")
        else {
            return;
        };
        require_phase9_real_quant_feature();
        let (name, rows, cols, data) = first_real_q5_0_matmul_candidate(&model);
        let lhs = perf_probe_lhs(rows);
        let iters = std::env::var("NEO_F4_Q5_PERF_ITERS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(3)
            .max(1);

        let native_warm = forward_logic::matmul_vec_mixed_quant_for_test(
            &lhs,
            1,
            rows,
            TensorDtype::Q5_0,
            data,
            cols,
        )
        .unwrap();
        let reference_warm = forward_logic::matmul_vec_mixed_quant_dequant_reference_for_test(
            &lhs,
            1,
            rows,
            TensorDtype::Q5_0,
            data,
            cols,
        )
        .unwrap();
        assert_eq!(native_warm.len(), reference_warm.len(), "{name} output len");
        std::hint::black_box((&native_warm, &reference_warm));

        let native_started = std::time::Instant::now();
        let mut native_checksum = 0.0f32;
        for _ in 0..iters {
            let out = forward_logic::matmul_vec_mixed_quant_for_test(
                &lhs,
                1,
                rows,
                TensorDtype::Q5_0,
                data,
                cols,
            )
            .unwrap();
            native_checksum += out.iter().copied().sum::<f32>();
            std::hint::black_box(&out);
        }
        let native_elapsed = native_started.elapsed();

        let reference_started = std::time::Instant::now();
        let mut reference_checksum = 0.0f32;
        for _ in 0..iters {
            let out = forward_logic::matmul_vec_mixed_quant_dequant_reference_for_test(
                &lhs,
                1,
                rows,
                TensorDtype::Q5_0,
                data,
                cols,
            )
            .unwrap();
            reference_checksum += out.iter().copied().sum::<f32>();
            std::hint::black_box(&out);
        }
        let reference_elapsed = reference_started.elapsed();
        std::hint::black_box((native_checksum, reference_checksum));

        eprintln!(
            "F4_Q5_PERF tensor={name} rows={rows} cols={cols} iters={iters} native_ms={:.3} reference_ms={:.3} speedup={:.3}",
            native_elapsed.as_secs_f64() * 1000.0 / iters as f64,
            reference_elapsed.as_secs_f64() * 1000.0 / iters as f64,
            reference_elapsed.as_secs_f64() / native_elapsed.as_secs_f64(),
        );
    }

    #[cfg(feature = "phase9-real-quant")]
    #[test]
    fn forward_f4_q6_k_output_perf_probe_when_requested() {
        if std::env::var("NEO_F4_Q6_OUTPUT_PERF").ok().as_deref() != Some("1") {
            return;
        }
        let Some(model) =
            read_real_neom_or_skip("forward_f4_q6_k_output_perf_probe_when_requested")
        else {
            return;
        };
        let config = model.model_config.expect("Qwen3.5 config");
        let rows = config.hidden_size;
        let cols = config.vocab;
        assert_eq!(rows, 1024);
        assert_eq!(cols, 248_320);
        assert_eq!(
            real_neom_tensor_dtype(&model, "output.weight", rows, cols),
            TensorDtype::Q6K
        );
        let data = real_neom_tensor_to_quantized_rhs(&model, "output.weight", rows, cols)
            .expect("Qwen3.5 output head remains raw Q6_K");
        let lhs = perf_probe_lhs(rows);
        let started = std::time::Instant::now();
        let logits = forward_logic::matmul_vec_mixed_quant_for_test(
            &lhs,
            1,
            rows,
            TensorDtype::Q6K,
            data,
            cols,
        )
        .unwrap();
        let elapsed = started.elapsed();
        std::hint::black_box(&logits);
        eprintln!(
            "F4_Q6_OUTPUT_PERF rows={rows} cols={cols} elapsed_ms={:.3}",
            elapsed.as_secs_f64() * 1000.0
        );
    }

    #[test]
    fn real_model_forward_profile_when_requested() {
        if std::env::var("NEO_REAL_PROFILE").ok().as_deref() != Some("1") {
            return;
        }
        let Some(model) = read_real_neom_or_skip("real_model_forward_profile_when_requested")
        else {
            return;
        };
        let prompt =
            layer_trace_tokens_from_env().unwrap_or_else(|| real_neom_stage_a_prompt(&model));
        let mut trace = RealForwardTrace::default();
        let logits = real_neom_trace_token_stream(&model, &prompt, &mut trace);
        assert_all_finite("profile logits", &logits);
    }

    #[test]
    fn real_model_stage_a_token_parser_accepts_cli_lists() {
        assert_eq!(
            parse_token_id_list("271, 198 9707\n42").unwrap(),
            vec![271, 198, 9707, 42]
        );
        assert!(parse_token_id_list("271 nope").is_err());
    }

    #[test]
    fn real_model_neom_load_validation_when_available() {
        let Some((model, config)) =
            read_real_neom_parse_only_or_skip("real_model_neom_load_validation_when_available")
        else {
            return;
        };

        assert!(config.layer_count > 0);
        assert!(config.hidden_size > 0);
        assert!(config.vocab > 0);
        assert!(config.rope_dimension_count > 0);
        assert!(config.rope_dimension_count <= config.head_dim);
        assert!(config.rope_dimension_count.is_multiple_of(2));
        let q_len = config.q_heads * config.head_dim;
        assert!(
            config.q_projection_size == q_len || config.q_projection_size == q_len * 2,
            "q_projection_size must be q_len or fused q+gate"
        );
        assert!(model.tensor_named("token_embd.weight").weight_len > 0);
        assert!(model.tensor_named("output.weight").weight_len > 0);
        eprintln!(
            "REAL_NEOM_LOAD tensors={} layers={} hidden={} intermediate={} vocab={} q_heads={} kv_heads={} head_dim={} rope_dimensions={} q_projection_size={}",
            model.tensors.len(),
            config.layer_count,
            config.hidden_size,
            config.intermediate_size,
            config.vocab,
            config.q_heads,
            config.kv_heads,
            config.head_dim,
            config.rope_dimension_count,
            config.q_projection_size
        );
    }

    #[test]
    fn real_model_qwen35_schedule_and_gdn_geometry_when_available() {
        let Some((model, config)) = read_real_neom_parse_only_or_skip(
            "real_model_qwen35_schedule_and_gdn_geometry_when_available",
        ) else {
            return;
        };
        let validated = validate_neom(&model.bytes).expect("real .neom validates");
        let schedule = crate::model_logic::resolve_schedule(
            &model.bytes,
            &validated.header,
            config.layer_count,
        )
        .expect("real .neom schedule resolves");
        let mut gdn_layers = Vec::new();
        let mut attention_layers = Vec::new();
        for (layer_idx, entry) in schedule.layers.iter().enumerate() {
            match entry.kind {
                crate::model_logic::LayerKind::GatedDeltaNetLinear => {
                    assert_ne!(entry.config_ref, 0, "GDN config_ref must be nonzero");
                    gdn_layers.push((layer_idx, entry.config_ref));
                }
                crate::model_logic::LayerKind::Qwen2AttentionFFN => {
                    assert_eq!(entry.config_ref, 0, "attention config_ref must be zero");
                    attention_layers.push(layer_idx);
                }
            }
        }
        if gdn_layers.is_empty() {
            eprintln!(
                "SKIP real_model_qwen35_schedule_and_gdn_geometry_when_available: real .neom has no GDN schedule entries"
            );
            return;
        }

        assert_eq!(schedule.layers.len(), 24);
        assert_eq!(gdn_layers.len(), 18);
        assert_eq!(attention_layers, vec![3usize, 7, 11, 15, 19, 23]);
        assert_eq!(config.head_dim, 256);
        assert_eq!(config.rope_dimension_count, 64);
        for (idx, &(layer_idx, config_ref)) in gdn_layers.iter().enumerate() {
            assert_eq!(config_ref, (idx + 1) as u32, "GDN config_ref sequence");
            assert_ne!(
                layer_idx, 3,
                "layer 3 is the first full-attention layer, not GDN"
            );
        }

        let layer_config = Qwen2LayerConfig::from_model_config(&config).unwrap();
        let first = real_neom_gdn_config(&model, gdn_layers[0].0, layer_config);
        assert_eq!(first.hidden_size, 1024);
        assert_eq!(first.num_k_heads, 16);
        assert_eq!(first.num_v_heads, 16);
        assert_eq!(first.head_k_dim, 128);
        assert_eq!(first.head_v_dim, 128);
        assert_eq!(first.conv_kernel_size, 4);
        assert_eq!(first.conv_dim(), 6144);
        assert_eq!(first.value_dim(), 2048);
        assert_eq!(first.state_size(), 262_144);
        for &(layer_idx, _) in &gdn_layers {
            assert_eq!(
                real_neom_gdn_geometry(real_neom_gdn_config(&model, layer_idx, layer_config)),
                real_neom_gdn_geometry(first),
                "all B3 Qwen3.5 GDN layers share geometry"
            );
        }
        eprintln!(
            "REAL_QWEN35_GDN layers={} full_attention={} attention_layers={:?} hidden={} conv_dim={} value_dim={} heads={} head_dim={} state_size={}",
            gdn_layers.len(),
            attention_layers.len(),
            attention_layers,
            first.hidden_size,
            first.conv_dim(),
            first.value_dim(),
            first.num_v_heads,
            first.head_v_dim,
            first.state_size()
        );
    }

    #[test]
    fn real_model_stage_a_quant_dtype_inference_is_unambiguous() {
        assert_eq!(
            infer_real_neom_quant_dtype("q5", 896, 4864, 4864 * (896 / 32) * 22).unwrap(),
            TensorDtype::Q5_0
        );
        assert_eq!(
            infer_real_neom_quant_dtype("q6", 4864, 896, 896 * (4864 / 256) * 210).unwrap(),
            TensorDtype::Q6K
        );
        assert_eq!(
            infer_real_neom_quant_dtype("q8", 896, 128, 128 * (896 / 32) * 34).unwrap(),
            TensorDtype::Q8_0
        );
    }

    #[test]
    fn real_model_layer_trace_reference_parser_accepts_optional_fields() {
        let parsed = parse_layer_trace_reference(
            "pos=0,layer=none,stage=model.input_embed,len=4,sum=1.5,absmax=2.0;\
             pos=0,layer=3,stage=l_out,hash=0x1234",
        )
        .unwrap();

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].position, 0);
        assert_eq!(parsed[0].layer, None);
        assert_eq!(parsed[0].stage, "model.input_embed");
        assert_eq!(parsed[0].len, Some(4));
        assert_eq!(parsed[0].sum, Some(1.5));
        assert_eq!(parsed[0].abs_max, Some(2.0));
        assert_eq!(parsed[1].layer, Some(3));
        assert_eq!(parsed[1].hash64, Some(0x1234));
    }

    #[test]
    fn real_model_layer_trace_comparison_reports_first_bad_layer() {
        let actual = vec![
            layer_fingerprint(0, None, "model.input_embed", &[1.0, -2.0, 3.0]),
            layer_fingerprint(0, Some(0), "l_out", &[0.25, 0.5]),
            layer_fingerprint(0, Some(1), "l_out", &[4.0, 5.0]),
        ];
        let reference = vec![
            LayerTraceReference {
                position: 0,
                layer: None,
                stage: "model.input_embed".to_string(),
                len: Some(actual[0].len),
                sum: Some(actual[0].sum),
                abs_max: Some(actual[0].abs_max),
                hash64: Some(actual[0].hash64),
            },
            LayerTraceReference {
                position: 0,
                layer: Some(0),
                stage: "l_out".to_string(),
                len: Some(actual[1].len),
                sum: Some(actual[1].sum + 1.0),
                abs_max: None,
                hash64: None,
            },
        ];
        let err = compare_layer_trace(&actual, &reference).unwrap_err();
        assert!(
            err.contains("first_mismatch ref_index=1 pos=0 layer=0 stage=l_out"),
            "{err}"
        );
        assert!(err.contains("sum got="), "{err}");
    }

    fn assert_gqa_attention_kat(
        label: &str,
        params: ops::GqaAttentionParams,
        query: &[f32],
        key: &[f32],
        value: &[f32],
        expected: &[f32],
    ) {
        let query_shape = shape3(params.n_q_heads, params.seq_len, params.head_dim);
        let kv_shape = shape3(params.n_kv_heads, params.seq_len, params.head_dim);
        let param_shape = shape1(ops::GQA_ATTENTION_PARAM_COUNT as u64);
        let query_bytes = bytes_from_f32(query);
        let key_bytes = bytes_from_f32(key);
        let value_bytes = bytes_from_f32(value);
        let param_bytes = bytes_from_f32(&[
            params.seq_len as f32,
            params.n_q_heads as f32,
            params.n_kv_heads as f32,
            params.head_dim as f32,
        ]);
        let mut direct = vec![0u8; expected.len() * 4];
        let mut dispatched = vec![0u8; expected.len() * 4];

        ops::gqa_causal_attention(
            &view(&query_shape, &query_bytes),
            &view(&kv_shape, &key_bytes),
            &view(&kv_shape, &value_bytes),
            params,
            &mut direct,
        )
        .unwrap();
        ops::dispatch(
            OpKind::GqaAttention,
            &[
                view(&query_shape, &query_bytes),
                view(&kv_shape, &key_bytes),
                view(&kv_shape, &value_bytes),
                view(&param_shape, &param_bytes),
            ],
            &query_shape,
            &mut dispatched,
        )
        .unwrap();

        assert_f32_bytes_close(label, &direct, expected, REL_EPS);
        assert_eq!(direct, dispatched);
    }

    fn tiny_kv_config() -> Qwen2LayerConfig {
        Qwen2LayerConfig {
            hidden_size: 8,
            intermediate_size: 4,
            q_heads: 4,
            kv_heads: 2,
            head_dim: 2,
            rope_dimension_count: 2,
            attn_q_projection_size: 8,
            rope_theta_scale_bits: f32_to_bits(0.5),
            rms_norm_eps_bits: f32_to_bits(RMS_NORM_EPS),
        }
    }

    fn tiny_uncached_logits(context: &[u32], config: Qwen2LayerConfig) -> Vec<f32> {
        let mut q = Vec::new();
        let mut k = Vec::new();
        let mut v = Vec::new();
        for (pos, &token) in context.iter().enumerate() {
            let (q_pos, k_pos, v_pos) = tiny_token_qkv(token, pos, config);
            q.extend_from_slice(&q_pos);
            k.extend_from_slice(&k_pos);
            v.extend_from_slice(&v_pos);
        }
        let hidden =
            forward_logic::gqa_attention_uncached_decode(&q, &k, &v, config, context.len())
                .unwrap();
        tiny_lm_head(&hidden)
    }

    fn tiny_cached_logits(
        token: u32,
        pos: usize,
        config: Qwen2LayerConfig,
        cache: &mut Qwen2KvCacheLayer,
    ) -> Vec<f32> {
        let (q_rope, k_rope, v) = tiny_token_qkv(token, pos, config);
        let seam = forward_logic::Qwen2AttentionSeam {
            attn_norm: Vec::new(),
            q: Vec::new(),
            k: Vec::new(),
            v,
            gate: None,
            q_rope,
            k_rope,
        };
        let hidden =
            forward_logic::gqa_attention_cached_decode(&seam, cache, config, pos as u64).unwrap();
        tiny_lm_head(&hidden)
    }

    fn tiny_token_qkv(
        token: u32,
        pos: usize,
        config: Qwen2LayerConfig,
    ) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        assert_eq!(config.q_heads, 4);
        assert_eq!(config.kv_heads, 2);
        assert_eq!(config.head_dim, 2);
        let t = token as f32;
        let p = pos as f32;
        let q = vec![
            0.25 + 0.10 * t + 0.01 * p,
            -0.15 + 0.02 * t,
            0.35 - 0.03 * t + 0.04 * p,
            0.05 + 0.05 * t,
            -0.45 + 0.01 * t,
            0.30 + 0.02 * p,
            0.20 + 0.04 * t,
            -0.10 + 0.03 * p,
        ];
        let k = vec![
            0.40 + 0.03 * t,
            -0.20 + 0.02 * p,
            0.10 + 0.05 * t,
            0.25 - 0.01 * p,
        ];
        let v = vec![
            0.50 + 0.10 * t,
            -0.30 + 0.03 * p,
            0.20 - 0.02 * t,
            0.40 + 0.01 * p,
        ];
        (q, k, v)
    }

    fn tiny_lm_head(hidden: &[f32]) -> Vec<f32> {
        assert_eq!(hidden.len(), 8);
        vec![
            hidden[0] - hidden[1],
            hidden[2] * 0.5,
            -hidden[3],
            hidden[4] + hidden[5] * 0.25,
            hidden[6] - hidden[7],
            1.0 + hidden.iter().sum::<f32>() * 0.01,
        ]
    }

    /// Phase 8 BUILD-8.3a ISC-2: raw-source `get_scale_min_k4` 6-bit shuffle.
    /// This traps the high-bit interleave used by Q4_K scales/mins.
    #[test]
    fn q4_k_isc2_scale_min_unpack_kat() {
        let scales = [
            0xc1, 0x82, 0x43, 0x04, 0xf5, 0xb6, 0x77, 0x38, 0x9a, 0xbc, 0xde, 0xf0,
        ];
        let expected = [
            (1, 53),
            (2, 54),
            (3, 55),
            (4, 56),
            (58, 57),
            (44, 43),
            (30, 29),
            (0, 15),
        ];

        for (idx, pair) in expected.iter().enumerate() {
            assert_eq!(ops::q4_k::get_scale_min_k4(idx, &scales), Ok(*pair));
        }
    }

    /// Phase 8 BUILD-8.3a ISC-1: hand-laid 144-byte Q4_K block oracle.
    /// Layout is d=f16(0.5), dmin=f16(0.25), 12 packed scale bytes, then 128
    /// quant bytes. Expected values are independently fixed from
    /// `y = d*scale*q - dmin*min`, not produced by the implementation.
    #[test]
    fn q4_k_isc1_dequantizes_hand_laid_block_kat() {
        const BLOCK: [u8; ops::q4_k::Q4_K_BLOCK_BYTES] = [
            0x00, 0x38, 0x00, 0x34, 0xc1, 0x82, 0x43, 0x04, 0xf5, 0xb6, 0x77, 0x38, 0x9a, 0xbc,
            0xde, 0xf0, 0xf0, 0xe1, 0xd2, 0xc3, 0xb4, 0xa5, 0x96, 0x87, 0x78, 0x69, 0x5a, 0x4b,
            0x3c, 0x2d, 0x1e, 0x0f, 0xf0, 0xe1, 0xd2, 0xc3, 0xb4, 0xa5, 0x96, 0x87, 0x78, 0x69,
            0x5a, 0x4b, 0x3c, 0x2d, 0x1e, 0x0f, 0xf0, 0xe1, 0xd2, 0xc3, 0xb4, 0xa5, 0x96, 0x87,
            0x78, 0x69, 0x5a, 0x4b, 0x3c, 0x2d, 0x1e, 0x0f, 0xf0, 0xe1, 0xd2, 0xc3, 0xb4, 0xa5,
            0x96, 0x87, 0x78, 0x69, 0x5a, 0x4b, 0x3c, 0x2d, 0x1e, 0x0f, 0xf0, 0xe1, 0xd2, 0xc3,
            0xb4, 0xa5, 0x96, 0x87, 0x78, 0x69, 0x5a, 0x4b, 0x3c, 0x2d, 0x1e, 0x0f, 0xf0, 0xe1,
            0xd2, 0xc3, 0xb4, 0xa5, 0x96, 0x87, 0x78, 0x69, 0x5a, 0x4b, 0x3c, 0x2d, 0x1e, 0x0f,
            0xf0, 0xe1, 0xd2, 0xc3, 0xb4, 0xa5, 0x96, 0x87, 0x78, 0x69, 0x5a, 0x4b, 0x3c, 0x2d,
            0x1e, 0x0f, 0xf0, 0xe1, 0xd2, 0xc3, 0xb4, 0xa5, 0x96, 0x87, 0x78, 0x69, 0x5a, 0x4b,
            0x3c, 0x2d, 0x1e, 0x0f,
        ];
        const EXPECTED: [f32; ops::q4_k::Q4_K_VALUES] = [
            -13.25, -12.75, -12.25, -11.75, -11.25, -10.75, -10.25, -9.75, -9.25, -8.75, -8.25,
            -7.75, -7.25, -6.75, -6.25, -5.75, -13.25, -12.75, -12.25, -11.75, -11.25, -10.75,
            -10.25, -9.75, -9.25, -8.75, -8.25, -7.75, -7.25, -6.75, -6.25, -5.75, 1.5, 0.5, -0.5,
            -1.5, -2.5, -3.5, -4.5, -5.5, -6.5, -7.5, -8.5, -9.5, -10.5, -11.5, -12.5, -13.5, 1.5,
            0.5, -0.5, -1.5, -2.5, -3.5, -4.5, -5.5, -6.5, -7.5, -8.5, -9.5, -10.5, -11.5, -12.5,
            -13.5, -13.75, -12.25, -10.75, -9.25, -7.75, -6.25, -4.75, -3.25, -1.75, -0.25, 1.25,
            2.75, 4.25, 5.75, 7.25, 8.75, -13.75, -12.25, -10.75, -9.25, -7.75, -6.25, -4.75,
            -3.25, -1.75, -0.25, 1.25, 2.75, 4.25, 5.75, 7.25, 8.75, 16.0, 14.0, 12.0, 10.0, 8.0,
            6.0, 4.0, 2.0, 0.0, -2.0, -4.0, -6.0, -8.0, -10.0, -12.0, -14.0, 16.0, 14.0, 12.0,
            10.0, 8.0, 6.0, 4.0, 2.0, 0.0, -2.0, -4.0, -6.0, -8.0, -10.0, -12.0, -14.0, -14.25,
            14.75, 43.75, 72.75, 101.75, 130.75, 159.75, 188.75, 217.75, 246.75, 275.75, 304.75,
            333.75, 362.75, 391.75, 420.75, -14.25, 14.75, 43.75, 72.75, 101.75, 130.75, 159.75,
            188.75, 217.75, 246.75, 275.75, 304.75, 333.75, 362.75, 391.75, 420.75, 319.25, 297.25,
            275.25, 253.25, 231.25, 209.25, 187.25, 165.25, 143.25, 121.25, 99.25, 77.25, 55.25,
            33.25, 11.25, -10.75, 319.25, 297.25, 275.25, 253.25, 231.25, 209.25, 187.25, 165.25,
            143.25, 121.25, 99.25, 77.25, 55.25, 33.25, 11.25, -10.75, -7.25, 7.75, 22.75, 37.75,
            52.75, 67.75, 82.75, 97.75, 112.75, 127.75, 142.75, 157.75, 172.75, 187.75, 202.75,
            217.75, -7.25, 7.75, 22.75, 37.75, 52.75, 67.75, 82.75, 97.75, 112.75, 127.75, 142.75,
            157.75, 172.75, 187.75, 202.75, 217.75, -3.75, -3.75, -3.75, -3.75, -3.75, -3.75,
            -3.75, -3.75, -3.75, -3.75, -3.75, -3.75, -3.75, -3.75, -3.75, -3.75, -3.75, -3.75,
            -3.75, -3.75, -3.75, -3.75, -3.75, -3.75, -3.75, -3.75, -3.75, -3.75, -3.75, -3.75,
            -3.75, -3.75,
        ];

        let got = ops::q4_k::dequantize_q4_k(&BLOCK).unwrap();

        assert_eq!(got, EXPECTED);
    }

    /// Phase 9 Q4_K dequant AVX2 ISC-1: the selectable AVX2 kernel is
    /// bit-identical to the scalar oracle on the existing hand-laid 8.3a
    /// block. If the host lacks AVX2 state, `AutoAvx2` falls back to scalar
    /// (trivially equal). Also covers the trap cases: all-zero quants, all-0xff
    /// nibbles (max), alternating nibbles, and a block with distinct sc/m per
    /// sub-block.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn phase9_avx2_q4_k_dequant_bit_identical_hand_laid() {
        // The 8.3a oracle block — a real representative block with nontrivial
        // d/dmin/scales/nibbles. Reuse the EXACT same BLOCK + EXPECTED from
        // the scalar KAT above so any divergence is unambiguously the AVX2.
        const BLOCK: [u8; ops::q4_k::Q4_K_BLOCK_BYTES] = [
            0x00, 0x38, 0x00, 0x34, 0xc1, 0x82, 0x43, 0x04, 0xf5, 0xb6, 0x77, 0x38, 0x9a, 0xbc,
            0xde, 0xf0, 0xf0, 0xe1, 0xd2, 0xc3, 0xb4, 0xa5, 0x96, 0x87, 0x78, 0x69, 0x5a, 0x4b,
            0x3c, 0x2d, 0x1e, 0x0f, 0xf0, 0xe1, 0xd2, 0xc3, 0xb4, 0xa5, 0x96, 0x87, 0x78, 0x69,
            0x5a, 0x4b, 0x3c, 0x2d, 0x1e, 0x0f, 0xf0, 0xe1, 0xd2, 0xc3, 0xb4, 0xa5, 0x96, 0x87,
            0x78, 0x69, 0x5a, 0x4b, 0x3c, 0x2d, 0x1e, 0x0f, 0xf0, 0xe1, 0xd2, 0xc3, 0xb4, 0xa5,
            0x96, 0x87, 0x78, 0x69, 0x5a, 0x4b, 0x3c, 0x2d, 0x1e, 0x0f, 0xf0, 0xe1, 0xd2, 0xc3,
            0xb4, 0xa5, 0x96, 0x87, 0x78, 0x69, 0x5a, 0x4b, 0x3c, 0x2d, 0x1e, 0x0f, 0xf0, 0xe1,
            0xd2, 0xc3, 0xb4, 0xa5, 0x96, 0x87, 0x78, 0x69, 0x5a, 0x4b, 0x3c, 0x2d, 0x1e, 0x0f,
            0xf0, 0xe1, 0xd2, 0xc3, 0xb4, 0xa5, 0x96, 0x87, 0x78, 0x69, 0x5a, 0x4b, 0x3c, 0x2d,
            0x1e, 0x0f, 0xf0, 0xe1, 0xd2, 0xc3, 0xb4, 0xa5, 0x96, 0x87, 0x78, 0x69, 0x5a, 0x4b,
            0x3c, 0x2d, 0x1e, 0x0f,
        ];
        let scalar = ops::q4_k::dequantize_q4_k(&BLOCK).unwrap();
        let avx2 = ops::qk_matmul::dequantize_q4_k_with_kernel(
            &BLOCK,
            ops::qk_matmul::QKMatmulKernel::AutoAvx2,
        )
        .unwrap();
        for i in 0..scalar.len() {
            assert_eq!(
                avx2[i].to_bits(),
                scalar[i].to_bits(),
                "real block lane {i}: scalar={} avx2={}",
                scalar[i],
                avx2[i]
            );
        }

        // (a) All-zero block — d=0, dmin=0, all quants 0 → all outputs 0.0.
        let zero_block = [0u8; ops::q4_k::Q4_K_BLOCK_BYTES];
        let scalar_z = ops::q4_k::dequantize_q4_k(&zero_block).unwrap();
        let avx2_z = ops::qk_matmul::dequantize_q4_k_with_kernel(
            &zero_block,
            ops::qk_matmul::QKMatmulKernel::AutoAvx2,
        )
        .unwrap();
        for i in 0..scalar_z.len() {
            assert_eq!(
                avx2_z[i].to_bits(),
                scalar_z[i].to_bits(),
                "zero block lane {i}"
            );
        }

        // (b) All-0xff nibbles — d=f16(1.0), dmin=f16(0.0), all quants 0xff
        //     → low nibbles = 15, high nibbles = 15. Max nibble value.
        let mut ff_block = [0u8; ops::q4_k::Q4_K_BLOCK_BYTES];
        ff_block[0] = 0x00;
        ff_block[1] = 0x3c; // f16(1.0)
        ff_block[2] = 0x00;
        ff_block[3] = 0x00; // f16(0.0) = dmin=0
        // Scales: all 1 (sc=1, m=0 for all 8 sub-blocks).
        ff_block[4..16].fill(0x01);
        // Quants: all 0xff.
        ff_block[16..].fill(0xff);
        let scalar_ff = ops::q4_k::dequantize_q4_k(&ff_block).unwrap();
        let avx2_ff = ops::qk_matmul::dequantize_q4_k_with_kernel(
            &ff_block,
            ops::qk_matmul::QKMatmulKernel::AutoAvx2,
        )
        .unwrap();
        for i in 0..scalar_ff.len() {
            assert_eq!(
                avx2_ff[i].to_bits(),
                scalar_ff[i].to_bits(),
                "0xff block lane {i}: scalar={} avx2={}",
                scalar_ff[i],
                avx2_ff[i]
            );
        }

        // (c) Alternating nibbles — 0x0f repeated → low=15, high=0; 0xf0
        //     repeated → low=0, high=15. Tests the nibble extraction.
        let mut alt_block = [0u8; ops::q4_k::Q4_K_BLOCK_BYTES];
        alt_block[0] = 0x00;
        alt_block[1] = 0x3c; // f16(1.0)
        alt_block[2] = 0x00;
        alt_block[3] = 0x34; // f16(0.25)
        alt_block[4..16].fill(0x01); // sc=1, m=0
        for i in 0..128 {
            alt_block[16 + i] = if i % 2 == 0 { 0x0f } else { 0xf0 };
        }
        let scalar_alt = ops::q4_k::dequantize_q4_k(&alt_block).unwrap();
        let avx2_alt = ops::qk_matmul::dequantize_q4_k_with_kernel(
            &alt_block,
            ops::qk_matmul::QKMatmulKernel::AutoAvx2,
        )
        .unwrap();
        for i in 0..scalar_alt.len() {
            assert_eq!(
                avx2_alt[i].to_bits(),
                scalar_alt[i].to_bits(),
                "alternating block lane {i}: scalar={} avx2={}",
                scalar_alt[i],
                avx2_alt[i]
            );
        }

        // (d) Distinct sc/m per sub-block — each of the 8 sub-blocks gets a
        //     different (sc, m) pair, exercising the get_scale_min_k4 depack
        //     across all index ranges (0..3 low, 4..7 high).
        let mut distinct = [0u8; ops::q4_k::Q4_K_BLOCK_BYTES];
        distinct[0] = 0x00;
        distinct[1] = 0x3c; // d = f16(1.0)
        distinct[2] = 0x00;
        distinct[3] = 0x34; // dmin = f16(0.25)
        // Hand-craft scales so each sub gets distinct (sc, m).
        // Indices 0..3: scales[i] = sc, scales[i+4] = m.
        distinct[4] = 1; // sc0=1
        distinct[5] = 2; // sc1=2
        distinct[6] = 3; // sc2=3
        distinct[7] = 4; // sc3=4
        distinct[8] = 10; // m0=10
        distinct[9] = 20; // m1=20
        distinct[10] = 30; // m2=30
        distinct[11] = 40; // m3=40
        // Indices 4..7: scales[i+4] = (sc & 0x0f) | (m & 0x0f) << 4,
        // scales[i-4] |= (sc >> 4) << 6, scales[i] |= (m >> 4) << 6.
        // sc4=5, m4=11: scales[8] = (5&0xf)|(11&0xf)<<4 = 0xb5; scales[0] |= (5>>4)<<6 = 0
        // sc5=6, m5=12: scales[9] = (6&0xf)|(12&0xf)<<4 = 0xc6; scales[1] |= 0
        // sc6=7, m6=13: scales[10] = (7&0xf)|(13&0xf)<<4 = 0xd7; scales[2] |= 0
        // sc7=8, m7=14: scales[11] = (8&0xf)|(14&0xf)<<4 = 0xe8; scales[3] |= 0
        distinct[8] = 0xb5;
        distinct[9] = 0xc6;
        distinct[10] = 0xd7;
        distinct[11] = 0xe8;
        // Quants: a repeating pattern.
        for i in 0..128 {
            distinct[16 + i] = ((i * 17) & 0xff) as u8;
        }
        let scalar_d = ops::q4_k::dequantize_q4_k(&distinct).unwrap();
        let avx2_d = ops::qk_matmul::dequantize_q4_k_with_kernel(
            &distinct,
            ops::qk_matmul::QKMatmulKernel::AutoAvx2,
        )
        .unwrap();
        for i in 0..scalar_d.len() {
            assert_eq!(
                avx2_d[i].to_bits(),
                scalar_d[i].to_bits(),
                "distinct-sc-m block lane {i}: scalar={} avx2={}",
                scalar_d[i],
                avx2_d[i]
            );
        }
    }

    /// Arthur latency NOTE fix 3: prefill that skips the lm_head for every
    /// prompt token but the last leaves the KV cache and the last position's
    /// logits bit-identical to computing logits for every token. Drives the
    /// production `decode_logic` (`DecodeCursor::step` +
    /// `prefill_last_logits`, the exact code `InventoryDecodeState::step` and
    /// the kernel's `sys_generate` prefill run) through the real
    /// `forward_logic` layers, against an independent per-token reference
    /// (review item 18: a step that skips a layer, a cache write or the
    /// position bump when it skips the logits fails here). Real Qwen2.5-0.5B
    /// layer and vocab shapes, synthetic quantized weights (Q5_0/Q8_0/Q4_K, the
    /// 896-input matrices cannot be K-quants).
    #[cfg(feature = "phase9-real-quant")]
    #[test]
    fn prefill_lm_head_skip_is_bit_identical() {
        use super::decode_logic::{DecodeCursor, TokenForward, prefill_last_logits};
        use super::forward_logic::{
            Qwen2KvCacheLayer, Qwen2LayerConfig, Qwen2LayerDtypes, Qwen2LayerWeights,
            Qwen2OutputWeights, Qwen2QuantizedLayerWeights, QwenAttentionNormWeights, TensorDtype,
            forward_layer_with_gqa_attention_with_norms, qwen2_output_logits,
        };
        ops::qk_matmul::init_avx2_decision();
        const LAYERS: usize = 2;
        let (h, kv, ff, vocab) = (896usize, 128usize, 4864usize, 151_936usize);
        let config = Qwen2LayerConfig::qwen25_05b();
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut rnd = |n: usize| -> Vec<u8> {
            (0..n)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    seed as u8
                })
                .collect()
        };
        // Block bytes with every f16 scale pinned small and finite.
        let mut quant = |dtype: TensorDtype, shared: usize, cols: usize| -> Vec<u8> {
            let (block, values, scales): (usize, usize, &[usize]) = match dtype {
                TensorDtype::Q4K => (144, 256, &[0, 2]),
                TensorDtype::Q5_0 => (22, 32, &[0]),
                TensorDtype::Q8_0 => (34, 32, &[0]),
                _ => unreachable!(),
            };
            let mut w = rnd(cols * shared / values * block);
            for b in w.chunks_mut(block) {
                for &o in scales {
                    b[o] = 0x00;
                    b[o + 1] = 0x20; // 2^-7
                }
            }
            w
        };
        let dtypes = Qwen2LayerDtypes {
            attn_q: TensorDtype::Q5_0,
            attn_k: TensorDtype::Q5_0,
            attn_v: TensorDtype::Q8_0,
            attn_o: TensorDtype::Q5_0,
            ffn_gate: TensorDtype::Q5_0,
            ffn_up: TensorDtype::Q5_0,
            ffn_down: TensorDtype::Q4K,
        };
        let layers: Vec<[Vec<u8>; 7]> = (0..LAYERS)
            .map(|_| {
                [
                    quant(dtypes.attn_q, h, h),
                    quant(dtypes.attn_k, h, kv),
                    quant(dtypes.attn_v, h, kv),
                    quant(dtypes.attn_o, h, h),
                    quant(dtypes.ffn_gate, h, ff),
                    quant(dtypes.ffn_up, h, ff),
                    quant(dtypes.ffn_down, ff, h),
                ]
            })
            .collect();
        let lm_head = quant(TensorDtype::Q8_0, h, vocab);
        let norm = vec![1.0f32; h];
        let q_bias = vec![0.01f32; h];
        let kv_bias = vec![0.02f32; kv];
        let embed = |token: u32| -> Vec<f32> {
            (0..h)
                .map(|i| (((i as u32 * 31 + token * 17) % 97) as f32 - 48.0) * 0.004)
                .collect()
        };
        let weights = |i: usize| Qwen2LayerWeights {
            attn_norm: &norm,
            attn_q: &[],
            attn_k: &[],
            attn_v: &[],
            attn_q_bias: Some(&q_bias),
            attn_k_bias: Some(&kv_bias),
            attn_v_bias: Some(&kv_bias),
            attn_o: &[],
            ffn_norm: &norm,
            ffn_gate: &[],
            ffn_up: &[],
            ffn_down: &[],
            dtypes,
            quantized: Qwen2QuantizedLayerWeights {
                attn_q: Some(&layers[i][0]),
                attn_k: Some(&layers[i][1]),
                attn_v: Some(&layers[i][2]),
                attn_o: Some(&layers[i][3]),
                ffn_gate: Some(&layers[i][4]),
                ffn_up: Some(&layers[i][5]),
                ffn_down: Some(&layers[i][6]),
            },
        };
        let output = |hidden: &[f32]| {
            qwen2_output_logits(
                hidden,
                Qwen2OutputWeights {
                    output_norm: &norm,
                    output: &[],
                    output_dtype: TensorDtype::Q8_0,
                    output_quantized: Some(&lm_head),
                    output_bias: None,
                },
                config,
                vocab,
            )
            .unwrap()
        };
        // One token through the layer stack at `position`, advancing `caches`.
        let layer_stack = |caches: &mut Vec<Qwen2KvCacheLayer>, token: u32, position: u64| {
            let mut hidden = embed(token);
            for (i, cache) in caches.iter_mut().enumerate() {
                hidden = forward_layer_with_gqa_attention_with_norms(
                    &hidden,
                    weights(i),
                    config,
                    position,
                    true,
                    Some(cache),
                    None,
                    QwenAttentionNormWeights::NONE,
                )
                .unwrap()
                .ffn
                .residual;
            }
            hidden
        };
        let new_caches = || -> Vec<Qwen2KvCacheLayer> {
            (0..LAYERS)
                .map(|_| Qwen2KvCacheLayer::new(config, 8).unwrap())
                .collect()
        };
        let prompt = [151_644u32, 8_948, 198];

        // Without the skip: logits for every prompt token (the old loop).
        let mut caches_a = new_caches();
        let mut logits_a = Vec::new();
        for (pos, &token) in prompt.iter().enumerate() {
            logits_a = output(&layer_stack(&mut caches_a, token, pos as u64));
        }

        // With the skip: the production cursor and driver, over the same
        // layers and lm_head (the kernel's adapter is these same two calls).
        struct Forward<'a, L, O> {
            caches: Vec<Qwen2KvCacheLayer>,
            layer_stack: &'a L,
            output: &'a O,
            lm_head_calls: usize,
        }
        impl<L, O> TokenForward for Forward<'_, L, O>
        where
            L: Fn(&mut Vec<Qwen2KvCacheLayer>, u32, u64) -> Vec<f32>,
            O: Fn(&[f32]) -> Vec<f32>,
        {
            type Error = &'static str;
            fn layers(&mut self, token: u32, position: usize) -> Result<Vec<f32>, &'static str> {
                Ok((self.layer_stack)(&mut self.caches, token, position as u64))
            }
            fn output(&mut self, hidden: &[f32]) -> Result<Vec<f32>, &'static str> {
                self.lm_head_calls += 1;
                Ok((self.output)(hidden))
            }
        }
        let mut forward_b = Forward {
            caches: new_caches(),
            layer_stack: &layer_stack,
            output: &output,
            lm_head_calls: 0,
        };
        let mut cursor = DecodeCursor::new(8);
        let logits_b = prefill_last_logits(&prompt, |token, want_logits| {
            cursor.step(&mut forward_b, token, want_logits, "full")
        })
        .unwrap();
        let lm_head_calls = forward_b.lm_head_calls;
        assert_eq!(
            cursor.position(),
            prompt.len(),
            "every prompt token advances the position"
        );

        assert_eq!(
            lm_head_calls, 1,
            "the lm_head runs for the last prompt token only"
        );
        assert_eq!(logits_a.len(), vocab);
        assert!(
            logits_a
                .iter()
                .zip(&logits_b)
                .all(|(a, b)| a.to_bits() == b.to_bits())
                && logits_a.len() == logits_b.len(),
            "last-position logits differ"
        );
        // The caches are identical too: the next (decode) position's layer
        // output, which reads every cached key and value, matches bit for bit.
        let next_a = output(&layer_stack(&mut caches_a, 872, prompt.len() as u64));
        let next_b = cursor
            .step(&mut forward_b, 872, true, "full")
            .unwrap()
            .expect("decode logits");
        assert!(
            next_a
                .iter()
                .zip(&next_b)
                .all(|(a, b)| a.to_bits() == b.to_bits()),
            "next-position logits differ: the cache or position diverged"
        );
        // Past the context bound the step fails and changes nothing.
        let mut tiny = DecodeCursor::new(0);
        assert_eq!(tiny.step(&mut forward_b, 1, true, "full"), Err("full"));
        assert_eq!(tiny.position(), 0);
        // The empty prompt returns empty logits, as the per-token loop did.
        assert!(
            prefill_last_logits::<()>(&[], |_, _| unreachable!())
                .unwrap()
                .is_empty()
        );
    }

    /// Arthur latency NOTE fix 2 (Janus F1, items 8/10): the AVX2 census only
    /// says "yes" when every online CPU's OWN check said yes. Asymmetric CPUs
    /// (an AP without XSAVE that never writes XCR0; an FXSAVE build with BSP
    /// XCR0=7 and AP XCR0=3) make the AP's local check say no, and that veto
    /// decides "no" for all, so no CPU runs AVX2 its own check would reject.
    #[test]
    fn avx2_census_requires_every_cpu() {
        use super::ops::qk_matmul::{Avx2Census, Avx2Decision};
        // Homogeneous: every CPU yes => yes, once all have reported.
        let c = Avx2Census::new();
        for _ in 0..4 {
            c.report(true);
        }
        assert_eq!(c.publish(4), Avx2Decision::Yes);

        // F1 item 8: BSP has XSAVE/AVX2 (XCR0=7), the AP lacks XSAVE and keeps
        // XCR0=1, so its local check says no.
        let c = Avx2Census::new();
        c.report(true); // BSP
        c.report(false); // AP
        assert_eq!(c.decision(), Avx2Decision::No, "a veto decides at once");
        assert_eq!(c.publish(2), Avx2Decision::No);

        // F1 item 10 (FXSAVE build): both AVX2-capable, BSP XCR0=7, AP XCR0=3.
        // Same shape: the AP's own XCR0 check fails, so the census is "no".
        let c = Avx2Census::new();
        c.report(false); // AP reports first this time
        c.report(true); // BSP
        assert_eq!(c.publish(2), Avx2Decision::No);

        // An incomplete census never says yes: each call keeps its local check.
        let c = Avx2Census::new();
        c.report(true);
        assert_eq!(c.publish(2), Avx2Decision::Undecided);
        assert_eq!(c.publish(0), Avx2Decision::Undecided);

        // A CPU that reports after "yes" was published still vetoes (it does
        // so on itself, before it can run a matmul).
        let c = Avx2Census::new();
        c.report(true);
        c.report(true);
        assert_eq!(c.publish(2), Avx2Decision::Yes);
        c.report(false);
        assert_eq!(c.decision(), Avx2Decision::No);
        // ...and a later publish cannot bring "yes" back.
        assert_eq!(c.publish(3), Avx2Decision::No);
    }

    /// The census under concurrent reporters and publishers: with one "no"
    /// among them, no interleaving ends at (or passes through a final) "yes".
    #[test]
    fn avx2_census_veto_wins_under_concurrency() {
        use super::ops::qk_matmul::{Avx2Census, Avx2Decision};
        use std::sync::Arc;
        for round in 0..200usize {
            let c = Arc::new(Avx2Census::new());
            let cpus = 8usize;
            let no_cpu = round % cpus;
            let handles: Vec<_> = (0..cpus)
                .map(|cpu| {
                    let c = c.clone();
                    std::thread::spawn(move || {
                        c.report(cpu != no_cpu);
                        // Publishers race the reports.
                        c.publish(cpus as u32)
                    })
                })
                .collect();
            for h in handles {
                let _ = h.join().unwrap();
            }
            assert_eq!(c.publish(cpus as u32), Avx2Decision::No, "round {round}");
            assert_eq!(c.reported(), cpus as u32);
        }
    }

    /// The global census on this host: one CPU reporting its own check gives
    /// that same answer, and a cached "yes" still picks the AVX2 kernel, which
    /// stays bit-identical to scalar (a decision can't change an output).
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn avx2_census_global_agrees_with_local_check() {
        let local = ops::qk_matmul::local_avx2_check();
        // Before or after another test thread's report, the answer is local.
        assert_eq!(ops::qk_matmul::q4_k_q8_k_avx2_available(), local);
        ops::qk_matmul::init_avx2_decision();
        assert_eq!(ops::qk_matmul::q4_k_q8_k_avx2_available(), local);

        let mut seed = 0x1357_9bdf_2468_ace0u64;
        let x: Vec<f32> = (0..512)
            .map(|i| ((i * 37 % 101) as f32 - 50.0) * 0.013)
            .collect();
        let mut q8k = vec![ops::qk_matmul::BlockQ8K::ZERO; 2];
        ops::qk_matmul::quantize_row_q8_k(&x, &mut q8k).unwrap();
        let q4 = phase9_random_q4_k_blocks(2, &mut seed);
        let auto = ops::qk_matmul::vec_dot_q4_k_q8_k_with_kernel(
            &q4,
            &q8k,
            ops::qk_matmul::QKMatmulKernel::AutoAvx2,
        )
        .unwrap();
        let scalar = ops::qk_matmul::vec_dot_q4_k_q8_k_with_kernel(
            &q4,
            &q8k,
            ops::qk_matmul::QKMatmulKernel::Scalar,
        )
        .unwrap();
        assert_eq!(auto.to_bits(), scalar.to_bits());
    }

    /// Phase 9 Q4_K dequant AVX2 ISC-4: randomized exact-equality sweep. The
    /// scalar is the oracle; any AVX2 difference is a bug. Skipped when the
    /// host lacks AVX2 state (mirrors the 9.3 dot sweep).
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn phase9_avx2_q4_k_dequant_bit_identical_random_sweep() {
        if !ops::qk_matmul::q4_k_q8_k_avx2_available() {
            eprintln!("SKIP phase9_avx2_q4_k_dequant_bit_identical_random_sweep: AVX2 unavailable");
            return;
        }

        let mut seed = 0x85a3_08d3_243f_6a88u64;
        for case in 0..64 {
            // Generate 1..4 random Q4_K blocks.
            let blocks = 1 + (case % 4);
            let q4 = phase9_random_q4_k_blocks(blocks, &mut seed);
            for b in 0..blocks {
                let off = b * ops::q4_k::Q4_K_BLOCK_BYTES;
                let block = &q4[off..off + ops::q4_k::Q4_K_BLOCK_BYTES];
                let scalar = ops::q4_k::dequantize_q4_k(block).unwrap();
                let avx2 = ops::qk_matmul::dequantize_q4_k_with_kernel(
                    block,
                    ops::qk_matmul::QKMatmulKernel::AutoAvx2,
                )
                .unwrap();
                for i in 0..scalar.len() {
                    assert_eq!(
                        avx2[i].to_bits(),
                        scalar[i].to_bits(),
                        "case={case} block={b} lane={i}: scalar={} avx2={}",
                        scalar[i],
                        avx2[i]
                    );
                }
            }
        }
    }

    /// Phase 9 BUILD-9.1 ISC-1: hand-laid `block_q8_K` byte oracle.
    /// Inputs are `x = -q/127` with the first value forcing llama.cpp's
    /// `max=+1.0`, so `iscale=-127/max` yields exact quants and bsums.
    #[test]
    fn phase9_q8_k_quantizer_hand_laid_byte_oracle() {
        let mut input = [0.0f32; ops::qk_matmul::Q8_K_VALUES];
        let mut expected = [0u8; ops::qk_matmul::Q8_K_BLOCK_BYTES];
        expected[0..4].copy_from_slice(&0xbc01_0204u32.to_le_bytes()); // -1.0f32 / 127.0
        for idx in 0..ops::qk_matmul::Q8_K_VALUES {
            let q = phase9_q8_oracle_quant(idx);
            input[idx] = -(q as f32) / 127.0;
            expected[4 + idx] = q as u8;
        }
        let expected_bsums = [
            -1927i16, -1672, -1416, -1160, -904, -648, -392, -136, 120, 376, 632, 888, 1144, 1400,
            1656, 1912,
        ];
        for (idx, bsum) in expected_bsums.iter().enumerate() {
            let off = 4 + ops::qk_matmul::Q8_K_VALUES + idx * 2;
            expected[off..off + 2].copy_from_slice(&bsum.to_le_bytes());
        }

        let mut blocks = [ops::qk_matmul::BlockQ8K::ZERO; 1];
        ops::qk_matmul::quantize_row_q8_k(&input, &mut blocks).unwrap();
        let mut got = [0u8; ops::qk_matmul::Q8_K_BLOCK_BYTES];
        blocks[0].write_bytes(&mut got).unwrap();

        assert_eq!(got, expected);
    }

    /// Phase 9 BUILD-9.1 ISC-2/3: scalar q4_K x q8_K dot KAT. The reference
    /// helper below is raw-sourced from llama.cpp
    /// `74ade52741203e5c8f81eaf06a96cb1cfe15f2a3`
    /// `ggml_vec_dot_q4_K_q8_K_generic`; the input generator follows
    /// `tests/test-quantize-fns.cpp` (`0.1 + 2*cos(i + offset)`, N=4096).
    #[test]
    fn phase9_q4k_q8k_scalar_dot_llama_reference_kat() {
        const TEST_SIZE: usize = 32 * 128;
        const MAX_DOT_PRODUCT_ERROR: f32 = 0.02;

        let weights = phase9_generate_cosine_data(0.0, TEST_SIZE);
        let activations = phase9_generate_cosine_data(1.0, TEST_SIZE);
        let q4 = phase9_synthetic_q4_k_from_cosine(&weights);
        let mut q8 = vec![ops::qk_matmul::BlockQ8K::ZERO; TEST_SIZE / ops::qk_matmul::Q8_K_VALUES];
        ops::qk_matmul::quantize_row_q8_k(&activations, &mut q8).unwrap();

        let got = ops::qk_matmul::vec_dot_q4_k_q8_k(&q4, &q8).unwrap();
        let reference = llama_reference_vec_dot_q4_k_q8_k(&q4, &q8);
        let normalized_reference_delta = (got - reference).abs() / TEST_SIZE as f32;
        assert!(
            normalized_reference_delta <= MAX_DOT_PRODUCT_ERROR,
            "normalized llama-reference delta {normalized_reference_delta} > {MAX_DOT_PRODUCT_ERROR}: got {got}, reference {reference}"
        );

        let f32_reference = dot_product(&weights, &activations);
        let normalized_f32_delta = (reference - f32_reference).abs() / TEST_SIZE as f32;
        assert!(
            normalized_f32_delta <= MAX_DOT_PRODUCT_ERROR,
            "normalized f32-source delta {normalized_f32_delta} > {MAX_DOT_PRODUCT_ERROR}: reference {reference}, f32 {f32_reference}"
        );

        let mut matmul_out = [0.0f32; 1];
        ops::qk_matmul::matmul_q4_k_q8_k(&q8, &q4, 1, 1, TEST_SIZE, &mut matmul_out).unwrap();
        assert_eq!(matmul_out[0], got);
    }

    /// Phase 9 BUILD-9.3 ISC-1/2/3: the selectable AVX2 kernel is bit-identical
    /// to the scalar oracle on the 9.1 synthetic KAT. If the host lacks AVX2
    /// state support, `AutoAvx2` must fall back to scalar exactly.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn phase9_avx2_dot_bit_identical_to_scalar_kat() {
        const TEST_SIZE: usize = 32 * 128;

        let weights = phase9_generate_cosine_data(0.0, TEST_SIZE);
        let activations = phase9_generate_cosine_data(1.0, TEST_SIZE);
        let q4 = phase9_synthetic_q4_k_from_cosine(&weights);
        let mut q8 = vec![ops::qk_matmul::BlockQ8K::ZERO; TEST_SIZE / ops::qk_matmul::Q8_K_VALUES];
        ops::qk_matmul::quantize_row_q8_k(&activations, &mut q8).unwrap();

        let scalar = ops::qk_matmul::vec_dot_q4_k_q8_k(&q4, &q8).unwrap();
        let forced_scalar = ops::qk_matmul::vec_dot_q4_k_q8_k_with_kernel(
            &q4,
            &q8,
            ops::qk_matmul::QKMatmulKernel::Scalar,
        )
        .unwrap();
        let auto = ops::qk_matmul::vec_dot_q4_k_q8_k_with_kernel(
            &q4,
            &q8,
            ops::qk_matmul::QKMatmulKernel::AutoAvx2,
        )
        .unwrap();

        assert_eq!(forced_scalar.to_bits(), scalar.to_bits());
        assert_eq!(
            auto.to_bits(),
            scalar.to_bits(),
            "AutoAvx2 diverged from scalar; avx2_available={}",
            ops::qk_matmul::q4_k_q8_k_avx2_available()
        );

        let mut scalar_out = [0.0f32; 1];
        let mut auto_out = [0.0f32; 1];
        ops::qk_matmul::matmul_q4_k_q8_k(&q8, &q4, 1, 1, TEST_SIZE, &mut scalar_out).unwrap();
        ops::qk_matmul::matmul_q4_k_q8_k_with_kernel(
            &q8,
            &q4,
            1,
            1,
            TEST_SIZE,
            &mut auto_out,
            ops::qk_matmul::QKMatmulKernel::AutoAvx2,
        )
        .unwrap();
        assert_eq!(auto_out[0].to_bits(), scalar_out[0].to_bits());
    }

    /// Phase 9 BUILD-9.3 ISC-2: randomized exact-equality sweep. The scalar
    /// integer path is the oracle; any AVX2 difference is a bug.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn phase9_avx2_dot_bit_identical_random_sweep() {
        if !ops::qk_matmul::q4_k_q8_k_avx2_available() {
            eprintln!("SKIP phase9_avx2_dot_bit_identical_random_sweep: AVX2 unavailable");
            return;
        }

        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        for case in 0..64 {
            let blocks = 1 + (case % 4);
            let q4 = phase9_random_q4_k_blocks(blocks, &mut seed);
            let q8 = phase9_random_q8_k_blocks(blocks, &mut seed);
            let scalar = ops::qk_matmul::vec_dot_q4_k_q8_k(&q4, &q8).unwrap();
            let avx2 = ops::qk_matmul::vec_dot_q4_k_q8_k_with_kernel(
                &q4,
                &q8,
                ops::qk_matmul::QKMatmulKernel::AutoAvx2,
            )
            .unwrap();
            assert_eq!(
                avx2.to_bits(),
                scalar.to_bits(),
                "case={case} blocks={blocks} scalar={scalar} avx2={avx2}"
            );
        }
    }

    /// Phase 9 BUILD-9.3 ISC-2: matrix wrapper remains bit-identical when the
    /// AVX2 dot is selected explicitly by the host test.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn phase9_avx2_matmul_bit_identical_random_sweep() {
        if !ops::qk_matmul::q4_k_q8_k_avx2_available() {
            eprintln!("SKIP phase9_avx2_matmul_bit_identical_random_sweep: AVX2 unavailable");
            return;
        }

        let mut seed = 0x243f_6a88_85a3_08d3u64;
        for case in 0..16 {
            let blocks_per_dot = 1 + (case % 3);
            let rows = 2usize;
            let cols = 3usize;
            let shared = blocks_per_dot * ops::qk_matmul::Q8_K_VALUES;
            let lhs_q8 = phase9_random_q8_k_blocks(rows * blocks_per_dot, &mut seed);
            let rhs_q4 = phase9_random_q4_k_blocks(cols * blocks_per_dot, &mut seed);
            let mut scalar = [0.0f32; 6];
            let mut avx2 = [0.0f32; 6];
            ops::qk_matmul::matmul_q4_k_q8_k(&lhs_q8, &rhs_q4, rows, cols, shared, &mut scalar)
                .unwrap();
            ops::qk_matmul::matmul_q4_k_q8_k_with_kernel(
                &lhs_q8,
                &rhs_q4,
                rows,
                cols,
                shared,
                &mut avx2,
                ops::qk_matmul::QKMatmulKernel::AutoAvx2,
            )
            .unwrap();
            for idx in 0..scalar.len() {
                assert_eq!(
                    avx2[idx].to_bits(),
                    scalar[idx].to_bits(),
                    "case={case} idx={idx} scalar={} avx2={}",
                    scalar[idx],
                    avx2[idx]
                );
            }
        }
    }

    /// Phase 9 BUILD-9.3+ (q8_K quantize AVX2) ISC-1/2: the AVX2 block
    /// quantizer is bit-identical to the scalar oracle on hand-laid trap
    /// cases. Runs whether or not the host has AVX2: when AVX2 is unavailable
    /// `AutoAvx2` falls back to scalar (trivially equal); when available the
    /// AVX2 path must be exact. Covers the discriminator traps: all-zero /
    /// `max==0` (no div-by-zero — match the scalar `BlockQ8K::ZERO` arm),
    /// negative-zero, uniform, alternating ±127 clamp boundaries, negative-max
    /// sign, round-half ties (`max=254` ⇒ `iscale=-127/254=-0.5` ⇒
    /// half-integer products exercise the magic-number round-to-even
    /// direction), and a multi-block row.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn phase9_avx2_q8_k_quantize_bit_identical_hand_laid() {
        let n = ops::qk_matmul::Q8_K_VALUES;

        // (a) sequential 0..255 — general case, positive max at the last lane.
        let mut a = vec![0.0f32; n];
        for i in 0..n {
            a[i] = i as f32;
        }
        phase9_assert_q8_k_quantize_avx2_eq_scalar(&a, "sequential_0_255");

        // (b) all zeros — amax == 0.0 zero-block trap (no div-by-zero).
        phase9_assert_q8_k_quantize_avx2_eq_scalar(&vec![0.0f32; n], "all_zeros");

        // (c) all negative zero — abs(-0.0) == 0.0, same zero-block branch.
        phase9_assert_q8_k_quantize_avx2_eq_scalar(&vec![-0.0f32; n], "all_neg_zero");

        // (d) uniform nonzero value.
        phase9_assert_q8_k_quantize_avx2_eq_scalar(&vec![3.5f32; n], "uniform_3p5");

        // (e) alternating ±127 extremes — iscale == -1.0, products hit ±127
        //     exactly (the clamp boundary; no clamp fires).
        let mut e = vec![0.0f32; n];
        for i in 0..n {
            e[i] = if i % 2 == 0 { 127.0 } else { -127.0 };
        }
        phase9_assert_q8_k_quantize_avx2_eq_scalar(&e, "alternating_pm127");

        // (f) negative-max — largest-abs element is negative: max = -100.0,
        //     iscale = -127 / -100 = +1.27 (positive). The sign trap.
        let mut f = vec![50.0f32; n];
        f[0] = -100.0;
        phase9_assert_q8_k_quantize_avx2_eq_scalar(&f, "negative_max");

        // (g) round-half ties — max = 254.0 gives iscale = -127/254 = -0.5
        //     exactly (127/254 == 1/2, and both are exactly representable).
        //     Odd-negative inputs produce half-integer products (0.5, 1.5,
        //     2.5, ...); the magic-number nearest_int must round ties the same
        //     direction as the scalar (round-to-even). The earlier A/B tripped
        //     here by using `_mm256_cvtps_epi32` which can diverge at ties.
        let mut g = vec![0.0f32; n];
        g[0] = 254.0;
        for k in 0..9usize {
            g[1 + k] = -((2 * k + 1) as f32);
        }
        phase9_assert_q8_k_quantize_avx2_eq_scalar(&g, "round_half_ties");

        // (h) two-block row — exercises the quantize_row dispatch loop with >1
        //     block and a mixed zero-then-nonzero pattern.
        let mut h = vec![0.0f32; 2 * n];
        for i in 0..n {
            h[n + i] = i as f32;
        }
        phase9_assert_q8_k_quantize_avx2_eq_scalar(&h, "two_block_mixed");
    }

    /// Phase 9 BUILD-9.3+ (q8_K quantize AVX2) ISC-2: randomized exact-equality
    /// sweep. The scalar integer path is the oracle; any AVX2 difference is a
    /// bug. Skipped when the host lacks AVX2 state support (mirrors the 9.3
    /// dot sweep).
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn phase9_avx2_q8_k_quantize_bit_identical_random_sweep() {
        if !ops::qk_matmul::q4_k_q8_k_avx2_available() {
            eprintln!(
                "SKIP phase9_avx2_q8_k_quantize_bit_identical_random_sweep: AVX2 unavailable"
            );
            return;
        }

        let mut seed = 0xd1b5_4a5d_60cf_a1c7u64;
        let n = ops::qk_matmul::Q8_K_VALUES;
        for case in 0..128 {
            let blocks = 1 + (case % 8);
            let mut input = vec![0.0f32; blocks * n];
            for v in &mut input {
                // Span normal ranges including small magnitudes and ±extremes;
                // occasionally inject zeros to exercise the amax == 0 branch.
                let kind = phase9_next_u32(&mut seed) % 16;
                *v = match kind {
                    0 => 0.0,
                    1 => -0.0,
                    2 => 127.0,
                    3 => -127.0,
                    _ => ((phase9_next_u32(&mut seed) % 511) as i32 - 255) as f32,
                };
            }
            phase9_assert_q8_k_quantize_avx2_eq_scalar(&input, &format!("random_{case}"));
        }
    }

    /// Phase 8 BUILD-8.3 integration ISC-1: the tensor-level dequant seam
    /// consumes GGUF tensor metadata plus raw imported bytes and produces f32
    /// buffers for both banked formats this build wires, Q4_K and Q8_0.
    #[test]
    fn gguf_dequant_isc1_q4_and_q8_tensor_paths_kat() {
        let mut q4 = [0u8; BLOCK_Q4_K_SIZE];
        q4[0..2].copy_from_slice(&0x3c00u16.to_le_bytes()); // f16(1.0)
        q4[4..8].copy_from_slice(&[1, 1, 1, 1]);
        q4[12..16].copy_from_slice(&[1, 1, 1, 1]);
        for idx in 0..(QK_K / 2) {
            q4[16 + idx] = (((idx + 1) & 0x0f) << 4 | ((15usize.wrapping_sub(idx)) & 0x0f)) as u8;
        }
        let q4_tensor = gguf_tensor("synthetic.q4", GgmlType::Q4K, &[QK_K as u64]);
        let q4_dequant =
            gguf_dequant_logic::dequantize_tensor_data_to_f32(&q4_tensor, &q4).unwrap();
        let mut q4_expected = Vec::with_capacity(QK_K);
        for chunk in q4[16..].chunks_exact(32) {
            q4_expected.extend(chunk.iter().map(|byte| (byte & 0x0f) as f32));
            q4_expected.extend(chunk.iter().map(|byte| (byte >> 4) as f32));
        }
        assert_eq!(q4_dequant, q4_expected);

        let mut q8 = [0u8; BLOCK_Q8_0_SIZE];
        q8[0..2].copy_from_slice(&0x4000u16.to_le_bytes()); // f16(2.0)
        for idx in 0..QK8_0 {
            q8[2 + idx] = (idx as i8 - 16) as u8;
        }
        let q8_tensor = gguf_tensor("synthetic.q8", GgmlType::Q8_0, &[QK8_0 as u64]);
        let q8_dequant =
            gguf_dequant_logic::dequantize_tensor_data_to_f32(&q8_tensor, &q8).unwrap();
        let q8_expected: Vec<f32> = (0..QK8_0)
            .map(|idx| (idx as i8 - 16) as f32 * 2.0)
            .collect();
        assert_eq!(q8_dequant, q8_expected);
    }

    /// Phase 8 BUILD-8.4 integration ISC-1 support: the real Qwen2.5 Q4_K_M
    /// FFN/projection tensors include GGML Q5_0 blocks. This pins the 22-byte
    /// `d + qh + qs` layout and signed 5-bit reconstruction before those real
    /// weights are used by the FFN/RoPE oracles.
    #[test]
    fn gguf_dequant_isc1_q5_0_tensor_path_kat() {
        const BLOCK_Q5_0_SIZE: usize = 22;
        let mut q5 = [0u8; BLOCK_Q5_0_SIZE];
        q5[0..2].copy_from_slice(&0x3c00u16.to_le_bytes()); // f16(1.0)
        q5[2..6].copy_from_slice(&0xaaaa_5555u32.to_le_bytes());
        for idx in 0..16 {
            q5[6 + idx] = ((((idx + 1) & 0x0f) << 4) | ((15usize.wrapping_sub(idx)) & 0x0f)) as u8;
        }
        let q5_tensor = gguf_tensor("synthetic.q5_0", GgmlType::Q5_0, &[32]);
        let q5_dequant =
            gguf_dequant_logic::dequantize_tensor_data_to_f32(&q5_tensor, &q5).unwrap();
        let q5_expected = llama_reference_dequant_q5_0(&q5);

        assert_eq!(q5_dequant, q5_expected);
        assert_eq!(&q5_dequant[0..4], &[15.0, -2.0, 13.0, -4.0]);
        assert_eq!(&q5_dequant[16..20], &[-15.0, 2.0, -13.0, 4.0]);
    }

    /// OT-16 B3 real Qwen3.5 Q4_K_M support: the selected GGUF contains q5_K
    /// tensors, whose 176-byte `d/dmin + scales + qh + qs` block layout differs
    /// from the older q5_0 path. Pin the llama.cpp 5f04dc7 `dequantize_row_q5_K`
    /// reconstruction independently of the raw-block importer/runtime path.
    #[test]
    fn gguf_dequant_isc1_q5_k_tensor_path_kat() {
        const BLOCK_Q5_K_SIZE: usize = 176;
        let mut q5 = [0u8; BLOCK_Q5_K_SIZE];
        q5[0..2].copy_from_slice(&0x3c00u16.to_le_bytes()); // d = f16(1.0)
        q5[2..4].copy_from_slice(&0x3c00u16.to_le_bytes()); // dmin = f16(1.0)
        q5[4..16].copy_from_slice(&[2, 3, 4, 5, 1, 2, 3, 4, 0x46, 0x57, 0x68, 0x79]);
        for idx in 0..32 {
            q5[16 + idx] = match idx % 4 {
                0 => 0b0101_0101,
                1 => 0b1010_1010,
                2 => 0xff,
                _ => 0x00,
            };
        }
        for idx in 0..128 {
            q5[48 + idx] = ((((idx + 10) & 0x0f) << 4) | ((idx + 5) & 0x0f)) as u8;
        }

        let q5_tensor = gguf_tensor("synthetic.q5_K", GgmlType::Q5K, &[QK_K as u64]);
        let q5_dequant =
            gguf_dequant_logic::dequantize_tensor_data_to_f32(&q5_tensor, &q5).unwrap();
        let q5_expected = llama_reference_dequant_q5_k(&q5);

        assert_eq!(q5_dequant, q5_expected);
        assert_eq!(q5_dequant[0], 41.0);
        assert_eq!(q5_dequant[1], 11.0);
        assert_eq!(q5_dequant[32], 28.0);
        assert_eq!(q5_dequant[33], 79.0);
    }

    /// Phase 8 BUILD-8.5 support: the real Qwen2.5 Q4_K_M file contains
    /// q6_K ffn_down tensors. This pins the 210-byte `ql + qh + scales + d`
    /// layout from llama.cpp 5f04dc7 `block_q6_K`/`dequantize_row_q6_K`.
    #[test]
    fn gguf_dequant_isc1_q6_k_tensor_path_kat() {
        const BLOCK_Q6_K_SIZE: usize = 210;
        let mut q6 = [0u8; BLOCK_Q6_K_SIZE];
        for (idx, scale) in q6[192..208].iter_mut().enumerate() {
            *scale = (idx + 1) as u8;
        }
        q6[0] = 0b1010_0101;
        q6[32] = 0b0011_1100;
        q6[16] = 0b0101_0001;
        q6[48] = 0b1110_0010;
        q6[64] = 0b0110_0111;
        q6[96] = 0b1001_0100;
        q6[128] = 0b11_10_01_00;
        q6[144] = 0b00_01_10_11;
        q6[160] = 0b01_00_11_10;
        q6[208..210].copy_from_slice(&0x3c00u16.to_le_bytes()); // f16(1.0)

        let q6_tensor = gguf_tensor("synthetic.q6_K", GgmlType::Q6K, &[256]);
        let q6_dequant =
            gguf_dequant_logic::dequantize_tensor_data_to_f32(&q6_tensor, &q6).unwrap();
        let q6_expected = llama_reference_dequant_q6_k(&q6);

        assert_eq!(q6_dequant, q6_expected);
        assert_eq!(q6_dequant[0], -27.0);
        assert_eq!(q6_dequant[32], -12.0);
        assert_eq!(q6_dequant[64], 50.0);
        assert_eq!(q6_dequant[96], 133.0);
        assert_eq!(q6_dequant[16], 17.0 * 2.0);
        assert_eq!(q6_dequant[48], 2.0 * 4.0);
        assert_eq!(q6_dequant[80], -11.0 * 6.0);
        assert_eq!(q6_dequant[112], -18.0 * 8.0);
        assert_eq!(q6_dequant[128], 7.0 * 9.0);
        assert_eq!(q6_dequant[160], 20.0 * 11.0);
        assert_eq!(q6_dequant[192], -26.0 * 13.0);
        assert_eq!(q6_dequant[224], -7.0 * 15.0);
    }

    /// OT-28 F1 ISC-4: each supported quantized weight type takes the new
    /// dtype-dispatched path without materializing the full dequantized RHS.
    /// The dot product uses a 127-hot activation so the q8 activation quantizer
    /// is exact and the expected output is the hand-encoded dequant value times
    /// 127 for each column, independent of any writer or round-trip fixture.
    #[cfg(feature = "phase9-real-quant")]
    #[test]
    fn forward_f1_mixed_quant_direct_matmul_path_kat() {
        assert_mixed_quant_path(
            "q4_K",
            TensorDtype::Q4K,
            QK_K,
            &[mixed_q4_k_block(5), mixed_q4_k_block(9)].concat(),
            5.0,
            9.0,
        );
        assert_mixed_quant_path(
            "q5_0",
            TensorDtype::Q5_0,
            32,
            &[mixed_q5_0_block(3), mixed_q5_0_block(-5)].concat(),
            3.0,
            -5.0,
        );
        assert_mixed_quant_path(
            "q5_K",
            TensorDtype::Q5K,
            QK_K,
            &[mixed_q5_k_block(5), mixed_q5_k_block(9)].concat(),
            5.0,
            9.0,
        );
        assert_mixed_quant_path(
            "q6_K",
            TensorDtype::Q6K,
            QK_K,
            &[mixed_q6_k_block(5), mixed_q6_k_block(-7)].concat(),
            5.0,
            -7.0,
        );
        assert_mixed_quant_path(
            "q8_0",
            TensorDtype::Q8_0,
            QK8_0,
            &[mixed_q8_0_block(11), mixed_q8_0_block(-13)].concat(),
            11.0,
            -13.0,
        );
    }

    /// F4 ISC-4(a): hand-laid native quant dot byte-oracles. The activation is
    /// 127-hot so both q8_K and q8_0 quantizers encode it exactly (`d = 1` or
    /// `d = -1`, one signed lane at +/-127). Scalar and AutoAvx2 must agree
    /// bit-for-bit and hit the hand-computed dot.
    #[cfg(feature = "phase9-real-quant")]
    #[test]
    fn forward_f4_native_quant_dot_byte_oracles_scalar_and_auto() {
        let mut lhs_q8_0_src = [0.0f32; ops::qk_matmul::Q8_0_VALUES];
        lhs_q8_0_src[0] = 127.0;
        let mut lhs_q8_0 = [ops::qk_matmul::BlockQ8Zero::ZERO; 1];
        ops::qk_matmul::quantize_row_q8_0(&lhs_q8_0_src, &mut lhs_q8_0).unwrap();

        let q5 = mixed_q5_0_block(3);
        let q5_scalar = ops::qk_matmul::vec_dot_q5_0_q8_0(&q5, &lhs_q8_0).unwrap();
        let q5_auto = ops::qk_matmul::vec_dot_q5_0_q8_0_with_kernel(
            &q5,
            &lhs_q8_0,
            ops::qk_matmul::QKMatmulKernel::AutoAvx2,
        )
        .unwrap();
        assert_eq!(q5_scalar.to_bits(), (3.0f32 * 127.0).to_bits());
        assert_eq!(q5_auto.to_bits(), q5_scalar.to_bits());

        let q8 = mixed_q8_0_block(11);
        let q8_scalar = ops::qk_matmul::vec_dot_q8_0_q8_0(&q8, &lhs_q8_0).unwrap();
        let q8_auto = ops::qk_matmul::vec_dot_q8_0_q8_0_with_kernel(
            &q8,
            &lhs_q8_0,
            ops::qk_matmul::QKMatmulKernel::AutoAvx2,
        )
        .unwrap();
        assert_eq!(q8_scalar.to_bits(), (11.0f32 * 127.0).to_bits());
        assert_eq!(q8_auto.to_bits(), q8_scalar.to_bits());

        let mut lhs_q8_k_src = [0.0f32; ops::qk_matmul::Q8_K_VALUES];
        lhs_q8_k_src[0] = 127.0;
        let mut lhs_q8_k = [ops::qk_matmul::BlockQ8K::ZERO; 1];
        ops::qk_matmul::quantize_row_q8_k(&lhs_q8_k_src, &mut lhs_q8_k).unwrap();

        let q4 = mixed_q4_k_block(5);
        let q4_scalar = ops::qk_matmul::vec_dot_q4_k_q8_k(&q4, &lhs_q8_k).unwrap();
        let q4_auto = ops::qk_matmul::vec_dot_q4_k_q8_k_with_kernel(
            &q4,
            &lhs_q8_k,
            ops::qk_matmul::QKMatmulKernel::AutoAvx2,
        )
        .unwrap();
        assert_eq!(q4_scalar.to_bits(), (5.0f32 * 127.0).to_bits());
        assert_eq!(q4_auto.to_bits(), q4_scalar.to_bits());

        let q5_k = mixed_q5_k_block(5);
        let q5_k_scalar = ops::qk_matmul::vec_dot_q5_k_q8_k(&q5_k, &lhs_q8_k).unwrap();
        let q5_k_auto = ops::qk_matmul::vec_dot_q5_k_q8_k_with_kernel(
            &q5_k,
            &lhs_q8_k,
            ops::qk_matmul::QKMatmulKernel::AutoAvx2,
        )
        .unwrap();
        assert_eq!(q5_k_scalar.to_bits(), (5.0f32 * 127.0).to_bits());
        assert_eq!(q5_k_auto.to_bits(), q5_k_scalar.to_bits());

        let q6 = mixed_q6_k_block(-7);
        let q6_scalar = ops::qk_matmul::vec_dot_q6_k_q8_k(&q6, &lhs_q8_k).unwrap();
        let q6_auto = ops::qk_matmul::vec_dot_q6_k_q8_k_with_kernel(
            &q6,
            &lhs_q8_k,
            ops::qk_matmul::QKMatmulKernel::AutoAvx2,
        )
        .unwrap();
        assert_eq!(q6_scalar.to_bits(), (-7.0f32 * 127.0).to_bits());
        assert_eq!(q6_auto.to_bits(), q6_scalar.to_bits());
    }

    /// F4 ISC-4(b): native kernels transfer the OT-30 dequant-to-f32 reference
    /// result on Qwen-shaped widths when the q8 activation quantization is
    /// exact. This keeps the reference path as a separate correctness anchor.
    #[cfg(feature = "phase9-real-quant")]
    #[test]
    fn forward_f4_native_vs_dequant_reference_qwen_shape_kat() {
        assert_native_matches_dequant_reference(
            "q5_0-hidden",
            TensorDtype::Q5_0,
            forward_logic::QWEN25_05B_HIDDEN_SIZE,
            repeated_blocks(
                mixed_q5_0_block(3),
                forward_logic::QWEN25_05B_HIDDEN_SIZE / 32,
                2,
            ),
        );
        assert_native_matches_dequant_reference(
            "q8_0-hidden",
            TensorDtype::Q8_0,
            forward_logic::QWEN25_05B_HIDDEN_SIZE,
            repeated_blocks(
                mixed_q8_0_block(-13),
                forward_logic::QWEN25_05B_HIDDEN_SIZE / QK8_0,
                2,
            ),
        );
        assert_native_matches_dequant_reference(
            "q4_K-intermediate",
            TensorDtype::Q4K,
            forward_logic::QWEN25_05B_INTERMEDIATE_SIZE,
            repeated_blocks(
                mixed_q4_k_block(5),
                forward_logic::QWEN25_05B_INTERMEDIATE_SIZE / QK_K,
                2,
            ),
        );
        assert_native_matches_dequant_reference(
            "q5_K-qwen35-hidden",
            TensorDtype::Q5K,
            QK_K * 4,
            repeated_blocks(mixed_q5_k_block(5), 4, 2),
        );
        assert_native_matches_dequant_reference(
            "q6_K-intermediate",
            TensorDtype::Q6K,
            forward_logic::QWEN25_05B_INTERMEDIATE_SIZE,
            repeated_blocks(
                mixed_q6_k_block(-7),
                forward_logic::QWEN25_05B_INTERMEDIATE_SIZE / QK_K,
                2,
            ),
        );
    }

    #[cfg(feature = "phase9-real-quant")]
    fn assert_mixed_quant_path(
        label: &str,
        dtype: TensorDtype,
        rows: usize,
        data: &[u8],
        expected_col0: f32,
        expected_col1: f32,
    ) {
        let tensor = gguf_tensor(
            &format!("forward_f1.{label}"),
            tensor_dtype_to_ggml(dtype),
            &[rows as u64, 2],
        );
        let dequant = gguf_dequant_logic::dequantize_tensor_data_to_f32(&tensor, data).unwrap();
        assert_eq!(dequant[0], expected_col0, "{label} dequant col0");
        assert_eq!(dequant[rows], expected_col1, "{label} dequant col1");

        let mut lhs = vec![0.0f32; rows];
        lhs[0] = 127.0;
        let got =
            forward_logic::matmul_vec_mixed_quant_for_test(&lhs, 1, rows, dtype, data, 2).unwrap();
        assert_eq!(
            got,
            vec![expected_col0 * 127.0, expected_col1 * 127.0],
            "{label} matmul"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // OT-34: threaded-matmul column-partition DETERMINISM (ISC-2)
    //
    // The load-bearing artifact: the column-partitioned matmul must be
    // byte-identical to the single-threaded whole-matrix call at EVERY partition
    // count. This proves I-1 (result invariance) holds under parallelism — a
    // partition bug that swaps/drops columns or (the trap) splits the reduction
    // axis would diverge a bit and fail here. Pre-seeded from Prometheus's 6-trap
    // determinism bank (RESEARCH-2026-07-04-...-FINDINGS.md): the varying-partition
    // sweep directly falsifies trap 1 (float reassociation) + trap 2
    // (thread-count-dependent tiling). UNGATED so it runs in the default
    // `cargo test --workspace` close-gate step (it drives the partition helpers
    // directly, so it needs neither `phase9-real-quant` nor the cross-core
    // executor). The block builders replicate the verified layouts of the
    // `phase9-real-quant`-gated `mixed_*_block` helpers, with a per-column-varying
    // live value so distinct columns catch a mis-assembled output.
    // ─────────────────────────────────────────────────────────────────────────

    fn ot34_q4_k_block(v: u8) -> Vec<u8> {
        let mut block = vec![0u8; ops::q4_k::Q4_K_BLOCK_BYTES];
        block[0..2].copy_from_slice(&0x3c00u16.to_le_bytes()); // d = 1.0 (f16)
        block[4..8].copy_from_slice(&[1, 1, 1, 1]);
        block[12..16].copy_from_slice(&[1, 1, 1, 1]);
        block[16] = v & 0x0f;
        block[17] = v.rotate_left(3) & 0x0f;
        block
    }

    fn ot34_q6_k_block(v: u8) -> Vec<u8> {
        let mut block = vec![0u8; ops::qk_matmul::Q6_K_BLOCK_BYTES];
        for high in &mut block[128..192] {
            *high = 0xaa;
        }
        for scale in &mut block[192..208] {
            *scale = 1;
        }
        block[208..210].copy_from_slice(&0x3c00u16.to_le_bytes()); // d = 1.0 (f16)
        block[0] = v & 0x0f;
        block[1] = v.rotate_left(2) & 0x0f;
        block[128] = (block[128] & !0x03) | ((v >> 4) & 0x03);
        block
    }

    fn ot34_q5_0_block(v: u8) -> Vec<u8> {
        let mut block = vec![0u8; ops::qk_matmul::Q5_0_BLOCK_BYTES];
        block[0..2].copy_from_slice(&0x3c00u16.to_le_bytes()); // d = 1.0 (f16)
        block[2..6].copy_from_slice(&u32::MAX.to_le_bytes());
        block[6] = v & 0x0f;
        block[7] = v.rotate_left(1) & 0x0f;
        block
    }

    fn ot34_q8_0_block(v: u8) -> Vec<u8> {
        let mut block = vec![0u8; ops::qk_matmul::Q8_0_BLOCK_BYTES];
        block[0..2].copy_from_slice(&0x3c00u16.to_le_bytes()); // d = 1.0 (f16)
        block[2] = v;
        block[3] = v.wrapping_mul(3).wrapping_add(1);
        block
    }

    fn ot34_q5_k_block(v: u8) -> Vec<u8> {
        let mut block = vec![0u8; ops::qk_matmul::Q5_K_BLOCK_BYTES];
        block[0..2].copy_from_slice(&0x3c00u16.to_le_bytes());
        block[4] = 1;
        block[48] = v & 0x0f;
        if v & 0x10 != 0 {
            block[16] = 1;
        }
        block
    }

    /// Column-major quantized weight bytes: `cols` columns × `bpd` blocks/column,
    /// each block's live value varying with `(col, block)` so columns differ.
    fn ot34_weights(kind: forward_logic::MatmulPartitionKind, bpd: usize, cols: usize) -> Vec<u8> {
        use forward_logic::MatmulPartitionKind::*;
        let mut out = Vec::new();
        for c in 0..cols {
            for b in 0..bpd {
                let v = ((c * 7 + b * 3 + 1) & 0xff) as u8;
                let block = match kind {
                    Q4K => ot34_q4_k_block(v),
                    Q5K => ot34_q5_k_block(v),
                    Q6K => ot34_q6_k_block(v),
                    Q5_0 => ot34_q5_0_block(v),
                    Q8_0 => ot34_q8_0_block(v),
                };
                out.extend_from_slice(&block);
            }
        }
        out
    }

    /// A deterministic non-trivial f32 activation of length `shared`.
    fn ot34_activation(shared: usize) -> Vec<f32> {
        (0..shared)
            .map(|i| ((i % 17) as f32 - 8.0) * 0.5 + 0.3)
            .collect()
    }

    fn assert_q8k_partition_invariant(
        kind: forward_logic::MatmulPartitionKind,
        bpd: usize,
        cols: usize,
    ) {
        let block_values = ops::qk_matmul::Q8_K_VALUES;
        let shared = bpd * block_values;
        let rhs = ot34_weights(kind, bpd, cols);
        let lhs = ot34_activation(shared);
        let mut lhs_q8 = vec![ops::qk_matmul::BlockQ8K::ZERO; bpd];
        ops::qk_matmul::quantize_row_q8_k(&lhs, &mut lhs_q8).unwrap();

        // Reference: one partition == the whole-matrix call.
        let mut reference = vec![0.0f32; cols];
        forward_logic::matmul_q8k_columns_seq(kind, &lhs_q8, &rhs, cols, shared, 1, &mut reference)
            .unwrap();

        // ISC-4 kill-switch equality: the OT-34 dispatch at parts=1 (the
        // feature-off / thread_count=1 behavior) is BIT-FOR-BIT the LITERAL
        // pre-OT-34 kernel call. `raw` calls `qk_matmul::matmul_*_with_kernel`
        // directly (exactly what `forward_logic` did before OT-34), so this is a
        // non-circular proof that the wrapper adds zero bits.
        let kernel = ops::qk_matmul::QKMatmulKernel::AutoAvx2;
        let mut raw = vec![0.0f32; cols];
        match kind {
            forward_logic::MatmulPartitionKind::Q4K => {
                ops::qk_matmul::matmul_q4_k_q8_k_with_kernel(
                    &lhs_q8, &rhs, 1, cols, shared, &mut raw, kernel,
                )
            }
            forward_logic::MatmulPartitionKind::Q6K => {
                ops::qk_matmul::matmul_q6_k_q8_k_with_kernel(
                    &lhs_q8, &rhs, 1, cols, shared, &mut raw, kernel,
                )
            }
            _ => unreachable!("q8k invariant helper only handles Q4K/Q6K"),
        }
        .unwrap();
        for i in 0..cols {
            assert_eq!(
                reference[i].to_bits(),
                raw[i].to_bits(),
                "{kind:?} parts=1 vs literal kernel col {i}"
            );
        }

        // Every partition count is byte-identical to the reference.
        for &parts in &[2usize, 3, 4, 5, 7, 8, cols] {
            let mut out = vec![0.0f32; cols];
            forward_logic::matmul_q8k_columns_seq(
                kind, &lhs_q8, &rhs, cols, shared, parts, &mut out,
            )
            .unwrap();
            for i in 0..cols {
                assert_eq!(
                    out[i].to_bits(),
                    reference[i].to_bits(),
                    "{kind:?} parts={parts} col={i} diverged (nondeterministic partition!)"
                );
            }
        }
    }

    fn assert_q8_0_partition_invariant(
        kind: forward_logic::MatmulPartitionKind,
        bpd: usize,
        cols: usize,
    ) {
        let block_values = ops::qk_matmul::Q8_0_VALUES;
        let shared = bpd * block_values;
        let rhs = ot34_weights(kind, bpd, cols);
        let lhs = ot34_activation(shared);
        let mut lhs_q8 = vec![ops::qk_matmul::BlockQ8Zero::ZERO; bpd];
        ops::qk_matmul::quantize_row_q8_0(&lhs, &mut lhs_q8).unwrap();

        let mut reference = vec![0.0f32; cols];
        forward_logic::matmul_q8_0_columns_seq(
            kind,
            &lhs_q8,
            &rhs,
            cols,
            shared,
            1,
            &mut reference,
        )
        .unwrap();

        // ISC-4 kill-switch equality: parts=1 == the literal pre-OT-34 kernel call.
        let kernel = ops::qk_matmul::QKMatmulKernel::AutoAvx2;
        let mut raw = vec![0.0f32; cols];
        match kind {
            forward_logic::MatmulPartitionKind::Q5_0 => {
                ops::qk_matmul::matmul_q5_0_q8_0_with_kernel(
                    &lhs_q8, &rhs, 1, cols, shared, &mut raw, kernel,
                )
            }
            forward_logic::MatmulPartitionKind::Q8_0 => {
                ops::qk_matmul::matmul_q8_0_q8_0_with_kernel(
                    &lhs_q8, &rhs, 1, cols, shared, &mut raw, kernel,
                )
            }
            _ => unreachable!("q8_0 invariant helper only handles Q5_0/Q8_0"),
        }
        .unwrap();
        for i in 0..cols {
            assert_eq!(
                reference[i].to_bits(),
                raw[i].to_bits(),
                "{kind:?} parts=1 vs literal kernel col {i}"
            );
        }

        for &parts in &[2usize, 3, 4, 5, 7, 8, cols] {
            let mut out = vec![0.0f32; cols];
            forward_logic::matmul_q8_0_columns_seq(
                kind, &lhs_q8, &rhs, cols, shared, parts, &mut out,
            )
            .unwrap();
            for i in 0..cols {
                assert_eq!(
                    out[i].to_bits(),
                    reference[i].to_bits(),
                    "{kind:?} parts={parts} col={i} diverged (nondeterministic partition!)"
                );
            }
        }
    }

    /// OT-34 ISC-2: the column-partitioned matmul is byte-identical to the
    /// single-threaded whole-matrix call at every partition count, for all four
    /// quantized weight families. Widths chosen so partitions do NOT divide evenly
    /// (remainder distribution exercised) and a wide `.neom`-shaped case is hit.
    #[test]
    fn ot34_threaded_matmul_column_partition_is_byte_identical() {
        use forward_logic::MatmulPartitionKind::*;
        for &cols in &[13usize, 32, 151] {
            assert_q8k_partition_invariant(Q4K, 2, cols);
            assert_q8k_partition_invariant(Q6K, 2, cols);
            assert_q8_0_partition_invariant(Q5_0, 2, cols);
            assert_q8_0_partition_invariant(Q8_0, 2, cols);
        }
        // Single-block contraction (bpd=1) too — the lm_head / narrow-K shape.
        assert_q8k_partition_invariant(Q4K, 1, 64);
        assert_q8_0_partition_invariant(Q8_0, 1, 64);
    }

    /// OT-34 F1 (AAG bounce): the fork-join barrier decision must be fail-STOP.
    /// The load-bearing soundness rule: a slot an AP has *claimed* (`pending ==
    /// false`) that is not yet `done` must NEVER yield `Steal` or any action the
    /// BSP maps to a return-that-frees-`out` — it may only `Spin` (wait) or, past
    /// the hard limit, `Halt` (fail-stop). Stealing is reserved for the *unclaimed*
    /// case, where no executor is live. This exhaustively sweeps the 4-bool input
    /// space and asserts that invariant, converting F1 from reasoned to
    /// regression-guarded without needing a real AP.
    #[test]
    fn ot34_barrier_action_is_fail_stop_never_frees_under_live_executor() {
        use super::par_matmul_logic::{BarrierAction, barrier_action};
        for &done in &[false, true] {
            for &pending in &[false, true] {
                for &past_grace in &[false, true] {
                    for &past_limit in &[false, true] {
                        let a = barrier_action(done, pending, past_grace, past_limit);
                        if done {
                            assert_eq!(a, BarrierAction::Done, "done ⇒ Done");
                            continue;
                        }
                        // A CLAIMED, unfinished slot: NEVER stealable (a live
                        // executor owns it), NEVER Done. Halt past the limit, else
                        // Spin. This is the F1 UAF guard.
                        if !pending {
                            assert_ne!(
                                a,
                                BarrierAction::Steal,
                                "claimed+unfinished must never be re-stolen (live executor)"
                            );
                            let expected = if past_limit {
                                BarrierAction::Halt
                            } else {
                                BarrierAction::Spin
                            };
                            assert_eq!(
                                a, expected,
                                "claimed+unfinished (past_limit={past_limit}) must be {expected:?}"
                            );
                            continue;
                        }
                        // Unclaimed (no live executor): stealable once past grace,
                        // else spin. Never Halt (nothing to be wedged on).
                        assert_ne!(a, BarrierAction::Halt, "unclaimed must never Halt");
                        let expected = if past_grace {
                            BarrierAction::Steal
                        } else {
                            BarrierAction::Spin
                        };
                        assert_eq!(a, expected, "unclaimed (past_grace={past_grace})");
                    }
                }
            }
        }
    }

    /// Arthur latency round 2: the steal grace in TSC time. With a calibrated
    /// TSC it is `grace_us` after the publish, whatever the spin count (a
    /// million slow spins no longer stretch it, and none are needed); with no
    /// TSC rate it is the old spin count, unchanged.
    #[test]
    fn steal_grace_is_time_when_calibrated_else_spins() {
        use super::par_matmul_logic::past_steal_grace;
        let hz = 3_000_000_000u64; // 3,000 cycles per microsecond
        // 50 us = 150,000 cycles.
        assert!(!past_steal_grace(149_999, hz, 0, 50, 2_000_000));
        assert!(past_steal_grace(150_000, hz, 0, 50, 2_000_000));
        // The spin count is ignored when calibrated, in both directions.
        assert!(!past_steal_grace(149_999, hz, u64::MAX, 50, 2_000_000));
        assert!(past_steal_grace(150_000, hz, 0, 50, 2_000_000));
        // Uncalibrated: exactly the previous rule, spins >= grace_spins.
        assert!(!past_steal_grace(u64::MAX, 0, 1_999_999, 50, 2_000_000));
        assert!(past_steal_grace(0, 0, 2_000_000, 50, 2_000_000));
    }

    /// Chad, relay #52 item 3: the grace threshold is multiplied before it is
    /// divided, rounded up, in u128. A sub-MHz rate kept 0 cycles under
    /// `tsc_hz / 1_000_000 * grace_us` and stole on the first look.
    #[test]
    fn steal_grace_cycles_rounds_up_without_truncation() {
        use super::par_matmul_logic::{past_steal_grace, steal_grace_cycles};
        // 500 kHz (leaf 15 with no reference): 50 us = 25 cycles, not 0.
        assert_eq!(steal_grace_cycles(500_000, 50), 25);
        assert!(!past_steal_grace(0, 500_000, u64::MAX, 50, 2_000_000));
        assert!(!past_steal_grace(24, 500_000, 0, 50, 2_000_000));
        assert!(past_steal_grace(25, 500_000, 0, 50, 2_000_000));
        // A non-integral MHz rate: 2_399_999_999 Hz * 50 us = 119_999.99995
        // cycles, rounded up to 120_000 (the old form gave 2_399 * 50 = 119_950).
        assert_eq!(steal_grace_cycles(2_399_999_999, 50), 120_000);
        assert!(!past_steal_grace(119_999, 2_399_999_999, 0, 50, 2_000_000));
        assert!(past_steal_grace(120_000, 2_399_999_999, 0, 50, 2_000_000));
        // 3 GHz: exactly 150_000.
        assert_eq!(steal_grace_cycles(3_000_000_000, 50), 150_000);
        // u64::MAX Hz: the product needs u128; 50 us of it still fits in u64.
        assert_eq!(
            steal_grace_cycles(u64::MAX, 50),
            ((u64::MAX as u128 * 50).div_ceil(1_000_000)) as u64
        );
        // Past the u64 range it saturates rather than wrapping small.
        assert_eq!(steal_grace_cycles(u64::MAX, 1_000_000_000), u64::MAX);
        // A 1 Hz clock still waits a whole cycle for any non-zero grace.
        assert_eq!(steal_grace_cycles(1, 50), 1);
    }

    /// CPUID.06H / 01H bit positions (Intel SDM vol. 2A, CPUID leaf 06H: EAX[7]
    /// HWP, EAX[10] HWP_EPP, ECX[0] hardware coordination feedback; leaf 01H
    /// ECX[7] EIST), each bit alone so a shifted mask fails.
    #[test]
    fn cpufreq_power_caps_bits() {
        use super::cpufreq_logic::PowerCaps;
        let none = PowerCaps::from_cpuid(0, 0, 0);
        assert_eq!(none, PowerCaps::default());
        assert!(PowerCaps::from_cpuid(0x80, 0, 0).hwp);
        assert!(!PowerCaps::from_cpuid(0x40, 0, 0).hwp);
        assert!(PowerCaps::from_cpuid(0x400, 0, 0).hwp_epp);
        assert!(!PowerCaps::from_cpuid(0x200, 0, 0).hwp_epp);
        assert!(PowerCaps::from_cpuid(0, 1, 0).aperf_mperf);
        assert!(!PowerCaps::from_cpuid(0, 2, 0).aperf_mperf);
        assert!(PowerCaps::from_cpuid(0, 0, 0x80).eist);
        assert!(!PowerCaps::from_cpuid(0, 0, 0x100).eist);
    }

    /// IA32_HWP_CAPABILITIES and IA32_HWP_REQUEST byte layouts, hand-laid out
    /// (SDM vol. 3B, 15.4.2/15.4.4): capabilities 0x0108_2c33 = lowest 1,
    /// efficient 8, guaranteed 0x2c, highest 0x33; the performance request is
    /// min = lowest in [7:0], max = highest in [15:8], desired 0, EPP 0.
    #[test]
    fn cpufreq_hwp_layout_byte_oracle() {
        use super::cpufreq_logic::{HwpCaps, hwp_performance_request};
        let caps = HwpCaps::from_msr(0x0108_2c33);
        assert_eq!(
            caps,
            HwpCaps {
                highest: 0x33,
                guaranteed: 0x2c,
                efficient: 0x08,
                lowest: 0x01,
            }
        );
        assert_eq!(hwp_performance_request(caps), 0x0000_3301);
        // Upper capability bits never leak into the request.
        assert_eq!(
            hwp_performance_request(HwpCaps::from_msr(0xffff_ffff_0108_2c33)),
            0x0000_3301
        );
    }

    /// APERF/MPERF: clock = tsc_hz * dAPERF / dMPERF (SDM vol. 3B, 15.2).
    #[test]
    fn cpufreq_effective_mhz() {
        use super::cpufreq_logic::effective_mhz;
        // A 2.0 GHz TSC; APERF ran 40% of MPERF => 800 MHz (a CPU at a low
        // P-state); 2.2x => 4.4 GHz (turbo).
        assert_eq!(effective_mhz(2_000_000_000, 400, 1_000), Some(800));
        assert_eq!(effective_mhz(2_000_000_000, 2_200, 1_000), Some(4_400));
        // Large deltas do not overflow (u128 intermediate).
        assert_eq!(
            effective_mhz(3_000_000_000, u64::MAX / 2, u64::MAX / 2),
            Some(3_000)
        );
        assert_eq!(effective_mhz(2_000_000_000, 5, 0), None);
        assert_eq!(effective_mhz(0, 5, 5), None);
    }

    #[cfg(feature = "phase9-real-quant")]
    fn assert_native_matches_dequant_reference(
        label: &str,
        dtype: TensorDtype,
        rows: usize,
        data: Vec<u8>,
    ) {
        let mut lhs = vec![0.0f32; rows];
        let block = match dtype {
            TensorDtype::Q4K | TensorDtype::Q5K | TensorDtype::Q6K => QK_K,
            TensorDtype::Q5_0 | TensorDtype::Q8_0 => QK8_0,
            TensorDtype::F32 => unreachable!(),
        };
        for block_base in (0..rows).step_by(block) {
            lhs[block_base] = 127.0;
        }

        let native =
            forward_logic::matmul_vec_mixed_quant_for_test(&lhs, 1, rows, dtype, &data, 2).unwrap();
        let reference = forward_logic::matmul_vec_mixed_quant_dequant_reference_for_test(
            &lhs, 1, rows, dtype, &data, 2,
        )
        .unwrap();
        assert_eq!(native.len(), reference.len(), "{label} length");
        for idx in 0..native.len() {
            assert_eq!(
                native[idx].to_bits(),
                reference[idx].to_bits(),
                "{label} idx={idx}: native={} reference={}",
                native[idx],
                reference[idx]
            );
        }
    }

    #[cfg(feature = "phase9-real-quant")]
    fn repeated_blocks(block: Vec<u8>, blocks_per_col: usize, cols: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(block.len() * blocks_per_col * cols);
        for _ in 0..cols {
            for _ in 0..blocks_per_col {
                out.extend_from_slice(&block);
            }
        }
        out
    }

    #[cfg(feature = "phase9-real-quant")]
    fn tensor_dtype_to_ggml(dtype: TensorDtype) -> GgmlType {
        match dtype {
            TensorDtype::F32 => GgmlType::F32,
            TensorDtype::Q4K => GgmlType::Q4K,
            TensorDtype::Q5_0 => GgmlType::Q5_0,
            TensorDtype::Q5K => GgmlType::Q5K,
            TensorDtype::Q6K => GgmlType::Q6K,
            TensorDtype::Q8_0 => GgmlType::Q8_0,
        }
    }

    #[cfg(feature = "phase9-real-quant")]
    fn mixed_q4_k_block(value0: u8) -> Vec<u8> {
        let mut block = vec![0u8; BLOCK_Q4_K_SIZE];
        block[0..2].copy_from_slice(&0x3c00u16.to_le_bytes());
        block[4..8].copy_from_slice(&[1, 1, 1, 1]);
        block[12..16].copy_from_slice(&[1, 1, 1, 1]);
        block[16] = value0 & 0x0f;
        block
    }

    #[cfg(feature = "phase9-real-quant")]
    fn mixed_q5_0_block(value0: i8) -> Vec<u8> {
        const BLOCK_Q5_0_SIZE: usize = 22;
        let encoded = (value0 as i16 + 16) as u8;
        let mut block = vec![0u8; BLOCK_Q5_0_SIZE];
        block[0..2].copy_from_slice(&0x3c00u16.to_le_bytes());
        block[2..6].copy_from_slice(&u32::MAX.to_le_bytes());
        block[6] = encoded & 0x0f;
        if encoded & 0x10 == 0 {
            let mut qh = u32::from_le_bytes([block[2], block[3], block[4], block[5]]);
            qh &= !1;
            block[2..6].copy_from_slice(&qh.to_le_bytes());
        }
        block
    }

    #[cfg(feature = "phase9-real-quant")]
    fn mixed_q5_k_block(value0: u8) -> Vec<u8> {
        let mut block = vec![0u8; ops::qk_matmul::Q5_K_BLOCK_BYTES];
        block[0..2].copy_from_slice(&0x3c00u16.to_le_bytes());
        block[4] = 1;
        block[48] = value0 & 0x0f;
        if value0 & 0x10 != 0 {
            block[16] = 1;
        }
        block
    }

    #[cfg(feature = "phase9-real-quant")]
    fn mixed_q6_k_block(value0: i8) -> Vec<u8> {
        const BLOCK_Q6_K_SIZE: usize = 210;
        let encoded = (value0 as i16 + 32) as u8;
        let mut block = vec![0u8; BLOCK_Q6_K_SIZE];
        for high in &mut block[128..192] {
            *high = 0xaa;
        }
        for scale in &mut block[192..208] {
            *scale = 1;
        }
        block[208..210].copy_from_slice(&0x3c00u16.to_le_bytes());
        block[0] = encoded & 0x0f;
        block[128] = (block[128] & !0x03) | ((encoded >> 4) & 0x03);
        block
    }

    #[cfg(feature = "phase9-real-quant")]
    fn mixed_q8_0_block(value0: i8) -> Vec<u8> {
        let mut block = vec![0u8; BLOCK_Q8_0_SIZE];
        block[0..2].copy_from_slice(&0x3c00u16.to_le_bytes());
        block[2] = value0 as u8;
        block
    }

    /// Phase 8 BUILD-8.3 integration ISC-1/ISC-2: optional real-model gate.
    /// Set NEO_QWEN25_GGUF to Qwen2.5-0.5B-Q4_K_M. The dequant reference below
    /// is a direct raw-source transcription of llama.cpp 5f04dc7
    /// `dequantize_row_q4_K`/`get_scale_min_k4`, separate from the kernel
    /// helper under test. The matmul oracle is hard-pinned from that reference
    /// over the same real tensor bytes, not a round-trip.
    #[test]
    fn gguf_dequant_isc1_isc2_real_qwen_ffn_down_oracles_when_available() {
        let Ok(path) = std::env::var("NEO_QWEN25_GGUF") else {
            eprintln!(
                "SKIP gguf_dequant_isc1_isc2_real_qwen_ffn_down_oracles_when_available: NEO_QWEN25_GGUF unset"
            );
            return;
        };

        let bytes = std::fs::read(&path).expect("read real Qwen2.5 GGUF");
        let parsed = parse_gguf(&bytes).expect("real Qwen2.5 GGUF parses");
        let tensor = parsed
            .tensor_named("blk.11.ffn_down.weight")
            .expect("Qwen2.5 ffn_down Q4_K tensor");
        assert_eq!(tensor.ty, GgmlType::Q4K);
        assert_eq!(&tensor.dims[..tensor.n_dims as usize], &[4864, 896]);
        assert_eq!(tensor.byte_len, 2_451_456);

        let dequant = gguf_dequant_logic::dequantize_named_tensor_to_f32(
            &parsed,
            &bytes,
            "blk.11.ffn_down.weight",
        )
        .unwrap();
        assert_eq!(dequant.len(), 4_358_144);

        let raw = parsed.tensor_data(&bytes, tensor).unwrap();
        let reference = llama_reference_dequant_q4_k(raw);
        assert_f32_slice_close("real qwen ffn_down full dequant", &dequant, &reference, 0.0);
        assert_f32_slice_close(
            "real qwen ffn_down first block",
            &dequant[..REAL_QWEN_FFN_DOWN_FIRST16.len()],
            &REAL_QWEN_FFN_DOWN_FIRST16,
            REAL_WEIGHT_REL_EPS,
        );

        let input: Vec<f32> = (0..QK_K)
            .map(|idx| ((idx % 16) as f32 - 8.0) / 8.0)
            .collect();
        let lhs = bytes_from_f32(&input);
        let rhs = bytes_from_f32(&dequant[..(QK_K * 4)]);
        let lhs_shape = shape2(1, QK_K as u64);
        let rhs_shape = shape2(QK_K as u64, 4);
        let mut out = vec![0u8; REAL_QWEN_FFN_DOWN_MATMUL4.len() * 4];

        ops::matmul(&view(&lhs_shape, &lhs), &view(&rhs_shape, &rhs), &mut out).unwrap();

        assert_f32_bytes_close(
            "real qwen ffn_down first-slab matmul",
            &out,
            &REAL_QWEN_FFN_DOWN_MATMUL4,
            REL_EPS,
        );
    }

    #[test]
    fn add_supports_2d_bias_broadcast() {
        let lhs_bits = [
            0x3f80_0000,
            0x4000_0000,
            0x4040_0000,
            0x4080_0000,
            0x40a0_0000,
            0x40c0_0000,
        ];
        let rhs_bits = [0x4120_0000, 0x41a0_0000, 0x41f0_0000];
        let expected = [
            0x4130_0000,
            0x41b0_0000,
            0x4204_0000,
            0x4160_0000,
            0x41c8_0000,
            0x4210_0000,
        ];
        let lhs = bytes_from_bits(&lhs_bits);
        let rhs = bytes_from_bits(&rhs_bits);
        let mut out = vec![0u8; expected.len() * 4];
        let lhs_shape = shape2(2, 3);
        let rhs_shape = shape2(1, 3);

        ops::add(&view(&lhs_shape, &lhs), &view(&rhs_shape, &rhs), &mut out).unwrap();

        assert_eq!(bits_from_bytes(&out), expected);
    }

    #[test]
    fn add_supports_rank1_adr031_oracle() {
        let lhs = bytes_from_bits(&[0x3f80_0000, 0x4000_0000]);
        let rhs = bytes_from_bits(&[0x4040_0000, 0x4080_0000]);
        let expected = [0x4080_0000, 0x40c0_0000];
        let shape = shape1(2);
        let mut out = vec![0u8; expected.len() * 4];

        ops::add(&view(&shape, &lhs), &view(&shape, &rhs), &mut out).unwrap();

        assert_eq!(bits_from_bytes(&out), expected);
    }

    /// Phase 7 BUILD-4-1 (ISC-5, REQUIRED): independent byte oracle for the Gen8+
    /// PPGTT page-table entries. A leaf PTE and a directory entry for known physical
    /// addresses, with present / R-W bits, asserted **byte-for-byte** — independent of
    /// the construction code (the page table is an in-memory byte format). Includes the
    /// read-only leaf (the 4-2 enforcement bit) as a forward oracle.
    #[test]
    fn intel_gen9_ppgtt_pte_matches_hand_byte_oracle() {
        use super::intel_gen9_ppgtt as ppgtt;

        // A leaf PTE for phys 0x1234_5000, present + writable. By hand:
        //   0x1234_5000 (addr, 4 KiB-aligned) | PRESENT(bit0) | RW(bit1) = 0x1234_5003.
        const RW_LEAF_PHYS: u64 = 0x1234_5000;
        let rw_leaf = ppgtt::leaf_pte(RW_LEAF_PHYS, true);
        assert_eq!(rw_leaf, 0x1234_5003);
        assert_eq!(
            rw_leaf.to_le_bytes(),
            [0x03, 0x50, 0x34, 0x12, 0x00, 0x00, 0x00, 0x00]
        );

        // The same frame mapped READ-ONLY (4-2 forward proof): present, no RW bit.
        //   0x1234_5000 | PRESENT = 0x1234_5001.
        let ro_leaf = ppgtt::leaf_pte(RW_LEAF_PHYS, false);
        assert_eq!(ro_leaf, 0x1234_5001);
        assert_eq!(
            ro_leaf.to_le_bytes(),
            [0x01, 0x50, 0x34, 0x12, 0x00, 0x00, 0x00, 0x00]
        );

        // A directory entry (PML4E/PDPE/PDE) pointing at the next table at phys
        // 0x6789_a000: present + RW by construction. 0x6789_a000 | 0x3 = 0x6789_a003.
        let dir = ppgtt::dir_entry(0x6789_a000);
        assert_eq!(dir, 0x6789_a003);
        assert_eq!(
            dir.to_le_bytes(),
            [0x03, 0xa0, 0x89, 0x67, 0x00, 0x00, 0x00, 0x00]
        );

        // The address field is bits [47:12]; sub-page bits in `phys` are masked out
        // (callers pass frame-aligned addresses, but the mask is the contract).
        assert_eq!(ppgtt::leaf_pte(0x1234_5abc, true), 0x1234_5003);

        // 32-bit-legacy VA decomposition: all three data VAs live under PDP[0], PD[0],
        // in one PT — at PT indices 0x130/0x131/0x132 (3-level walk, no PML4).
        assert_eq!(ppgtt::pdp_index(ppgtt::PPGTT_LHS_VA), 0);
        assert_eq!(ppgtt::pd_index(ppgtt::PPGTT_LHS_VA), 0);
        assert_eq!(ppgtt::pt_index(ppgtt::PPGTT_LHS_VA), 0x130);
        assert_eq!(ppgtt::pt_index(ppgtt::PPGTT_RHS_VA), 0x131);
        assert_eq!(ppgtt::pt_index(ppgtt::PPGTT_OUTPUT_VA), 0x132);
    }

    /// Phase 7 BUILD-4-1 (ISC-2/5, iter3): independent byte oracle for the 32-bit
    /// legacy PPGTT-root load — one MI_LOAD_REGISTER_IMM(8) writing all four PDP
    /// register pairs: PDP0 = the Page Directory (UDW then LDW), PDP1..3 = 0.
    /// Hand-laid for a known `pd_phys` straddling the 32-bit boundary.
    #[test]
    fn intel_gen9_ppgtt_pdp_load_matches_hand_byte_oracle() {
        use super::intel_gen9_ppgtt as ppgtt;

        // Page Directory at phys 0x1_2345_6000 (>32 bits, to exercise the UDW/LDW split).
        const PD_PHYS: u64 = 0x1_2345_6000;
        // By hand (little-endian DWords):
        //   MI_LOAD_REGISTER_IMM(8) = (0x22<<23)|(2*8-1) = 0x1100_000f
        //   then 8 (offset,value) pairs: PDP0_UDW=hi, PDP0_LDW=lo, PDP1..3 UDW/LDW = 0.
        //   PDP{0,1,2,3}_LDW = 0x2270/0x2278/0x2280/0x2288 ; UDW = LDW+4.
        const EXPECTED: [u8; ppgtt::PDP_LOAD_BYTES] = [
            0x0f, 0x00, 0x00, 0x11, // MI_LOAD_REGISTER_IMM(8)
            0x74, 0x22, 0x00, 0x00, // PDP0_UDW (0x2274)
            0x01, 0x00, 0x00, 0x00, //   pd_phys[63:32] = 1
            0x70, 0x22, 0x00, 0x00, // PDP0_LDW (0x2270)
            0x00, 0x60, 0x45, 0x23, //   pd_phys[31:0] = 0x2345_6000
            0x7c, 0x22, 0x00, 0x00, // PDP1_UDW (0x227c)
            0x00, 0x00, 0x00, 0x00, //   0
            0x78, 0x22, 0x00, 0x00, // PDP1_LDW (0x2278)
            0x00, 0x00, 0x00, 0x00, //   0
            0x84, 0x22, 0x00, 0x00, // PDP2_UDW (0x2284)
            0x00, 0x00, 0x00, 0x00, //   0
            0x80, 0x22, 0x00, 0x00, // PDP2_LDW (0x2280)
            0x00, 0x00, 0x00, 0x00, //   0
            0x8c, 0x22, 0x00, 0x00, // PDP3_UDW (0x228c)
            0x00, 0x00, 0x00, 0x00, //   0
            0x88, 0x22, 0x00, 0x00, // PDP3_LDW (0x2288)
            0x00, 0x00, 0x00, 0x00, //   0
        ];

        let mut encoded = [0u8; ppgtt::PDP_LOAD_BYTES];
        assert_eq!(
            ppgtt::encode_pdp_load(&mut encoded, PD_PHYS),
            Some(ppgtt::PDP_LOAD_BYTES)
        );
        assert_eq!(encoded, EXPECTED);

        // GFX_MODE masked-**clear** of the 48-bit bit (forces 32-bit legacy mode even
        // if a prior 48-bit boot left it set): mask in [31:16], value clear → 0x0080_0000.
        assert_eq!(ppgtt::gfx_mode_clear_48b(), 0x0080_0000);
        assert_eq!(ppgtt::RENDER_RING_PDP0_LDW, 0x2270);
        assert_eq!(ppgtt::RENDER_RING_PDP3_LDW, 0x2288);
        assert_eq!(ppgtt::RENDER_RING_GFX_MODE, 0x229c);
    }

    // ── Phase 7.5 BUILD-V: VT-d byte/arithmetic oracles (ISC-7 / I-12) ───────────
    // Every on-hardware VT-d table is an in-memory byte format. Each EXPECTED
    // value below is hand-computed from the Intel VT-d spec, asserted against
    // the `vtd_logic` const-fn builders — independent of the construction code.

    /// VT-d Root entry (§9.1) byte oracle: Present(bit0) + CTP(bits 63:12).
    #[test]
    fn vtd_root_entry_byte_oracle() {
        use super::vtd_logic as vtd;

        // Context table at phys 0x1234_5000, present: low = addr | P = 0x1234_5001,
        // high = 0 (reserved in legacy mode).
        let re = vtd::root_entry(0x1234_5000, true);
        assert_eq!(re, [0x1234_5001, 0x0]);
        // The 16-byte little-endian image:
        let mut bytes = [0u8; 16];
        bytes[0..8].copy_from_slice(&re[0].to_le_bytes());
        bytes[8..16].copy_from_slice(&re[1].to_le_bytes());
        assert_eq!(
            bytes,
            [
                0x01, 0x50, 0x34, 0x12, 0x00, 0x00, 0x00, 0x00, // low: P + CTP
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // high: reserved
            ]
        );

        // Not-present: P bit clear, address retained.
        assert_eq!(vtd::root_entry(0x1234_5000, false), [0x1234_5000, 0x0]);
        // Sub-page bits are masked out of the address field (the contract).
        assert_eq!(vtd::root_entry(0x1234_5abc, true), [0x1234_5001, 0x0]);
    }

    /// VT-d Context entry (§9.3) byte oracle: P(0)+FPD(1)+T(3:2) | SLPTPTR(63:12)
    /// in the low qword; AW(2:0) | DID(23:8) in the high qword.
    #[test]
    fn vtd_context_entry_byte_oracle() {
        use super::vtd_logic as vtd;

        // SLPTPTR=0x0012_3000, DID=0x0007, AW=48-bit/4-level(0b010), present.
        // low  = 0x0012_3000 | P(1)                 = 0x0012_3001
        // high = AW(2) | (DID(7) << 8)              = 0x0000_0702
        let ce = vtd::context_entry(0x0012_3000, 0x0007, vtd::AW_48BIT_4LVL, true);
        assert_eq!(ce, [0x0012_3001, 0x0000_0702]);
        // FPD (bit 1) MUST be clear — faults are recorded (the (c2) guard).
        assert_eq!(ce[0] & vtd::CTX_FPD, 0);

        let mut bytes = [0u8; 16];
        bytes[0..8].copy_from_slice(&ce[0].to_le_bytes());
        bytes[8..16].copy_from_slice(&ce[1].to_le_bytes());
        assert_eq!(
            bytes,
            [
                0x01, 0x30, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00, // low: P + SLPTPTR
                0x02, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // high: AW + DID
            ]
        );

        // Not-present clears only bit 0.
        assert_eq!(
            vtd::context_entry(0x0012_3000, 0x0007, vtd::AW_48BIT_4LVL, false),
            [0x0012_3000, 0x0000_0702]
        );
        // 39-bit/3-level AW field is 0b001.
        let ce3 = vtd::context_entry(0x0012_3000, 0x0001, vtd::AW_39BIT_3LVL, true);
        assert_eq!(ce3[1], 0x0000_0101);
    }

    /// OT-63 GPU P1 Slice 5 (ADR-077 §Slice-5 + amendment 5): the graphics VT-d
    /// domain (`DID_GRAPHICS=3`, mapping the named graphics arena) is the SAME
    /// second-level page-table object as the identity/fence domains — it reuses
    /// `context_entry`/`sl_leaf`/`sl_dir`/`root_entry` verbatim, only the
    /// domain-id changes. This oracle pins that "no new byte layout" claim:
    /// DID=3 lands in the context-entry high bits [23:8] exactly as DID=1/2 do,
    /// and the leaf/dir entries the graphics domain writes are byte-identical to
    /// the fence domain's. So Slice 5 introduces no new on-hardware byte format.
    #[test]
    fn vtd_graphics_domain_reuses_pinned_layout_byte_oracle() {
        use super::vtd_logic as vtd;

        // Context entry for the graphics domain: SLPTPTR=0x0012_3000, DID=3
        // (DID_GRAPHICS), AW=48-bit/4-level, present.
        // low  = 0x0012_3000 | P(1)        = 0x0012_3001
        // high = AW(2) | (DID(3) << 8)     = 0x0000_0302
        let ce = vtd::context_entry(0x0012_3000, 3, vtd::AW_48BIT_4LVL, true);
        assert_eq!(ce, [0x0012_3001, 0x0000_0302]);
        // FPD clear — an out-of-graphics-domain GPU DMA MUST fault + record
        // (the falsifiable negative control).
        assert_eq!(ce[0] & vtd::CTX_FPD, 0);
        let mut bytes = [0u8; 16];
        bytes[0..8].copy_from_slice(&ce[0].to_le_bytes());
        bytes[8..16].copy_from_slice(&ce[1].to_le_bytes());
        assert_eq!(
            bytes,
            [
                0x01, 0x30, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00, // low: P + SLPTPTR
                0x02, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // high: AW + DID=3
            ]
        );

        // The graphics domain maps its arena with the identical 4 KiB R+W leaf
        // and non-leaf dir entries as the connectivity fence domain — same
        // pinned second-level format (no new byte layout for Slice 5).
        assert_eq!(vtd::sl_leaf(0x1234_5000, true, true), 0x1234_5003);
        assert_eq!(vtd::sl_dir(0x6789_a000), 0x6789_a003);
        // An out-of-domain page is not-present (R|W both clear) ⇒ the GPU DMA
        // there faults.
        assert_eq!(vtd::sl_leaf(0x1234_5000, false, false), 0x1234_5000);
    }

    /// VT-d Second-Level paging entries (§9.7) byte oracle. The load-bearing
    /// difference from x86/PPGTT: NO present bit — validity IS R|W; a zero
    /// entry is not-present.
    #[test]
    fn vtd_second_level_pte_byte_oracle() {
        use super::vtd_logic as vtd;

        // R+W leaf for phys 0x1234_5000: 0x1234_5000 | R(1) | W(2) = 0x1234_5003.
        let rw = vtd::sl_leaf(0x1234_5000, true, true);
        assert_eq!(rw, 0x1234_5003);
        assert_eq!(
            rw.to_le_bytes(),
            [0x03, 0x50, 0x34, 0x12, 0x00, 0x00, 0x00, 0x00]
        );

        // Read-only leaf: R only = 0x1234_5001.
        assert_eq!(vtd::sl_leaf(0x1234_5000, true, false), 0x1234_5001);

        // The (c2)-critical case: read=false, write=false ⇒ NOT-present (0 access
        // bits), so the entry is purely the address with no implicit valid bit —
        // the device cannot reach it. (Contrast PPGTT leaf_pte(_, false) which
        // still sets bit0 Present.)
        assert_eq!(vtd::sl_leaf(0x1234_5000, false, false), 0x1234_5000);

        // Non-leaf directory entry → next table at 0x6789_a000, R+W: 0x6789_a003.
        assert_eq!(vtd::sl_dir(0x6789_a000), 0x6789_a003);

        // Large-page leaf (PS bit 7) for a 1 GiB region at 0x4000_0000, R+W:
        // 0x4000_0000 | R|W(3) | PS(0x80) = 0x4000_0083.
        assert_eq!(vtd::sl_leaf_large(0x4000_0000, true, true), 0x4000_0083);

        // Sub-page bits masked from the address field.
        assert_eq!(vtd::sl_leaf(0x1234_5abc, true, true), 0x1234_5003);
    }

    /// VT-d 4-level / 48-bit IOVA decomposition oracle.
    #[test]
    fn vtd_iova_decomposition() {
        use super::vtd_logic as vtd;

        // iova built from distinct indices: PML4=1, PDPT=2, PD=3, PT=4, offset 0.
        // (1<<39)|(2<<30)|(3<<21)|(4<<12) = 0x80_8060_4000.
        const IOVA: u64 = (1 << 39) | (2 << 30) | (3 << 21) | (4 << 12);
        assert_eq!(IOVA, 0x80_8060_4000);
        assert_eq!(vtd::sl_pml4_index(IOVA), 1);
        assert_eq!(vtd::sl_pdpt_index(IOVA), 2);
        assert_eq!(vtd::sl_pd_index(IOVA), 3);
        assert_eq!(vtd::sl_pt_index(IOVA), 4);
        assert_eq!(vtd::SL_ENTRIES_PER_TABLE, 512);
    }

    /// CAP decode + AGAW selection (§10.4.2 / V7 width discriminator).
    #[test]
    fn vtd_cap_decode_and_agaw_selection() {
        use super::vtd_logic as vtd;

        // A CAP with MGAW field=38 (→39-bit host width), SAGAW=0b00100 (48-bit
        // supported), FRO field=0x10 (→offset 0x100), NFR field=0 (→1 register),
        // CM=0.  CAP = (0x10<<24) | (38<<16) | (4<<8) = 0x1026_0400.
        const CAP: u64 = (0x10 << 24) | (38 << 16) | (4 << 8);
        assert_eq!(CAP, 0x1026_0400);
        assert_eq!(vtd::cap_mgaw_bits(CAP), 39);
        assert_eq!(vtd::cap_sagaw(CAP), 0b00100);
        assert_eq!(vtd::cap_fault_recording_offset(CAP), 0x100);
        assert_eq!(vtd::cap_num_fault_recording(CAP), 1);
        assert!(!vtd::cap_caching_mode(CAP));
        // Caching-mode set (QEMU intel-iommu,caching-mode=on): bit 7.
        assert!(vtd::cap_caching_mode(CAP | (1 << 7)));

        // SLLPS (super-page support, bits 37:34). The bare CAP has none.
        assert_eq!(vtd::cap_sllps(CAP), 0);
        assert!(!vtd::cap_supports_1g_page(CAP));
        assert!(!vtd::cap_supports_2m_page(CAP));
        // 2 MiB (bit 34) + 1 GiB (bit 35) set — QEMU intel-iommu's default.
        let cap_sp = CAP | (1 << 34) | (1 << 35);
        assert_eq!(vtd::cap_sllps(cap_sp), 0b0011);
        assert!(vtd::cap_supports_1g_page(cap_sp));
        assert!(vtd::cap_supports_2m_page(cap_sp));

        // AGAW selection prefers 48-bit/4-level.
        assert_eq!(
            vtd::select_agaw(0b00100),
            Some(vtd::AgawChoice {
                levels: 4,
                aw_field: vtd::AW_48BIT_4LVL,
                addr_width: 48
            })
        );
        // Both supported → still prefer 48-bit.
        assert_eq!(vtd::select_agaw(0b00110).unwrap().levels, 4);
        // Only 39-bit/3-level.
        assert_eq!(
            vtd::select_agaw(0b00010),
            Some(vtd::AgawChoice {
                levels: 3,
                aw_field: vtd::AW_39BIT_3LVL,
                addr_width: 39
            })
        );
        // Only 2-level/30-bit (unsupported by us) ⇒ None — a conclusive V7 fail.
        assert_eq!(vtd::select_agaw(0b00001), None);

        // ECAP.IRO (bits 17:8) = 0x20 → IOTLB register block at 0x200.
        const ECAP: u64 = 0x20 << 8;
        assert_eq!(vtd::ecap_iotlb_offset(ECAP), 0x200);
        // ECAP.C (bit 0) = coherency. Clear here; set ⇒ no table clflush needed.
        assert!(!vtd::ecap_coherent(ECAP));
        assert!(vtd::ecap_coherent(ECAP | 1));
    }

    /// Source-id encoding (§3.4) — the (bus<<8)|(dev<<3)|func requester id.
    #[test]
    fn vtd_source_id_encoding() {
        use super::vtd_logic as vtd;

        assert_eq!(vtd::source_id(0x00, 0x1f, 0x7), 0x00ff);
        assert_eq!(vtd::source_id(0x02, 0x00, 0x0), 0x0200);
        assert_eq!(vtd::source_id(0x12, 0x05, 0x3), 0x122b);
        // Round-trip the index extractors used to walk Root[bus]→Context[devfn].
        assert_eq!(vtd::source_id_bus(0x122b), 0x12);
        assert_eq!(vtd::source_id_devfn(0x122b), 0x2b);
        // Out-of-range dev/func bits are masked, never bleed into the bus byte.
        assert_eq!(vtd::source_id(0x00, 0xff, 0xff), 0x00ff);
    }

    /// Fault-recording-register decode (§11.4.7.1) — the (c2) evidence path.
    #[test]
    fn vtd_fault_record_decode_oracle() {
        use super::vtd_logic as vtd;

        // A WRITE fault by source-id 0x00f8 (bus0 dev0x1f func0) at out-of-arena
        // address 0x4000_0000, reason 0x05, fault bit set.
        //   low  = faulting addr (bits 63:12)             = 0x4000_0000
        //   high = F(63) | T=0(write) | reason(0x05<<32) | sid(0xf8)
        //        = 0x8000_0005_0000_00f8
        let fr = vtd::decode_fault_record(0x4000_0000, 0x8000_0005_0000_00f8);
        assert_eq!(
            fr,
            vtd::FaultRecord {
                fault: true,
                is_read: false,
                reason: 0x05,
                source_id: 0x00f8,
                fault_addr: 0x4000_0000,
            }
        );

        // A READ fault sets bit 62.
        let fr_rd = vtd::decode_fault_record(0x5000_0000, 0xc000_0007_0000_0210);
        assert!(fr_rd.fault && fr_rd.is_read);
        assert_eq!(fr_rd.reason, 0x07);
        assert_eq!(fr_rd.source_id, 0x0210);
        assert_eq!(fr_rd.fault_addr, 0x5000_0000);

        // No fault recorded ⇒ fault bit clear.
        assert!(!vtd::decode_fault_record(0, 0).fault);
        // The RW1C clear value writes 1 to the F bit only.
        assert_eq!(vtd::FAULT_RECORD_CLEAR_F, 0x8000_0000_0000_0000);
    }

    /// DMAR ACPI table (§8) byte oracle — the format-ABI hand-laid example
    /// required by the CLAUDE.md standing rule (I-12). A minimal-but-real DMAR:
    /// one DRHD (INCLUDE_PCI_ALL, register base 0xFED9_0000, one device scope)
    /// and one RMRR ([0x000a_0000, 0x000b_ffff]). Total length 96 bytes.
    #[test]
    fn dmar_table_byte_oracle() {
        use super::dmar_logic as dmar;

        #[rustfmt::skip]
        const DMAR: [u8; 96] = [
            // ── SDT header (36 bytes) ───────────────────────────────────────
            b'D', b'M', b'A', b'R',          // signature
            0x60, 0x00, 0x00, 0x00,          // length = 96
            0x01,                            // revision
            0x00,                            // checksum (validated in sdt.rs, not here)
            b'N', b'E', b'O', b'P', b'L', 0, // oem_id[6]
            b'V', b'T', b'D', b'O', b'R', b'A', b'C', b'L', // oem_table_id[8]
            0x01, 0x00, 0x00, 0x00,          // oem_revision
            b'N', b'E', b'O', 0x00,          // creator_id
            0x01, 0x00, 0x00, 0x00,          // creator_revision
            // ── DMAR-specific fixed fields (offset 36) ──────────────────────
            0x26,                            // Host Address Width = 38 (→39-bit)
            0x00,                            // Flags (no INTR_REMAP)
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0,    // reserved[10]  (table offset 48 next)
            // ── DRHD remapping structure (offset 48, length 24) ─────────────
            0x00, 0x00,                      // type = 0 (DRHD)
            0x18, 0x00,                      // length = 24
            0x01,                            // flags = INCLUDE_PCI_ALL
            0x00,                            // reserved
            0x00, 0x00,                      // segment = 0
            0x00, 0x00, 0xd9, 0xfe, 0x00, 0x00, 0x00, 0x00, // register base = 0xFED9_0000
            // device scope: PCI endpoint, len 8, start-bus 0, path (dev=2,func=0)
            0x01, 0x08, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00,
            // ── RMRR remapping structure (offset 72, length 24) ─────────────
            0x01, 0x00,                      // type = 1 (RMRR)
            0x18, 0x00,                      // length = 24
            0x00, 0x00,                      // reserved
            0x00, 0x00,                      // segment = 0
            0x00, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, // base  = 0x000a_0000
            0xff, 0xff, 0x0b, 0x00, 0x00, 0x00, 0x00, 0x00, // limit = 0x000b_ffff
        ];

        assert_eq!(dmar::sdt_length(&DMAR), Some(96));
        assert_eq!(dmar::dmar_host_addr_width(&DMAR), Some(39));
        assert_eq!(dmar::dmar_flags(&DMAR), Some(0));

        let structs: Vec<_> = dmar::DmarStructs::new(&DMAR).collect();
        assert_eq!(structs.len(), 2);

        let drhd = &structs[0];
        assert_eq!(drhd.kind, dmar::DMAR_TYPE_DRHD);
        assert_eq!(dmar::drhd_register_base(drhd), Some(0xfed9_0000));
        assert_eq!(dmar::drhd_include_pci_all(drhd), Some(true));
        assert_eq!(dmar::drhd_segment(drhd), Some(0));

        let rmrr = &structs[1];
        assert_eq!(rmrr.kind, dmar::DMAR_TYPE_RMRR);
        assert_eq!(
            dmar::rmrr_base_limit(rmrr),
            Some((0x000a_0000, 0x000b_ffff))
        );
        assert_eq!(dmar::rmrr_segment(rmrr), Some(0));
    }

    /// DMAR parse totality (I-14(b) at the parse layer): a truncated buffer and a
    /// malformed zero-length structure must terminate safely, never panic/loop.
    #[test]
    fn dmar_parse_is_total_on_malformed_input() {
        use super::dmar_logic as dmar;

        // SDT claims length 96 but only 60 bytes are present: the DRHD header is
        // readable, but its body overruns the buffer ⇒ the walk yields nothing
        // (the structure overruns `end`), never an OOB read.
        #[rustfmt::skip]
        let mut truncated = [0u8; 60];
        truncated[4..8].copy_from_slice(&96u32.to_le_bytes()); // length = 96 (> buffer)
        truncated[36] = 0x26;
        truncated[48] = 0x00;
        truncated[49] = 0x00; // DRHD type
        truncated[50] = 0x18;
        truncated[51] = 0x00; // length 24 (overruns 60)
        assert_eq!(dmar::DmarStructs::new(&truncated).count(), 0);

        // A zero-length structure must stop the walk (no infinite loop).
        #[rustfmt::skip]
        let mut zero_len = [0u8; 56];
        zero_len[4..8].copy_from_slice(&56u32.to_le_bytes());
        zero_len[36] = 0x26;
        zero_len[48] = 0x00;
        zero_len[49] = 0x00; // type
        zero_len[50] = 0x00;
        zero_len[51] = 0x00; // length = 0 (malformed)
        assert_eq!(dmar::DmarStructs::new(&zero_len).count(), 0);

        // A buffer too small to even hold the DMAR fixed header yields nothing.
        assert_eq!(dmar::DmarStructs::new(&[0u8; 8]).count(), 0);
        assert_eq!(dmar::sdt_length(&[0u8; 2]), None);
    }

    /// Phase 7 BUILD-4X-1 (ISC-2/3/6): independent byte oracle for the 64-bit execlist
    /// context descriptor and the four-DWord ELSP submit sequence. Hand-laid for a known
    /// (LRCA, ctx_id); the descriptor is the value whose wrongness makes the engine
    /// silently reject the submission (failure-mode table mode 1).
    #[test]
    fn intel_gen9_execlist_descriptor_matches_hand_byte_oracle() {
        use super::intel_gen9_execlist as xl;

        // LRC at GGTT 0x0020_0000, software context id 1. By hand:
        //   flags = VALID(1<<0) | PRIVILEGE(1<<8=0x100) | (LEGACY 0b11 << 3 = 0x18) = 0x119
        //   lrca & 0xffff_f000 = 0x0020_0000
        //   ctx_id << 32       = 0x0000_0001_0000_0000
        //   desc = 0x0000_0001_0020_0119
        const LRCA: u64 = 0x0020_0000;
        const CTX_ID: u32 = 1;
        let desc = xl::context_descriptor(LRCA, CTX_ID);
        assert_eq!(desc, 0x0000_0001_0020_0119);
        assert_eq!(
            desc.to_le_bytes(),
            [0x19, 0x01, 0x20, 0x00, 0x01, 0x00, 0x00, 0x00]
        );
        assert_eq!(xl::context_descriptor_flags(), 0x119);

        // ── ISC-6 non-i915 cross-check (the byte-oracle deliverable) ──
        // Independently rebuild the descriptor field-by-field from the PRM Vol 2d
        // "Context Descriptor Format" BIT RANGES, cross-confirmed against the Fuchsia
        // Magma `msd-intel-gen` descriptor builder (a clean-room, non-i915 driver). This
        // derivation does NOT call the module's named GEN8_CTX_* constants — it lays the
        // bits directly from the PRM field definitions, closing the "two i915 readers
        // agree even if both wrong" loophole the research flagged.
        let mut prm_desc: u64 = 0;
        prm_desc |= 1 << 0; // bit 0    : Valid
        prm_desc |= 0b11 << 3; // bits 4:3 : Addressing Mode = 3 (Gen9 48-bit / 4-level)
        prm_desc |= 1 << 8; // bit 8    : Privilege (Supervisor)
        prm_desc |= LRCA & 0x0000_0000_ffff_f000; // bits 31:12 : LRCA (GGTT page of PPHWSP)
        prm_desc |= (CTX_ID as u64) << 32; // bits 52:32 : Context ID (Gen9 — NOT the Gen11 split)
        assert_eq!(
            prm_desc, desc,
            "PRM/Fuchsia field-range descriptor must match the encoder"
        );

        // ELSP submit: element 1 empty (0,0), then element 0 high, element 0 low (latches).
        //   [0, 0, desc[63:32]=0x1, desc[31:0]=0x0020_0119]
        assert_eq!(
            xl::elsp_submit_dwords(desc),
            [0x0000_0000, 0x0000_0000, 0x0000_0001, 0x0020_0119]
        );

        // The ELSP / EXECLIST / CSB / HWSP / bring-up register offsets (RCS base 0x2000):
        // pinned so a transposed offset is caught at host time (RESEARCH §1/§4/§5).
        assert_eq!(xl::RING_ELSP, 0x2230);
        assert_eq!(xl::RING_EXECLIST_STATUS_LO, 0x2234);
        assert_eq!(xl::ring_context_status_buf_status(0), 0x2370);
        assert_eq!(xl::ring_context_status_buf_ctxid(0), 0x2374);
        assert_eq!(xl::ring_context_status_buf_status(5), 0x2398);
        assert_eq!(xl::RING_CONTEXT_STATUS_PTR, 0x23a0);
        assert_eq!(xl::RING_HWS_PGA, 0x2080);
        assert_eq!(xl::RING_HWSTAM, 0x2098);
        assert_eq!(xl::RING_MI_MODE, 0x209c);
        // Execlist ENABLE is the masked SET of bit 15 — the inverse of BUILD-4-1's clear.
        assert_eq!(xl::ring_mode_enable_execlists(), 0x8000_8000);
        // MI_MODE un-stop = masked-clear of STOP_RING (bit 8).
        assert_eq!(xl::mi_mode_unstop_ring(), 0x0100_0000);
        // WaIdleLiteRestore pad is 2 dwords / 8 bytes (the corrected value).
        assert_eq!(xl::WA_TAIL_DWORDS, 2);
        assert_eq!(xl::WA_TAIL_BYTES, 8);
    }

    /// Phase 7 BUILD-4X-1 (ISC-1/6): independent byte oracle for the LRC register-state
    /// image — the ring-pointer block (HEAD/TAIL/START/CTL), the single 48-bit PML4 slot
    /// (CTX_PDP0), and the three MI_LOAD_REGISTER_IMM headers. These are the dwords whose
    /// wrongness silently breaks the context (it switches in but runs nothing / the wrong
    /// ring). **The dword map is the non-i915 cross-check deliverable:** the indices below
    /// are confirmed against Fuchsia Magma `msd-intel-gen` `register_state_helper.h`
    /// (clean-room, not derived from i915) — MI_LRI headers at 0x1/0x21/0x41, ring
    /// head/tail/start/control VALUES at 0x05/0x07/0x09/0x0b, PML4 at PDP0 0x31/0x33. This
    /// catches the off-by-one trap (the image starts with MI_NOOP at dword 0, NOT the LRI
    /// header) that the cited research surfaced.
    #[test]
    fn intel_gen9_execlist_lrc_reg_state_matches_hand_byte_oracle() {
        use super::intel_gen9_execlist as xl;

        // Non-i915 (Fuchsia) cross-check of the CTX_* dword map — independent of i915.
        assert_eq!(xl::CTX_LRI_HEADER_0, 0x01); // NOT 0x00 — dword 0 is MI_NOOP
        assert_eq!(xl::CTX_RING_HEAD_VALUE, 0x05);
        assert_eq!(xl::CTX_RING_TAIL_VALUE, 0x07);
        assert_eq!(xl::CTX_RING_START_VALUE, 0x09);
        assert_eq!(xl::CTX_RING_CTL_VALUE, 0x0b);
        assert_eq!(xl::CTX_LRI_HEADER_1, 0x21);
        assert_eq!(xl::CTX_PDP0_UDW_VALUE, 0x31);
        assert_eq!(xl::CTX_PDP0_LDW_VALUE, 0x33);
        assert_eq!(xl::CTX_LRI_HEADER_2, 0x41);

        // Known params: ring at GGTT 0x0010_0000, RING_CTL=0x1 (1 page, valid), HEAD=0,
        // TAIL=0x11c (276 batch + 8 WaIdleLiteRestore pad), PML4 page at 0x1_2345_6000
        // (straddling 32 bits) → CTX_PDP0, PDP1..3 = 0.
        let params = xl::LrcParams {
            ring_start: 0x0010_0000,
            ring_ctl: 0x0000_0001,
            ring_head: 0,
            ring_tail: 0x0000_011c,
            pdp: [0x1_2345_6000, 0, 0, 0],
        };
        let mut image = [0u8; xl::LRC_REG_STATE_BYTES];
        assert_eq!(
            xl::write_lrc_reg_state(&mut image, params),
            Some(xl::LRC_REG_STATE_BYTES)
        );

        // Hand-laid little-endian DWords at their fixed DWord indices (byte = idx*4).
        let dw = |idx: usize| -> [u8; 4] {
            [
                image[idx * 4],
                image[idx * 4 + 1],
                image[idx * 4 + 2],
                image[idx * 4 + 3],
            ]
        };
        // Dword 0x00 is MI_NOOP — the image does NOT start with the LRI header.
        assert_eq!(dw(0x00), [0x00, 0x00, 0x00, 0x00]);
        // LRI header 0 @0x01 = MI_LOAD_REGISTER_IMM(14) | FORCE_POSTED = 0x1100_101b.
        assert_eq!(dw(0x01), [0x1b, 0x10, 0x00, 0x11]);
        // CONTEXT_CONTROL register (0x2244) @0x02 + masked-enable inhibit bits (0x0009_0009) @0x03.
        assert_eq!(dw(0x02), [0x44, 0x22, 0x00, 0x00]);
        assert_eq!(dw(0x03), [0x09, 0x00, 0x09, 0x00]);
        // Ring-pointer block (the +1 value-slot rule): reg addr at even, value at +1.
        assert_eq!(dw(0x06), [0x30, 0x20, 0x00, 0x00]); // RING_TAIL reg (0x2030)
        assert_eq!(dw(0x07), [0x1c, 0x01, 0x00, 0x00]); //   tail value = 0x11c
        assert_eq!(dw(0x08), [0x38, 0x20, 0x00, 0x00]); // RING_START reg (0x2038)
        assert_eq!(dw(0x09), [0x00, 0x00, 0x10, 0x00]); //   start value = 0x0010_0000
        assert_eq!(dw(0x0a), [0x3c, 0x20, 0x00, 0x00]); // RING_CTL reg (0x203c)
        assert_eq!(dw(0x0b), [0x01, 0x00, 0x00, 0x00]); //   ctl value = 0x1
        // CTX_BB_STATE value @0x11 = RING_BB_PPGTT (bit 5).
        assert_eq!(dw(0x11), [0x20, 0x00, 0x00, 0x00]);
        // LRI header 1 @0x21 = MI_LOAD_REGISTER_IMM(9) | FORCE_POSTED = 0x1100_1011.
        assert_eq!(dw(0x21), [0x11, 0x10, 0x00, 0x11]);
        // Single 48-bit PML4 root: CTX_PDP0_UDW reg(0x2274)@0x30 + hi@0x31, LDW reg(0x2270)@0x32 + lo@0x33.
        assert_eq!(dw(0x30), [0x74, 0x22, 0x00, 0x00]); // PDP0_UDW reg
        assert_eq!(dw(0x31), [0x01, 0x00, 0x00, 0x00]); //   pml4[63:32] = 1
        assert_eq!(dw(0x32), [0x70, 0x22, 0x00, 0x00]); // PDP0_LDW reg
        assert_eq!(dw(0x33), [0x00, 0x60, 0x45, 0x23]); //   pml4[31:0] = 0x2345_6000
        // LRI header 2 @0x41 = MI_LOAD_REGISTER_IMM(1) | FORCE_POSTED = 0x1100_1001 + R_PWR_CLK_STATE.
        assert_eq!(dw(0x41), [0x01, 0x10, 0x00, 0x11]);
        assert_eq!(dw(0x42), [0xc8, 0x20, 0x00, 0x00]); // R_PWR_CLK_STATE (0x20c8)

        // The populated extent is 0x44 dwords; everything past it stays zero.
        assert_eq!(xl::LRC_REG_STATE_DWORDS, 0x44);
        assert_eq!(xl::context_control_value(), 0x0009_0009);
    }

    /// Phase 7 BUILD-4X-1 (iter2): the 48-bit / 4-level VA decomposition (PML4→PDP→PD→PT,
    /// 9 bits/level) for the no-op's pipeline pages. iter1 silicon proved the execlist
    /// context resolves STATE_BASE/IDD/kernel through its PPGTT, so the PML4 must map
    /// these pages. All the no-op VAs (0x10_0000…0x13_3000) share PML4[0]→PDP[0]→PD[0] in
    /// one PT — so four table frames suffice; the leaves land at PT[0x100…0x133].
    #[test]
    fn intel_gen9_execlist_pml4_va_decomposition_oracle() {
        use super::intel_gen9_execlist as xl;

        // STATE page 0x11_0000: by hand 0x110000>>39=0, (0x110000>>30)&0x1ff=0,
        // (0x110000>>21)&0x1ff=0, (0x110000>>12)&0x1ff=0x110.
        assert_eq!(xl::pml4_index(0x11_0000), 0);
        assert_eq!(xl::pdp_index_48(0x11_0000), 0);
        assert_eq!(xl::pd_index_48(0x11_0000), 0);
        assert_eq!(xl::pt_index_48(0x11_0000), 0x110);

        // All seven no-op pages share the same PML4/PDP/PD entry (index 0) and differ
        // only in the PT index — so one PT page holds every leaf.
        let pages: [(u64, usize); 7] = [
            (0x10_0000, 0x100), // ring
            (0x11_0000, 0x110), // state
            (0x12_0000, 0x120), // kernel
            (0x13_0000, 0x130), // lhs
            (0x13_1000, 0x131), // rhs
            (0x13_2000, 0x132), // output
            (0x13_3000, 0x133), // seqno
        ];
        for (va, pt) in pages {
            assert_eq!(xl::pml4_index(va), 0);
            assert_eq!(xl::pdp_index_48(va), 0);
            assert_eq!(xl::pd_index_48(va), 0);
            assert_eq!(xl::pt_index_48(va), pt);
        }

        // A VA in the next 2 MiB region would need a different PD entry — the bound that
        // makes "one PT covers all no-op pages" true (a guard against silent overflow).
        assert_eq!(xl::pd_index_48(0x20_0000), 1);
    }

    /// Phase 7 BUILD-4X-2 (ISC-5): the per-context PML4 must map the Add's **data buffers**
    /// (lhs/rhs/output) AND the CURBE indirect-data heap at their A64 VAs, present+RW. The
    /// data buffers are PPGTT-only (cleared from GGTT — the self-proving gate), so their PT
    /// leaves must resolve to the right frames present+RW. Reuses the 4-1 `leaf_pte` oracle.
    #[test]
    fn intel_gen9_execlist_data_pml4_leaves_oracle() {
        use super::intel_gen9_execlist as xl;
        use super::intel_gen9_ppgtt as ppgtt;

        // The Add's data + CURBE VAs all live under PML4[0]→PDP[0]→PD[0] in one PT (the
        // CURBE at 0x13_4000 is the 4X-2 addition to the 4X-1 page set).
        let data_pages: [(u64, usize); 4] = [
            (0x13_0000, 0x130), // lhs (PPGTT-only)
            (0x13_1000, 0x131), // rhs (PPGTT-only)
            (0x13_2000, 0x132), // output (PPGTT-only)
            (0x13_4000, 0x134), // curbe (indirect-data heap)
        ];
        for (va, pt) in data_pages {
            assert_eq!(xl::pml4_index(va), 0);
            assert_eq!(xl::pdp_index_48(va), 0);
            assert_eq!(xl::pd_index_48(va), 0);
            assert_eq!(xl::pt_index_48(va), pt);
            assert!(xl::pt_index_48(va) < 512); // one PT covers every 4X-2 leaf
        }

        // A data leaf maps a frame present+RW (the EU writes `output`): for phys
        // 0x4a37a000, present(bit0)+RW(bit1) → 0x4a37a000 | 0x3 = 0x4a37a003.
        let leaf = ppgtt::leaf_pte(0x4a37a000, true);
        assert_eq!(leaf, 0x4a37a003);
        assert_eq!(
            leaf.to_le_bytes(),
            [0x03, 0xa0, 0x37, 0x4a, 0x00, 0x00, 0x00, 0x00]
        );
    }

    /// Phase 7 BUILD-4X-3 (ISC-5): the negative-test page-table invariants, hand-laid and
    /// asserted independent of the construction code. (a) An **RO** leaf (write-blocked at
    /// the device — the §9.7 RW-on-RO enforcement): present, RW-bit CLEAR. (b) The
    /// **two-context** invariant: tenant A's tensor VA is present+RW in A's PML4, while in
    /// tenant B's PML4 the *same* PT slot is left **absent** (entry = 0, present-bit clear) —
    /// so B's A64 access to that VA cannot resolve and must fault. This is the isolation the
    /// gate proves; a zeroed PT entry is the "VA omitted from this context" condition.
    #[test]
    fn intel_gen9_execlist_negative_pte_invariants_oracle() {
        use super::intel_gen9_ppgtt as ppgtt;

        const FRAME: u64 = 0x4a37a000; // a tensor's physical frame (4 KiB-aligned)

        // (a) RW leaf (tenant's writable own data): present(bit0)+RW(bit1) = …003.
        let rw = ppgtt::leaf_pte(FRAME, true);
        assert_eq!(rw & 0x1, 0x1, "RW leaf is present");
        assert_eq!(rw & 0x2, 0x2, "RW leaf is writable");
        assert_eq!(rw, 0x4a37a003);

        // (a) RO leaf (RW-on-RO: a write through this faults): present, RW-bit CLEAR = …001.
        let ro = ppgtt::leaf_pte(FRAME, false);
        assert_eq!(ro & 0x1, 0x1, "RO leaf is present (reads succeed)");
        assert_eq!(ro & 0x2, 0x0, "RO leaf is NOT writable (writes fault)");
        assert_eq!(ro, 0x4a37a001);

        // (b) The cross-tenant invariant. Tenant A maps lhs VA 0x13_0000 present+RW; tenant
        // B's PML4 OMITS that leaf, so B's PT slot at the same index stays 0 (the page table
        // is zero-initialized). A zero entry has the present bit clear → unresolvable → fault.
        let a_leaf = ppgtt::leaf_pte(FRAME, true); // present in A's page tree
        let b_leaf_absent: u64 = 0; // omitted in B's page tree (zeroed PT slot)
        assert_ne!(a_leaf & 0x1, 0, "A's tensor VA is present in A's PML4");
        assert_eq!(
            b_leaf_absent & 0x1,
            0,
            "the same VA is ABSENT (not present) in B's PML4"
        );
        // Both contexts index the same PT slot for the shared VA — the difference is purely
        // presence (A maps it, B doesn't), which is what the silicon enforces as isolation.
        use super::intel_gen9_execlist as xl;
        assert_eq!(xl::pt_index_48(0x13_0000), 0x130);
    }

    #[test]
    fn intel_gen9_noop_batch_matches_hand_byte_oracle() {
        const SEQNO: u32 = 0x1020_3040;
        // The no-op (dispatch bisector) batch — byte-identical to the silicon-proven
        // iter6 batch: PIPELINE_SELECT, PIPE_CONTROL(flush), STATE_BASE_ADDRESS,
        // PIPE_CONTROL(invalidate), MEDIA_VFE_STATE (CURBE alloc 0), IDL,
        // MEDIA_STATE_FLUSH, GPGPU_WALKER(SIMD32, all-lanes masks), PIPE_CONTROL(seqno),
        // MI_BATCH_BUFFER_END. iter7 keeps this unchanged so a retiring no-op still
        // isolates the dispatch path.
        const EXPECTED: [u8; intel_gen9_4a::NOOP_BATCH_BYTES] = [
            0x02, 0x03, 0x04, 0x69, 0x04, 0x00, 0x00, 0x7a, 0x20, 0x00, 0x10, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x11, 0x00, 0x01, 0x61, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0xf0, 0xff, 0xff, 0x01, 0xf0, 0xff, 0xff,
            0x01, 0xf0, 0xff, 0xff, 0x01, 0xf0, 0xff, 0xff, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x7a, 0x0c, 0x0c, 0x10, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x07, 0x00, 0x00, 0x70, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x02, 0xa7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x02, 0x70,
            0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x11, 0x00, 0x00, 0x00,
            0x04, 0x70, 0x00, 0x00, 0x00, 0x00, 0x0d, 0x00, 0x05, 0x71, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x04, 0x00, 0x00, 0x7a,
            0x24, 0x40, 0x10, 0x00, 0x00, 0x30, 0x13, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x30,
            0x20, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05,
        ];

        let mut encoded = [0u8; intel_gen9_4a::NOOP_BATCH_BYTES];
        assert_eq!(
            intel_gen9_4a::encode_noop_batch(&mut encoded, SEQNO),
            Some(intel_gen9_4a::NOOP_BATCH_BYTES)
        );
        assert_eq!(encoded, EXPECTED);
        // STATE_BASE_ADDRESS (the iter6 fix) opens at DWord index 7 (byte 28).
        assert_eq!(&encoded[28..32], &[0x11, 0x00, 0x01, 0x61]);
        assert_eq!(intel_gen9_4a::STATE_BASE_ADDRESS_DWORDS, 19);
        assert_eq!(intel_gen9_4a::NOOP_BATCH_DWORDS, 69);
    }

    #[test]
    fn intel_gen9_add_batch_matches_hand_byte_oracle() {
        const SEQNO: u32 = 0x1020_3040;
        // iter8 Add batch = the 69-DW no-op shape with the GPGPU_WALKER Indirect Data
        // fields set (the [cross][per-thread] payload heap) and the masks trimmed to
        // the 2 live SIMD32 lanes. No MEDIA_CURBE_LOAD (NEO indirect-data path).
        const EXPECTED: [u8; intel_gen9_4a::ADD_BATCH_BYTES] = [
            0x02, 0x03, 0x04, 0x69, 0x04, 0x00, 0x00, 0x7a, 0x20, 0x00, 0x10, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x11, 0x00, 0x01, 0x61, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0xf0, 0xff, 0xff, 0x01, 0xf0, 0xff, 0xff,
            0x01, 0xf0, 0xff, 0xff, 0x01, 0xf0, 0xff, 0xff, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x7a, 0x0c, 0x0c, 0x10, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x07, 0x00, 0x00, 0x70, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x02, 0xa7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x02, 0x70,
            0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x11, 0x00, 0x00, 0x00,
            0x04, 0x70, 0x00, 0x00, 0x00, 0x00, 0x0d, 0x00, 0x05, 0x71, 0x00, 0x00, 0x00, 0x00,
            0x40, 0x01, 0x00, 0x00, 0x00, 0x40, 0x13, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x7a,
            0x24, 0x40, 0x10, 0x00, 0x00, 0x30, 0x13, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x30,
            0x20, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05,
        ];

        let mut encoded = [0u8; intel_gen9_4a::ADD_BATCH_BYTES];
        assert_eq!(
            intel_gen9_4a::encode_add_batch(&mut encoded, SEQNO),
            Some(intel_gen9_4a::ADD_BATCH_BYTES)
        );
        assert_eq!(encoded, EXPECTED);
        assert_eq!(intel_gen9_4a::ADD_BATCH_DWORDS, 69);
        // SBA header still opens at byte 28 (the iter6 fix is preserved).
        assert_eq!(&encoded[28..32], &[0x11, 0x00, 0x01, 0x61]);
        // MEDIA_VFE_STATE DW5 (byte 148): URB alloc 2 | CURBE alloc 0 = 0x00020000
        // (no CURBE — the payload comes via the walker indirect-data path).
        assert_eq!(&encoded[148..152], &[0x00, 0x00, 0x02, 0x00]);
        // MEDIA_INTERFACE_DESCRIPTOR_LOAD follows MEDIA_VFE_STATE directly at byte 164
        // (header 0x70020002) — proving NO MEDIA_CURBE_LOAD was inserted.
        assert_eq!(&encoded[164..168], &[0x02, 0x00, 0x02, 0x70]);
        // GPGPU_WALKER Indirect Data: DW2 length (byte 196) = 0x140 (320), DW3 start
        // address (byte 200) = GGTT curbe_base 0x00134000.
        assert_eq!(&encoded[196..200], &[0x40, 0x01, 0x00, 0x00]);
        assert_eq!(
            u32::from_le_bytes([encoded[200], encoded[201], encoded[202], encoded[203]]),
            intel_gen9_4a::GGTT_CURBE_BASE as u32
        );
        assert_eq!(intel_gen9_4a::WALKER_INDIRECT_DATA_LENGTH, 320);
        // GPGPU_WALKER right/bottom execution masks (bytes 240/244) trimmed to the 2
        // live lanes (0x3) — the no-op keeps 0xffffffff.
        assert_eq!(&encoded[240..244], &[0x03, 0x00, 0x00, 0x00]);
        assert_eq!(&encoded[244..248], &[0x03, 0x00, 0x00, 0x00]);
        assert_eq!(
            intel_gen9_4a::add_oracle_output(),
            [0x00, 0x00, 0x80, 0x40, 0x00, 0x00, 0xc0, 0x40]
        );
        assert_eq!(intel_gen9_4a::ADD_RESULT_TOLERANCE_ULP, 0);
    }

    /// iter8 ISC-5: independent hand-laid byte fixture for the per-thread `local_id`
    /// block (NEO `generateLocalIDsSimd` layout). For the single SIMD32 thread over
    /// the 2-element NDRange: `local_id.x` lane 0 = 0, lane 1 = 1 (u16 at byte 2),
    /// lanes 2..31 = 0; `.y`/`.z` (bytes 64.. / 128..) = 0. 192 B = 6 GRF.
    #[test]
    fn intel_gen9_local_id_payload_matches_hand_byte_oracle() {
        let p = intel_gen9_4a::build_local_id_payload();
        assert_eq!(p.len(), 192);
        assert_eq!(intel_gen9_4a::LOCAL_ID_PAYLOAD_BYTES, 192);
        // local_id.x: lane 0 (bytes 0..2) = 0, lane 1 (bytes 2..4) = 1 (u16 LE).
        assert_eq!(&p[0..2], &[0x00, 0x00]);
        assert_eq!(&p[2..4], &[0x01, 0x00]);
        // lanes 2..31 of x (bytes 4..64) = 0; .y (64..128) and .z (128..192) = 0.
        assert!(p[4..].iter().all(|&b| b == 0));
        assert_eq!(intel_gen9_4a::LOCAL_ID_OFF_X, 0);
        assert_eq!(intel_gen9_4a::LOCAL_ID_OFF_Y, 64);
        assert_eq!(intel_gen9_4a::LOCAL_ID_OFF_Z, 128);
        // 6-GRF per-thread span (192/32); matches the IDD Constant Indirect URB read.
        assert_eq!(intel_gen9_4a::CURBE_PER_THREAD_GRF, 6);
        // The walker Indirect Data Length covers cross (96) + per-thread (192) = 288,
        // aligned up to 64 = 320; the heap is 288 bytes.
        assert_eq!(intel_gen9_4a::INDIRECT_HEAP_BYTES, 288);
        assert_eq!(intel_gen9_4a::WALKER_INDIRECT_DATA_LENGTH, 320);
    }

    /// iter7 ISC-7: independent hand-laid byte fixture for the stateless-A64
    /// cross-thread (CURBE) argument payload. Hand-computed here for known GGTT
    /// buffer addresses, asserted against `build_curbe_payload` independently of the
    /// encoder. Offsets cross-checked against `add_f32x2.gen9.zeinfo`
    /// (payload_arguments: a@32, b@40, o@48, local_size@12, enqueued_local_size@64).
    #[test]
    fn intel_gen9_curbe_payload_matches_hand_byte_oracle() {
        // Distinct, easily-recognizable A64 base pointers (the BUILD-4a GGTT bases).
        const LHS: u64 = 0x0013_0000;
        const RHS: u64 = 0x0013_1000;
        const OUT: u64 = 0x0013_2000;
        // Hand-laid 96-byte payload: zeros except local_size/enqueued_local_size =
        // (2,1,1) and the three little-endian A64 pointers at bytes 32/40/48.
        const EXPECTED: [u8; intel_gen9_4a::CURBE_PAYLOAD_BYTES] = [
            // [0..12) global_id_offset = 0,0,0
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // [12..24) local_size = 2,1,1
            0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
            // [24..32) pad
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // [32..40) arg a = 0x0013_0000 (LE u64)
            0x00, 0x00, 0x13, 0x00, 0x00, 0x00, 0x00, 0x00,
            // [40..48) arg b = 0x0013_1000
            0x00, 0x10, 0x13, 0x00, 0x00, 0x00, 0x00, 0x00,
            // [48..56) arg o = 0x0013_2000
            0x00, 0x20, 0x13, 0x00, 0x00, 0x00, 0x00, 0x00, // [56..64) pad
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // [64..76) enqueued_local_size = 2,1,1
            0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
            // [76..96) pad
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let payload = intel_gen9_4a::build_curbe_payload(LHS, RHS, OUT);
        assert_eq!(payload, EXPECTED);
        // Spot-assert the live contract fields independently (the zeinfo offsets).
        assert_eq!(intel_gen9_4a::CURBE_OFF_ARG_A, 32);
        assert_eq!(intel_gen9_4a::CURBE_OFF_ARG_B, 40);
        assert_eq!(intel_gen9_4a::CURBE_OFF_ARG_O, 48);
        assert_eq!(&payload[32..40], &LHS.to_le_bytes());
        assert_eq!(&payload[40..48], &RHS.to_le_bytes());
        assert_eq!(&payload[48..56], &OUT.to_le_bytes());
        // 3-GRF cross-thread span (ceil(76/32)); matches the IDD read length.
        assert_eq!(intel_gen9_4a::CURBE_CROSS_THREAD_GRF, 3);
        assert_eq!(intel_gen9_4a::CURBE_PAYLOAD_BYTES, 96);
    }

    /// BUILD-4b-1 ISC-6: independent hand-laid byte fixture for the matmul
    /// cross-thread payload. Hand-computed for known GGTT operand addresses + dims,
    /// asserted against `build_matmul_curbe_payload` independently of the encoder.
    /// Offsets cross-checked against `matmul_f32.gen9.zeinfo` (a@32, b@40, c@48,
    /// m@56, n@60, k@64, local_size@12, enqueued_local_size@68) — its OWN ABI, not
    /// the Add's (the dims + enqueued offset differ).
    #[test]
    fn intel_gen9_matmul_payload_matches_hand_byte_oracle() {
        const A: u64 = 0x0013_0000;
        const B: u64 = 0x0013_1000;
        const C: u64 = 0x0013_2000;
        const M: u32 = 2;
        const N: u32 = 2;
        const K: u32 = 2;
        // m·n = 4 work items → local_size / enqueued_local_size = (4,1,1).
        const EXPECTED: [u8; intel_gen9_4a::CURBE_PAYLOAD_BYTES] = [
            // [0..12) global_id_offset = 0,0,0
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // [12..24) local_size = 4,1,1
            0x04, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
            // [24..32) pad
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // [32..40) a = 0x0013_0000
            0x00, 0x00, 0x13, 0x00, 0x00, 0x00, 0x00, 0x00, // [40..48) b = 0x0013_1000
            0x00, 0x10, 0x13, 0x00, 0x00, 0x00, 0x00, 0x00, // [48..56) c = 0x0013_2000
            0x00, 0x20, 0x13, 0x00, 0x00, 0x00, 0x00, 0x00,
            // [56..60) m=2  [60..64) n=2  [64..68) k=2
            0x02, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
            // [68..80) enqueued_local_size = 4,1,1
            0x04, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
            // [80..96) pad
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let payload = intel_gen9_4a::build_matmul_curbe_payload(A, B, C, M, N, K);
        assert_eq!(payload, EXPECTED);
        // Spot-assert the live contract fields independently (the zeinfo offsets).
        assert_eq!(intel_gen9_4a::MATMUL_OFF_ARG_A, 32);
        assert_eq!(intel_gen9_4a::MATMUL_OFF_ARG_B, 40);
        assert_eq!(intel_gen9_4a::MATMUL_OFF_ARG_C, 48);
        assert_eq!(intel_gen9_4a::MATMUL_OFF_M, 56);
        assert_eq!(intel_gen9_4a::MATMUL_OFF_N, 60);
        assert_eq!(intel_gen9_4a::MATMUL_OFF_K, 64);
        assert_eq!(intel_gen9_4a::MATMUL_OFF_ENQUEUED_LOCAL_SIZE, 68);
        assert_eq!(&payload[32..40], &A.to_le_bytes());
        assert_eq!(&payload[40..48], &B.to_le_bytes());
        assert_eq!(&payload[48..56], &C.to_le_bytes());
        assert_eq!(&payload[56..60], &M.to_le_bytes());
        assert_eq!(&payload[64..68], &K.to_le_bytes());
    }

    /// The finalizer-built matmul kernel: real stateless-A64 f32 matmul terminating
    /// in an EOT `send`. Independent oracle: `intel_gen9_kernels/matmul_f32.gen9.asm`.
    #[test]
    fn intel_gen9_matmul_kernel_is_finalizer_eot() {
        assert_eq!(intel_gen9_4a::MATMUL_F32_KERNEL_GEN9.len(), 1216);
        // Opens with the r0 dispatch-header copy mov (DW0 = 0x00600001), as the Add.
        assert_eq!(
            blob_dw(&intel_gen9_4a::MATMUL_F32_KERNEL_GEN9, 0),
            0x0060_0001
        );
        // Terminates in the spawner EOT send.
        assert!(intel_gen9_4a::matmul_kernel_has_eot());
        assert_eq!(
            intel_gen9_4a::MATMUL_F32_KERNEL_EOT_SOURCE,
            "send (1|M0) null r127 0x27 0x02000010 {EOT}"
        );
    }

    /// BUILD-4b-1 ISC-5: the ≤2⁻¹² relative tolerance checker (matmul vs host f32).
    #[test]
    fn intel_gen9_matmul_tolerance_checker() {
        let host = 6.0f32.to_le_bytes();
        // Exact match passes.
        assert!(intel_gen9_4a::matmul_within_tolerance(&host, &host));
        // Within 2⁻¹² relative (|Δ| = 6·2⁻¹³ < 6·2⁻¹²) passes.
        let near = (6.0f32 + 6.0 / 8192.0).to_le_bytes();
        assert!(intel_gen9_4a::matmul_within_tolerance(&near, &host));
        // Beyond tolerance (1% off) fails.
        let far = (6.0f32 * 1.01).to_le_bytes();
        assert!(!intel_gen9_4a::matmul_within_tolerance(&far, &host));
        assert_eq!(intel_gen9_4a::MATMUL_REL_TOLERANCE, 1.0 / 4096.0);
        // The fixed-point max-rel reporter (units of 2⁻²⁴; tolerance = 4096).
        assert_eq!(intel_gen9_4a::MATMUL_REL_TOLERANCE_E24, 4096);
        assert_eq!(intel_gen9_4a::matmul_max_rel_error_e24(&host, &host), 0);
        // near = 6 + 6·2⁻¹³ → rel = 2⁻¹³ → ·2²⁴ = 2048 (≤ 4096).
        assert_eq!(intel_gen9_4a::matmul_max_rel_error_e24(&near, &host), 2048);
        assert!(intel_gen9_4a::matmul_max_rel_error_e24(&far, &host) > 4096);
    }

    /// BUILD-4b-2 ISC-6: independent hand-laid byte fixture for the GELU pointwise
    /// cross-thread payload. Hand-computed for known GGTT in/out addresses + count,
    /// asserted against `build_gelu_curbe_payload` independently of the encoder.
    /// Offsets cross-checked against `gelu_f32.gen9.zeinfo` (in@32, out@40, count@48
    /// byvalue i32, local_size@12, enqueued_local_size@52) — its OWN ABI, NOT the
    /// matmul's: a pointwise kernel has only two pointers + a scalar count, so the
    /// payload span ends at byte 64 (2 cross-thread GRF, one fewer than matmul's 3).
    #[test]
    fn intel_gen9_gelu_payload_matches_hand_byte_oracle() {
        const IN: u64 = 0x0013_0000;
        const OUT: u64 = 0x0013_2000;
        const COUNT: u32 = 4;
        // count = 4 work items → local_size / enqueued_local_size = (4,1,1).
        const EXPECTED: [u8; intel_gen9_4a::CURBE_PAYLOAD_BYTES] = [
            // [0..12) global_id_offset = 0,0,0
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // [12..24) local_size = 4,1,1
            0x04, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
            // [24..32) pad
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // [32..40) in = 0x0013_0000
            0x00, 0x00, 0x13, 0x00, 0x00, 0x00, 0x00, 0x00, // [40..48) out = 0x0013_2000
            0x00, 0x20, 0x13, 0x00, 0x00, 0x00, 0x00, 0x00, // [48..52) count = 4
            0x04, 0x00, 0x00, 0x00, // [52..64) enqueued_local_size = 4,1,1
            0x04, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
            // [64..96) pad
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];
        let payload = intel_gen9_4a::build_gelu_curbe_payload(IN, OUT, COUNT);
        assert_eq!(payload, EXPECTED);
        // Spot-assert the live contract fields independently (the zeinfo offsets).
        assert_eq!(intel_gen9_4a::GELU_OFF_ARG_IN, 32);
        assert_eq!(intel_gen9_4a::GELU_OFF_ARG_OUT, 40);
        assert_eq!(intel_gen9_4a::GELU_OFF_COUNT, 48);
        assert_eq!(intel_gen9_4a::GELU_OFF_ENQUEUED_LOCAL_SIZE, 52);
        // Pointwise GELU reads 2 cross-thread GRF (span [0,64)), vs matmul's 3.
        assert_eq!(intel_gen9_4a::GELU_CROSS_THREAD_GRF, 2);
        assert_eq!(&payload[32..40], &IN.to_le_bytes());
        assert_eq!(&payload[40..48], &OUT.to_le_bytes());
        assert_eq!(&payload[48..52], &COUNT.to_le_bytes());
    }

    /// The M710q regression: HD 530 `0x1912` must be bound, with its SKU label.
    ///
    /// Janus's first M710q Slice-4 run failed `candidates=0 probe_ok=0` and never
    /// reached the EU, purely because this ID was absent. QEMU cannot catch that
    /// class — it has no Intel iGPU — so this is where it gets caught.
    #[test]
    fn intel_accel_allowlist_binds_the_m710q_hd530() {
        assert!(accel_intel_ids::is_supported_gen9_igpu_id(0x1912));
        assert_eq!(
            accel_intel_ids::gen9_sku_name(0x1912),
            Some("HD530-SKL-GT2")
        );
    }

    /// The OptiPlex IDs that Phase 6/7 actually proved on silicon stay bound —
    /// widening the allowlist must never silently drop the known-good silicon.
    #[test]
    fn intel_accel_allowlist_keeps_the_proven_uhd630_ids() {
        for id in [0x9bc8u16, 0x9bc5, 0x3e92, 0x3e98] {
            assert!(
                accel_intel_ids::is_supported_gen9_igpu_id(id),
                "device 0x{id:04x} was proven on OptiPlex silicon and must stay bound"
            );
        }
    }

    /// The allowlist is an allowlist: an Intel display device that is *not* listed
    /// is refused. `0x1916` is a real Skylake GT2 (ULT) part deliberately not in
    /// the table — so this fails if the lookup ever degenerates into "any Gen9".
    #[test]
    fn intel_accel_allowlist_refuses_unlisted_intel_display_devices() {
        for id in [0x1916u16, 0x0412, 0x5912, 0x0000, 0xffff] {
            assert!(!accel_intel_ids::is_supported_gen9_igpu_id(id));
            assert_eq!(accel_intel_ids::gen9_sku_name(id), None);
        }
    }

    /// **The load-bearing invariant.** `intel_gen9_4a` bakes ONE
    /// `MEDIA_VFE_STATE` Maximum-Number-of-Threads value for every device it can
    /// dispatch to. That is only legal while every accepted device has the same EU
    /// count — all `1x3x8` GT2, 24 EU, and Gen9 runs 7 threads/EU (Intel
    /// compute-runtime: `ThreadCount = EUCount * 7u`, identically in
    /// `hw_info_skl.cpp` and `hw_info_cfl.cpp`).
    ///
    /// So this asserts the baked field equals the value *derived* from each entry's
    /// own EU count. Adding a GT1/GT3/GT4 Gen9 part now **fails here** instead of
    /// silently over-programming the VFE thread limit on that SKU — which is the
    /// kind of wrongness a blind bare-metal run reports only as an unexplained hang.
    #[test]
    fn intel_accel_allowlist_agrees_with_the_baked_vfe_thread_limit() {
        let baked = intel_gen9_4a::MEDIA_VFE_STATE_DW3 >> 16;
        assert_eq!(
            baked, 167,
            "MEDIA_VFE_STATE DW3 max-threads is bias-1 of 168"
        );
        for (id, sku, eus) in accel_intel_ids::SUPPORTED_GEN9_IGPUS {
            assert_eq!(
                accel_intel_ids::vfe_max_threads_field(eus),
                baked,
                "0x{id:04x} ({sku}) has {eus} EUs, which does not agree with the \
                 single baked MEDIA_VFE_STATE thread limit — make that per-SKU first"
            );
            assert_eq!(accel_intel_ids::gen9_eu_count(id), Some(eus));
        }
    }

    /// The finalizer-built GELU kernel: real stateless-A64 pointwise GELU terminating
    /// in an EOT `send`. Independent oracle: `intel_gen9_kernels/gelu_f32.gen9.asm`.
    /// The whole point of GELU on silicon is that `tanh` lowers to the Gen9 hardware
    /// extended-math unit — so this also asserts the committed disasm shows a `math`
    /// instruction (not a software polynomial), the property that makes the ≤2⁻¹²
    /// host-f32 tolerance non-vacuous (GPU hw-math != soft-float != host libm).
    #[test]
    fn intel_gen9_gelu_kernel_is_finalizer_eot_with_hw_math() {
        assert_eq!(intel_gen9_4a::GELU_F32_KERNEL_GEN9.len(), 4096);
        // Opens with the r0 dispatch-header copy mov (DW0 = 0x00600001), as Add/matmul.
        assert_eq!(
            blob_dw(&intel_gen9_4a::GELU_F32_KERNEL_GEN9, 0),
            0x0060_0001
        );
        // Terminates in the spawner EOT send.
        assert!(intel_gen9_4a::gelu_kernel_has_eot());
        assert_eq!(
            intel_gen9_4a::GELU_F32_KERNEL_EOT_SOURCE,
            "send (1|M0) null r127 0x27 0x02000010 {EOT}"
        );
        // The committed disasm shows the Gen9 hardware extended-math path (math.exp /
        // math.inv / math.fdiv implementing tanh) — verified at build time, asserted
        // here so a future re-finalize that loses the hardware math (e.g. a polynomial
        // fallback) fails the host gate before it ever reaches silicon.
        const GELU_ASM: &str =
            include_str!("../../../kernel/src/drivers/accel/intel_gen9_kernels/gelu_f32.gen9.asm");
        assert!(
            GELU_ASM.contains("math."),
            "GELU disasm must contain a hardware `math` instruction (the point of GELU on silicon)"
        );
        assert!(
            GELU_ASM.contains("math.exp"),
            "GELU tanh lowers to math.exp on Gen9"
        );
    }

    /// BUILD-4b-2 ISC-6: the ≤2⁻¹² relative tolerance checker (GELU vs host f32). Same
    /// op-agnostic relative-bound math as matmul, exposed under the GELU-named API.
    /// Unlike matmul (bit-exact, no FMA on Gen9), GELU's hardware `tanh` genuinely
    /// diverges, so this bound fires non-vacuously on silicon.
    #[test]
    fn intel_gen9_gelu_tolerance_checker() {
        let host = 0.5f32.to_le_bytes();
        // Exact match passes.
        assert!(intel_gen9_4a::gelu_within_tolerance(&host, &host));
        // Within 2⁻¹² relative passes; the reference is near-1 so scale = max(|h|,1) = 1.
        let near = (0.5f32 + 1.0 / 8192.0).to_le_bytes();
        assert!(intel_gen9_4a::gelu_within_tolerance(&near, &host));
        // Beyond tolerance (1% off) fails.
        let far = (0.5f32 + 0.01).to_le_bytes();
        assert!(!intel_gen9_4a::gelu_within_tolerance(&far, &host));
        assert_eq!(intel_gen9_4a::GELU_REL_TOLERANCE, 1.0 / 4096.0);
        assert_eq!(intel_gen9_4a::GELU_REL_TOLERANCE_E24, 4096);
        assert_eq!(intel_gen9_4a::gelu_max_rel_error_e24(&host, &host), 0);
        // near = 0.5 + 2⁻¹³ → rel (scale 1) = 2⁻¹³ → ·2²⁴ = 2048 (≤ 4096).
        assert_eq!(intel_gen9_4a::gelu_max_rel_error_e24(&near, &host), 2048);
        assert!(intel_gen9_4a::gelu_max_rel_error_e24(&far, &host) > 4096);
    }

    /// BUILD-4b-2 ISC-3/4 reference generator. The accel_intel forward-pass-equivalence
    /// scenario embeds two 16-byte references for the `valid_transformer_block` model
    /// (input [1,2,3,4]; gamma=1, beta=0, W[i]=i·0.1, gamma2=1):
    ///   - EXPECTED_CPU = the soft-float (`ai::ops`) forward — the byte-exact Phase-5
    ///     transparency oracle the pure-accel_cpu run must reproduce.
    ///   - REF_HOST = the host-f32 (libm) forward — the "closest-to-true" reference both
    ///     the pure-CPU and the mixed-GPU runs must land within the accumulated bound B.
    ///     This test recomputes both and asserts they equal the values embedded in
    ///     `test_harness.rs` (printed below for the embed when they change), so CI guards the
    ///     constants. Run with `--nocapture` to print the bytes for embedding.
    #[test]
    fn transformer_forward_references_match_embedded() {
        // valid_transformer_block weights (neomc generate_valid_transformer_block).
        let input = bytes_from_f32(&[1.0, 2.0, 3.0, 4.0]);
        let gamma = bytes_from_f32(&[1.0, 1.0, 1.0, 1.0]);
        let beta = bytes_from_f32(&[0.0, 0.0, 0.0, 0.0]);
        let w_vals: Vec<f32> = (0..16).map(|i| i as f32 * 0.1).collect();
        let w = bytes_from_f32(&w_vals);
        let gamma2 = bytes_from_f32(&[1.0, 1.0, 1.0, 1.0]);
        let s14 = shape2(1, 4);
        let s4 = shape1(4);
        let s44 = shape2(4, 4);

        // EXPECTED_CPU: the actual ai::ops soft-float forward (same code the kernel runs).
        let mut ln = vec![0u8; 16];
        ops::layer_norm_affine(
            &view(&s14, &input),
            &view(&s4, &gamma),
            &view(&s4, &beta),
            &mut ln,
        )
        .unwrap();
        let mut mm = vec![0u8; 16];
        ops::matmul(&view(&s14, &ln), &view(&s44, &w), &mut mm).unwrap();
        let mut g = vec![0u8; 16];
        ops::gelu(&view(&s14, &mm), &mut g).unwrap();
        let mut r = vec![0u8; 16];
        ops::rms_norm_affine(&view(&s14, &g), &view(&s4, &gamma2), &mut r).unwrap();
        let mut expected_cpu = vec![0u8; 16];
        ops::softmax(&view(&s14, &r), &mut expected_cpu).unwrap();

        // REF_HOST: the host-f32 (libm) forward — the closest-to-true reference.
        let ref_host = transformer_host_f32_reference([1.0, 2.0, 3.0, 4.0], &w_vals);
        let ref_host_bytes = bytes_from_f32(&ref_host);

        // GELU op-level host-f32 reference (ISC-3 op-level, the NON-VACUOUS proof). The
        // forward output above is softmax-flattened to ~[0.25;4] (the terminal softmax
        // washes out the upstream GELU divergence), so the forward-level bound is
        // satisfied trivially; the tolerance that genuinely BITES is proven here, at the
        // op level, where the GPU hardware-tanh diverges from libm. 8 spread inputs.
        let gelu_inputs = [-2.0f32, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0, 3.0];
        let gelu_host: Vec<f32> = gelu_inputs.iter().map(|&v| host_f32_gelu(v)).collect();
        let gelu_host_bytes = bytes_from_f32(&gelu_host);
        // The soft-float ai::ops gelu for the same inputs (what accel_cpu computes).
        let gi_bytes = bytes_from_f32(&gelu_inputs);
        let mut gelu_soft = vec![0u8; 32];
        ops::gelu(&view(&shape1(8), &gi_bytes), &mut gelu_soft).unwrap();

        // Print for embedding (visible with --nocapture).
        std::println!("EXPECTED_CPU   = {:02x?}", expected_cpu);
        std::println!("REF_HOST       = {:02x?}", ref_host_bytes);
        std::println!("GELU_OP_INPUT  = {:02x?}", gi_bytes);
        std::println!("GELU_OP_HOST   = {:02x?}", gelu_host_bytes);
        let fwd_rel = intel_gen9_4a::gelu_max_rel_error_e24(&expected_cpu, &ref_host_bytes);
        let gelu_soft_rel = intel_gen9_4a::gelu_max_rel_error_e24(&gelu_soft, &gelu_host_bytes);
        std::println!(
            "fwd cpu_vs_host max_rel_e24={} (B_e24={}); gelu soft_vs_host max_rel_e24={}",
            fwd_rel,
            16_777_216u32 / 512,
            gelu_soft_rel
        );

        // The values embedded in kernel/src/test_harness.rs. Asserting here keeps the
        // kernel constants honest (CI fails if the op semantics drift).
        const EMBED_EXPECTED_CPU: [u8; 16] = [
            0xff, 0xff, 0x7f, 0x3e, 0x00, 0x00, 0x80, 0x3e, 0xfe, 0xff, 0x7f, 0x3e, 0xfe, 0xff,
            0x7f, 0x3e,
        ];
        const EMBED_REF_HOST: [u8; 16] = [
            0xff, 0xff, 0x7f, 0x3e, 0x00, 0x00, 0x80, 0x3e, 0xfe, 0xff, 0x7f, 0x3e, 0xfe, 0xff,
            0x7f, 0x3e,
        ];
        const EMBED_GELU_OP_HOST: [u8; 32] = [
            0xc0, 0xf7, 0x39, 0xbd, 0x90, 0x9e, 0x22, 0xbe, 0x26, 0xfd, 0x1d, 0xbe, 0x00, 0x00,
            0x00, 0x00, 0x6d, 0x01, 0xb1, 0x3e, 0x5c, 0x58, 0x57, 0x3f, 0x42, 0x30, 0xfa, 0x3f,
            0x68, 0xc4, 0x3f, 0x40,
        ];
        assert_eq!(
            expected_cpu.as_slice(),
            EMBED_EXPECTED_CPU,
            "soft-float forward changed; update TRANSFORMER_EXPECTED_CPU in test_harness.rs"
        );
        assert_eq!(
            ref_host_bytes.as_slice(),
            EMBED_REF_HOST,
            "libm forward changed; update TRANSFORMER_REF_HOST in test_harness.rs"
        );
        assert_eq!(
            gelu_host_bytes.as_slice(),
            EMBED_GELU_OP_HOST,
            "libm gelu changed; update GELU_OP_HOST_REF in test_harness.rs"
        );
        // The forward bound B (2⁻⁹) is satisfied (here trivially — softmax-flattened).
        assert!(fwd_rel < 16_777_216 / 512);
        // The soft-float gelu is itself within the per-op 2⁻¹² of host (Phase-5).
        assert!(gelu_soft_rel <= intel_gen9_4a::GELU_REL_TOLERANCE_E24);
    }

    /// Host-f32 (libm) GELU for a single element: the op-level "closest-to-true"
    /// reference (ISC-3 op-level). tanh approximation form matching `ai::ops::gelu`.
    fn host_f32_gelu(v: f32) -> f32 {
        let scaled = 0.797_884_6_f32 * (v + 0.044715f32 * v * v * v);
        v * (0.5f32 * (1.0f32 + scaled.tanh()))
    }

    /// Host-f32 (libm) forward for `valid_transformer_block`: the closest-to-true
    /// reference. input → LayerNorm(γ=1,β=0) → MatMul(W) → Gelu(tanh) → RmsNorm(γ=1) →
    /// Softmax, all in native f32 with std libm transcendentals (eps = 1e-5, matching
    /// the kernel's 0x3727c5ac). The op formulas mirror `ai::ops`; this is the host-f32
    /// "truth" both the soft-float and GPU runs are bounded against (ISC-3).
    fn transformer_host_f32_reference(x: [f32; 4], w: &[f32]) -> [f32; 4] {
        // LayerNorm over the 4-vector (γ=1, β=0).
        let mean = (x[0] + x[1] + x[2] + x[3]) / 4.0;
        let var = ((x[0] - mean).powi(2)
            + (x[1] - mean).powi(2)
            + (x[2] - mean).powi(2)
            + (x[3] - mean).powi(2))
            / 4.0;
        let std = (var + 1e-5f32).sqrt();
        let ln: [f32; 4] = core::array::from_fn(|i| (x[i] - mean) / std);
        // MatMul ln[1,4] · W[4,4] (row-major), native f32 accumulation.
        let mm: [f32; 4] =
            core::array::from_fn(|j| (0..4).map(|i| ln[i] * w[i * 4 + j]).sum::<f32>());
        // GELU (tanh approximation) via libm tanh.
        let gelu = |v: f32| {
            let scaled = 0.797_884_6_f32 * (v + 0.044715f32 * v * v * v);
            v * (0.5f32 * (1.0f32 + scaled.tanh()))
        };
        let g: [f32; 4] = core::array::from_fn(|i| gelu(mm[i]));
        // RmsNorm (γ=1).
        let ms = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2] + g[3] * g[3]) / 4.0;
        let rms = (ms + 1e-5f32).sqrt();
        let rr: [f32; 4] = core::array::from_fn(|i| g[i] / rms);
        // Softmax (subtract max, exp, normalize).
        let mx = rr.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let ex: [f32; 4] = core::array::from_fn(|i| (rr[i] - mx).exp());
        let s = ex[0] + ex[1] + ex[2] + ex[3];
        core::array::from_fn(|i| ex[i] / s)
    }

    fn bytes_from_f32(values: &[f32]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(values.len() * 4);
        for v in values {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        bytes
    }

    /// Read a little-endian native-instruction DWord out of an embedded EU blob.
    fn blob_dw(blob: &[u8], byte: usize) -> u32 {
        u32::from_le_bytes([blob[byte], blob[byte + 1], blob[byte + 2], blob[byte + 3]])
    }

    /// The finalizer-built Add kernel: a real elementwise f32 add terminating in
    /// an EOT `send`. The independent byte oracle is the checked-in disassembly
    /// `intel_gen9_kernels/add_f32x2.gen9.asm` (produced by `ocloc disasm`, a
    /// decode pass distinct from the `ocloc compile` emit pass); this test pins
    /// the structural invariants of the shipped bytes.
    #[test]
    fn intel_gen9_add_kernel_is_finalizer_eot() {
        assert_eq!(intel_gen9_4a::ADD_F32X2_KERNEL_GEN9.len(), 640);
        // Both finalizer kernels open with the `mov` that copies the r0 dispatch
        // header (DW0 = MOV | ExecSize SIMD8 = 0x00600001).
        assert_eq!(
            blob_dw(&intel_gen9_4a::ADD_F32X2_KERNEL_GEN9, 0),
            0x0060_0001
        );
        // Terminates in the spawner EOT send (DW3 descriptor 0x82000010).
        assert!(intel_gen9_4a::add_kernel_has_pinned_eot_tail());
        assert_eq!(
            intel_gen9_4a::ADD_F32X2_KERNEL_EOT_SOURCE,
            "send (1|M0) null r127 0x27 0x02000010 {EOT}"
        );
    }

    #[test]
    fn intel_gen9_smoke_batch_matches_hand_byte_oracle() {
        const SEQNO: u32 = 0x1020_3040;
        const EXPECTED: [u8; intel_gen9_4a::SMOKE_BATCH_BYTES] = [
            0x04, 0x00, 0x00, 0x7a, 0x24, 0x40, 0x10, 0x00, 0x00, 0x30, 0x13, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x40, 0x30, 0x20, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05,
        ];

        let mut encoded = [0u8; intel_gen9_4a::SMOKE_BATCH_BYTES];
        assert_eq!(
            intel_gen9_4a::encode_smoke_batch(&mut encoded, SEQNO),
            Some(intel_gen9_4a::SMOKE_BATCH_BYTES)
        );
        assert_eq!(encoded, EXPECTED);
    }

    /// The finalizer-built EOT-only no-op kernel. Independent byte oracle: the
    /// checked-in disassembly `intel_gen9_kernels/noop_eot.gen9.asm`. This test
    /// pins the structural invariants of the shipped 192-byte blob and bit-asserts
    /// the retirement-critical EOT send descriptor (EOT / mlen / rlen).
    #[test]
    fn intel_gen9_noop_kernel_is_finalizer_eot() {
        assert_eq!(intel_gen9_4a::NOOP_KERNEL_BYTES, 192);
        assert_eq!(intel_gen9_4a::NOOP_KERNEL_GEN9.len(), 192);

        // Leading header `mov` (copies r0), and the spawner EOT `send` (native
        // instruction at byte 40 of the .text: DW0 then DW3 descriptor at +12).
        assert_eq!(
            blob_dw(&intel_gen9_4a::NOOP_KERNEL_GEN9, 0),
            intel_gen9_4a::NOOP_KERNEL_MOV_DW0
        );
        assert_eq!(intel_gen9_4a::NOOP_KERNEL_MOV_DW0, 0x0060_0001);
        assert_eq!(
            blob_dw(&intel_gen9_4a::NOOP_KERNEL_GEN9, 40),
            intel_gen9_4a::NOOP_KERNEL_SEND_DW0
        );
        assert_eq!(intel_gen9_4a::NOOP_KERNEL_SEND_DW0, 0x0700_0031);
        let send_dw3 = blob_dw(&intel_gen9_4a::NOOP_KERNEL_GEN9, 52);
        assert_eq!(send_dw3, intel_gen9_4a::NOOP_KERNEL_SEND_DW3);

        // Bit-assert the retirement-critical send descriptor fields (DW3).
        assert_eq!(send_dw3, 0x8200_0010);
        assert_eq!(send_dw3 & (1 << 31), 1 << 31, "EOT must be set");
        assert_eq!((send_dw3 >> 25) & 0xF, 1, "mlen must be 1");
        assert_eq!((send_dw3 >> 20) & 0x1F, 0, "rlen must be 0");

        assert!(intel_gen9_4a::noop_kernel_has_eot());
        assert_eq!(
            intel_gen9_4a::NOOP_KERNEL_EOT_SOURCE,
            "send (1|M0) null r127 0x27 0x02000010 {EOT}"
        );
    }

    #[test]
    fn relu_zeroes_negative_patterns() {
        let input_bits = [0xbf80_0000, 0, 0x4000_0000];
        let expected = [0, 0, 0x4000_0000];
        let input = bytes_from_bits(&input_bits);
        let mut out = vec![0u8; expected.len() * 4];
        let input_shape = shape2(1, 3);

        ops::relu(&view(&input_shape, &input), &mut out).unwrap();

        assert_eq!(bits_from_bytes(&out), expected);
    }

    #[test]
    fn embedding_gathers_i32_indices_against_hand_byte_oracle() {
        let indices = bytes_from_i32(&[2, 0, 3]);
        let table = bytes_from_bits(&[
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
        ]);
        let expected: [u8; 36] = [
            0x00, 0x00, 0x80, 0xbf, 0x00, 0x00, 0x00, 0xc0, 0x00, 0x00, 0x40, 0xc0, 0x00, 0x00,
            0x80, 0x3f, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x40, 0x40, 0xcd, 0xcc, 0xcc, 0x3d,
            0xcd, 0xcc, 0x4c, 0x3e, 0x9a, 0x99, 0x99, 0x3e,
        ];
        let index_shape = shape1(3);
        let table_shape = shape2(4, 3);
        let mut out = vec![0u8; expected.len()];

        ops::embedding(
            &i32_view(&index_shape, &indices),
            &view(&table_shape, &table),
            &mut out,
        )
        .unwrap();

        assert_eq!(out, expected);
    }

    #[test]
    fn dispatch_embedding_gathers_against_hand_byte_oracle() {
        let indices = bytes_from_i32(&[2, 0, 3]);
        let table = bytes_from_bits(&[
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
        ]);
        let expected: [u8; 36] = [
            0x00, 0x00, 0x80, 0xbf, 0x00, 0x00, 0x00, 0xc0, 0x00, 0x00, 0x40, 0xc0, 0x00, 0x00,
            0x80, 0x3f, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x40, 0x40, 0xcd, 0xcc, 0xcc, 0x3d,
            0xcd, 0xcc, 0x4c, 0x3e, 0x9a, 0x99, 0x99, 0x3e,
        ];
        let index_shape = shape1(3);
        let table_shape = shape2(4, 3);
        let output_shape = shape2(3, 3);
        let inputs = [i32_view(&index_shape, &indices), view(&table_shape, &table)];
        let mut out = vec![0u8; expected.len()];

        ops::dispatch(OpKind::Embedding, &inputs, &output_shape, &mut out).unwrap();

        assert_eq!(out, expected);
    }

    #[test]
    fn embedding_rejects_oob_and_negative_indices_before_read() {
        let table = bytes_from_bits(&[
            0x3f80_0000,
            0x4000_0000,
            0x4040_0000,
            0x4080_0000,
            0x40a0_0000,
            0x40c0_0000,
        ]);
        let table_shape = shape2(2, 3);
        let index_shape = shape1(1);

        for indices in [bytes_from_i32(&[2]), bytes_from_i32(&[-1])] {
            let mut out = vec![0u8; 3 * 4];
            assert_eq!(
                ops::embedding(
                    &i32_view(&index_shape, &indices),
                    &view(&table_shape, &table),
                    &mut out,
                ),
                Err(ops::OpError::Index)
            );
        }
    }

    #[test]
    fn embedding_rejects_wrong_index_dtype() {
        let indices = bytes_from_bits(&[0]);
        let table = bytes_from_bits(&[0x3f80_0000, 0x4000_0000, 0x4040_0000]);
        let index_shape = shape1(1);
        let table_shape = shape2(1, 3);
        let mut out = vec![0u8; 3 * 4];

        assert_eq!(
            ops::embedding(
                &view(&index_shape, &indices),
                &view(&table_shape, &table),
                &mut out,
            ),
            Err(ops::OpError::Dtype)
        );
    }

    #[test]
    fn embedding_rejects_unsupported_input_ranks() {
        let indices = bytes_from_i32(&[0, 1, 2]);
        let table = bytes_from_bits(&[
            0x3f80_0000,
            0x4000_0000,
            0x4040_0000,
            0x4080_0000,
            0x40a0_0000,
            0x40c0_0000,
        ]);
        let mut out = vec![0u8; 3 * 3 * 4];

        assert_eq!(
            ops::embedding(
                &i32_view(&shape2(1, 3), &indices),
                &view(&shape2(2, 3), &table),
                &mut out,
            ),
            Err(ops::OpError::Shape)
        );

        assert_eq!(
            ops::embedding(
                &i32_view(&shape1(3), &indices),
                &view(&shape1(6), &table),
                &mut out,
            ),
            Err(ops::OpError::Shape)
        );
    }

    #[test]
    fn valid_mlp_forward_matches_hand_oracle() {
        let input = bytes_from_bits(&[0x3f80_0000, 0x4000_0000, 0x4040_0000, 0x4080_0000]);
        let w1 = bytes_from_bits(&(0..32).map(int_f32_bits).collect::<Vec<_>>());
        let b1 = bytes_from_bits(&[
            0x0000_0000,
            0x3dcc_cccd,
            0x3e4c_cccd,
            0x3e99_999a,
            0x3ecc_cccd,
            0x3f00_0000,
            0x3f19_999a,
            0x3f33_3333,
        ]);
        let w2 = bytes_from_bits(&[
            0x0000_0000,
            0x3c23_d70a,
            0x3ca3_d70a,
            0x3cf5_c28f,
            0x3d23_d70a,
            0x3d4c_cccc,
            0x3d75_c28f,
            0x3d8f_5c29,
            0x3da3_d70a,
            0x3db8_51eb,
            0x3dcc_cccc,
            0x3de1_47ae,
            0x3df5_c28f,
            0x3e05_1eb8,
            0x3e0f_5c29,
            0x3e19_9999,
            0x3e23_d70a,
            0x3e2e_147b,
            0x3e38_51eb,
            0x3e42_8f5c,
            0x3e4c_cccc,
            0x3e57_0a3d,
            0x3e61_47ae,
            0x3e6b_851e,
        ]);
        let b2 = bytes_from_bits(&[0x0000_0000, 0x3d4c_cccd, 0x3dcc_cccd]);
        let expected = [0x4330_d1ea, 0x4340_7f7d, 0x4350_2d0e];
        let input_shape = shape2(1, 4);
        let w1_shape = shape2(4, 8);
        let hidden_shape = shape2(1, 8);
        let w2_shape = shape2(8, 3);
        let output_shape = shape2(1, 3);

        let mut mm1 = vec![0u8; 8 * 4];
        ops::matmul(&view(&input_shape, &input), &view(&w1_shape, &w1), &mut mm1).unwrap();
        let mut add1 = vec![0u8; 8 * 4];
        ops::add(
            &view(&hidden_shape, &mm1),
            &view(&hidden_shape, &b1),
            &mut add1,
        )
        .unwrap();
        let mut relu1 = vec![0u8; 8 * 4];
        ops::relu(&view(&hidden_shape, &add1), &mut relu1).unwrap();
        let mut mm2 = vec![0u8; 3 * 4];
        ops::matmul(
            &view(&hidden_shape, &relu1),
            &view(&w2_shape, &w2),
            &mut mm2,
        )
        .unwrap();
        let mut out = vec![0u8; 3 * 4];
        ops::add(
            &view(&output_shape, &mm2),
            &view(&output_shape, &b2),
            &mut out,
        )
        .unwrap();

        assert_eq!(bits_from_bytes(&out), expected);
    }

    #[test]
    fn valid_transformer_block_softmax_sums_to_one() {
        let input = bytes_from_bits(&[
            f32_to_bits(1.0),
            f32_to_bits(2.0),
            f32_to_bits(3.0),
            f32_to_bits(4.0),
        ]);
        let gamma = bytes_from_bits(&[f32_to_bits(1.0); 4]);
        let beta = bytes_from_bits(&[f32_to_bits(0.0); 4]);
        let w = bytes_from_bits(
            &(0..16)
                .map(|i| f32_to_bits(i as f32 * 0.1))
                .collect::<Vec<_>>(),
        );
        let gamma2 = bytes_from_bits(&[f32_to_bits(1.0); 4]);

        let vector_shape = shape1(4);
        let input_shape = shape2(1, 4);
        let w_shape = shape2(4, 4);

        let mut ln = vec![0u8; 4 * 4];
        ops::layer_norm_affine(
            &view(&input_shape, &input),
            &view(&vector_shape, &gamma),
            &view(&vector_shape, &beta),
            &mut ln,
        )
        .unwrap();

        let mut mm = vec![0u8; 4 * 4];
        ops::matmul(&view(&input_shape, &ln), &view(&w_shape, &w), &mut mm).unwrap();

        let mut gelu = vec![0u8; 4 * 4];
        ops::gelu(&view(&input_shape, &mm), &mut gelu).unwrap();

        let mut rms = vec![0u8; 4 * 4];
        ops::rms_norm_affine(
            &view(&input_shape, &gelu),
            &view(&vector_shape, &gamma2),
            &mut rms,
        )
        .unwrap();

        let mut out = vec![0u8; 4 * 4];
        ops::softmax(&view(&input_shape, &rms), &mut out).unwrap();

        let out_bits = bits_from_bytes(&out);
        let out_f32: Vec<f32> = out_bits.iter().map(|&b| bits_to_f32(b)).collect();
        let sum: f32 = out_f32.iter().sum();
        assert!(
            (sum - 1.0).abs() <= 0.01,
            "softmax sum = {:.8}, values = {:?}, bits = {:08x?}",
            sum,
            out_f32,
            out_bits
        );

        let rms_f32: Vec<f32> = bits_from_bytes(&rms)
            .iter()
            .map(|&b| bits_to_f32(b))
            .collect();
        let host_max = rms_f32
            .iter()
            .copied()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        let exps: Vec<f32> = rms_f32.iter().map(|&x| (x - host_max).exp()).collect();
        let host_sum: f32 = exps.iter().sum();
        let expected: Vec<f32> = exps.iter().map(|&x| x / host_sum).collect();
        for i in 0..out_f32.len() {
            let err = rel_err(out_f32[i], expected[i]);
            assert!(
                err <= REL_EPS,
                "softmax[{}] = {:.8} vs {:.8}, rel_err = {:.2e} > {:.2e}",
                i,
                out_f32[i],
                expected[i],
                err,
                REL_EPS
            );
        }
    }

    fn view<'a>(shape: &'a Shape, bytes: &'a [u8]) -> TensorView<'a> {
        typed_view(Dtype::F32, shape, bytes)
    }

    fn i32_view<'a>(shape: &'a Shape, bytes: &'a [u8]) -> TensorView<'a> {
        typed_view(Dtype::I32, shape, bytes)
    }

    fn typed_view<'a>(dtype: Dtype, shape: &'a Shape, bytes: &'a [u8]) -> TensorView<'a> {
        TensorView {
            dtype,
            shape,
            bytes,
        }
    }

    fn shape1(cols: u64) -> Shape {
        let mut dims = [0u64; 8];
        dims[0] = cols;
        Shape::new(1, dims).unwrap()
    }

    fn shape2(rows: u64, cols: u64) -> Shape {
        let mut dims = [0u64; 8];
        dims[0] = rows;
        dims[1] = cols;
        Shape::new(2, dims).unwrap()
    }

    fn shape3(a: u64, b: u64, c: u64) -> Shape {
        let mut dims = [0u64; 8];
        dims[0] = a;
        dims[1] = b;
        dims[2] = c;
        Shape::new(3, dims).unwrap()
    }

    fn bytes_from_bits(bits: &[u32]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(bits.len() * 4);
        for bit_pattern in bits {
            bytes.extend_from_slice(&bit_pattern.to_le_bytes());
        }
        bytes
    }

    fn bytes_from_i32(values: &[i32]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(values.len() * 4);
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    fn bits_from_bytes(bytes: &[u8]) -> Vec<u32> {
        bytes
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect()
    }

    fn int_f32_bits(value: u32) -> u32 {
        if value == 0 {
            return 0;
        }
        let high_bit = 31 - value.leading_zeros();
        let exp = high_bit + 127;
        let shifted = value << (23 - high_bit);
        (exp << 23) | (shifted & 0x007f_ffff)
    }

    fn f32_to_bits(f: f32) -> u32 {
        f.to_bits()
    }

    fn soft_matvec(input: &[f32], rows: usize, rhs: &[f32], cols: usize) -> Vec<f32> {
        assert_eq!(input.len(), rows);
        assert_eq!(rhs.len(), rows * cols);
        let mut out = vec![0.0f32; cols];
        for col in 0..cols {
            let mut acc = soft_f32::zero();
            for row in 0..rows {
                acc = soft_f32::add(
                    acc,
                    soft_f32::mul(input[row].to_bits(), rhs[row * cols + col].to_bits()),
                );
            }
            out[col] = f32::from_bits(acc);
        }
        out
    }

    fn sigmoid_via_soft(x: f32) -> f32 {
        let one = 1.0f32.to_bits();
        let sigmoid_den = soft_f32::add(one, soft_f32::exp((-x).to_bits()));
        f32::from_bits(soft_f32::div(one, sigmoid_den))
    }

    fn softplus_via_soft(x: f32) -> f32 {
        let one = 1.0f32.to_bits();
        if x > 0.0 {
            let sum = soft_f32::add(one, soft_f32::exp((-x).to_bits()));
            let correction = soft_f32::ln(sum);
            f32::from_bits(soft_f32::add(x.to_bits(), correction))
        } else {
            let sum = soft_f32::add(one, soft_f32::exp(x.to_bits()));
            f32::from_bits(soft_f32::ln(sum))
        }
    }

    fn bits_to_f32(b: u32) -> f32 {
        f32::from_bits(b)
    }

    fn gguf_tensor(name: &str, ty: GgmlType, dims_in: &[u64]) -> GgufTensorInfo {
        let mut dims = [1u64; GGML_MAX_DIMS];
        for (idx, dim) in dims_in.iter().enumerate() {
            dims[idx] = *dim;
        }
        let elements = dims_in.iter().product::<u64>();
        GgufTensorInfo {
            name: name.to_string(),
            n_dims: dims_in.len() as u32,
            dims,
            ty,
            offset: 0,
            byte_len: elements / ty.block_size() * ty.type_size(),
            data_start: 0,
        }
    }

    fn read_real_qwen_gguf_or_skip(test_name: &str) -> Option<(Vec<u8>, GgufFile)> {
        let Ok(path) = std::env::var("NEO_QWEN25_GGUF") else {
            eprintln!("SKIP {test_name}: NEO_QWEN25_GGUF unset");
            return None;
        };
        let bytes = std::fs::read(&path).expect("read real Qwen2.5 GGUF");
        let parsed = parse_gguf(&bytes).expect("real Qwen2.5 GGUF parses");
        Some((bytes, parsed))
    }

    #[derive(Clone, Debug)]
    struct RealNeomModel {
        bytes: Vec<u8>,
        tensors: Vec<RealNeomTensor>,
        model_config: Option<ModelConfig>,
        layer_config: Qwen2LayerConfig,
    }

    #[derive(Clone, Debug)]
    struct RealNeomTensor {
        name: String,
        dtype: NeomDtype,
        rank: u32,
        dims: [u64; 8],
        weight_off: usize,
        weight_len: usize,
    }

    impl RealNeomModel {
        fn tensor_named_opt(&self, name: &str) -> Option<&RealNeomTensor> {
            self.tensors.iter().find(|tensor| tensor.name == name)
        }

        fn tensor_named(&self, name: &str) -> &RealNeomTensor {
            self.tensor_named_opt(name)
                .unwrap_or_else(|| panic!("real .neom tensor '{name}' exists"))
        }

        fn tensor_data(&self, tensor: &RealNeomTensor) -> &[u8] {
            &self.bytes[tensor.weight_off..tensor.weight_off + tensor.weight_len]
        }
    }

    #[derive(Clone, Debug)]
    struct RealDecodeReport {
        tokens: Vec<u32>,
        steps: Vec<RealDecodeStep>,
        elapsed: std::time::Duration,
    }

    #[derive(Clone, Debug)]
    struct RealDecodeStep {
        token: u32,
        logit: f32,
        runner_up_token: u32,
        runner_up_logit: f32,
    }

    #[derive(Copy, Clone)]
    struct RealLayerContext {
        position: usize,
        layer_idx: usize,
        config_ref: u32,
    }

    struct RealNeomRuntimeState {
        schedule: crate::model_logic::LayerSchedule,
        kv_caches: Qwen2KvCacheSet,
        gdn_states: RealNeomGdnStateSet,
    }

    impl RealNeomRuntimeState {
        fn new(model: &RealNeomModel, max_context: usize) -> Self {
            let config = model.layer_config;
            let layer_count = real_neom_layer_count(model);
            let schedule = real_neom_schedule(model);
            assert_eq!(
                schedule.layers.len(),
                layer_count,
                "real .neom schedule length"
            );
            Self {
                gdn_states: RealNeomGdnStateSet::new(model, &schedule, config),
                schedule,
                kv_caches: Qwen2KvCacheSet::new(layer_count, config, max_context).unwrap(),
            }
        }
    }

    struct RealNeomGdnLayerState {
        layer_idx: usize,
        config_ref: u32,
        config: ops::gdn::GdnLayerConfig,
        recurrent_state: Vec<f32>,
        conv_state: Vec<f32>,
    }

    struct RealNeomGdnStateSet {
        layers: Vec<RealNeomGdnLayerState>,
    }

    impl RealNeomGdnStateSet {
        fn new(
            model: &RealNeomModel,
            schedule: &crate::model_logic::LayerSchedule,
            config: Qwen2LayerConfig,
        ) -> Self {
            let mut layers = Vec::new();
            for (layer_idx, entry) in schedule.layers.iter().enumerate() {
                if entry.kind == crate::model_logic::LayerKind::GatedDeltaNetLinear {
                    let gdn_config = real_neom_gdn_config(model, layer_idx, config);
                    layers.push(RealNeomGdnLayerState {
                        layer_idx,
                        config_ref: entry.config_ref,
                        config: gdn_config,
                        recurrent_state: vec![0.0; gdn_config.state_size()],
                        conv_state: vec![0.0; gdn_config.conv_state_size().unwrap()],
                    });
                }
            }
            Self { layers }
        }

        fn layer_mut(&mut self, layer_idx: usize, config_ref: u32) -> &mut RealNeomGdnLayerState {
            self.layers
                .iter_mut()
                .find(|state| state.layer_idx == layer_idx && state.config_ref == config_ref)
                .unwrap_or_else(|| {
                    panic!(
                        "real .neom GDN state exists for layer {layer_idx} config_ref {config_ref}"
                    )
                })
        }
    }

    #[derive(Clone, Debug)]
    struct LayerFingerprint {
        position: usize,
        layer: Option<usize>,
        stage: String,
        len: usize,
        sum: f64,
        abs_max: f32,
        hash64: u64,
    }

    #[derive(Clone, Debug)]
    struct LayerTraceReference {
        position: usize,
        layer: Option<usize>,
        stage: String,
        len: Option<usize>,
        sum: Option<f64>,
        abs_max: Option<f32>,
        hash64: Option<u64>,
    }

    #[derive(Default, Copy, Clone, Debug, Eq, PartialEq)]
    struct RealDtypeCounts {
        f32: usize,
        q4_k: usize,
        q5_0: usize,
        q5_k: usize,
        q6_k: usize,
        q8_0: usize,
    }

    impl RealDtypeCounts {
        fn record(&mut self, dtype: TensorDtype) {
            match dtype {
                TensorDtype::F32 => self.f32 += 1,
                TensorDtype::Q4K => self.q4_k += 1,
                TensorDtype::Q5_0 => self.q5_0 += 1,
                TensorDtype::Q5K => self.q5_k += 1,
                TensorDtype::Q6K => self.q6_k += 1,
                TensorDtype::Q8_0 => self.q8_0 += 1,
            }
        }

        fn record_layer(&mut self, dtypes: Qwen2LayerDtypes) {
            self.record(dtypes.attn_q);
            self.record(dtypes.attn_k);
            self.record(dtypes.attn_v);
            self.record(dtypes.attn_o);
            self.record_ffn(dtypes);
        }

        fn record_ffn(&mut self, dtypes: Qwen2LayerDtypes) {
            self.record(dtypes.ffn_gate);
            self.record(dtypes.ffn_up);
            self.record(dtypes.ffn_down);
        }

        fn summary(self) -> String {
            format!(
                "f32={} q4_K={} q5_0={} q5_K={} q6_K={} q8_0={}",
                self.f32, self.q4_k, self.q5_0, self.q5_k, self.q6_k, self.q8_0
            )
        }

        /// Total number of quantized (non-f32) weight tensors counted.
        fn total_quant(&self) -> usize {
            self.q4_k + self.q5_0 + self.q5_k + self.q6_k + self.q8_0
        }

        /// True iff every quant type present in `model` was also exercised here
        /// — i.e. the forward matmul did not silently skip a quant path the model
        /// actually contains. Data-driven so any model's quant mix passes: the
        /// 0.5B's q5_0/q6_K/q8_0 and the 1.5B Q4_K_M's q4_K/q6_K alike.
        fn covers(&self, model: &RealDtypeCounts) -> bool {
            (model.q4_k == 0 || self.q4_k > 0)
                && (model.q5_0 == 0 || self.q5_0 > 0)
                && (model.q5_k == 0 || self.q5_k > 0)
                && (model.q6_k == 0 || self.q6_k > 0)
                && (model.q8_0 == 0 || self.q8_0 > 0)
        }
    }

    #[derive(Default, Clone, Debug)]
    struct RealForwardTrace {
        dtype_counts: RealDtypeCounts,
        layer_trace: Vec<LayerFingerprint>,
    }

    impl RealForwardTrace {
        fn record_layer_trace(
            &mut self,
            position: usize,
            layer: Option<usize>,
            stage: &str,
            values: &[f32],
        ) {
            self.layer_trace
                .push(layer_fingerprint(position, layer, stage, values));
        }
    }

    fn read_real_neom_or_skip(test_name: &str) -> Option<RealNeomModel> {
        let Ok(path) = std::env::var("NEO_REAL_NEOM") else {
            eprintln!("SKIP {test_name}: NEO_REAL_NEOM unset");
            return None;
        };
        let bytes = std::fs::read(&path).expect("read real Qwen2.5 .neom");
        let validated = validate_neom(&bytes).expect("real Qwen2.5 .neom validates");
        let model_config =
            derive_model_config(&bytes, &validated).expect("real Qwen2.5 .neom config derives");
        let layer_config =
            Qwen2LayerConfig::for_model_config(model_config).expect("real Qwen2.5 config selects");
        let mut model = parse_real_neom(&bytes).expect("real Qwen2.5 .neom parses");
        model.model_config = model_config;
        model.layer_config = layer_config;
        Some(model)
    }

    fn read_real_neom_parse_only_or_skip(test_name: &str) -> Option<(RealNeomModel, ModelConfig)> {
        let Ok(path) = std::env::var("NEO_REAL_NEOM") else {
            eprintln!("SKIP {test_name}: NEO_REAL_NEOM unset");
            return None;
        };
        let bytes = std::fs::read(&path).expect("read real .neom");
        let validated = validate_neom(&bytes).expect("real .neom validates");
        let model_config = derive_model_config(&bytes, &validated)
            .expect("real .neom config derives")
            .expect("real .neom has config metadata");
        let mut model = parse_real_neom(&bytes).expect("real .neom parses");
        model.model_config = Some(model_config);
        Some((model, model_config))
    }

    #[cfg(feature = "phase9-real-quant")]
    fn require_phase9_real_quant_feature() {}

    #[cfg(not(feature = "phase9-real-quant"))]
    fn require_phase9_real_quant_feature() {
        panic!(
            "NEO_REAL_NEOM was set; rerun with `--features phase9-real-quant` so F1 mixed-quant matmul is active"
        );
    }

    fn parse_real_neom(bytes: &[u8]) -> Result<RealNeomModel, String> {
        if bytes.len() < 96 {
            return Err("real .neom is shorter than the fixed header".to_string());
        }
        if bytes.get(0..4) != Some(b"NEOM") {
            return Err("real .neom has invalid magic".to_string());
        }
        let version = read_u32_le_at(bytes, 4)?;
        let header_size = read_u32_le_at(bytes, 8)? as usize;
        let flags = read_u32_le_at(bytes, 12)?;
        if !matches!(version, 1..=4) {
            return Err(format!("real .neom version {version} is unsupported"));
        }
        if (version == 1 && header_size != 96)
            || (version == 2 && header_size != 108)
            || (version == 3 && header_size != 124)
            || (version == 4 && header_size != 128)
        {
            return Err(format!(
                "real .neom version {version} has bad header size {header_size}"
            ));
        }
        if flags != 0 {
            return Err(format!("real .neom flags must be zero, got {flags}"));
        }
        let tensor_count = read_u32_le_at(bytes, 16)? as usize;
        let tensor_table_off = read_u64_le_at(bytes, 32)? as usize;
        let string_off = read_u64_le_at(bytes, 56)? as usize;
        let string_len = read_u64_le_at(bytes, 64)? as usize;
        let total_size = read_u64_le_at(bytes, 88)? as usize;
        if total_size != bytes.len() {
            return Err(format!(
                "real .neom total_size {total_size} != file size {}",
                bytes.len()
            ));
        }
        let tensor_table_len = tensor_count
            .checked_mul(96)
            .ok_or_else(|| "real .neom tensor table length overflow".to_string())?;
        checked_slice(bytes, tensor_table_off, tensor_table_len)?;
        checked_slice(bytes, string_off, string_len)?;

        let mut tensors = Vec::with_capacity(tensor_count);
        for idx in 0..tensor_count {
            let base = tensor_table_off + idx * 96;
            let dtype = NeomDtype::from_abi(read_u32_le_at(bytes, base)?)
                .map_err(|_| format!("real .neom tensor[{idx}] has invalid dtype"))?;
            let rank = read_u32_le_at(bytes, base + 4)?;
            let mut dims = [0u64; 8];
            for (dim_idx, dim) in dims.iter_mut().enumerate() {
                *dim = read_u64_le_at(bytes, base + 8 + dim_idx * 8)?;
            }
            let weight_off = read_u64_le_at(bytes, base + 72)? as usize;
            let weight_len = read_u64_le_at(bytes, base + 80)? as usize;
            let name_off = read_u32_le_at(bytes, base + 88)? as usize;
            let name_len = read_u32_le_at(bytes, base + 92)? as usize;
            checked_slice(bytes, weight_off, weight_len)?;
            let name_start = string_off
                .checked_add(name_off)
                .ok_or_else(|| "real .neom name offset overflow".to_string())?;
            let name_bytes = checked_slice(bytes, name_start, name_len)?;
            let name = std::str::from_utf8(name_bytes)
                .map_err(|err| format!("real .neom tensor[{idx}] name is not UTF-8: {err}"))?
                .to_string();
            tensors.push(RealNeomTensor {
                name,
                dtype,
                rank,
                dims,
                weight_off,
                weight_len,
            });
        }

        Ok(RealNeomModel {
            bytes: bytes.to_vec(),
            tensors,
            model_config: None,
            layer_config: Qwen2LayerConfig::qwen25_05b(),
        })
    }

    fn checked_slice(bytes: &[u8], start: usize, len: usize) -> Result<&[u8], String> {
        let end = start
            .checked_add(len)
            .ok_or_else(|| "real .neom range overflow".to_string())?;
        bytes
            .get(start..end)
            .ok_or_else(|| format!("real .neom range {start}..{end} is out of bounds"))
    }

    fn real_neom_layer_count(model: &RealNeomModel) -> usize {
        #[cfg(feature = "phase9-data-driven-config")]
        {
            model
                .model_config
                .expect("feature-on real .neom config is required")
                .layer_count
        }
        #[cfg(not(feature = "phase9-data-driven-config"))]
        {
            let _ = model;
            forward_logic::QWEN25_05B_LAYER_COUNT
        }
    }

    fn real_neom_schedule(model: &RealNeomModel) -> crate::model_logic::LayerSchedule {
        let validated = validate_neom(&model.bytes).expect("real .neom validates");
        crate::model_logic::resolve_schedule(
            &model.bytes,
            &validated.header,
            real_neom_layer_count(model),
        )
        .expect("real .neom schedule resolves")
    }

    fn real_neom_vocab_size(model: &RealNeomModel) -> usize {
        #[cfg(feature = "phase9-data-driven-config")]
        {
            model
                .model_config
                .expect("feature-on real .neom config is required")
                .vocab
        }
        #[cfg(not(feature = "phase9-data-driven-config"))]
        {
            let _ = model;
            REAL_QWEN_VOCAB_SIZE
        }
    }

    fn real_neom_stage_a_prompt(model: &RealNeomModel) -> Vec<u32> {
        let raw_prompt = std::env::var("NEO_REAL_PROMPT").unwrap_or_else(|_| "Hello".to_string());
        let neom_path = std::env::var("NEO_REAL_NEOM").expect("NEO_REAL_NEOM set");
        let vocab_path = std::path::Path::new(&neom_path).with_extension("vocab");
        let sidecar = std::fs::read(&vocab_path)
            .unwrap_or_else(|err| panic!("read .vocab sidecar {}: {err}", vocab_path.display()));
        let real_vocab = crate::vocab_sidecar::try_build_real_vocab(&sidecar)
            .expect("build real vocab from sidecar");
        let prompt = real_vocab
            .tokenizer
            .encode(&raw_prompt)
            .expect("encode NEO_REAL_PROMPT with real tokenizer sidecar");
        assert!(
            !prompt.is_empty(),
            "NEO_REAL_PROMPT must encode to at least one token"
        );
        assert!(
            prompt
                .iter()
                .all(|&token| (token as usize) < real_neom_vocab_size(model)),
            "NEO_REAL_PROMPT produced an out-of-vocabulary token"
        );
        eprintln!("REAL_MODEL_STAGE_A raw_prompt={raw_prompt:?} prompt_ids={prompt:?}");
        prompt
    }

    fn expected_llama_tokens_from_env() -> Vec<u32> {
        let value = std::env::var("NEO_REAL_LLAMA_TOKENS").expect(
            "NEO_REAL_NEOM was set; set NEO_REAL_LLAMA_TOKENS to llama.cpp generated token IDs",
        );
        parse_token_id_list(&value).expect("parse NEO_REAL_LLAMA_TOKENS")
    }

    fn parse_token_id_list(value: &str) -> Result<Vec<u32>, String> {
        let mut out = Vec::new();
        for part in value.split([',', ' ', '\n', '\r', '\t']) {
            if part.is_empty() {
                continue;
            }
            out.push(
                part.parse::<u32>()
                    .map_err(|err| format!("token id '{part}' is invalid: {err}"))?,
            );
        }
        if out.is_empty() {
            return Err("token list is empty".to_string());
        }
        Ok(out)
    }

    fn layer_trace_tokens_from_env() -> Option<Vec<u32>> {
        std::env::var("NEO_REAL_LAYER_TRACE_TOKENS")
            .ok()
            .map(|value| parse_token_id_list(&value).expect("parse NEO_REAL_LAYER_TRACE_TOKENS"))
    }

    fn parse_layer_trace_reference(value: &str) -> Result<Vec<LayerTraceReference>, String> {
        let mut entries = Vec::new();
        for (entry_idx, raw_entry) in value.split([';', '\n']).enumerate() {
            let raw_entry = raw_entry.trim();
            if raw_entry.is_empty() {
                continue;
            }
            let mut position = None;
            let mut layer = None;
            let mut layer_seen = false;
            let mut stage = None;
            let mut len = None;
            let mut sum = None;
            let mut abs_max = None;
            let mut hash64 = None;
            for field in raw_entry.split([',', ' ', '\t']) {
                let field = field.trim();
                if field.is_empty() {
                    continue;
                }
                let (key, val) = field.split_once('=').ok_or_else(|| {
                    format!("layer trace entry {entry_idx} field '{field}' missing '='")
                })?;
                match key {
                    "pos" | "position" => {
                        position = Some(parse_usize_field(key, val)?);
                    }
                    "layer" => {
                        layer_seen = true;
                        layer = parse_optional_layer(val)?;
                    }
                    "stage" | "name" => {
                        if val.is_empty() {
                            return Err(format!("layer trace entry {entry_idx} has empty stage"));
                        }
                        stage = Some(val.to_string());
                    }
                    "len" | "length" => {
                        len = Some(parse_usize_field(key, val)?);
                    }
                    "sum" => {
                        sum = Some(val.parse::<f64>().map_err(|err| {
                            format!("entry {entry_idx} sum '{val}' invalid: {err}")
                        })?);
                    }
                    "absmax" | "abs_max" => {
                        abs_max = Some(val.parse::<f32>().map_err(|err| {
                            format!("entry {entry_idx} absmax '{val}' invalid: {err}")
                        })?);
                    }
                    "hash" | "hash64" | "fnv64" => {
                        hash64 = Some(parse_u64_field(key, val)?);
                    }
                    other => {
                        return Err(format!(
                            "layer trace entry {entry_idx} has unknown field '{other}'"
                        ));
                    }
                }
            }
            if !layer_seen {
                layer = None;
            }
            if len.is_none() && sum.is_none() && abs_max.is_none() && hash64.is_none() {
                return Err(format!(
                    "layer trace entry {entry_idx} needs at least one of len/sum/absmax/hash"
                ));
            }
            entries.push(LayerTraceReference {
                position: position
                    .ok_or_else(|| format!("layer trace entry {entry_idx} missing pos"))?,
                layer,
                stage: stage
                    .ok_or_else(|| format!("layer trace entry {entry_idx} missing stage"))?,
                len,
                sum,
                abs_max,
                hash64,
            });
        }
        if entries.is_empty() {
            return Err("layer trace is empty".to_string());
        }
        Ok(entries)
    }

    fn parse_usize_field(key: &str, value: &str) -> Result<usize, String> {
        value
            .parse::<usize>()
            .map_err(|err| format!("{key} '{value}' invalid: {err}"))
    }

    fn parse_u64_field(key: &str, value: &str) -> Result<u64, String> {
        if let Some(hex) = value
            .strip_prefix("0x")
            .or_else(|| value.strip_prefix("0X"))
        {
            u64::from_str_radix(hex, 16).map_err(|err| format!("{key} '{value}' invalid: {err}"))
        } else {
            value
                .parse::<u64>()
                .map_err(|err| format!("{key} '{value}' invalid: {err}"))
        }
    }

    fn parse_optional_layer(value: &str) -> Result<Option<usize>, String> {
        match value {
            "" | "none" | "global" | "-1" => Ok(None),
            other => other
                .parse::<usize>()
                .map(Some)
                .map_err(|err| format!("layer '{value}' invalid: {err}")),
        }
    }

    fn compare_layer_trace(
        actual: &[LayerFingerprint],
        reference: &[LayerTraceReference],
    ) -> Result<usize, String> {
        let mut compared = 0usize;
        for (ref_idx, expected) in reference.iter().enumerate() {
            let Some(got) = actual.iter().find(|entry| {
                entry.position == expected.position
                    && entry.layer == expected.layer
                    && entry.stage == expected.stage
            }) else {
                return Err(format!(
                    "missing ref_index={ref_idx} pos={} layer={} stage={} actual_entries={}",
                    expected.position,
                    layer_label(expected.layer),
                    expected.stage,
                    actual.len()
                ));
            };
            let mut reasons = Vec::new();
            if expected.len.is_some_and(|len| len != got.len) {
                reasons.push(format!(
                    "len got={} expected={}",
                    got.len,
                    expected.len.unwrap()
                ));
            }
            if expected.hash64.is_some_and(|hash| hash != got.hash64) {
                reasons.push(format!(
                    "hash got=0x{:016x} expected=0x{:016x}",
                    got.hash64,
                    expected.hash64.unwrap()
                ));
            }
            if let Some(sum) = expected.sum {
                let diff = (got.sum - sum).abs();
                let tol = 1.0e-4f64.max(sum.abs() * 1.0e-5);
                if diff > tol {
                    reasons.push(format!(
                        "sum got={:.9} expected={:.9} diff={:.9} tol={:.9}",
                        got.sum, sum, diff, tol
                    ));
                }
            }
            if let Some(abs_max) = expected.abs_max {
                let diff = (got.abs_max - abs_max).abs();
                let tol = 1.0e-5f32.max(abs_max.abs() * 1.0e-5);
                if diff > tol {
                    reasons.push(format!(
                        "absmax got={:.9} expected={:.9} diff={:.9} tol={:.9}",
                        got.abs_max, abs_max, diff, tol
                    ));
                }
            }
            if !reasons.is_empty() {
                return Err(format!(
                    "first_mismatch ref_index={ref_idx} pos={} layer={} stage={} {}",
                    expected.position,
                    layer_label(expected.layer),
                    expected.stage,
                    reasons.join("; ")
                ));
            }
            compared += 1;
        }
        Ok(compared)
    }

    fn layer_label(layer: Option<usize>) -> String {
        layer
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string())
    }

    fn layer_fingerprint(
        position: usize,
        layer: Option<usize>,
        stage: &str,
        values: &[f32],
    ) -> LayerFingerprint {
        let mut sum = 0.0f64;
        let mut abs_max = 0.0f32;
        let mut hash = 0xcbf29ce484222325u64;
        for value in values {
            sum += f64::from(*value);
            abs_max = abs_max.max(value.abs());
            for byte in value.to_bits().to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
        LayerFingerprint {
            position,
            layer,
            stage: stage.to_string(),
            len: values.len(),
            sum,
            abs_max,
            hash64: hash,
        }
    }

    fn log_real_neom_layer_dtype_inventory(model: &RealNeomModel) -> RealDtypeCounts {
        let config = model.layer_config;
        let schedule = real_neom_schedule(model);
        let mut counts = RealDtypeCounts::default();
        for (layer, entry) in schedule.layers.iter().enumerate() {
            match entry.kind {
                crate::model_logic::LayerKind::Qwen2AttentionFFN => {
                    let dtypes = real_neom_attention_layer_dtypes(model, layer, config);
                    counts.record_layer(dtypes);
                    eprintln!(
                        "REAL_MODEL_STAGE_A layer={layer} kind=attention dtypes attn_q={} attn_k={} attn_v={} attn_o={} ffn_gate={} ffn_up={} ffn_down={}",
                        dtypes.attn_q.name(),
                        dtypes.attn_k.name(),
                        dtypes.attn_v.name(),
                        dtypes.attn_o.name(),
                        dtypes.ffn_gate.name(),
                        dtypes.ffn_up.name(),
                        dtypes.ffn_down.name()
                    );
                }
                crate::model_logic::LayerKind::GatedDeltaNetLinear => {
                    let gdn_config = real_neom_gdn_config(model, layer, config);
                    let qkv = real_neom_tensor_dtype_any(
                        model,
                        &gdn_layer_tensor_candidates(layer, "in_proj_qkv.weight"),
                        config.hidden_size,
                        gdn_config.conv_dim(),
                    );
                    let z = real_neom_tensor_dtype_any(
                        model,
                        &gdn_layer_tensor_candidates(layer, "in_proj_z.weight"),
                        config.hidden_size,
                        gdn_config.value_dim(),
                    );
                    let beta = real_neom_tensor_dtype_any(
                        model,
                        &gdn_layer_tensor_candidates(layer, "in_proj_b.weight"),
                        config.hidden_size,
                        gdn_config.num_v_heads,
                    );
                    let alpha = real_neom_tensor_dtype_any(
                        model,
                        &gdn_layer_tensor_candidates(layer, "in_proj_a.weight"),
                        config.hidden_size,
                        gdn_config.num_v_heads,
                    );
                    let out = real_neom_tensor_dtype_any(
                        model,
                        &gdn_layer_tensor_candidates(layer, "out_proj.weight"),
                        gdn_config.value_dim(),
                        config.hidden_size,
                    );
                    for dtype in [qkv, z, beta, alpha, out] {
                        counts.record(dtype);
                    }
                    let ffn = real_neom_ffn_layer_dtypes(model, layer, config);
                    counts.record_ffn(ffn);
                    eprintln!(
                        "REAL_MODEL_STAGE_A layer={layer} kind=gdn dtypes qkv={} z={} beta={} alpha={} out={} ffn_gate={} ffn_up={} ffn_down={}",
                        qkv.name(),
                        z.name(),
                        beta.name(),
                        alpha.name(),
                        out.name(),
                        ffn.ffn_gate.name(),
                        ffn.ffn_up.name(),
                        ffn.ffn_down.name()
                    );
                }
            }
        }
        let output_dtype = real_neom_tensor_dtype(
            model,
            "output.weight",
            config.hidden_size,
            real_neom_vocab_size(model),
        );
        eprintln!(
            "REAL_MODEL_STAGE_A output.weight dtype={} (raw quantized output head used when feature-on)",
            output_dtype.name()
        );
        counts
    }

    fn real_neom_greedy_decode(
        model: &RealNeomModel,
        prompt: &[u32],
        max_new_tokens: usize,
        trace: &mut RealForwardTrace,
        stop_on_divergence: Option<&[u32]>,
    ) -> RealDecodeReport {
        assert!(!prompt.is_empty(), "real .neom prompt must not be empty");
        let started = std::time::Instant::now();
        let mut state = RealNeomRuntimeState::new(model, prompt.len() + max_new_tokens);
        let mut tokens = Vec::with_capacity(max_new_tokens);
        let mut steps = Vec::with_capacity(max_new_tokens);
        let mut position = 0usize;
        let mut logits = Vec::new();
        for &token in prompt {
            logits = real_neom_next_logits_with_trace(model, token, position, &mut state, trace);
            position += 1;
        }
        for step_idx in 0..max_new_tokens {
            assert_all_finite(&format!("real .neom logits step {step_idx}"), &logits);
            let token = argmax_token(&logits).unwrap();
            let top = top_two_logits(&logits);
            assert_eq!(token, top.token, "top-two helper must match argmax");
            steps.push(top);
            tokens.push(token);
            if stop_on_divergence.is_some_and(|expected| expected[step_idx] != token) {
                break;
            }
            if step_idx + 1 < max_new_tokens {
                logits =
                    real_neom_next_logits_with_trace(model, token, position, &mut state, trace);
                position += 1;
            }
        }
        RealDecodeReport {
            tokens,
            steps,
            elapsed: started.elapsed(),
        }
    }

    fn real_neom_trace_token_stream(
        model: &RealNeomModel,
        tokens: &[u32],
        trace: &mut RealForwardTrace,
    ) -> Vec<f32> {
        let mut state = RealNeomRuntimeState::new(model, tokens.len());
        let mut logits = Vec::new();
        for (position, &token) in tokens.iter().enumerate() {
            logits = real_neom_next_logits_with_trace(model, token, position, &mut state, trace);
        }
        logits
    }

    fn real_neom_next_logits_with_trace(
        model: &RealNeomModel,
        token: u32,
        position: usize,
        state: &mut RealNeomRuntimeState,
        trace: &mut RealForwardTrace,
    ) -> Vec<f32> {
        let profile_position = std::env::var("NEO_REAL_PROFILE_POSITION")
            .ok()
            .and_then(|value| value.parse::<usize>().ok());
        let profile = std::env::var("NEO_REAL_PROFILE").ok().as_deref() == Some("1")
            && profile_position.is_none_or(|target| target == position);
        let forward_started = std::time::Instant::now();
        let config = model.layer_config;
        let layer_count = real_neom_layer_count(model);
        let vocab_size = real_neom_vocab_size(model);
        let mut hidden = real_neom_token_embedding(model, token, config.hidden_size, vocab_size);
        assert_all_finite(
            &format!("real .neom token_embd position {position}"),
            &hidden,
        );
        trace.record_layer_trace(position, None, "model.input_embed", &hidden);
        for layer_idx in 0..layer_count {
            let entry = state.schedule.layers[layer_idx];
            let layer_started = std::time::Instant::now();
            hidden = match entry.kind {
                crate::model_logic::LayerKind::Qwen2AttentionFFN => real_neom_run_attention_layer(
                    model,
                    &hidden,
                    config,
                    position,
                    layer_idx,
                    state.kv_caches.layer_mut(layer_idx).unwrap(),
                    trace,
                ),
                crate::model_logic::LayerKind::GatedDeltaNetLinear => real_neom_run_gdn_layer(
                    model,
                    &hidden,
                    config,
                    RealLayerContext {
                        position,
                        layer_idx,
                        config_ref: entry.config_ref,
                    },
                    &mut state.gdn_states,
                    trace,
                ),
            };
            if profile {
                eprintln!(
                    "REAL_PROFILE position={position} layer={layer_idx} kind={:?} elapsed_ms={:.3}",
                    entry.kind,
                    layer_started.elapsed().as_secs_f64() * 1000.0
                );
            }
        }

        let output_started = std::time::Instant::now();
        let output_norm =
            real_neom_tensor_to_f32(model, "output_norm.weight", 1, config.hidden_size);
        let output_dtype =
            real_neom_tensor_dtype(model, "output.weight", config.hidden_size, vocab_size);
        let output = real_neom_tensor_to_ops_rhs_placeholder_if_quantized(
            model,
            "output.weight",
            config.hidden_size,
            vocab_size,
        );
        let output_quantized = real_neom_tensor_to_quantized_rhs(
            model,
            "output.weight",
            config.hidden_size,
            vocab_size,
        );
        let logits = forward_logic::qwen2_output_logits(
            &hidden,
            Qwen2OutputWeights {
                output_norm: &output_norm,
                output: &output,
                output_dtype,
                output_quantized,
                output_bias: None,
            },
            config,
            vocab_size,
        )
        .unwrap();
        let result_norm =
            reference_rms_norm(&hidden, &output_norm, bits_to_f32(config.rms_norm_eps_bits));
        trace.record_layer_trace(position, None, "result_norm", &result_norm);
        trace.record_layer_trace(position, None, "result_output", &logits);
        if profile {
            eprintln!(
                "REAL_PROFILE position={position} output elapsed_ms={:.3} total_ms={:.3}",
                output_started.elapsed().as_secs_f64() * 1000.0,
                forward_started.elapsed().as_secs_f64() * 1000.0
            );
        }
        logits
    }

    fn real_neom_run_attention_layer(
        model: &RealNeomModel,
        hidden: &[f32],
        config: Qwen2LayerConfig,
        position: usize,
        layer_idx: usize,
        cache: &mut Qwen2KvCacheLayer,
        trace: &mut RealForwardTrace,
    ) -> Vec<f32> {
        let layer = real_neom_attention_layer_buffers(model, layer_idx, config);
        trace.dtype_counts.record_layer(layer.dtypes);
        let out = forward_logic::forward_layer_with_gqa_attention_with_norms(
            hidden,
            layer.weights(),
            config,
            position as u64,
            true,
            Some(cache),
            None,
            layer.attention_norms(),
        )
        .unwrap();
        assert_all_finite(
            &format!("real .neom position {position} layer {layer_idx} attn_norm"),
            &out.attention.attn_norm,
        );
        trace.record_layer_trace(
            position,
            Some(layer_idx),
            "attn_norm",
            &out.attention.attn_norm,
        );
        assert_all_finite(
            &format!("real .neom position {position} layer {layer_idx} q"),
            &out.attention.q,
        );
        trace.record_layer_trace(position, Some(layer_idx), "q", &out.attention.q);
        if let Some(gate) = out.attention.gate.as_deref() {
            assert_all_finite(
                &format!("real .neom position {position} layer {layer_idx} q_gate"),
                gate,
            );
            trace.record_layer_trace(position, Some(layer_idx), "q_gate", gate);
        }
        assert_all_finite(
            &format!("real .neom position {position} layer {layer_idx} k"),
            &out.attention.k,
        );
        trace.record_layer_trace(position, Some(layer_idx), "k", &out.attention.k);
        assert_all_finite(
            &format!("real .neom position {position} layer {layer_idx} v"),
            &out.attention.v,
        );
        trace.record_layer_trace(position, Some(layer_idx), "v", &out.attention.v);
        assert_all_finite(
            &format!("real .neom position {position} layer {layer_idx} q_rope"),
            &out.attention.q_rope,
        );
        trace.record_layer_trace(position, Some(layer_idx), "q_rope", &out.attention.q_rope);
        assert_all_finite(
            &format!("real .neom position {position} layer {layer_idx} k_rope"),
            &out.attention.k_rope,
        );
        trace.record_layer_trace(position, Some(layer_idx), "k_rope", &out.attention.k_rope);
        assert_all_finite(
            &format!("real .neom position {position} layer {layer_idx} ffn_input"),
            &out.ffn_input,
        );
        trace.record_layer_trace(position, Some(layer_idx), "ffn_input", &out.ffn_input);
        assert_all_finite(
            &format!("real .neom position {position} layer {layer_idx} ffn_norm"),
            &out.ffn.ffn_norm,
        );
        trace.record_layer_trace(position, Some(layer_idx), "ffn_norm", &out.ffn.ffn_norm);
        assert_all_finite(
            &format!("real .neom position {position} layer {layer_idx} ffn_out"),
            &out.ffn.ffn_out,
        );
        trace.record_layer_trace(position, Some(layer_idx), "ffn_out", &out.ffn.ffn_out);
        assert_all_finite(
            &format!("real .neom position {position} layer {layer_idx} ffn_residual"),
            &out.ffn.residual,
        );
        trace.record_layer_trace(position, Some(layer_idx), "l_out", &out.ffn.residual);
        out.ffn.residual
    }

    fn real_neom_run_gdn_layer(
        model: &RealNeomModel,
        hidden: &[f32],
        config: Qwen2LayerConfig,
        context: RealLayerContext,
        states: &mut RealNeomGdnStateSet,
        trace: &mut RealForwardTrace,
    ) -> Vec<f32> {
        let RealLayerContext {
            position,
            layer_idx,
            config_ref,
        } = context;
        let attn_norm_weight = real_neom_tensor_to_f32(
            model,
            &qwen_layer_tensor_name(layer_idx, "attn_norm.weight"),
            1,
            config.hidden_size,
        );
        let attn_norm =
            forward_logic::rms_norm_vec(hidden, &attn_norm_weight, config.rms_norm_eps_bits)
                .unwrap();
        assert_all_finite(
            &format!("real .neom position {position} layer {layer_idx} attn_norm"),
            &attn_norm,
        );
        trace.record_layer_trace(position, Some(layer_idx), "attn_norm", &attn_norm);

        let gdn_state = states.layer_mut(layer_idx, config_ref);
        let gdn = real_neom_gdn_layer_buffers(model, layer_idx, gdn_state.config);
        for dtype in [
            gdn.in_proj_qkv.dtype,
            gdn.in_proj_z.dtype,
            gdn.in_proj_b.dtype,
            gdn.in_proj_a.dtype,
            gdn.out_proj.dtype,
        ] {
            trace.dtype_counts.record(dtype);
        }
        let qkv =
            gdn.in_proj_qkv
                .project(&attn_norm, config.hidden_size, gdn_state.config.conv_dim());
        trace.record_layer_trace(position, Some(layer_idx), "gdn_qkv", &qkv);
        let z = gdn
            .in_proj_z
            .project(&attn_norm, config.hidden_size, gdn_state.config.value_dim());
        trace.record_layer_trace(position, Some(layer_idx), "gdn_z", &z);
        let beta_raw =
            gdn.in_proj_b
                .project(&attn_norm, config.hidden_size, gdn_state.config.num_v_heads);
        trace.record_layer_trace(position, Some(layer_idx), "gdn_beta", &beta_raw);
        let a_raw =
            gdn.in_proj_a
                .project(&attn_norm, config.hidden_size, gdn_state.config.num_v_heads);
        trace.record_layer_trace(position, Some(layer_idx), "gdn_alpha", &a_raw);
        let gdn_core = ops::gdn::gdn_layer_core_projected(
            ops::gdn::GdnProjectedInputs {
                qkv: &qkv,
                z: &z,
                beta_raw: &beta_raw,
                a_raw: &a_raw,
            },
            gdn.weights(),
            gdn_state.config,
            &mut gdn_state.recurrent_state,
            &mut gdn_state.conv_state,
        )
        .unwrap();
        trace.record_layer_trace(position, Some(layer_idx), "gdn_core", &gdn_core);
        let attention_output =
            gdn.out_proj
                .project(&gdn_core, gdn_state.config.value_dim(), config.hidden_size);
        assert_all_finite(
            &format!("real .neom position {position} layer {layer_idx} gdn_projected"),
            &attention_output,
        );
        trace.record_layer_trace(
            position,
            Some(layer_idx),
            "gdn_projected",
            &attention_output,
        );

        let ffn_input = forward_logic::add_vec(hidden, &attention_output).unwrap();
        assert_all_finite(
            &format!("real .neom position {position} layer {layer_idx} ffn_input"),
            &ffn_input,
        );
        trace.record_layer_trace(position, Some(layer_idx), "ffn_input", &ffn_input);

        let ffn = real_neom_ffn_buffers(model, layer_idx, config);
        trace.dtype_counts.record_ffn(ffn.dtypes());
        let out = forward_logic::swiglu_ffn(&ffn_input, ffn.weights(), config).unwrap();
        assert_all_finite(
            &format!("real .neom position {position} layer {layer_idx} ffn_norm"),
            &out.ffn_norm,
        );
        trace.record_layer_trace(position, Some(layer_idx), "ffn_norm", &out.ffn_norm);
        assert_all_finite(
            &format!("real .neom position {position} layer {layer_idx} ffn_out"),
            &out.ffn_out,
        );
        trace.record_layer_trace(position, Some(layer_idx), "ffn_out", &out.ffn_out);
        assert_all_finite(
            &format!("real .neom position {position} layer {layer_idx} ffn_residual"),
            &out.residual,
        );
        trace.record_layer_trace(position, Some(layer_idx), "l_out", &out.residual);
        out.residual
    }

    struct RealNeomAttentionLayerBuffers<'a> {
        attn_norm: Vec<f32>,
        attn_q: Vec<f32>,
        attn_k: Vec<f32>,
        attn_v: Vec<f32>,
        attn_q_bias: Option<Vec<f32>>,
        attn_k_bias: Option<Vec<f32>>,
        attn_v_bias: Option<Vec<f32>>,
        attn_q_norm: Option<Vec<f32>>,
        attn_k_norm: Option<Vec<f32>>,
        attn_o: Vec<f32>,
        ffn_norm: Vec<f32>,
        ffn_gate: Vec<f32>,
        ffn_up: Vec<f32>,
        ffn_down: Vec<f32>,
        dtypes: Qwen2LayerDtypes,
        attn_q_quantized: Option<&'a [u8]>,
        attn_k_quantized: Option<&'a [u8]>,
        attn_v_quantized: Option<&'a [u8]>,
        attn_o_quantized: Option<&'a [u8]>,
        ffn_gate_quantized: Option<&'a [u8]>,
        ffn_up_quantized: Option<&'a [u8]>,
        ffn_down_quantized: Option<&'a [u8]>,
    }

    impl<'a> RealNeomAttentionLayerBuffers<'a> {
        fn weights(&self) -> Qwen2LayerWeights<'_> {
            Qwen2LayerWeights {
                attn_norm: &self.attn_norm,
                attn_q: &self.attn_q,
                attn_k: &self.attn_k,
                attn_v: &self.attn_v,
                attn_q_bias: self.attn_q_bias.as_deref(),
                attn_k_bias: self.attn_k_bias.as_deref(),
                attn_v_bias: self.attn_v_bias.as_deref(),
                attn_o: &self.attn_o,
                ffn_norm: &self.ffn_norm,
                ffn_gate: &self.ffn_gate,
                ffn_up: &self.ffn_up,
                ffn_down: &self.ffn_down,
                dtypes: self.dtypes,
                quantized: Qwen2QuantizedLayerWeights {
                    attn_q: self.attn_q_quantized,
                    attn_k: self.attn_k_quantized,
                    attn_v: self.attn_v_quantized,
                    attn_o: self.attn_o_quantized,
                    ffn_gate: self.ffn_gate_quantized,
                    ffn_up: self.ffn_up_quantized,
                    ffn_down: self.ffn_down_quantized,
                },
            }
        }

        fn attention_norms(&self) -> QwenAttentionNormWeights<'_> {
            QwenAttentionNormWeights {
                q_norm: self.attn_q_norm.as_deref(),
                k_norm: self.attn_k_norm.as_deref(),
            }
        }
    }

    fn real_neom_attention_layer_buffers(
        model: &RealNeomModel,
        layer: usize,
        config: Qwen2LayerConfig,
    ) -> RealNeomAttentionLayerBuffers<'_> {
        let attn_q_name = qwen_layer_tensor_name(layer, "attn_q.weight");
        let attn_k_name = qwen_layer_tensor_name(layer, "attn_k.weight");
        let attn_v_name = qwen_layer_tensor_name(layer, "attn_v.weight");
        let attn_o_name = qwen_layer_tensor_name(layer, "attn_output.weight");
        let ffn_gate_name = qwen_layer_tensor_name(layer, "ffn_gate.weight");
        let ffn_up_name = qwen_layer_tensor_name(layer, "ffn_up.weight");
        let ffn_down_name = qwen_layer_tensor_name(layer, "ffn_down.weight");
        RealNeomAttentionLayerBuffers {
            attn_norm: real_neom_tensor_to_f32(
                model,
                &qwen_layer_tensor_name(layer, "attn_norm.weight"),
                1,
                config.hidden_size,
            ),
            attn_q: real_neom_tensor_to_ops_rhs_placeholder_if_quantized(
                model,
                &attn_q_name,
                config.hidden_size,
                config.attn_q_projection_size,
            ),
            attn_k: real_neom_tensor_to_ops_rhs_placeholder_if_quantized(
                model,
                &attn_k_name,
                config.hidden_size,
                config.kv_len(),
            ),
            attn_v: real_neom_tensor_to_ops_rhs_placeholder_if_quantized(
                model,
                &attn_v_name,
                config.hidden_size,
                config.kv_len(),
            ),
            attn_q_bias: real_neom_tensor_to_f32_if_present(
                model,
                &qwen_layer_tensor_name(layer, "attn_q.bias"),
                1,
                config.attn_q_projection_size,
            ),
            attn_k_bias: real_neom_tensor_to_f32_if_present(
                model,
                &qwen_layer_tensor_name(layer, "attn_k.bias"),
                1,
                config.kv_len(),
            ),
            attn_v_bias: real_neom_tensor_to_f32_if_present(
                model,
                &qwen_layer_tensor_name(layer, "attn_v.bias"),
                1,
                config.kv_len(),
            ),
            attn_q_norm: real_neom_tensor_to_f32_if_present(
                model,
                &qwen_layer_tensor_name(layer, "attn_q_norm.weight"),
                1,
                config.head_dim,
            ),
            attn_k_norm: real_neom_tensor_to_f32_if_present(
                model,
                &qwen_layer_tensor_name(layer, "attn_k_norm.weight"),
                1,
                config.head_dim,
            ),
            attn_o: real_neom_tensor_to_ops_rhs_placeholder_if_quantized(
                model,
                &attn_o_name,
                config.q_len(),
                config.hidden_size,
            ),
            ffn_norm: real_neom_tensor_to_flat_f32_any(
                model,
                &ffn_norm_tensor_candidates(layer),
                config.hidden_size,
            ),
            ffn_gate: real_neom_tensor_to_ops_rhs_placeholder_if_quantized(
                model,
                &ffn_gate_name,
                config.hidden_size,
                config.intermediate_size,
            ),
            ffn_up: real_neom_tensor_to_ops_rhs_placeholder_if_quantized(
                model,
                &ffn_up_name,
                config.hidden_size,
                config.intermediate_size,
            ),
            ffn_down: real_neom_tensor_to_ops_rhs_placeholder_if_quantized(
                model,
                &ffn_down_name,
                config.intermediate_size,
                config.hidden_size,
            ),
            dtypes: real_neom_attention_layer_dtypes(model, layer, config),
            attn_q_quantized: real_neom_tensor_to_quantized_rhs(
                model,
                &attn_q_name,
                config.hidden_size,
                config.attn_q_projection_size,
            ),
            attn_k_quantized: real_neom_tensor_to_quantized_rhs(
                model,
                &attn_k_name,
                config.hidden_size,
                config.kv_len(),
            ),
            attn_v_quantized: real_neom_tensor_to_quantized_rhs(
                model,
                &attn_v_name,
                config.hidden_size,
                config.kv_len(),
            ),
            attn_o_quantized: real_neom_tensor_to_quantized_rhs(
                model,
                &attn_o_name,
                config.q_len(),
                config.hidden_size,
            ),
            ffn_gate_quantized: real_neom_tensor_to_quantized_rhs(
                model,
                &ffn_gate_name,
                config.hidden_size,
                config.intermediate_size,
            ),
            ffn_up_quantized: real_neom_tensor_to_quantized_rhs(
                model,
                &ffn_up_name,
                config.hidden_size,
                config.intermediate_size,
            ),
            ffn_down_quantized: real_neom_tensor_to_quantized_rhs(
                model,
                &ffn_down_name,
                config.intermediate_size,
                config.hidden_size,
            ),
        }
    }

    struct RealNeomGdnMatrix<'a> {
        f32: Vec<f32>,
        dtype: TensorDtype,
        quantized: Option<&'a [u8]>,
    }

    impl RealNeomGdnMatrix<'_> {
        fn project(&self, input: &[f32], rows: usize, cols: usize) -> Vec<f32> {
            forward_logic::matmul_vec_mixed_weight(
                input,
                1,
                rows,
                &self.f32,
                self.dtype,
                self.quantized,
                cols,
            )
            .unwrap()
        }
    }

    struct RealNeomGdnLayerBuffers<'a> {
        in_proj_qkv: RealNeomGdnMatrix<'a>,
        in_proj_z: RealNeomGdnMatrix<'a>,
        in_proj_b: RealNeomGdnMatrix<'a>,
        in_proj_a: RealNeomGdnMatrix<'a>,
        conv1d_weight: Vec<f32>,
        a_log: Vec<f32>,
        dt_bias: Vec<f32>,
        norm_weight: Vec<f32>,
        out_proj: RealNeomGdnMatrix<'a>,
    }

    impl RealNeomGdnLayerBuffers<'_> {
        fn weights(&self) -> ops::gdn::GdnLayerWeights<'_> {
            ops::gdn::GdnLayerWeights {
                in_proj_qkv: &self.in_proj_qkv.f32,
                in_proj_z: &self.in_proj_z.f32,
                in_proj_b: &self.in_proj_b.f32,
                in_proj_a: &self.in_proj_a.f32,
                conv1d_weight: &self.conv1d_weight,
                a_log: &self.a_log,
                dt_bias: &self.dt_bias,
                norm_weight: &self.norm_weight,
                out_proj: &self.out_proj.f32,
            }
        }
    }

    fn real_neom_gdn_matrix_any<'a>(
        model: &'a RealNeomModel,
        names: &[String],
        rows: usize,
        cols: usize,
    ) -> RealNeomGdnMatrix<'a> {
        let name = names
            .iter()
            .find(|name| model.tensor_named_opt(name).is_some())
            .unwrap_or_else(|| panic!("real .neom tensor exists for one of {names:?}"));
        RealNeomGdnMatrix {
            f32: real_neom_tensor_to_ops_rhs_placeholder_if_quantized(model, name, rows, cols),
            dtype: real_neom_tensor_dtype(model, name, rows, cols),
            quantized: real_neom_tensor_to_quantized_rhs(model, name, rows, cols),
        }
    }

    fn real_neom_gdn_layer_buffers(
        model: &RealNeomModel,
        layer: usize,
        config: ops::gdn::GdnLayerConfig,
    ) -> RealNeomGdnLayerBuffers<'_> {
        RealNeomGdnLayerBuffers {
            in_proj_qkv: real_neom_gdn_matrix_any(
                model,
                &gdn_layer_tensor_candidates(layer, "in_proj_qkv.weight"),
                config.hidden_size,
                config.conv_dim(),
            ),
            in_proj_z: real_neom_gdn_matrix_any(
                model,
                &gdn_layer_tensor_candidates(layer, "in_proj_z.weight"),
                config.hidden_size,
                config.value_dim(),
            ),
            in_proj_b: real_neom_gdn_matrix_any(
                model,
                &gdn_layer_tensor_candidates(layer, "in_proj_b.weight"),
                config.hidden_size,
                config.num_v_heads,
            ),
            in_proj_a: real_neom_gdn_matrix_any(
                model,
                &gdn_layer_tensor_candidates(layer, "in_proj_a.weight"),
                config.hidden_size,
                config.num_v_heads,
            ),
            conv1d_weight: real_neom_tensor_to_f32_any(
                model,
                &gdn_layer_tensor_candidates(layer, "conv1d.weight"),
                config.conv_kernel_size,
                config.conv_dim(),
            ),
            a_log: real_neom_tensor_to_flat_f32_any(
                model,
                &gdn_layer_tensor_candidates(layer, "A_log"),
                config.num_v_heads,
            ),
            dt_bias: real_neom_tensor_to_flat_f32_any(
                model,
                &gdn_layer_tensor_candidates(layer, "dt_bias"),
                config.num_v_heads,
            ),
            norm_weight: real_neom_tensor_to_flat_f32_any(
                model,
                &gdn_layer_tensor_candidates(layer, "norm.weight"),
                config.head_v_dim,
            ),
            out_proj: real_neom_gdn_matrix_any(
                model,
                &gdn_layer_tensor_candidates(layer, "out_proj.weight"),
                config.value_dim(),
                config.hidden_size,
            ),
        }
    }

    struct RealNeomFfnBuffers<'a> {
        ffn_norm: Vec<f32>,
        ffn_gate: Vec<f32>,
        ffn_up: Vec<f32>,
        ffn_down: Vec<f32>,
        ffn_gate_dtype: TensorDtype,
        ffn_up_dtype: TensorDtype,
        ffn_down_dtype: TensorDtype,
        ffn_gate_quantized: Option<&'a [u8]>,
        ffn_up_quantized: Option<&'a [u8]>,
        ffn_down_quantized: Option<&'a [u8]>,
    }

    impl<'a> RealNeomFfnBuffers<'a> {
        fn weights(&self) -> Qwen2LayerWeights<'_> {
            Qwen2LayerWeights {
                attn_norm: &[],
                attn_q: &[],
                attn_k: &[],
                attn_v: &[],
                attn_q_bias: None,
                attn_k_bias: None,
                attn_v_bias: None,
                attn_o: &[],
                ffn_norm: &self.ffn_norm,
                ffn_gate: &self.ffn_gate,
                ffn_up: &self.ffn_up,
                ffn_down: &self.ffn_down,
                dtypes: self.dtypes(),
                quantized: Qwen2QuantizedLayerWeights {
                    attn_q: None,
                    attn_k: None,
                    attn_v: None,
                    attn_o: None,
                    ffn_gate: self.ffn_gate_quantized,
                    ffn_up: self.ffn_up_quantized,
                    ffn_down: self.ffn_down_quantized,
                },
            }
        }

        fn dtypes(&self) -> Qwen2LayerDtypes {
            Qwen2LayerDtypes {
                attn_q: TensorDtype::F32,
                attn_k: TensorDtype::F32,
                attn_v: TensorDtype::F32,
                attn_o: TensorDtype::F32,
                ffn_gate: self.ffn_gate_dtype,
                ffn_up: self.ffn_up_dtype,
                ffn_down: self.ffn_down_dtype,
            }
        }
    }

    fn real_neom_ffn_buffers(
        model: &RealNeomModel,
        layer: usize,
        config: Qwen2LayerConfig,
    ) -> RealNeomFfnBuffers<'_> {
        let ffn_gate_name = qwen_layer_tensor_name(layer, "ffn_gate.weight");
        let ffn_up_name = qwen_layer_tensor_name(layer, "ffn_up.weight");
        let ffn_down_name = qwen_layer_tensor_name(layer, "ffn_down.weight");
        RealNeomFfnBuffers {
            ffn_norm: real_neom_tensor_to_flat_f32_any(
                model,
                &ffn_norm_tensor_candidates(layer),
                config.hidden_size,
            ),
            ffn_gate: real_neom_tensor_to_ops_rhs_placeholder_if_quantized(
                model,
                &ffn_gate_name,
                config.hidden_size,
                config.intermediate_size,
            ),
            ffn_up: real_neom_tensor_to_ops_rhs_placeholder_if_quantized(
                model,
                &ffn_up_name,
                config.hidden_size,
                config.intermediate_size,
            ),
            ffn_down: real_neom_tensor_to_ops_rhs_placeholder_if_quantized(
                model,
                &ffn_down_name,
                config.intermediate_size,
                config.hidden_size,
            ),
            ffn_gate_dtype: real_neom_tensor_dtype(
                model,
                &ffn_gate_name,
                config.hidden_size,
                config.intermediate_size,
            ),
            ffn_up_dtype: real_neom_tensor_dtype(
                model,
                &ffn_up_name,
                config.hidden_size,
                config.intermediate_size,
            ),
            ffn_down_dtype: real_neom_tensor_dtype(
                model,
                &ffn_down_name,
                config.intermediate_size,
                config.hidden_size,
            ),
            ffn_gate_quantized: real_neom_tensor_to_quantized_rhs(
                model,
                &ffn_gate_name,
                config.hidden_size,
                config.intermediate_size,
            ),
            ffn_up_quantized: real_neom_tensor_to_quantized_rhs(
                model,
                &ffn_up_name,
                config.hidden_size,
                config.intermediate_size,
            ),
            ffn_down_quantized: real_neom_tensor_to_quantized_rhs(
                model,
                &ffn_down_name,
                config.intermediate_size,
                config.hidden_size,
            ),
        }
    }

    fn real_neom_attention_layer_dtypes(
        model: &RealNeomModel,
        layer: usize,
        config: Qwen2LayerConfig,
    ) -> Qwen2LayerDtypes {
        Qwen2LayerDtypes {
            attn_q: real_neom_tensor_dtype(
                model,
                &qwen_layer_tensor_name(layer, "attn_q.weight"),
                config.hidden_size,
                config.attn_q_projection_size,
            ),
            attn_k: real_neom_tensor_dtype(
                model,
                &qwen_layer_tensor_name(layer, "attn_k.weight"),
                config.hidden_size,
                config.kv_len(),
            ),
            attn_v: real_neom_tensor_dtype(
                model,
                &qwen_layer_tensor_name(layer, "attn_v.weight"),
                config.hidden_size,
                config.kv_len(),
            ),
            attn_o: real_neom_tensor_dtype(
                model,
                &qwen_layer_tensor_name(layer, "attn_output.weight"),
                config.q_len(),
                config.hidden_size,
            ),
            ffn_gate: real_neom_tensor_dtype(
                model,
                &qwen_layer_tensor_name(layer, "ffn_gate.weight"),
                config.hidden_size,
                config.intermediate_size,
            ),
            ffn_up: real_neom_tensor_dtype(
                model,
                &qwen_layer_tensor_name(layer, "ffn_up.weight"),
                config.hidden_size,
                config.intermediate_size,
            ),
            ffn_down: real_neom_tensor_dtype(
                model,
                &qwen_layer_tensor_name(layer, "ffn_down.weight"),
                config.intermediate_size,
                config.hidden_size,
            ),
        }
    }

    fn real_neom_ffn_layer_dtypes(
        model: &RealNeomModel,
        layer: usize,
        config: Qwen2LayerConfig,
    ) -> Qwen2LayerDtypes {
        Qwen2LayerDtypes {
            attn_q: TensorDtype::F32,
            attn_k: TensorDtype::F32,
            attn_v: TensorDtype::F32,
            attn_o: TensorDtype::F32,
            ffn_gate: real_neom_tensor_dtype(
                model,
                &qwen_layer_tensor_name(layer, "ffn_gate.weight"),
                config.hidden_size,
                config.intermediate_size,
            ),
            ffn_up: real_neom_tensor_dtype(
                model,
                &qwen_layer_tensor_name(layer, "ffn_up.weight"),
                config.hidden_size,
                config.intermediate_size,
            ),
            ffn_down: real_neom_tensor_dtype(
                model,
                &qwen_layer_tensor_name(layer, "ffn_down.weight"),
                config.intermediate_size,
                config.hidden_size,
            ),
        }
    }

    #[cfg(feature = "phase9-real-quant")]
    fn first_real_q5_0_matmul_candidate(model: &RealNeomModel) -> (String, usize, usize, &[u8]) {
        let config = model.layer_config;
        for layer in 0..real_neom_layer_count(model) {
            let candidates = [
                (
                    qwen_layer_tensor_name(layer, "attn_q.weight"),
                    config.hidden_size,
                    config.q_len(),
                ),
                (
                    qwen_layer_tensor_name(layer, "attn_k.weight"),
                    config.hidden_size,
                    config.kv_len(),
                ),
                (
                    qwen_layer_tensor_name(layer, "attn_v.weight"),
                    config.hidden_size,
                    config.kv_len(),
                ),
                (
                    qwen_layer_tensor_name(layer, "attn_output.weight"),
                    config.hidden_size,
                    config.hidden_size,
                ),
                (
                    qwen_layer_tensor_name(layer, "ffn_gate.weight"),
                    config.hidden_size,
                    config.intermediate_size,
                ),
                (
                    qwen_layer_tensor_name(layer, "ffn_up.weight"),
                    config.hidden_size,
                    config.intermediate_size,
                ),
                (
                    qwen_layer_tensor_name(layer, "ffn_down.weight"),
                    config.intermediate_size,
                    config.hidden_size,
                ),
            ];
            for (name, rows, cols) in candidates {
                if real_neom_tensor_dtype(model, &name, rows, cols) == TensorDtype::Q5_0 {
                    let data = real_neom_tensor_to_quantized_rhs(model, &name, rows, cols)
                        .expect("q5_0 candidate is raw quantized");
                    return (name, rows, cols, data);
                }
            }
        }

        let rows = config.hidden_size;
        let cols = real_neom_vocab_size(model);
        if real_neom_tensor_dtype(model, "output.weight", rows, cols) == TensorDtype::Q5_0 {
            let data = real_neom_tensor_to_quantized_rhs(model, "output.weight", rows, cols)
                .expect("q5_0 output is raw quantized");
            return ("output.weight".to_string(), rows, cols, data);
        }

        panic!("real .neom did not contain a q5_0 matmul tensor");
    }

    #[cfg(feature = "phase9-real-quant")]
    fn perf_probe_lhs(rows: usize) -> Vec<f32> {
        let mut lhs = Vec::with_capacity(rows);
        for idx in 0..rows {
            let centered = ((idx * 37 + 11) % 251) as f32 - 125.0;
            lhs.push(centered / 16.0);
        }
        lhs
    }

    fn real_neom_tensor_to_ops_rhs(
        model: &RealNeomModel,
        name: &str,
        rows: usize,
        cols: usize,
    ) -> Vec<f32> {
        let raw = real_neom_tensor_to_f32(model, name, rows, cols);
        assert_eq!(raw.len(), rows * cols, "{name} element count");
        let mut out = vec![0.0f32; raw.len()];
        for col in 0..cols {
            for row in 0..rows {
                out[row * cols + col] = raw[col * rows + row];
            }
        }
        out
    }

    fn real_neom_tensor_to_ops_rhs_placeholder_if_quantized(
        model: &RealNeomModel,
        name: &str,
        rows: usize,
        cols: usize,
    ) -> Vec<f32> {
        if real_neom_tensor_dtype(model, name, rows, cols).is_quantized() {
            Vec::new()
        } else {
            real_neom_tensor_to_ops_rhs(model, name, rows, cols)
        }
    }

    fn real_neom_tensor_to_f32_any(
        model: &RealNeomModel,
        names: &[String],
        rows: usize,
        cols: usize,
    ) -> Vec<f32> {
        for name in names {
            if model.tensor_named_opt(name).is_some() {
                return real_neom_tensor_to_f32(model, name, rows, cols);
            }
        }
        panic!("real .neom tensor exists for one of {names:?}");
    }

    fn real_neom_tensor_to_flat_f32_any(
        model: &RealNeomModel,
        names: &[String],
        expected_len: usize,
    ) -> Vec<f32> {
        let values = real_neom_tensor_to_f32_any(model, names, 1, expected_len);
        assert_eq!(values.len(), expected_len, "flat tensor length");
        values
    }

    fn real_neom_tensor_to_f32_if_present(
        model: &RealNeomModel,
        name: &str,
        rows: usize,
        cols: usize,
    ) -> Option<Vec<f32>> {
        model
            .tensor_named_opt(name)
            .map(|_| real_neom_tensor_to_f32(model, name, rows, cols))
    }

    fn real_neom_tensor_to_f32(
        model: &RealNeomModel,
        name: &str,
        rows: usize,
        cols: usize,
    ) -> Vec<f32> {
        let tensor = model.tensor_named(name);
        let data = model.tensor_data(tensor);
        match tensor.dtype {
            NeomDtype::F32 => neom_f32_from_bytes(data),
            NeomDtype::F16 => data
                .chunks_exact(2)
                .map(|chunk| reference_f16_bits_to_f32(u16::from_le_bytes([chunk[0], chunk[1]])))
                .collect(),
            NeomDtype::BF16 => data
                .chunks_exact(2)
                .map(|chunk| {
                    f32::from_bits((u16::from_le_bytes([chunk[0], chunk[1]]) as u32) << 16)
                })
                .collect(),
            NeomDtype::U8 => {
                let dtype = infer_real_neom_quant_dtype(name, rows, cols, data.len()).unwrap();
                let gguf = gguf_tensor(
                    name,
                    real_tensor_dtype_to_ggml(dtype),
                    &[rows as u64, cols as u64],
                );
                tensor_data_to_f32(&gguf, data)
            }
            other => panic!("{name} has unsupported real .neom dtype {other:?}"),
        }
    }

    fn real_neom_tensor_to_quantized_rhs<'a>(
        model: &'a RealNeomModel,
        name: &str,
        rows: usize,
        cols: usize,
    ) -> Option<&'a [u8]> {
        let tensor = model.tensor_named(name);
        if !matches!(tensor.dtype, NeomDtype::U8 | NeomDtype::Q5K) {
            return None;
        }
        let dtype = real_neom_tensor_dtype(model, name, rows, cols);
        assert!(dtype.is_quantized());
        Some(model.tensor_data(tensor))
    }

    fn real_neom_tensor_dtype(
        model: &RealNeomModel,
        name: &str,
        rows: usize,
        cols: usize,
    ) -> TensorDtype {
        let tensor = model.tensor_named(name);
        match tensor.dtype {
            NeomDtype::F32 | NeomDtype::F16 | NeomDtype::BF16 => TensorDtype::F32,
            NeomDtype::Q5K => {
                assert_eq!(
                    quantized_cols_for_type(rows, tensor.weight_len, GgmlType::Q5K),
                    cols,
                    "{name} q5_K geometry"
                );
                TensorDtype::Q5K
            }
            NeomDtype::U8 => {
                infer_real_neom_quant_dtype(name, rows, cols, tensor.weight_len).unwrap()
            }
            other => panic!("{name} has unsupported real .neom dtype {other:?}"),
        }
    }

    fn real_neom_tensor_dtype_any(
        model: &RealNeomModel,
        names: &[String],
        rows: usize,
        cols: usize,
    ) -> TensorDtype {
        let name = names
            .iter()
            .find(|name| model.tensor_named_opt(name).is_some())
            .unwrap_or_else(|| panic!("real .neom tensor exists for one of {names:?}"));
        real_neom_tensor_dtype(model, name, rows, cols)
    }

    fn real_neom_token_embedding(
        model: &RealNeomModel,
        token: u32,
        hidden_size: usize,
        vocab_size: usize,
    ) -> Vec<f32> {
        assert!((token as usize) < vocab_size);
        let tensor = model.tensor_named("token_embd.weight");
        let data = model.tensor_data(tensor);
        match tensor.dtype {
            NeomDtype::F32 | NeomDtype::F16 | NeomDtype::BF16 => {
                let element_size = tensor.dtype.size_bytes() as usize;
                let row_bytes = hidden_size * element_size;
                let start = token as usize * row_bytes;
                scalar_neom_data_to_f32(tensor.dtype, &data[start..start + row_bytes])
            }
            NeomDtype::U8 => {
                let dtype = infer_real_neom_quant_dtype(
                    "token_embd.weight",
                    hidden_size,
                    vocab_size,
                    data.len(),
                )
                .unwrap();
                let ty = real_tensor_dtype_to_ggml(dtype);
                let row_bytes = hidden_size / ty.block_size() as usize * ty.type_size() as usize;
                let start = token as usize * row_bytes;
                let row = &data[start..start + row_bytes];
                let row_tensor = gguf_tensor("token_embd.row", ty, &[hidden_size as u64]);
                tensor_data_to_f32(&row_tensor, row)
            }
            other => panic!("token_embd.weight has unsupported real .neom dtype {other:?}"),
        }
    }

    fn scalar_neom_data_to_f32(dtype: NeomDtype, data: &[u8]) -> Vec<f32> {
        match dtype {
            NeomDtype::F32 => neom_f32_from_bytes(data),
            NeomDtype::F16 => data
                .chunks_exact(2)
                .map(|chunk| reference_f16_bits_to_f32(u16::from_le_bytes([chunk[0], chunk[1]])))
                .collect(),
            NeomDtype::BF16 => data
                .chunks_exact(2)
                .map(|chunk| {
                    f32::from_bits((u16::from_le_bytes([chunk[0], chunk[1]]) as u32) << 16)
                })
                .collect(),
            other => panic!("unsupported scalar real .neom dtype {other:?}"),
        }
    }

    fn neom_f32_from_bytes(data: &[u8]) -> Vec<f32> {
        assert_eq!(data.len() % 4, 0);
        data.chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect()
    }

    fn infer_real_neom_quant_dtype(
        name: &str,
        rows: usize,
        cols: usize,
        byte_len: usize,
    ) -> Result<TensorDtype, String> {
        let candidates = [
            (TensorDtype::Q4K, GgmlType::Q4K),
            (TensorDtype::Q5_0, GgmlType::Q5_0),
            (TensorDtype::Q6K, GgmlType::Q6K),
            (TensorDtype::Q8_0, GgmlType::Q8_0),
        ];
        let mut matched = None;
        for (dtype, ty) in candidates {
            let block = ty.block_size() as usize;
            let type_size = ty.type_size() as usize;
            if rows % block != 0 {
                continue;
            }
            let expected = cols
                .checked_mul(rows / block)
                .and_then(|value| value.checked_mul(type_size))
                .ok_or_else(|| format!("{name} quant byte length overflow"))?;
            if expected == byte_len {
                if matched.is_some() {
                    return Err(format!(
                        "{name} quant dtype is ambiguous for {byte_len} bytes"
                    ));
                }
                matched = Some(dtype);
            }
        }
        matched.ok_or_else(|| {
            format!(
                "{name} byte_len={byte_len} does not match q4_K/q5_0/q6_K/q8_0 for {rows}x{cols}"
            )
        })
    }

    fn real_neom_gdn_config(
        model: &RealNeomModel,
        layer: usize,
        base: Qwen2LayerConfig,
    ) -> ops::gdn::GdnLayerConfig {
        let conv_dim = real_neom_rhs_cols(
            model,
            &qwen_layer_tensor_name(layer, "attn_qkv.weight"),
            base.hidden_size,
        );
        let value_dim = real_neom_rhs_cols(
            model,
            &qwen_layer_tensor_name(layer, "attn_gate.weight"),
            base.hidden_size,
        );
        let num_v_heads = real_neom_rhs_cols(
            model,
            &qwen_layer_tensor_name(layer, "ssm_beta.weight"),
            base.hidden_size,
        );
        assert_eq!(
            real_neom_rhs_cols(
                model,
                &qwen_layer_tensor_name(layer, "ssm_alpha.weight"),
                base.hidden_size,
            ),
            num_v_heads
        );
        assert_eq!(
            real_neom_flat_tensor_len(model, &qwen_layer_tensor_name(layer, "ssm_dt.bias")),
            num_v_heads
        );
        assert_eq!(
            real_neom_flat_tensor_len(model, &qwen_layer_tensor_name(layer, "ssm_a")),
            num_v_heads
        );
        let head_v_dim =
            real_neom_flat_tensor_len(model, &qwen_layer_tensor_name(layer, "ssm_norm.weight"));
        assert_eq!(value_dim, num_v_heads * head_v_dim);
        assert!(conv_dim > value_dim);
        let key_twice = conv_dim - value_dim;
        assert_eq!(key_twice % 2, 0);
        let key_dim = key_twice / 2;
        assert_eq!(key_dim % head_v_dim, 0);
        let num_k_heads = key_dim / head_v_dim;
        let conv_elems =
            real_neom_flat_tensor_len(model, &qwen_layer_tensor_name(layer, "ssm_conv1d.weight"));
        assert_eq!(conv_elems % conv_dim, 0);
        let conv_kernel_size = conv_elems / conv_dim;
        assert_eq!(
            real_neom_rhs_cols(
                model,
                &qwen_layer_tensor_name(layer, "ssm_out.weight"),
                value_dim,
            ),
            base.hidden_size
        );
        ops::gdn::GdnLayerConfig {
            hidden_size: base.hidden_size,
            num_k_heads,
            num_v_heads,
            head_k_dim: head_v_dim,
            head_v_dim,
            conv_kernel_size,
            rms_norm_eps: f32::from_bits(base.rms_norm_eps_bits),
        }
    }

    fn real_neom_gdn_geometry(config: ops::gdn::GdnLayerConfig) -> [usize; 8] {
        [
            config.hidden_size,
            config.num_k_heads,
            config.num_v_heads,
            config.head_k_dim,
            config.head_v_dim,
            config.conv_kernel_size,
            config.conv_dim(),
            config.value_dim(),
        ]
    }

    fn real_neom_rhs_cols(model: &RealNeomModel, name: &str, rows: usize) -> usize {
        let tensor = model.tensor_named(name);
        if tensor.dtype == NeomDtype::Q5K {
            return quantized_cols_for_type(rows, tensor.weight_len, GgmlType::Q5K);
        }
        if tensor.dtype == NeomDtype::U8 {
            return infer_real_neom_quant_rhs_cols(name, rows, tensor.weight_len).unwrap();
        }
        let elements = real_neom_scalar_tensor_len(tensor);
        if tensor.rank == 2 {
            assert_eq!(tensor.dims[0] as usize, rows, "{name} rows");
            assert_eq!(elements, rows * tensor.dims[1] as usize, "{name} elements");
            return tensor.dims[1] as usize;
        }
        assert_eq!(elements % rows, 0, "{name} rows divide elements");
        let cols = elements / rows;
        assert_ne!(cols, 0, "{name} columns");
        cols
    }

    fn infer_real_neom_quant_rhs_cols(
        name: &str,
        rows: usize,
        byte_len: usize,
    ) -> Result<usize, String> {
        let candidates = [
            (TensorDtype::Q4K, GgmlType::Q4K),
            (TensorDtype::Q5_0, GgmlType::Q5_0),
            (TensorDtype::Q6K, GgmlType::Q6K),
            (TensorDtype::Q8_0, GgmlType::Q8_0),
        ];
        let mut matched = None;
        for (_dtype, ty) in candidates {
            let block = ty.block_size() as usize;
            let type_size = ty.type_size() as usize;
            if rows % block != 0 {
                continue;
            }
            let per_col = rows / block * type_size;
            if per_col == 0 || byte_len % per_col != 0 {
                continue;
            }
            let cols = byte_len / per_col;
            if cols == 0 || infer_real_neom_quant_dtype(name, rows, cols, byte_len).is_err() {
                continue;
            }
            if matched.is_some_and(|found| found != cols) {
                return Err(format!(
                    "{name} quant column count is ambiguous for {byte_len} bytes"
                ));
            }
            matched = Some(cols);
        }
        matched.ok_or_else(|| {
            format!("{name} byte_len={byte_len} does not match a supported quantized RHS")
        })
    }

    fn quantized_cols_for_type(rows: usize, byte_len: usize, ty: GgmlType) -> usize {
        let block = ty.block_size() as usize;
        let type_size = ty.type_size() as usize;
        assert_ne!(rows, 0);
        assert_eq!(rows % block, 0);
        let per_col = rows / block * type_size;
        assert_ne!(per_col, 0);
        assert_eq!(byte_len % per_col, 0);
        let cols = byte_len / per_col;
        assert_ne!(cols, 0);
        cols
    }

    fn real_neom_flat_tensor_len(model: &RealNeomModel, name: &str) -> usize {
        real_neom_scalar_tensor_len(model.tensor_named(name))
    }

    fn real_neom_scalar_tensor_len(tensor: &RealNeomTensor) -> usize {
        assert_ne!(
            tensor.dtype,
            NeomDtype::U8,
            "{} is quantized, not scalar",
            tensor.name
        );
        let element_size = tensor.dtype.size_bytes() as usize;
        assert_eq!(tensor.weight_len % element_size, 0, "{}", tensor.name);
        let elements = tensor.weight_len / element_size;
        let mut shape_elements = 1usize;
        for idx in 0..tensor.rank as usize {
            shape_elements *= tensor.dims[idx] as usize;
        }
        assert_eq!(
            elements, shape_elements,
            "{} shape matches bytes",
            tensor.name
        );
        elements
    }

    fn real_tensor_dtype_to_ggml(dtype: TensorDtype) -> GgmlType {
        match dtype {
            TensorDtype::F32 => GgmlType::F32,
            TensorDtype::Q4K => GgmlType::Q4K,
            TensorDtype::Q5_0 => GgmlType::Q5_0,
            TensorDtype::Q5K => GgmlType::Q5K,
            TensorDtype::Q6K => GgmlType::Q6K,
            TensorDtype::Q8_0 => GgmlType::Q8_0,
        }
    }

    fn top_two_logits(logits: &[f32]) -> RealDecodeStep {
        assert!(logits.len() >= 2);
        let mut best_token = 0usize;
        let mut best = logits[0];
        let mut runner_up_token = 1usize;
        let mut runner_up = logits[1];
        if runner_up > best {
            best_token = 1;
            best = runner_up;
            runner_up_token = 0;
            runner_up = logits[0];
        }
        for (token, &logit) in logits.iter().enumerate().skip(2) {
            if logit > best {
                runner_up_token = best_token;
                runner_up = best;
                best_token = token;
                best = logit;
            } else if logit > runner_up {
                runner_up_token = token;
                runner_up = logit;
            }
        }
        RealDecodeStep {
            token: best_token as u32,
            logit: best,
            runner_up_token: runner_up_token as u32,
            runner_up_logit: runner_up,
        }
    }

    /// Near-tie tolerance for the teacher-forced Stage-A parity oracle, in
    /// llama.cpp logprob units. Bounded at the reference's own observed
    /// self-margin (the 1.5B's token-0 ` gorgeous`/` beautiful` gap is 0.0242
    /// logprob); a greedy flip is tolerated ONLY when the reference itself ranks
    /// the two tokens within this margin — never looser. Anything wider is gross
    /// and fails. See the OT-30 methodology note in the build RETURN.
    const NEAR_TIE_EPS_LOGPROB: f32 = 0.025;

    enum ParityClass {
        Strict,
        NearTie,
        Gross,
    }

    /// Classify one teacher-forced position. `ref_topk` is the reference's
    /// top-K `(id, logprob)` (descending). A flip is a tolerated near-tie ONLY
    /// when the kernel and reference share the same top-2 (swapped) AND the
    /// reference's own top1/top2 self-margin is within `eps_logprob`.
    fn classify_parity(
        kernel_top1: u32,
        kernel_top2: u32,
        ref_topk: &[(u32, f32)],
        eps_logprob: f32,
    ) -> ParityClass {
        let ref_top1 = ref_topk[0].0;
        if kernel_top1 == ref_top1 {
            return ParityClass::Strict;
        }
        if ref_topk.len() >= 2 {
            let ref_top2 = ref_topk[1].0;
            let ref_self_margin = ref_topk[0].1 - ref_topk[1].1;
            let same_top2_swapped = kernel_top1 == ref_top2 && kernel_top2 == ref_top1;
            if same_top2_swapped && ref_self_margin <= eps_logprob {
                return ParityClass::NearTie;
            }
        }
        ParityClass::Gross
    }

    struct TeacherForcedParity {
        strict: usize,
        near_tie: usize,
        gross: usize,
        per_token_ms: f64,
        notes: Vec<String>,
    }

    /// Teacher-forced Stage-A parity: at each position the kernel's argmax is
    /// compared to the reference, then the KV cache is advanced with the
    /// REFERENCE top-1 (not the kernel's choice), so a near-tie at one position
    /// does not cascade. Returns the per-class breakdown.
    fn real_neom_teacher_forced_parity(
        model: &RealNeomModel,
        prompt: &[u32],
        ref_topk: &[Vec<(u32, f32)>],
        eps_logprob: f32,
        trace: &mut RealForwardTrace,
    ) -> TeacherForcedParity {
        assert!(
            !prompt.is_empty(),
            "teacher-forced prompt must not be empty"
        );
        assert!(
            !ref_topk.is_empty(),
            "teacher-forced reference must not be empty"
        );
        let started = std::time::Instant::now();
        let n = ref_topk.len();
        let mut state = RealNeomRuntimeState::new(model, prompt.len() + n);
        let mut position = 0usize;
        let mut logits = Vec::new();
        for &token in prompt {
            logits = real_neom_next_logits_with_trace(model, token, position, &mut state, trace);
            position += 1;
        }
        let mut strict = 0usize;
        let mut near_tie = 0usize;
        let mut gross = 0usize;
        let mut notes = Vec::new();
        for (i, entries) in ref_topk.iter().enumerate() {
            assert_all_finite(&format!("teacher-forced logits step {i}"), &logits);
            let kernel = top_two_logits(&logits);
            match classify_parity(kernel.token, kernel.runner_up_token, entries, eps_logprob) {
                ParityClass::Strict => strict += 1,
                ParityClass::NearTie => {
                    near_tie += 1;
                    notes.push(format!(
                        "pos {i}: NEAR-TIE kernel_top1={} ref_top1={} ref_self_margin={:.4}",
                        kernel.token,
                        entries[0].0,
                        entries[0].1 - entries[1].1
                    ));
                }
                ParityClass::Gross => {
                    gross += 1;
                    notes.push(format!(
                        "pos {i}: GROSS kernel_top1={} kernel_top2={} ref_top1={}",
                        kernel.token, kernel.runner_up_token, entries[0].0
                    ));
                }
            }
            if i + 1 < n {
                let ref_top1 = entries[0].0;
                logits =
                    real_neom_next_logits_with_trace(model, ref_top1, position, &mut state, trace);
                position += 1;
            }
        }
        TeacherForcedParity {
            strict,
            near_tie,
            gross,
            per_token_ms: started.elapsed().as_secs_f64() * 1000.0 / n as f64,
            notes,
        }
    }

    /// Parse the per-position reference: positions separated by `;`, entries by
    /// `,`, each entry `id:logprob` (descending logprob). Captured from
    /// llama.cpp `/completion` `n_probs=K` over the greedy decode.
    fn parse_topk_list(value: &str) -> Result<Vec<Vec<(u32, f32)>>, String> {
        let mut positions = Vec::new();
        for (pi, pos) in value.split(';').enumerate() {
            let pos = pos.trim();
            if pos.is_empty() {
                continue;
            }
            let mut entries = Vec::new();
            for entry in pos.split(',') {
                let entry = entry.trim();
                if entry.is_empty() {
                    continue;
                }
                let (id_s, lp_s) = entry
                    .split_once(':')
                    .ok_or_else(|| format!("position {pi} entry '{entry}' is not id:logprob"))?;
                let id = id_s
                    .trim()
                    .parse::<u32>()
                    .map_err(|err| format!("position {pi} id '{id_s}' invalid: {err}"))?;
                let lp = lp_s
                    .trim()
                    .parse::<f32>()
                    .map_err(|err| format!("position {pi} logprob '{lp_s}' invalid: {err}"))?;
                entries.push((id, lp));
            }
            if entries.is_empty() {
                return Err(format!("position {pi} has no entries"));
            }
            positions.push(entries);
        }
        if positions.is_empty() {
            return Err("topk list is empty".to_string());
        }
        Ok(positions)
    }

    fn ref_topk_from_env_or_skip(test_name: &str) -> Option<Vec<Vec<(u32, f32)>>> {
        let Ok(value) = std::env::var("NEO_REAL_LLAMA_TOPK") else {
            eprintln!(
                "SKIP {test_name}: NEO_REAL_LLAMA_TOPK unset (per-position 'id:logprob,...;...' \
                 reference for near-tie-aware parity)"
            );
            return None;
        };
        Some(parse_topk_list(&value).expect("parse NEO_REAL_LLAMA_TOPK"))
    }

    fn first_token_divergence(got: &[u32], expected: &[u32]) -> Option<usize> {
        got.iter()
            .zip(expected.iter())
            .position(|(got, expected)| got != expected)
            .or_else(|| (got.len() != expected.len()).then_some(got.len().min(expected.len())))
    }

    fn assert_all_finite(label: &str, values: &[f32]) {
        for (idx, &value) in values.iter().enumerate() {
            assert!(value.is_finite(), "{label}[{idx}] is not finite: {value}");
        }
    }

    fn read_u32_le_at(bytes: &[u8], off: usize) -> Result<u32, String> {
        let raw = checked_slice(bytes, off, 4)?;
        Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
    }

    fn read_u64_le_at(bytes: &[u8], off: usize) -> Result<u64, String> {
        let raw = checked_slice(bytes, off, 8)?;
        Ok(u64::from_le_bytes([
            raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
        ]))
    }

    fn assert_real_tensor(file: &GgufFile, name: &str, dims: &[u64]) {
        let tensor = file.tensor_named(name).expect("real Qwen tensor exists");
        assert_eq!(&tensor.dims[..tensor.n_dims as usize], dims, "{name} dims");
    }

    struct RealQwenLayerBuffers<'a> {
        attn_norm: Vec<f32>,
        attn_q: Vec<f32>,
        attn_k: Vec<f32>,
        attn_v: Vec<f32>,
        attn_q_bias: Vec<f32>,
        attn_k_bias: Vec<f32>,
        attn_v_bias: Vec<f32>,
        attn_o: Vec<f32>,
        ffn_norm: Vec<f32>,
        ffn_gate: Vec<f32>,
        ffn_up: Vec<f32>,
        ffn_down: Vec<f32>,
        dtypes: Qwen2LayerDtypes,
        attn_q_quantized: Option<&'a [u8]>,
        attn_k_quantized: Option<&'a [u8]>,
        attn_v_quantized: Option<&'a [u8]>,
        attn_o_quantized: Option<&'a [u8]>,
        ffn_gate_quantized: Option<&'a [u8]>,
        ffn_up_quantized: Option<&'a [u8]>,
        ffn_down_quantized: Option<&'a [u8]>,
    }

    impl<'a> RealQwenLayerBuffers<'a> {
        fn quantized_weights(&self) -> Qwen2QuantizedLayerWeights<'a> {
            Qwen2QuantizedLayerWeights {
                attn_q: self.attn_q_quantized,
                attn_k: self.attn_k_quantized,
                attn_v: self.attn_v_quantized,
                attn_o: self.attn_o_quantized,
                ffn_gate: self.ffn_gate_quantized,
                ffn_up: self.ffn_up_quantized,
                ffn_down: self.ffn_down_quantized,
            }
        }
    }

    fn real_qwen_cached_logits(file: &GgufFile, bytes: &[u8], context: &[u32]) -> Vec<f32> {
        real_qwen_cached_logits_with_quantized(file, bytes, context, true)
    }

    fn real_qwen_cached_logits_with_quantized(
        file: &GgufFile,
        bytes: &[u8],
        context: &[u32],
        use_quantized: bool,
    ) -> Vec<f32> {
        let config = Qwen2LayerConfig::qwen25_05b();
        let output_tensor = file
            .tensor_named("output.weight")
            .expect("real Qwen output tensor");
        assert_eq!(
            &output_tensor.dims[..output_tensor.n_dims as usize],
            &[config.hidden_size as u64, REAL_QWEN_VOCAB_SIZE as u64],
            "output.weight dims"
        );
        let mut caches =
            Qwen2KvCacheSet::new(forward_logic::QWEN25_05B_LAYER_COUNT, config, context.len())
                .unwrap();
        let mut hidden = Vec::new();

        for (position, &token) in context.iter().enumerate() {
            hidden = real_qwen_token_embedding(file, bytes, token, config.hidden_size);
            let mut layer_idx = 0usize;
            while layer_idx < forward_logic::QWEN25_05B_LAYER_COUNT {
                let layer = real_qwen_layer_buffers(file, bytes, layer_idx, config);
                let quantized = if use_quantized {
                    layer.quantized_weights()
                } else {
                    Qwen2QuantizedLayerWeights::NONE
                };
                let dtypes = if use_quantized {
                    layer.dtypes
                } else {
                    Qwen2LayerDtypes::F32
                };
                let weights = Qwen2LayerWeights {
                    attn_norm: &layer.attn_norm,
                    attn_q: &layer.attn_q,
                    attn_k: &layer.attn_k,
                    attn_v: &layer.attn_v,
                    attn_q_bias: Some(&layer.attn_q_bias),
                    attn_k_bias: Some(&layer.attn_k_bias),
                    attn_v_bias: Some(&layer.attn_v_bias),
                    attn_o: &layer.attn_o,
                    ffn_norm: &layer.ffn_norm,
                    ffn_gate: &layer.ffn_gate,
                    ffn_up: &layer.ffn_up,
                    ffn_down: &layer.ffn_down,
                    dtypes,
                    quantized,
                };
                let cache = caches.layer_mut(layer_idx).unwrap();
                hidden = forward_logic::forward_layer_with_gqa_attention(
                    &hidden,
                    weights,
                    config,
                    position as u64,
                    true,
                    Some(cache),
                    None,
                )
                .unwrap()
                .ffn
                .residual;
                layer_idx += 1;
            }
        }

        let output_norm = real_tensor_to_f32(file, bytes, "output_norm.weight");
        let output = real_tensor_to_ops_rhs(
            file,
            bytes,
            "output.weight",
            config.hidden_size,
            REAL_QWEN_VOCAB_SIZE,
        );
        let output_bias = file
            .tensor_named("output.bias")
            .map(|_| real_tensor_to_f32(file, bytes, "output.bias"));
        let output_bias_ref = output_bias.as_deref();
        forward_logic::qwen2_output_logits(
            &hidden,
            Qwen2OutputWeights {
                output_norm: &output_norm,
                output: &output,
                output_dtype: TensorDtype::F32,
                output_quantized: None,
                output_bias: output_bias_ref,
            },
            config,
            REAL_QWEN_VOCAB_SIZE,
        )
        .unwrap()
    }

    fn real_qwen_layer_buffers<'a>(
        file: &GgufFile,
        bytes: &'a [u8],
        layer: usize,
        config: Qwen2LayerConfig,
    ) -> RealQwenLayerBuffers<'a> {
        let attn_q_name = qwen_layer_tensor_name(layer, "attn_q.weight");
        let attn_k_name = qwen_layer_tensor_name(layer, "attn_k.weight");
        let attn_v_name = qwen_layer_tensor_name(layer, "attn_v.weight");
        let attn_o_name = qwen_layer_tensor_name(layer, "attn_output.weight");
        let ffn_gate_name = qwen_layer_tensor_name(layer, "ffn_gate.weight");
        let ffn_up_name = qwen_layer_tensor_name(layer, "ffn_up.weight");
        let ffn_down_name = qwen_layer_tensor_name(layer, "ffn_down.weight");
        RealQwenLayerBuffers {
            attn_norm: real_tensor_to_f32(
                file,
                bytes,
                &qwen_layer_tensor_name(layer, "attn_norm.weight"),
            ),
            attn_q: real_tensor_to_ops_rhs(
                file,
                bytes,
                &attn_q_name,
                config.hidden_size,
                config.q_len(),
            ),
            attn_k: real_tensor_to_ops_rhs(
                file,
                bytes,
                &attn_k_name,
                config.hidden_size,
                config.kv_len(),
            ),
            attn_v: real_tensor_to_ops_rhs(
                file,
                bytes,
                &attn_v_name,
                config.hidden_size,
                config.kv_len(),
            ),
            attn_q_bias: real_tensor_to_f32(
                file,
                bytes,
                &qwen_layer_tensor_name(layer, "attn_q.bias"),
            ),
            attn_k_bias: real_tensor_to_f32(
                file,
                bytes,
                &qwen_layer_tensor_name(layer, "attn_k.bias"),
            ),
            attn_v_bias: real_tensor_to_f32(
                file,
                bytes,
                &qwen_layer_tensor_name(layer, "attn_v.bias"),
            ),
            attn_o: real_tensor_to_ops_rhs(
                file,
                bytes,
                &attn_o_name,
                config.hidden_size,
                config.hidden_size,
            ),
            ffn_norm: real_tensor_to_f32(
                file,
                bytes,
                &qwen_layer_tensor_name(layer, "ffn_norm.weight"),
            ),
            ffn_gate: real_tensor_to_ops_rhs(
                file,
                bytes,
                &ffn_gate_name,
                config.hidden_size,
                config.intermediate_size,
            ),
            ffn_up: real_tensor_to_ops_rhs(
                file,
                bytes,
                &ffn_up_name,
                config.hidden_size,
                config.intermediate_size,
            ),
            ffn_down: real_tensor_to_ops_rhs(
                file,
                bytes,
                &ffn_down_name,
                config.intermediate_size,
                config.hidden_size,
            ),
            dtypes: Qwen2LayerDtypes {
                attn_q: real_tensor_dtype(file, &attn_q_name),
                attn_k: real_tensor_dtype(file, &attn_k_name),
                attn_v: real_tensor_dtype(file, &attn_v_name),
                attn_o: real_tensor_dtype(file, &attn_o_name),
                ffn_gate: real_tensor_dtype(file, &ffn_gate_name),
                ffn_up: real_tensor_dtype(file, &ffn_up_name),
                ffn_down: real_tensor_dtype(file, &ffn_down_name),
            },
            attn_q_quantized: real_tensor_to_quantized_rhs(
                file,
                bytes,
                &attn_q_name,
                config.hidden_size,
                config.q_len(),
            ),
            attn_k_quantized: real_tensor_to_quantized_rhs(
                file,
                bytes,
                &attn_k_name,
                config.hidden_size,
                config.kv_len(),
            ),
            attn_v_quantized: real_tensor_to_quantized_rhs(
                file,
                bytes,
                &attn_v_name,
                config.hidden_size,
                config.kv_len(),
            ),
            attn_o_quantized: real_tensor_to_quantized_rhs(
                file,
                bytes,
                &attn_o_name,
                config.hidden_size,
                config.hidden_size,
            ),
            ffn_gate_quantized: real_tensor_to_quantized_rhs(
                file,
                bytes,
                &ffn_gate_name,
                config.hidden_size,
                config.intermediate_size,
            ),
            ffn_up_quantized: real_tensor_to_quantized_rhs(
                file,
                bytes,
                &ffn_up_name,
                config.hidden_size,
                config.intermediate_size,
            ),
            ffn_down_quantized: real_tensor_to_quantized_rhs(
                file,
                bytes,
                &ffn_down_name,
                config.intermediate_size,
                config.hidden_size,
            ),
        }
    }

    fn qwen_layer_tensor_name(layer: usize, suffix: &str) -> String {
        format!("blk.{layer}.{suffix}")
    }

    fn ffn_norm_tensor_candidates(layer: usize) -> [String; 2] {
        [
            format!("blk.{layer}.post_attention_norm.weight"),
            format!("blk.{layer}.ffn_norm.weight"),
        ]
    }

    fn gdn_layer_tensor_candidates(layer: usize, suffix: &str) -> [String; 5] {
        let qwen35_name = match suffix {
            "in_proj_qkv.weight" => format!("blk.{layer}.attn_qkv.weight"),
            "in_proj_z.weight" => format!("blk.{layer}.attn_gate.weight"),
            "in_proj_b.weight" => format!("blk.{layer}.ssm_beta.weight"),
            "in_proj_a.weight" => format!("blk.{layer}.ssm_alpha.weight"),
            "conv1d.weight" => format!("blk.{layer}.ssm_conv1d.weight"),
            "A_log" => format!("blk.{layer}.ssm_a"),
            "dt_bias" => format!("blk.{layer}.ssm_dt.bias"),
            "norm.weight" => format!("blk.{layer}.ssm_norm.weight"),
            "out_proj.weight" => format!("blk.{layer}.ssm_out.weight"),
            _ => format!("blk.{layer}.{suffix}"),
        };

        [
            qwen35_name,
            format!("blk.{layer}.gdn.{suffix}"),
            format!("blk.{layer}.linear_attn.{suffix}"),
            format!("blk.{layer}.ssm.{suffix}"),
            format!("blk.{layer}.{suffix}"),
        ]
    }

    fn real_tensor_to_ops_rhs(
        file: &GgufFile,
        bytes: &[u8],
        name: &str,
        rows: usize,
        cols: usize,
    ) -> Vec<f32> {
        let tensor = file.tensor_named(name).expect("real Qwen tensor exists");
        assert_eq!(
            &tensor.dims[..tensor.n_dims as usize],
            &[rows as u64, cols as u64],
            "{name} dims"
        );
        let raw = real_tensor_to_f32(file, bytes, name);
        assert_eq!(raw.len(), rows * cols, "{name} element count");
        let mut out = vec![0.0f32; raw.len()];
        let mut col = 0usize;
        while col < cols {
            let mut row = 0usize;
            while row < rows {
                out[row * cols + col] = raw[col * rows + row];
                row += 1;
            }
            col += 1;
        }
        out
    }

    fn real_tensor_dtype(file: &GgufFile, name: &str) -> TensorDtype {
        match file.tensor_named(name).expect("real Qwen tensor exists").ty {
            GgmlType::F32 => TensorDtype::F32,
            GgmlType::Q4K => TensorDtype::Q4K,
            GgmlType::Q5_0 => TensorDtype::Q5_0,
            GgmlType::Q5K => TensorDtype::Q5K,
            GgmlType::Q6K => TensorDtype::Q6K,
            GgmlType::Q8_0 => TensorDtype::Q8_0,
            other => panic!("{name} has unsupported forward dtype {}", other.name()),
        }
    }

    fn real_tensor_to_quantized_rhs<'a>(
        file: &GgufFile,
        bytes: &'a [u8],
        name: &str,
        rows: usize,
        cols: usize,
    ) -> Option<&'a [u8]> {
        let tensor = file.tensor_named(name).expect("real Qwen tensor exists");
        assert_eq!(
            &tensor.dims[..tensor.n_dims as usize],
            &[rows as u64, cols as u64],
            "{name} dims"
        );
        if !matches!(
            tensor.ty,
            GgmlType::Q4K | GgmlType::Q5_0 | GgmlType::Q5K | GgmlType::Q6K | GgmlType::Q8_0
        ) {
            return None;
        }

        let block = tensor.ty.block_size() as usize;
        let type_size = tensor.ty.type_size() as usize;
        assert_eq!(rows % block, 0, "{name} quant row-block multiple");
        let data = file.tensor_data(bytes, tensor).unwrap();
        let expected_len = cols * (rows / block) * type_size;
        assert_eq!(data.len(), expected_len, "{name} quant byte length");
        Some(data)
    }

    fn real_qwen_token_embedding(
        file: &GgufFile,
        bytes: &[u8],
        token: u32,
        hidden_size: usize,
    ) -> Vec<f32> {
        let tensor = file
            .tensor_named("token_embd.weight")
            .expect("real Qwen token embedding tensor");
        assert_eq!(
            &tensor.dims[..tensor.n_dims as usize],
            &[hidden_size as u64, REAL_QWEN_VOCAB_SIZE as u64],
            "token_embd.weight dims"
        );
        assert!((token as usize) < REAL_QWEN_VOCAB_SIZE);
        let block = tensor.ty.block_size() as usize;
        let type_size = tensor.ty.type_size() as usize;
        assert_eq!(hidden_size % block, 0);
        let row_bytes = hidden_size / block * type_size;
        let start = token as usize * row_bytes;
        let data = file.tensor_data(bytes, tensor).unwrap();
        let row = &data[start..start + row_bytes];
        let row_tensor = gguf_tensor("token_embd.row", tensor.ty, &[hidden_size as u64]);
        tensor_data_to_f32(&row_tensor, row)
    }

    fn real_tensor_to_f32(file: &GgufFile, bytes: &[u8], name: &str) -> Vec<f32> {
        let tensor = file.tensor_named(name).expect("real Qwen tensor exists");
        let data = file.tensor_data(bytes, tensor).unwrap();
        tensor_data_to_f32(tensor, data)
    }

    fn tensor_data_to_f32(tensor: &GgufTensorInfo, data: &[u8]) -> Vec<f32> {
        match tensor.ty {
            GgmlType::Q4K | GgmlType::Q5_0 | GgmlType::Q5K | GgmlType::Q6K | GgmlType::Q8_0 => {
                gguf_dequant_logic::dequantize_tensor_data_to_f32(tensor, data).unwrap()
            }
            GgmlType::F32 => data
                .chunks_exact(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect(),
            GgmlType::F16 => data
                .chunks_exact(2)
                .map(|chunk| reference_f16_bits_to_f32(u16::from_le_bytes([chunk[0], chunk[1]])))
                .collect(),
            _ => panic!("unsupported tensor type {}", tensor.ty.name()),
        }
    }

    fn real_qwen_quant_reference(file: &GgufFile, bytes: &[u8], name: &str) -> Vec<f32> {
        let tensor = file
            .tensor_named(name)
            .expect("real Qwen quantized tensor exists");
        let data = file.tensor_data(bytes, tensor).unwrap();
        match tensor.ty {
            GgmlType::Q4K => llama_reference_dequant_q4_k(data),
            GgmlType::Q5_0 => llama_reference_dequant_q5_0(data),
            GgmlType::Q5K => llama_reference_dequant_q5_k(data),
            GgmlType::Q6K => llama_reference_dequant_q6_k(data),
            GgmlType::Q8_0 => llama_reference_dequant_q8_0(data),
            _ => panic!("{name} has unsupported tensor type {}", tensor.ty.name()),
        }
    }

    fn qwen_fixed_hidden_input() -> Vec<f32> {
        (0..896usize)
            .map(|idx| {
                let coarse = ((idx % 31) as f32 - 15.0) / 48.0;
                let fine = ((idx / 31) % 11) as f32 * 0.001;
                coarse + fine
            })
            .collect()
    }

    fn reference_rms_norm(input: &[f32], gamma: &[f32], eps: f32) -> Vec<f32> {
        assert_eq!(input.len(), gamma.len());
        let mean_sq = input.iter().map(|value| value * value).sum::<f32>() / input.len() as f32;
        let denom = (mean_sq + eps).sqrt();
        input
            .iter()
            .zip(gamma.iter())
            .map(|(&value, &scale)| value / denom * scale)
            .collect()
    }

    fn reference_rms_norm_heads(
        input: &[f32],
        gamma: &[f32],
        heads: usize,
        head_dim: usize,
        eps: f32,
    ) -> Vec<f32> {
        assert_eq!(input.len(), heads * head_dim);
        assert_eq!(gamma.len(), head_dim);
        let mut out = Vec::with_capacity(input.len());
        for head in 0..heads {
            let start = head * head_dim;
            let end = start + head_dim;
            out.extend_from_slice(&reference_rms_norm(&input[start..end], gamma, eps));
        }
        out
    }

    fn reference_matmul(
        lhs: &[f32],
        lhs_rows: usize,
        lhs_cols: usize,
        rhs: &[f32],
        rhs_cols: usize,
    ) -> Vec<f32> {
        assert_eq!(lhs.len(), lhs_rows * lhs_cols);
        assert_eq!(rhs.len(), lhs_cols * rhs_cols);
        let mut out = vec![0.0f32; lhs_rows * rhs_cols];
        for row in 0..lhs_rows {
            for col in 0..rhs_cols {
                let mut acc = 0.0f32;
                for inner in 0..lhs_cols {
                    acc += lhs[row * lhs_cols + inner] * rhs[inner * rhs_cols + col];
                }
                out[row * rhs_cols + col] = acc;
            }
        }
        out
    }

    fn reference_swiglu(gate: &[f32], up: &[f32]) -> Vec<f32> {
        assert_eq!(gate.len(), up.len());
        gate.iter()
            .zip(up.iter())
            .map(|(&gate, &up)| gate / (1.0 + (-gate).exp()) * up)
            .collect()
    }

    fn reference_rope_neox(
        input: &[f32],
        heads: usize,
        head_dim: usize,
        rope_dimensions: usize,
        position: u64,
        theta_scale: f32,
    ) -> Vec<f32> {
        assert_eq!(input.len(), heads * head_dim);
        assert!(rope_dimensions <= head_dim);
        assert_eq!(rope_dimensions % 2, 0);
        let mut out = input.to_vec();
        for head in 0..heads {
            let base = head * head_dim;
            let pair_offset = rope_dimensions / 2;
            let mut theta = position as f32;
            for pair in 0..pair_offset {
                let x0 = input[base + pair];
                let x1 = input[base + pair_offset + pair];
                let cos_theta = theta.cos();
                let sin_theta = theta.sin();
                out[base + pair] = x0 * cos_theta - x1 * sin_theta;
                out[base + pair_offset + pair] = x0 * sin_theta + x1 * cos_theta;
                theta *= theta_scale;
            }
        }
        out
    }

    fn reference_add(lhs: &[f32], rhs: &[f32]) -> Vec<f32> {
        assert_eq!(lhs.len(), rhs.len());
        lhs.iter()
            .zip(rhs.iter())
            .map(|(&lhs, &rhs)| lhs + rhs)
            .collect()
    }

    fn phase9_q8_oracle_quant(idx: usize) -> i8 {
        if idx == 0 {
            -127
        } else {
            (idx as i16 - 128).clamp(-127, 127) as i8
        }
    }

    fn phase9_generate_cosine_data(offset: f32, len: usize) -> Vec<f32> {
        (0..len)
            .map(|idx| 0.1 + 2.0 * (idx as f32 + offset).cos())
            .collect()
    }

    fn phase9_synthetic_q4_k_from_cosine(weights: &[f32]) -> Vec<u8> {
        assert_eq!(weights.len() % QK_K, 0);
        const D_BITS: u16 = 0x2400; // f16(1/64)
        const DMIN_BITS: u16 = 0x2800; // f16(1/32)
        const D: f32 = 1.0 / 64.0;
        const DMIN: f32 = 1.0 / 32.0;

        let mut out = Vec::with_capacity(weights.len() / QK_K * BLOCK_Q4_K_SIZE);
        for source in weights.chunks_exact(QK_K) {
            let mut scale_quants = [0u8; 8];
            let mut min_quants = [0u8; 8];
            let mut qvals = [0u8; QK_K];

            for sub in 0..8 {
                let start = sub * 32;
                let chunk = &source[start..start + 32];
                let min = chunk
                    .iter()
                    .fold(f32::INFINITY, |acc, &value| acc.min(value));
                let max = chunk
                    .iter()
                    .fold(f32::NEG_INFINITY, |acc, &value| acc.max(value));
                let scale = ((max - min) / 15.0).max(D);
                let scale_quant = ((scale / D).round() as i32).clamp(1, 63) as u8;
                let min_quant = if min < 0.0 {
                    ((-min / DMIN).round() as i32).clamp(0, 63) as u8
                } else {
                    0
                };
                scale_quants[sub] = scale_quant;
                min_quants[sub] = min_quant;

                let dequant_scale = D * scale_quant as f32;
                let dequant_min = DMIN * min_quant as f32;
                for lane in 0..32 {
                    let q = ((chunk[lane] + dequant_min) / dequant_scale).round() as i32;
                    qvals[start + lane] = q.clamp(0, 15) as u8;
                }
            }

            let packed_scales = phase9_pack_q4_k_scales(scale_quants, min_quants);
            let mut block = [0u8; BLOCK_Q4_K_SIZE];
            block[0..2].copy_from_slice(&D_BITS.to_le_bytes());
            block[2..4].copy_from_slice(&DMIN_BITS.to_le_bytes());
            block[4..16].copy_from_slice(&packed_scales);
            for pair in 0..4 {
                for lane in 0..32 {
                    let low = qvals[pair * 64 + lane];
                    let high = qvals[pair * 64 + 32 + lane];
                    block[16 + pair * 32 + lane] = low | (high << 4);
                }
            }
            out.extend_from_slice(&block);
        }
        out
    }

    fn phase9_pack_q4_k_scales(scales: [u8; 8], mins: [u8; 8]) -> [u8; K_SCALE_SIZE] {
        let mut packed = [0u8; K_SCALE_SIZE];
        for idx in 0..4 {
            packed[idx] = scales[idx] & 0x3f;
            packed[idx + 4] = mins[idx] & 0x3f;
        }
        for idx in 4..8 {
            packed[idx + 4] = (scales[idx] & 0x0f) | ((mins[idx] & 0x0f) << 4);
            packed[idx - 4] |= (scales[idx] >> 4) << 6;
            packed[idx] |= (mins[idx] >> 4) << 6;
        }
        packed
    }

    fn phase9_next_u32(seed: &mut u64) -> u32 {
        *seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (*seed >> 32) as u32
    }

    fn phase9_random_q4_k_blocks(blocks: usize, seed: &mut u64) -> Vec<u8> {
        const D_BITS: [u16; 4] = [0x2400, 0x2800, 0x2c00, 0x3000];
        const DMIN_BITS: [u16; 4] = [0x2000, 0x2400, 0x2800, 0x2c00];

        let mut out = Vec::with_capacity(blocks * BLOCK_Q4_K_SIZE);
        for block_idx in 0..blocks {
            let mut scale_quants = [0u8; 8];
            let mut min_quants = [0u8; 8];
            for idx in 0..8 {
                scale_quants[idx] = (phase9_next_u32(seed) % 64) as u8;
                min_quants[idx] = (phase9_next_u32(seed) % 64) as u8;
            }

            let mut block = [0u8; BLOCK_Q4_K_SIZE];
            block[0..2].copy_from_slice(&D_BITS[block_idx % D_BITS.len()].to_le_bytes());
            block[2..4].copy_from_slice(&DMIN_BITS[block_idx % DMIN_BITS.len()].to_le_bytes());
            block[4..16].copy_from_slice(&phase9_pack_q4_k_scales(scale_quants, min_quants));
            for lane in 0..(QK_K / 2) {
                let low = (phase9_next_u32(seed) & 0x0f) as u8;
                let high = (phase9_next_u32(seed) & 0x0f) as u8;
                block[16 + lane] = low | (high << 4);
            }
            out.extend_from_slice(&block);
        }
        out
    }

    fn phase9_random_q8_k_blocks(blocks: usize, seed: &mut u64) -> Vec<ops::qk_matmul::BlockQ8K> {
        let mut out = vec![ops::qk_matmul::BlockQ8K::ZERO; blocks];
        for block in &mut out {
            let scale = ((phase9_next_u32(seed) % 1024) + 1) as f32 / 1024.0;
            block.d = if phase9_next_u32(seed) & 1 == 0 {
                scale
            } else {
                -scale
            };
            for lane in 0..ops::qk_matmul::Q8_K_VALUES {
                block.qs[lane] = ((phase9_next_u32(seed) % 255) as i16 - 127) as i8;
            }
            for group in 0..ops::qk_matmul::Q8_K_BSUMS {
                let mut sum = 0i16;
                for lane in 0..16 {
                    sum += block.qs[group * 16 + lane] as i16;
                }
                block.bsums[group] = sum;
            }
        }
        out
    }

    /// Compare scalar vs AVX2-dispatched q8_K quantization block-by-block on
    /// `(d, qs, bsums)`. `d` is compared by bit-pattern (exact, not
    /// tolerance); `qs` and `bsums` are integer arrays. Any divergence is an
    /// AVX2 bug, never a scalar bug (the scalar is the frozen oracle).
    fn phase9_assert_q8_k_quantize_avx2_eq_scalar(input: &[f32], case: &str) {
        let n = ops::qk_matmul::Q8_K_VALUES;
        assert_eq!(
            input.len() % n,
            0,
            "{case}: input len {} not a multiple of {n}",
            input.len()
        );
        let blocks = input.len() / n;
        let mut scalar = vec![ops::qk_matmul::BlockQ8K::ZERO; blocks];
        let mut avx2 = vec![ops::qk_matmul::BlockQ8K::ZERO; blocks];
        ops::qk_matmul::quantize_row_q8_k(input, &mut scalar).unwrap();
        ops::qk_matmul::quantize_row_q8_k_with_kernel(
            input,
            &mut avx2,
            ops::qk_matmul::QKMatmulKernel::AutoAvx2,
        )
        .unwrap();
        for (b, (s, a)) in scalar.iter().zip(avx2.iter()).enumerate() {
            assert_eq!(
                s.d.to_bits(),
                a.d.to_bits(),
                "{case} block={b} d: scalar={} avx2={}",
                s.d,
                a.d
            );
            assert_eq!(s.qs, a.qs, "{case} block={b}: qs mismatch");
            assert_eq!(s.bsums, a.bsums, "{case} block={b}: bsums mismatch");
        }
    }

    fn llama_reference_vec_dot_q4_k_q8_k(q4: &[u8], q8: &[ops::qk_matmul::BlockQ8K]) -> f32 {
        assert_eq!(q4.len(), q8.len() * BLOCK_Q4_K_SIZE);
        let mut sums = [0.0f32; 8];
        let mut sumf = 0.0f32;
        for (block_idx, q4_block) in q4.chunks_exact(BLOCK_Q4_K_SIZE).enumerate() {
            let q8_block = &q8[block_idx];
            let mut aux8 = [0i8; QK_K];
            let mut aux_idx = 0usize;
            let qs = &q4_block[16..144];
            for q4_chunk in qs.chunks_exact(32) {
                for &byte in q4_chunk {
                    aux8[aux_idx] = (byte & 0x0f) as i8;
                    aux_idx += 1;
                }
                for &byte in q4_chunk {
                    aux8[aux_idx] = (byte >> 4) as i8;
                    aux_idx += 1;
                }
            }

            let (scales, mins) = llama_reference_q4_k_depack_scales(&q4_block[4..16]);
            let mut sumi = 0i32;
            for group in 0..(QK_K / 16) {
                sumi += q8_block.bsums[group] as i32 * mins[group / 2] as i32;
            }

            let mut aux32 = [0i32; 8];
            let mut q8_idx = 0usize;
            let mut a_idx = 0usize;
            for &scale in &scales {
                for _ in 0..4 {
                    for lane in 0..8 {
                        aux32[lane] += scale as i32
                            * q8_block.qs[q8_idx + lane] as i32
                            * aux8[a_idx + lane] as i32;
                    }
                    q8_idx += 8;
                    a_idx += 8;
                }
            }

            let d = reference_f16_bits_to_f32(u16::from_le_bytes([q4_block[0], q4_block[1]]))
                * q8_block.d;
            for lane in 0..8 {
                sums[lane] += d * aux32[lane] as f32;
            }
            let dmin = reference_f16_bits_to_f32(u16::from_le_bytes([q4_block[2], q4_block[3]]))
                * q8_block.d;
            sumf -= dmin * sumi as f32;
        }
        sumf + sums.iter().sum::<f32>()
    }

    fn llama_reference_q4_k_depack_scales(raw: &[u8]) -> ([u8; 8], [u8; 8]) {
        assert_eq!(raw.len(), K_SCALE_SIZE);
        const KMASK1: u32 = 0x3f3f_3f3f;
        const KMASK2: u32 = 0x0f0f_0f0f;
        const KMASK3: u32 = 0x0303_0303;
        let mut bytes = [0u8; 16];
        bytes[..K_SCALE_SIZE].copy_from_slice(raw);
        let mut utmp = [0u32; 4];
        for idx in 0..4 {
            let off = idx * 4;
            utmp[idx] =
                u32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]]);
        }
        utmp[3] = ((utmp[2] >> 4) & KMASK2) | (((utmp[1] >> 6) & KMASK3) << 4);
        let uaux = utmp[1] & KMASK1;
        utmp[1] = (utmp[2] & KMASK2) | (((utmp[0] >> 6) & KMASK3) << 4);
        utmp[2] = uaux;
        utmp[0] &= KMASK1;

        let mut depacked = [0u8; 16];
        for (idx, word) in utmp.iter().enumerate() {
            depacked[idx * 4..idx * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        let mut scales = [0u8; 8];
        let mut mins = [0u8; 8];
        scales.copy_from_slice(&depacked[..8]);
        mins.copy_from_slice(&depacked[8..16]);
        (scales, mins)
    }

    fn dot_product(lhs: &[f32], rhs: &[f32]) -> f32 {
        assert_eq!(lhs.len(), rhs.len());
        lhs.iter()
            .zip(rhs.iter())
            .fold(0.0f32, |acc, (&lhs, &rhs)| acc + lhs * rhs)
    }

    fn llama_reference_dequant_q4_k(raw: &[u8]) -> Vec<f32> {
        assert_eq!(raw.len() % BLOCK_Q4_K_SIZE, 0);
        let mut out = Vec::with_capacity(raw.len() / BLOCK_Q4_K_SIZE * QK_K);
        for block in raw.chunks_exact(BLOCK_Q4_K_SIZE) {
            let d = reference_f16_bits_to_f32(u16::from_le_bytes([block[0], block[1]]));
            let dmin = reference_f16_bits_to_f32(u16::from_le_bytes([block[2], block[3]]));
            let scales = &block[4..16];
            let qs = &block[16..144];
            let mut scale_idx = 0usize;
            let mut quant_idx = 0usize;
            while scale_idx < 8 {
                let (sc1, m1) = reference_get_scale_min_k4(scale_idx, scales);
                let (sc2, m2) = reference_get_scale_min_k4(scale_idx + 1, scales);
                let d1 = d * sc1 as f32;
                let min1 = dmin * m1 as f32;
                let d2 = d * sc2 as f32;
                let min2 = dmin * m2 as f32;
                for lane in 0..32 {
                    out.push(d1 * (qs[quant_idx + lane] & 0x0f) as f32 - min1);
                }
                for lane in 0..32 {
                    out.push(d2 * (qs[quant_idx + lane] >> 4) as f32 - min2);
                }
                quant_idx += 32;
                scale_idx += 2;
            }
        }
        out
    }

    fn llama_reference_dequant_q5_0(raw: &[u8]) -> Vec<f32> {
        const BLOCK_Q5_0_SIZE: usize = 22;
        const QK5_0: usize = 32;
        assert_eq!(raw.len() % BLOCK_Q5_0_SIZE, 0);
        let mut out = Vec::with_capacity(raw.len() / BLOCK_Q5_0_SIZE * QK5_0);
        for block in raw.chunks_exact(BLOCK_Q5_0_SIZE) {
            let d = reference_f16_bits_to_f32(u16::from_le_bytes([block[0], block[1]]));
            let qh = u32::from_le_bytes([block[2], block[3], block[4], block[5]]);
            let qs = &block[6..22];
            for (idx, &q) in qs.iter().enumerate().take(16) {
                let high = ((qh >> idx) & 1) << 4;
                let quant = ((q & 0x0f) as u32 | high) as i32 - 16;
                out.push(quant as f32 * d);
            }
            for (idx, &q) in qs.iter().enumerate().take(16) {
                let high = ((qh >> (idx + 16)) & 1) << 4;
                let quant = ((q >> 4) as u32 | high) as i32 - 16;
                out.push(quant as f32 * d);
            }
        }
        out
    }

    fn llama_reference_dequant_q5_k(raw: &[u8]) -> Vec<f32> {
        const BLOCK_Q5_K_SIZE: usize = 176;
        assert_eq!(raw.len() % BLOCK_Q5_K_SIZE, 0);
        let mut out = Vec::with_capacity(raw.len() / BLOCK_Q5_K_SIZE * QK_K);
        for block in raw.chunks_exact(BLOCK_Q5_K_SIZE) {
            let d = reference_f16_bits_to_f32(u16::from_le_bytes([block[0], block[1]]));
            let dmin = reference_f16_bits_to_f32(u16::from_le_bytes([block[2], block[3]]));
            let scales = &block[4..16];
            let qh = &block[16..48];
            let qs = &block[48..176];

            let mut scale_idx = 0usize;
            let mut quant_idx = 0usize;
            let mut high_low = 1u8;
            let mut high_high = 2u8;
            while scale_idx < 8 {
                let (sc1, m1) = reference_get_scale_min_k4(scale_idx, scales);
                let (sc2, m2) = reference_get_scale_min_k4(scale_idx + 1, scales);
                let d1 = d * sc1 as f32;
                let min1 = dmin * m1 as f32;
                let d2 = d * sc2 as f32;
                let min2 = dmin * m2 as f32;

                for lane in 0..32 {
                    let high = if qh[lane] & high_low != 0 { 16 } else { 0 };
                    out.push(d1 * ((qs[quant_idx + lane] & 0x0f) + high) as f32 - min1);
                }
                for lane in 0..32 {
                    let high = if qh[lane] & high_high != 0 { 16 } else { 0 };
                    out.push(d2 * ((qs[quant_idx + lane] >> 4) + high) as f32 - min2);
                }

                quant_idx += 32;
                scale_idx += 2;
                high_low <<= 2;
                high_high <<= 2;
            }
        }
        out
    }

    fn llama_reference_dequant_q6_k(raw: &[u8]) -> Vec<f32> {
        const BLOCK_Q6_K_SIZE: usize = 210;
        assert_eq!(raw.len() % BLOCK_Q6_K_SIZE, 0);
        let mut out = Vec::with_capacity(raw.len() / BLOCK_Q6_K_SIZE * QK_K);
        for block in raw.chunks_exact(BLOCK_Q6_K_SIZE) {
            let ql = &block[0..128];
            let qh = &block[128..192];
            let scales = &block[192..208];
            let d = reference_f16_bits_to_f32(u16::from_le_bytes([block[208], block[209]]));
            let mut ql_base = 0usize;
            let mut qh_base = 0usize;
            let mut scale_base = 0usize;
            while ql_base < 128 {
                let mut values = [0.0f32; 128];
                for lane in 0..32 {
                    let scale_pair = lane / 16;
                    let high = qh[qh_base + lane];
                    let low_a = ql[ql_base + lane];
                    let low_b = ql[ql_base + 32 + lane];
                    let q1 = (((low_a & 0x0f) | ((high & 0x03) << 4)) as i8) - 32;
                    let q2 = (((low_b & 0x0f) | (((high >> 2) & 0x03) << 4)) as i8) - 32;
                    let q3 = (((low_a >> 4) | (((high >> 4) & 0x03) << 4)) as i8) - 32;
                    let q4 = (((low_b >> 4) | (((high >> 6) & 0x03) << 4)) as i8) - 32;
                    values[lane] = d * scales[scale_base + scale_pair] as i8 as f32 * q1 as f32;
                    values[lane + 32] =
                        d * scales[scale_base + scale_pair + 2] as i8 as f32 * q2 as f32;
                    values[lane + 64] =
                        d * scales[scale_base + scale_pair + 4] as i8 as f32 * q3 as f32;
                    values[lane + 96] =
                        d * scales[scale_base + scale_pair + 6] as i8 as f32 * q4 as f32;
                }
                out.extend_from_slice(&values);
                ql_base += 64;
                qh_base += 32;
                scale_base += 8;
            }
        }
        out
    }

    fn llama_reference_dequant_q8_0(raw: &[u8]) -> Vec<f32> {
        assert_eq!(raw.len() % BLOCK_Q8_0_SIZE, 0);
        let mut out = Vec::with_capacity(raw.len() / BLOCK_Q8_0_SIZE * QK8_0);
        for block in raw.chunks_exact(BLOCK_Q8_0_SIZE) {
            let d = reference_f16_bits_to_f32(u16::from_le_bytes([block[0], block[1]]));
            for idx in 0..QK8_0 {
                out.push(block[2 + idx] as i8 as f32 * d);
            }
        }
        out
    }

    fn reference_get_scale_min_k4(index: usize, scales: &[u8]) -> (u8, u8) {
        if index < 4 {
            (scales[index] & 63, scales[index + 4] & 63)
        } else {
            (
                (scales[index + 4] & 0x0f) | ((scales[index - 4] >> 6) << 4),
                (scales[index + 4] >> 4) | ((scales[index] >> 6) << 4),
            )
        }
    }

    fn reference_f16_bits_to_f32(bits: u16) -> f32 {
        let sign = ((bits & 0x8000) as u32) << 16;
        let exp = ((bits & 0x7c00) >> 10) as u32;
        let frac = (bits & 0x03ff) as u32;
        let out = if exp == 0 {
            if frac == 0 {
                sign
            } else {
                let mut mant = frac;
                let mut exp2 = -14i32;
                while mant & 0x0400 == 0 {
                    mant <<= 1;
                    exp2 -= 1;
                }
                mant &= 0x03ff;
                sign | (((exp2 + 127) as u32) << 23) | (mant << 13)
            }
        } else if exp == 0x1f {
            sign | 0x7f80_0000 | (frac << 13)
        } else {
            sign | ((exp + 112) << 23) | (frac << 13)
        };
        f32::from_bits(out)
    }

    fn rel_err(a: f32, b: f32) -> f32 {
        let diff = (a - b).abs();
        let norm = a.abs().max(b.abs());
        if norm < 1e-30 {
            return diff;
        }
        diff / norm
    }

    const REL_EPS: f32 = 1.0 / (1 << 12) as f32;
    const ROPE_REL_EPS: f32 = 1.0 / (1 << 9) as f32;
    const REAL_WEIGHT_REL_EPS: f32 = 1.0e-6;
    const REAL_FORWARD_REL_EPS: f32 = 5.0e-2;
    const RMS_NORM_EPS: f32 = 1.0e-5;
    const REAL_QWEN_VOCAB_SIZE: usize = 151_936;
    const REAL_QWEN_HELLO_TOKEN: u32 = 9707;
    const REAL_QWEN_STAGE_A_PROMPT: [u32; 1] = [REAL_QWEN_HELLO_TOKEN];
    const REAL_QWEN_LLAMA_CPP_HELLO_TOP1: u32 = 271;
    const REAL_QWEN_FFN_DOWN_FIRST16: [f32; 16] = [
        0.002_591_61,
        -0.002_524_852_8,
        -0.010_199_547,
        0.001_312_494_3,
        0.000_033_378_6,
        -0.002_524_852_8,
        -0.001_245_737_1,
        -0.003_803_968_4,
        -0.002_524_852_8,
        0.003_870_725_6,
        0.006_428_957,
        0.000_033_378_6,
        -0.003_803_968_4,
        -0.002_524_852_8,
        -0.006_362_2,
        -0.010_199_547,
    ];
    const REAL_QWEN_FFN_DOWN_MATMUL4: [f32; 4] =
        [0.040_196_516, 0.008_409_955, -0.001_657_456_2, 0.021_694_81];

    fn assert_f32_slice_close(label: &str, got: &[f32], expected: &[f32], tolerance: f32) {
        assert_eq!(got.len(), expected.len(), "{label} length");
        for (idx, (&got, &want)) in got.iter().zip(expected.iter()).enumerate() {
            if tolerance == 0.0 {
                assert_eq!(
                    got.to_bits(),
                    want.to_bits(),
                    "{label}[{idx}] = {got:.8} vs {want:.8}"
                );
                continue;
            }
            let err = rel_err(got, want);
            assert!(
                err <= tolerance,
                "{label}[{idx}] = {got:.8} vs {want:.8}, rel_err = {err:.2e} > {tolerance:.2e}"
            );
        }
    }

    fn assert_f32_bytes_close(label: &str, got: &[u8], expected: &[f32], tolerance: f32) {
        let got_bits = bits_from_bytes(got);
        let got_f32: Vec<f32> = got_bits.iter().map(|&b| bits_to_f32(b)).collect();
        assert_eq!(got_f32.len(), expected.len(), "{label} length");
        for (idx, (&got, &want)) in got_f32.iter().zip(expected.iter()).enumerate() {
            let err = rel_err(got, want);
            assert!(
                err <= tolerance,
                "{label}[{idx}] = {got:.8} vs {want:.8}, rel_err = {err:.2e} > {tolerance:.2e}, bits = 0x{:08x}",
                got_bits[idx]
            );
        }
    }

    #[test]
    fn soft_f32_mul_correctly_rounded_sweep() {
        let test_pairs: Vec<(f32, f32)> = vec![
            (0.5, 0.8623),
            (-0.14797, 0.5),
            (0.021895, 0.5),
            (2.0, 3.0),
            (-1.5, 0.7),
            (1e-5, 1000.0),
            (f32::from_bits(0x3f31_7218), -7.0),
            (f32::from_bits(0x3fb8_aa3b), -5.0),
            (0.00138889, -0.14797),
            (0.008333, 0.021895),
            (0.00138889, 0.021895),
            (0.000694, -0.14797),
        ];
        for &(a, b) in &test_pairs {
            let a_bits = f32_to_bits(a);
            let b_bits = f32_to_bits(b);
            let result = soft_f32::mul(a_bits, b_bits);
            let result_f = bits_to_f32(result);
            let expected = a * b;
            let err = rel_err(result_f, expected);
            assert!(
                err <= REL_EPS,
                "mul({:.6}, {:.6}) = {:.10} vs {:.10}, rel_err = {:.2e}",
                a,
                b,
                result_f,
                expected,
                err
            );
        }
    }

    #[test]
    fn soft_f32_add_correctly_rounded_sweep() {
        let test_pairs: Vec<(f32, f32)> = vec![
            (0.5, 0.8623),
            (-0.14797, 0.008333),
            (1.0, 0.0109475),
            (0.0109475, 0.000287),
            (0.000287, 0.000003),
            (-5.0, 4.85203),
            (0.8623, -0.14797),
        ];
        for &(a, b) in &test_pairs {
            let a_bits = f32_to_bits(a);
            let b_bits = f32_to_bits(b);
            let result = soft_f32::add(a_bits, b_bits);
            let result_f = bits_to_f32(result);
            let expected = a + b;
            let err = rel_err(result_f, expected);
            assert!(
                err <= REL_EPS,
                "add({:.6}, {:.6}) = {:.10} vs {:.10}, rel_err = {:.2e}",
                a,
                b,
                result_f,
                expected,
                err
            );
        }
    }

    #[test]
    fn soft_f32_exp_poly_only_sweep() {
        let ln2_half = (2.0_f32.ln()) / 2.0;
        let step = ln2_half / 100.0;
        for i in 0..=200 {
            let r = -ln2_half + step * i as f32;
            let r_bits = f32_to_bits(r);
            let result = soft_f32::exp_poly(r_bits);
            let result_f = bits_to_f32(result);
            let expected = r.exp();
            let err = rel_err(result_f, expected);
            assert!(
                err <= REL_EPS,
                "exp_poly({:.6}) = {:.10} vs {:.10}, rel_err = {:.2e}",
                r,
                result_f,
                expected,
                err
            );
        }
    }

    #[test]
    fn soft_f32_exp_accuracy_sweep() {
        let test_values: Vec<f32> = vec![
            -5.0, -4.0, -3.0, -2.0, -1.5, -1.0, -0.5, -0.25, -0.1, -0.01, 0.0, 0.01, 0.1, 0.25,
            0.5, 0.75, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0,
        ];
        for &x in &test_values {
            let bits = f32_to_bits(x);
            let result = soft_f32::exp(bits);
            let result_f = bits_to_f32(result);
            let expected = x.exp();
            let err = rel_err(result_f, expected);
            assert!(
                err <= REL_EPS,
                "exp({:.4}) = {:.8} vs {:.8}, rel_err = {:.2e} > {:.2e}",
                x,
                result_f,
                expected,
                err,
                REL_EPS
            );
        }
    }

    #[test]
    fn soft_f32_sqrt_accuracy_sweep() {
        let test_values: Vec<f32> = vec![
            0.0,
            0.01,
            0.25,
            0.5,
            1.0,
            2.0,
            4.0,
            9.0,
            16.0,
            25.0,
            0.1,
            0.3,
            0.7,
            1.5,
            f32::from_bits(0x4048_f5c3),
            7.77,
            100.0,
            500.0,
        ];
        for &x in &test_values {
            let bits = f32_to_bits(x);
            let result = soft_f32::sqrt(bits);
            let result_f = bits_to_f32(result);
            let expected = x.sqrt();
            let err = rel_err(result_f, expected);
            assert!(
                err <= REL_EPS,
                "sqrt({:.4}) = {:.8} vs {:.8}, rel_err = {:.2e} > {:.2e}",
                x,
                result_f,
                expected,
                err,
                REL_EPS
            );
        }
    }

    #[test]
    fn soft_f32_tanh_accuracy_sweep() {
        let test_values: Vec<f32> = vec![
            -3.0, -2.0, -1.5, -1.0, -0.5, -0.25, -0.1, -0.01, 0.0, 0.01, 0.1, 0.25, 0.5, 1.0, 1.5,
            2.0, 3.0,
        ];
        for &x in &test_values {
            let bits = f32_to_bits(x);
            let result = soft_f32::tanh(bits);
            let result_f = bits_to_f32(result);
            let expected = x.tanh();
            let err = rel_err(result_f, expected);
            assert!(
                err <= REL_EPS,
                "tanh({:.4}) = {:.8} vs {:.8}, rel_err = {:.2e} > {:.2e}",
                x,
                result_f,
                expected,
                err,
                REL_EPS
            );
        }
    }

    #[test]
    fn soft_f32_div_accuracy_sweep() {
        let test_pairs: Vec<(f32, f32)> = vec![
            (1.0, 3.0),
            (7.0, 2.0),
            (0.5, 4.0),
            (-6.0, 3.0),
            (100.0, 7.0),
            (0.001, 0.01),
            (f32::from_bits(0x4048_f5c3), 2.71),
            (1e6, 1e3),
            (1e-3, 1e-6),
        ];
        for &(a, b) in &test_pairs {
            let a_bits = f32_to_bits(a);
            let b_bits = f32_to_bits(b);
            let result = soft_f32::div(a_bits, b_bits);
            let result_f = bits_to_f32(result);
            let expected = a / b;
            let err = rel_err(result_f, expected);
            assert!(
                err <= REL_EPS,
                "div({:.4}, {:.4}) = {:.8} vs {:.8}, rel_err = {:.2e} > {:.2e}",
                a,
                b,
                result_f,
                expected,
                err,
                REL_EPS
            );
        }
    }

    #[test]
    fn soft_f32_fmax_accuracy() {
        assert_eq!(
            soft_f32::fmax(f32_to_bits(1.0), f32_to_bits(2.0)),
            f32_to_bits(2.0)
        );
        assert_eq!(
            soft_f32::fmax(f32_to_bits(2.0), f32_to_bits(1.0)),
            f32_to_bits(2.0)
        );
        assert_eq!(
            soft_f32::fmax(f32_to_bits(-1.0), f32_to_bits(-2.0)),
            f32_to_bits(-1.0)
        );
        assert_eq!(
            soft_f32::fmax(f32_to_bits(-3.0), f32_to_bits(5.0)),
            f32_to_bits(5.0)
        );
    }

    #[test]
    fn softmax_accuracy_vs_host() {
        let input: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let input_bits: Vec<u32> = input.iter().map(|&x| f32_to_bits(x)).collect();
        let input_bytes = bytes_from_bits(&input_bits);
        let shape = shape2(1, 5);
        let mut out = vec![0u8; input.len() * 4];

        ops::softmax(&view(&shape, &input_bytes), &mut out).unwrap();

        let out_bits = bits_from_bytes(&out);
        let out_f32: Vec<f32> = out_bits.iter().map(|&b| bits_to_f32(b)).collect();

        let host_max = *input
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        let exps: Vec<f32> = input.iter().map(|&x| (x - host_max).exp()).collect();
        let sum: f32 = exps.iter().sum();
        let expected: Vec<f32> = exps.iter().map(|&e| e / sum).collect();

        for i in 0..input.len() {
            let err = rel_err(out_f32[i], expected[i]);
            assert!(
                err <= REL_EPS,
                "softmax[{}] = {:.8} vs {:.8}, rel_err = {:.2e} > {:.2e}",
                i,
                out_f32[i],
                expected[i],
                err,
                REL_EPS
            );
        }
    }

    #[test]
    fn gelu_accuracy_vs_host() {
        let input: Vec<f32> = vec![-2.0, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0];
        let input_bits: Vec<u32> = input.iter().map(|&x| f32_to_bits(x)).collect();
        let input_bytes = bytes_from_bits(&input_bits);
        let shape = shape2(1, input.len() as u64);
        let mut out = vec![0u8; input.len() * 4];

        ops::gelu(&view(&shape, &input_bytes), &mut out).unwrap();

        let out_bits = bits_from_bytes(&out);
        let out_f32: Vec<f32> = out_bits.iter().map(|&b| bits_to_f32(b)).collect();

        for i in 0..input.len() {
            let x = input[i];
            let sqrt2pi = (2.0_f32 / std::f32::consts::PI).sqrt();
            let expected = x * 0.5 * (1.0 + (sqrt2pi * (x + 0.044715 * x.powi(3))).tanh());
            let err = rel_err(out_f32[i], expected);
            assert!(
                err <= REL_EPS,
                "gelu({:.4}) = {:.8} vs {:.8}, rel_err = {:.2e} > {:.2e}",
                x,
                out_f32[i],
                expected,
                err,
                REL_EPS
            );
        }
    }

    #[test]
    fn layer_norm_accuracy_vs_host() {
        let input: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let eps = 1e-5;
        let mean: f32 = input.iter().sum::<f32>() / input.len() as f32;
        let var: f32 = input.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / input.len() as f32;
        let std = (var + eps).sqrt();
        let expected: Vec<f32> = input.iter().map(|x| (x - mean) / std).collect();

        let input_bits: Vec<u32> = input.iter().map(|&x| f32_to_bits(x)).collect();
        let input_bytes = bytes_from_bits(&input_bits);
        let shape = shape2(1, input.len() as u64);
        let mut out = vec![0u8; input.len() * 4];

        ops::layer_norm(&view(&shape, &input_bytes), &mut out).unwrap();

        let out_bits = bits_from_bytes(&out);
        let out_f32: Vec<f32> = out_bits.iter().map(|&b| bits_to_f32(b)).collect();

        for i in 0..input.len() {
            let err = rel_err(out_f32[i], expected[i]);
            assert!(
                err <= REL_EPS,
                "layer_norm[{}] = {:.8} vs {:.8}, rel_err = {:.2e} > {:.2e}",
                i,
                out_f32[i],
                expected[i],
                err,
                REL_EPS
            );
        }
    }

    #[test]
    fn rms_norm_accuracy_vs_host() {
        let input: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let eps = 1e-5;
        let rms_sq: f32 = input.iter().map(|x| x * x).sum::<f32>() / input.len() as f32;
        let rms = (rms_sq + eps).sqrt();
        let expected: Vec<f32> = input.iter().map(|x| x / rms).collect();

        let input_bits: Vec<u32> = input.iter().map(|&x| f32_to_bits(x)).collect();
        let input_bytes = bytes_from_bits(&input_bits);
        let shape = shape2(1, input.len() as u64);
        let mut out = vec![0u8; input.len() * 4];

        ops::rms_norm(&view(&shape, &input_bytes), &mut out).unwrap();

        let out_bits = bits_from_bytes(&out);
        let out_f32: Vec<f32> = out_bits.iter().map(|&b| bits_to_f32(b)).collect();

        for i in 0..input.len() {
            let err = rel_err(out_f32[i], expected[i]);
            assert!(
                err <= REL_EPS,
                "rms_norm[{}] = {:.8} vs {:.8}, rel_err = {:.2e} > {:.2e}",
                i,
                out_f32[i],
                expected[i],
                err,
                REL_EPS
            );
        }
    }

    #[test]
    fn soft_f32_exp_determinism() {
        let inputs = [f32_to_bits(0.5), f32_to_bits(-1.3), f32_to_bits(2.7)];
        for bits in &inputs {
            let r1 = soft_f32::exp(*bits);
            let r2 = soft_f32::exp(*bits);
            assert_eq!(r1, r2, "exp not deterministic for input 0x{:08x}", bits);
        }
    }

    #[test]
    fn soft_f32_sqrt_determinism() {
        let inputs = [f32_to_bits(2.0), f32_to_bits(0.5), f32_to_bits(100.0)];
        for bits in &inputs {
            let r1 = soft_f32::sqrt(*bits);
            let r2 = soft_f32::sqrt(*bits);
            assert_eq!(r1, r2, "sqrt not deterministic for input 0x{:08x}", bits);
        }
    }

    #[test]
    fn soft_f32_tanh_determinism() {
        let inputs = [f32_to_bits(0.5), f32_to_bits(-1.0), f32_to_bits(2.0)];
        for bits in &inputs {
            let r1 = soft_f32::tanh(*bits);
            let r2 = soft_f32::tanh(*bits);
            assert_eq!(r1, r2, "tanh not deterministic for input 0x{:08x}", bits);
        }
    }

    #[test]
    fn fb_console_layout_scrolls_and_backspaces() {
        use super::fb_console_logic::{ConsoleCursor, ConsoleSink};

        #[derive(Default)]
        struct Sink {
            ops: Vec<String>,
        }

        impl ConsoleSink for Sink {
            fn put(&mut self, row: usize, col: usize, byte: u8) {
                self.ops.push(format!("put:{row}:{col}:{}", byte as char));
            }

            fn clear(&mut self, row: usize, col: usize) {
                self.ops.push(format!("clear:{row}:{col}"));
            }

            fn scroll(&mut self) {
                self.ops.push("scroll".to_string());
            }
        }

        let mut cursor = ConsoleCursor::new(4, 2);
        let mut sink = Sink::default();
        for byte in b"abcd" {
            cursor.write_byte(*byte, &mut sink);
        }
        assert_eq!(cursor.position(), (1, 0));
        cursor.write_byte(0x08, &mut sink);
        assert_eq!(cursor.position(), (0, 3));
        assert!(sink.ops.contains(&"clear:0:3".to_string()));
        cursor.write_byte(b'Z', &mut sink);
        assert_eq!(cursor.position(), (1, 0));

        sink.ops.clear();
        for byte in b"12345678" {
            cursor.write_byte(*byte, &mut sink);
        }
        assert!(sink.ops.iter().any(|op| op == "scroll"));
        assert_eq!(cursor.position(), (1, 0));
    }

    #[test]
    fn usb_hid_boot_keyboard_ascii_oracles() {
        use super::usb_hid_logic::{BootKeyboardDecoder, keycode_to_ascii};

        assert_eq!(keycode_to_ascii(0x04, false), Some(b'a'));
        assert_eq!(keycode_to_ascii(0x04, true), Some(b'A'));
        assert_eq!(keycode_to_ascii(0x1e, false), Some(b'1'));
        assert_eq!(keycode_to_ascii(0x1e, true), Some(b'!'));
        assert_eq!(keycode_to_ascii(0x28, false), Some(b'\n'));
        assert_eq!(keycode_to_ascii(0x2a, false), Some(0x08));
        assert_eq!(keycode_to_ascii(0x37, true), Some(b'>'));
        assert_eq!(keycode_to_ascii(0, false), None);

        let mut decoder = BootKeyboardDecoder::new();
        let mut out = [0u8; 8];
        assert_eq!(
            decoder.decode_report(&[0, 0, 0x08, 0, 0, 0, 0, 0], &mut out),
            1
        );
        assert_eq!(out[0], b'e');
        assert_eq!(
            decoder.decode_report(&[0, 0, 0x08, 0, 0, 0, 0, 0], &mut out),
            0
        );
        assert_eq!(
            decoder.decode_report(&[0, 0, 0, 0, 0, 0, 0, 0], &mut out),
            0
        );
        assert_eq!(
            decoder.decode_report(&[0, 0, 0x1b, 0x0c, 0, 0, 0, 0], &mut out),
            2
        );
        assert_eq!(&out[..2], b"xi");
    }

    #[test]
    fn atrium_input_event_byte_oracles() {
        use super::atrium_input_logic::{InputEventV1, decode_event, encode_event};

        // Hand-laid independently: kind, pressed, LE keycode, modifiers, pad.
        let key_fixture = [0x01, 0x01, 0x04, 0x00, 0x22, 0x00, 0x00, 0x00];
        let key = InputEventV1::key(0x04, 0x22, true);
        assert_eq!(encode_event(key), key_fixture);
        assert_eq!(decode_event(key_fixture), key);

        // Pointer: kind, buttons, reserved code, signed LE dx/dy.
        let pointer_fixture = [0x02, 0x05, 0x00, 0x00, 0xfb, 0xff, 0x07, 0x00];
        let pointer = InputEventV1::pointer(-5, 7, 0x05);
        assert_eq!(encode_event(pointer), pointer_fixture);
        assert_eq!(decode_event(pointer_fixture), pointer);
    }

    #[test]
    fn atrium_hid_kill_switch_preserves_direct_keyboard_bytes() {
        use super::atrium_input_logic::routing_enabled;
        use super::usb_hid_logic::BootKeyboardDecoder;

        // Feature compiled IN + runtime OFF must select the exact legacy ASCII
        // decoder/sink. The same hand report produces the same fd-0 byte.
        assert!(!routing_enabled(true, false));
        let report = [0x02, 0, 0x04, 0, 0, 0, 0, 0];
        let mut baseline = BootKeyboardDecoder::new();
        let mut switched_off = BootKeyboardDecoder::new();
        let mut a = [0u8; 8];
        let mut b = [0u8; 8];
        let an = baseline.decode_report(&report, &mut a);
        let bn = switched_off.decode_report(&report, &mut b);
        assert_eq!(an, 1);
        assert_eq!(bn, an);
        assert_eq!(&b[..bn], &a[..an]);
        assert_eq!(b[0], b'A');
    }

    #[test]
    fn usb_hid_key_transition_oracle() {
        use super::usb_hid_logic::{BootKeyEventDecoder, KeyEvent};
        let mut decoder = BootKeyEventDecoder::new();
        let mut out = [KeyEvent {
            keycode: 0,
            modifiers: 0,
            pressed: false,
        }; 12];
        assert_eq!(
            decoder.decode_report(&[2, 0, 4, 0, 0, 0, 0, 0], &mut out),
            1
        );
        assert_eq!(
            out[0],
            KeyEvent {
                keycode: 4,
                modifiers: 2,
                pressed: true
            }
        );
        assert_eq!(
            decoder.decode_report(&[0, 0, 0, 0, 0, 0, 0, 0], &mut out),
            1
        );
        assert_eq!(
            out[0],
            KeyEvent {
                keycode: 4,
                modifiers: 0,
                pressed: false
            }
        );
    }

    #[test]
    fn usb_parse_hid_boot_keyboard_topology() {
        use super::usb_parse::{find_hid_boot_keyboard, parse_configuration_set};

        #[rustfmt::skip]
        let bytes: [u8; 34] = [
            9, 0x02, 34, 0, 1, 1, 0, 0xA0, 50,
            // HID boot keyboard interface: class=3, subclass=1, protocol=1.
            9, 0x04, 2, 0, 1, 0x03, 0x01, 0x01, 0,
            // HID descriptor, intentionally skipped by Phase 8b.
            9, 0x21, 0x11, 0x01, 0, 1, 0x22, 63, 0,
            7, 0x05, 0x81, 0x03, 8, 0, 10,
        ];
        let config = parse_configuration_set(&bytes);
        let topo = find_hid_boot_keyboard(core::slice::from_ref(&config))
            .expect("boot keyboard topology found");
        assert_eq!(topo.config_value, 1);
        assert_eq!(topo.interface, 2);
        assert_eq!(topo.int_in.address, 0x81);
        assert_eq!(topo.int_in.max_packet, 8);

        let mouse = parse_configuration_set(&[
            9, 0x02, 25, 0, 1, 1, 0, 0xA0, 50, 9, 0x04, 0, 0, 1, 0x03, 0x01, 0x02, 0, 7, 0x05,
            0x81, 0x03, 8, 0, 10,
        ]);
        assert!(find_hid_boot_keyboard(core::slice::from_ref(&mouse)).is_none());
    }

    /// HAND-LAID USB HID boot-protocol MOUSE report byte oracle (USB HID 1.11
    /// §B.2), asserted independently of the decoder: byte 0 = button bitmap
    /// (bit0 L / bit1 R / bit2 M), byte 1 = signed dX (i8), byte 2 = signed dY
    /// (i8), byte 3 = signed wheel (i8, the common extension). The dispatch's
    /// worked vector [0x01, 0x05, 0xFB, 0x02] → left down, dX=+5, dY=−5, wheel=+2
    /// is the anchor; the negative-delta and short-report cases cover the two
    /// documented traps (the i8-sign trap and the variable-length trap).
    #[test]
    fn usb_hid_boot_mouse_report_oracle() {
        use super::usb_hid_logic::{
            BootMouseDecoder, MOUSE_BTN_LEFT, MOUSE_BTN_MIDDLE, MOUSE_BTN_RIGHT, PointerEvent,
        };

        let mut decoder = BootMouseDecoder::new();

        // The dispatch's anchor vector: left-button down, dX=+5, dY=−5, wheel=+2.
        assert_eq!(
            decoder.decode_report(&[0x01, 0x05, 0xFB, 0x02]),
            Some(PointerEvent {
                buttons: MOUSE_BTN_LEFT,
                dx: 5,
                dy: -5,
                wheel: 2,
            })
        );
        // Signed-delta trap: 0xFF is −1, not 255 (buttons stayed 0x01 → a change
        // vs the previous 0x01? no — but motion is non-zero, so it emits).
        assert_eq!(
            decoder.decode_report(&[0x01, 0xFF, 0xFF, 0x00]),
            Some(PointerEvent {
                buttons: MOUSE_BTN_LEFT,
                dx: -1,
                dy: -1,
                wheel: 0,
            })
        );
        // Variable-length trap: a 3-byte report (no wheel byte) decodes wheel=0.
        // Right button, dX=+10.
        assert_eq!(
            decoder.decode_report(&[0x02, 0x0A, 0x00]),
            Some(PointerEvent {
                buttons: MOUSE_BTN_RIGHT,
                dx: 10,
                dy: 0,
                wheel: 0,
            })
        );
        // Wheel-only report (no motion, middle button change) still emits.
        assert_eq!(
            decoder.decode_report(&[0x04, 0x00, 0x00, 0x03]),
            Some(PointerEvent {
                buttons: MOUSE_BTN_MIDDLE,
                dx: 0,
                dy: 0,
                wheel: 3,
            })
        );
        // Idle-repeat suppression: same buttons (0x04), zero motion, zero wheel
        // → NOT an event (the all-zero-movement trap).
        assert_eq!(decoder.decode_report(&[0x04, 0x00, 0x00, 0x00]), None);
        // A pure button *release* (buttons change to 0, zero motion) IS an event.
        assert_eq!(
            decoder.decode_report(&[0x00, 0x00, 0x00, 0x00]),
            Some(PointerEvent {
                buttons: 0,
                dx: 0,
                dy: 0,
                wheel: 0,
            })
        );
        // A too-short report (< 3 bytes) is never decoded.
        assert_eq!(decoder.decode_report(&[0x01, 0x05]), None);
    }

    #[test]
    fn usb_parse_hid_boot_mouse_topology() {
        use super::usb_parse::{
            find_hid_boot_keyboard, find_hid_boot_mouse, parse_configuration_set,
        };

        // Boot mouse: class=3, subclass=1, protocol=2, one interrupt-IN endpoint.
        let mouse = parse_configuration_set(&[
            9, 0x02, 25, 0, 1, 1, 0, 0xA0, 50, 9, 0x04, 3, 0, 1, 0x03, 0x01, 0x02, 0, 7, 0x05,
            0x81, 0x03, 4, 0, 10,
        ]);
        let topo =
            find_hid_boot_mouse(core::slice::from_ref(&mouse)).expect("boot mouse topology found");
        assert_eq!(topo.config_value, 1);
        assert_eq!(topo.interface, 3);
        assert_eq!(topo.int_in.address, 0x81);
        assert_eq!(topo.int_in.max_packet, 4);

        // Cross-checks: the mouse finder rejects a keyboard, and vice-versa (the
        // composite-device protocol-byte discrimination).
        assert!(find_hid_boot_mouse(core::slice::from_ref(&mouse)).is_some());
        assert!(find_hid_boot_keyboard(core::slice::from_ref(&mouse)).is_none());
        let keyboard = parse_configuration_set(&[
            9, 0x02, 25, 0, 1, 1, 0, 0xA0, 50, 9, 0x04, 0, 0, 1, 0x03, 0x01, 0x01, 0, 7, 0x05,
            0x81, 0x03, 8, 0, 10,
        ]);
        assert!(find_hid_boot_mouse(core::slice::from_ref(&keyboard)).is_none());
    }

    /// Phase 7.5 BUILD-B (ISC-1a): the HAND-LAID CDC-ECM configuration
    /// descriptor byte oracle — every offset computed by hand from USB 2.0
    /// §9.6 + ECM 1.2 §5.4, independent of any writer (the ADR-028 lesson:
    /// a parser tested only against its own writer agrees with shared
    /// mistakes). Layout: config(9) + comm IF(9) + ECM functional(13) +
    /// int EP(7) + data IF alt0(9, no EPs) + data IF alt1(9) + bulk IN(7)
    /// + bulk OUT(7) = wTotalLength 70.
    #[test]
    fn usb_parse_ecm_byte_oracle() {
        use super::usb_parse::{find_ecm, parse_configuration_set};

        #[rustfmt::skip]
        let bytes: [u8; 70] = [
            // CONFIGURATION: len=9 type=2 wTotalLength=70 numIf=2 value=2
            9, 0x02, 70, 0, 2, 2, 0, 0xA0, 50,
            // INTERFACE 0 alt 0: class 02 (CDC comm), subclass 06 (ECM)
            9, 0x04, 0, 0, 1, 0x02, 0x06, 0x00, 0,
            // CS_INTERFACE Ethernet Networking (0x0F): iMACAddress=3,
            // bmEthernetStatistics=0, wMaxSegmentSize=1514 (0xEA 0x05)
            13, 0x24, 0x0F, 3, 0, 0, 0, 0, 0xEA, 0x05, 0, 0, 0,
            // ENDPOINT 0x81 interrupt (attrs 3), MPS=16, interval=8
            7, 0x05, 0x81, 0x03, 16, 0, 8,
            // INTERFACE 1 alt 0: class 0A (CDC data), NO endpoints
            9, 0x04, 1, 0, 0, 0x0A, 0x00, 0x00, 0,
            // INTERFACE 1 alt 1: class 0A, 2 endpoints (THE ECM trap)
            9, 0x04, 1, 1, 2, 0x0A, 0x00, 0x00, 0,
            // ENDPOINT 0x82 bulk IN, MPS=512 (0x00 0x02)
            7, 0x05, 0x82, 0x02, 0x00, 0x02, 0,
            // ENDPOINT 0x03 bulk OUT, MPS=512
            7, 0x05, 0x03, 0x02, 0x00, 0x02, 0,
        ];
        let config = parse_configuration_set(&bytes);
        assert_eq!(config.value, 2);
        assert_eq!(config.interfaces.len(), 3); // comm, data alt0, data alt1
        let comm = &config.interfaces[0];
        assert_eq!(
            (comm.number, comm.alt_setting, comm.class, comm.subclass),
            (0, 0, 0x02, 0x06)
        );
        let ecm = comm.ecm.expect("ECM functional descriptor parsed");
        assert_eq!(ecm.mac_string_index, 3);
        assert_eq!(ecm.max_segment_size, 1514);
        assert_eq!(comm.endpoints.len(), 1);
        assert!(comm.endpoints[0].is_interrupt() && comm.endpoints[0].is_in());
        assert_eq!(comm.endpoints[0].max_packet, 16);
        let alt0 = &config.interfaces[1];
        assert_eq!(
            (alt0.number, alt0.alt_setting, alt0.endpoints.len()),
            (1, 0, 0)
        );
        let alt1 = &config.interfaces[2];
        assert_eq!(
            (alt1.number, alt1.alt_setting, alt1.endpoints.len()),
            (1, 1, 2)
        );

        // The topology finder: selects config 2, data alt 1, the bulk pair.
        let topo = find_ecm(core::slice::from_ref(&config)).expect("ECM function found");
        assert_eq!(topo.config_value, 2);
        assert_eq!(topo.comm_interface, 0);
        assert_eq!(topo.data_interface, 1);
        assert_eq!(topo.data_alt_setting, 1); // alt 0 (no EPs) must NOT win
        assert_eq!(topo.bulk_in.address, 0x82);
        assert_eq!(topo.bulk_out.address, 0x03);
        assert_eq!(topo.int_ep.address, 0x81);
        assert_eq!(topo.max_segment_size, 1514);
        assert_eq!(topo.mac_string_index, 3);
    }

    /// BUILD-B (ISC-1/ISC-6): parser totality + selection-by-content truth
    /// table — truncated sets, zero-length descriptors, RNDIS-first
    /// dual-config devices, and non-ECM devices (the E1 verdict).
    #[test]
    fn usb_parse_truth_tables() {
        use super::usb_parse::{
            EcmNotification, find_ecm, mac_from_string_descriptor, parse_configuration_set,
            parse_notification,
        };

        // Totality: empty, runt, zero-length descriptor, truncated tail.
        assert_eq!(parse_configuration_set(&[]).interfaces.len(), 0);
        assert_eq!(parse_configuration_set(&[9]).interfaces.len(), 0);
        assert_eq!(
            parse_configuration_set(&[0, 0x04, 1, 1]).interfaces.len(),
            0
        );
        let truncated = [9u8, 0x04, 0, 0, 1, 0x02, 0x06, 0x00, 0, 7, 0x05, 0x81];
        let parsed = parse_configuration_set(&truncated);
        assert_eq!(parsed.interfaces.len(), 1); // the truncated EP is dropped
        assert_eq!(parsed.interfaces[0].endpoints.len(), 0);

        // E1: a vendor-class device has no ECM function.
        let vendor = parse_configuration_set(&[
            9, 0x02, 18, 0, 1, 1, 0, 0xA0, 50, 9, 0x04, 0, 0, 2, 0xFF, 0x00, 0x00, 0,
        ]);
        assert!(find_ecm(core::slice::from_ref(&vendor)).is_none());

        // Dual-config: config 1 is RNDIS-shaped (class 02/02), config 2 is
        // ECM — selection BY CONTENT picks config 2 with zero RNDIS code.
        let rndis_cfg = parse_configuration_set(&[
            9, 0x02, 18, 0, 1, 1, 0, 0xA0, 50, 9, 0x04, 0, 0, 1, 0x02, 0x02, 0xFF, 0,
        ]);
        let ecm_cfg = parse_configuration_set(&[
            9, 0x02, 53, 0, 2, 2, 0, 0xA0, 50, 9, 0x04, 0, 0, 1, 0x02, 0x06, 0x00, 0, 13, 0x24,
            0x0F, 5, 0, 0, 0, 0, 0xEA, 0x05, 0, 0, 0, 7, 0x05, 0x81, 0x03, 16, 0, 8, 9, 0x04, 1, 1,
            2, 0x0A, 0x00, 0x00, 0, 7, 0x05, 0x82, 0x02, 0x00, 0x02, 0, 7, 0x05, 0x03, 0x02, 0x00,
            0x02, 0,
        ]);
        let topo = find_ecm(&[rndis_cfg, ecm_cfg]).expect("ECM config found by content");
        assert_eq!(topo.config_value, 2);

        // Notifications (CDC §6.3): NetworkConnection up/down, speed
        // change, unknown (counted, never fatal), runt (None).
        assert_eq!(
            parse_notification(&[0xA1, 0x00, 1, 0, 0, 0, 0, 0]),
            Some(EcmNotification::NetworkConnection { up: true })
        );
        assert_eq!(
            parse_notification(&[0xA1, 0x00, 0, 0, 0, 0, 0, 0]),
            Some(EcmNotification::NetworkConnection { up: false })
        );
        assert_eq!(
            parse_notification(&[0xA1, 0x2A, 0, 0, 0, 0, 8, 0]),
            Some(EcmNotification::ConnectionSpeedChange)
        );
        assert_eq!(
            parse_notification(&[0xA1, 0x77, 0, 0, 0, 0, 0, 0]),
            Some(EcmNotification::Other(0x77))
        );
        assert_eq!(parse_notification(&[0xA1, 0x00, 1, 0]), None);
        assert_eq!(parse_notification(&[0x21, 0x00, 1, 0, 0, 0, 0, 0]), None);

        // MAC string descriptor (ECM §5.4: 12 UTF-16LE hex digits).
        let mut mac_desc = [0u8; 26];
        mac_desc[0] = 26;
        mac_desc[1] = 0x03;
        for (i, c) in b"40A1B2C3D4E5".iter().enumerate() {
            mac_desc[2 + i * 2] = *c;
        }
        assert_eq!(
            mac_from_string_descriptor(&mac_desc),
            Some([0x40, 0xA1, 0xB2, 0xC3, 0xD4, 0xE5])
        );
        // Lowercase accepted; wrong type, runt, non-hex refused.
        let mut lower = mac_desc;
        for (i, c) in b"40a1b2c3d4e5".iter().enumerate() {
            lower[2 + i * 2] = *c;
        }
        assert!(mac_from_string_descriptor(&lower).is_some());
        let mut wrong_type = mac_desc;
        wrong_type[1] = 0x02;
        assert_eq!(mac_from_string_descriptor(&wrong_type), None);
        assert_eq!(mac_from_string_descriptor(&mac_desc[..10]), None);
        let mut non_hex = mac_desc;
        non_hex[2] = b'G';
        assert_eq!(mac_from_string_descriptor(&non_hex), None);
        // Non-Latin code unit (high byte set) refused.
        let mut wide = mac_desc;
        wide[3] = 0x04;
        assert_eq!(mac_from_string_descriptor(&wide), None);

        // SuperSpeed Endpoint Companion (USB 3.2 §9.6.7, Forge M1): a
        // type-0x30 descriptor following an endpoint attaches bMaxBurst to
        // THAT endpoint; endpoints without a companion stay at 0 (FS/HS).
        #[rustfmt::skip]
        let ss = parse_configuration_set(&[
            9, 0x02, 40, 0, 1, 1, 0, 0xA0, 50,
            9, 0x04, 0, 0, 2, 0x0A, 0x00, 0x00, 0,
            // bulk IN, MPS=1024, then its companion with bMaxBurst=3
            7, 0x05, 0x82, 0x02, 0x00, 0x04, 0,
            6, 0x30, 3, 0, 0, 0,
            // bulk OUT with NO companion
            7, 0x05, 0x03, 0x02, 0x00, 0x04, 0,
        ]);
        let eps = &ss.interfaces[0].endpoints;
        assert_eq!(eps.len(), 2);
        assert_eq!((eps[0].address, eps[0].max_burst), (0x82, 3));
        assert_eq!((eps[1].address, eps[1].max_burst), (0x03, 0));
    }

    /// BUILD-B (ISC-3): DCI math, EP-type codes, and the periodic-interval
    /// conversion (xHCI §4.5.1, table 6-9, §6.2.3.6) — the EP-context
    /// arithmetic under a truth table before any blind run.
    #[test]
    fn xhci_endpoint_context_arithmetic() {
        use super::xhci_ring_logic::{
            Trb, TrbType, control_for, dci, endpoint_interval, ep_type_code,
        };

        // DCI: EP0 → 1; EPn OUT → 2n; EPn IN → 2n+1.
        assert_eq!(dci(0, false), 1);
        assert_eq!(dci(0, true), 1);
        assert_eq!(dci(1, false), 2);
        assert_eq!(dci(1, true), 3);
        assert_eq!(dci(2, true), 5);
        assert_eq!(dci(15, true), 31);

        // EP types: bulk OUT 2 / IN 6, interrupt OUT 3 / IN 7, isoch 1/5,
        // control 4 either direction.
        assert_eq!(ep_type_code(0x02, false), 2);
        assert_eq!(ep_type_code(0x02, true), 6);
        assert_eq!(ep_type_code(0x03, false), 3);
        assert_eq!(ep_type_code(0x03, true), 7);
        assert_eq!(ep_type_code(0x01, false), 1);
        assert_eq!(ep_type_code(0x01, true), 5);
        assert_eq!(ep_type_code(0x00, false), 4);
        assert_eq!(ep_type_code(0x00, true), 4);

        // Intervals: bulk is always 0; HS/SS interrupt bInterval is an
        // exponent (1..16 → 0..15, clamped); FS interrupt bInterval is in
        // ms (1 ms = 2^3 × 125 µs → 3 + floor(log2(ms))).
        assert_eq!(endpoint_interval(true, 4, 0x02), 0);
        assert_eq!(endpoint_interval(true, 1, 0x03), 0);
        assert_eq!(endpoint_interval(true, 4, 0x03), 3);
        assert_eq!(endpoint_interval(true, 16, 0x03), 15);
        assert_eq!(endpoint_interval(true, 0, 0x03), 0); // clamped up
        assert_eq!(endpoint_interval(true, 200, 0x03), 15); // clamped down
        assert_eq!(endpoint_interval(false, 1, 0x03), 3); // 1 ms
        assert_eq!(endpoint_interval(false, 8, 0x03), 6); // 8 ms
        assert_eq!(endpoint_interval(false, 255, 0x03), 10); // 128 ms ≤ 255
        assert_eq!(endpoint_interval(false, 0, 0x03), 3); // clamped up

        // Transfer Event field extraction (§6.4.2.1 byte-oracle style):
        // slot in control[31:24], DCI in control[20:16], residue in
        // status[23:0], completion code in status[31:24].
        let event = Trb {
            parameter: 0x1000,
            status: (13 << 24) | 534, // Short Packet, 534 bytes NOT transferred
            control: control_for(TrbType::TransferEvent, true, (7 << 24) | (3 << 16)),
        };
        assert_eq!(event.slot_id(), 7);
        assert_eq!(event.ep_id(), 3);
        assert_eq!(event.completion_code(), 13);
        assert_eq!(event.transfer_residue(), 534);
        // A 2048-byte post completing short with residue 534 carried 1514.
        assert_eq!(2048 - event.transfer_residue() as usize, 1514);
    }
}

// ─────────────────────────── BUILD-LO: the latency oracle (Tier A) ───────────────────────────
// The §4 (docs/complexity-governance.md) hot-path latency oracle, host tier: the
// PURE, kernel-free hot-path functions measured in isolation with rdtsc cycles
// (median-of-N over a batched loop, warmed up). These give portable, deterministic
// SHAPE proofs (decide is O(devices); accounting is independent of tensor *size*)
// plus machine-noted cycles/iter for the §5 record. The absolute numbers are
// machine-specific; only the SHAPE assertions are enforced (generous ceilings that
// exclude super-linearity without flaking on host scheduler jitter). Run with
// `cargo test -p ai-ops-tests latency_oracle -- --nocapture` to capture the numbers.
#[cfg(all(test, target_arch = "x86_64"))]
mod latency_oracle_host_bench {
    use super::arena_logic::{SlotKey, bump_generation, key_matches};
    use super::placement_policy::{DeviceCandidate, decide};
    use super::sharing_logic::{SharedRecord, attributed_resident_bytes, total_resident_bytes};
    use std::hint::black_box;

    /// `lfence; rdtsc` — serialized cycle read (the kernel Tier-B idiom, on the host).
    fn host_rdtsc() -> u64 {
        // SAFETY: `_mm_lfence` + `_rdtsc` only read the time-stamp counter; no
        // memory or stack effects. Baseline x86_64 — no target-feature needed.
        unsafe {
            core::arch::x86_64::_mm_lfence();
            core::arch::x86_64::_rdtsc()
        }
    }

    /// Median cycles/iter of `f`, amortizing rdtsc overhead over a `batch`-call
    /// inner loop and taking the median across `samples` outer reads (robust to
    /// host scheduler jitter). Warms up before timing.
    fn host_bench<F: FnMut()>(samples: usize, batch: usize, mut f: F) -> u64 {
        for _ in 0..200 {
            for _ in 0..batch {
                f();
            }
        }
        let mut runs = vec![0u64; samples];
        for r in runs.iter_mut() {
            let start = host_rdtsc();
            for _ in 0..batch {
                f();
            }
            let end = host_rdtsc();
            *r = end.wrapping_sub(start) / batch as u64;
        }
        runs.sort_unstable();
        runs[samples / 2]
    }

    fn make_candidates(n: usize) -> Vec<DeviceCandidate> {
        // n accelerators (none over quota, varied residency) + one CPU fallback —
        // every candidate selectable, so `decide` walks the whole set (worst case).
        let mut v = Vec::with_capacity(n + 1);
        v.push(DeviceCandidate {
            device_abi: 0,
            resident_bytes: 0,
            covers_op: true,
            over_quota: false,
            is_cpu_fallback: true,
        });
        for i in 0..n {
            v.push(DeviceCandidate {
                device_abi: (i as u32) + 1,
                resident_bytes: (i as u64) * 64,
                covers_op: true,
                over_quota: false,
                is_cpu_fallback: false,
            });
        }
        v
    }

    /// ISC-1: `placement_policy::decide` is O(devices) — sweep the candidate count
    /// and show the cost grows linearly (not super-linearly). Records cycles/iter.
    /// `decide` does no allocation (it is `#![no_std]`-clean over a `&[_]` slice,
    /// returning a `Copy` `Placement` — verified by inspection / the kernel-free mod).
    ///
    /// The shape check fits the log-log slope of cycles against device count over
    /// the sizes where per-device cost dominates (>= 16). Linear predicts ~1,
    /// quadratic ~2; the assert sits at 1.5. Each size keeps its fastest of
    /// `ROUNDS` interleaved measurements: host noise only ever adds cycles, and
    /// interleaving spreads a slow stretch over every size instead of one. The
    /// previous single-ratio form (`cyc(128) < 20 * cyc(8)`) failed on hosted CI
    /// whenever one of its two points caught a noise spike.
    #[test]
    fn lo_isc1_decide_is_o_devices() {
        const ROUNDS: usize = 5;
        const FIT_FROM: usize = 16;
        const MAX_SLOPE: f64 = 1.5;
        let counts = [1usize, 2, 4, 8, 16, 32, 64, 128, 256];
        let cands: Vec<_> = counts.iter().map(|&n| make_candidates(n)).collect();
        let mut cyc = [u64::MAX; 9];
        for _ in 0..ROUNDS {
            for (idx, &n) in counts.iter().enumerate() {
                let total = (n as u64) * 64;
                let c = host_bench(64, 256, || {
                    let p = decide(black_box(&cands[idx]), black_box(total));
                    black_box(&p);
                });
                cyc[idx] = cyc[idx].min(c);
            }
        }
        for (&n, &c) in counts.iter().zip(cyc.iter()) {
            println!(
                "NEO_TEST/1 event=measure name=ai_latency_oracle tier=A op=decide devices={n} cycles_per_iter={c}"
            );
        }

        // Least-squares slope of ln(cycles) on ln(devices).
        let points: Vec<(f64, f64)> = counts
            .iter()
            .zip(cyc.iter())
            .filter(|&(&n, _)| n >= FIT_FROM)
            .map(|(&n, &c)| ((n as f64).ln(), (c.max(1) as f64).ln()))
            .collect();
        let len = points.len() as f64;
        let mean_x = points.iter().map(|p| p.0).sum::<f64>() / len;
        let mean_y = points.iter().map(|p| p.1).sum::<f64>() / len;
        let cov: f64 = points.iter().map(|p| (p.0 - mean_x) * (p.1 - mean_y)).sum();
        let var: f64 = points.iter().map(|p| (p.0 - mean_x).powi(2)).sum();
        let slope = cov / var;
        println!(
            "NEO_TEST/1 event=measure name=ai_latency_oracle tier=A op=decide log_log_slope={slope:.3}"
        );
        assert!(
            slope < MAX_SLOPE,
            "decide super-linear in device count: log-log slope {slope:.3} >= {MAX_SLOPE} \
             over devices >= {FIT_FROM} (cycles: {cyc:?})"
        );
    }

    /// ISC-1: §9.5 accounting (`attributed_resident_bytes` / `total_resident_bytes`)
    /// is independent of tensor *size* — the bytes value is a field compare/return,
    /// never a byte touch. Sweep `bytes` across 2^8 … 2^40 and show flat cycles.
    #[test]
    fn lo_isc1_accounting_size_independent() {
        let sizes = [256u64, 1 << 20, 1u64 << 30, 1u64 << 40];
        let owner = 7u32;
        let principals = [7u32, 8, 9, 10];
        let mut amin = u64::MAX;
        let mut amax = 0u64;
        for &bytes in sizes.iter() {
            let rec = SharedRecord { owner, bytes };
            let attr = host_bench(64, 256, || {
                let a = attributed_resident_bytes(black_box(&rec), black_box(owner));
                black_box(a);
            });
            let tot = host_bench(64, 256, || {
                let t = total_resident_bytes(black_box(&rec), black_box(&principals));
                black_box(t);
            });
            println!(
                "NEO_TEST/1 event=measure name=ai_latency_oracle tier=A op=accounting bytes={} attributed_cycles={} total4_cycles={}",
                bytes, attr, tot
            );
            amin = amin.min(attr);
            amax = amax.max(attr);
        }
        // Flat ⇒ size-independent. A size-LINEAR cost would explode ~2^32× across
        // the sweep; a generous absolute+ratio ceiling excludes that at tiny values.
        assert!(
            amax <= amin.saturating_mul(4) + 20,
            "accounting cost scales with tensor size: min={amin} max={amax}"
        );
    }

    /// ISC-1: `total_resident_bytes` is O(principals) — sweep the principal count
    /// and record. (The accounting count-once walk is one pass over the referencing
    /// set.) Shape, not a hard threshold.
    #[test]
    fn lo_isc1_accounting_is_o_principals() {
        let rec = SharedRecord {
            owner: 1,
            bytes: 4096,
        };
        let counts = [1usize, 4, 16, 64];
        let mut cyc = [0u64; 4];
        for (idx, &n) in counts.iter().enumerate() {
            let principals: Vec<u32> = (1..=n as u32).collect();
            cyc[idx] = host_bench(64, 256, || {
                let t = total_resident_bytes(black_box(&rec), black_box(&principals));
                black_box(t);
            });
            println!(
                "NEO_TEST/1 event=measure name=ai_latency_oracle tier=A op=total_resident_bytes principals={} cycles_per_iter={}",
                n, cyc[idx]
            );
        }
        // 16× the input (4 → 64). Linear ~16×; quadratic ~256×. 40× ceiling.
        let c4 = cyc[1].max(1);
        let c64 = cyc[3];
        assert!(
            c64 < c4 * 40,
            "total_resident_bytes super-linear in principals: c(4)={c4} c(64)={c64}"
        );
    }

    /// ISC-1: the generational slot key compare (`arena_logic::key_matches` +
    /// `bump_generation`) — the I-13 stale-key gate every keyed `mint` runs first —
    /// is O(1). Records cycles/iter.
    #[test]
    fn lo_isc1_generation_compare_is_o1() {
        let key = SlotKey {
            index: 42,
            generation: 7,
        };
        let cyc = host_bench(64, 256, || {
            let m = key_matches(black_box(7u32), black_box(key), black_box(42u32));
            black_box(m);
            let g = bump_generation(black_box(7u32));
            black_box(g);
        });
        println!(
            "NEO_TEST/1 event=measure name=ai_latency_oracle tier=A op=generation_compare cycles_per_iter={}",
            cyc
        );
        // Pure arithmetic + two equality checks — must be a small constant.
        assert!(
            cyc < 500,
            "generation compare unexpectedly expensive: {cyc} cycles"
        );
    }

    // === OT-63 GPU P1 Slice 3 — the separable-blur EU kernel + ABI + PRM oracle ===
    use crate::intel_gen9_4a;

    /// Provenance: the pinned `BLUR_KERNEL_GEN9` blob IS the literal
    /// `ocloc`-extracted `.text.separable_blur` (4736 bytes). Its sha256 must equal
    /// the value the finalizer emitted (README blur row) — a re-finalize that
    /// changed the bytes fails here (the exact-bytes / determinism gate), and the
    /// blob must terminate in the finalizer EOT send (else a Slice-4 run hangs).
    #[test]
    fn blur_kernel_blob_provenance() {
        use sha2::{Digest, Sha256};
        assert_eq!(intel_gen9_4a::BLUR_KERNEL_BYTES, 4736);
        assert_eq!(intel_gen9_4a::BLUR_KERNEL_GEN9.len(), 4736);
        let sha = Sha256::digest(intel_gen9_4a::BLUR_KERNEL_GEN9);
        let hex: String = sha.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex, "d40067da102c34ef8ab5ea1b79a306a63abc700a7662537264bd00924d77bfff",
            "blur .krn sha256 drift — re-finalize changed the EU bytes"
        );
        assert!(
            intel_gen9_4a::blur_kernel_has_eot(),
            "blur blob must terminate in the finalizer spawner EOT send"
        );
    }

    /// The cross-thread argument ABI must match the independent oracle
    /// `blur.gen9.zeinfo`: src@32, dst@40, weights@48 (stateless A64); width@56,
    /// height@60, stride@64, radius@68, axis@72 (byvalue u32); enqueued@76; span
    /// [0,88) → 3 GRF cross-thread; SLM=0, barrier=0 (no keys in the zeinfo).
    #[test]
    fn blur_cross_thread_abi_matches_zeinfo() {
        assert_eq!(intel_gen9_4a::BLUR_OFF_ARG_SRC, 32);
        assert_eq!(intel_gen9_4a::BLUR_OFF_ARG_DST, 40);
        assert_eq!(intel_gen9_4a::BLUR_OFF_ARG_WEIGHTS, 48);
        assert_eq!(intel_gen9_4a::BLUR_OFF_WIDTH, 56);
        assert_eq!(intel_gen9_4a::BLUR_OFF_HEIGHT, 60);
        assert_eq!(intel_gen9_4a::BLUR_OFF_STRIDE, 64);
        assert_eq!(intel_gen9_4a::BLUR_OFF_RADIUS, 68);
        assert_eq!(intel_gen9_4a::BLUR_OFF_AXIS, 72);
        assert_eq!(intel_gen9_4a::BLUR_OFF_ENQUEUED_LOCAL_SIZE, 76);
        // span ends at 88 → ceil(88/32) = 3 GRF cross-thread.
        assert_eq!(intel_gen9_4a::BLUR_CROSS_THREAD_GRF, 3);
        // ADR-077 v2 §4.5: the linear-buffer blur uses no SLM, no barrier.
        assert_eq!(intel_gen9_4a::BLUR_SLM_BYTES, 0);
        assert_eq!(intel_gen9_4a::BLUR_BARRIER_COUNT, 0);
        assert_eq!(intel_gen9_4a::BLUR_AXIS_H, 0);
        assert_eq!(intel_gen9_4a::BLUR_AXIS_V, 1);
    }

    /// The cross-thread payload builder lays each field at its zeinfo offset (LE).
    #[test]
    fn blur_curbe_payload_byte_layout() {
        let p = intel_gen9_4a::build_blur_curbe_payload(
            0x1000,
            0x2000,
            0x3000,
            8,
            8,
            32,
            1,
            intel_gen9_4a::BLUR_AXIS_H,
        );
        let le64 = |o: usize| u64::from_le_bytes(p[o..o + 8].try_into().unwrap());
        let le32 = |o: usize| u32::from_le_bytes(p[o..o + 4].try_into().unwrap());
        assert_eq!(le64(intel_gen9_4a::BLUR_OFF_ARG_SRC), 0x1000);
        assert_eq!(le64(intel_gen9_4a::BLUR_OFF_ARG_DST), 0x2000);
        assert_eq!(le64(intel_gen9_4a::BLUR_OFF_ARG_WEIGHTS), 0x3000);
        assert_eq!(le32(intel_gen9_4a::BLUR_OFF_WIDTH), 8);
        assert_eq!(le32(intel_gen9_4a::BLUR_OFF_HEIGHT), 8);
        assert_eq!(le32(intel_gen9_4a::BLUR_OFF_STRIDE), 32);
        assert_eq!(le32(intel_gen9_4a::BLUR_OFF_RADIUS), 1);
        assert_eq!(le32(intel_gen9_4a::BLUR_OFF_AXIS), 0);
        // The kernel is pinned to a 32x8 workgroup. SIMD32 therefore requires
        // eight hardware threads, not one thread with an 8x8 payload.
        assert_eq!(le32(12), 32);
        assert_eq!(le32(16), 8);
        assert_eq!(le32(20), 1);
        assert_eq!(le32(intel_gen9_4a::BLUR_OFF_ENQUEUED_LOCAL_SIZE), 32);
        assert_eq!(le32(intel_gen9_4a::BLUR_OFF_ENQUEUED_LOCAL_SIZE + 4), 8);
        assert_eq!(le32(intel_gen9_4a::BLUR_OFF_ENQUEUED_LOCAL_SIZE + 8), 1);
    }

    /// Raw NEO `generateLocalIDsSimd` cross-check: a 32x8/SIMD32 group is
    /// eight sequential 192-byte per-thread blocks. Each thread owns one y row
    /// while its 32 lanes carry x=0..31.
    #[test]
    fn blur_local_id_payload_covers_all_eight_rows() {
        let payload = intel_gen9_4a::build_blur_local_id_payload();
        assert_eq!(intel_gen9_4a::BLUR_THREADS_PER_GROUP, 8);
        assert_eq!(payload.len(), 8 * intel_gen9_4a::LOCAL_ID_PAYLOAD_BYTES);
        for thread in 0..8usize {
            let base = thread * intel_gen9_4a::LOCAL_ID_PAYLOAD_BYTES;
            for lane in 0..32usize {
                let x = u16::from_le_bytes(
                    payload[base + lane * 2..base + lane * 2 + 2]
                        .try_into()
                        .unwrap(),
                );
                let y_off = base + intel_gen9_4a::LOCAL_ID_OFF_Y + lane * 2;
                let y = u16::from_le_bytes(payload[y_off..y_off + 2].try_into().unwrap());
                assert_eq!(x, lane as u16);
                assert_eq!(y, thread as u16);
            }
        }
    }

    /// amendment-6 PRM command-byte oracle: the H→V inter-pass PIPE_CONTROL, laid
    /// out HAND-COMPUTED from the Kaby Lake PRM (PIPE_CONTROL 6-DWord; DW1 = CS
    /// Stall bit20 | DC Flush bit5), independent of the encoder's constants. A
    /// wrong bit fails here even though QEMU cannot run the Gen9 EU.
    #[test]
    fn blur_interpass_pipe_control_matches_prm_byte_oracle() {
        // Hand-laid 24 bytes: DW0 0x7a000004, DW1 0x00100020, DW2..5 = 0 (no
        // post-sync write; that is the trailing seqno PIPE_CONTROL).
        #[rustfmt::skip]
        const PRM_ORACLE: [u8; 24] = [
            0x04, 0x00, 0x00, 0x7a, // DW0 PIPE_CONTROL, 6 DWords
            0x20, 0x00, 0x10, 0x00, // DW1 CS Stall (bit20) + DC Flush (bit5)
            0x00, 0x00, 0x00, 0x00, // DW2 Address low
            0x00, 0x00, 0x00, 0x00, // DW3 Address high
            0x00, 0x00, 0x00, 0x00, // DW4 Immediate low
            0x00, 0x00, 0x00, 0x00, // DW5 Immediate high
        ];
        let mut got = [0u8; 24];
        assert_eq!(
            intel_gen9_4a::encode_blur_interpass_barrier(&mut got),
            Some(24)
        );
        assert_eq!(
            got, PRM_ORACLE,
            "inter-pass PIPE_CONTROL bytes drifted from the PRM oracle"
        );
        // DW1 bit20 (CS Stall) and bit5 (DC Flush) are exactly what makes Pass V
        // observe Pass H's A64 dataport writes to `mid`.
        let dw1 = u32::from_le_bytes(got[4..8].try_into().unwrap());
        assert_eq!(dw1 & (1 << 20), 1 << 20, "CS Stall (bit20) missing");
        assert_eq!(dw1 & (1 << 5), 1 << 5, "DC Flush (bit5) missing");
        // MOCS is a cacheability index, not a barrier — the blur programs uncached
        // stateless (SBA DW3 = 0), coherent under the PIPE_CONTROL ordering.
        assert_eq!(intel_gen9_4a::BLUR_STATELESS_DATAPORT_MOCS, 0);
    }

    /// The two-pass blur command batch structure: 92 DWords; a GPGPU_WALKER at the
    /// H slot and the V slot; the inter-pass PIPE_CONTROL STRICTLY between them; the
    /// trailing seqno PIPE_CONTROL + MI_BATCH_BUFFER_END. (Structure + command bytes
    /// are host-proven; the 2-D walker geometry / real EU run is the Slice-4 gate.)
    #[test]
    fn blur_two_pass_batch_structure() {
        const SEQNO: u32 = 0x0A0B_0C0D;
        let layout = intel_gen9_4a::BlurBatchLayout {
            state_base: 0x0011_0000,
            kernel_base: 0x0012_0000,
            seqno_base: 0x0013_3000,
            curbe_h_base: 0x0013_4000,
            curbe_v_base: 0x0013_5000,
        };
        let mut buf = [0u8; intel_gen9_4a::BLUR_BATCH_BYTES];
        assert_eq!(
            intel_gen9_4a::encode_blur_batch(&mut buf, SEQNO, 640, 480, layout),
            Some(intel_gen9_4a::BLUR_BATCH_BYTES)
        );
        assert_eq!(intel_gen9_4a::BLUR_BATCH_DWORDS, 92);
        let dw = |i: usize| u32::from_le_bytes(buf[i * 4..i * 4 + 4].try_into().unwrap());
        // Independent IDD oracle: eight hardware threads in the descriptor must
        // equal walker (depth=1 * height=1 * width=8). Mesa's Gen9 path likewise
        // packs dispatch.threads into ThreadWidthCounterMaximum.
        assert_eq!(
            intel_gen9_4a::blur_interface_descriptor_dwords(),
            [0x0012_0000, 0, 0, 0, 0, 6 << 16, 8, 3]
        );
        // MEDIA_VFE_STATE DW5's URB Entry Allocation Size is a U16-1 field in
        // 32-byte registers. It must cover the complete 1,664-byte indirect
        // payload (cross-thread data plus all eight local-ID rows).
        let encoded_urb_regs = dw(intel_gen9_4a::BLUR_VFE_STATE_DW + 5) >> 16;
        let allocated_urb_bytes = (encoded_urb_regs + 1) * 32;
        assert_eq!(intel_gen9_4a::BLUR_URB_ENTRY_ALLOCATION_REGS, 52);
        assert_eq!(encoded_urb_regs, 51);
        assert!(
            allocated_urb_bytes >= intel_gen9_4a::BLUR_WALKER_INDIRECT_DATA_LENGTH,
            "VFE URB entry truncates the walker indirect payload"
        );
        // GPGPU_WALKER header opcode (0x7105000d) at both walker slots.
        const WALKER: u32 = 0x7105_000d;
        assert_eq!(
            dw(intel_gen9_4a::BLUR_WALKER_H_DW),
            WALKER,
            "H walker missing"
        );
        assert_eq!(
            dw(intel_gen9_4a::BLUR_WALKER_V_DW),
            WALKER,
            "V walker missing"
        );
        // The inter-pass PIPE_CONTROL sits strictly between the two walkers
        // (a compile-time invariant of the fixed packet lengths).
        const { assert!(intel_gen9_4a::BLUR_WALKER_H_DW < intel_gen9_4a::BLUR_INTERPASS_PC_DW) };
        const { assert!(intel_gen9_4a::BLUR_INTERPASS_PC_DW < intel_gen9_4a::BLUR_WALKER_V_DW) };
        assert_eq!(
            dw(intel_gen9_4a::BLUR_INTERPASS_PC_DW),
            0x7a00_0004,
            "inter-pass PC header"
        );
        assert_eq!(
            dw(intel_gen9_4a::BLUR_INTERPASS_PC_DW + 1),
            0x0010_0020,
            "inter-pass PC must be CS-stall + DC-flush"
        );
        // The two walkers carry the two distinct cross-thread heaps (H vs V).
        assert_eq!(
            dw(intel_gen9_4a::BLUR_WALKER_H_DW + 3),
            layout.curbe_h_base as u32
        );
        assert_eq!(
            dw(intel_gen9_4a::BLUR_WALKER_V_DW + 3),
            layout.curbe_v_base as u32
        );
        // DW2 covers cross-thread + all eight per-thread local-ID blocks,
        // aligned to 64 B. DW4 encodes SIMD32 and ThreadWidthMax=7 (=8 threads).
        assert_eq!(
            dw(intel_gen9_4a::BLUR_WALKER_H_DW + 2),
            intel_gen9_4a::BLUR_WALKER_INDIRECT_DATA_LENGTH
        );
        assert_eq!(dw(intel_gen9_4a::BLUR_WALKER_H_DW + 4), (0b10 << 30) | 7);
        // 2-D group geometry: ceil(640/32)=20 x, ceil(480/8)=60 y.
        assert_eq!(intel_gen9_4a::blur_group_dims(640, 480), (20, 60));
        assert_eq!(dw(intel_gen9_4a::BLUR_WALKER_H_DW + 7), 20);
        assert_eq!(dw(intel_gen9_4a::BLUR_WALKER_H_DW + 10), 60);
        // Trailing: seqno PIPE_CONTROL then MI_BATCH_BUFFER_END (DW 92-7..).
        assert_eq!(dw(85), 0x7a00_0004, "trailing seqno PC header");
        assert_eq!(dw(89), SEQNO, "seqno immediate");
        assert_eq!(dw(91), 0x0500_0000, "MI_BATCH_BUFFER_END");
    }
}

/// B1c (F1): the one pre-allocation bounds policy for the three generate ABIs.
#[cfg(test)]
mod generate_bounds_tests {
    use super::generate_logic::{
        GenerateBounds, GenerateRequest, GenerateRequestError, MAX_CONSTRAINED_CANDIDATE_SEQUENCES,
        MAX_CONSTRAINED_CANDIDATE_TOKENS, MAX_GENERATE_MASK_BITS, MAX_GENERATE_MASK_BYTES,
        MAX_GENERATE_NEW_TOKENS, MAX_GENERATE_PROMPT_TOKENS, check_generate_request,
        check_prompt_len,
    };

    /// B1c AAG J2: the register-carried check the constrained and masked arms
    /// run before their descriptor copy is exactly the policy's prompt rule.
    #[test]
    fn register_prompt_check_matches_the_policy() {
        for prompt_len in [
            0,
            1,
            2,
            MAX_GENERATE_PROMPT_TOKENS - 1,
            MAX_GENERATE_PROMPT_TOKENS,
        ]
        .into_iter()
        .chain([MAX_GENERATE_PROMPT_TOKENS + 1, usize::MAX])
        {
            let full = plain(prompt_len, 1).err() == Some(GenerateRequestError::Prompt);
            assert_eq!(
                check_prompt_len(prompt_len).is_err(),
                full,
                "prompt_len={prompt_len}"
            );
        }
    }

    fn plain(prompt_len: usize, max_new: usize) -> Result<GenerateBounds, GenerateRequestError> {
        check_generate_request(GenerateRequest::Plain {
            prompt_len,
            max_new,
        })
    }

    fn constrained(
        prompt_len: usize,
        max_new: usize,
        sequence_count: usize,
        tokens_len: usize,
    ) -> Result<GenerateBounds, GenerateRequestError> {
        check_generate_request(GenerateRequest::Constrained {
            prompt_len,
            max_new,
            sequence_count,
            tokens_len,
        })
    }

    fn masked(
        prompt_len: usize,
        disabled: bool,
        mask_bits: usize,
        mask_bytes: usize,
    ) -> Result<GenerateBounds, GenerateRequestError> {
        check_generate_request(GenerateRequest::Masked {
            prompt_len,
            disabled,
            mask_bits,
            mask_bytes,
        })
    }

    #[test]
    fn the_limits_are_the_documented_values() {
        assert_eq!(MAX_GENERATE_NEW_TOKENS, 256);
        assert_eq!(MAX_GENERATE_PROMPT_TOKENS, 2048);
        assert_eq!(MAX_CONSTRAINED_CANDIDATE_SEQUENCES, 16);
        assert_eq!(MAX_CONSTRAINED_CANDIDATE_TOKENS, 1024);
        assert_eq!(MAX_GENERATE_MASK_BITS, 262_144);
        assert_eq!(MAX_GENERATE_MASK_BYTES, 32_768);
    }

    #[test]
    fn plain_caps_and_their_edges() {
        let ok = |p, n| {
            Ok(GenerateBounds {
                prompt_len: p,
                max_new: n,
            })
        };
        assert_eq!(plain(1, 1), ok(1, 1));
        assert_eq!(plain(2048, 256), ok(2048, 256));
        assert_eq!(plain(2049, 1), Err(GenerateRequestError::Prompt));
        assert_eq!(plain(1, 257), Err(GenerateRequestError::NewTokens));
        assert_eq!(plain(0, 1), Err(GenerateRequestError::Prompt));
        assert_eq!(plain(1, 0), Err(GenerateRequestError::NewTokens));
        assert!(plain(usize::MAX, usize::MAX).is_err());
        // Arthur's largest requests: a 512-token window plus its 32 new
        // tokens, and the agentic path's 192.
        assert!(plain(512 + 64, 32).is_ok());
        assert!(plain(512, 192).is_ok());
    }

    #[test]
    fn constrained_gains_the_prompt_cap_and_keeps_its_own_limits() {
        assert_eq!(
            constrained(2048, 256, 16, 1024),
            Ok(GenerateBounds {
                prompt_len: 2048,
                max_new: 256
            })
        );
        // New in B1c: the prompt cap (it had none before).
        assert_eq!(
            constrained(2049, 1, 1, 1),
            Err(GenerateRequestError::Prompt)
        );
        assert_eq!(constrained(0, 1, 1, 1), Err(GenerateRequestError::Prompt));
        // Kept: its new-token, sequence and candidate-token limits.
        assert_eq!(
            constrained(1, 257, 1, 1),
            Err(GenerateRequestError::NewTokens)
        );
        assert_eq!(
            constrained(1, 0, 1, 1),
            Err(GenerateRequestError::NewTokens)
        );
        assert_eq!(
            constrained(1, 1, 17, 1),
            Err(GenerateRequestError::Sequences)
        );
        assert_eq!(
            constrained(1, 1, 0, 1),
            Err(GenerateRequestError::Sequences)
        );
        assert_eq!(
            constrained(1, 1, 1, 1025),
            Err(GenerateRequestError::CandidateTokens)
        );
        assert_eq!(
            constrained(1, 1, 1, 0),
            Err(GenerateRequestError::CandidateTokens)
        );
    }

    #[test]
    fn masked_generates_exactly_one_token_under_the_prompt_cap() {
        let one = |p| {
            Ok(GenerateBounds {
                prompt_len: p,
                max_new: 1,
            })
        };
        assert_eq!(masked(2048, false, 9, 2), one(2048));
        assert_eq!(
            masked(7, true, 0, 0),
            one(7),
            "disabled ignores the mask fields"
        );
        assert_eq!(
            masked(1, false, MAX_GENERATE_MASK_BITS, MAX_GENERATE_MASK_BYTES),
            one(1)
        );
        assert_eq!(masked(2049, false, 8, 1), Err(GenerateRequestError::Prompt));
        assert_eq!(masked(2049, true, 0, 0), Err(GenerateRequestError::Prompt));
        assert_eq!(masked(0, false, 8, 1), Err(GenerateRequestError::Prompt));
        assert_eq!(masked(1, false, 0, 0), Err(GenerateRequestError::Mask));
        assert_eq!(masked(1, false, 9, 1), Err(GenerateRequestError::Mask));
        assert_eq!(masked(1, false, 8, 2), Err(GenerateRequestError::Mask));
        assert_eq!(
            masked(
                1,
                false,
                MAX_GENERATE_MASK_BITS + 8,
                MAX_GENERATE_MASK_BYTES + 1
            ),
            Err(GenerateRequestError::Mask)
        );
        assert_eq!(
            masked(1, false, usize::MAX, usize::MAX),
            Err(GenerateRequestError::Mask)
        );
    }

    /// The 0.5B KV cache at the caps: 24 layers x 2 (K, V) x 128 (kv_len) x
    /// 4 bytes per token.
    #[test]
    fn kv_cache_at_the_caps_fits_the_boot_module_heap() {
        const PER_TOKEN: usize = 24 * 2 * 128 * 4;
        let kv = (MAX_GENERATE_PROMPT_TOKENS + MAX_GENERATE_NEW_TOKENS) * PER_TOKEN;
        assert_eq!(kv, 56_623_104);
        assert!(
            kv < 305 * 1024 * 1024 / 4,
            "under a quarter of the 305 MiB tier"
        );
    }
}

/// B1c (F6): the attention step's heap shape at the 0.5B geometry and the
/// 2,304-token cap, measured with a per-thread counting allocator (other tests
/// on other threads are not counted). The budget in the B1c NOTE section 6
/// rests on this term: the full query `[14 x S x 64]`, the active K and V,
/// their byte copies, and the softmax weights live at once inside one layer.
/// Every soft-f32 entry op checks MXCSR (relay #68, item 4).
#[cfg(test)]
mod mxcsr_entry_check_tests;

/// Lever 2: the `f32` SwiGLU the executor splits is byte-identical, whole and split.
#[cfg(test)]
mod swiglu_split_kat;

/// Lever 2: the cached attention step split by q-head range is byte-identical.
#[cfg(test)]
mod attention_heads_split_kat;

/// Lever 1c: the cached attention step reads the KV cache in place, byte-identically.
#[cfg(test)]
mod attention_lever1c_kat;

/// Lever 1b: the rewritten last-position attention is byte-identical.
#[cfg(test)]
mod attention_lever1b_kat;

/// The `prefill-perf` overlay's `perf` line (sw=/at= shares).
#[cfg(test)]
mod prefill_perf_line_tests;

/// Lever 1: hardware `soft_f32` add/sub/mul are byte-identical to the integer path.
#[cfg(test)]
mod soft_f32_hw_kat;

#[cfg(test)]
mod attention_heap_shape_tests {
    use super::forward_logic::{
        Qwen2AttentionSeam, Qwen2KvCacheLayer, Qwen2LayerConfig, gqa_attention_cached_decode,
    };
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    struct Counting;

    thread_local! {
        static TRACKING: Cell<bool> = const { Cell::new(false) };
        static LIVE: Cell<isize> = const { Cell::new(0) };
        static PEAK: Cell<isize> = const { Cell::new(0) };
    }

    fn note(delta: isize) {
        let _ = TRACKING.try_with(|tracking| {
            if tracking.get() {
                LIVE.with(|live| {
                    let now = live.get() + delta;
                    live.set(now);
                    PEAK.with(|peak| peak.set(peak.get().max(now)));
                });
            }
        });
    }

    // SAFETY: forwards to `System`; the bookkeeping touches only const
    // thread-locals (no allocation inside the allocator).
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let ptr = unsafe { System.alloc(layout) };
            if !ptr.is_null() {
                note(layout.size() as isize);
            }
            ptr
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            note(-(layout.size() as isize));
            unsafe { System.dealloc(ptr, layout) };
        }
    }

    #[global_allocator]
    static ALLOCATOR: Counting = Counting;

    /// Peak live bytes allocated by `f` above the level at its start.
    fn peak_of<R>(f: impl FnOnce() -> R) -> (R, usize) {
        LIVE.with(|l| l.set(0));
        PEAK.with(|p| p.set(0));
        TRACKING.with(|t| t.set(true));
        let result = f();
        TRACKING.with(|t| t.set(false));
        (result, PEAK.with(Cell::get) as usize)
    }

    #[test]
    fn decode_attention_at_the_cap_peaks_at_4_bytes_per_position() {
        const S: usize = 2_048 + 256;
        let config = Qwen2LayerConfig::qwen25_05b();
        assert_eq!(
            (config.q_heads, config.kv_heads, config.head_dim),
            (14, 2, 64)
        );
        let mut cache = Qwen2KvCacheLayer::new(config, S).expect("cache");
        let kv = vec![0.25f32; config.kv_len()];
        for position in 0..S - 1 {
            cache
                .append_rope_kv(position as u64, &kv, &kv)
                .expect("append");
        }
        let seam = Qwen2AttentionSeam {
            attn_norm: Vec::new(),
            q: Vec::new(),
            k: Vec::new(),
            v: kv.clone(),
            gate: None,
            q_rope: vec![0.5f32; config.q_len()],
            k_rope: kv.clone(),
        };
        let (out, peak) =
            peak_of(|| gqa_attention_cached_decode(&seam, &mut cache, config, (S - 1) as u64));
        assert_eq!(out.expect("attention").len(), config.q_len());
        // Lever 1c reads the cache in place: the only allocations are the
        // 4 B softmax weight per position and the 14*64*4 = 3,584 B output
        // row. Before it, the step copied the query, K and V twice (f32 and
        // bytes): 9,220 B per position, 21.2 MB at the cap.
        let expected = 4 * S + config.q_len() * 4;
        println!("attention peak={peak} expected={expected} S={S}");
        assert_eq!(
            peak, expected,
            "decode attention allocated beyond weights + output"
        );
    }
}

/// `NEOVOCB1`, the prebuilt binary vocab (`kernel/src/ai/vocab_bin_logic.rs`,
/// `Plans/NOTE-2026-10-26-arthur-binary-vocab-design-s1.md`): the byte
/// oracle, the reader's KAT, text-vs-binary equivalence, and one refusal per
/// validation rule and converter precondition.
#[cfg(test)]
mod vocab_bin_tests {
    use crate::tokenizer_logic::{BpeTables, TokenizerError};
    use crate::vocab_bin_logic::{ConvertError, NO_ID, VocabBin, VocabBinError, write};
    use crate::vocab_sidecar::{
        QwenTemplate, RealVocab, convert_text_to_binary, try_build_binary_vocab,
        try_build_real_vocab,
    };

    /// The design NOTE's oracle, laid out by hand (not by the writer): the
    /// five tokens `\n`, `Ġ`, `h`, `i`, `hi`, the merge `h`+`i` at rank 0,
    /// eos 4.
    #[rustfmt::skip]
    const ORACLE: [u8; 136] = [
        0x4E, 0x45, 0x4F, 0x56, 0x4F, 0x43, 0x42, 0x31, 0x01, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00,
        0x05, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x04, 0x00, 0x00, 0x00,
        0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x88, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x40, 0x00, 0x00, 0x00, 0x58, 0x00, 0x00, 0x00, 0x60, 0x00, 0x00, 0x00, 0x78, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00,
        0x05, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x0A, 0xC4, 0xA0, 0x68, 0x69, 0x68, 0x69, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00,
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00,
        0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// The oracle's source, in the text sidecar format.
    fn oracle_sidecar() -> String {
        let mut s = String::from("NEOM_TOKENIZER\t1\neos\t4\ntokens\t5\n");
        for (id, t) in [&b"\n"[..], &[0xC4, 0xA0], b"h", b"i", b"hi"]
            .iter()
            .enumerate()
        {
            s.push_str(&format!("{id}\t{}\n", hex(t)));
        }
        s.push_str(&format!("merges\t1\n0\t{}\n", hex(b"h i")));
        s
    }

    fn sidecar(header: &str, tokens: &[&[u8]], merges: &[&[u8]]) -> String {
        let mut s = format!("NEOM_TOKENIZER\t1\n{header}tokens\t{}\n", tokens.len());
        for (id, t) in tokens.iter().enumerate() {
            s.push_str(&format!("{id}\t{}\n", hex(t)));
        }
        s.push_str(&format!("merges\t{}\n", merges.len()));
        for (rank, m) in merges.iter().enumerate() {
            s.push_str(&format!("{rank}\t{}\n", hex(m)));
        }
        s
    }

    /// Test 1: the converter's output for the oracle's sidecar is the 136
    /// literal bytes.
    #[test]
    fn converter_output_is_the_hand_laid_oracle() {
        let out = convert_text_to_binary(oracle_sidecar().as_bytes()).expect("convert");
        assert_eq!(out.as_slice(), &ORACLE[..]);
        // The writer alone, with the source in another order, agrees.
        let toks: [(u32, &[u8]); 5] = [
            (4, b"hi"),
            (2, b"h"),
            (0, b"\n"),
            (3, b"i"),
            (1, &[0xC4, 0xA0]),
        ];
        let w = write(&toks, &[(b"h", b"i", 0)], None, Some(4), None).expect("write");
        assert_eq!(w.as_slice(), &ORACLE[..]);
    }

    /// The oracle rebuilt field by field from the NOTE's layout table, so the
    /// literal above is itself checked against the spec, not only against
    /// the writer.
    #[test]
    fn oracle_matches_an_independent_construction_from_the_spec() {
        let mut b: Vec<u8> = Vec::new();
        let u32s = |b: &mut Vec<u8>, vs: &[u32]| vs.iter().for_each(|v| b.extend(v.to_le_bytes()));
        b.extend(b"NEOVOCB1");
        u32s(&mut b, &[1, 64, 5, 1, NO_ID, 4, NO_ID, 0]);
        b.extend(136u64.to_le_bytes());
        u32s(&mut b, &[64, 88, 96, 120]);
        assert_eq!(b.len(), 64);
        u32s(&mut b, &[0, 1, 3, 4, 5, 7]); // \n | C4 A0 | h | i | hi
        assert_eq!(b.len(), 88);
        b.extend([0x0A, 0xC4, 0xA0, b'h', b'i', b'h', b'i', 0]);
        // By bytes: 0A < 68 < 68 69 < 69 < C4 A0.
        u32s(&mut b, &[0, 2, 4, 3, 1, 0]);
        assert_eq!(b.len(), 120);
        u32s(&mut b, &[2, 3, 4, 0]);
        assert_eq!(b.as_slice(), &ORACLE[..]);
    }

    /// Test 2: the reader on the oracle gives the text tokenizer's own KAT.
    #[test]
    fn reader_on_the_oracle_encodes_and_decodes_like_the_text_kat() {
        let v = VocabBin::new(&ORACLE[..]).expect("valid");
        assert_eq!(v.token_count(), 5);
        assert_eq!(v.merge_count(), 1);
        assert_eq!((v.bos(), v.eos(), v.pad()), (None, Some(4), None));
        assert_eq!(v.encode("hi"), Ok(vec![4]));
        assert_eq!(v.encode("h i"), Ok(vec![2, 1, 3]));
        assert_eq!(v.decode(&[4]).as_deref(), Ok("hi"));
        assert_eq!(v.decode(&[2, 1, 3]).as_deref(), Ok("h i"));
        assert_eq!(v.decode(&[5]), Err(TokenizerError::UnknownId));
        assert_eq!(v.encode("x"), Err(TokenizerError::UnknownToken));
        assert_eq!(v.token_id(b"hi"), Some(4));
        assert_eq!(v.token_id(b"zz"), None);
        assert_eq!(v.merge_rank(b"h", b"i"), Some(0));
        assert_eq!(v.merge_rank(b"i", b"h"), None);
    }

    /// Arthur's holder over the binary: the same answers as over the text.
    #[test]
    fn binary_real_vocab_matches_text_on_the_oracle() {
        let text = try_build_real_vocab(oracle_sidecar().as_bytes()).expect("text");
        let bin = try_build_binary_vocab(ORACLE.to_vec()).expect("binary");
        assert_equivalent(&text, &bin, &corpus());
    }

    fn assert_equivalent(text: &RealVocab, bin: &RealVocab, corpus: &[String]) {
        assert_eq!(text.token_count, bin.token_count);
        assert_eq!(text.bos_token_id, bin.bos_token_id);
        assert_eq!(text.eos_token_id, bin.eos_token_id);
        assert_eq!(text.pad_token_id, bin.pad_token_id);
        for s in corpus {
            let t = text.tokenizer.encode(s);
            assert_eq!(t, bin.tokenizer.encode(s), "encode {s:?}");
            if let Ok(ids) = t {
                let d = text.tokenizer.decode(&ids);
                assert_eq!(d, bin.tokenizer.decode(&ids), "decode {s:?}");
                assert_eq!(d.as_deref(), Ok(s.as_str()), "round trip {s:?}");
            }
        }
        // Decode over every id (and one past the end), alone: the same bytes
        // or the same error, and the same control-token set.
        let n = text.token_count as u32;
        for id in 0..=n {
            assert_eq!(
                text.tokenizer.decode(&[id]),
                bin.tokenizer.decode(&[id]),
                "decode [{id}]"
            );
            assert_eq!(
                text.reply_tokens(&[id]),
                bin.reply_tokens(&[id]),
                "control {id}"
            );
        }
        // The chat template, as Arthur frames a turn.
        for input in ["hello world", "What is the capital of France?", ""] {
            let frame = |v: &RealVocab| {
                let mut out = Vec::new();
                QwenTemplate::new()
                    .frame_turn_ids(input, |s| v.tokenizer.encode(s).ok(), &mut out)
                    .map(|()| out)
            };
            assert_eq!(frame(text), frame(bin), "template {input:?}");
        }
    }

    /// The chat template's text, a fixed set of sentences, and 300 generated
    /// strings over letters, digits, punctuation, white space and non-ASCII.
    fn corpus() -> Vec<String> {
        let mut c: Vec<String> = [
            "system\nYou are a helpful assistant.",
            "user\nhello world",
            "assistant\n",
            "\n",
            "hi",
            "h i",
            "Hello",
            "What is the capital of France?",
            "Paris.",
            "The quick brown fox jumps over the lazy dog.",
            "  leading and trailing spaces  ",
            "tabs\tand\r\nCRLF\n\nlines",
            "numbers 0 1 12 123 3.14159 -42 1e10",
            "punctuation!?.,;:'\"()[]{}<>/\\|@#$%^&*-_=+~`",
            "<|im_start|>user\nnot a control token<|im_end|>",
            "list: a, b, c; then (x) [y] {z}",
            "camelCase snake_case kebab-case SCREAMING_CASE",
            "https://example.com/path?q=1&r=2#frag",
            "fn main() { println!(\"hi\"); }",
            "café naïve résumé",
            "中文字符和标点。",
            "日本語のテキスト",
            "Привет, мир!",
            "emoji 🙂🚀 and ✓",
            "mixed ASCII and ünïcödé",
            "a",
            "",
            " ",
            "   ",
            "\t\t",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "ababababababab",
            "I'm can't won't it's they're",
            "Arthur, open the Lens.",
            "list /",
            "2026-10-26 04:05 UTC",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let alphabet: Vec<char> =
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 \n\t.,!?'-:;()éü中文🙂"
                .chars()
                .collect();
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..300 {
            let len = (next() % 40) as usize;
            c.push(
                (0..len)
                    .map(|_| alphabet[(next() % alphabet.len() as u64) as usize])
                    .collect(),
            );
        }
        c
    }

    /// The Stage-B fixture vocab exactly as `xtask`
    /// (`write_stage_b_vocab_sidecar`) writes it for the QEMU scenarios:
    /// 151,936 dense ids, the 256 byte tokens, `<|im_end|>` at 151,644,
    /// `<unused_N>` elsewhere, no merges.
    fn stage_b_fixture_sidecar() -> String {
        fn direct(b: u8) -> bool {
            (33..=126).contains(&b) || (161..=172).contains(&b) || b >= 174
        }
        fn byte_token(b: u8) -> Vec<u8> {
            let scalar = if direct(b) {
                b as u32
            } else {
                256 + (0..b).filter(|&c| !direct(c)).count() as u32
            };
            char::from_u32(scalar).unwrap().to_string().into_bytes()
        }
        let mut s = String::from("NEOM_TOKENIZER\t1\nbos\t151644\neos\t151644\npad\t151644\n");
        s.push_str("tokens\t151936\n");
        for id in 0..151_936usize {
            let token = if id == 151_644 {
                b"<|im_end|>".to_vec()
            } else if id < 256 {
                byte_token(id as u8)
            } else {
                format!("<unused_{id}>").into_bytes()
            };
            s.push_str(&format!("{id}\t{}\n", hex(&token)));
        }
        s.push_str("merges\t0\n");
        s
    }

    /// Test 3 (fixture): text-built and binary-built give the same encode,
    /// decode, specials and control set over the corpus.
    #[test]
    fn binary_equals_text_on_the_stage_b_fixture_vocab() {
        let text_src = stage_b_fixture_sidecar();
        let text = try_build_real_vocab(text_src.as_bytes()).expect("text");
        let bin =
            try_build_binary_vocab(convert_text_to_binary(text_src.as_bytes()).expect("convert"))
                .expect("binary");
        assert_equivalent(&text, &bin, &corpus());
    }

    /// Test 3 (real): the same over the real Qwen2.5-0.5B sidecar, when
    /// `NEO_REAL_VOCAB` names it (7,990,862 bytes; CI has none). Also the
    /// binary reproduces the boot prompt raw-captured from the kernel
    /// (`ot37_real_brain_chat_reply_when_available`'s `PROMPT`).
    #[test]
    fn binary_equals_text_on_the_real_vocab_when_available() {
        let Ok(path) = std::env::var("NEO_REAL_VOCAB") else {
            eprintln!(
                "binary_equals_text_on_the_real_vocab_when_available: NEO_REAL_VOCAB unset, skipped"
            );
            return;
        };
        let src = std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
        let text = try_build_real_vocab(&src).expect("text");
        let bin_bytes = convert_text_to_binary(&src).expect("the real sidecar converts");
        if let Ok(out) = std::env::var("NEO_REAL_VOCB_OUT") {
            std::fs::write(&out, &bin_bytes).expect("write .vocb");
        }
        let bin = try_build_binary_vocab(bin_bytes).expect("binary");
        assert_eq!(bin.token_count, 151_936);
        assert_equivalent(&text, &bin, &corpus());
        const PROMPT: [u32; 21] = [
            151644, 8948, 198, 2610, 525, 264, 10950, 17847, 13, 151645, 198, 151644, 872, 198,
            14990, 1879, 151645, 198, 151644, 77091, 198,
        ];
        let mut built = Vec::new();
        QwenTemplate::new()
            .frame_turn_ids("hello world", |s| bin.tokenizer.encode(s).ok(), &mut built)
            .expect("frame");
        assert_eq!(built, PROMPT);
        eprintln!("REAL_VOCAB_EQUIVALENCE=ok corpus={}", corpus().len());
    }

    fn put32(b: &mut [u8], at: usize, v: u32) {
        b[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }

    fn refused(b: &[u8]) -> VocabBinError {
        match VocabBin::new(b) {
            Ok(_) => panic!("accepted a bad file"),
            Err(e) => e,
        }
    }

    /// Test 4: one mutated-byte case per validation rule, each refused by
    /// that rule.
    #[test]
    fn reader_refuses_each_broken_rule() {
        let case = |f: &dyn Fn(&mut Vec<u8>)| {
            let mut b = ORACLE.to_vec();
            f(&mut b);
            refused(&b)
        };
        use VocabBinError::*;
        assert_eq!(case(&|b| b.truncate(63)), TooShort);
        assert_eq!(case(&|b| b[0] ^= 1), Magic);
        assert_eq!(case(&|b| put32(b, 0x08, 2)), Version);
        assert_eq!(case(&|b| put32(b, 0x0C, 0x48)), HeaderSize);
        assert_eq!(case(&|b| put32(b, 0x24, 1)), Flags);
        assert_eq!(case(&|b| put32(b, 0x28, 0x90)), TotalSize);
        assert_eq!(case(&|b| b.truncate(128)), TotalSize);
        // Each section offset moved, and a file with a trailing record.
        assert_eq!(case(&|b| put32(b, 0x30, 0x48)), Layout);
        assert_eq!(case(&|b| put32(b, 0x34, 0x60)), Layout);
        assert_eq!(case(&|b| put32(b, 0x38, 0x68)), Layout);
        assert_eq!(case(&|b| put32(b, 0x3C, 0x80)), Layout);
        assert_eq!(
            case(&|b| {
                b.extend([0; 16]);
                b[0x28] = 0x98;
            }),
            Layout
        );
        // A count far past the file: bounded, not trusted.
        assert_eq!(case(&|b| put32(b, 0x10, 0xFFFF_FFF0)), Layout);
        assert_eq!(case(&|b| put32(b, 0x14, 0x1000_0000)), Layout);
        // offsets[N] past the file.
        assert_eq!(case(&|b| put32(b, 0x54, 0x100)), Layout);
        assert_eq!(case(&|b| b[0x5F] = 1), Padding);
        assert_eq!(case(&|b| b[0x74] = 1), Padding);
        assert_eq!(case(&|b| put32(b, 0x40, 1)), Offsets);
        // An empty token (offsets[2] == offsets[1]).
        assert_eq!(case(&|b| put32(b, 0x48, 1)), Offsets);
        assert_eq!(case(&|b| put32(b, 0x60, 5)), SortedNotPermutation);
        assert_eq!(case(&|b| put32(b, 0x64, 0)), SortedNotPermutation);
        assert_eq!(
            case(&|b| {
                put32(b, 0x60, 2);
                put32(b, 0x64, 0);
            }),
            SortedOrder
        );
        assert_eq!(case(&|b| put32(b, 0x78, 5)), MergeId);
        assert_eq!(case(&|b| put32(b, 0x80, 5)), MergeId);
        assert_eq!(case(&|b| put32(b, 0x84, 1)), MergeRank);
        assert_eq!(case(&|b| put32(b, 0x80, 0)), MergeBytes);
        assert_eq!(case(&|b| put32(b, 0x1C, 5)), Special);
        assert_eq!(case(&|b| put32(b, 0x18, 7)), Special);
        assert_eq!(case(&|b| put32(b, 0x20, 5)), Special);
    }

    /// The merge rules that need two merges: order, and a repeated rank.
    #[test]
    fn reader_refuses_unordered_merges_and_repeated_ranks() {
        let toks: [(u32, &[u8]); 6] = [
            (0, b"a"),
            (1, b"b"),
            (2, b"c"),
            (3, b"ab"),
            (4, b"bc"),
            (5, b"abc"),
        ];
        let merges: [(&[u8], &[u8], u32); 2] = [(b"a", b"b", 0), (b"b", b"c", 1)];
        let good = write(&toks, &merges, None, None, None).expect("write");
        assert!(VocabBin::new(&good[..]).is_ok());
        let m0 = u32::from_le_bytes(good[0x3C..0x40].try_into().unwrap()) as usize;
        let mut swapped = good.clone();
        swapped[m0..m0 + 16].copy_from_slice(&good[m0 + 16..m0 + 32]);
        swapped[m0 + 16..m0 + 32].copy_from_slice(&good[m0..m0 + 16]);
        assert_eq!(refused(&swapped), VocabBinError::MergeOrder);
        let mut same_rank = good.clone();
        put32(&mut same_rank, m0 + 16 + 12, 0);
        assert_eq!(refused(&same_rank), VocabBinError::MergeRank);
        let mut dup_pair = good.clone();
        dup_pair.copy_within(m0..m0 + 8, m0 + 16);
        assert_eq!(refused(&dup_pair), VocabBinError::MergeOrder);
    }

    /// The converter's preconditions: each violation refuses the source.
    #[test]
    fn converter_refuses_each_precondition_violation() {
        let conv = |s: String| convert_text_to_binary(s.as_bytes());
        assert_eq!(
            convert_text_to_binary(b"NOT_A_SIDECAR\n"),
            Err(ConvertError::Unparseable)
        );
        // Non-dense ids: the text format allows them (the control-token test
        // sidecar), the binary does not.
        let sparse = "NEOM_TOKENIZER\t1\ntokens\t2\n0\t61\n5\t62\nmerges\t0\n";
        assert_eq!(conv(sparse.into()), Err(ConvertError::BadId(5)));
        let dup_id = "NEOM_TOKENIZER\t1\ntokens\t2\n1\t61\n1\t62\nmerges\t0\n";
        assert_eq!(conv(dup_id.into()), Err(ConvertError::BadId(1)));
        assert_eq!(
            conv(sidecar("", &[b"a", b""], &[])),
            Err(ConvertError::EmptyToken(1))
        );
        assert_eq!(
            conv(sidecar("", &[b"a", b"b", b"a"], &[])),
            Err(ConvertError::DuplicateToken(2))
        );
        // `h i` with no `i` token; then with no `hi` token.
        assert_eq!(
            conv(sidecar("", &[b"h", b"hi"], &[b"h i"])),
            Err(ConvertError::MergePartNotToken(0))
        );
        assert_eq!(
            conv(sidecar("", &[b"h", b"i"], &[b"h i"])),
            Err(ConvertError::MergeResultNotToken(0))
        );
        assert_eq!(
            conv(sidecar("", &[b"h", b"i", b"hi"], &[b"h i", b"h i"])),
            Err(ConvertError::DuplicateMerge(1))
        );
        assert_eq!(
            conv(sidecar("eos\t3\n", &[b"h", b"i", b"hi"], &[])),
            Err(ConvertError::SpecialOutOfRange)
        );
        // The writer's own rank check (the text parse numbers merges itself).
        let toks: [(u32, &[u8]); 3] = [(0, b"h"), (1, b"i"), (2, b"hi")];
        assert_eq!(
            write(&toks, &[(b"h", b"i", 1)], None, None, None),
            Err(ConvertError::BadRank(1))
        );
        assert_eq!(
            write(&toks, &[(b"h", b"i", 0), (b"i", b"h", 0)], None, None, None),
            Err(ConvertError::BadRank(0))
        );
        assert_eq!(
            write(&[(0, b"h"), (2, b"i")], &[], None, None, None),
            Err(ConvertError::BadId(2))
        );
    }

    /// Deterministic: the same source converts to the same bytes.
    #[test]
    fn conversion_is_deterministic() {
        let src = stage_b_fixture_sidecar();
        let a = convert_text_to_binary(src.as_bytes()).expect("a");
        let b = convert_text_to_binary(src.as_bytes()).expect("b");
        assert_eq!(a, b);
    }
}

/// `prefill_perf`'s commit slot (review relay #64 item 4): pid and TSC in one
/// word, so a shared slot evicts, never misattributes.
#[cfg(test)]
mod commit_slot_tests {
    use super::commit_slot_logic::{PID_MASK, commit_tsc, commit_word};

    #[test]
    fn a_slot_answers_only_its_own_pid() {
        let tsc = 0x0000_1234_5678_9ABC;
        let word = commit_word(8, tsc);
        // The commit's TSC, to 2^20 cycles.
        assert_eq!(commit_tsc(word, 8), Some(tsc & !PID_MASK));
        assert!(tsc - commit_tsc(word, 8).unwrap() < 1 << 20);
        // Pid 16 shares slot 0 (pid % 8) but never reads pid 8's time.
        assert_eq!(commit_tsc(word, 16), None);
        // An empty slot answers no one.
        assert_eq!(commit_tsc(0, 8), None);
    }

    #[test]
    fn a_later_commit_evicts_and_never_mixes() {
        // Pids 8 and 16 commit into slot 0 on two CPUs: each store is one
        // word, so the slot holds one whole commit, never 16's TSC with 8's pid.
        let (a, b) = (commit_word(8, 0x1000_0000), commit_word(16, 0x7000_0000));
        for last in [a, b] {
            let for_8 = commit_tsc(last, 8);
            let for_16 = commit_tsc(last, 16);
            assert!(for_8.is_none() || for_16.is_none());
            if last == a {
                assert_eq!(for_8, Some(0x1000_0000 & !PID_MASK));
            } else {
                assert_eq!(for_16, Some(0x7000_0000 & !PID_MASK));
            }
        }
    }
}
