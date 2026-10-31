// SPDX-License-Identifier: MPL-2.0
//! Every op that loops over hardware-backed `soft_f32` arithmetic checks MXCSR at its entry
//! (Chad's review of #130, relay #68, item 4). Proven by behaviour, not by grep: with only the
//! entry checks active (the per-op check off, as in the kernel), each op must panic under a
//! non-kernel MXCSR (round toward +inf, `0x5F80`) and must not panic under the kernel's `0x1F80`.
//! Under the `soft-f32-only` kill-switch the ops use the integer path and skip the check, so there
//! no op may panic under either MXCSR.

use std::panic::{AssertUnwindSafe, catch_unwind};

use super::forward_logic;
use super::generate_logic::{Pcg32, SamplingConfig, sample_next_token};
use super::ops::{self, GqaAttentionParams, TensorView, gdn, soft_f32};
use super::tensor_logic::{Dtype, MAX_RANK, Shape};

const KERNEL_MXCSR: u32 = 0x1f80;
const ROUND_UP_MXCSR: u32 = 0x5f80;

fn read_mxcsr() -> u32 {
    let mut csr = 0u32;
    // SAFETY: STMXCSR stores the 32-bit MXCSR into a live local.
    unsafe {
        core::arch::asm!("stmxcsr [{}]", in(reg) &mut csr, options(nostack, preserves_flags))
    };
    csr
}

fn write_mxcsr(csr: u32) {
    // SAFETY: LDMXCSR loads a valid control word (no reserved bits set) from a live local.
    unsafe { core::arch::asm!("ldmxcsr [{}]", in(reg) &csr, options(nostack, preserves_flags)) };
}

/// Runs `f` under `csr` with only entry-level checks; returns the panic message, if any. The
/// caller's MXCSR is restored even when `f` panics.
fn panic_under(csr: u32, f: &mut dyn FnMut()) -> Option<String> {
    let saved = read_mxcsr();
    write_mxcsr(csr);
    let result = catch_unwind(AssertUnwindSafe(|| {
        soft_f32::with_entry_mxcsr_checks_only(&mut *f)
    }));
    write_mxcsr(saved);
    result.err().map(|p| {
        p.downcast_ref::<String>()
            .cloned()
            .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default()
    })
}

fn one(shape_dims: &[u64]) -> Shape {
    let mut d = [0u64; MAX_RANK];
    d[..shape_dims.len()].copy_from_slice(shape_dims);
    Shape::new(shape_dims.len() as u32, d).unwrap()
}

type Case<'a> = (&'static str, Box<dyn FnMut() + 'a>);

#[test]
fn every_soft_f32_entry_op_checks_mxcsr() {
    let s1 = one(&[1]);
    let s3 = one(&[1, 1, 1]);
    let b = 1.0f32.to_le_bytes();
    let v1 = || TensorView {
        dtype: Dtype::F32,
        shape: &s1,
        bytes: &b,
    };
    let v3 = |shape| TensorView {
        dtype: Dtype::F32,
        shape,
        bytes: &b,
    };
    let gqa = GqaAttentionParams {
        seq_len: 1,
        n_q_heads: 1,
        n_kv_heads: 1,
        head_dim: 1,
    };
    let cfg = gdn::GdnLayerConfig {
        hidden_size: 1,
        num_k_heads: 1,
        num_v_heads: 1,
        head_k_dim: 1,
        head_v_dim: 1,
        conv_kernel_size: 2,
        rms_norm_eps: 1e-6,
    };
    let w = gdn::GdnLayerWeights {
        in_proj_qkv: &[],
        in_proj_z: &[],
        in_proj_b: &[],
        in_proj_a: &[],
        conv1d_weight: &[],
        a_log: &[],
        dt_bias: &[],
        norm_weight: &[],
        out_proj: &[],
    };
    let out = [0u8; 4];
    let fout = [0f32; 1];
    let mut cases: Vec<Case<'_>> = vec![
        (
            "ops::add",
            Box::new(|| {
                let _ = ops::add(&v1(), &v1(), &mut out.clone());
            }),
        ),
        (
            "ops::mul",
            Box::new(|| {
                let _ = ops::mul(&v1(), &v1(), &mut out.clone());
            }),
        ),
        (
            "ops::silu",
            Box::new(|| {
                let _ = ops::silu(&v1(), &mut out.clone());
            }),
        ),
        (
            "ops::swiglu",
            Box::new(|| {
                let _ = ops::swiglu(&v1(), &v1(), &mut out.clone());
            }),
        ),
        (
            "ops::swiglu_f32",
            Box::new(|| {
                let _ = ops::swiglu_f32(&[1.0], &[1.0], &mut fout.clone());
            }),
        ),
        (
            "ops::softmax",
            Box::new(|| {
                let _ = ops::softmax(&v1(), &mut out.clone());
            }),
        ),
        (
            "ops::gelu",
            Box::new(|| {
                let _ = ops::gelu(&v1(), &mut out.clone());
            }),
        ),
        (
            "ops::rms_norm",
            Box::new(|| {
                let _ = ops::rms_norm(&v1(), &mut out.clone());
            }),
        ),
        (
            "ops::layer_norm",
            Box::new(|| {
                let _ = ops::layer_norm(&v1(), &mut out.clone());
            }),
        ),
        (
            "ops::matmul",
            Box::new(|| {
                let _ = ops::matmul(&v1(), &v1(), &mut out.clone());
            }),
        ),
        (
            "ops::gqa_causal_attention",
            Box::new(|| {
                let _ =
                    ops::gqa_causal_attention(&v3(&s3), &v3(&s3), &v3(&s3), gqa, &mut out.clone());
            }),
        ),
        (
            "ops::gqa_causal_attention_last",
            Box::new(|| {
                let _ = ops::gqa_causal_attention_last(
                    &v3(&s3),
                    &v3(&s3),
                    &v3(&s3),
                    gqa,
                    &mut out.clone(),
                );
            }),
        ),
        (
            "ops::gqa_causal_attention_last_strided",
            Box::new(|| {
                let _ = ops::gqa_causal_attention_last_strided(
                    &[1.0],
                    &[1.0],
                    &[1.0],
                    1,
                    gqa,
                    &mut fout.clone(),
                );
            }),
        ),
        (
            "ops::gqa_causal_attention_last_strided_heads",
            Box::new(|| {
                let _ = ops::gqa_causal_attention_last_strided_heads(
                    &[1.0],
                    &[1.0],
                    &[1.0],
                    1,
                    gqa,
                    0..1,
                    &mut [0u32; 1],
                    &mut fout.clone(),
                );
            }),
        ),
        (
            "forward_logic::qwen2_output_logits",
            Box::new(|| {
                let _ = forward_logic::qwen2_output_logits(
                    &[],
                    forward_logic::Qwen2OutputWeights {
                        output_norm: &[],
                        output: &[],
                        output_dtype: forward_logic::TensorDtype::F32,
                        output_quantized: None,
                        output_bias: None,
                    },
                    forward_logic::Qwen2LayerConfig::qwen25_05b(),
                    1,
                );
            }),
        ),
        (
            "gdn::gdn_recurrent_step",
            Box::new(|| {
                let _ = gdn::gdn_recurrent_step(&[], &[], &[], &[], &[], &mut [], &mut []);
            }),
        ),
        (
            "gdn::gdn_causal_conv1d_update",
            Box::new(|| {
                let _ = gdn::gdn_causal_conv1d_update(&[], &mut [], &[], &mut []);
            }),
        ),
        (
            "gdn::gdn_l2norm",
            Box::new(|| {
                let _ = gdn::gdn_l2norm(&[1.0], &mut fout.clone(), 1e-6);
            }),
        ),
        (
            "gdn::gdn_rmsnorm_gated",
            Box::new(|| {
                let _ = gdn::gdn_rmsnorm_gated(&[1.0], &[1.0], &[1.0], 1e-6, &mut fout.clone());
            }),
        ),
        (
            "gdn::gdn_layer_forward",
            Box::new(|| {
                let _ = gdn::gdn_layer_forward(&[], w, cfg, &mut [], &mut []);
            }),
        ),
        (
            "gdn::gdn_layer_projected",
            Box::new(|| {
                let _ = gdn::gdn_layer_projected(&[], w, cfg, &mut [], &mut []);
            }),
        ),
        (
            "gdn::gdn_layer_core_projected",
            Box::new(|| {
                let inputs = gdn::GdnProjectedInputs {
                    qkv: &[],
                    z: &[],
                    beta_raw: &[],
                    a_raw: &[],
                };
                {
                    let _ = gdn::gdn_layer_core_projected(inputs, w, cfg, &mut [], &mut []);
                }
            }),
        ),
        (
            "generate_logic::sample_next_token (fill_weights)",
            Box::new(|| {
                let config = SamplingConfig {
                    greedy: false,
                    temperature: 1.0,
                    top_k: None,
                    top_p: 1.0,
                };
                {
                    let _ = sample_next_token(&[0.0, 1.0], config, &mut Pcg32::seeded(1, 2));
                }
            }),
        ),
    ];
    for (name, f) in cases.iter_mut() {
        assert_eq!(
            panic_under(KERNEL_MXCSR, &mut **f),
            None,
            "{name} panicked under the kernel MXCSR"
        );
        let msg = panic_under(ROUND_UP_MXCSR, &mut **f);
        if cfg!(feature = "soft-f32-only") {
            // The kill-switch build runs the integer path, which MXCSR cannot affect, so the
            // entry check is skipped by design (Chad's review of #134, relay #71).
            assert_eq!(
                msg, None,
                "{name} panicked under 0x5F80 in the soft-f32-only build"
            );
        } else {
            assert!(
                msg.as_deref().is_some_and(|m| m.contains("MXCSR")),
                "{name}: no MXCSR entry check fired under 0x5F80 (got {msg:?})"
            );
        }
    }
}
