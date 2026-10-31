// SPDX-License-Identifier: MPL-2.0
//! Rig-agnostic benchmark harness for the Phase 8 LogansOS vs llama.cpp run.
//!
//! The harness records the pre-number manifest, runs external subject commands,
//! keeps raw CSV samples, and summarizes the pre-registered p50/p99, tok/s, and
//! fairness metrics. LogansOS and llama.cpp remain external commands so the same
//! tool works for QEMU, matched VMs, and the later bare-metal rig.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::Mutex;
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const DEFAULT_CONTEXT: &str = "4096";
const DEFAULT_PROMPT_TOKEN: &str = "9707";
const DEFAULT_THREADS: &str = "1";
const SAMPLE_HEADER: &str = "phase,sample,tenant,subject,scenario,exit_code,start_unix_ms,elapsed_ns,decode_ns,tokens_expected,tokens_observed,token_source,valid_for_latency,rss_bytes,avg_token_ns,token_latencies_ns,stdout_path,stderr_path,command\n";

static SAMPLE_WRITE_LOCK: Mutex<()> = Mutex::new(());

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("bench-harness: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        print_help();
        return Ok(());
    }

    match args[0].as_str() {
        "manifest" => cmd_manifest(&args[1..]),
        "check-manifest" => cmd_check_manifest(&args[1..]),
        "run-command" => cmd_run_command(&args[1..]),
        "run-parallel" => cmd_run_parallel(&args[1..]),
        "summarize" => cmd_summarize(&args[1..]),
        "self-test" => cmd_self_test(),
        "help" | "-h" | "--help" => {
            print_help();
            Ok(())
        }
        other => Err(format!(
            "unknown command `{other}`; try `bench-harness help`"
        )),
    }
}

fn print_help() {
    println!(
        "\
bench-harness commands:
  manifest        create the pre-number manifest and raw CSV directory
  check-manifest  validate that the required pre-number fields are present
  run-command     run one subject command serially
  run-parallel    run one subject command concurrently for N tenants
  summarize       summarize raw/samples.csv into markdown
  self-test       run parser/stat self-tests without collecting benchmark data

Subject commands should emit:
  NEO_BENCH/1 event=token index=0 latency_ns=1234567
  NEO_BENCH/1 event=done tokens=128 decode_ns=158000000 rss_bytes=123456789
"
    );
}

#[derive(Default)]
struct Opts {
    values: BTreeMap<String, String>,
    flags: BTreeMap<String, bool>,
    trailing: Vec<String>,
}

impl Opts {
    fn req(&self, key: &str) -> Result<&str, String> {
        self.values
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| format!("missing required --{key}"))
    }

    fn opt(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    fn value_or(&self, key: &str, default: &str) -> String {
        self.opt(key).unwrap_or(default).to_string()
    }

    fn flag(&self, key: &str) -> bool {
        self.flags.get(key).copied().unwrap_or(false)
    }
}

fn parse_opts(args: &[String]) -> Result<Opts, String> {
    let mut opts = Opts::default();
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];
        if arg == "--" {
            opts.trailing = args[i + 1..].to_vec();
            return Ok(opts);
        }

        let Some(stripped) = arg.strip_prefix("--") else {
            return Err(format!(
                "unexpected positional argument `{arg}` before `--`"
            ));
        };

        if stripped.is_empty() {
            return Err("empty option name".to_string());
        }

        if i + 1 < args.len() && !args[i + 1].starts_with("--") {
            opts.values
                .insert(stripped.to_string(), args[i + 1].clone());
            i += 2;
        } else {
            opts.flags.insert(stripped.to_string(), true);
            i += 1;
        }
    }

    Ok(opts)
}

fn cmd_manifest(args: &[String]) -> Result<(), String> {
    let opts = parse_opts(args)?;
    let out = PathBuf::from(opts.req("out")?);
    let manifest_path = out.join("manifest.json");
    if manifest_path.exists() {
        return Err(format!(
            "{} already exists; create a new run directory for a new manifest",
            manifest_path.display()
        ));
    }

    let rig_mode = opts.req("rig-mode")?;
    let hardware_id = opts.req("hardware-id")?;
    let cpu_pinning = opts.req("cpu-pinning")?;
    let load_state = opts.req("load-state")?;
    let model = PathBuf::from(opts.req("model")?);
    let llamacpp_src = PathBuf::from(opts.req("llamacpp-src")?);
    let threads = opts.value_or("threads", DEFAULT_THREADS);
    let context = opts.value_or("context", DEFAULT_CONTEXT);
    let prompt_token = opts.value_or("prompt-token", DEFAULT_PROMPT_TOKEN);
    let notes = opts.value_or("notes", "");
    let normalized_rig = normalize_token(rig_mode);
    let valid_for_ledger =
        opts.flag("valid-for-ledger") || normalized_rig == "bare-metal-dual-boot";
    let ledger_class = if valid_for_ledger {
        "valid-for-ledger"
    } else if normalized_rig == "controlled-same-host-vm" {
        "scoping-controlled-same-host"
    } else {
        "not-valid-for-ledger"
    };

    fs::create_dir_all(out.join("raw").join("logs")).map_err(|err| {
        format!(
            "failed to create benchmark directory {}: {err}",
            out.display()
        )
    })?;

    let model_sha256 = sha256_file(&model)?;
    let logansos_commit = git_stdout(None, &["rev-parse", "HEAD"]);
    let logansos_branch = git_stdout(None, &["branch", "--show-current"]);
    let llamacpp_commit = git_stdout(Some(&llamacpp_src), &["rev-parse", "HEAD"]);
    let created_unix_ms = unix_millis();

    let manifest = format!(
        concat!(
            "{{\n",
            "  \"schema\": \"NEO_BENCH_MANIFEST/1\",\n",
            "  \"created_unix_ms\": {created_unix_ms},\n",
            "  \"rig_mode\": {rig_mode},\n",
            "  \"ledger_class\": {ledger_class},\n",
            "  \"hardware_id\": {hardware_id},\n",
            "  \"cpu_pinning\": {cpu_pinning},\n",
            "  \"load_state\": {load_state},\n",
            "  \"logansos_branch\": {logansos_branch},\n",
            "  \"logansos_commit\": {logansos_commit},\n",
            "  \"llamacpp_src\": {llamacpp_src},\n",
            "  \"llamacpp_commit\": {llamacpp_commit},\n",
            "  \"model\": {model},\n",
            "  \"model_sha256\": {model_sha256},\n",
            "  \"context\": {context},\n",
            "  \"threads\": {threads},\n",
            "  \"prompt_token\": {prompt_token},\n",
            "  \"line_protocol\": \"NEO_BENCH/1\",\n",
            "  \"notes\": {notes}\n",
            "}}\n"
        ),
        created_unix_ms = created_unix_ms,
        rig_mode = json_string(rig_mode),
        ledger_class = json_string(ledger_class),
        hardware_id = json_string(hardware_id),
        cpu_pinning = json_string(cpu_pinning),
        load_state = json_string(load_state),
        logansos_branch = json_string(&logansos_branch),
        logansos_commit = json_string(&logansos_commit),
        llamacpp_src = json_string(&llamacpp_src.display().to_string()),
        llamacpp_commit = json_string(&llamacpp_commit),
        model = json_string(&model.display().to_string()),
        model_sha256 = json_string(&model_sha256),
        context = json_string(&context),
        threads = json_string(&threads),
        prompt_token = json_string(&prompt_token),
        notes = json_string(&notes),
    );

    fs::write(&manifest_path, manifest)
        .map_err(|err| format!("failed to write {}: {err}", manifest_path.display()))?;
    init_samples_csv(&out.join("raw").join("samples.csv"))?;
    println!("manifest written: {}", manifest_path.display());
    println!("ledger_class: {ledger_class}");
    Ok(())
}

fn cmd_check_manifest(args: &[String]) -> Result<(), String> {
    let opts = parse_opts(args)?;
    let manifest = PathBuf::from(opts.req("manifest")?);
    validate_manifest(&manifest)?;
    println!("manifest OK: {}", manifest.display());
    Ok(())
}

fn validate_manifest(path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("failed to read manifest {}: {err}", path.display()))?;
    let required = [
        "\"schema\": \"NEO_BENCH_MANIFEST/1\"",
        "\"rig_mode\"",
        "\"ledger_class\"",
        "\"hardware_id\"",
        "\"cpu_pinning\"",
        "\"load_state\"",
        "\"logansos_commit\"",
        "\"llamacpp_commit\"",
        "\"model_sha256\"",
        "\"line_protocol\": \"NEO_BENCH/1\"",
    ];

    for marker in required {
        if !text.contains(marker) {
            return Err(format!(
                "manifest {} is missing required marker {marker}",
                path.display()
            ));
        }
    }

    let lower = text.to_ascii_lowercase();
    for unresolved in ["tbd", "fill exact", "before run", "\"unknown\""] {
        if lower.contains(unresolved) {
            return Err(format!(
                "manifest {} contains unresolved field marker `{unresolved}`",
                path.display()
            ));
        }
    }

    Ok(())
}

#[derive(Clone)]
struct RunConfig {
    raw: PathBuf,
    logs: PathBuf,
    subject: String,
    scenario: String,
    samples: usize,
    warmups: usize,
    tokens_expected: usize,
    allow_wall_clock_only: bool,
    command: Vec<String>,
}

impl RunConfig {
    fn from_opts(opts: &Opts) -> Result<Self, String> {
        let manifest = PathBuf::from(opts.req("manifest")?);
        validate_manifest(&manifest)?;
        let root = manifest
            .parent()
            .ok_or_else(|| format!("manifest {} has no parent directory", manifest.display()))?;
        let raw = root.join("raw").join("samples.csv");
        if !raw.exists() {
            init_samples_csv(&raw)?;
        }

        let command = opts.trailing.clone();
        if command.is_empty() {
            return Err("missing subject command after `--`".to_string());
        }

        Ok(Self {
            raw,
            logs: root.join("raw").join("logs"),
            subject: opts.req("subject")?.to_string(),
            scenario: opts.req("scenario")?.to_string(),
            samples: parse_usize_opt(opts.opt("samples"), 10, "samples")?,
            warmups: parse_usize_opt(opts.opt("warmups"), 1, "warmups")?,
            tokens_expected: parse_usize_opt(opts.opt("tokens"), 128, "tokens")?,
            allow_wall_clock_only: opts.flag("allow-wall-clock-only"),
            command,
        })
    }
}

fn cmd_run_command(args: &[String]) -> Result<(), String> {
    let opts = parse_opts(args)?;
    let cfg = RunConfig::from_opts(&opts)?;
    let tenant = parse_usize_opt(opts.opt("tenant"), 0, "tenant")?;

    for sample in 0..cfg.warmups {
        run_serial_sample(&cfg, "warmup", sample, tenant, false)?;
    }

    for sample in 0..cfg.samples {
        run_serial_sample(&cfg, "measure", sample, tenant, true)?;
    }

    Ok(())
}

fn cmd_run_parallel(args: &[String]) -> Result<(), String> {
    let opts = parse_opts(args)?;
    let cfg = RunConfig::from_opts(&opts)?;
    let tenants = parse_usize_opt(opts.opt("tenants"), 2, "tenants")?;
    if tenants == 0 {
        return Err("--tenants must be greater than zero".to_string());
    }

    for sample in 0..cfg.warmups {
        run_parallel_group(&cfg, "warmup", sample, tenants, false)?;
    }

    for sample in 0..cfg.samples {
        run_parallel_group(&cfg, "measure", sample, tenants, true)?;
    }

    Ok(())
}

fn run_serial_sample(
    cfg: &RunConfig,
    phase: &str,
    sample: usize,
    tenant: usize,
    record: bool,
) -> Result<(), String> {
    let completed = execute_sample(cfg, phase, sample, tenant)?;
    if record {
        append_sample(&cfg.raw, &completed.row)?;
    }
    if completed.exit_code != 0 {
        return Err(format!(
            "subject {} scenario {} sample {sample} tenant {tenant} exited {}",
            cfg.subject, cfg.scenario, completed.exit_code
        ));
    }
    Ok(())
}

fn run_parallel_group(
    cfg: &RunConfig,
    phase: &str,
    sample: usize,
    tenants: usize,
    record: bool,
) -> Result<(), String> {
    let mut handles = Vec::with_capacity(tenants);
    for tenant in 0..tenants {
        let thread_cfg = cfg.clone();
        let phase = phase.to_string();
        handles.push(thread::spawn(move || {
            execute_sample(&thread_cfg, &phase, sample, tenant)
        }));
    }

    let mut completed = Vec::with_capacity(tenants);
    for handle in handles {
        completed.push(
            handle
                .join()
                .map_err(|_| "benchmark subject thread panicked".to_string())??,
        );
    }

    if record {
        for sample in &completed {
            append_sample(&cfg.raw, &sample.row)?;
        }
    }

    for sample in completed {
        if sample.exit_code != 0 {
            return Err(format!(
                "subject {} scenario {} sample {} tenant {} exited {}",
                cfg.subject, cfg.scenario, sample.row.sample, sample.row.tenant, sample.exit_code
            ));
        }
    }

    Ok(())
}

struct CompletedSample {
    exit_code: i32,
    row: SampleRowOwned,
}

fn execute_sample(
    cfg: &RunConfig,
    phase: &str,
    sample: usize,
    tenant: usize,
) -> Result<CompletedSample, String> {
    fs::create_dir_all(&cfg.logs)
        .map_err(|err| format!("failed to create log dir {}: {err}", cfg.logs.display()))?;
    let stem = format!(
        "{}_{}_{}_tenant{}_sample{}",
        sanitize_path_part(&cfg.scenario),
        sanitize_path_part(&cfg.subject),
        sanitize_path_part(phase),
        tenant,
        sample
    );
    let stdout_path = cfg.logs.join(format!("{stem}_stdout.txt"));
    let stderr_path = cfg.logs.join(format!("{stem}_stderr.txt"));

    let start_unix_ms = unix_millis();
    let mut command = Command::new(&cfg.command[0]);
    command
        .args(&cfg.command[1..])
        .env("NEO_BENCH_SUBJECT", &cfg.subject)
        .env("NEO_BENCH_SCENARIO", &cfg.scenario)
        .env("NEO_BENCH_PHASE", phase)
        .env("NEO_BENCH_SAMPLE", sample.to_string())
        .env("NEO_BENCH_TENANT", tenant.to_string())
        .env("NEO_BENCH_TOKENS", cfg.tokens_expected.to_string());

    let start = Instant::now();
    let output = command
        .output()
        .map_err(|err| format!("failed to run `{}`: {err}", cfg.command[0]))?;
    let elapsed_ns = start.elapsed().as_nanos();

    fs::write(&stdout_path, &output.stdout)
        .map_err(|err| format!("failed to write {}: {err}", stdout_path.display()))?;
    fs::write(&stderr_path, &output.stderr)
        .map_err(|err| format!("failed to write {}: {err}", stderr_path.display()))?;

    let stdout_text = String::from_utf8_lossy(&output.stdout);
    let stderr_text = String::from_utf8_lossy(&output.stderr);
    let protocol = parse_protocol(&format!("{stdout_text}\n{stderr_text}"));
    let token_line_count = protocol.token_latencies.len();
    let valid_for_latency = token_line_count >= cfg.tokens_expected;
    if !valid_for_latency && !cfg.allow_wall_clock_only {
        return Err(format!(
            "subject {} scenario {} sample {sample} tenant {tenant} emitted {token_line_count} token latency lines; expected at least {}. Use --allow-wall-clock-only only for harness shakedown rows.",
            cfg.subject, cfg.scenario, cfg.tokens_expected
        ));
    }

    let exit_code = output.status.code().unwrap_or(-1);
    let tokens_observed = protocol.done_tokens.unwrap_or(token_line_count);
    let decode_ns = protocol.decode_ns.or_else(|| {
        if token_line_count > 0 {
            Some(protocol.token_latencies.iter().sum())
        } else if cfg.allow_wall_clock_only {
            Some(elapsed_ns)
        } else {
            None
        }
    });
    let avg_token_ns = if token_line_count > 0 {
        Some(protocol.token_latencies.iter().sum::<u128>() / token_line_count as u128)
    } else {
        None
    };
    let token_source = if token_line_count > 0 {
        "neo_bench_token_protocol"
    } else {
        "wall_clock_only"
    };

    let row = SampleRowOwned {
        phase: phase.to_string(),
        sample,
        tenant,
        subject: cfg.subject.clone(),
        scenario: cfg.scenario.clone(),
        exit_code,
        start_unix_ms,
        elapsed_ns,
        decode_ns,
        tokens_expected: cfg.tokens_expected,
        tokens_observed,
        token_source: token_source.to_string(),
        valid_for_latency,
        rss_bytes: protocol.rss_bytes,
        avg_token_ns,
        token_latencies_ns: protocol.token_latencies,
        stdout_path,
        stderr_path,
        command: cfg.command.join(" "),
    };

    Ok(CompletedSample { exit_code, row })
}

#[derive(Default, Debug, PartialEq, Eq)]
struct ProtocolEvents {
    token_latencies: Vec<u128>,
    done_tokens: Option<usize>,
    decode_ns: Option<u128>,
    rss_bytes: Option<u64>,
}

fn parse_protocol(text: &str) -> ProtocolEvents {
    let mut events = ProtocolEvents::default();

    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("NEO_BENCH/1 ") else {
            continue;
        };
        let fields = parse_kv_fields(rest);
        match fields.get("event").map(String::as_str) {
            Some("token") => {
                if let Some(latency) =
                    first_present(&fields, &["latency_ns", "token_latency_ns", "ns"])
                        .and_then(|value| value.parse::<u128>().ok())
                {
                    events.token_latencies.push(latency);
                }
            }
            Some("done") => {
                events.done_tokens = first_present(&fields, &["tokens", "tokens_observed"])
                    .and_then(|value| value.parse::<usize>().ok());
                events.decode_ns = first_present(&fields, &["decode_ns", "elapsed_ns"])
                    .and_then(|value| value.parse::<u128>().ok());
                events.rss_bytes = first_present(&fields, &["rss_bytes", "resident_bytes"])
                    .and_then(|value| value.parse::<u64>().ok());
            }
            _ => {}
        }
    }

    events
}

fn parse_kv_fields(text: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    for token in text.split_whitespace() {
        if let Some((key, value)) = token.split_once('=') {
            fields.insert(key.to_string(), value.trim_matches('"').to_string());
        }
    }
    fields
}

fn first_present<'a>(fields: &'a BTreeMap<String, String>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| fields.get(*key).map(String::as_str))
}

#[derive(Clone)]
struct SampleRowOwned {
    phase: String,
    sample: usize,
    tenant: usize,
    subject: String,
    scenario: String,
    exit_code: i32,
    start_unix_ms: u128,
    elapsed_ns: u128,
    decode_ns: Option<u128>,
    tokens_expected: usize,
    tokens_observed: usize,
    token_source: String,
    valid_for_latency: bool,
    rss_bytes: Option<u64>,
    avg_token_ns: Option<u128>,
    token_latencies_ns: Vec<u128>,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
    command: String,
}

impl SampleRowOwned {
    fn to_csv_line(&self) -> String {
        let latencies = self
            .token_latencies_ns
            .iter()
            .map(u128::to_string)
            .collect::<Vec<_>>()
            .join(";");
        let fields = [
            self.phase.clone(),
            self.sample.to_string(),
            self.tenant.to_string(),
            self.subject.clone(),
            self.scenario.clone(),
            self.exit_code.to_string(),
            self.start_unix_ms.to_string(),
            self.elapsed_ns.to_string(),
            opt_u128(self.decode_ns),
            self.tokens_expected.to_string(),
            self.tokens_observed.to_string(),
            self.token_source.clone(),
            self.valid_for_latency.to_string(),
            opt_u64(self.rss_bytes),
            opt_u128(self.avg_token_ns),
            latencies,
            self.stdout_path.display().to_string(),
            self.stderr_path.display().to_string(),
            self.command.clone(),
        ];

        let mut line = fields
            .iter()
            .map(|field| csv_escape(field))
            .collect::<Vec<_>>()
            .join(",");
        line.push('\n');
        line
    }
}

fn append_sample(raw: &Path, row: &SampleRowOwned) -> Result<(), String> {
    let _guard = SAMPLE_WRITE_LOCK
        .lock()
        .map_err(|_| "sample write lock poisoned".to_string())?;
    if let Some(parent) = raw.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(raw)
        .map_err(|err| format!("failed to open {}: {err}", raw.display()))?;
    if file
        .metadata()
        .map_err(|err| format!("failed to stat {}: {err}", raw.display()))?
        .len()
        == 0
    {
        file.write_all(SAMPLE_HEADER.as_bytes())
            .map_err(|err| format!("failed to write {}: {err}", raw.display()))?;
    }
    file.write_all(row.to_csv_line().as_bytes())
        .map_err(|err| format!("failed to append {}: {err}", raw.display()))?;
    Ok(())
}

fn init_samples_csv(raw: &Path) -> Result<(), String> {
    if let Some(parent) = raw.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    let mut file =
        File::create(raw).map_err(|err| format!("failed to create {}: {err}", raw.display()))?;
    file.write_all(SAMPLE_HEADER.as_bytes())
        .map_err(|err| format!("failed to write {}: {err}", raw.display()))
}

fn cmd_summarize(args: &[String]) -> Result<(), String> {
    let opts = parse_opts(args)?;
    let raw = PathBuf::from(opts.req("raw")?);
    let out = PathBuf::from(opts.req("out")?);
    let rows = read_summary_rows(&raw)?;
    let mut groups: BTreeMap<(String, String, usize), Vec<SummaryRow>> = BTreeMap::new();
    for row in rows.into_iter().filter(|row| row.phase == "measure") {
        groups
            .entry((row.subject.clone(), row.scenario.clone(), row.tenant))
            .or_default()
            .push(row);
    }

    let mut report = String::new();
    writeln!(
        &mut report,
        "<!-- SPDX-License-Identifier: CC-BY-4.0 -->\n# Benchmark Summary\n"
    )
    .expect("String write cannot fail");
    writeln!(
        &mut report,
        "| Subject | Scenario | Tenant | Samples | Valid token samples | Invalid rows | p50 token ms | p99 token ms | tok/s | Max RSS MiB |"
    )
    .expect("String write cannot fail");
    writeln!(
        &mut report,
        "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"
    )
    .expect("String write cannot fail");

    let mut fairness_groups: BTreeMap<(String, String), Vec<(usize, f64)>> = BTreeMap::new();

    for ((subject, scenario, tenant), rows) in &groups {
        let valid_rows: Vec<&SummaryRow> =
            rows.iter().filter(|row| row.valid_for_latency).collect();
        let invalid = rows.len().saturating_sub(valid_rows.len());
        let token_latencies = valid_rows
            .iter()
            .flat_map(|row| row.token_latencies_ns.iter().copied())
            .collect::<Vec<_>>();
        let p50 = percentile(&token_latencies, 50.0);
        let p99 = percentile(&token_latencies, 99.0);
        let decode_sum_ns = valid_rows
            .iter()
            .filter_map(|row| row.decode_ns)
            .sum::<u128>();
        let tokens = valid_rows
            .iter()
            .map(|row| row.tokens_observed)
            .sum::<usize>();
        let tok_s = tokens_per_second(tokens, decode_sum_ns);
        let max_rss_bytes = rows.iter().filter_map(|row| row.rss_bytes).max();
        if tok_s > 0.0 {
            fairness_groups
                .entry((subject.clone(), scenario.clone()))
                .or_default()
                .push((*tenant, tok_s));
        }

        writeln!(
            &mut report,
            "| {subject} | {scenario} | {tenant} | {} | {} | {invalid} | {} | {} | {:.3} | {} |",
            rows.len(),
            valid_rows.len(),
            fmt_ms_opt(p50),
            fmt_ms_opt(p99),
            tok_s,
            fmt_mib_opt(max_rss_bytes),
        )
        .expect("String write cannot fail");
    }

    writeln!(&mut report, "\n## Multi-Tenant Fairness\n").expect("String write cannot fail");
    writeln!(
        &mut report,
        "| Subject | Scenario | Tenants | Jain fairness | max/min tok/s |"
    )
    .expect("String write cannot fail");
    writeln!(&mut report, "| --- | --- | ---: | ---: | ---: |").expect("String write cannot fail");
    for ((subject, scenario), values) in fairness_groups {
        if values.len() < 2 {
            continue;
        }
        let tok_s_values = values.iter().map(|(_, value)| *value).collect::<Vec<_>>();
        let fairness = jain_fairness(&tok_s_values).unwrap_or(0.0);
        let min = tok_s_values.iter().copied().fold(f64::INFINITY, f64::min);
        let max = tok_s_values.iter().copied().fold(0.0, f64::max);
        let ratio = if min > 0.0 { max / min } else { f64::INFINITY };
        writeln!(
            &mut report,
            "| {subject} | {scenario} | {} | {:.6} | {:.3} |",
            values.len(),
            fairness,
            ratio
        )
        .expect("String write cannot fail");
    }

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    fs::write(&out, report).map_err(|err| format!("failed to write {}: {err}", out.display()))?;
    println!("summary written: {}", out.display());
    Ok(())
}

#[derive(Clone)]
struct SummaryRow {
    phase: String,
    subject: String,
    scenario: String,
    tenant: usize,
    valid_for_latency: bool,
    decode_ns: Option<u128>,
    tokens_observed: usize,
    token_latencies_ns: Vec<u128>,
    rss_bytes: Option<u64>,
}

fn read_summary_rows(raw: &Path) -> Result<Vec<SummaryRow>, String> {
    let text = fs::read_to_string(raw)
        .map_err(|err| format!("failed to read {}: {err}", raw.display()))?;
    let mut lines = text.lines();
    let header_line = lines
        .next()
        .ok_or_else(|| format!("{} is empty", raw.display()))?;
    let headers = parse_csv_line(header_line)?;
    let index = |name: &str| {
        headers
            .iter()
            .position(|header| header == name)
            .ok_or_else(|| format!("{} missing CSV column {name}", raw.display()))
    };
    let phase_idx = index("phase")?;
    let subject_idx = index("subject")?;
    let scenario_idx = index("scenario")?;
    let tenant_idx = index("tenant")?;
    let valid_idx = index("valid_for_latency")?;
    let decode_idx = index("decode_ns")?;
    let tokens_idx = index("tokens_observed")?;
    let latencies_idx = index("token_latencies_ns")?;
    let rss_idx = index("rss_bytes")?;

    let mut rows = Vec::new();
    for (line_no, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let cells = parse_csv_line(line)?;
        if cells.len() != headers.len() {
            return Err(format!(
                "{}:{} has {} cells, expected {}",
                raw.display(),
                line_no + 2,
                cells.len(),
                headers.len()
            ));
        }
        let get = |idx: usize| cells.get(idx).map(String::as_str).unwrap_or("");
        let token_latencies_ns = if get(latencies_idx).is_empty() {
            Vec::new()
        } else {
            get(latencies_idx)
                .split(';')
                .map(|value| {
                    value.parse::<u128>().map_err(|err| {
                        format!(
                            "{}:{} invalid token latency `{value}`: {err}",
                            raw.display(),
                            line_no + 2
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
        };

        rows.push(SummaryRow {
            phase: get(phase_idx).to_string(),
            subject: get(subject_idx).to_string(),
            scenario: get(scenario_idx).to_string(),
            tenant: parse_usize_value(get(tenant_idx), "tenant")?,
            valid_for_latency: get(valid_idx) == "true",
            decode_ns: parse_u128_opt(get(decode_idx)),
            tokens_observed: parse_usize_value(get(tokens_idx), "tokens_observed")?,
            token_latencies_ns,
            rss_bytes: parse_u64_opt(get(rss_idx)),
        });
    }

    Ok(rows)
}

fn cmd_self_test() -> Result<(), String> {
    let protocol = parse_protocol(
        "\
noise
NEO_BENCH/1 event=token index=0 latency_ns=100
NEO_BENCH/1 event=token index=1 latency_ns=300
NEO_BENCH/1 event=done tokens=2 decode_ns=400 rss_bytes=8192
",
    );
    if protocol.token_latencies != vec![100, 300] {
        return Err("protocol token parser self-test failed".to_string());
    }
    if protocol.done_tokens != Some(2)
        || protocol.decode_ns != Some(400)
        || protocol.rss_bytes != Some(8192)
    {
        return Err("protocol done parser self-test failed".to_string());
    }
    if percentile(&protocol.token_latencies, 50.0) != Some(100)
        || percentile(&protocol.token_latencies, 99.0) != Some(300)
    {
        return Err("percentile self-test failed".to_string());
    }
    let cells = parse_csv_line("a,\"b,c\",\"d\"\"e\"")?;
    if cells != vec!["a", "b,c", "d\"e"] {
        return Err("CSV parser self-test failed".to_string());
    }
    println!("bench-harness self-test OK");
    Ok(())
}

fn percentile(values: &[u128], pct: f64) -> Option<u128> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let rank = ((pct / 100.0) * sorted.len() as f64).ceil() as usize;
    let index = rank.saturating_sub(1).min(sorted.len() - 1);
    Some(sorted[index])
}

fn tokens_per_second(tokens: usize, ns: u128) -> f64 {
    if tokens == 0 || ns == 0 {
        return 0.0;
    }
    tokens as f64 / (ns as f64 / 1_000_000_000.0)
}

fn jain_fairness(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let sum = values.iter().sum::<f64>();
    let sum_sq = values.iter().map(|value| value * value).sum::<f64>();
    if sum_sq == 0.0 {
        return None;
    }
    Some((sum * sum) / (values.len() as f64 * sum_sq))
}

fn fmt_ms_opt(value: Option<u128>) -> String {
    value
        .map(|ns| format!("{:.3}", ns as f64 / 1_000_000.0))
        .unwrap_or_else(|| "n/a".to_string())
}

fn fmt_mib_opt(value: Option<u64>) -> String {
    value
        .map(|bytes| format!("{:.3}", bytes as f64 / (1024.0 * 1024.0)))
        .unwrap_or_else(|| "n/a".to_string())
}

fn parse_csv_line(line: &str) -> Result<Vec<String>, String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    let mut in_quotes = false;

    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            '"' if in_quotes => in_quotes = false,
            '"' if current.is_empty() => in_quotes = true,
            ',' if !in_quotes => {
                cells.push(current);
                current = String::new();
            }
            _ => current.push(ch),
        }
    }

    if in_quotes {
        return Err("unterminated quoted CSV field".to_string());
    }
    cells.push(current);
    Ok(cells)
}

fn csv_escape(value: &str) -> String {
    if !value.contains([',', '"', '\n', '\r']) {
        return value.to_string();
    }
    let escaped = value.replace('"', "\"\"");
    format!("\"{escaped}\"")
}

fn parse_usize_opt(value: Option<&str>, default: usize, name: &str) -> Result<usize, String> {
    match value {
        Some(value) => parse_usize_value(value, name),
        None => Ok(default),
    }
}

fn parse_usize_value(value: &str, name: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|err| format!("invalid --{name} value `{value}`: {err}"))
}

fn parse_u128_opt(value: &str) -> Option<u128> {
    if value.is_empty() {
        None
    } else {
        value.parse::<u128>().ok()
    }
}

fn parse_u64_opt(value: &str) -> Option<u64> {
    if value.is_empty() {
        None
    } else {
        value.parse::<u64>().ok()
    }
}

fn opt_u128(value: Option<u128>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn opt_u64(value: Option<u64>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn normalize_token(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('_', "-")
}

fn sanitize_path_part(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file =
        File::open(path).map_err(|err| format!("failed to open {}: {err}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buf)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn git_stdout(repo: Option<&Path>, args: &[&str]) -> String {
    let mut cmd = Command::new("git");
    if let Some(repo) = repo {
        cmd.arg("-C").arg(repo);
    }
    cmd.args(args);
    match cmd.output() {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => "unknown".to_string(),
    }
}

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => {
                write!(&mut out, "\\u{:04x}", ch as u32).expect("String write cannot fail");
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_protocol_events() {
        let parsed = parse_protocol(
            "\
NEO_BENCH/1 event=token index=0 latency_ns=7
NEO_BENCH/1 event=token index=1 token_latency_ns=11
NEO_BENCH/1 event=done tokens=2 decode_ns=18 rss_bytes=4096
",
        );
        assert_eq!(parsed.token_latencies, vec![7, 11]);
        assert_eq!(parsed.done_tokens, Some(2));
        assert_eq!(parsed.decode_ns, Some(18));
        assert_eq!(parsed.rss_bytes, Some(4096));
    }

    #[test]
    fn percentile_uses_nearest_rank() {
        let values = vec![10, 20, 30, 40];
        assert_eq!(percentile(&values, 50.0), Some(20));
        assert_eq!(percentile(&values, 99.0), Some(40));
    }

    #[test]
    fn parses_csv_quotes() {
        let cells = parse_csv_line("a,\"b,c\",\"d\"\"e\"").unwrap();
        assert_eq!(cells, vec!["a", "b,c", "d\"e"]);
    }

    #[test]
    fn computes_jain_fairness() {
        let equal = jain_fairness(&[10.0, 10.0, 10.0]).unwrap();
        assert!((equal - 1.0).abs() < 0.000001);
        let uneven = jain_fairness(&[1.0, 3.0]).unwrap();
        assert!(uneven < 1.0);
    }
}
