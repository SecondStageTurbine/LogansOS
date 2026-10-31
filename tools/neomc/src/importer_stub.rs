// SPDX-License-Identifier: MPL-2.0
//! Importer stub — documented roadmap for ONNX/GGUF/SafeTensors → `.neom` conversion.
//!
//! # Roadmap
//!
//! BUILD-2 shipped the `.neom` writer and validator. L1 now ships the Qwen2
//! GGUF host importer in `gguf_import.rs`; the remaining planned paths are:
//!
//! 1. **SafeTensors** — lowest risk. SafeTensors is a flat key→(dtype, shape,
//!    data) map with a JSON header. Each key maps directly to a `TensorSpec`.
//!    Graph structure would come from a companion JSON manifest describing ops
//!    and value refs.
//!
//! 2. **ONNX** — medium risk. Requires parsing the protobuf model proto, mapping
//!    ONNX opsets to our OpCode set (ADR-028 D3), and handling ONNX's value
//!    information / initializer semantics. Some ONNX ops have no direct
//!    equivalent and would need decomposition before emission.
//!
//! All importers share the same `NeomBuilder` API — they produce `TensorSpec`,
//! `NodeSpec`, and `IoSpec` instances, then call `builder.build_and_write()`.

#![allow(dead_code)]

/// Stub trait for a future format importer.
pub trait ModelImporter {
    /// Returns a descriptive name for the source format.
    fn format_name(&self) -> &str;

    /// Import the model from the given path and return a `NeomBuilder` populated
    /// with tensors, nodes, and IO declarations.
    fn import(&self, _path: &std::path::Path) -> Result<(), String> {
        Err(format!(
            "Importer '{}' is not yet implemented (BUILD-2 stub). See importer_stub.rs for roadmap.",
            self.format_name()
        ))
    }
}

/// Placeholder for SafeTensors import.
pub struct SafeTensorsImporter;

impl ModelImporter for SafeTensorsImporter {
    fn format_name(&self) -> &str {
        "safetensors"
    }
}

/// Placeholder for ONNX import.
pub struct OnnxImporter;

impl ModelImporter for OnnxImporter {
    fn format_name(&self) -> &str {
        "onnx"
    }
}

/// Placeholder for GGUF import.
pub struct GgufImporter;

impl ModelImporter for GgufImporter {
    fn format_name(&self) -> &str {
        "gguf"
    }

    fn import(&self, path: &std::path::Path) -> Result<(), String> {
        Err(format!(
            "GGUF import is implemented by the CLI because it needs output and tokenizer-sidecar paths: neomc import {} -o <model.neom>",
            path.display()
        ))
    }
}
