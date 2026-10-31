// SPDX-License-Identifier: MPL-2.0
//! The recording handler's shared state. Every hardware access the
//! interpreter asks for is logged and answered from a scripted model; nothing
//! here ever touches real hardware, ports or physical memory.
//!
//! The scripted model is a zero-initialised, byte-granular write-back store
//! per address space: a read returns the bytes last written at that address,
//! or zero. Zero is "no device answers", the least-assuming reply, and makes
//! every run deterministic.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// Accesses allowed per evaluation before the run is declared
/// non-terminating. A firmware loop polling a status bit that the zero model
/// never sets would otherwise spin forever.
pub const ACCESS_BUDGET: u64 = 200_000;

/// Panic payload marker raised from inside the handler when the budget is spent.
pub const BUDGET_SENTINEL: &str = "acpi-aml-grade: access budget exhausted";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Space {
    SystemMemory,
    SystemIo,
    PciConfig,
}

impl Space {
    pub fn name(self) -> &'static str {
        match self {
            Space::SystemMemory => "SystemMemory",
            Space::SystemIo => "SystemIO",
            Space::PciConfig => "PCI_Config",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Access {
    pub space: Space,
    /// Absolute address (memory), port (I/O), or the config-space offset (PCI).
    pub addr: u64,
    /// For PCI: (segment, bus, device, function).
    pub pci: Option<(u16, u8, u8, u8)>,
    pub width: u8,
    pub write: bool,
    pub value: u64,
    /// Index of the evaluation (load / method) that issued it.
    pub eval: usize,
}

#[derive(Default)]
struct State {
    log: Vec<Access>,
    store: BTreeMap<(Space, u64, u64), u8>,
    budget_left: u64,
    clock_ns: u64,
    eval: usize,
    mutexes: u32,
    /// Physical-mapping requests for addresses that are not corpus tables
    /// (acpi 6.x maps SystemMemory fixed-hardware registers this way).
    foreign_maps: Vec<(u64, usize)>,
}

#[derive(Clone)]
pub struct Recorder(Arc<Mutex<State>>);

impl Default for Recorder {
    fn default() -> Self {
        Self::new()
    }
}

impl Recorder {
    pub fn new() -> Self {
        Recorder(Arc::new(Mutex::new(State {
            budget_left: ACCESS_BUDGET,
            ..Default::default()
        })))
    }

    /// Start a new evaluation: fresh budget, accesses tagged with `eval`.
    pub fn begin_eval(&self, eval: usize) {
        let mut s = self.0.lock().unwrap();
        s.eval = eval;
        s.budget_left = ACCESS_BUDGET;
    }

    fn key(pci: Option<(u16, u8, u8, u8)>) -> u64 {
        pci.map(|(s, b, d, f)| {
            ((s as u64) << 24) | ((b as u64) << 16) | ((d as u64) << 8) | f as u64
        })
        .unwrap_or(0)
    }

    fn charge(s: &mut State) -> bool {
        if s.budget_left == 0 {
            return false;
        }
        s.budget_left -= 1;
        true
    }

    pub fn read(&self, space: Space, addr: u64, pci: Option<(u16, u8, u8, u8)>, width: u8) -> u64 {
        let value;
        {
            let mut s = self.0.lock().unwrap();
            if !Self::charge(&mut s) {
                drop(s);
                panic!("{BUDGET_SENTINEL}");
            }
            let k = Self::key(pci);
            let mut v = 0u64;
            for i in 0..width as u64 {
                let b = s.store.get(&(space, k, addr + i)).copied().unwrap_or(0);
                v |= (b as u64) << (8 * i);
            }
            value = v;
            let eval = s.eval;
            s.log.push(Access {
                space,
                addr,
                pci,
                width,
                write: false,
                value,
                eval,
            });
        }
        value
    }

    pub fn write(
        &self,
        space: Space,
        addr: u64,
        pci: Option<(u16, u8, u8, u8)>,
        width: u8,
        value: u64,
    ) {
        let mut s = self.0.lock().unwrap();
        if !Self::charge(&mut s) {
            drop(s);
            panic!("{BUDGET_SENTINEL}");
        }
        let k = Self::key(pci);
        for i in 0..width as u64 {
            s.store
                .insert((space, k, addr + i), (value >> (8 * i)) as u8);
        }
        let eval = s.eval;
        s.log.push(Access {
            space,
            addr,
            pci,
            width,
            write: true,
            value,
            eval,
        });
    }

    /// A monotonic fake clock: every query advances it 1 ms, so firmware
    /// timeout loops (`Timer`, `Sleep`, `Stall`) terminate deterministically.
    pub fn tick_ns(&self, add: u64) -> u64 {
        let mut s = self.0.lock().unwrap();
        s.clock_ns = s.clock_ns.saturating_add(add.max(1_000_000));
        s.clock_ns
    }

    pub fn new_mutex(&self) -> u32 {
        let mut s = self.0.lock().unwrap();
        s.mutexes += 1;
        s.mutexes
    }

    pub fn foreign_map(&self, addr: u64, size: usize) {
        self.0.lock().unwrap().foreign_maps.push((addr, size));
    }

    pub fn log(&self) -> Vec<Access> {
        self.0.lock().unwrap().log.clone()
    }

    pub fn foreign_maps(&self) -> Vec<(u64, usize)> {
        self.0.lock().unwrap().foreign_maps.clone()
    }
}

// ---- panic and log capture -------------------------------------------------

thread_local! {
    static LAST_PANIC: RefCell<Option<String>> = const { RefCell::new(None) };
    static LOG_TAIL: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static WARN_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub fn reset_warn_count() {
    WARN_COUNT.with(|c| c.set(0));
}

pub fn warn_count() -> usize {
    WARN_COUNT.with(|c| c.get())
}

/// Silence the default panic printer and keep the message + location of the
/// latest panic on this thread, so a crate-internal `panic!`/`todo!` becomes
/// a classified failure instead of noise.
pub fn install_hooks() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = info.payload().downcast_ref::<String>() {
                s.clone()
            } else {
                "<non-string panic payload>".to_string()
            };
            let loc = info
                .location()
                .map(|l| format!("{}:{}", short_path(l.file()), l.line()))
                .unwrap_or_default();
            if std::env::var_os("ACPI_GRADE_LOUD_PANICS").is_some() {
                eprintln!("[acpi-aml-grade] panic: {msg} @ {loc}");
            }
            LAST_PANIC.with(|p| *p.borrow_mut() = Some(format!("{msg} @ {loc}")));
        }));
        let _ = log::set_logger(&CAPTURE);
        log::set_max_level(log::LevelFilter::Warn);
    });
}

/// `.../registry/src/<index>/acpi-6.1.1/src/aml/mod.rs` -> `acpi-6.1.1/src/aml/mod.rs`
/// (and `.../third_party/acpi-loganfy/src/aml/mod.rs` -> `acpi-loganfy/src/aml/mod.rs`),
/// with `/` separators on every host (a Windows build reports `\`), so the
/// committed report is byte-identical wherever it is regenerated.
pub fn short_path(p: &str) -> String {
    let p = p.replace('\\', "/");
    for krate in ["acpi-6.1.1", "acpi-loganfy", "aml-0.16.4"] {
        if let Some(i) = p.find(krate) {
            return p[i..].to_string();
        }
    }
    p
}

pub fn take_panic() -> Option<String> {
    LAST_PANIC.with(|p| p.borrow_mut().take())
}

pub fn clear_log_tail() {
    LOG_TAIL.with(|l| l.borrow_mut().clear());
}

pub fn log_tail() -> Vec<String> {
    LOG_TAIL.with(|l| l.borrow().clone())
}

struct Capture;
static CAPTURE: Capture = Capture;

impl log::Log for Capture {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Warn
    }
    fn log(&self, r: &log::Record) {
        WARN_COUNT.with(|c| c.set(c.get() + 1));
        let line = format!("{} {}: {}", r.level(), r.target(), r.args());
        LOG_TAIL.with(|l| {
            let mut l = l.borrow_mut();
            if l.len() >= 8 {
                l.remove(0);
            }
            l.push(line);
        });
    }
    fn flush(&self) {}
}

#[cfg(test)]
mod tests {
    use super::short_path;

    #[test]
    fn panic_sites_are_host_independent() {
        let win = r"C:\Users\x\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\acpi-6.1.1\src\aml\mod.rs";
        let unix =
            "/root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/acpi-6.1.1/src/aml/mod.rs";
        assert_eq!(short_path(win), "acpi-6.1.1/src/aml/mod.rs");
        assert_eq!(short_path(unix), "acpi-6.1.1/src/aml/mod.rs");
        let vendored_win =
            r"E:\w\tools\acpi-aml-grade\..\..\third_party\acpi-loganfy\src\aml\mod.rs";
        let vendored_unix = "/w/tools/acpi-aml-grade/../../third_party/acpi-loganfy/src/aml/mod.rs";
        assert_eq!(short_path(vendored_win), "acpi-loganfy/src/aml/mod.rs");
        assert_eq!(short_path(vendored_unix), "acpi-loganfy/src/aml/mod.rs");
    }
}
