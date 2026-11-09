// SPDX-License-Identifier: MPL-2.0
//! Host-only quantization-drift harness.
//!
//! Runs the kernel's own forward pass (the files are path-`mod`ed below, the
//! same way `tools/ai-ops-tests` mounts them) over Arthur's shipped
//! `model-qwen2.5-0.5b.neom`, and compares it with llama.cpp runs of the same
//! Q4_K_M GGUF and of an F32 conversion of the published safetensors.
//!
//! The harness is hermetic: it does nothing without `QUANT_DRIFT_NEOM`, and the
//! model/corpus files are never committed. The committed artifact is
//! `fixtures/quant-drift-kat-2026-11-08.json`, which pins the inputs and records
//! the reference token IDs and perplexities.
#![allow(
    dead_code,
    clippy::new_without_default,
    clippy::needless_range_loop,
    clippy::too_many_arguments
)]

extern crate alloc;

#[path = "../../../kernel/src/ai/forward_logic.rs"]
pub mod forward_logic;
#[path = "../../../kernel/src/ai/generate_logic.rs"]
pub mod generate_logic;
#[path = "../../../kernel/src/ai/gguf_dequant_logic.rs"]
pub mod gguf_dequant_logic;
#[path = "../../../kernel/src/ai/gguf_logic.rs"]
pub mod gguf_logic;
#[path = "../../../kernel/src/ai/kv_layout.rs"]
pub mod kv_layout;
#[path = "../../../kernel/src/ai/model_logic.rs"]
pub mod model_logic;
#[path = "../../../kernel/src/ai/ops/mod.rs"]
pub mod ops;
#[path = "../../../kernel/src/ai/tensor_logic.rs"]
pub mod tensor_logic;
#[path = "../../../kernel/src/ai/tokenizer_logic.rs"]
pub mod tokenizer_logic;
#[path = "../../../kernel/src/ai/vocab_bin_logic.rs"]
pub mod vocab_bin_logic;
#[path = "../../../userland/neo-arthur/src/vocab_sidecar.rs"]
pub mod vocab_sidecar;

pub mod model;
pub mod prompts;
pub mod report;
