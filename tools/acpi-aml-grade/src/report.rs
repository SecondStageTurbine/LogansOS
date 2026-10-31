// SPDX-License-Identifier: MPL-2.0
//! Report types shared by both graders, and the out-of-region analysis.

use crate::classify::{Class, Ctx, classify};
use crate::corpus::CorpusSet;
use crate::recorder::{Access, Space};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Setup,
    Load,
    Sta,
    Ini,
    /// A device probe (`probes.json`), or the resource decode of its `_CRS`.
    Probe,
}

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Phase::Setup => "setup",
            Phase::Load => "load",
            Phase::Sta => "_STA",
            Phase::Ini => "_INI",
            Phase::Probe => "probe",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The crate returned an `Err`.
    Error,
    /// The crate panicked (`panic!`, `todo!`, `unwrap`, index out of bounds):
    /// in a kernel this is a crash, not an error.
    Panic,
    /// The evaluation spent `ACCESS_BUDGET` handler accesses.
    Budget,
    /// The evaluation completed but touched hardware outside every region
    /// the set's tables declare.
    Access,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Error => "error",
            Mode::Panic => "panic",
            Mode::Budget => "budget",
            Mode::Access => "access",
        }
    }
}

/// One attempted unit of work: a table load or one method evaluation.
#[derive(Clone, Debug)]
pub struct Eval {
    pub phase: Phase,
    /// Table file for loads, absolute AML path for methods.
    pub target: String,
    pub ok: bool,
    /// A short rendering of the returned object (methods only).
    pub result: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Failure {
    pub eval: usize,
    pub phase: Phase,
    pub target: String,
    pub mode: Mode,
    /// The crate's own error / panic text, verbatim.
    pub raw: String,
    /// The crate's `warn!`/`error!` log lines leading up to it.
    pub logs: Vec<String>,
    /// The offending access, for `Mode::Access`.
    pub access: Option<Access>,
    pub class: Class,
    /// The failure mode it counts under (stable across instances).
    pub key: String,
    /// Per-instance detail (addresses, call-path candidates).
    pub detail: String,
}

/// An OpRegion as the interpreter's own namespace holds it after loading.
#[derive(Clone, Debug)]
pub struct Region {
    pub space: Option<Space>,
    pub space_name: String,
    pub base: u64,
    pub length: u64,
    pub path: String,
}

/// The outcome of one AML known-answer test (`run_kat` in either `acpi`
/// grader): the table load, each call's rendered value or error/panic text,
/// and every handler access made.
#[derive(Debug)]
pub struct Kat {
    pub load: Result<(), String>,
    pub results: Vec<Result<String, String>>,
    pub accesses: Vec<Access>,
}

#[derive(Clone, Debug)]
pub struct CrateGrade {
    pub krate: &'static str,
    pub version: &'static str,
    pub set: String,
    pub evals: Vec<Eval>,
    pub failures: Vec<Failure>,
    pub regions: Vec<Region>,
    pub accesses: Vec<Access>,
    /// The crate's own all-in-one init entry point, run on a fresh context.
    pub stock_init: String,
    pub notes: Vec<String>,
}

impl CrateGrade {
    pub fn new(krate: &'static str, version: &'static str, set: &str) -> Self {
        CrateGrade {
            krate,
            version,
            set: set.to_string(),
            evals: Vec::new(),
            failures: Vec::new(),
            regions: Vec::new(),
            accesses: Vec::new(),
            stock_init: String::new(),
            notes: Vec::new(),
        }
    }

    pub fn push_eval(&mut self, phase: Phase, target: &str) -> usize {
        self.evals.push(Eval {
            phase,
            target: target.to_string(),
            ok: false,
            result: None,
        });
        self.evals.len() - 1
    }

    /// Record a failure; it is classified by `finalize` once the regions
    /// and method bodies the classifier consults are known.
    pub fn fail(&mut self, eval: usize, mode: Mode, raw: String, logs: Vec<String>) {
        self.fail_access(eval, mode, raw, logs, None);
    }

    fn fail_access(
        &mut self,
        eval: usize,
        mode: Mode,
        raw: String,
        logs: Vec<String>,
        access: Option<Access>,
    ) {
        let e = &self.evals[eval];
        self.failures.push(Failure {
            eval,
            phase: e.phase,
            target: e.target.clone(),
            mode,
            raw,
            logs,
            access,
            class: Class::Unclassified,
            key: String::new(),
            detail: String::new(),
        });
    }

    /// Classify every failure. `methods` maps absolute AML paths to method
    /// bodies, as the interpreter's namespace holds them.
    pub fn finalize(&mut self, methods: &std::collections::BTreeMap<String, Vec<u8>>) {
        let regions = self.regions.clone();
        for f in &mut self.failures {
            let verdict = classify(&Ctx {
                krate: self.krate,
                phase: f.phase,
                target: &f.target,
                mode: f.mode,
                raw: &f.raw,
                access: f.access.as_ref(),
                regions: &regions,
                methods,
                logs: &f.logs,
            });
            f.class = verdict.class;
            f.key = verdict.key;
            f.detail = verdict.detail;
        }
    }

    fn count(&self, phase: Phase) -> (usize, usize) {
        let all: Vec<_> = self.evals.iter().filter(|e| e.phase == phase).collect();
        (all.len(), all.iter().filter(|e| e.ok).count())
    }

    pub fn dsdt_clean(&self) -> bool {
        self.evals
            .iter()
            .any(|e| e.phase == Phase::Load && e.target.starts_with("DSDT") && e.ok)
    }

    pub fn all_tables_clean(&self) -> bool {
        let loads: Vec<_> = self
            .evals
            .iter()
            .filter(|e| e.phase == Phase::Load)
            .collect();
        !loads.is_empty() && loads.iter().all(|e| e.ok)
    }

    pub fn to_json(&self) -> Value {
        let (lt, lo) = self.count(Phase::Load);
        let (st, so) = self.count(Phase::Sta);
        let (it, io) = self.count(Phase::Ini);
        let mut by_space = std::collections::BTreeMap::<String, (u64, u64)>::new();
        for a in &self.accesses {
            let e = by_space.entry(a.space.name().to_string()).or_default();
            if a.write { e.1 += 1 } else { e.0 += 1 }
        }
        json!({
            "crate": self.krate,
            "version": self.version,
            "set": self.set,
            "dsdt_parse_clean": self.dsdt_clean(),
            "all_tables_parse_clean": self.all_tables_clean(),
            "tables": {"attempted": lt, "completed": lo},
            "sta": {"attempted": st, "completed": so},
            "ini": {"attempted": it, "completed": io},
            "stock_init_entry_point": self.stock_init,
            "loads": self.evals.iter().filter(|e| e.phase == Phase::Load)
                .map(|e| json!({"table": e.target, "ok": e.ok})).collect::<Vec<_>>(),
            "evaluations": self.evals.iter().filter(|e| e.phase != Phase::Load && e.phase != Phase::Setup)
                .map(|e| json!({"method": e.target, "ok": e.ok, "result": e.result})).collect::<Vec<_>>(),
            "failures": self.failures.iter().map(|f| json!({
                "phase": f.phase.name(),
                "target": f.target,
                "mode": f.mode.name(),
                "class": f.class.name(),
                "mode_key": f.key,
                "detail": f.detail,
                "raw": f.raw,
                "log": f.logs,
            })).collect::<Vec<_>>(),
            "declared_regions": self.regions.iter().map(|r| json!({
                "path": r.path, "space": r.space_name,
                "base": format!("{:#x}", r.base), "length": format!("{:#x}", r.length),
            })).collect::<Vec<_>>(),
            "handler_accesses": {
                "total": self.accesses.len(),
                "by_space_reads_writes": by_space.iter()
                    .map(|(k, (r, w))| (k.clone(), json!({"reads": r, "writes": w})))
                    .collect::<serde_json::Map<_, _>>(),
                "log": self.accesses.iter().map(|a| json!({
                    "eval": self.evals.get(a.eval).map(|e| e.target.clone()),
                    "space": a.space.name(),
                    "pci": a.pci.map(|(s, b, d, f)| format!("{s:04x}:{b:02x}:{d:02x}.{f}")),
                    "addr": format!("{:#x}", a.addr),
                    "width": a.width,
                    "rw": if a.write { "W" } else { "R" },
                    "value": format!("{:#x}", a.value),
                })).collect::<Vec<_>>(),
            },
            "device_probes": self.evals.iter().enumerate().filter(|(_, e)| e.phase == Phase::Probe)
                .map(|(i, e)| json!({
                    "call": e.target, "ok": e.ok, "result": e.result,
                    "failure": self.failures.iter().find(|f| f.eval == i)
                        .map(|f| format!("{}: {}", f.class.name(), f.key)),
                })).collect::<Vec<_>>(),
            "notes": self.notes,
        })
    }
}

/// The fixed-hardware register blocks the FADT itself declares (ACPI 6.5
/// §5.2.9 legacy 32-bit fields): accesses there are "declared by the
/// set's tables" even though no OpRegion names them.
pub fn fadt_blocks(set: &CorpusSet) -> Vec<Region> {
    let Some(f) = set.find("FACP") else {
        return Vec::new();
    };
    let b = &f.bytes;
    let u32_at = |o: usize| {
        b.get(o..o + 4)
            .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
            .unwrap_or(0)
    };
    let u8_at = |o: usize| b.get(o).copied().unwrap_or(0);
    let blocks = [
        ("PM1a_EVT_BLK", 56, u8_at(88) as u64),
        ("PM1b_EVT_BLK", 60, u8_at(88) as u64),
        ("PM1a_CNT_BLK", 64, u8_at(89) as u64),
        ("PM1b_CNT_BLK", 68, u8_at(89) as u64),
        ("PM2_CNT_BLK", 72, u8_at(90) as u64),
        ("PM_TMR_BLK", 76, u8_at(91) as u64),
        ("GPE0_BLK", 80, u8_at(92) as u64),
        ("GPE1_BLK", 84, u8_at(93) as u64),
    ];
    blocks
        .iter()
        .filter(|(_, off, len)| u32_at(*off) != 0 && *len != 0)
        .map(|(name, off, len)| Region {
            space: Some(Space::SystemIo),
            space_name: "SystemIO".to_string(),
            base: u32_at(*off) as u64,
            length: *len,
            path: format!("FADT.{name}"),
        })
        .collect()
}

/// Turn every handler access that falls outside all declared regions into a
/// failure on the evaluation that issued it (one per distinct address). The
/// evaluation's own completion verdict is left as the interpreter returned it.
pub fn check_regions(g: &mut CrateGrade, fadt: &[Region]) {
    let mut seen = std::collections::BTreeSet::new();
    let accesses = g.accesses.clone();
    for a in &accesses {
        let inside = g.regions.iter().chain(fadt.iter()).any(|r| {
            r.space == Some(a.space)
                && a.addr >= r.base
                && a.addr + a.width as u64 <= r.base + r.length
        });
        if inside || !seen.insert((a.eval, a.space, a.addr)) {
            continue;
        }
        let raw = format!(
            "{} {} {:#x} width {} outside every declared region",
            if a.write { "write" } else { "read" },
            a.space.name(),
            a.addr,
            a.width
        );
        g.fail_access(a.eval, Mode::Access, raw, Vec::new(), Some(a.clone()));
    }
}
