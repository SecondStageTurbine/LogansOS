// SPDX-License-Identifier: MPL-2.0
//! `neomc` — `.neom` v1 authoring tool (ADR-028 compliant).
//!
//! Writes spec-compliant `.neom` files, validates them with the self-validator,
//! and generates test fixtures for kernel integration testing.

use neomc::{fixtures, format, gguf_import, inspect, validator, writer};

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("neomc: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(cmd) = args.next() else {
        print_help();
        return Ok(());
    };
    let rest: Vec<String> = args.collect();
    match cmd.as_str() {
        "import" => cmd_import(&rest),
        "write" => cmd_write(&rest),
        "validate" => cmd_validate(&rest),
        "inspect" => cmd_inspect(&rest),
        "fixtures" => cmd_fixtures(&rest),
        "help" | "-h" | "--help" => {
            print_help();
            Ok(())
        }
        other => Err(format!("unknown command `{other}`. Use `neomc help`.")),
    }
}

fn cmd_import(args: &[String]) -> Result<(), String> {
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut vocab_output: Option<PathBuf> = None;
    let mut arch = gguf_import::ImportArch::Qwen2;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| format!("{} requires a path", args[i]))?;
                output = Some(PathBuf::from(value));
                i += 2;
            }
            "--arch" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--arch requires a value".to_string())?;
                arch = gguf_import::ImportArch::parse(value)?;
                i += 2;
            }
            "--vocab-output" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--vocab-output requires a path".to_string())?;
                vocab_output = Some(PathBuf::from(value));
                i += 2;
            }
            flag if flag.starts_with('-') => {
                return Err(format!("unknown import flag '{flag}'"));
            }
            value => {
                if input.is_some() {
                    return Err(format!("unexpected extra import input '{value}'"));
                }
                input = Some(PathBuf::from(value));
                i += 1;
            }
        }
    }

    let input = input.ok_or_else(|| "import requires a GGUF input path".to_string())?;
    let output = output.ok_or_else(|| "import requires -o <output.neom>".to_string())?;
    let options = gguf_import::ImportOptions { arch, vocab_output };
    let inventory = gguf_import::import_gguf_file(&input, &output, &options)?;

    println!(
        "imported {} tensors ({} weight bytes) into {} ({} bytes)",
        inventory.tensor_count,
        inventory.weight_bytes,
        inventory.output_path.display(),
        inventory.total_size
    );
    println!(
        "tensor types: f32={} f16={} bf16={} i32={} i8={} u8={} q4_K={} q5_0={} q5_K={} q6_K={} q8_0={} q8_K={}",
        inventory.counts.f32,
        inventory.counts.f16,
        inventory.counts.bf16,
        inventory.counts.i32,
        inventory.counts.i8,
        inventory.counts.u8,
        inventory.counts.q4_k,
        inventory.counts.q5_0,
        inventory.counts.q5_k,
        inventory.counts.q6_k,
        inventory.counts.q8_0,
        inventory.counts.q8_k
    );
    println!(
        "wrote tokenizer sidecar {} ({} tokens, {} merges)",
        inventory.vocab_path.display(),
        inventory.vocab_tokens,
        inventory.vocab_merges
    );
    Ok(())
}

fn cmd_write(args: &[String]) -> Result<(), String> {
    // For BUILD-2, `write` emits the valid MLP fixture to the given output path.
    // A full spec-file parser (e.g. JSON/YAML DSL) is future work; the builder
    // API is the programmatic interface.
    let out = required_path(args, "--output")?;
    let buf = writer::NeomBuilder::new()
        .build()
        .map_err(|e| format!("builder error: {e}"))?;

    // Self-validate before writing (ISC-2)
    validator::validate(&buf).map_err(|e| format!("self-validation failed: {e}"))?;

    format::write_file(&buf, &out).map_err(|_| format!("failed to write {}", out.display()))?;
    println!("wrote {} bytes to {}", buf.len(), out.display());
    Ok(())
}

fn cmd_validate(args: &[String]) -> Result<(), String> {
    let path = required_path(args, "--input")?;
    let data = std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    match validator::validate(&data) {
        Ok(()) => {
            println!("validate ok: {} ({} bytes)", path.display(), data.len());
            Ok(())
        }
        Err(e) => {
            println!("validate FAIL: {} — {}", path.display(), e);
            Err(format!("validation failed: {e}"))
        }
    }
}

fn cmd_fixtures(args: &[String]) -> Result<(), String> {
    let out_dir = optional_path(args, "--output-dir")
        .unwrap_or_else(|| PathBuf::from("target/neomc-fixtures"));
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("create {}: {e}", out_dir.display()))?;

    let paths =
        fixtures::generate_all(&out_dir).map_err(|e| format!("fixture generation failed: {e}"))?;

    // Validate each positive fixture.
    let valid_names: Vec<&&str> = paths.keys().filter(|&&k| k.starts_with("valid_")).collect();
    for &name in &valid_names {
        let path = paths.get(name).unwrap();
        let valid_data = std::fs::read(path).map_err(|e| format!("read fixture: {e}"))?;
        validator::validate(&valid_data)
            .map_err(|e| format!("valid fixture `{name}` failed self-validation: {e}"))?;
    }

    // Validate each negative fixture fails for the expected reason
    let neg_names: Vec<&&str> = paths.keys().filter(|&&k| k.starts_with("neg_")).collect();
    for &name in &neg_names {
        let path = paths.get(name).unwrap();
        let data = std::fs::read(path).map_err(|e| format!("read fixture: {e}"))?;
        match validator::validate(&data) {
            Ok(()) => {
                return Err(format!(
                    "negative fixture `{name}` should have failed validation but passed"
                ));
            }
            Err(e) => {
                println!("  {name}: rejected ({e})");
            }
        }
    }

    println!(
        "fixtures written to {} ({} valid, {} negative)",
        out_dir.display(),
        valid_names.len(),
        neg_names.len()
    );
    Ok(())
}

fn required_path(args: &[String], flag: &str) -> Result<PathBuf, String> {
    optional_path(args, flag).ok_or_else(|| format!("missing required {flag}"))
}

fn optional_path(args: &[String], flag: &str) -> Option<PathBuf> {
    optional_value(args, flag).map(PathBuf::from)
}

fn optional_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find_map(|pair| (pair[0] == flag).then(|| pair[1].clone()))
}

/// B1c (F4): print each tensor's storage needs and the minimum `MAX_ORDER`.
fn cmd_inspect(args: &[String]) -> Result<(), String> {
    let [path] = args else {
        return Err("usage: neomc inspect <model.neom>".to_string());
    };
    let data = std::fs::read(path).map_err(|e| format!("read {path}: {e}"))?;
    let rows = inspect::inspect(&data).map_err(|e| format!("{path}: {e:?}"))?;
    print!("{}", inspect::render(&rows));
    Ok(())
}

fn print_help() {
    println!("neomc — `.neom` v1 authoring tool (ADR-028 compliant)");
    println!();
    println!("USAGE:");
    println!(
        "    neomc import <model.gguf> -o <model.neom> [--arch qwen2|qwen35] [--vocab-output <model.vocab>]"
    );
    println!(
        "        Import a Qwen2.5 or Qwen3.5 GGUF into .neom weights and a tokenizer sidecar."
    );
    println!("        The .neom output is self-validated before writing.");
    println!();
    println!("    neomc write --output <path.neom>");
    println!("        Build a .neom v1 file and write it to the given path.");
    println!("        The file is self-validated before writing (refuses bad output).");
    println!();
    println!("    neomc validate --input <path.neom>");
    println!("        Validate an existing .neom file against ADR-028 D2 rules.");
    println!();
    println!("    neomc inspect <path.neom>");
    println!("        Validate, then list each tensor's dtype, dims, bytes, pages and buddy");
    println!("        order, and the minimum MAX_ORDER that holds the largest tensor.");
    println!();
    println!("    neomc fixtures [--output-dir <dir>]");
    println!("        Generate the valid MLP fixture and negative test corpus into <dir>.");
    println!("        Default output dir: target/neomc-fixtures");
    println!();
    println!("    neomc help");
    println!("        Show this help message.");
}
