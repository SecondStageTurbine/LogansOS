// SPDX-License-Identifier: MPL-2.0
//! OT-86 Fix 2: deterministic corpus-seeded fuzzing of the AML interpreter.
//! See `acpi_aml_grade::fuzz` for how cases are made.
//!
//! Run from the repo root, with unwinding (the workspace profile aborts) and
//! the dev profile's overflow checks:
//! ```text
//! CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade --bin acpi-aml-fuzz -- \
//!     run --seed 0x0786 --cases 200000 --jobs 4 --report tools/acpi-aml-grade/report/fuzz-fix1.json
//! CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade --bin acpi-aml-fuzz -- \
//!     case --seed 0x0786 --index 1234 --dump case.aml
//! ```
//!
//! `run` splits the case range over `--jobs` worker processes. A case that
//! panics inside the crate is caught and reported. A case that runs past
//! `--timeout-s` is a hang: the worker is killed, the case recorded as
//! non-termination (OT-86 L7 data, not a Fix 2 failure) and the worker
//! restarted after it. A worker that dies mid-case (stack overflow, an
//! allocation over the cap) is a crash and fails the run, like a panic.

use acpi_aml_grade::fuzz;
use acpi_aml_grade::grade_acpi6::{fix1, stock};
use serde_json::json;
use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// One allocation larger than this fails, so firmware-sized requests (a
/// `Buffer (0xFFFFFFFF)`) end the worker as a recorded crash instead of
/// exhausting the host.
const ALLOC_CAP: usize = 1 << 30;

struct Capped;

unsafe impl GlobalAlloc for Capped {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if l.size() > ALLOC_CAP {
            return std::ptr::null_mut();
        }
        unsafe { System.alloc(l) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        if l.size() > ALLOC_CAP {
            return std::ptr::null_mut();
        }
        unsafe { System.alloc_zeroed(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if n > ALLOC_CAP {
            return std::ptr::null_mut();
        }
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static ALLOC: Capped = Capped;

const CORPUS: &str = "fixtures/acpi";
const KATS: &str = "tools/acpi-aml-grade/tests/fix1_kats.rs";

fn arg<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn num(args: &[String], name: &str, default: u64) -> u64 {
    match arg(args, name) {
        None => default,
        Some(v) => match v.strip_prefix("0x") {
            Some(h) => u64::from_str_radix(h, 16),
            None => v.parse(),
        }
        .unwrap_or_else(|_| die(&format!("{name}: not a number: {v}"))),
    }
}

fn die(msg: &str) -> ! {
    eprintln!("acpi-aml-fuzz: {msg}");
    std::process::exit(2)
}

fn run_one(
    krate: &str,
    facp: &acpi_aml_grade::corpus::Table,
    rev: u8,
    aml: &[u8],
) -> Option<String> {
    match krate {
        "fix1" => fix1::fuzz_one(facp, rev, aml, fuzz::CALLS_PER_CASE),
        "stock" => stock::fuzz_one(facp, rev, aml, fuzz::CALLS_PER_CASE),
        other => die(&format!("--crate {other}: expected fix1 or stock")),
    }
}

fn setup() -> (Vec<fuzz::Source>, acpi_aml_grade::corpus::Table) {
    let sets = acpi_aml_grade::corpus::load_corpus(Path::new(CORPUS))
        .unwrap_or_else(|e| die(&format!("read {CORPUS} (run from the repo root): {e}")));
    let pool = fuzz::pool(&sets, Path::new(KATS));
    let facp = fuzz::facp(Path::new(CORPUS)).unwrap_or_else(|e| die(&e.to_string()));
    (pool, facp)
}

fn worker(args: &[String]) {
    if cfg!(panic = "abort") {
        die("build with CARGO_PROFILE_DEV_PANIC=unwind");
    }
    let krate = arg(args, "--crate").unwrap_or("fix1");
    let seed = num(args, "--seed", 0x0786);
    let (from, to) = (num(args, "--from", 0), num(args, "--to", 0));
    let (pool, facp) = setup();
    let out = std::io::stdout();
    for index in from..to {
        let (src, bytes, rev) = fuzz::case(seed, index, &pool);
        let mut o = out.lock();
        let _ = writeln!(o, "B {index}");
        let _ = o.flush();
        drop(o);
        if let Some(msg) = run_one(krate, &facp, rev, &bytes) {
            let mut o = out.lock();
            let _ = writeln!(
                o,
                "P {index} {:016x} {}\t{}",
                fuzz::hash(&bytes),
                pool[src].origin,
                msg.replace('\n', " ")
            );
            let _ = o.flush();
        }
    }
    let _ = writeln!(out.lock(), "D");
}

fn case_cmd(args: &[String]) {
    let krate = arg(args, "--crate").unwrap_or("fix1");
    let seed = num(args, "--seed", 0x0786);
    let index = num(args, "--index", 0);
    let (pool, facp) = setup();
    let (src, bytes, rev) = fuzz::case(seed, index, &pool);
    println!(
        "case {index} of seed {seed:#x}: source {} (revision {rev}), {} bytes, hash {:016x}",
        pool[src].origin,
        bytes.len(),
        fuzz::hash(&bytes)
    );
    if let Some(path) = arg(args, "--dump") {
        std::fs::write(path, &bytes).unwrap_or_else(|e| die(&e.to_string()));
        println!("bytes written to {path}");
    }
    match run_one(krate, &facp, rev, &bytes) {
        Some(msg) => println!("PANIC: {msg}"),
        None => println!("no panic"),
    }
}

#[derive(Default)]
struct Found {
    panics: Vec<serde_json::Value>,
    hangs: Vec<serde_json::Value>,
    crashes: Vec<serde_json::Value>,
}

/// Drive one worker over `from..to`, restarting it after a hang or a crash.
fn drive(
    krate: &str,
    seed: u64,
    from: u64,
    to: u64,
    timeout: Duration,
    describe: &dyn Fn(u64) -> serde_json::Value,
) -> Found {
    let exe = std::env::current_exe().unwrap();
    let mut found = Found::default();
    let mut next = from;
    while next < to {
        let mut child = Command::new(&exe)
            .args(["worker", "--crate", krate])
            .args([
                "--seed",
                &seed.to_string(),
                "--from",
                &next.to_string(),
                "--to",
                &to.to_string(),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn worker");
        let (tx, rx) = mpsc::channel();
        let stdout = child.stdout.take().unwrap();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut current = next;
        let mut done = false;
        loop {
            match rx.recv_timeout(timeout) {
                Ok(line) if line == "D" => {
                    done = true;
                    break;
                }
                Ok(line) if line.starts_with("B ") => current = line[2..].parse().unwrap(),
                Ok(line) if line.starts_with("P ") => {
                    let mut parts = line[2..].splitn(3, ' ');
                    let index: u64 = parts.next().unwrap().parse().unwrap();
                    let hash = parts.next().unwrap_or("").to_string();
                    let (origin, msg) = parts
                        .next()
                        .unwrap_or("")
                        .split_once('\t')
                        .unwrap_or(("", ""));
                    found.panics.push(
                        json!({"index": index, "hash": hash, "source": origin, "panic": msg}),
                    );
                }
                Ok(_) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    found.hangs.push(describe(current));
                    break;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let status = child.wait().map(|s| s.to_string()).unwrap_or_default();
                    let mut c = describe(current);
                    c["exit"] = json!(status);
                    found.crashes.push(c);
                    break;
                }
            }
        }
        if done {
            let _ = child.wait();
            break;
        }
        next = current + 1;
    }
    found
}

fn run(args: &[String]) {
    let krate = arg(args, "--crate").unwrap_or("fix1").to_string();
    let seed = num(args, "--seed", 0x0786);
    let cases = num(args, "--cases", 200_000);
    let jobs = num(args, "--jobs", 4).max(1);
    let timeout = Duration::from_secs(num(args, "--timeout-s", 20));
    let (pool, _) = setup();
    let pool = std::sync::Arc::new(pool);
    let started = Instant::now();
    let chunk = cases.div_ceil(jobs);
    let handles: Vec<_> = (0..jobs)
        .map(|j| {
            let (krate, pool) = (krate.clone(), pool.clone());
            std::thread::spawn(move || {
                let describe = |index: u64| {
                    let (src, bytes, _) = fuzz::case(seed, index, &pool);
                    json!({"index": index, "hash": format!("{:016x}", fuzz::hash(&bytes)), "source": pool[src].origin})
                };
                drive(&krate, seed, j * chunk, ((j + 1) * chunk).min(cases), timeout, &describe)
            })
        })
        .collect();
    let mut all = Found::default();
    for h in handles {
        let f = h.join().unwrap();
        all.panics.extend(f.panics);
        all.hangs.extend(f.hangs);
        all.crashes.extend(f.crashes);
    }
    for list in [&mut all.panics, &mut all.hangs, &mut all.crashes] {
        list.sort_by_key(|v| v["index"].as_u64());
    }
    let mut sites = std::collections::BTreeMap::<String, u64>::new();
    for p in &all.panics {
        let site = p["panic"]
            .as_str()
            .unwrap_or("")
            .rsplit(" @ ")
            .next()
            .unwrap_or("")
            .to_string();
        *sites.entry(site).or_default() += 1;
    }
    let report = json!({
        "crate": krate, "seed": format!("{seed:#x}"), "cases": cases,
        "timeout_s": timeout.as_secs(), "pool_sources": pool.len(),
        "panic_count": all.panics.len(), "panic_sites": sites,
        "hang_count": all.hangs.len(), "crash_count": all.crashes.len(),
        "panics": all.panics, "hangs": all.hangs, "crashes": all.crashes,
    });
    let text = serde_json::to_string_pretty(&report).unwrap() + "\n";
    if let Some(path) = arg(args, "--report") {
        std::fs::write(path, &text).unwrap_or_else(|e| die(&e.to_string()));
    }
    eprintln!(
        "{krate}: {cases} cases (seed {seed:#x}, {} sources) in {:.0} s: {} panics at {} sites, {} hangs, {} crashes",
        pool.len(),
        started.elapsed().as_secs_f64(),
        report["panic_count"],
        report["panic_sites"].as_object().unwrap().len(),
        report["hang_count"],
        report["crash_count"]
    );
    if report["panic_count"] != 0 || report["crash_count"] != 0 {
        std::process::exit(1);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => run(&args),
        Some("worker") => worker(&args),
        Some("case") => case_cmd(&args),
        _ => die(
            "usage: acpi-aml-fuzz run|case|worker [--crate fix1|stock] [--seed S] [--cases N] [--jobs J] [--timeout-s T] [--report PATH] [--index I] [--dump PATH]",
        ),
    }
}
