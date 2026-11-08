// SPDX-License-Identifier: MPL-2.0
//! Driver for the quantization-drift KAT.
//!
//! ```text
//! quant-drift-tests prompts --neom M.neom --corpus-text test.txt \
//!     --out-prompts prompts.tsv --out-corpus corpus.u32
//! quant-drift-tests run --neom M.neom --prompts prompts.tsv --corpus corpus.u32 \
//!     --label ours --sha256 HEX --source TEXT --tool TEXT --out ours.tsv
//! quant-drift-tests compare --ours ours.tsv --q4 q4.tsv --fp32 fp32.tsv \
//!     --meta meta.tsv --kat-out KAT.json --markdown-out REPORT.md
//! ```

use std::collections::BTreeMap;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::process::ExitCode;

use quant_drift_tests::model::Model;
use quant_drift_tests::prompts::{CORPUS_EVAL_START, CORPUS_TOKENS, PROMPTS};
use quant_drift_tests::report::{self, KatMeta, RefRun};
use quant_drift_tests::vocab_sidecar::RealVocab;

const MAX_NEW: usize = 32;
const LOGIT_DUMP_MAGIC: &[u8; 4] = b"QDLG";
const HIDDEN_DUMP_MAGIC: &[u8; 4] = b"QDLH";
const LOGIT_DUMP_VERSION: u32 = 1;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first() else {
        print_help();
        return ExitCode::FAILURE;
    };
    let options = match parse_options(&args[1..]) {
        Ok(options) => options,
        Err(err) => {
            eprintln!("quant-drift-tests: {err}");
            return ExitCode::FAILURE;
        }
    };
    let result = match command.as_str() {
        "prompts" => cmd_prompts(&options),
        "run" => cmd_run(&options),
        "compare" => cmd_compare(&options),
        "control" => cmd_control(&options),
        "encode" => cmd_encode(&options),
        "logits" => cmd_logits(&options),
        "hidden" => cmd_hidden(&options),
        "debug-layer" => cmd_debug_layer(&options),
        "help" | "-h" | "--help" => {
            print_help();
            Ok(())
        }
        other => Err(format!("unknown command {other:?}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("quant-drift-tests: {err}");
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    eprintln!(
        "quant-drift-tests <prompts|run|compare|control|encode|logits>\n\
         \n\
         prompts  --neom M.neom [--corpus-text test.txt] --out-prompts prompts.tsv "
    );
}

/// Load the shipped `.neom`, or the same metadata/vocabulary with the weights
/// of a fully dequantized F32 GGUF (`--gguf-f32`).
fn load_model(options: &BTreeMap<String, String>) -> Result<(Model, RealVocab), String> {
    let neom_path = Path::new(required(options, "neom")?);
    match options.get("gguf-f32") {
        Some(gguf_path) => Model::load_f32_gguf(Path::new(gguf_path), neom_path),
        None => Model::load(neom_path),
    }
    .map_err(|err| format!("load model: {err}"))
}

fn parse_options(args: &[String]) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    let mut index = 0;
    while index < args.len() {
        let key = args[index]
            .strip_prefix("--")
            .ok_or_else(|| format!("expected --name, got {:?}", args[index]))?
            .to_string();
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("--{key} needs a value"))?
            .clone();
        out.insert(key, value);
        index += 2;
    }
    Ok(out)
}

fn required<'a>(options: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, String> {
    options
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| format!("--{key} is required"))
}

fn cmd_prompts(options: &BTreeMap<String, String>) -> Result<(), String> {
    let (model, vocab) = load_model(options)?;
    let mut run = RefRun {
        model: "ours".to_string(),
        ..Default::default()
    };
    for prompt in PROMPTS {
        let ids = model.frame_prompt(&vocab, prompt.text)?;
        run.prompts.push(report::PromptRun {
            id: prompt.id.to_string(),
            text_hex: hex(prompt.text.as_bytes()),
            ids,
            generated: Vec::new(),
            top1: Vec::new(),
            ..Default::default()
        });
    }
    let out = required(options, "out-prompts")?;
    std::fs::write(out, run.to_tsv()).map_err(|err| format!("write {out}: {err}"))?;
    eprintln!(
        "prompts: {} prompts, {} total prompt tokens -> {out}",
        run.prompts.len(),
        run.prompts
            .iter()
            .map(|prompt| prompt.ids.len())
            .sum::<usize>()
    );

    if let Some(text_path) = options.get("corpus-text") {
        let text =
            std::fs::read_to_string(text_path).map_err(|err| format!("read {text_path}: {err}"))?;
        let all = model.encode(&vocab, &text)?;
        if all.len() < CORPUS_TOKENS {
            return Err(format!(
                "corpus tokenizes to {} tokens, need {CORPUS_TOKENS}",
                all.len()
            ));
        }
        let slice = &all[..CORPUS_TOKENS];
        let out = required(options, "out-corpus")?;
        let mut bytes = Vec::with_capacity(slice.len() * 4);
        for id in slice {
            bytes.extend_from_slice(&id.to_le_bytes());
        }
        std::fs::write(out, &bytes).map_err(|err| format!("write {out}: {err}"))?;
        eprintln!(
            "corpus: {} tokens ({:?}..) -> {out}",
            slice.len(),
            &slice[..4]
        );
    }
    Ok(())
}

fn cmd_run(options: &BTreeMap<String, String>) -> Result<(), String> {
    let (model, _vocab) = load_model(options)?;
    let prompts_text = std::fs::read_to_string(required(options, "prompts")?)
        .map_err(|err| format!("read prompts: {err}"))?;
    let mut input = RefRun::from_tsv(&prompts_text)?;
    if input.prompts.iter().any(|prompt| prompt.ids.is_empty()) {
        return Err("prompts file has an empty prompt_ids line".to_string());
    }

    let prompts_only = options
        .get("prompts-only")
        .map(|value| value == "1" || value == "true")
        .unwrap_or(false);
    let started = std::time::Instant::now();
    let mut run = RefRun {
        model: required(options, "label")?.to_string(),
        model_sha256: options.get("sha256").cloned().unwrap_or_default(),
        model_source: options.get("source").cloned().unwrap_or_default(),
        tool: options.get("tool").cloned().unwrap_or_default(),
        ..Default::default()
    };
    for prompt in &mut input.prompts {
        // Every model receives the SAME token IDs (bar A/B comparability).
        let generated_steps = model.greedy_with_steps(&prompt.ids, MAX_NEW);
        let generated = generated_steps
            .iter()
            .map(|step| step.token)
            .collect::<Vec<_>>();
        let generated_top2 = generated_steps
            .iter()
            .map(|step| step.runner_up_token)
            .collect::<Vec<_>>();
        let generated_top1_logits = generated_steps
            .iter()
            .map(|step| step.logit)
            .collect::<Vec<_>>();
        let generated_top2_logits = generated_steps
            .iter()
            .map(|step| step.runner_up_logit)
            .collect::<Vec<_>>();
        let steps = model.top_two_per_position(&prompt.ids);
        let top1 = steps.iter().map(|step| step.token).collect::<Vec<_>>();
        let top2 = steps
            .iter()
            .map(|step| step.runner_up_token)
            .collect::<Vec<_>>();
        let top1_logits = steps.iter().map(|step| step.logit).collect::<Vec<_>>();
        let top2_logits = steps
            .iter()
            .map(|step| step.runner_up_logit)
            .collect::<Vec<_>>();
        run.prompts.push(report::PromptRun {
            id: prompt.id.clone(),
            text_hex: prompt.text_hex.clone(),
            ids: prompt.ids.clone(),
            generated,
            top1,
            top1_logits,
            top2,
            top2_logits,
            generated_top2,
            generated_top1_logits,
            generated_top2_logits,
        });
        eprintln!(
            "  {}: {} prompt tokens, {} generated ({:.1}s)",
            prompt.id,
            prompt.ids.len(),
            MAX_NEW,
            started.elapsed().as_secs_f64()
        );
    }

    let corpus_tokens = options
        .get("corpus-tokens")
        .map(|value| value.parse::<usize>())
        .transpose()
        .map_err(|_| "--corpus-tokens must be an integer".to_string())?
        .unwrap_or(CORPUS_TOKENS);
    let eval_start = options
        .get("eval-start")
        .map(|value| value.parse::<usize>())
        .transpose()
        .map_err(|_| "--eval-start must be an integer".to_string())?
        .unwrap_or(CORPUS_EVAL_START);
    let corpus_offset = options
        .get("corpus-offset")
        .map(|value| value.parse::<usize>())
        .transpose()
        .map_err(|_| "--corpus-offset must be an integer".to_string())?
        .unwrap_or(0);
    run.corpus_tokens = corpus_tokens;
    run.corpus_eval_start = eval_start;
    run.corpus_window_offset = corpus_offset;
    run.corpus_slice_blake3 = options.get("corpus-blake3").cloned().unwrap_or_default();
    if !prompts_only {
        let corpus_path = required(options, "corpus")?;
        let corpus = read_u32_file(corpus_path)?;
        if corpus.len() < corpus_offset + corpus_tokens {
            return Err(format!(
                "corpus has {} tokens, need offset {} + {}",
                corpus.len(),
                corpus_offset,
                corpus_tokens
            ));
        }
        let corpus = &corpus[corpus_offset..corpus_offset + corpus_tokens];
        let (nll_sum, nll_count) = model.nll_sum(corpus, eval_start);
        run.corpus_nll_sum = nll_sum;
        run.corpus_nll_count = nll_count;
    }

    let out = required(options, "out")?;
    std::fs::write(out, run.to_tsv()).map_err(|err| format!("write {out}: {err}"))?;
    if prompts_only {
        eprintln!(
            "run {}: prompts only, total {:.1}s -> {out}",
            run.model,
            started.elapsed().as_secs_f64()
        );
    } else {
        eprintln!(
            "run {}: PPL {:.6} over {} targets, total {:.1}s -> {out}",
            run.model,
            run.perplexity(),
            run.corpus_nll_count,
            started.elapsed().as_secs_f64()
        );
    }
    Ok(())
}

fn cmd_control(options: &BTreeMap<String, String>) -> Result<(), String> {
    let candidate = read_run(required(options, "candidate")?)?;
    let reference = read_run(required(options, "reference")?)?;
    let comparison = report::compare(&candidate, &reference)?;
    let json = report::control_json(&comparison);
    if let Some(path) = options.get("out") {
        std::fs::write(path, &json).map_err(|err| format!("write {path}: {err}"))?;
    }
    print!("{json}");
    let pass = comparison.top1_agreement() >= 0.98 && comparison.ppl_relative_delta <= 0.01;
    if !pass {
        return Err(format!(
            "control C failed: top1 {:.4}% (bar 98%), PPL delta {:.4}% (bar 1%)",
            comparison.top1_agreement() * 100.0,
            comparison.ppl_relative_delta * 100.0
        ));
    }
    Ok(())
}

/// Dump the full 151,936-wide logit rows at the named positions.
///
/// Format (`QDLG`, little-endian): magic, version, n_vocab, row_count; then per
/// row a u32 name length, the prompt id bytes, a u32 position, and `n_vocab`
/// f32 logits. `ref/llama_drift.cpp --dump-logits` writes the same format, so a
/// byte-for-byte comparison is a plain array subtraction.
fn cmd_logits(options: &BTreeMap<String, String>) -> Result<(), String> {
    let (model, _vocab) = load_model(options)?;
    let prompts_text = std::fs::read_to_string(required(options, "prompts")?)
        .map_err(|err| format!("read prompts: {err}"))?;
    let input = RefRun::from_tsv(&prompts_text)?;
    let positions = read_positions(required(options, "positions")?)?;
    let rows = positions.values().map(Vec::len).sum::<usize>();
    let path = required(options, "out")?;
    let file = std::fs::File::create(path).map_err(|err| format!("create {path}: {err}"))?;
    let mut out = BufWriter::new(file);
    write_logit_header(&mut out, model.vocab_size as u32, rows as u32)?;
    for prompt in &input.prompts {
        let Some(wanted) = positions.get(&prompt.id) else {
            continue;
        };
        let found = model.logits_at_positions(&prompt.ids, wanted);
        if found.len() != wanted.len() {
            return Err(format!(
                "{}: wanted {} positions, found {}",
                prompt.id,
                wanted.len(),
                found.len()
            ));
        }
        for (index, logits) in found {
            write_logit_row(&mut out, &prompt.id, index, &logits)?;
        }
    }
    out.flush().map_err(|err| format!("flush {path}: {err}"))?;
    eprintln!("logits: {rows} rows of {} -> {path}", model.vocab_size);
    Ok(())
}

/// Dump every intermediate vector of one layer at one position, one named
/// vector per TSV line: `name<TAB>v0,...`.
fn cmd_debug_layer(options: &BTreeMap<String, String>) -> Result<(), String> {
    let (model, _vocab) = load_model(options)?;
    let prompts_text = std::fs::read_to_string(required(options, "prompts")?)
        .map_err(|err| format!("read prompts: {err}"))?;
    let input = RefRun::from_tsv(&prompts_text)?;
    let prompt_id = required(options, "prompt")?;
    let prompt = input
        .prompt(prompt_id)
        .ok_or_else(|| format!("no prompt {prompt_id:?}"))?;
    let index = required(options, "index")?
        .parse::<usize>()
        .map_err(|_| "--index must be an integer".to_string())?;
    let layer = required(options, "layer")?
        .parse::<usize>()
        .map_err(|_| "--layer must be an integer".to_string())?;
    let vectors = model.layer_debug_at(&prompt.ids, index, layer)?;
    let mut out = String::new();
    for (name, values) in vectors {
        out.push_str(&name);
        out.push('\t');
        for (i, value) in values.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&format!("{value:.9}"));
        }
        out.push('\n');
    }
    let path = required(options, "out")?;
    std::fs::write(path, out).map_err(|err| format!("write {path}: {err}"))?;
    Ok(())
}

/// Dump the residual-stream hidden state after the embedding and after every
/// transformer layer at the named positions. Format (`QDLH`, little-endian):
/// magic, version, hidden_size, layer_count+1, row_count; then per row a u32
/// prompt-id length, the id bytes, a u32 position, and every layer's f32 row.
fn cmd_hidden(options: &BTreeMap<String, String>) -> Result<(), String> {
    let (model, _vocab) = load_model(options)?;
    let prompts_text = std::fs::read_to_string(required(options, "prompts")?)
        .map_err(|err| format!("read prompts: {err}"))?;
    let input = RefRun::from_tsv(&prompts_text)?;
    let positions = read_positions(required(options, "positions")?)?;
    let rows = positions.values().map(Vec::len).sum::<usize>();
    let path = required(options, "out")?;
    let file = std::fs::File::create(path).map_err(|err| format!("create {path}: {err}"))?;
    let mut out = BufWriter::new(file);
    out.write_all(HIDDEN_DUMP_MAGIC)
        .and_then(|()| out.write_all(&1u32.to_le_bytes()))
        .and_then(|()| out.write_all(&(model.config.hidden_size as u32).to_le_bytes()))
        .and_then(|()| out.write_all(&((model.layer_count + 1) as u32).to_le_bytes()))
        .and_then(|()| out.write_all(&(rows as u32).to_le_bytes()))
        .map_err(|err| format!("write hidden header: {err}"))?;
    for prompt in &input.prompts {
        let Some(wanted) = positions.get(&prompt.id) else {
            continue;
        };
        for (index, layers) in model.hidden_per_layer_at_positions(&prompt.ids, wanted) {
            out.write_all(&(prompt.id.len() as u32).to_le_bytes())
                .and_then(|()| out.write_all(prompt.id.as_bytes()))
                .and_then(|()| out.write_all(&(index as u32).to_le_bytes()))
                .map_err(|err| format!("write hidden row: {err}"))?;
            for layer in &layers {
                let mut bytes = Vec::with_capacity(layer.len() * 4);
                for value in layer {
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
                out.write_all(&bytes)
                    .map_err(|err| format!("write hidden values: {err}"))?;
            }
        }
    }
    out.flush().map_err(|err| format!("flush {path}: {err}"))?;
    eprintln!(
        "hidden: {rows} rows x {} layers x {} -> {path}",
        model.layer_count + 1,
        model.config.hidden_size
    );
    Ok(())
}

fn read_positions(path: &str) -> Result<BTreeMap<String, Vec<usize>>, String> {
    let text = std::fs::read_to_string(path).map_err(|err| format!("read {path}: {err}"))?;
    let mut out: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (prompt, index) = line
            .split_once('\t')
            .ok_or_else(|| format!("{path}: expected <prompt>\\t<index>, got {line:?}"))?;
        let index = index
            .parse::<usize>()
            .map_err(|_| format!("{path}: bad position {index:?}"))?;
        out.entry(prompt.to_string()).or_default().push(index);
    }
    Ok(out)
}

fn write_logit_header(
    out: &mut BufWriter<std::fs::File>,
    n_vocab: u32,
    rows: u32,
) -> Result<(), String> {
    out.write_all(LOGIT_DUMP_MAGIC)
        .and_then(|()| out.write_all(&LOGIT_DUMP_VERSION.to_le_bytes()))
        .and_then(|()| out.write_all(&n_vocab.to_le_bytes()))
        .and_then(|()| out.write_all(&rows.to_le_bytes()))
        .map_err(|err| format!("write logit header: {err}"))
}

fn write_logit_row(
    out: &mut BufWriter<std::fs::File>,
    prompt: &str,
    index: usize,
    logits: &[f32],
) -> Result<(), String> {
    out.write_all(&(prompt.len() as u32).to_le_bytes())
        .and_then(|()| out.write_all(prompt.as_bytes()))
        .and_then(|()| out.write_all(&(index as u32).to_le_bytes()))
        .map_err(|err| format!("write logit row: {err}"))?;
    let mut bytes = Vec::with_capacity(logits.len() * 4);
    for logit in logits {
        bytes.extend_from_slice(&logit.to_le_bytes());
    }
    out.write_all(&bytes)
        .map_err(|err| format!("write logit values: {err}"))
}

fn cmd_compare(options: &BTreeMap<String, String>) -> Result<(), String> {
    let ours = read_run(required(options, "ours")?)?;
    let q4 = read_run(required(options, "q4")?)?;
    let fp32 = read_run(required(options, "fp32")?)?;
    let meta = read_meta(required(options, "meta")?)?;

    let ours_vs_q4 = report::compare(&ours, &q4)?;
    let q4_vs_fp32 = report::compare(&q4, &fp32)?;
    let ours_vs_fp32 = report::compare(&ours, &fp32)?;
    // The boot "reply" is the greedy stream up to the first <|im_end|>; tokens
    // after EOS are forced by the 32-token cap and are not part of the reply.
    let boot = |run: &RefRun| {
        run.prompt("boot_hello_world")
            .map(|prompt| {
                let end = prompt
                    .generated
                    .iter()
                    .position(|&token| token == 151_645)
                    .unwrap_or(prompt.generated.len());
                prompt.generated[..end].to_vec()
            })
            .ok_or_else(|| format!("{} lacks the boot prompt", run.model))
    };
    // Bar B is the SHIPPED model's quantization loss: ours (Q4_K_M) vs F32.
    // Bar A separately checks ours against llama.cpp's Q4_K_M; the llama Q4
    // vs F32 comparison is kept as a cross-check in the report.
    let boot_reply_equal = boot(&ours)? == boot(&fp32)?;
    let bars = report::bars(&ours_vs_q4, &ours_vs_fp32, boot_reply_equal);

    let kat_meta = KatMeta {
        date: meta.get("date").cloned().unwrap_or_default(),
        host: meta.get("host").cloned().unwrap_or_default(),
        neom_sha256: meta.get("neom_sha256").cloned().unwrap_or_default(),
        neom_blake3: meta.get("neom_blake3").cloned().unwrap_or_default(),
        gguf_sha256: meta.get("gguf_sha256").cloned().unwrap_or_default(),
        gguf_revision: meta.get("gguf_revision").cloned().unwrap_or_default(),
        fp32_sha256: meta.get("fp32_sha256").cloned().unwrap_or_default(),
        fp32_revision: meta.get("fp32_revision").cloned().unwrap_or_default(),
        llama_commit: meta.get("llama_commit").cloned().unwrap_or_default(),
        corpus_parquet_sha256: meta
            .get("corpus_parquet_sha256")
            .cloned()
            .unwrap_or_default(),
        corpus_text_sha256: meta.get("corpus_text_sha256").cloned().unwrap_or_default(),
        corpus_tokens: ours.corpus_tokens,
        corpus_eval_start: ours.corpus_eval_start,
        corpus_window_offset: ours.corpus_window_offset,
        corpus_slice_blake3: ours.corpus_slice_blake3.clone(),
    };

    let report_md = report::markdown(
        &ours,
        &q4,
        &fp32,
        &ours_vs_q4,
        &ours_vs_fp32,
        &q4_vs_fp32,
        &bars,
    );
    if let Some(path) = options.get("markdown-out") {
        std::fs::write(path, &report_md).map_err(|err| format!("write {path}: {err}"))?;
    }
    if let Some(path) = options.get("kat-out") {
        let kat = report::kat_json(
            &kat_meta,
            &ours,
            &q4,
            &fp32,
            &ours_vs_q4,
            &q4_vs_fp32,
            &ours_vs_fp32,
            &bars,
        );
        std::fs::write(path, kat).map_err(|err| format!("write {path}: {err}"))?;
    }
    print!("{report_md}");

    if !bars.a_pass() || !bars.b_pass() {
        return Err("acceptance bars failed (see report)".to_string());
    }
    Ok(())
}

/// Diagnostic: print the shipped `.vocab` tokenizer's IDs for a piece of text.
/// Used to compare it with the model's own GGUF tokenizer on the same bytes.
fn cmd_encode(options: &BTreeMap<String, String>) -> Result<(), String> {
    let neom_path = Path::new(required(options, "neom")?);
    let (model, vocab) = Model::load(neom_path)?;
    if let Some(ids) = options.get("ids") {
        let ids: Vec<u32> = ids
            .split(',')
            .map(|part| part.parse::<u32>().map_err(|_| format!("bad id {part:?}")))
            .collect::<Result<_, _>>()?;
        let decoded = vocab
            .tokenizer
            .decode(&ids)
            .map_err(|err| format!("decode: {err:?}"))?;
        println!("ids={ids:?}");
        println!("decoded={decoded:?}");
        return Ok(());
    }
    let text = required(options, "text")?;
    let ids = model.encode(&vocab, text)?;
    let decoded = vocab
        .tokenizer
        .decode(&ids)
        .map_err(|err| format!("decode: {err:?}"))?;
    println!("ids={ids:?}");
    println!("decoded={decoded:?}");
    Ok(())
}

fn read_run(path: &str) -> Result<RefRun, String> {
    let text = std::fs::read_to_string(path).map_err(|err| format!("read {path}: {err}"))?;
    let mut run = RefRun::from_tsv(&text)?;
    if run.model.is_empty() {
        run.model = path.to_string();
    }
    Ok(run)
}

fn read_meta(path: &str) -> Result<BTreeMap<String, String>, String> {
    let text = std::fs::read_to_string(path).map_err(|err| format!("read {path}: {err}"))?;
    let mut out = BTreeMap::new();
    for line in text.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            out.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    Ok(out)
}

fn read_u32_file(path: &str) -> Result<Vec<u32>, String> {
    let bytes = std::fs::read(path).map_err(|err| format!("read {path}: {err}"))?;
    if bytes.len() % 4 != 0 {
        return Err(format!("{path} is not a u32 token file"));
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
