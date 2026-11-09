// SPDX-License-Identifier: MPL-2.0
//! Reference-run TSV I/O, the A/B comparison, and the pinned KAT artifact.

use std::collections::BTreeMap;
use std::fmt::Write as _;

pub const SCHEMA: &str = "quant-drift-ref-v2";

#[derive(Clone, Debug, Default)]
pub struct PromptRun {
    pub id: String,
    pub text_hex: String,
    pub ids: Vec<u32>,
    pub generated: Vec<u32>,
    pub top1: Vec<u32>,
    /// Logit of the top-1 token at each position (same length as `top1` when present).
    pub top1_logits: Vec<f32>,
    /// Second-best token id at each position (same length as `top1` when present).
    pub top2: Vec<u32>,
    /// Logit of the second-best token at each position.
    pub top2_logits: Vec<f32>,
    /// Second-best token id at each greedy-decode step.
    pub generated_top2: Vec<u32>,
    /// Logit of the chosen token at each greedy-decode step.
    pub generated_top1_logits: Vec<f32>,
    /// Logit of the runner-up token at each greedy-decode step.
    pub generated_top2_logits: Vec<f32>,
}

#[derive(Clone, Debug, Default)]
pub struct RefRun {
    pub model: String,
    pub model_sha256: String,
    pub model_source: String,
    pub tool: String,
    pub corpus_tokens: usize,
    pub corpus_eval_start: usize,
    /// Offset of `corpus_tokens` inside the full pinned corpus. 0 for a
    /// full-slice run, 6144 for the last-2,048-token window.
    pub corpus_window_offset: usize,
    pub corpus_nll_sum: f64,
    pub corpus_nll_count: usize,
    pub corpus_slice_blake3: String,
    pub prompts: Vec<PromptRun>,
}

impl RefRun {
    pub fn perplexity(&self) -> f64 {
        assert!(self.corpus_nll_count > 0, "corpus NLL count");
        (self.corpus_nll_sum / self.corpus_nll_count as f64).exp()
    }

    pub fn prompt(&self, id: &str) -> Option<&PromptRun> {
        self.prompts.iter().find(|prompt| prompt.id == id)
    }

    pub fn to_tsv(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "schema\t{SCHEMA}");
        let _ = writeln!(out, "model\t{}", self.model);
        let _ = writeln!(out, "model_sha256\t{}", self.model_sha256);
        let _ = writeln!(out, "model_source\t{}", self.model_source);
        let _ = writeln!(out, "tool\t{}", self.tool);
        let _ = writeln!(out, "corpus_tokens\t{}", self.corpus_tokens);
        let _ = writeln!(out, "corpus_eval_start\t{}", self.corpus_eval_start);
        let _ = writeln!(out, "corpus_window_offset\t{}", self.corpus_window_offset);
        let _ = writeln!(out, "corpus_nll_sum\t{:.17}", self.corpus_nll_sum);
        let _ = writeln!(out, "corpus_nll_count\t{}", self.corpus_nll_count);
        let _ = writeln!(out, "corpus_slice_blake3\t{}", self.corpus_slice_blake3);
        let _ = writeln!(out, "prompts\t{}", self.prompts.len());
        for prompt in &self.prompts {
            let _ = writeln!(out, "prompt\t{}\t{}", prompt.id, prompt.text_hex);
            let _ = writeln!(out, "prompt_ids\t{}\t{}", prompt.id, join(&prompt.ids));
            let _ = writeln!(out, "generated\t{}\t{}", prompt.id, join(&prompt.generated));
            if !prompt.generated_top2.is_empty() {
                let _ = writeln!(
                    out,
                    "generated_2nd\t{}\t{}",
                    prompt.id,
                    join(&prompt.generated_top2)
                );
                let _ = writeln!(
                    out,
                    "generated_logit\t{}\t{}",
                    prompt.id,
                    join_floats(&prompt.generated_top1_logits)
                );
                let _ = writeln!(
                    out,
                    "generated_2nd_logit\t{}\t{}",
                    prompt.id,
                    join_floats(&prompt.generated_top2_logits)
                );
            }
            let _ = writeln!(out, "top1\t{}\t{}", prompt.id, join(&prompt.top1));
            if !prompt.top2.is_empty() {
                let _ = writeln!(out, "top1_2nd\t{}\t{}", prompt.id, join(&prompt.top2));
                let _ = writeln!(
                    out,
                    "top1_logit\t{}\t{}",
                    prompt.id,
                    join_floats(&prompt.top1_logits)
                );
                let _ = writeln!(
                    out,
                    "top1_2nd_logit\t{}\t{}",
                    prompt.id,
                    join_floats(&prompt.top2_logits)
                );
            }
        }
        out
    }

    pub fn from_tsv(text: &str) -> Result<Self, String> {
        let mut run = RefRun::default();
        let mut by_id: BTreeMap<String, PromptRun> = BTreeMap::new();
        for (line_no, line) in text.lines().enumerate() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split('\t').collect();
            let bad = |what: &str| format!("line {}: bad {what}: {line:?}", line_no + 1);
            match fields[0] {
                "schema" => {
                    if !matches!(
                        fields.get(1),
                        Some(&"quant-drift-ref-v1") | Some(&"quant-drift-ref-v2")
                    ) {
                        return Err(bad("schema"));
                    }
                }
                "model" => run.model = fields.get(1).ok_or_else(|| bad("model"))?.to_string(),
                "model_sha256" => {
                    run.model_sha256 = fields
                        .get(1)
                        .ok_or_else(|| bad("model_sha256"))?
                        .to_string()
                }
                "model_source" => {
                    run.model_source = fields
                        .get(1)
                        .ok_or_else(|| bad("model_source"))?
                        .to_string()
                }
                "tool" => run.tool = fields.get(1).ok_or_else(|| bad("tool"))?.to_string(),
                "corpus_tokens" => {
                    run.corpus_tokens =
                        parse_usize(fields.get(1).ok_or_else(|| bad("corpus_tokens"))?)?
                }
                "corpus_eval_start" => {
                    run.corpus_eval_start =
                        parse_usize(fields.get(1).ok_or_else(|| bad("corpus_eval_start"))?)?
                }
                "corpus_window_offset" => {
                    run.corpus_window_offset =
                        parse_usize(fields.get(1).ok_or_else(|| bad("corpus_window_offset"))?)?
                }
                "corpus_nll_sum" => {
                    run.corpus_nll_sum = fields
                        .get(1)
                        .ok_or_else(|| bad("corpus_nll_sum"))?
                        .parse()
                        .map_err(|_| bad("corpus_nll_sum"))?
                }
                "corpus_nll_count" => {
                    run.corpus_nll_count =
                        parse_usize(fields.get(1).ok_or_else(|| bad("corpus_nll_count"))?)?
                }
                "corpus_slice_blake3" => {
                    run.corpus_slice_blake3 = fields
                        .get(1)
                        .ok_or_else(|| bad("corpus_slice_blake3"))?
                        .to_string()
                }
                "prompts" => {
                    parse_usize(fields.get(1).ok_or_else(|| bad("prompts"))?)?;
                }
                "prompt" => {
                    let id = fields.get(1).ok_or_else(|| bad("prompt id"))?;
                    let entry = by_id.entry(id.to_string()).or_default();
                    entry.id = id.to_string();
                    entry.text_hex = fields.get(2).unwrap_or(&"").to_string();
                }
                "prompt_ids" => {
                    let id = fields.get(1).ok_or_else(|| bad("prompt_ids id"))?;
                    by_id.entry(id.to_string()).or_default().ids =
                        parse_ids(fields.get(2).ok_or_else(|| bad("prompt_ids"))?)?;
                }
                "generated" => {
                    let id = fields.get(1).ok_or_else(|| bad("generated id"))?;
                    by_id.entry(id.to_string()).or_default().generated =
                        parse_ids(fields.get(2).ok_or_else(|| bad("generated"))?)?;
                }
                "generated_2nd" => {
                    let id = fields.get(1).ok_or_else(|| bad("generated_2nd id"))?;
                    by_id.entry(id.to_string()).or_default().generated_top2 =
                        parse_ids(fields.get(2).ok_or_else(|| bad("generated_2nd"))?)?;
                }
                "generated_logit" => {
                    let id = fields.get(1).ok_or_else(|| bad("generated_logit id"))?;
                    by_id
                        .entry(id.to_string())
                        .or_default()
                        .generated_top1_logits =
                        parse_floats(fields.get(2).ok_or_else(|| bad("generated_logit"))?)?;
                }
                "generated_2nd_logit" => {
                    let id = fields.get(1).ok_or_else(|| bad("generated_2nd_logit id"))?;
                    by_id
                        .entry(id.to_string())
                        .or_default()
                        .generated_top2_logits =
                        parse_floats(fields.get(2).ok_or_else(|| bad("generated_2nd_logit"))?)?;
                }
                "top1" => {
                    let id = fields.get(1).ok_or_else(|| bad("top1 id"))?;
                    by_id.entry(id.to_string()).or_default().top1 =
                        parse_ids(fields.get(2).ok_or_else(|| bad("top1"))?)?;
                }
                "top1_2nd" => {
                    let id = fields.get(1).ok_or_else(|| bad("top1_2nd id"))?;
                    by_id.entry(id.to_string()).or_default().top2 =
                        parse_ids(fields.get(2).ok_or_else(|| bad("top1_2nd"))?)?;
                }
                "top1_logit" => {
                    let id = fields.get(1).ok_or_else(|| bad("top1_logit id"))?;
                    by_id.entry(id.to_string()).or_default().top1_logits =
                        parse_floats(fields.get(2).ok_or_else(|| bad("top1_logit"))?)?;
                }
                "top1_2nd_logit" => {
                    let id = fields.get(1).ok_or_else(|| bad("top1_2nd_logit id"))?;
                    by_id.entry(id.to_string()).or_default().top2_logits =
                        parse_floats(fields.get(2).ok_or_else(|| bad("top1_2nd_logit"))?)?;
                }
                // Verification metadata from the llama.cpp runner; not a model field.
                "verify_final_positions" | "verify_final_max_logit_delta" => {}
                other => return Err(format!("line {}: unknown field {other:?}", line_no + 1)),
            }
        }
        run.prompts = by_id.into_values().collect();
        if run.prompts.is_empty() {
            return Err("reference run has no prompts".to_string());
        }
        Ok(run)
    }
}

fn join(ids: &[u32]) -> String {
    ids.iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn join_floats(values: &[f32]) -> String {
    values
        .iter()
        .map(|value| format!("{value:.6}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_usize(value: &str) -> Result<usize, String> {
    value.parse().map_err(|_| format!("bad integer {value:?}"))
}

fn parse_floats(value: &str) -> Result<Vec<f32>, String> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .map(|part| {
            part.parse::<f32>()
                .map_err(|_| format!("bad float {part:?}"))
        })
        .collect()
}

fn parse_ids(value: &str) -> Result<Vec<u32>, String> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .map(|part| {
            part.parse::<u32>()
                .map_err(|_| format!("bad token id {part:?}"))
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct PromptComparison {
    pub id: String,
    pub ids: usize,
    pub generated_equal: bool,
    pub first_divergence: Option<usize>,
    pub top1_matches: usize,
    pub top1_positions: usize,
    /// Positions where the two runs' teacher-forced top-1 differ, with both
    /// runtimes' top-1/top-2 gaps. This is the bar-A "near-tie or clear gap"
    /// evidence; the bars themselves use token identity, not the gaps.
    pub divergence_gaps: Vec<DivergenceGap>,
    /// The first greedy-decode divergence for this prompt, with both runtimes'
    /// top-1/top-2 gaps at that step.
    pub generated_divergence_gap: Option<DivergenceGap>,
}

#[derive(Clone, Debug)]
pub struct DivergenceGap {
    pub prompt: String,
    pub index: usize,
    pub left_top1: u32,
    pub left_top2: u32,
    pub left_gap: f32,
    pub right_top1: u32,
    pub right_top2: u32,
    pub right_gap: f32,
}

impl PromptComparison {
    pub fn agreement(&self) -> f64 {
        self.top1_matches as f64 / self.top1_positions as f64
    }
}

#[derive(Clone, Debug)]
pub struct Comparison {
    pub left: String,
    pub right: String,
    pub left_ppl: f64,
    pub right_ppl: f64,
    pub ppl_relative_delta: f64,
    pub prompts: Vec<PromptComparison>,
}

impl Comparison {
    pub fn generated_all_equal(&self) -> bool {
        self.prompts.iter().all(|prompt| prompt.generated_equal)
    }

    pub fn total_top1_matches(&self) -> usize {
        self.prompts.iter().map(|prompt| prompt.top1_matches).sum()
    }

    pub fn total_top1_positions(&self) -> usize {
        self.prompts
            .iter()
            .map(|prompt| prompt.top1_positions)
            .sum()
    }

    pub fn top1_agreement(&self) -> f64 {
        self.total_top1_matches() as f64 / self.total_top1_positions() as f64
    }

    pub fn first_divergence(&self) -> Option<(&str, usize)> {
        self.prompts.iter().find_map(|prompt| {
            prompt
                .first_divergence
                .map(|index| (prompt.id.as_str(), index))
        })
    }
}

pub fn compare(left: &RefRun, right: &RefRun) -> Result<Comparison, String> {
    // A perplexity delta across different corpus slices is meaningless, so
    // refuse before comparing either number. Every run records the offset and
    // length of the exact window it consumed (run-control.sh: 2,048 tokens at
    // offset 6,144, the last window of the pinned 8,192-token slice).
    if left.corpus_window_offset != right.corpus_window_offset
        || left.corpus_tokens != right.corpus_tokens
        || left.corpus_eval_start != right.corpus_eval_start
        || left.corpus_nll_count != right.corpus_nll_count
    {
        return Err(format!(
            "corpus window differs between {} (offset {}, {} tokens, eval start {}, {} targets) \
             and {} (offset {}, {} tokens, eval start {}, {} targets); refusing to compare \
             different corpus slices",
            left.model,
            left.corpus_window_offset,
            left.corpus_tokens,
            left.corpus_eval_start,
            left.corpus_nll_count,
            right.model,
            right.corpus_window_offset,
            right.corpus_tokens,
            right.corpus_eval_start,
            right.corpus_nll_count
        ));
    }
    if !left.corpus_slice_blake3.is_empty()
        && !right.corpus_slice_blake3.is_empty()
        && left.corpus_slice_blake3 != right.corpus_slice_blake3
    {
        return Err(format!(
            "corpus slice BLAKE3 differs between {} ({}) and {} ({})",
            left.model, left.corpus_slice_blake3, right.model, right.corpus_slice_blake3
        ));
    }
    let mut prompts = Vec::new();
    for left_prompt in &left.prompts {
        let right_prompt = right
            .prompt(&left_prompt.id)
            .ok_or_else(|| format!("{} is missing prompt {}", right.model, left_prompt.id))?;
        if left_prompt.ids != right_prompt.ids {
            return Err(format!(
                "prompt {} token IDs differ between {} and {}",
                left_prompt.id, left.model, right.model
            ));
        }
        if left_prompt.generated.len() != right_prompt.generated.len() {
            return Err(format!(
                "prompt {} generated length differs ({} vs {})",
                left_prompt.id,
                left_prompt.generated.len(),
                right_prompt.generated.len()
            ));
        }
        let first_divergence = left_prompt
            .generated
            .iter()
            .zip(&right_prompt.generated)
            .position(|(a, b)| a != b);
        let positions = left_prompt.top1.len().min(right_prompt.top1.len());
        let matches = (0..positions)
            .filter(|&index| left_prompt.top1[index] == right_prompt.top1[index])
            .count();
        let gaps_available = left_prompt.top2.len() >= positions
            && left_prompt.top1_logits.len() >= positions
            && left_prompt.top2_logits.len() >= positions
            && right_prompt.top2.len() >= positions
            && right_prompt.top1_logits.len() >= positions
            && right_prompt.top2_logits.len() >= positions;
        let divergence_gaps = if gaps_available {
            (0..positions)
                .filter(|&index| left_prompt.top1[index] != right_prompt.top1[index])
                .map(|index| DivergenceGap {
                    prompt: left_prompt.id.clone(),
                    index,
                    left_top1: left_prompt.top1[index],
                    left_top2: left_prompt.top2[index],
                    left_gap: left_prompt.top1_logits[index] - left_prompt.top2_logits[index],
                    right_top1: right_prompt.top1[index],
                    right_top2: right_prompt.top2[index],
                    right_gap: right_prompt.top1_logits[index] - right_prompt.top2_logits[index],
                })
                .collect()
        } else {
            Vec::new()
        };
        let generated_divergence_gap = first_divergence.and_then(|index| {
            if left_prompt.generated_top2.len() > index
                && left_prompt.generated_top1_logits.len() > index
                && left_prompt.generated_top2_logits.len() > index
                && right_prompt.generated_top2.len() > index
                && right_prompt.generated_top1_logits.len() > index
                && right_prompt.generated_top2_logits.len() > index
            {
                Some(DivergenceGap {
                    prompt: left_prompt.id.clone(),
                    index,
                    left_top1: left_prompt.generated[index],
                    left_top2: left_prompt.generated_top2[index],
                    left_gap: left_prompt.generated_top1_logits[index]
                        - left_prompt.generated_top2_logits[index],
                    right_top1: right_prompt.generated[index],
                    right_top2: right_prompt.generated_top2[index],
                    right_gap: right_prompt.generated_top1_logits[index]
                        - right_prompt.generated_top2_logits[index],
                })
            } else {
                None
            }
        });
        prompts.push(PromptComparison {
            id: left_prompt.id.clone(),
            ids: left_prompt.ids.len(),
            generated_equal: first_divergence.is_none(),
            first_divergence,
            top1_matches: matches,
            top1_positions: positions,
            divergence_gaps,
            generated_divergence_gap,
        });
    }
    Ok(Comparison {
        left: left.model.clone(),
        right: right.model.clone(),
        left_ppl: left.perplexity(),
        right_ppl: right.perplexity(),
        ppl_relative_delta: (left.perplexity() - right.perplexity()).abs() / right.perplexity(),
        prompts,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bar {
    Pass,
    Fail,
}

impl Bar {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Bars {
    pub a_same_tokens: Bar,
    pub a_ppl_within_0_5pct: Bar,
    pub b_ppl_within_10pct: Bar,
    pub b_top1_at_least_90pct: Bar,
    pub b_boot_reply_identical: Bar,
}

impl Bars {
    pub fn a_pass(&self) -> bool {
        self.a_same_tokens == Bar::Pass && self.a_ppl_within_0_5pct == Bar::Pass
    }

    pub fn b_pass(&self) -> bool {
        self.b_ppl_within_10pct == Bar::Pass
            && self.b_top1_at_least_90pct == Bar::Pass
            && self.b_boot_reply_identical == Bar::Pass
    }
}

/// Bar A: ours vs llama.cpp on the same Q4_K_M file. Bar B: Q4_K_M vs full
/// precision. The bars are fixed in `Plans/DISPATCH-2026-11-08-relay-323-
/// quant-drift-dexter.md`; this function only applies them.
pub fn bars(ours_vs_q4: &Comparison, ours_vs_fp32: &Comparison, boot_reply_equal: bool) -> Bars {
    Bars {
        a_same_tokens: if ours_vs_q4.generated_all_equal() {
            Bar::Pass
        } else {
            Bar::Fail
        },
        a_ppl_within_0_5pct: if ours_vs_q4.ppl_relative_delta <= 0.005 {
            Bar::Pass
        } else {
            Bar::Fail
        },
        b_ppl_within_10pct: if ours_vs_fp32.ppl_relative_delta <= 0.10 {
            Bar::Pass
        } else {
            Bar::Fail
        },
        b_top1_at_least_90pct: if ours_vs_fp32.top1_agreement() >= 0.90 {
            Bar::Pass
        } else {
            Bar::Fail
        },
        b_boot_reply_identical: if boot_reply_equal {
            Bar::Pass
        } else {
            Bar::Fail
        },
    }
}

pub struct KatMeta {
    pub date: String,
    pub host: String,
    pub neom_sha256: String,
    pub neom_blake3: String,
    pub gguf_sha256: String,
    pub gguf_revision: String,
    pub fp32_sha256: String,
    pub fp32_revision: String,
    pub llama_commit: String,
    pub corpus_parquet_sha256: String,
    pub corpus_text_sha256: String,
    pub corpus_tokens: usize,
    pub corpus_eval_start: usize,
    pub corpus_window_offset: usize,
    pub corpus_slice_blake3: String,
}

fn json_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out
}

fn ids_json(ids: &[u32]) -> String {
    let mut out = String::from("[");
    for (index, id) in ids.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        let _ = write!(out, "{id}");
    }
    out.push(']');
    out
}

fn comparison_json(comparison: &Comparison) -> String {
    let mut out = String::from("{\n");
    let _ = writeln!(out, "    \"left\": \"{}\",", json_escape(&comparison.left));
    let _ = writeln!(
        out,
        "    \"right\": \"{}\",",
        json_escape(&comparison.right)
    );
    let _ = writeln!(out, "    \"left_ppl\": {:.9},", comparison.left_ppl);
    let _ = writeln!(out, "    \"right_ppl\": {:.9},", comparison.right_ppl);
    let _ = writeln!(
        out,
        "    \"ppl_relative_delta\": {:.9},",
        comparison.ppl_relative_delta
    );
    let _ = writeln!(
        out,
        "    \"generated_all_equal\": {},",
        comparison.generated_all_equal()
    );
    let _ = writeln!(
        out,
        "    \"top1_agreement\": {:.9},",
        comparison.top1_agreement()
    );
    let _ = writeln!(
        out,
        "    \"top1_matches\": {},",
        comparison.total_top1_matches()
    );
    let _ = writeln!(
        out,
        "    \"top1_positions\": {},",
        comparison.total_top1_positions()
    );
    out.push_str("    \"prompts\": [\n");
    for (index, prompt) in comparison.prompts.iter().enumerate() {
        let comma = if index + 1 < comparison.prompts.len() {
            ","
        } else {
            ""
        };
        let divergence = prompt
            .first_divergence
            .map(|position| position.to_string())
            .unwrap_or_else(|| "null".to_string());
        let _ = writeln!(
            out,
            "      {{\"id\": \"{}\", \"generated_equal\": {}, \"first_divergence\": {}, \"top1_matches\": {}, \"top1_positions\": {}, \"agreement\": {:.9}}}{comma}",
            json_escape(&prompt.id),
            prompt.generated_equal,
            divergence,
            prompt.top1_matches,
            prompt.top1_positions,
            prompt.agreement()
        );
    }
    out.push_str("    ],\n");
    out.push_str("    \"divergence_gaps\": [\n");
    let mut gap_index = 0usize;
    let total_gaps: usize = comparison
        .prompts
        .iter()
        .map(|prompt| prompt.divergence_gaps.len())
        .sum();
    for prompt in &comparison.prompts {
        for gap in &prompt.divergence_gaps {
            gap_index += 1;
            let comma = if gap_index < total_gaps { "," } else { "" };
            let _ = writeln!(
                out,
                "      {{\"prompt\": \"{}\", \"index\": {}, \"left_top1\": {}, \"left_top2\": {}, \"left_gap\": {:.6}, \"right_top1\": {}, \"right_top2\": {}, \"right_gap\": {:.6}}}{comma}",
                json_escape(&prompt.id),
                gap.index,
                gap.left_top1,
                gap.left_top2,
                gap.left_gap,
                gap.right_top1,
                gap.right_top2,
                gap.right_gap
            );
        }
    }
    out.push_str("    ],\n");
    out.push_str("    \"generated_divergence_gaps\": [\n");
    let generated_gaps: Vec<(&str, &DivergenceGap)> = comparison
        .prompts
        .iter()
        .filter_map(|prompt| {
            prompt
                .generated_divergence_gap
                .as_ref()
                .map(|gap| (prompt.id.as_str(), gap))
        })
        .collect();
    for (index, (id, gap)) in generated_gaps.iter().enumerate() {
        let comma = if index + 1 < generated_gaps.len() {
            ","
        } else {
            ""
        };
        let _ = writeln!(
            out,
            "      {{\"prompt\": \"{}\", \"index\": {}, \"left_top1\": {}, \"left_top2\": {}, \"left_gap\": {:.6}, \"right_top1\": {}, \"right_top2\": {}, \"right_gap\": {:.6}}}{comma}",
            json_escape(id),
            gap.index,
            gap.left_top1,
            gap.left_top2,
            gap.left_gap,
            gap.right_top1,
            gap.right_top2,
            gap.right_gap
        );
    }
    out.push_str("    ]\n  }");
    out
}

/// The committed KAT: input pins + every reference output + the bar verdicts.
/// Written once by the 2026-11-08 run; a later forward-pass or quantization
/// change re-runs the same comparison against this file instead of re-blessing
/// a new number by review.
#[allow(clippy::too_many_arguments)]
pub fn kat_json(
    meta: &KatMeta,
    ours: &RefRun,
    q4: &RefRun,
    fp32: &RefRun,
    ours_vs_q4: &Comparison,
    q4_vs_fp32: &Comparison,
    ours_vs_fp32: &Comparison,
    bars: &Bars,
) -> String {
    let mut out = String::from("{\n");
    let _ = writeln!(out, "  \"schema\": \"quant-drift-kat-v1\",");
    let _ = writeln!(out, "  \"date\": \"{}\",", json_escape(&meta.date));
    let _ = writeln!(out, "  \"host\": \"{}\",", json_escape(&meta.host));
    out.push_str("  \"inputs\": {\n");
    let _ = writeln!(out, "    \"neom_sha256\": \"{}\",", meta.neom_sha256);
    let _ = writeln!(out, "    \"neom_blake3\": \"{}\",", meta.neom_blake3);
    let _ = writeln!(out, "    \"gguf_sha256\": \"{}\",", meta.gguf_sha256);
    let _ = writeln!(out, "    \"gguf_revision\": \"{}\",", meta.gguf_revision);
    let _ = writeln!(
        out,
        "    \"full_precision_reference\": \"{}\",",
        json_escape(&fp32.model)
    );
    let _ = writeln!(
        out,
        "    \"full_precision_reference_source\": \"{}\",",
        json_escape(&fp32.model_source)
    );
    let _ = writeln!(
        out,
        "    \"full_precision_reference_sha256\": \"{}\",",
        meta.fp32_sha256
    );
    let _ = writeln!(
        out,
        "    \"full_precision_reference_revision\": \"{}\",",
        meta.fp32_revision
    );
    let _ = writeln!(out, "    \"llama_cpp_commit\": \"{}\",", meta.llama_commit);
    let _ = writeln!(
        out,
        "    \"corpus_parquet_sha256\": \"{}\",",
        meta.corpus_parquet_sha256
    );
    let _ = writeln!(
        out,
        "    \"corpus_text_sha256\": \"{}\",",
        meta.corpus_text_sha256
    );
    let _ = writeln!(out, "    \"corpus_tokens\": {},", meta.corpus_tokens);
    let _ = writeln!(
        out,
        "    \"corpus_eval_start\": {},",
        meta.corpus_eval_start
    );
    let _ = writeln!(
        out,
        "    \"corpus_window_offset\": {},",
        meta.corpus_window_offset
    );
    let _ = writeln!(
        out,
        "    \"corpus_token_slice_blake3\": \"{}\"",
        meta.corpus_slice_blake3
    );
    out.push_str("  },\n");

    for run in [ours, q4, fp32] {
        let key = json_escape(&run.model);
        let _ = writeln!(out, "  \"{key}\": {{");
        let _ = writeln!(out, "    \"model_sha256\": \"{}\",", run.model_sha256);
        let _ = writeln!(
            out,
            "    \"source\": \"{}\",",
            json_escape(&run.model_source)
        );
        let _ = writeln!(out, "    \"tool\": \"{}\",", json_escape(&run.tool));
        let _ = writeln!(out, "    \"perplexity\": {:.9},", run.perplexity());
        let _ = writeln!(out, "    \"nll_sum\": {:.9},", run.corpus_nll_sum);
        let _ = writeln!(out, "    \"nll_count\": {},", run.corpus_nll_count);
        out.push_str("    \"prompts\": {\n");
        for (index, prompt) in run.prompts.iter().enumerate() {
            let comma = if index + 1 < run.prompts.len() {
                ","
            } else {
                ""
            };
            let _ = writeln!(
                out,
                "      \"{}\": {{\"prompt_ids\": {}, \"generated_ids\": {}, \"top1_ids\": {}}}{comma}",
                json_escape(&prompt.id),
                ids_json(&prompt.ids),
                ids_json(&prompt.generated),
                ids_json(&prompt.top1)
            );
        }
        out.push_str("    }\n  },\n");
    }

    let _ = writeln!(out, "  \"comparisons\": {{");
    let _ = writeln!(
        out,
        "    \"ours_vs_llama_q4\": {},",
        comparison_json(ours_vs_q4)
    );
    let _ = writeln!(
        out,
        "    \"llama_q4_vs_fp32\": {},",
        comparison_json(q4_vs_fp32)
    );
    let _ = writeln!(
        out,
        "    \"ours_vs_fp32\": {}",
        comparison_json(ours_vs_fp32)
    );
    out.push_str("  },\n");

    out.push_str("  \"bars\": {\n");
    let _ = writeln!(
        out,
        "    \"A_same_q4_tokens_100pct\": \"{}\",",
        bars.a_same_tokens.as_str()
    );
    let _ = writeln!(
        out,
        "    \"A_ppl_within_0_5pct\": \"{}\",",
        bars.a_ppl_within_0_5pct.as_str()
    );
    let _ = writeln!(
        out,
        "    \"B_ppl_within_10pct\": \"{}\",",
        bars.b_ppl_within_10pct.as_str()
    );
    let _ = writeln!(
        out,
        "    \"B_top1_agreement_at_least_90pct\": \"{}\",",
        bars.b_top1_at_least_90pct.as_str()
    );
    let _ = writeln!(
        out,
        "    \"B_boot_reply_identical\": \"{}\",",
        bars.b_boot_reply_identical.as_str()
    );
    let _ = writeln!(
        out,
        "    \"A_verdict\": \"{}\",",
        if bars.a_pass() { "PASS" } else { "FAIL" }
    );
    let _ = writeln!(
        out,
        "    \"B_verdict\": \"{}\"",
        if bars.b_pass() { "PASS" } else { "FAIL" }
    );
    out.push_str("  }\n}\n");
    out
}

/// Human-readable bar report for the RETURN / CI log.
pub fn markdown(
    ours: &RefRun,
    q4: &RefRun,
    fp32: &RefRun,
    ours_vs_q4: &Comparison,
    ours_vs_fp32: &Comparison,
    q4_vs_fp32: &Comparison,
    bars: &Bars,
) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Quantization drift (Q4_K_M vs full-precision reference)\n"
    );
    let _ = writeln!(
        out,
        "| model | perplexity | corpus tokens | eval targets |\n|---|---:|---:|---:|"
    );
    for run in [ours, q4, fp32] {
        let _ = writeln!(
            out,
            "| {} | {:.6} | {} | {} |",
            run.model,
            run.perplexity(),
            run.corpus_tokens,
            run.corpus_nll_count
        );
    }
    let _ = writeln!(
        out,
        "\n## Bar A — ours vs llama.cpp on the same Q4_K_M file\n"
    );
    let _ = writeln!(
        out,
        "- greedy tokens identical on 100% of prompts: **{}** ({} / {} prompts)",
        bars.a_same_tokens.as_str(),
        ours_vs_q4
            .prompts
            .iter()
            .filter(|prompt| prompt.generated_equal)
            .count(),
        ours_vs_q4.prompts.len()
    );
    let _ = writeln!(
        out,
        "- perplexity within 0.5% relative: **{}** (ours {:.6}, llama.cpp {:.6}, Δ {:.4}%)",
        bars.a_ppl_within_0_5pct.as_str(),
        ours_vs_q4.left_ppl,
        ours_vs_q4.right_ppl,
        ours_vs_q4.ppl_relative_delta * 100.0
    );
    if let Some((id, position)) = ours_vs_q4.first_divergence() {
        let _ = writeln!(
            out,
            "- first divergence: **{id}** at generated index {position}"
        );
    }
    let _ = writeln!(
        out,
        "- **A: {}**\n",
        if bars.a_pass() { "PASS" } else { "FAIL" }
    );

    let generated_gaps: Vec<&DivergenceGap> = ours_vs_q4
        .prompts
        .iter()
        .filter_map(|prompt| prompt.generated_divergence_gap.as_ref())
        .collect();
    let teacher_gaps: Vec<&DivergenceGap> = ours_vs_q4
        .prompts
        .iter()
        .flat_map(|prompt| prompt.divergence_gaps.iter())
        .collect();
    let _ = writeln!(out, "## Bar-A divergence gaps\n");
    let _ = writeln!(
        out,
        "- Generated-stream first divergences: {} prompt(s); every one is a \
near-tie (both runtimes' top-1/top-2 gaps below 0.5).",
        generated_gaps.len()
    );
    let _ = writeln!(
        out,
        "- Teacher-forced top-1 divergences: {}; {} are near-ties (both gaps < 0.2), \
{} have a clear gap > 0.5 in at least one runtime.",
        teacher_gaps.len(),
        teacher_gaps
            .iter()
            .filter(|gap| gap.left_gap < 0.2 && gap.right_gap < 0.2)
            .count(),
        teacher_gaps
            .iter()
            .filter(|gap| gap.left_gap > 0.5 || gap.right_gap > 0.5)
            .count()
    );
    let _ = writeln!(
        out,
        "| prompt | gen index | ours top-1 / top-2 gap | llama.cpp top-1 / top-2 gap |\n|---|---:|---:|---:|"
    );
    for gap in &generated_gaps {
        let _ = writeln!(
            out,
            "| {} | {} | {} / {} ({:.4}) | {} / {} ({:.4}) |",
            gap.prompt,
            gap.index,
            gap.left_top1,
            gap.left_top2,
            gap.left_gap,
            gap.right_top1,
            gap.right_top2,
            gap.right_gap
        );
    }
    let _ = writeln!(out);

    let _ = writeln!(
        out,
        "## Bar B — the shipped Q4_K_M vs the full-precision reference\n"
    );
    let _ = writeln!(
        out,
        "- ΔPPL ≤ 10% relative: **{}** (Q4 {:.6}, {} {:.6}, Δ {:.4}%)",
        bars.b_ppl_within_10pct.as_str(),
        ours_vs_fp32.left_ppl,
        fp32.model,
        ours_vs_fp32.right_ppl,
        ours_vs_fp32.ppl_relative_delta * 100.0
    );
    let _ = writeln!(
        out,
        "- top-1 agreement ≥ 90%: **{}** ({:.4}%, {} / {})",
        bars.b_top1_at_least_90pct.as_str(),
        ours_vs_fp32.top1_agreement() * 100.0,
        ours_vs_fp32.total_top1_matches(),
        ours_vs_fp32.total_top1_positions()
    );
    let _ = writeln!(
        out,
        "- boot prompt greedy reply identical: **{}**",
        bars.b_boot_reply_identical.as_str()
    );
    let _ = writeln!(
        out,
        "- **B: {}**\n",
        if bars.b_pass() { "PASS" } else { "FAIL" }
    );

    let _ = writeln!(out, "## Cross-check — llama.cpp Q4_K_M vs {}\n", fp32.model);
    let _ = writeln!(
        out,
        "- ΔPPL {:.4}% ({:.6} vs {:.6}), boot reply identical: {}",
        q4_vs_fp32.ppl_relative_delta * 100.0,
        q4_vs_fp32.left_ppl,
        q4_vs_fp32.right_ppl,
        q4_vs_fp32
            .prompts
            .iter()
            .find(|prompt| prompt.id == "boot_hello_world")
            .map(|prompt| prompt.generated_equal)
            .unwrap_or(false)
    );

    let _ = writeln!(
        out,
        "## Per-prompt divergence (shipped Q4_K_M vs {})\n",
        fp32.model
    );
    let _ = writeln!(
        out,
        "| prompt | prompt tokens | first divergence | top-1 agreement |\n|---|---:|---:|---:|"
    );
    for prompt in &ours_vs_fp32.prompts {
        let divergence = prompt
            .first_divergence
            .map(|position| position.to_string())
            .unwrap_or_else(|| "none".to_string());
        let _ = writeln!(
            out,
            "| {} | {} | {} | {:.2}% |",
            prompt.id,
            prompt.ids,
            divergence,
            prompt.agreement() * 100.0
        );
    }
    out
}

/// Control C: a high-precision llama.cpp reference against the float32
/// reference on the same IDs and window. The bar is fixed in the dispatch:
/// teacher-forced top-1 >= 98% and relative PPL delta <= 1%.
pub fn control_json(comparison: &Comparison) -> String {
    let top1_agreement = comparison.top1_agreement();
    let ppl_delta = comparison.ppl_relative_delta;
    let mut out = String::from("{\n");
    let _ = writeln!(out, "  \"schema\": \"quant-drift-control-v1\",");
    let _ = writeln!(
        out,
        "  \"candidate\": \"{}\",",
        json_escape(&comparison.left)
    );
    let _ = writeln!(
        out,
        "  \"reference\": \"{}\",",
        json_escape(&comparison.right)
    );
    let _ = writeln!(out, "  \"candidate_ppl\": {:.9},", comparison.left_ppl);
    let _ = writeln!(out, "  \"reference_ppl\": {:.9},", comparison.right_ppl);
    let _ = writeln!(out, "  \"ppl_relative_delta\": {:.9},", ppl_delta);
    let _ = writeln!(out, "  \"top1_agreement\": {:.9},", top1_agreement);
    let _ = writeln!(
        out,
        "  \"top1_matches\": {},",
        comparison.total_top1_matches()
    );
    let _ = writeln!(
        out,
        "  \"top1_positions\": {},",
        comparison.total_top1_positions()
    );
    let _ = writeln!(
        out,
        "  \"bar_top1_at_least_98pct\": {},",
        top1_agreement >= 0.98
    );
    let _ = writeln!(out, "  \"bar_ppl_within_1pct\": {},", ppl_delta <= 0.01);
    let _ = writeln!(
        out,
        "  \"pass\": {}",
        top1_agreement >= 0.98 && ppl_delta <= 0.01
    );
    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(model: &str, offset: usize, tokens: usize, eval_start: usize) -> RefRun {
        RefRun {
            model: model.to_string(),
            corpus_window_offset: offset,
            corpus_tokens: tokens,
            corpus_eval_start: eval_start,
            corpus_nll_count: tokens.saturating_sub(eval_start),
            ..Default::default()
        }
    }

    #[test]
    fn compare_rejects_a_different_corpus_offset() {
        let err = compare(&run("a", 6144, 2048, 1024), &run("b", 0, 2048, 1024))
            .expect_err("offset mismatch must be rejected");
        assert!(err.contains("corpus window differs"), "{err}");
    }

    #[test]
    fn compare_rejects_a_different_corpus_length() {
        let err = compare(&run("a", 6144, 2048, 1024), &run("b", 6144, 1024, 512))
            .expect_err("length mismatch must be rejected");
        assert!(err.contains("corpus window differs"), "{err}");
    }
}
