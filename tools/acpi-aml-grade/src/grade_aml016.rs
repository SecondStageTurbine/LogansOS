// SPDX-License-Identifier: MPL-2.0
//! Grader for the standalone `aml` crate, 0.16.4 (`aml::AmlContext`).
//!
//! Driven only through the crate's public API: `AmlContext::new` with a
//! boxed recording `Handler`, `parse_table` per DSDT/SSDT body, the public
//! `namespace` for the walk, `invoke_method` per `_STA`/`_INI`, and
//! `initialize_objects` (the crate's own init) on a fresh context. The crate
//! is unmodified.

use crate::corpus::{CorpusSet, Table};
use crate::probe::{Call, HID_I2C_UUID, clip, eisa_id, parse_call};
use crate::recorder::{self, BUDGET_SENTINEL, Recorder, Space};
use crate::report::{CrateGrade, Mode, Phase, Region, check_regions, fadt_blocks};
use aml::value::{Args, MethodCode, RegionSpace};
use aml::{AmlContext, AmlName, AmlValue, DebugVerbosity, Handler};
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

pub const CRATE: &str = "aml";
pub const VERSION: &str = "0.16.4";

struct H16(Recorder);

impl Handler for H16 {
    fn read_u8(&self, a: usize) -> u8 {
        self.0.read(Space::SystemMemory, a as u64, None, 1) as u8
    }
    fn read_u16(&self, a: usize) -> u16 {
        self.0.read(Space::SystemMemory, a as u64, None, 2) as u16
    }
    fn read_u32(&self, a: usize) -> u32 {
        self.0.read(Space::SystemMemory, a as u64, None, 4) as u32
    }
    fn read_u64(&self, a: usize) -> u64 {
        self.0.read(Space::SystemMemory, a as u64, None, 8)
    }
    fn write_u8(&mut self, a: usize, v: u8) {
        self.0
            .write(Space::SystemMemory, a as u64, None, 1, v as u64)
    }
    fn write_u16(&mut self, a: usize, v: u16) {
        self.0
            .write(Space::SystemMemory, a as u64, None, 2, v as u64)
    }
    fn write_u32(&mut self, a: usize, v: u32) {
        self.0
            .write(Space::SystemMemory, a as u64, None, 4, v as u64)
    }
    fn write_u64(&mut self, a: usize, v: u64) {
        self.0.write(Space::SystemMemory, a as u64, None, 8, v)
    }
    fn read_io_u8(&self, p: u16) -> u8 {
        self.0.read(Space::SystemIo, p as u64, None, 1) as u8
    }
    fn read_io_u16(&self, p: u16) -> u16 {
        self.0.read(Space::SystemIo, p as u64, None, 2) as u16
    }
    fn read_io_u32(&self, p: u16) -> u32 {
        self.0.read(Space::SystemIo, p as u64, None, 4) as u32
    }
    fn write_io_u8(&self, p: u16, v: u8) {
        self.0.write(Space::SystemIo, p as u64, None, 1, v as u64)
    }
    fn write_io_u16(&self, p: u16, v: u16) {
        self.0.write(Space::SystemIo, p as u64, None, 2, v as u64)
    }
    fn write_io_u32(&self, p: u16, v: u32) {
        self.0.write(Space::SystemIo, p as u64, None, 4, v as u64)
    }
    fn read_pci_u8(&self, s: u16, b: u8, d: u8, f: u8, o: u16) -> u8 {
        self.0
            .read(Space::PciConfig, o as u64, Some((s, b, d, f)), 1) as u8
    }
    fn read_pci_u16(&self, s: u16, b: u8, d: u8, f: u8, o: u16) -> u16 {
        self.0
            .read(Space::PciConfig, o as u64, Some((s, b, d, f)), 2) as u16
    }
    fn read_pci_u32(&self, s: u16, b: u8, d: u8, f: u8, o: u16) -> u32 {
        self.0
            .read(Space::PciConfig, o as u64, Some((s, b, d, f)), 4) as u32
    }
    fn write_pci_u8(&self, s: u16, b: u8, d: u8, f: u8, o: u16, v: u8) {
        self.0
            .write(Space::PciConfig, o as u64, Some((s, b, d, f)), 1, v as u64)
    }
    fn write_pci_u16(&self, s: u16, b: u8, d: u8, f: u8, o: u16, v: u16) {
        self.0
            .write(Space::PciConfig, o as u64, Some((s, b, d, f)), 2, v as u64)
    }
    fn write_pci_u32(&self, s: u16, b: u8, d: u8, f: u8, o: u16, v: u32) {
        self.0
            .write(Space::PciConfig, o as u64, Some((s, b, d, f)), 4, v as u64)
    }
}

fn space_of(s: RegionSpace) -> (Option<Space>, String) {
    match s {
        RegionSpace::SystemMemory => (Some(Space::SystemMemory), "SystemMemory".into()),
        RegionSpace::SystemIo => (Some(Space::SystemIo), "SystemIO".into()),
        RegionSpace::PciConfig => (Some(Space::PciConfig), "PCI_Config".into()),
        other => (None, format!("{other:?}")),
    }
}

fn guarded<R>(f: impl FnOnce() -> R) -> Result<R, (Mode, String)> {
    recorder::clear_log_tail();
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => Ok(r),
        Err(_) => {
            let msg = recorder::take_panic().unwrap_or_else(|| "<panic>".to_string());
            let mode = if msg.contains(BUDGET_SENTINEL) {
                Mode::Budget
            } else {
                Mode::Panic
            };
            Err((mode, msg))
        }
    }
}

fn aml_tables(set: &CorpusSet) -> Vec<&Table> {
    set.dsdt().into_iter().chain(set.ssdts()).collect()
}

fn child(path: &str, seg: &str) -> String {
    if path == "\\" {
        format!("\\{seg}")
    } else {
        format!("{path}.{seg}")
    }
}

/// A rendering that keeps buffer bytes (a `_CRS` or `_DSM` result).
fn render_full(v: &AmlValue) -> String {
    match v {
        AmlValue::Buffer(b) => format!(
            "Buffer({})",
            b.lock()
                .iter()
                .map(|x| format!("{x:02x}"))
                .collect::<String>()
        ),
        other => render(other),
    }
}

/// A short, stable rendering of a returned value.
fn render(v: &AmlValue) -> String {
    match v {
        AmlValue::Integer(i) => format!("{i:#x}"),
        AmlValue::Boolean(b) => format!("{b}"),
        AmlValue::String(s) => format!("{s:?}"),
        other => format!("{:?}", other.type_of()),
    }
}

pub fn grade(set: &CorpusSet) -> CrateGrade {
    recorder::install_hooks();
    let mut g = CrateGrade::new(CRATE, VERSION, &set.name);
    let rec = Recorder::new();
    let setup = g.push_eval(Phase::Setup, "AmlContext::new");
    let mut ctx =
        match guarded(|| AmlContext::new(Box::new(H16(rec.clone())), DebugVerbosity::None)) {
            Ok(c) => c,
            Err((m, e)) => {
                g.fail(setup, m, e, recorder::log_tail());
                return g;
            }
        };
    g.evals[setup].ok = true;

    for t in aml_tables(set) {
        let i = g.push_eval(Phase::Load, &format!("{} ({})", t.signature, t.file));
        if std::env::var_os("ACPI_GRADE_LOUD_PANICS").is_some() {
            eprintln!("[acpi-aml-grade] {} load {}", CRATE, t.file);
        }
        rec.begin_eval(i);
        match guarded(|| ctx.parse_table(t.aml_body())) {
            Ok(Ok(())) => g.evals[i].ok = true,
            Ok(Err(e)) => g.fail(i, Mode::Error, format!("{e:?}"), recorder::log_tail()),
            Err((m, e)) => g.fail(i, m, e, recorder::log_tail()),
        }
    }

    let mut targets = Vec::new();
    let walk = guarded(|| {
        ctx.namespace.clone().traverse(|path, level| {
            let p = path.as_string();
            for seg in ["_STA", "_INI"] {
                if level.values.keys().any(|k| k.as_str() == seg) {
                    targets.push((
                        if seg == "_STA" {
                            Phase::Sta
                        } else {
                            Phase::Ini
                        },
                        child(&p, seg),
                    ));
                }
            }
            Ok(true)
        })
    });
    if !matches!(walk, Ok(Ok(()))) {
        g.notes
            .push(format!("namespace walk did not complete: {walk:?}"));
    }

    for (phase, path) in targets {
        let i = g.push_eval(phase, &path);
        if std::env::var_os("ACPI_GRADE_LOUD_PANICS").is_some() {
            eprintln!("[acpi-aml-grade] {} {} {path}", CRATE, phase.name());
        }
        rec.begin_eval(i);
        let r = guarded(|| {
            let name = AmlName::from_str(&path)?;
            ctx.invoke_method(&name, Args::EMPTY).map(|v| render(&v))
        });
        match r {
            Ok(Ok(v)) => {
                g.evals[i].ok = true;
                g.evals[i].result = Some(v);
            }
            Ok(Err(e)) => g.fail(i, Mode::Error, format!("{e:?}"), recorder::log_tail()),
            Err((m, e)) => g.fail(i, m, e, recorder::log_tail()),
        }
    }

    // Device probes (probes.json): the named methods, then the crate's own
    // resource decoder on each `_CRS` result.
    for probe in &set.probes {
        for m in &probe.methods {
            let target = format!("{}.{m}", probe.path);
            let i = g.push_eval(Phase::Probe, &target);
            rec.begin_eval(i);
            let r = guarded(|| {
                let (name, args) = match parse_call(m) {
                    Some(Call::Plain(n)) => (n, Args::EMPTY),
                    Some(Call::DsmHidI2c { rev, func }) => {
                        let uuid = AmlValue::Buffer(Arc::new(spinning_top::Spinlock::new(
                            HID_I2C_UUID.to_vec(),
                        )));
                        let mut a = Args::EMPTY;
                        a.0[0] = Some(uuid);
                        a.0[1] = Some(AmlValue::Integer(rev));
                        a.0[2] = Some(AmlValue::Integer(func));
                        a.0[3] = Some(AmlValue::Package(Vec::new()));
                        ("_DSM", a)
                    }
                    None => return Err(aml::AmlError::Unimplemented),
                };
                let path = AmlName::from_str(&format!("{}.{name}", probe.path))?;
                match ctx.invoke_method(&path, args) {
                    // Absent is an answer, not a failure (see grade_acpi6).
                    Err(aml::AmlError::ValueDoesNotExist(n)) if n == path => Ok(None),
                    other => other.map(Some),
                }
            });
            let value = match r {
                Ok(Ok(None)) => {
                    g.evals[i].ok = true;
                    g.evals[i].result = Some("(absent)".into());
                    None
                }
                Ok(Ok(Some(v))) => {
                    g.evals[i].ok = true;
                    let mut text = render_full(&v);
                    if let (AmlValue::Integer(n), true) = (&v, m == "_HID") {
                        text = format!("{text} = EISAID {}", eisa_id(*n));
                    }
                    g.evals[i].result = Some(clip(text, 300));
                    Some(v)
                }
                Ok(Err(e)) => {
                    g.fail(i, Mode::Error, format!("{e:?}"), recorder::log_tail());
                    None
                }
                Err((md, e)) => {
                    g.fail(i, md, e, recorder::log_tail());
                    None
                }
            };
            if let (Some(v), true) = (value, m == "_CRS") {
                let j = g.push_eval(
                    Phase::Probe,
                    &format!("{target} -> resource_descriptor_list"),
                );
                rec.begin_eval(j);
                match guarded(|| aml::resource::resource_descriptor_list(&v)) {
                    Ok(Ok(list)) => {
                        g.evals[j].ok = true;
                        g.evals[j].result = Some(clip(format!("{list:?}"), 400));
                    }
                    Ok(Err(e)) => g.fail(j, Mode::Error, format!("{e:?}"), recorder::log_tail()),
                    Err((md, e)) => g.fail(j, md, e, recorder::log_tail()),
                }
            }
        }
    }

    let mut regions = Vec::new();
    let mut methods = BTreeMap::new();
    let ns = ctx.namespace.clone();
    let _ = guarded(|| {
        ctx.namespace.clone().traverse(|path, level| {
            for (seg, handle) in level.values.iter() {
                if let Ok(AmlValue::Method {
                    code: MethodCode::Aml(code),
                    ..
                }) = ns.get(*handle)
                {
                    methods.insert(child(&path.as_string(), seg.as_str()), code.clone());
                }
                if let Ok(AmlValue::OpRegion {
                    region,
                    offset,
                    length,
                    ..
                }) = ns.get(*handle)
                {
                    let (space, space_name) = space_of(*region);
                    regions.push(Region {
                        space,
                        space_name,
                        base: *offset,
                        length: *length,
                        path: child(&path.as_string(), seg.as_str()),
                    });
                }
            }
            Ok(true)
        })
    });
    g.regions = regions;
    g.accesses = rec.log();
    check_regions(&mut g, &fadt_blocks(set));
    // Loads are classified against the table's own AML body.
    for t in aml_tables(set) {
        methods.insert(
            format!("{} ({})", t.signature, t.file),
            t.aml_body().to_vec(),
        );
    }
    g.finalize(&methods);

    // The crate's own init entry point, on a fresh context.
    let fresh_rec = Recorder::new();
    let mut fresh = AmlContext::new(Box::new(H16(fresh_rec)), DebugVerbosity::None);
    for t in aml_tables(set) {
        let _ = guarded(|| fresh.parse_table(t.aml_body()));
    }
    g.stock_init = match guarded(|| fresh.initialize_objects()) {
        Ok(Ok(())) => "initialize_objects() -> Ok".to_string(),
        Ok(Err(e)) => format!("initialize_objects() -> Err({e:?})"),
        Err((m, e)) => format!("initialize_objects() {}: {e}", m.name()),
    };
    g
}
