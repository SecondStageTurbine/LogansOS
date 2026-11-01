// SPDX-License-Identifier: MPL-2.0
//! CLI for the Lane 2c decoder fuzzer. See `README.md`.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::str::FromStr;

use image_decode_fuzz::{
    Campaign, DEFAULT_CAP, DEFAULT_MAX_INPUT, DEFAULT_SECONDS, DEFAULT_SEED, Target, arm_allocator,
    install_crash_hook, minimize_with, run_campaign,
};

/// Bound on subprocess evaluations during a minimisation.
const MINIMIZE_EVALUATIONS: u64 = 4_000;

const USAGE: &str = "\
image-decode-fuzz — deterministic host fuzzing for inflate, PNG and JPEG decode

USAGE
  image-decode-fuzz [OPTIONS]
  image-decode-fuzz --check <FILE> --target <TARGET>
  image-decode-fuzz --minimize <FILE> --target <TARGET>

OPTIONS
  --target <TARGET>     inflate-zlib | inflate-raw | png | jpeg | all (default: all)
  --seconds <N>         wall-clock budget per target (default: 60)
  --iterations <N>      run exactly N inputs instead of a wall-clock budget
                        (0 = use --seconds; a crash's reproduction command uses this)
  --seed <N>            mutation seed (default: 234)
  --cap <BYTES>         decode size cap and single-allocation guard (default: 16 MiB)
  --max-input <BYTES>   maximum mutated input length (default: 64 KiB)
  --crash-dir <DIR>     where a crash input is written (default: target/image-decode-fuzz/crashes)
  --check <FILE>        run one input and exit 0 if it does not crash (used by --minimize)
  --minimize <FILE>     delta-debug a crashing input and write <FILE>.minimized.bin
  -h, --help            print this help

EXIT STATUS
  0  every target completed its budget with no crash, panic, timeout or over-cap allocation
  2  usage or I/O error
  a SIGABRT/SIGKILL (or a non-zero cargo status in CI) means the harness found a crash or
  the external timeout caught a decoder that never returned.
";

#[derive(Debug)]
enum Mode {
    Fuzz,
    Check(PathBuf),
    Minimize(PathBuf),
    Help,
}

#[derive(Debug)]
struct Args {
    mode: Mode,
    target: Option<Target>,
    seconds: u64,
    iterations: u64,
    seed: u64,
    cap: usize,
    max_input: usize,
    crash_dir: PathBuf,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            mode: Mode::Fuzz,
            target: None,
            seconds: DEFAULT_SECONDS,
            iterations: 0,
            seed: DEFAULT_SEED,
            cap: DEFAULT_CAP,
            max_input: DEFAULT_MAX_INPUT,
            crash_dir: PathBuf::from("target/image-decode-fuzz/crashes"),
        }
    }
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args::default();
    let mut check: Option<PathBuf> = None;
    let mut minimize: Option<PathBuf> = None;
    let mut iter = std::env::args().skip(1);

    while let Some(flag) = iter.next() {
        let mut value = |name: &str| -> Result<String, String> {
            iter.next().ok_or_else(|| format!("{name} needs a value"))
        };
        match flag.as_str() {
            "--target" => {
                let name = value("--target")?;
                args.target = if name == "all" {
                    None
                } else {
                    Some(Target::parse(&name).ok_or_else(|| format!("unknown target {name:?}"))?)
                };
            }
            "--seconds" => args.seconds = parse_number("--seconds", &value("--seconds")?)?,
            "--iterations" => {
                args.iterations = parse_number("--iterations", &value("--iterations")?)?;
            }
            "--seed" => args.seed = parse_number("--seed", &value("--seed")?)?,
            "--cap" => args.cap = parse_number("--cap", &value("--cap")?)?,
            "--max-input" => {
                args.max_input = parse_number("--max-input", &value("--max-input")?)?;
            }
            "--crash-dir" => args.crash_dir = PathBuf::from(value("--crash-dir")?),
            "--check" => check = Some(PathBuf::from(value("--check")?)),
            "--minimize" => minimize = Some(PathBuf::from(value("--minimize")?)),
            "-h" | "--help" => args.mode = Mode::Help,
            other => return Err(format!("unknown option {other:?}")),
        }
    }

    match (check, minimize) {
        (Some(_), Some(_)) => return Err("--check and --minimize are mutually exclusive".into()),
        (Some(path), None) => args.mode = Mode::Check(path),
        (None, Some(path)) => args.mode = Mode::Minimize(path),
        (None, None) => {}
    }

    if !matches!(args.mode, Mode::Help | Mode::Fuzz) && args.target.is_none() {
        return Err("--check and --minimize need one --target (not all)".into());
    }
    if args.cap < 64 * 1024 {
        return Err("--cap must be at least 64 KiB".into());
    }
    if args.max_input < 16 || args.max_input > 1024 * 1024 {
        return Err("--max-input must be between 16 and 1048576 bytes".into());
    }
    Ok(args)
}

fn parse_number<T: FromStr>(flag: &str, value: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("{flag} has an invalid value {value:?}"))
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("image-decode-fuzz: {error}");
            eprintln!();
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    if matches!(args.mode, Mode::Help) {
        println!("{USAGE}");
        return Ok(());
    }

    if let Err(error) = std::fs::create_dir_all(&args.crash_dir) {
        eprintln!(
            "image-decode-fuzz: warning: could not create {}: {error}",
            args.crash_dir.display()
        );
    }
    install_crash_hook(args.crash_dir.clone());
    arm_allocator(args.cap);

    match &args.mode {
        Mode::Help => unreachable!("handled above"),
        Mode::Fuzz => {
            let targets: Vec<Target> = match args.target {
                Some(target) => vec![target],
                None => Target::ALL.to_vec(),
            };
            for target in targets {
                let summary = run_campaign(&Campaign {
                    target,
                    seconds: args.seconds,
                    iterations: args.iterations,
                    seed: args.seed,
                    cap: args.cap,
                    max_input: args.max_input,
                });
                println!("{summary}");
            }
            println!(
                "image-decode-fuzz: all targets completed within budget; no crash, panic, timeout or over-cap allocation."
            );
            Ok(())
        }
        Mode::Check(path) => {
            let target = args.target.expect("validated by parse_args");
            let bytes = std::fs::read(path)
                .map_err(|error| format!("could not read {}: {error}", path.display()))?;
            // A crash aborts the process; a clean reject or decode returns 0.
            let _ = target.run(&bytes, args.cap);
            Ok(())
        }
        Mode::Minimize(path) => {
            let target = args.target.expect("validated by parse_args");
            let original = std::fs::read(path)
                .map_err(|error| format!("could not read {}: {error}", path.display()))?;
            let executable = std::env::current_exe()
                .map_err(|error| format!("could not find the harness binary: {error}"))?;
            let scratch = args
                .crash_dir
                .join(format!("minimise-candidate-{}.bin", std::process::id()));

            let minimized = minimize_with(&original, MINIMIZE_EVALUATIONS, |candidate| {
                std::fs::write(&scratch, candidate).is_ok()
                    && reproduces(&executable, target, args.cap, &scratch)
            });
            let _ = std::fs::remove_file(&scratch);

            let output = path.with_extension("minimized.bin");
            std::fs::write(&output, &minimized)
                .map_err(|error| format!("could not write {}: {error}", output.display()))?;
            println!(
                "image-decode-fuzz: minimised {} from {} to {} bytes -> {}",
                target.name(),
                original.len(),
                minimized.len(),
                output.display()
            );
            Ok(())
        }
    }
}

/// True when `file` still crashes (or hangs until the timeout) under `target`.
fn reproduces(executable: &Path, target: Target, cap: usize, file: &Path) -> bool {
    #[cfg(unix)]
    let mut command = {
        let mut command = Command::new("timeout");
        command.arg("--signal=KILL").arg("10s").arg(executable);
        command
    };
    #[cfg(not(unix))]
    let mut command = Command::new(executable);

    command
        .arg("--check")
        .arg(file)
        .arg("--target")
        .arg(target.name())
        .arg("--cap")
        .arg(cap.to_string());

    match command.status() {
        Ok(status) => !status.success(),
        Err(error) => {
            eprintln!("image-decode-fuzz: could not run a minimisation check: {error}");
            true
        }
    }
}
