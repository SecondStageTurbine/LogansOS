// SPDX-License-Identifier: MPL-2.0
// Body of both `acpi` 6.1.1 graders, `include!`d by `grade_acpi6.rs` once
// per crate copy with `acpi` bound to that copy. See that file.

use crate::corpus::{CorpusSet, Table};
use crate::probe::{Call, HID_I2C_UUID, clip, eisa_id, parse_call};
use crate::recorder::{self, BUDGET_SENTINEL, Recorder, Space};
use crate::report::{CrateGrade, Kat, Mode, Phase, Region, check_regions, fadt_blocks};
use acpi::aml::Interpreter;
use acpi::aml::namespace::AmlName;
use acpi::aml::object::{Object, WrappedObject};
use acpi::aml::op_region::RegionSpace;
use acpi::registers::FixedRegisters;
use acpi::sdt::facs::Facs;
use acpi::sdt::fadt::Fadt;
use acpi::{Handle, Handler, PciAddress, PhysicalMapping};
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr::NonNull;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

/// Synthetic physical addresses for corpus tables, far above anything a
/// firmware OpRegion names, so a table mapping can never alias an AML access.
const TABLE_BASE: usize = 0x7e00_0000_0000;
const TABLE_STRIDE: usize = 0x10_0000;

/// Backing store for mappings. Tables live in leaked, 8-byte-aligned
/// buffers (the FACS holds an `AtomicU32`), padded so a short table never
/// lets a struct read run off the end (QEMU's FADT is 244 bytes, the
/// crate's `Fadt` is larger).
#[derive(Clone)]
struct H6 {
    rec: Recorder,
    tables: Arc<Mutex<BTreeMap<usize, (usize, usize)>>>,
}

fn leak_aligned(bytes: &[u8], min_len: usize) -> usize {
    let words = bytes.len().max(min_len).div_ceil(8) + 1;
    let buf: &'static mut [u64] = Box::leak(vec![0u64; words].into_boxed_slice());
    let ptr = buf.as_mut_ptr() as *mut u8;
    // SAFETY: `buf` has `words * 8 >= bytes.len()` writable bytes.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len()) };
    ptr as usize
}

impl H6 {
    fn new(rec: Recorder) -> Self {
        H6 {
            rec,
            tables: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    fn place(&self, index: usize, t: &Table) -> usize {
        let phys = TABLE_BASE + index * TABLE_STRIDE;
        let ptr = leak_aligned(&t.bytes, 4096);
        self.tables
            .lock()
            .unwrap()
            .insert(phys, (ptr, t.bytes.len().max(4096)));
        phys
    }
}

impl Handler for H6 {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<Self, T> {
        let hit = self
            .tables
            .lock()
            .unwrap()
            .range(..=physical_address)
            .next_back()
            .and_then(|(&base, &(ptr, len))| {
                (physical_address + size <= base + len).then_some(ptr + (physical_address - base))
            });
        let ptr = hit.unwrap_or_else(|| {
            // Not a corpus table: the crate is mapping a fixed-hardware
            // register (a SystemMemory GAS). Hand it zeroed scratch memory
            // and record the request; nothing real is mapped.
            self.rec.foreign_map(physical_address as u64, size);
            leak_aligned(&[], size.max(std::mem::size_of::<T>()).max(4096))
        });
        PhysicalMapping {
            physical_start: physical_address,
            virtual_start: NonNull::new(ptr as *mut T).unwrap(),
            region_length: size,
            mapped_length: size,
            handler: self.clone(),
        }
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {}

    fn read_u8(&self, a: usize) -> u8 {
        self.rec.read(Space::SystemMemory, a as u64, None, 1) as u8
    }
    fn read_u16(&self, a: usize) -> u16 {
        self.rec.read(Space::SystemMemory, a as u64, None, 2) as u16
    }
    fn read_u32(&self, a: usize) -> u32 {
        self.rec.read(Space::SystemMemory, a as u64, None, 4) as u32
    }
    fn read_u64(&self, a: usize) -> u64 {
        self.rec.read(Space::SystemMemory, a as u64, None, 8)
    }
    fn write_u8(&self, a: usize, v: u8) {
        self.rec
            .write(Space::SystemMemory, a as u64, None, 1, v as u64)
    }
    fn write_u16(&self, a: usize, v: u16) {
        self.rec
            .write(Space::SystemMemory, a as u64, None, 2, v as u64)
    }
    fn write_u32(&self, a: usize, v: u32) {
        self.rec
            .write(Space::SystemMemory, a as u64, None, 4, v as u64)
    }
    fn write_u64(&self, a: usize, v: u64) {
        self.rec.write(Space::SystemMemory, a as u64, None, 8, v)
    }
    fn read_io_u8(&self, p: u16) -> u8 {
        self.rec.read(Space::SystemIo, p as u64, None, 1) as u8
    }
    fn read_io_u16(&self, p: u16) -> u16 {
        self.rec.read(Space::SystemIo, p as u64, None, 2) as u16
    }
    fn read_io_u32(&self, p: u16) -> u32 {
        self.rec.read(Space::SystemIo, p as u64, None, 4) as u32
    }
    fn write_io_u8(&self, p: u16, v: u8) {
        self.rec.write(Space::SystemIo, p as u64, None, 1, v as u64)
    }
    fn write_io_u16(&self, p: u16, v: u16) {
        self.rec.write(Space::SystemIo, p as u64, None, 2, v as u64)
    }
    fn write_io_u32(&self, p: u16, v: u32) {
        self.rec.write(Space::SystemIo, p as u64, None, 4, v as u64)
    }
    fn read_pci_u8(&self, a: PciAddress, o: u16) -> u8 {
        self.rec.read(Space::PciConfig, o as u64, Some(pci(a)), 1) as u8
    }
    fn read_pci_u16(&self, a: PciAddress, o: u16) -> u16 {
        self.rec.read(Space::PciConfig, o as u64, Some(pci(a)), 2) as u16
    }
    fn read_pci_u32(&self, a: PciAddress, o: u16) -> u32 {
        self.rec.read(Space::PciConfig, o as u64, Some(pci(a)), 4) as u32
    }
    fn write_pci_u8(&self, a: PciAddress, o: u16, v: u8) {
        self.rec
            .write(Space::PciConfig, o as u64, Some(pci(a)), 1, v as u64)
    }
    fn write_pci_u16(&self, a: PciAddress, o: u16, v: u16) {
        self.rec
            .write(Space::PciConfig, o as u64, Some(pci(a)), 2, v as u64)
    }
    fn write_pci_u32(&self, a: PciAddress, o: u16, v: u32) {
        self.rec
            .write(Space::PciConfig, o as u64, Some(pci(a)), 4, v as u64)
    }
    fn nanos_since_boot(&self) -> u64 {
        self.rec.tick_ns(0)
    }
    fn stall(&self, us: u64) {
        self.rec.tick_ns(us.saturating_mul(1_000));
    }
    fn sleep(&self, ms: u64) {
        self.rec.tick_ns(ms.saturating_mul(1_000_000));
    }
    fn create_mutex(&self) -> Handle {
        Handle(self.rec.new_mutex())
    }
    fn acquire(&self, _m: Handle, _t: u16) -> Result<(), acpi::aml::AmlError> {
        Ok(())
    }
    fn release(&self, _m: Handle) {}
}

fn pci(a: PciAddress) -> (u16, u8, u8, u8) {
    (a.segment(), a.bus(), a.device(), a.function())
}

fn space_of(s: RegionSpace) -> (Option<Space>, String) {
    match s {
        RegionSpace::SystemMemory => (Some(Space::SystemMemory), "SystemMemory".into()),
        RegionSpace::SystemIO => (Some(Space::SystemIo), "SystemIO".into()),
        RegionSpace::PciConfig => (Some(Space::PciConfig), "PCI_Config".into()),
        other => (None, format!("{other:?}")),
    }
}

/// Run `f`, mapping a panic to `Err((Mode, message))`.
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

struct Built {
    interp: Interpreter<H6>,
    rec: Recorder,
}

/// Build an interpreter the way `new_from_platform` would, but from corpus
/// bytes: FADT -> `FixedRegisters`, FACS -> global-lock mapping.
fn build(set: &CorpusSet) -> Result<Built, String> {
    let facp = set.find("FACP").ok_or("set has no FACP (FADT)")?;
    let dsdt = set.dsdt().ok_or("set has no DSDT")?;
    build_from(facp, set.find("FACS"), dsdt.revision())
}

fn build_from(facp: &Table, facs: Option<&Table>, dsdt_revision: u8) -> Result<Built, String> {
    let rec = Recorder::new();
    let h = H6::new(rec.clone());
    let fadt_phys = h.place(0, facp);
    // SAFETY: `fadt_phys` maps a padded copy of the corpus FADT.
    let fadt: PhysicalMapping<H6, Fadt> =
        unsafe { h.map_physical_region(fadt_phys, facp.bytes.len()) };
    let regs =
        FixedRegisters::new(&fadt, h.clone()).map_err(|e| format!("FixedRegisters::new: {e:?}"))?;
    let facs = facs.map(|t| {
        let p = h.place(1, t);
        // SAFETY: as above, for the corpus FACS.
        unsafe { h.map_physical_region::<Facs>(p, t.bytes.len()) }
    });
    // `Interpreter::new` takes an `Arc<FixedRegisters>`; the harness is
    // single-threaded, and the crate allows the same lint internally.
    #[allow(clippy::arc_with_non_send_sync)]
    let regs = Arc::new(regs);
    let interp = Interpreter::new(h, dsdt_revision, regs, facs);
    Ok(Built { interp, rec })
}

/// Run a hand-laid AML known-answer test: build an interpreter on `facp`
/// (any corpus FADT), load `aml` (a table's AML body, no header) as a table
/// of revision `dsdt_revision`, then evaluate each of `calls` in order on
/// the same interpreter. Panics are caught and reported, as in the grade.
pub fn run_kat(facp: &Table, dsdt_revision: u8, aml: &[u8], calls: &[&str]) -> Kat {
    recorder::install_hooks();
    let b = match guarded(|| build_from(facp, None, dsdt_revision)) {
        Ok(Ok(b)) => b,
        Ok(Err(e)) | Err((_, e)) => {
            return Kat {
                load: Err(format!("setup: {e}")),
                results: Vec::new(),
                accesses: Vec::new(),
            };
        }
    };
    let outcome = |r: Result<Result<String, acpi::aml::AmlError>, (Mode, String)>| match r {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => Err(format!("{e:?}")),
        Err((m, e)) => Err(format!("{}: {e}", m.name())),
    };
    let load = outcome(guarded(|| b.interp.load_table(aml).map(|()| String::new()))).map(|_| ());
    let results = calls
        .iter()
        .map(|call| {
            outcome(guarded(|| {
                let name = AmlName::from_str(call)?;
                b.interp.evaluate(name, vec![]).map(|o| render(&o))
            }))
        })
        .collect();
    Kat {
        load,
        results,
        accesses: b.rec.log(),
    }
}

/// Build an interpreter for `set` the way the grade does, load its AML tables
/// (load errors are ignored, as the grade records them), then evaluate each
/// `(path, args)` call. Returns the crate's own objects, so a test can decode
/// them with that crate's types (OT-86 Fix 3: the touchpad cross-check).
pub fn evaluate_on_set(
    set: &CorpusSet,
    calls: Vec<(&str, Vec<WrappedObject>)>,
) -> Result<Vec<Result<WrappedObject, String>>, String> {
    recorder::install_hooks();
    let b = match guarded(|| build(set)) {
        Ok(r) => r?,
        Err((m, e)) => return Err(format!("{}: {e}", m.name())),
    };
    for t in aml_tables(set) {
        let _ = guarded(|| b.interp.load_table(t.aml_body()));
    }
    Ok(calls
        .into_iter()
        .map(|(path, args)| {
            match guarded(|| {
                let name = AmlName::from_str(path)?;
                b.interp.evaluate(name, args)
            }) {
                Ok(Ok(o)) => Ok(o),
                Ok(Err(e)) => Err(format!("{e:?}")),
                Err((m, e)) => Err(format!("{}: {e}", m.name())),
            }
        })
        .collect())
}

/// One fuzz case (`fuzz.rs`): load `aml` as a table, then evaluate every
/// method the load left in the namespace (up to `max_calls`, zero Integers
/// for their arguments), and decode any Buffer a call returns as a resource
/// template. Errors are expected and ignored; the outcome is the first crate
/// panic, if any. The harness's own access budget is not a crate panic.
pub fn fuzz_one(facp: &Table, dsdt_revision: u8, aml: &[u8], max_calls: usize) -> Option<String> {
    recorder::install_hooks();
    let panic = |r: Result<(), (Mode, String)>| match r {
        Err((Mode::Panic, msg)) => Some(msg),
        _ => None,
    };
    let b = match guarded(|| build_from(facp, None, dsdt_revision)) {
        Ok(Ok(b)) => b,
        Ok(Err(_)) => return None,
        Err((m, msg)) => return panic(Err((m, msg))),
    };
    if let Some(p) = panic(guarded(|| {
        let _ = b.interp.load_table(aml);
    })) {
        return Some(format!("load: {p}"));
    }
    let mut calls = Vec::new();
    let walk = guarded(|| {
        let mut ns = b.interp.namespace.lock().clone();
        let _ = ns.traverse(|path, level| {
            for (seg, (_, obj)) in level.values.iter() {
                if let Object::Method { flags, .. } = &**obj {
                    calls.push((child(&path.as_string(), seg.as_str()), flags.arg_count()));
                }
            }
            Ok(true)
        });
    });
    if let Some(p) = panic(walk) {
        return Some(format!("walk: {p}"));
    }
    for (path, argc) in calls.into_iter().take(max_calls) {
        let r = guarded(|| {
            let Ok(name) = AmlName::from_str(&path) else { return };
            let args = (0..argc).map(|_| WrappedObject::new(Object::Integer(0))).collect();
            if let Ok(v) = b.interp.evaluate(name, args)
                && let Object::Buffer(_) = *v
            {
                let _ = acpi::aml::resource::resource_descriptor_list(v);
            }
        });
        if let Some(p) = panic(r) {
            return Some(format!("{path}: {p}"));
        }
    }
    None
}

/// The crate's resource decoder on raw `_CRS` bytes (a resource KAT).
pub fn decode_resources_kat(bytes: &[u8]) -> Result<String, String> {
    recorder::install_hooks();
    match guarded(|| acpi::aml::resource::resource_descriptor_list(Object::Buffer(bytes.to_vec()).wrap())) {
        Ok(Ok(list)) => Ok(format!("{list:?}")),
        Ok(Err(e)) => Err(format!("{e:?}")),
        Err((m, e)) => Err(format!("{}: {e}", m.name())),
    }
}

fn aml_tables(set: &CorpusSet) -> Vec<&Table> {
    set.dsdt().into_iter().chain(set.ssdts()).collect()
}

/// A rendering that keeps buffer bytes (a `_CRS` or `_DSM` result).
fn render(o: &acpi::aml::object::WrappedObject) -> String {
    match &**o {
        Object::Buffer(b) => format!(
            "Buffer({})",
            b.iter().map(|x| format!("{x:02x}")).collect::<String>()
        ),
        other => format!("{other}"),
    }
}

fn child(path: &str, seg: &str) -> String {
    if path == "\\" {
        format!("\\{seg}")
    } else {
        format!("{path}.{seg}")
    }
}

pub fn grade(set: &CorpusSet) -> CrateGrade {
    recorder::install_hooks();
    let mut g = CrateGrade::new(CRATE, VERSION, &set.name);
    let setup = g.push_eval(Phase::Setup, "Interpreter::new");
    let b = match guarded(|| build(set)) {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => {
            g.fail(setup, Mode::Error, e, recorder::log_tail());
            return g;
        }
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
        b.rec.begin_eval(i);
        match guarded(|| b.interp.load_table(t.aml_body())) {
            Ok(Ok(())) => g.evals[i].ok = true,
            Ok(Err(e)) => g.fail(i, Mode::Error, format!("{e:?}"), recorder::log_tail()),
            Err((m, e)) => g.fail(i, m, e, recorder::log_tail()),
        }
    }

    // Every `_STA` and `_INI` in the namespace, parent before child, each
    // evaluated whatever its device's status (the rung grades
    // interpretation coverage, not boot policy).
    let mut targets = Vec::new();
    let walk = guarded(|| {
        let mut ns = b.interp.namespace.lock().clone();
        ns.traverse(|path, level| {
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
        b.rec.begin_eval(i);
        let r = guarded(|| {
            let name = AmlName::from_str(&path)?;
            b.interp.evaluate(name, vec![]).map(|o| format!("{}", *o))
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
            b.rec.begin_eval(i);
            let r = guarded(|| {
                let (name, args) = match parse_call(m) {
                    Some(Call::Plain(n)) => (n, vec![]),
                    Some(Call::DsmHidI2c { rev, func }) => (
                        "_DSM",
                        vec![
                            WrappedObject::new(Object::Buffer(HID_I2C_UUID.to_vec())),
                            WrappedObject::new(Object::Integer(rev)),
                            WrappedObject::new(Object::Integer(func)),
                            WrappedObject::new(Object::Package(vec![])),
                        ],
                    ),
                    None => return Err(acpi::aml::AmlError::LibUnimplemented),
                };
                let path = AmlName::from_str(&format!("{}.{name}", probe.path))?;
                b.interp.evaluate_if_present(path, args)
            });
            let value = match r {
                // Absent is an answer, not a failure: no `_STA` means present
                // and functioning (ACPI 6.5 §6.3.7).
                Ok(Ok(None)) => {
                    g.evals[i].ok = true;
                    g.evals[i].result = Some("(absent)".into());
                    None
                }
                Ok(Ok(Some(v))) => {
                    g.evals[i].ok = true;
                    let mut text = render(&v);
                    if let (Object::Integer(n), true) = (&*v, m == "_HID") {
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
                b.rec.begin_eval(j);
                match guarded(|| acpi::aml::resource::resource_descriptor_list(v)) {
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

    // OpRegions as the interpreter holds them.
    let mut regions = Vec::new();
    let mut methods = BTreeMap::new();
    let _ = guarded(|| {
        let mut ns = b.interp.namespace.lock().clone();
        ns.traverse(|path, level| {
            for (seg, (_, obj)) in level.values.iter() {
                if let Object::Method { code, .. } = &**obj {
                    methods.insert(child(&path.as_string(), seg.as_str()), code.clone());
                }
                if let Object::OpRegion(r) = &**obj {
                    let (space, space_name) = space_of(r.space);
                    regions.push(Region {
                        space,
                        space_name,
                        base: r.base,
                        length: r.length,
                        path: child(&path.as_string(), seg.as_str()),
                    });
                }
            }
            Ok(true)
        })
    });
    g.regions = regions;
    g.accesses = b.rec.log();
    for (addr, size) in b.rec.foreign_maps() {
        g.notes.push(format!(
            "crate mapped non-table physical {addr:#x}+{size:#x} (fixed-hardware GAS)"
        ));
    }
    check_regions(&mut g, &fadt_blocks(set));
    // Loads are classified against the table's own AML body.
    for t in aml_tables(set) {
        methods.insert(
            format!("{} ({})", t.signature, t.file),
            t.aml_body().to_vec(),
        );
    }
    g.finalize(&methods);

    // The crate's own init entry point, on a fresh interpreter.
    g.stock_init = match build(set) {
        Err(e) => format!("not run: {e}"),
        Ok(fresh) => {
            for t in aml_tables(set) {
                let _ = guarded(|| fresh.interp.load_table(t.aml_body()));
            }
            recorder::reset_warn_count();
            match guarded(|| fresh.interp.initialize_namespace()) {
                Ok(()) => format!(
                    "initialize_namespace() returned; {} warn/error log lines",
                    recorder::warn_count()
                ),
                Err((m, e)) => format!("initialize_namespace() {}: {e}", m.name()),
            }
        }
    };
    g
}
